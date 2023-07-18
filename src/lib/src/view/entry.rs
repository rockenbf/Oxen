use crate::model::{CommitEntry, MetadataEntry, RemoteEntry};
use serde::{Deserialize, Serialize};

use super::{JsonDataFrame, StatusMessage};

#[derive(Deserialize, Serialize, Debug)]
pub struct EntryResponse {
    #[serde(flatten)]
    pub status: StatusMessage,
    pub entry: CommitEntry,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RemoteEntryResponse {
    #[serde(flatten)]
    pub status: StatusMessage,
    pub entry: RemoteEntry,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ResourceVersion {
    pub path: String,
    pub version: String,
}

impl ResourceVersion {
    pub fn from_parsed_resource(resource: &crate::model::ParsedResource) -> ResourceVersion {
        ResourceVersion {
            path: resource.file_path.to_string_lossy().to_string(),
            version: resource.version(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PaginatedEntries {
    #[serde(flatten)]
    pub status: StatusMessage,
    pub entries: Vec<RemoteEntry>,
    pub page_size: usize,
    pub page_number: usize,
    pub total_pages: usize,
    pub total_entries: usize,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DirectoryMetadata {
    pub data_types: JsonDataFrame,
    pub mime_types: JsonDataFrame,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PaginatedDirEntries {
    pub entries: Vec<MetadataEntry>,
    pub resource: Option<ResourceVersion>,
    pub metadata: Option<DirectoryMetadata>,
    pub page_size: usize,
    pub page_number: usize,
    pub total_pages: usize,
    pub total_entries: usize,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PaginatedDirEntriesResponse {
    #[serde(flatten)]
    pub status: StatusMessage,
    #[serde(flatten)]
    pub entries: PaginatedDirEntries,
    pub dir: MetadataEntry,
}

impl PaginatedDirEntriesResponse {
    pub fn ok_from(dir: MetadataEntry, paginated: PaginatedDirEntries) -> Self {
        Self {
            status: StatusMessage::resource_found(),
            entries: paginated,
            dir,
        }
    }
}
