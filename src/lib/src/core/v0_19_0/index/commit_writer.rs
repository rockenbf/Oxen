use std::collections::HashMap;
use std::path::Path;

use std::path::PathBuf;
use std::time::Duration;
use std::str;
use indicatif::{ProgressBar, ProgressStyle};
use rocksdb::{DBWithThreadMode, SingleThreaded, IteratorMode};
use time::OffsetDateTime;

use crate::config::UserConfig;
use crate::constants::DEFAULT_BRANCH_NAME;
use crate::constants::{HEAD_FILE, STAGED_DIR};
use crate::core::db;
use crate::core::db::merkle::merkle_node_db::MerkleNodeDB;
use crate::core::v0_10_0::index::RefWriter;
use crate::core::v0_19_0::add::EntryMetaData;
use crate::core::v0_19_0::index::merkle_tree::node::{FileChunkType, FileNode, FileStorageType, MerkleTreeNodeType, VNode};
use crate::error::OxenError;
use crate::model::{Commit, EntryDataType, LocalRepository};

use crate::{repositories, util};

use super::merkle_tree::node::{CommitNode, DirNode};

#[derive(Clone)]
struct EntryVNode {
    pub id: u128,
    pub entries: Vec<EntryMetaDataWithPath>
}

impl EntryVNode {
    pub fn new(id: u128) -> Self {
        EntryVNode {
            id,
            entries: vec![]
        }
    }
}

#[derive(Clone)]
struct EntryMetaDataWithPath {
    pub path: PathBuf,
    pub hash: u128,
    pub num_bytes: u64,
    pub data_type: EntryDataType,
}

pub fn commit(
    repo: &LocalRepository,
    message: impl AsRef<str>
) -> Result<Commit, OxenError> {
    let message = message.as_ref();
    let opts = db::key_val::opts::default();
    let db_path = util::fs::oxen_hidden_dir(&repo.path).join(STAGED_DIR);
    let db: DBWithThreadMode<SingleThreaded> =
        DBWithThreadMode::open_for_read_only(&opts, dunce::simplified(&db_path), true)?;

    let read_progress = ProgressBar::new_spinner();
    read_progress.set_style(ProgressStyle::default_spinner());
    read_progress.enable_steady_tick(Duration::from_millis(100));

    // Read all the staged entries
    let mut total_entries = 0;
    let iter = db.iterator(IteratorMode::Start);
    let mut dir_entries: HashMap<PathBuf, Vec<EntryMetaDataWithPath>> = HashMap::new();
    for item in iter {
        match item {
            // key = file path
            // value = EntryMetaData
            Ok((key, value)) => {
                let key = str::from_utf8(&key)?;
                let path = Path::new(key);
                let entry: EntryMetaData = rmp_serde::from_slice(&value).unwrap();
                let entry_w_path = EntryMetaDataWithPath {
                    path: path.to_path_buf(),
                    hash: entry.hash,
                    num_bytes: entry.num_bytes,
                    data_type: entry.data_type,
                };

                if let Some(parent) = path.parent() {
                    dir_entries
                        .entry(parent.to_path_buf())
                        .or_default()
                        .push(entry_w_path);
                } else {
                    dir_entries
                        .entry(PathBuf::from(""))
                        .or_default()
                        .push(entry_w_path);
                }

                total_entries += 1;
                read_progress.set_message(format!("Gathering {} entries to commit", total_entries));
            }
            Err(err) => {
                log::error!("Could not get staged entry: {}", err);
            }
        }
    }

    // Sort children and split into VNodes
    let vnode_entries = split_into_vnodes(&dir_entries)?;

    let commit_id = compute_commit_id(&vnode_entries)?;

    // if the HEAD file exists, we have parents
    // otherwise this is the first commit
    let head_path = util::fs::oxen_hidden_dir(&repo.path).join(HEAD_FILE);
    let parent_ids = if head_path.exists() {
        let commit = repositories::commits::head_commit(repo)?;
        vec![commit.hash_u128()]
    } else {
        vec![]
    };

    let cfg = UserConfig::get()?;
    let timestamp = OffsetDateTime::now_utc();

    let node = CommitNode {
        id: commit_id,
        parent_ids: parent_ids,
        message: message.to_string(),
        author: cfg.name,
        email: cfg.email,
        timestamp: timestamp,
        ..Default::default()
    };

    let mut commit_db = MerkleNodeDB::open_read_write(repo, &node)?;
    write_commit_entries(
        &repo,
        commit_id,
        &mut commit_db,
        &vnode_entries
    )?;

    // Write HEAD file and update branch
    let head_path = util::fs::oxen_hidden_dir(&repo.path).join(HEAD_FILE);
    log::debug!("Looking for HEAD file at {:?}", head_path);
    if !head_path.exists() {
        log::debug!("HEAD file does not exist, creating new branch");
        let ref_writer = RefWriter::new(repo)?;
        let branch_name = DEFAULT_BRANCH_NAME;
        let commit_id = format!("{:x}", commit_id);
        ref_writer.set_head(branch_name);
        ref_writer.set_branch_commit_id(branch_name, &commit_id)?;
        ref_writer.set_head_commit_id(&commit_id)?;
    }

    Ok(node.to_commit())
}

fn split_into_vnodes(
    entries: &HashMap<PathBuf, Vec<EntryMetaDataWithPath>>
) -> Result<HashMap<PathBuf, Vec<EntryVNode>>, OxenError> {
    let mut results: HashMap<PathBuf, Vec<EntryVNode>> = HashMap::new();

    // Create the VNode buckets per directory
    for (directory, children) in entries.iter() {
        // log2(N / 10000)
        let total_children = children.len();
        let num_vnodes = (total_children as f32 / 10000_f32).log2();
        let num_vnodes = 2u128.pow(num_vnodes.ceil() as u32);
        log::debug!("{} VNodes for {} children in {:?}", num_vnodes, total_children, directory);
        let mut vnode_children: Vec<EntryVNode> = vec![EntryVNode::new(0); num_vnodes as usize];

        // Split entries into vnodes
        for child in children.into_iter() {
            let bucket = child.hash % num_vnodes;
            vnode_children[bucket as usize].entries.push(child.clone());
        }

        // Compute hashes and sort entries
        for vnode in vnode_children.iter_mut() {
            // Compute hash for the vnode
            let mut vnode_hasher = xxhash_rust::xxh3::Xxh3::new();
            for entry in vnode.entries.iter() {
                vnode_hasher.update(&entry.hash.to_le_bytes());
            }
            vnode.id = vnode_hasher.digest128();

            // Sort the entries in the vnode by path
            // to make searching for entries faster
            vnode.entries.sort_by(|a, b| a.path.cmp(&b.path));
        }

        results.insert(directory.to_owned(), vnode_children);
    }

    log::debug!("split_into_vnodes results: {:?}", results.len());

    Ok(results)
}

fn compute_commit_id(entries: &HashMap<PathBuf, Vec<EntryVNode>>) -> Result<u128, OxenError> {
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    for (_, vnodes) in entries.iter() {
        for vnode in vnodes {
            for child in vnode.entries.iter() {
                hasher.update(&child.hash.to_le_bytes());
            }
        }
    }
    Ok(hasher.digest128())
}

fn write_commit_entries(
    repo: &LocalRepository,
    commit_id: u128,
    commit_db: &mut MerkleNodeDB,
    entries: &HashMap<PathBuf, Vec<EntryVNode>>
) -> Result<(), OxenError> {
    // Write the root dir, then recurse into the vnodes and subdirectories
    let root_path = PathBuf::from("");
    let dir_node = aggregate_dir_node(
        repo,
        commit_id,
        entries,
        &root_path
    )?;
    commit_db.add_child(&dir_node)?;

    let mut dir_db = MerkleNodeDB::open_read_write(repo, &dir_node)?;
    r_create_dir_node(
        repo,
        commit_id,
        &mut dir_db,
        entries,
        root_path
    )?;

    Ok(())
}

fn r_create_dir_node(
    repo: &LocalRepository,
    commit_id: u128,
    dir_db: &mut MerkleNodeDB,
    entries: &HashMap<PathBuf, Vec<EntryVNode>>,
    path: impl AsRef<Path>,
) -> Result<(), OxenError> {
    let path = path.as_ref().to_path_buf();

    let Some(vnodes) = entries.get(&path) else {
        let err_msg = format!("No entries found for directory {:?}", path);
        return Err(OxenError::basic_str(err_msg));
    };

    log::debug!("Writing dir {:?} with {} vnodes", path, vnodes.len());
    for vnode in vnodes.iter() {
        let vnode_obj = VNode {
            id: vnode.id, ..Default::default()
        };
        dir_db.add_child(&vnode_obj)?;
        log::debug!("Writing vnode {:?} with {} entries", vnode.id, vnode.entries.len());

        let mut vnode_db = MerkleNodeDB::open_read_write(repo, &vnode_obj)?;
        for entry in vnode.entries.iter() {
            log::debug!("Writing entry {:?} [{:?}] to {:?}", entry.path, entry.data_type, vnode_db.path());
            match entry.data_type {
                EntryDataType::Dir => {

                    let dir_node = aggregate_dir_node(
                        repo,
                        commit_id,
                        entries,
                        &entry.path
                    )?;
                    vnode_db.add_child(&dir_node)?;

                    let mut child_db = MerkleNodeDB::open_read_write(repo, &dir_node)?;
                    r_create_dir_node(
                        repo,
                        commit_id,
                        &mut child_db,
                        entries,
                        &entry.path
                    )?;
                }
                _ => {
                    let file_name = entry.path.file_name().unwrap_or_default().to_str().unwrap();

                    // Just single file chunk for now
                    let chunks = vec![entry.hash];
                    let file_node = FileNode {
                        name: file_name.to_owned(),
                        hash: entry.hash,
                        num_bytes: entry.num_bytes,
                        chunk_type: FileChunkType::SingleFile,
                        storage_backend: FileStorageType::Disk,
                        last_commit_id: commit_id,
                        last_modified_seconds: 0,
                        last_modified_nanoseconds: 0,
                        chunk_hashes: chunks,
                        data_type: entry.data_type.clone(),
                        mime_type: "".to_string(),
                        extension: "".to_string(),
                        dtype: MerkleTreeNodeType::File,
                    };
                    vnode_db.add_child(&file_node)?;
                }
            }
        }
    }

    Ok(())
}

fn get_children(
    entries: &HashMap<PathBuf, Vec<EntryVNode>>,
    dir_path: impl AsRef<Path>
) -> Result<Vec<PathBuf>, OxenError> {
    let dir_path = dir_path.as_ref().to_path_buf();
    let mut children = vec![];

    for (path, _) in entries.iter() {
        if path.starts_with(&dir_path) {
            children.push(path.clone());
        }
    }

    return Ok(children);
}

fn aggregate_dir_node(
    repo: &LocalRepository,
    commit_id: u128,
    entries: &HashMap<PathBuf, Vec<EntryVNode>>,
    path: impl AsRef<Path>,
) -> Result<DirNode, OxenError> {
    let path = path.as_ref().to_path_buf();
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut num_bytes = 0;
    let mut data_type_counts: HashMap<String, usize> = HashMap::new();

    let children = get_children(entries, &path)?;
    log::debug!("Aggregating dir {:?} with {:?} children", path, children);
    for child in children.iter() {
        let Some(vnodes) = entries.get(child) else {
            let err_msg = format!("No entries found for directory {:?}", path);
            return Err(OxenError::basic_str(err_msg));
        };

        for vnode in vnodes.iter() {
            for entry in vnode.entries.iter() {
                log::debug!("Aggregating entry {:?} [{:?}]", entry.path, entry.data_type);
                match entry.data_type {
                    EntryDataType::Dir => {
                        log::debug!("No need to aggregate {:?}", entry.path);
                    }
                    _ => {
                        hasher.update(&entry.hash.to_le_bytes());
                        num_bytes += entry.num_bytes;

                        *data_type_counts
                            .entry(entry.data_type.to_string())
                            .or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let hash = hasher.digest128();
    let file_name = path.file_name().unwrap_or_default().to_str().unwrap();

    let node = DirNode {
        dtype: MerkleTreeNodeType::Dir,
        name: file_name.to_owned(),
        hash,
        num_bytes,
        last_commit_id: commit_id,
        last_modified_seconds: 0,
        last_modified_nanoseconds: 0,
        data_type_counts,
    };
    Ok(node)
}