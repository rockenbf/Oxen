use std::path::{Path, PathBuf};

use futures::executor::LocalPool;
use polars::{frame::DataFrame, lazy::frame::LazyFrame};
use sql_query_builder::Select;

use crate::{
    api,
    constants::{CACHE_DIR, DUCKDB_CACHE_DIR, DUCKDB_DF_TABLE_NAME},
    core::db::df_db,
    error::OxenError,
    model::{schema::DataType, CommitEntry, LocalRepository, Schema},
    opts::PaginateOpts,
    util,
};

use super::tabular;

/// Module for handling the indexing of versioned dfs into duckdbs for SQL querying

pub fn db_cache_path(repo: &LocalRepository, entry: &CommitEntry) -> PathBuf {
    let hash_prefix = &entry.hash[0..2];
    let hash_suffix = &entry.hash[2..];
    let path = repo
        .path
        .join(CACHE_DIR)
        .join(DUCKDB_CACHE_DIR)
        .join(hash_prefix)
        .join(hash_suffix);
    path
}

pub fn get_conn(
    repo: &LocalRepository,
    entry: &CommitEntry,
) -> Result<duckdb::Connection, OxenError> {
    let duckdb_path = db_cache_path(repo, entry);
    let conn = df_db::get_connection(&duckdb_path)?;
    Ok(conn)
}

pub fn db_cache_dir(repo: &LocalRepository) -> PathBuf {
    repo.path.join(CACHE_DIR).join(DUCKDB_CACHE_DIR)
}

pub fn query_df(
    repo: &LocalRepository,
    entry: &CommitEntry,
    sql: String,
    page_opts: &PaginateOpts,
    conn: &mut duckdb::Connection,
) -> Result<DataFrame, OxenError> {
    let duckdb_path = db_cache_path(repo, entry);
    index_df(repo, entry, conn)?;

    // let limit_suffix = if page_opts.page_size > 0 {
    //     format!(" LIMIT {}", page_opts.page_size)
    // } else {
    //     "".to_string()
    // };

    // let offset_suffix = if page_opts.page_num > 0 {
    //     format!(" OFFSET {}", page_opts.page_size * (page_opts.page_num - 1))
    // } else {
    //     "".to_string()
    // };

    // // Strip a semicolon off the end of sql if it exists, then add limit suffix and offset suffix
    // let mut sql = sql.strip_suffix(";").unwrap_or(&sql).to_string();
    // sql.push_str(&limit_suffix);
    // sql.push_str(&offset_suffix);
    // sql.push_str(";");

    let conn = df_db::get_connection(&duckdb_path)?;
    log::debug!("connection created");

    let df = df_db::select_raw(&conn, &sql)?;
    log::debug!("got this query output");
    Ok(df)
}

pub fn text2sql_df(
    repo: &LocalRepository,
    entry: &CommitEntry,
    schema: &Schema,
    nlp: String,
    page_opts: &PaginateOpts,
    conn: &mut duckdb::Connection,
    host: String,
) -> Result<DataFrame, OxenError> {
    let sql = futures::executor::block_on(get_sql(schema, &nlp, host))?;
    println!("\n{}\n", sql);
    query_df(repo, entry, sql, page_opts, conn)
}

pub fn clear_all_cached_dfs(repo: &LocalRepository) -> Result<(), OxenError> {
    let db_cache = db_cache_dir(repo);
    if db_cache.exists() {
        std::fs::remove_dir_all(&db_cache)?;
    }
    Ok(())
}

pub fn clear_cached_df(repo: &LocalRepository, entry: &CommitEntry) -> Result<(), OxenError> {
    let duckdb_path = db_cache_path(repo, entry);
    if duckdb_path.exists() {
        std::fs::remove_file(&duckdb_path)?;
    }
    Ok(())
}

pub fn index_df(
    repo: &LocalRepository,
    entry: &CommitEntry,
    conn: &mut duckdb::Connection,
) -> Result<(), OxenError> {
    log::debug!("indexing df");
    let duckdb_path = db_cache_path(repo, entry);
    let default_parent = PathBuf::from("");
    let parent = duckdb_path.parent().unwrap_or(&default_parent);

    if df_db::table_exists(conn, DUCKDB_DF_TABLE_NAME)? {
        log::warn!(
            "index_df() file is already indexed at path {:?}",
            duckdb_path
        );
        return Ok(());
    }

    if !parent.exists() {
        util::fs::create_dir_all(&parent)?;
    }

    let version_path = util::fs::version_path(&repo, &entry);

    df_db::index_file(&version_path, &conn)?;

    log::debug!("file successfully indexed");

    Ok(())
}

async fn get_sql(schema: &Schema, q: &str, host: String) -> Result<String, OxenError> {
    let polars_schema = schema.to_polars();
    let schema_str = tabular::polars_schema_to_flat_str(&polars_schema);

    api::remote::text2sql::convert(q, &schema_str, Some(host.to_string())).await
}
