use crate::error::OxenError;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use xxhash_rust::xxh3::xxh3_128;

pub fn hash_buffer(buffer: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    let val = xxh3_128(buffer);
    val.hash(&mut hasher);
    format!("{:X}", hasher.finish())
}

pub fn hash_filename(path: &Path) -> String {
    let name = path.to_str().unwrap();
    let bytes = name.as_bytes();
    hash_buffer(bytes)
}

pub fn hash_buffer_128bit(buffer: &[u8]) -> u128 {
    xxh3_128(buffer)
}

pub fn hash_file_contents(path: &Path) -> Result<String, OxenError> {
    match File::open(path) {
        Ok(file) => {
            let mut reader = BufReader::new(file);
            let mut buffer = Vec::new();
            match reader.read_to_end(&mut buffer) {
                Ok(_) => {
                    let result = hash_buffer(&buffer);
                    Ok(result)
                }
                Err(_) => {
                    eprintln!("Could not read file to end {:?}", path);
                    Err(OxenError::basic_str("Could not read file to end"))
                }
            }
        }
        Err(_) => {
            let err = format!(
                "util::hasher::hash_file_contents Could not open file {:?}",
                path
            );
            Err(OxenError::basic_str(&err))
        }
    }
}

pub fn hash_file_contents_128bit(path: &Path) -> Result<u128, OxenError> {
    match File::open(path) {
        Ok(file) => {
            let mut reader = BufReader::new(file);
            let mut buffer = Vec::new();
            match reader.read_to_end(&mut buffer) {
                Ok(_) => {
                    let result = hash_buffer_128bit(&buffer);
                    Ok(result)
                }
                Err(_) => {
                    eprintln!("Could not read file to end {:?}", path);
                    Err(OxenError::basic_str("Could not read file to end"))
                }
            }
        }
        Err(_) => {
            let err = format!(
                "util::hasher::hash_file_contents Could not open file {:?}",
                path
            );
            Err(OxenError::basic_str(&err))
        }
    }
}
