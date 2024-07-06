/*
Write a db that is optimized for opening, finding by hash, listing.

Rocks db is too slow. It was taking ~100ms to open a db, and if we have > 10 vnodes,
that means we are taking > 1 second to open before doing any operations.

We can make this faster by using a simple file format.

Writing happens once at commit, then we read many times from the server and status.

Is also already sharded and optimized in the tree structure.
Reading, find by hash, listing is high throughput.

On Disk Format:

size
hash-int,data-offset,data-length
data
*/

use rmp_serde::Serializer;
use serde::{de, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use crate::error::OxenError;
use crate::util;

pub struct MerkleNodeLookup {
    pub size: u64,
    pub offsets: HashMap<u128, (u64, u64)>,
}

impl Default for MerkleNodeLookup {
    fn default() -> Self {
        Self::new()
    }
}

impl MerkleNodeLookup {
    pub fn new() -> Self {
        Self {
            size: 0,
            offsets: HashMap::new(),
        }
    }

    pub fn load(lookup_table_file: &mut File) -> Result<Self, OxenError> {
        // println!("Loading lookup table from {:?}", lookup_table_file);
        let mut file_data = Vec::new();
        lookup_table_file.read_to_end(&mut file_data)?;

        let mut cursor = std::io::Cursor::new(file_data);
        let mut buffer = [0u8; 8]; // u64 is 8 bytes
        cursor.read_exact(&mut buffer)?;
        let size = u64::from_le_bytes(buffer); // Use from_le_bytes or from_be_bytes based on endianness

        // println!("size: {}", size);

        let mut offsets: HashMap<u128, (u64, u64)> = HashMap::new();
        offsets.reserve(size as usize);

        for _ in 0..size {
            let mut buffer = [0u8; 16]; // u128 is 16 bytes
            cursor.read_exact(&mut buffer)?;
            let hash = u128::from_le_bytes(buffer);

            let mut buffer = [0u8; 8]; // u64 is 8 bytes
            cursor.read_exact(&mut buffer)?;
            let data_offset = u64::from_le_bytes(buffer);

            let mut buffer = [0u8; 8]; // u64 is 8 bytes
            cursor.read_exact(&mut buffer)?;
            let data_len = u64::from_le_bytes(buffer);

            offsets.insert(hash, (data_offset, data_len));
        }

        Ok(Self { size, offsets })
    }
}

pub struct MerkleNodeDB {
    read_only: bool,
    data_file: Option<File>,
    lookup_file: Option<File>,
    lookup: Option<MerkleNodeLookup>,
    size: u64,
    data_offset: u64,
}

impl MerkleNodeDB {
    pub fn size(&self) -> u64 {
        if let Some(lookup) = &self.lookup {
            return lookup.size;
        }

        self.size
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, OxenError> {
        Self::open(path, true)
    }

    pub fn open_read_write(path: impl AsRef<Path>) -> Result<Self, OxenError> {
        Self::open(path, false)
    }

    pub fn open(path: impl AsRef<Path>, read_only: bool) -> Result<Self, OxenError> {
        let path = path.as_ref();

        // mkdir if not exists
        if !path.exists() {
            util::fs::create_dir_all(path)?;
        }

        let lookup_path = path.join("lookup");
        let data_path = path.join("data");

        // println!("Opening merkle node db at {}", path.display());
        let (lookup, lookup_file, data_file): (
            Option<MerkleNodeLookup>,
            Option<File>,
            Option<File>,
        ) = if read_only {
            let mut lookup_file = File::open(lookup_path)?;
            let data_file = File::open(data_path)?;
            (
                Some(MerkleNodeLookup::load(&mut lookup_file)?),
                Some(lookup_file),
                Some(data_file),
            )
        } else {
            // self.lookup does not exist yet if we are writing (only write once)
            let lookup_file = File::create(lookup_path)?;
            let data_file = File::create(data_path)?;
            (None, Some(lookup_file), Some(data_file))
        };

        Ok(Self {
            read_only,
            data_file,
            lookup_file,
            lookup,
            size: 0,
            data_offset: 0,
        })
    }

    pub fn close(&mut self) -> Result<(), OxenError> {
        if let Some(data_file) = &mut self.data_file {
            data_file.flush()?;
            data_file.sync_data()?;
        } else {
            return Err(OxenError::basic_str("Must call open before closing"));
        }

        if let Some(lookup_file) = &mut self.lookup_file {
            lookup_file.flush()?;
            lookup_file.sync_data()?;
        } else {
            return Err(OxenError::basic_str("Must call open before closing"));
        }

        self.data_file = None;
        self.lookup_file = None;
        self.lookup = None;
        Ok(())
    }

    pub fn write_size(&mut self, size: u64) -> Result<(), OxenError> {
        if self.read_only {
            return Err(OxenError::basic_str("Cannot write to read-only db"));
        }

        if self.size > 0 {
            return Err(OxenError::basic_str("Cannot write size twice"));
        }

        if self.data_offset > 0 {
            return Err(OxenError::basic_str("Cannot write size after writing data"));
        }

        let Some(lookup_file) = self.lookup_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open before writing"));
        };

        // println!("Writing size: {}", size);
        let bytes = size.to_le_bytes();
        // println!("size: {:?}", bytes);
        lookup_file.write_all(&bytes)?;
        self.size = size;
        Ok(())
    }

    pub fn write_one<S: Serialize + Debug>(
        &mut self,
        hash: u128,
        item: &S,
    ) -> Result<(), OxenError> {
        if self.read_only {
            return Err(OxenError::basic_str("Cannot write to read-only db"));
        }

        if self.size == 0 {
            return Err(OxenError::basic_str(
                "Must call write_size() before writing",
            ));
        }

        let Some(lookup_file) = self.lookup_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open() before writing"));
        };
        let Some(data_file) = self.data_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open() before writing"));
        };

        // println!("---- {} {:x} -> {:?}", self.data_offset, hash, item);

        // TODO: Abstract and re-use in write_all
        let mut buf = Vec::new();
        item.serialize(&mut Serializer::new(&mut buf)).unwrap();

        let data_len = buf.len() as u64;
        lookup_file.write_all(&hash.to_le_bytes())?;
        lookup_file.write_all(&self.data_offset.to_le_bytes())?;
        lookup_file.write_all(&data_len.to_le_bytes())?;

        data_file.write_all(&buf)?;
        self.data_offset += data_len;

        Ok(())
    }

    pub fn write_all<S: Serialize>(&mut self, data: HashMap<u128, S>) -> Result<(), OxenError> {
        if self.read_only {
            return Err(OxenError::basic_str("Cannot write to read-only db"));
        }

        let Some(lookup_file) = self.lookup_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open before writing"));
        };
        let Some(data_file) = self.data_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open before writing"));
        };

        // Write the size of the data
        let size = data.len() as u64;
        lookup_file.write_all(&size.to_le_bytes())?;

        let mut data_offset: u64 = 0;
        // For each item,
        // write the hash,data-offset,data-len to the lookup table
        // then write the data to the data table
        for (hash, item) in data {
            let mut buf = Vec::new();
            item.serialize(&mut Serializer::new(&mut buf)).unwrap();

            let data_len = buf.len() as u64;
            lookup_file.write_all(&hash.to_le_bytes())?;
            lookup_file.write_all(&data_offset.to_le_bytes())?;
            lookup_file.write_all(&data_len.to_le_bytes())?;

            data_file.write_all(&buf)?;
            data_offset += data_len;
        }

        Ok(())
    }

    pub fn get<D>(&self, hash: u128) -> Result<D, OxenError>
    where
        D: de::DeserializeOwned,
    {
        let Some(lookup) = self.lookup.as_ref() else {
            return Err(OxenError::basic_str("Must call open before reading"));
        };

        let Some(mut data_file) = self.data_file.as_ref() else {
            return Err(OxenError::basic_str("Must call open before writing"));
        };

        // Find the offset and length of the data
        let Some(offset) = lookup.offsets.get(&hash) else {
            return Err(OxenError::basic_str("Cannot find hash in merkle node db"));
        };

        // Read from the data table at the offset
        // Allocate the exact amount of data
        let mut data = vec![0; offset.1 as usize];
        data_file.seek(SeekFrom::Start(offset.0))?;
        data_file.read_exact(&mut data)?;

        let val: D = rmp_serde::from_slice(&data).map_err(|e| {
            OxenError::basic_str(format!(
                "MerkleNodeDB.get({}): Error deserializing data: {:?}",
                hash, e
            ))
        })?;
        Ok(val)
    }

    pub fn list<D>(&mut self) -> Result<Vec<D>, OxenError>
    where
        D: de::DeserializeOwned,
    {
        let Some(lookup) = self.lookup.as_ref() else {
            return Err(OxenError::basic_str("Must call open before reading"));
        };
        let Some(data_file) = self.data_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open before writing"));
        };

        let mut ret: Vec<D> = Vec::new();
        // Iterate over offsets and read the data
        for (_, (offset, len)) in lookup.offsets.iter() {
            let mut data = vec![0; *len as usize];
            data_file.seek(SeekFrom::Start(*offset))?;
            data_file.read_exact(&mut data)?;
            let val: D = rmp_serde::from_slice(&data).unwrap();
            ret.push(val);
        }
        Ok(ret)
    }

    pub fn map<D>(&mut self) -> Result<HashMap<u128, D>, OxenError>
    where
        D: de::DeserializeOwned,
    {
        let Some(lookup) = self.lookup.as_ref() else {
            return Err(OxenError::basic_str("Must call open before reading"));
        };
        let Some(data_file) = self.data_file.as_mut() else {
            return Err(OxenError::basic_str("Must call open before writing"));
        };

        let mut file_data = Vec::new();
        data_file.read_to_end(&mut file_data)?;

        let mut ret: HashMap<u128, D> = HashMap::new();
        ret.reserve(lookup.size as usize);

        let mut cursor = std::io::Cursor::new(file_data);
        // Iterate over offsets and read the data
        for (hash, (offset, len)) in lookup.offsets.iter() {
            cursor.seek(SeekFrom::Start(*offset))?;
            let mut data = vec![0; *len as usize];
            cursor.read_exact(&mut data)?;
            let val: D = match rmp_serde::from_slice(&data) {
                Ok(val) => val,
                Err(e) => {
                    log::error!("Error deserializing data: {:?}", e);
                    return Err(OxenError::basic_str("Error deserializing data"));
                }
            };
            ret.insert(*hash, val);
        }

        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::index::commit_merkle_tree::{MerkleNode, MerkleNodeType};
    use crate::test;

    #[test]
    fn test_merkle_node_db() -> Result<(), OxenError> {
        test::run_empty_dir_test(|dir| {
            let mut writer_db = MerkleNodeDB::open_read_write(dir)?;
            writer_db.write_size(2)?;

            let node_readme = MerkleNode {
                dtype: MerkleNodeType::VNode,
                path: "README.md".to_string(),
            };
            writer_db.write_one(1234, &node_readme)?;

            let node_license = MerkleNode {
                dtype: MerkleNodeType::VNode,
                path: "LICENSE".to_string(),
            };
            writer_db.write_one(5678, &node_license)?;
            writer_db.close()?;

            let reader_db = MerkleNodeDB::open_read_only(dir)?;

            let size = reader_db.size();
            assert_eq!(size, 2);

            let data: MerkleNode = reader_db.get(1234)?;
            assert_eq!(data, node_readme);

            let data: MerkleNode = reader_db.get(5678)?;
            assert_eq!(data, node_license);

            Ok(())
        })
    }
}
