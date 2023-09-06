pub mod custom_data_type;
pub mod data_type;
pub mod field;

pub use custom_data_type::CustomDataType;
pub use data_type::DataType;
pub use field::Field;

use crate::util::hasher;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, path::PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Schema {
    pub name: Option<String>,
    pub hash: String,
    pub fields: Vec<Field>,
    // Optional string metadata on the schema, to allow for user driven features.
    pub metadata: Option<String>,
}

impl PartialEq for Schema {
    fn eq(&self, other: &Schema) -> bool {
        self.name == other.name && self.hash == other.hash && self.fields == other.fields
    }
}

impl Schema {
    pub fn new(name: impl AsRef<str>, fields: Vec<Field>) -> Schema {
        Schema {
            name: Some(name.as_ref().to_string()),
            hash: Schema::hash_fields(&fields),
            fields: fields.to_owned(),
            metadata: None,
        }
    }

    pub fn from_fields(fields: Vec<Field>) -> Schema {
        Schema {
            name: None,
            hash: Schema::hash_fields(&fields),
            fields: fields.to_owned(),
            metadata: None,
        }
    }

    pub fn to_polars(&self) -> polars::prelude::Schema {
        let mut schema = polars::prelude::Schema::new();
        for field in self.fields.iter() {
            let data_type = DataType::from_string(&field.dtype);
            schema.with_column(
                field.name.to_owned().into(),
                DataType::to_polars(&data_type),
            );
        }

        schema
    }

    pub fn from_polars(schema: &polars::prelude::Schema) -> Schema {
        let mut fields: Vec<Field> = vec![];
        for field in schema.iter_fields() {
            let f = Field::new(field.name(), field.data_type().to_string().as_str());
            fields.push(f);
        }

        Schema {
            name: None,
            hash: Schema::hash_fields(&fields),
            fields,
            metadata: None,
        }
    }

    /// Sets dtype_override on fields that match the name and dtype of the provided fields
    pub fn set_field_dtype_overrides(&mut self, fields: Vec<Field>) {
        for field in fields {
            log::debug!("set_field_dtype_overrides looking for field {:?}", field);
            if let Some(f) = self.fields.iter_mut().find(|f| f.name == field.name) {
                log::debug!("set_field_dtype_overrides updating override! {:?}", field);
                f.dtype_override = Some(field.dtype);
            }
        }
        self.hash = Schema::hash_fields(&self.fields);
    }

    /// Sets dtype_override on all fields from the provided schema
    pub fn set_field_dtype_overrides_from_schema(&mut self, schema: &Schema) {
        for field in schema.fields.iter() {
            if let Some(f) = self.fields.iter_mut().find(|f| f.name == field.name) {
                if field.dtype_override.is_some() {
                    f.dtype_override = field.dtype_override.clone();
                }

                if field.metadata.is_some() {
                    f.metadata = field.metadata.clone();
                }
            }
        }
        self.hash = Schema::hash_fields(&self.fields);
    }

    /// Add metadata to a column
    pub fn add_column_metadata(&mut self, name: &str, metadata: &str) {
        log::debug!("add_column_metadata {} {}", name, metadata);
        if let Some(f) = self.fields.iter_mut().find(|f| f.name == name) {
            f.metadata = Some(metadata.to_owned());
        }
        self.hash = Schema::hash_fields(&self.fields);
    }

    pub fn has_all_field_names(&self, schema: &polars::prelude::Schema) -> bool {
        log::debug!(
            "matches_polars checking size {} == {}",
            self.fields.len(),
            schema.len()
        );
        if self.fields.len() != schema.len() {
            // Print debug logic to help figure out why schemas don't match
            log::debug!("====schema.len {}====", schema.len());
            for field in schema.iter_fields() {
                log::debug!("schema.field: {}", field.name());
            }

            log::debug!("====self.fields.len {}====", self.fields.len());
            for field in self.fields.iter() {
                log::debug!("self.field: {}", field.name);
            }

            return false;
        }

        let mut has_all_fields = true;
        for field in schema.iter_fields() {
            if !self.has_field_name(&field.name) {
                has_all_fields = false;
                break;
            }
        }

        has_all_fields
    }

    pub fn has_field(&self, field: &Field) -> bool {
        self.fields
            .iter()
            .any(|f| f.name == field.name && f.dtype == field.dtype)
    }

    pub fn has_field_name(&self, name: &str) -> bool {
        self.fields.iter().any(|f| f.name == name)
    }

    pub fn get_field<S: AsRef<str>>(&self, name: S) -> Option<&Field> {
        let name = name.as_ref();
        self.fields.iter().find(|f| f.name == name)
    }

    fn hash_fields(fields: &Vec<Field>) -> String {
        let mut hash_buffers: Vec<String> = vec![];
        for f in fields {
            hash_buffers.push(format!("{}{}", f.name, f.dtype));
            if let Some(dtype_override) = &f.dtype_override {
                hash_buffers.push(dtype_override.to_string());
            }
            if let Some(metadata) = &f.metadata {
                hash_buffers.push(metadata.to_string());
            }
        }

        let buffer_str = hash_buffers.join("");
        let buffer = buffer_str.as_bytes();
        hasher::hash_buffer(buffer)
    }

    pub fn fields_to_csv(&self) -> String {
        self.fields.iter().map(|f| f.name.to_owned()).join(",")
    }

    pub fn fields_names(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.name.to_owned()).collect()
    }

    /// Compare the schemas, looking for added fields
    pub fn added_fields(&self, other: &Schema) -> Vec<Field> {
        let mut fields: Vec<Field> = vec![];

        // if field is in current schema but not in commit, it was added
        for current_field in self.fields.iter() {
            if !other.fields.iter().any(|f| f.name == current_field.name) {
                fields.push(current_field.clone());
            }
        }

        fields
    }

    // Compare the schemas, looking for removed fields
    pub fn removed_fields(&self, other: &Schema) -> Vec<Field> {
        let mut fields: Vec<Field> = vec![];

        // if field is in commit history but not in current, it was removed
        for commit_field in other.fields.iter() {
            if !self.fields.iter().any(|f| f.name == commit_field.name) {
                fields.push(commit_field.clone());
            }
        }

        fields
    }

    pub fn schemas_to_string(schemas: HashMap<PathBuf, Schema>) -> String {
        let mut table = comfy_table::Table::new();
        table.set_header(vec!["path", "name", "hash", "fields"]);

        for (path, schema) in schemas.iter() {
            let fields_str = Field::fields_to_string_with_limit(&schema.fields);
            if let Some(name) = &schema.name {
                table.add_row(vec![
                    path.to_string_lossy(),
                    name.into(),
                    schema.hash.clone().into(),
                    fields_str.into(),
                ]);
            } else {
                table.add_row(vec![
                    path.to_string_lossy(),
                    "?".into(),
                    schema.hash.clone().into(),
                    fields_str.into(),
                ]);
            }
        }
        table.to_string()
    }

    pub fn verbose_str(&self) -> String {
        let mut table = comfy_table::Table::new();
        table.set_header(vec!["name", "dtype", "dtype_override", "metadata"]);

        for field in self.fields.iter() {
            let mut row = vec![field.name.to_string(), field.dtype.to_string()];
            if let Some(val) = &field.dtype_override {
                row.push(val.to_owned())
            } else {
                row.push(String::from(""))
            }

            if let Some(val) = &field.metadata {
                row.push(val.to_owned())
            } else {
                row.push(String::from(""))
            }
            table.add_row(row);
        }
        format!("{}", table)
    }
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field_strs: Vec<String> = self
            .fields
            .iter()
            .map(|f| format!("{}:{}", f.name, f.dtype))
            .collect();
        let fields_str = field_strs.join(", ");
        write!(f, "{fields_str}")
    }
}

impl std::error::Error for Schema {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::model::schema::Field;
    use crate::model::schema::Schema;

    #[test]
    fn test_schemas_to_string_one_field() {
        let mut schemas = HashMap::new();
        schemas.insert(
            PathBuf::from("file.csv"),
            Schema {
                name: Some("bounding_box".to_string()),
                hash: "1234".to_string(),
                fields: vec![Field::new("file", "")],
                metadata: None,
            },
        );
        let table = Schema::schemas_to_string(schemas);
        println!("{}", table);
        assert_eq!(
            table,
            r"
+----------+--------------+------+--------+
| path     | name         | hash | fields |
+=========================================+
| file.csv | bounding_box | 1234 | [file] |
+----------+--------------+------+--------+"
                .trim()
        )
    }

    #[test]
    fn test_schemas_to_string_many_fields() {
        let mut schemas = HashMap::new();
        schemas.insert(
            PathBuf::from("another/file.csv"),
            Schema {
                name: Some("bounding_box".to_string()),
                hash: "1234".to_string(),
                fields: vec![
                    Field::new("file", "str"),
                    Field::new("x", "i64"),
                    Field::new("y", "i64"),
                    Field::new("w", "f64"),
                    Field::new("h", "f64"),
                ],
                metadata: None,
            },
        );
        let table = Schema::schemas_to_string(schemas);
        println!("{}", table);

        assert_eq!(
            table,
            r"
+------------------+--------------+------+----------------+
| path             | name         | hash | fields         |
+=========================================================+
| another/file.csv | bounding_box | 1234 | [file, ..., h] |
+------------------+--------------+------+----------------+"
                .trim()
        )
    }

    #[test]
    fn test_schemas_multiple_to_string_no_name() {
        let mut schemas = HashMap::new();

        schemas.insert(
            PathBuf::from("numero_uno.csv"),
            Schema {
                name: Some("bounding_box".to_string()),
                hash: "1234".to_string(),
                fields: vec![
                    Field::new("file", "str"),
                    Field::new("x", "i64"),
                    Field::new("y", "i64"),
                    Field::new("w", "f64"),
                    Field::new("h", "f64"),
                ],
                metadata: None,
            },
        );
        schemas.insert(
            PathBuf::from("numero_dos.csv"),
            Schema {
                name: None,
                hash: "5432".to_string(),
                fields: vec![Field::new("file", "str"), Field::new("x", "i64")],
                metadata: None,
            },
        );
        let table = Schema::schemas_to_string(schemas);
        println!("{}", table);

        assert_eq!(
            table,
            r"
+----------------+--------------+------+----------------+
| path           | name         | hash | fields         |
+=======================================================+
| numero_dos.csv | ?            | 5432 | [file, x]      |
|----------------+--------------+------+----------------|
| numero_uno.csv | bounding_box | 1234 | [file, ..., h] |
+----------------+--------------+------+----------------+"
                .trim()
        )
    }
}
