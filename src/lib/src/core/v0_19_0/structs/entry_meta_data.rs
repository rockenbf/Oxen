use crate::model::{EntryDataType, StagedEntryStatus};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EntryMetaData {
    pub hash: u128,
    pub num_bytes: u64,
    pub data_type: EntryDataType,
    pub status: StagedEntryStatus,
}

impl Default for EntryMetaData {
    fn default() -> Self {
        EntryMetaData {
            hash: 0,
            num_bytes: 0,
            data_type: EntryDataType::Binary,
            status: StagedEntryStatus::Added,
        }
    }
}

impl Display for EntryMetaData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EntryMetaData {{ hash: {:x}, num_bytes: {}, data_type: {:?}, status: {:?} }}",
            self.hash, self.num_bytes, self.data_type, self.status
        )
    }
}
