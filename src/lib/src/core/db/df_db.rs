//! Abstraction over DuckDB database to write and read dataframes from disk.
//!

use crate::constants::{DEFAULT_PAGE_SIZE, DUCKDB_DF_TABLE_NAME, OXEN_ID_COL};
use crate::core::db::df_db;
use crate::core::df::tabular;
use crate::error::OxenError;
use crate::model::schema::Field;
use crate::model::Schema;
use crate::opts::DFOpts;
use crate::{model, util};
use duckdb::arrow::record_batch::RecordBatch;
use duckdb::{params, ToSql};
use polars::prelude::*;
use std::io::Cursor;
use std::path::Path;

use sql_query_builder as sql;

/// Get a connection to a duckdb database.
pub fn get_connection(path: impl AsRef<Path>) -> Result<duckdb::Connection, OxenError> {
    let path = path.as_ref();
    // Create parent path if not exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let conn = duckdb::Connection::open(path)?;
    Ok(conn)
}

/// Create a table in a duckdb database based on an oxen schema.
pub fn create_table_if_not_exists(
    conn: &duckdb::Connection,
    schema: &Schema,
) -> Result<String, OxenError> {
    match &schema.name {
        Some(table_name) => p_create_table_if_not_exists(conn, table_name, &schema.fields),
        None => Err(OxenError::basic_str("Schema name is required")),
    }
}

/// Drop a table in a duckdb database.
pub fn drop_table(conn: &duckdb::Connection, table_name: impl AsRef<str>) -> Result<(), OxenError> {
    let table_name = table_name.as_ref();
    let sql = format!("DROP TABLE IF EXISTS {}", table_name);
    log::debug!("drop_table sql: {}", sql);
    conn.execute(&sql, []).map_err(OxenError::from)?;
    Ok(())
}

pub fn table_exists(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
) -> Result<bool, OxenError> {
    log::debug!("checking exists in path {:?}", conn);
    let table_name = table_name.as_ref();
    let sql = "SELECT EXISTS (SELECT 1 FROM duckdb_tables WHERE table_name = ?) AS table_exists";
    let mut stmt = conn.prepare(sql)?;
    let exists: bool = stmt.query_row(params![table_name], |row| row.get(0))?;
    log::debug!("got exists: {}", exists);
    Ok(exists)
}

/// Create a table from a set of oxen fields with data types.
fn p_create_table_if_not_exists(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
    fields: &[Field],
) -> Result<String, OxenError> {
    let table_name = table_name.as_ref();
    let columns: Vec<String> = fields.iter().map(|f| f.to_sql()).collect();
    let columns = columns.join(" NOT NULL,\n");
    let sql = format!("CREATE TABLE IF NOT EXISTS {} (\n{});", table_name, columns);
    log::debug!("create_table sql: {}", sql);
    conn.execute(&sql, [])?;
    Ok(table_name.to_owned())
}

/// Get the schema from the table.
pub fn get_schema(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
) -> Result<Schema, OxenError> {
    let table_name = table_name.as_ref();
    let sql = format!(
        "SELECT column_name, data_type FROM information_schema.columns WHERE table_name == '{}'",
        table_name
    );
    let mut stmt = conn.prepare(&sql)?;

    let mut fields = vec![];
    let rows = stmt.query_map([], |row| {
        let column_name: String = row.get(0)?;
        let data_type: String = row.get(1)?;

        Ok((column_name, data_type))
    })?;

    for row in rows {
        let (column_name, data_type) = row?;
        fields.push(Field::new(
            &column_name,
            model::schema::DataType::from_sql(data_type).as_str(),
        ));
    }

    Ok(Schema::new(table_name, fields))
}

pub fn get_schema_excluding_cols(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
    cols: &[&str],
) -> Result<Schema, OxenError> {
    let table_name = table_name.as_ref();
    let sql = format!(
        "SELECT column_name, data_type FROM information_schema.columns WHERE table_name == '{}' AND column_name NOT IN ({})",
        table_name, cols.iter().map(|col| format!("'{}'", col.replace('\'', "''"))).collect::<Vec<String>>().join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;

    let select = sql::Select::new().select("*").from(table_name);
    // let mut s_select = conn.prepare(&select.as_string())?;
    let _records = df_db::select(conn, &select)?;

    let mut fields = vec![];
    let rows = stmt.query_map([], |row| {
        let column_name: String = row.get(0)?;
        let data_type: String = row.get(1)?;

        Ok((column_name, data_type))
    })?;

    for row in rows {
        let (column_name, data_type) = row?;
        fields.push(Field::new(
            &column_name,
            model::schema::DataType::from_sql(data_type).as_str(),
        ));
    }

    Ok(Schema::new(table_name, fields))
}

// pub fn get_schema_without_id(
//     conn: &duckdb::Connection,
//     table_name: impl AsRef<str>,
// ) -> Result<Schema, OxenError> {
//     let table_name = table_name.as_ref();
//     let sql = format!(
//         "SELECT column_name, data_type FROM information_schema.columns WHERE table_name == '{}' AND column_name != '{}'",
//         table_name, OXEN_ID_COL
//     );
//     let mut stmt = conn.prepare(&sql)?;

//     let select = sql::Select::new().select("*").from(table_name);
//     // let mut s_select = conn.prepare(&select.as_string())?;
//     let _records = df_db::select(conn, &select)?;

//     let mut fields = vec![];
//     let rows = stmt.query_map([], |row| {
//         let column_name: String = row.get(0)?;
//         let data_type: String = row.get(1)?;

//         Ok((column_name, data_type))
//     })?;

//     for row in rows {
//         let (column_name, data_type) = row?;
//         fields.push(Field::new(
//             &column_name,
//             model::schema::DataType::from_sql(data_type).as_str(),
//         ));
//     }

//     Ok(Schema::new(table_name, fields))
// }

/// Query number of rows in a table.
pub fn count(conn: &duckdb::Connection, table_name: impl AsRef<str>) -> Result<usize, OxenError> {
    let table_name = table_name.as_ref();
    let sql = format!("SELECT count(*) FROM {}", table_name);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let size: usize = row.get(0)?;
        Ok(size)
    } else {
        Err(OxenError::basic_str(format!(
            "No rows in table {}",
            table_name
        )))
    }
}

/// Query number of rows in a table.
pub fn count_where(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
    where_clause: impl AsRef<str>,
) -> Result<usize, OxenError> {
    let table_name = table_name.as_ref();
    let where_clause = where_clause.as_ref();
    let sql = format!("SELECT count(*) FROM {} WHERE {}", table_name, where_clause);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let size: usize = row.get(0)?;
        Ok(size)
    } else {
        Err(OxenError::basic_str(format!(
            "No rows in table {}",
            table_name
        )))
    }
}

/// Select fields from a table.
pub fn select(conn: &duckdb::Connection, stmt: &sql::Select) -> Result<DataFrame, OxenError> {
    let sql = stmt.as_string();
    let df = select_raw(conn, &sql)?;
    Ok(df)
}

pub fn select_raw(conn: &duckdb::Connection, stmt: &str) -> Result<DataFrame, OxenError> {
    log::debug!("select_raw() sql: {}", stmt);
    let mut stmt = conn.prepare(stmt)?;

    // let pl: Vec<DataFrame> = stmt.query_polars([])?.collect();
    // let df = accumulate_dataframes_vertical_unchecked(pl);

    let records: Vec<RecordBatch> = stmt.query_arrow([])?.collect();
    log::debug!("got records: {:?}", records.len());

    if records.is_empty() {
        return Ok(DataFrame::default());
    }

    // Hacky to convert to json and then to polars...but the results from these queries should be small, and
    // if they are bigger, need to look into converting directly from arrow to polars.

    // Convert to Vec<&RecordBatch>
    let records: Vec<&RecordBatch> = records.iter().collect::<Vec<_>>();
    let buf = Vec::new();
    let mut writer = arrow_json::writer::ArrayWriter::new(buf);
    writer.write_batches(&records[..]).unwrap();
    writer.finish().unwrap();
    let json_bytes = writer.into_inner();

    let content = Cursor::new(json_bytes);
    let df = JsonReader::new(content).finish().unwrap();

    log::debug!("result df: {:?}", df);
    Ok(df)
}

pub fn select_with_opts(
    conn: &duckdb::Connection,
    stmt: &sql::Select,
    opts: &DFOpts,
) -> Result<DataFrame, OxenError> {
    let mut sql = stmt.as_string();

    if let Some(sort_by) = &opts.sort_by {
        sql.push_str(&format!(" ORDER BY {}", sort_by));
    }

    let pagination_clause = if let Some(page) = opts.page {
        let page = if page == 0 { 1 } else { page };
        let page_size = opts.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        format!(" LIMIT {} OFFSET {}", page_size, (page - 1) * page_size)
    } else {
        format!(" LIMIT {}", DEFAULT_PAGE_SIZE)
    };

    sql.push_str(&pagination_clause);

    log::debug!("select sql with opts: {}", sql);
    let mut stmt = conn.prepare(&sql)?;
    let records: Vec<RecordBatch> = stmt.query_arrow([])?.collect();
    log::debug!("got records with opts: {:?}", records.len());

    if records.is_empty() {
        return Ok(DataFrame::default());
    }
    // Convert to Vec<&RecordBatch>
    let records: Vec<&RecordBatch> = records.iter().collect::<Vec<_>>();

    // Write to json
    let buf = Vec::new();
    let mut writer = arrow_json::writer::ArrayWriter::new(buf);
    writer.write_batches(&records[..])?;
    writer.finish()?;

    log::debug!("Here are the actual records: {:?}", records);
    let json = writer.into_inner();

    log::debug!("got json: {:?}", json);

    let json_str = String::from_utf8(json)?;

    let content = Cursor::new(json_str.as_bytes());
    let df = JsonReader::new(content).finish().unwrap();

    log::debug!("got df from jsonreader: {:?}", df);

    Ok(df)
}

/// Insert a row from a polars dataframe into a duckdb table.
pub fn insert_polars_df(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
    df: &DataFrame,
) -> Result<DataFrame, OxenError> {
    let table_name = table_name.as_ref();

    let schema = df.schema();
    let column_names: Vec<String> = schema
        .iter_fields()
        .map(|f| format!("\"{}\"", f.name()))
        .collect();

    log::debug!("column names are {:?}", column_names);
    let placeholders: String = column_names
        .iter()
        .map(|_| "?".to_string())
        .collect::<Vec<_>>()
        .join(", ");
    log::debug!("placeholders are {}", placeholders);
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) RETURNING *, {}",
        table_name,
        column_names.join(", "),
        placeholders,
        OXEN_ID_COL
    );
    log::debug!("sql statement is {}", sql);

    let mut stmt = conn.prepare(&sql)?;

    // TODONOW: THIS SHOULD BULK INSERT!
    let mut result_df = DataFrame::default();
    for idx in 0..df.height() {
        let row = df.get(idx).unwrap();
        let boxed_values: Vec<Box<dyn ToSql>> = row
            .iter()
            .map(|v| tabular::value_to_tosql(v.to_owned()))
            .collect();

        let params: Vec<&dyn ToSql> = boxed_values
            .iter()
            .map(|boxed_value| &**boxed_value as &dyn ToSql)
            .collect();

        let result_set: Vec<RecordBatch> = stmt.query_arrow(params.as_slice())?.collect();
        let result_set: Vec<&RecordBatch> = result_set.iter().collect();
        let json = arrow_json::writer::record_batches_to_json_rows(&result_set[..]).unwrap();
        log::debug!("got json: {:?}", json);

        let json_str = serde_json::to_string(&json).unwrap();
        log::debug!("got json str: {:?}", json_str);

        let content = Cursor::new(json_str.as_bytes());
        let df = polars::io::json::JsonReader::new(content).finish().unwrap();

        result_df = if df.height() == 0 {
            df
        } else {
            result_df.vstack(&df).unwrap()
        };
    }

    log::debug!("returning df {:?} on add_row", result_df);

    Ok(result_df)
}

pub fn modify_row_with_polars_df(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
    id: &str,
    df: &DataFrame,
) -> Result<DataFrame, OxenError> {
    if df.height() != 1 {
        return Err(OxenError::basic_str(
            "df must have exactly one row to be used for modification",
        ));
    }

    let table_name = table_name.as_ref();

    let schema = df.schema();
    let column_names: Vec<String> = schema
        .iter_fields()
        .map(|f| format!("\"{}\"", f.name()))
        .collect();

    let set_clauses: String = column_names
        .iter()
        .map(|col_name| format!("{} = ?", col_name))
        .collect::<Vec<String>>()
        .join(", ");

    log::debug!("set clauses are {}", set_clauses);

    let where_clause = format!("\"{}\" = '{}'", OXEN_ID_COL, id);
    let sql = format!(
        "UPDATE {} SET {} WHERE {} RETURNING *",
        table_name, set_clauses, where_clause
    );

    let values = df.get(0).unwrap(); // Checked above

    log::debug!("df is {:?}", df);
    log::debug!("the values are {:?}", values);

    let boxed_values: Vec<Box<dyn ToSql>> = values
        .iter()
        .map(|v| tabular::value_to_tosql(v.to_owned()))
        .collect();

    let params: Vec<&dyn ToSql> = boxed_values
        .iter()
        .map(|boxed_value| &**boxed_value as &dyn ToSql)
        .collect();

    let mut stmt = conn.prepare(&sql)?;
    let result_set: Vec<RecordBatch> = stmt.query_arrow(params.as_slice())?.collect();
    let result_set: Vec<&RecordBatch> = result_set.iter().collect();
    let json = arrow_json::writer::record_batches_to_json_rows(&result_set[..]).unwrap();
    log::debug!("got json: {:?}", json);

    let json_str = serde_json::to_string(&json).unwrap();
    log::debug!("got json str: {:?}", json_str);

    let content = Cursor::new(json_str.as_bytes());
    let df = polars::io::json::JsonReader::new(content).finish().unwrap();

    Ok(df)
}

pub fn index_file(path: &Path, conn: &duckdb::Connection) -> Result<(), OxenError> {
    log::debug!("df_db:index_file() at path {:?}", path);
    let extension: &str = &util::fs::extension_from_path(path);
    let path_str = path.to_string_lossy().to_string();
    match extension {
        "csv" => {
            let query = format!(
                "CREATE TABLE {} AS SELECT * FROM read_csv('{}')",
                DUCKDB_DF_TABLE_NAME, path_str
            );
            conn.execute(&query, [])?;
        }
        "tsv" => {
            let query = format!(
                "CREATE TABLE {} AS SELECT * FROM read_csv('{}')",
                DUCKDB_DF_TABLE_NAME, path_str
            );
            conn.execute(&query, [])?;
        }
        "parquet" => {
            let query = format!(
                "CREATE TABLE {} AS SELECT * FROM read_parquet('{}')",
                DUCKDB_DF_TABLE_NAME, path_str
            );
            conn.execute(&query, [])?;
        }
        "jsonl" | "json" | "ndjson" => {
            let query = format!(
                "CREATE TABLE {} AS SELECT * FROM read_json('{}')",
                DUCKDB_DF_TABLE_NAME, path_str
            );
            conn.execute(&query, [])?;
        }
        _ => {
            return Err(OxenError::basic_str(
                "Invalid file type: expected .csv, .tsv, .parquet, .jsonl, .json, .ndjson",
            ))
        }
    }
    Ok(())
}

// TODO: We will eventually want to parse the actual type, not just the extension.
// For now, just treat the extension as law
pub fn index_file_with_id(path: &Path, conn: &duckdb::Connection) -> Result<(), OxenError> {
    log::debug!("df_db:index_file() at path {:?} into path {:?}", path, conn);
    let extension: &str = &util::fs::extension_from_path(path);
    let path_str = path.to_string_lossy().to_string();
    match extension {
        "csv" => {
            let query = format!("CREATE TABLE {} AS SELECT *, CAST(uuid() AS VARCHAR) AS {} FROM read_csv('{}', AUTO_DETECT=TRUE, header=True);", DUCKDB_DF_TABLE_NAME, OXEN_ID_COL, path.to_string_lossy());
            conn.execute(&query, [])?;
        }
        "tsv" => {
            let query = format!("CREATE TABLE {} AS SELECT *, CAST(uuid() AS VARCHAR) AS {} FROM read_csv('{}', AUTO_DETECT=TRUE, header=True);", DUCKDB_DF_TABLE_NAME, OXEN_ID_COL, path.to_string_lossy());
            conn.execute(&query, [])?;
        }
        "parquet" => {
            let query = format!("CREATE TABLE {} AS SELECT *, CAST(uuid() AS VARCHAR) AS {} FROM read_parquet('{}');", DUCKDB_DF_TABLE_NAME, OXEN_ID_COL, path.to_string_lossy());
            conn.execute(&query, [])?;
        }
        "jsonl" | "json" | "ndjson" => {
            let query = format!(
                "CREATE TABLE {} AS SELECT *, CAST(uuid() AS VARCHAR) AS {} FROM read_json('{}');",
                DUCKDB_DF_TABLE_NAME, OXEN_ID_COL, path_str
            );
            conn.execute(&query, [])?;
        }
        _ => {
            return Err(OxenError::basic_str(
                "Invalid file type: expected .csv, .tsv, .parquet, .jsonl, .json, .ndjson",
            ))
        }
    }

    let add_default_query = format!(
        "ALTER TABLE {} ALTER COLUMN {} SET DEFAULT CAST(uuid() AS VARCHAR);",
        DUCKDB_DF_TABLE_NAME, OXEN_ID_COL
    );
    conn.execute(&add_default_query, [])?;

    Ok(())
}

pub fn from_clause_from_disk_path(path: &Path) -> Result<String, OxenError> {
    let extension: &str = &util::fs::extension_from_path(path);
    match extension {
        "csv" => {
            let str_path = path.to_string_lossy().to_string();
            Ok(format!("read_csv('{}')", str_path))
        }
        "tsv" => {
            let str_path = path.to_string_lossy().to_string();
            Ok(format!("read_csv('{}')", str_path))
        }
        "parquet" => {
            let str_path = path.to_string_lossy().to_string();
            Ok(format!("read_parquet('{}')", str_path))
        }
        "jsonl" | "json" | "ndjson" => {
            let str_path = path.to_string_lossy().to_string();
            Ok(format!("read_json('{}')", str_path))
        }
        _ => Err(OxenError::basic_str(
            "Invalid file type: expected .csv, .tsv, .parquet, .jsonl, .json, .ndjson",
        )),
    }
}

pub fn preview(
    conn: &duckdb::Connection,
    table_name: impl AsRef<str>,
) -> Result<DataFrame, OxenError> {
    let table_name = table_name.as_ref();
    let query = format!("SELECT * FROM {} LIMIT 10", table_name);
    let df = select_raw(conn, &query)?;
    Ok(df)
}

#[cfg(test)]
mod tests {
    use crate::test;
    // use sql_query_builder as sql;

    use super::*;

    /*
    #[test]
    fn test_df_db_count() -> Result<(), OxenError> {
        // TODO: Create this db file in a temp dir
        let db_file = Path::new("data")
            .join("test")
            .join("db")
            .join("metadata.db");
        let conn = get_connection(db_file)?;

        let count = count(&conn, "metadata")?;

        assert_eq!(count, 16);

        Ok(())
    }

    #[test]
    fn test_df_db_select() -> Result<(), OxenError> {
        let db_file = Path::new("data")
            .join("test")
            .join("db")
            .join("metadata.db");
        let conn = get_connection(db_file)?;

        let offset = 0;
        let limit = 7;
        let fields = ["filename", "data_type"];

        let stmt = sql::Select::new()
            .select(&fields.join(", "))
            .offset(&offset.to_string())
            .limit(&limit.to_string())
            .from("metadata");

        let df = select(&conn, &stmt)?;

        assert!(df.width() == fields.len());
        assert!(df.height() == limit);

        Ok(())
    }
     */

    #[test]
    fn test_df_db_create() -> Result<(), OxenError> {
        test::run_empty_dir_test(|data_dir| {
            let db_file = data_dir.join("data.db");
            let conn = get_connection(db_file)?;
            // bounding_box -> min_x, min_y, width, height
            let schema = test::schema_bounding_box();
            create_table_if_not_exists(&conn, &schema)?;

            let num_entries = count(&conn, schema.name.unwrap())?;
            assert_eq!(num_entries, 0);

            Ok(())
        })
    }

    #[test]
    fn test_df_db_get_schema() -> Result<(), OxenError> {
        test::run_empty_dir_test(|data_dir| {
            let db_file = data_dir.join("data.db");
            let conn = get_connection(db_file)?;
            // bounding_box -> min_x, min_y, width, height
            let schema = test::schema_bounding_box();
            create_table_if_not_exists(&conn, &schema)?;

            let name = &schema.name.clone().unwrap();
            let found_schema = get_schema(&conn, name)?;
            assert_eq!(found_schema, schema);

            Ok(())
        })
    }
}
