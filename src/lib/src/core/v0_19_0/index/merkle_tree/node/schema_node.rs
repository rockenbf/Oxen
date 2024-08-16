//! This is a compact representation of a merkle tree schema node
//! that is stored in on disk
//!

use super::{MerkleTreeNode, MerkleTreeNodeIdType, MerkleTreeNodeType};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct SchemaNode {
    // The name of the file the schema references
    pub name: String,
    // TODO: add schema metadata here
    // * width
    // * height
    // * fields
    //   * name
    //   * type
    pub dtype: MerkleTreeNodeType,
    pub hash: u128,
}

impl Default for SchemaNode {
    fn default() -> Self {
        SchemaNode {
            dtype: MerkleTreeNodeType::Schema,
            name: "".to_string(),
            hash: 0,
        }
    }
}

impl MerkleTreeNodeIdType for SchemaNode {
    fn dtype(&self) -> MerkleTreeNodeType {
        self.dtype
    }

    fn id(&self) -> u128 {
        self.hash
    }
}

impl MerkleTreeNode for SchemaNode {}

impl Display for SchemaNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SchemaNode({:x})", self.hash)
    }
}