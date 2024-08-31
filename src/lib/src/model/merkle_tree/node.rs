// use std::fmt::{Display, Formatter, Result};

pub mod commit_node;
pub mod dir_node;
pub mod file_chunk_node;
pub mod file_node;
pub mod file_node_types;
pub mod merkle_tree_node_data;
pub mod schema_node;
pub mod vnode;

pub use commit_node::CommitNode;
pub use dir_node::DirNode;
pub use file_chunk_node::FileChunkNode;
pub use file_node::FileNode;
pub use file_node_types::{FileChunkType, FileStorageType};
pub use merkle_tree_node_data::MerkleTreeNodeData;
pub use schema_node::SchemaNode;
pub use vnode::VNode;

pub use crate::model::{MerkleTreeNodeType, TMerkleTreeNode};

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum EMerkleTreeNode {
    File(FileNode),
    Directory(DirNode),
    VNode(VNode),
    Schema(SchemaNode),
    FileChunk(FileChunkNode),
    Commit(CommitNode),
}

impl EMerkleTreeNode {
    pub fn dtype(&self) -> MerkleTreeNodeType {
        match self {
            EMerkleTreeNode::File(_) => MerkleTreeNodeType::File,
            EMerkleTreeNode::Directory(_) => MerkleTreeNodeType::Dir,
            EMerkleTreeNode::VNode(_) => MerkleTreeNodeType::VNode,
            EMerkleTreeNode::Schema(_) => MerkleTreeNodeType::Schema,
            EMerkleTreeNode::FileChunk(_) => MerkleTreeNodeType::FileChunk,
            EMerkleTreeNode::Commit(_) => MerkleTreeNodeType::Commit,
        }
    }

    pub fn is_leaf(&self) -> bool {
        match self {
            EMerkleTreeNode::File(_) => true,
            EMerkleTreeNode::Directory(_) => false,
            EMerkleTreeNode::VNode(_) => false,
            EMerkleTreeNode::Schema(_) => true,
            EMerkleTreeNode::FileChunk(_) => true,
            EMerkleTreeNode::Commit(_) => false,
        }
    }
}
