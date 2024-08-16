//! This is a compact representation of a merkle tree file chunk node
//! that is stored in on disk
//!

use serde::{Deserialize, Serialize};

use super::{MerkleTreeNode, MerkleTreeNodeIdType, MerkleTreeNodeType};

use std::fmt;
use std::fmt::Display;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct FileChunkNode {
    pub data: Vec<u8>,
    pub dtype: MerkleTreeNodeType,
    pub id: u128,
}

impl Default for FileChunkNode {
    fn default() -> Self {
        FileChunkNode {
            data: vec![],
            dtype: MerkleTreeNodeType::FileChunk,
            id: 0,
        }
    }
}

impl MerkleTreeNodeIdType for FileChunkNode {
    fn dtype(&self) -> MerkleTreeNodeType {
        self.dtype
    }

    fn id(&self) -> u128 {
        self.id
    }
}

impl MerkleTreeNode for FileChunkNode {}

impl Display for FileChunkNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FileChunkNode({:x})", self.id)
    }
}