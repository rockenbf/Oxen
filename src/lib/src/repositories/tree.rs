use std::collections::HashSet;
use std::path::Path;

use crate::core::v0_19_0::index::CommitMerkleTree;
use crate::error::OxenError;
use crate::model::merkle_tree::node::MerkleTreeNode;
use crate::model::{Commit, LocalRepository, MerkleHash};

pub fn get_node_by_id(
    repo: &LocalRepository,
    hash: &MerkleHash,
) -> Result<Option<MerkleTreeNode>, OxenError> {
    let load_recursive = false;
    CommitMerkleTree::read_node(repo, hash, load_recursive)
}

pub fn get_node_by_path(
    repo: &LocalRepository,
    commit: &Commit,
    path: impl AsRef<Path>,
) -> Result<Option<MerkleTreeNode>, OxenError> {
    let load_recursive = false;
    let node = CommitMerkleTree::from_path(repo, commit, path, load_recursive)?;
    Ok(Some(node.root))
}

pub fn get_dir_with_children(
    repo: &LocalRepository,
    commit: &Commit,
    path: impl AsRef<Path>,
) -> Result<Option<MerkleTreeNode>, OxenError> {
    CommitMerkleTree::dir_with_children(repo, commit, path)
}

pub fn get_dir_without_children(
    repo: &LocalRepository,
    commit: &Commit,
    path: impl AsRef<Path>,
) -> Result<Option<MerkleTreeNode>, OxenError> {
    CommitMerkleTree::dir_without_children(repo, commit, path)
}

pub fn list_missing_file_hashes(
    repo: &LocalRepository,
    hash: &MerkleHash,
) -> Result<HashSet<MerkleHash>, OxenError> {
    let Some(node) = CommitMerkleTree::read_node(repo, hash, false)? else {
        return Err(OxenError::basic_str(format!("Node {} not found", hash)));
    };
    node.list_missing_file_hashes(repo)
}

pub fn child_hashes(
    repo: &LocalRepository,
    hash: &MerkleHash,
) -> Result<Vec<MerkleHash>, OxenError> {
    let Some(node) = CommitMerkleTree::read_node(repo, hash, false)? else {
        return Err(OxenError::basic_str(format!("Node {} not found", hash)));
    };
    let mut children = vec![];
    for child in node.children {
        children.push(child.hash);
    }
    Ok(children)
}
