use chrono::Local;
use rocksdb::{DBWithThreadMode, MultiThreaded};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use time::OffsetDateTime;

use super::Migrate;

use crate::config::RepositoryConfig;
use crate::core;
use crate::core::db;
use crate::core::db::key_val::str_val_db;
use crate::core::db::key_val::tree_db::{TreeObject, TreeObjectChild};
use crate::core::v0_10_0::index::object_db_reader::get_object_reader;
use crate::core::v0_10_0::index::{
    CommitDirEntryReader, CommitEntryReader, CommitReader, ObjectDBReader,
};
use crate::core::v0_19_0::index::MerkleNodeDB;
use crate::core::versions::MinOxenVersion;
// use crate::core::v2::index::file_chunker::{ChunkShardManager, FileChunker};
use crate::error::OxenError;
use crate::model::merkle_tree::node::*;
use crate::model::metadata::generic_metadata::GenericMetadata;
use crate::model::CommitEntry;
use crate::model::EntryDataType;
use crate::model::MerkleHash;
use crate::model::MerkleTreeNodeType;
use crate::model::{Commit, LocalRepository};
use crate::util::progress_bar::{oxen_progress_bar, spinner_with_msg, ProgressBarType};
use crate::{constants, repositories, util};

use std::str::FromStr;

pub struct OptimizeMerkleTreesMigration;
impl Migrate for OptimizeMerkleTreesMigration {
    fn name(&self) -> &'static str {
        "optimize_merkle_trees"
    }

    fn description(&self) -> &'static str {
        "Optimize merkle trees for all commits"
    }

    fn up(&self, path: &Path, all: bool) -> Result<(), OxenError> {
        if all {
            create_merkle_trees_for_all_repos_up(path)?;
        } else {
            let repo = LocalRepository::new(path)?;
            create_merkle_trees_up(&repo)?;
        }
        Ok(())
    }

    fn down(&self, _path: &Path, _all: bool) -> Result<(), OxenError> {
        log::warn!("Optimize merkle trees migration is not reversible");
        Ok(())
    }

    fn is_needed(&self, repo: &LocalRepository) -> Result<bool, OxenError> {
        let tree_dir = repo
            .path
            .join(constants::OXEN_HIDDEN_DIR)
            .join(constants::TREE_DIR);
        if !tree_dir.exists() {
            return Ok(true);
        }
        Ok(false)
    }
}

pub fn create_merkle_trees_for_all_repos_up(path: &Path) -> Result<(), OxenError> {
    println!("🐂 Collecting namespaces to migrate...");
    let namespaces = repositories::list_namespaces(path)?;
    let bar = oxen_progress_bar(namespaces.len() as u64, ProgressBarType::Counter);
    println!("🐂 Migrating {} namespaces", namespaces.len());
    for namespace in namespaces {
        let namespace_path = path.join(namespace);
        // Show the canonical namespace path
        log::debug!(
            "This is the namespace path we're walking: {:?}",
            namespace_path.canonicalize()?
        );
        let repos = repositories::list_repos_in_namespace(&namespace_path);
        for repo in repos {
            match create_merkle_trees_up(&repo) {
                Ok(_) => {}
                Err(err) => {
                    log::error!(
                        "Could not migrate merkle trees for repo {:?}\nErr: {}",
                        repo.path.canonicalize(),
                        err
                    )
                }
            }
        }
        bar.inc(1);
    }
    Ok(())
}

pub fn create_merkle_trees_up(repo: &LocalRepository) -> Result<(), OxenError> {
    println!("👋 Starting to migrate merkle trees for {:?}", repo.path);

    // Get all commits in repo, then construct merkle tree for each commit
    let commit_reader = CommitReader::new(repo)?;
    let all_commits = commit_reader.list_all_sorted_by_timestamp()?;
    println!("Migrate {} commits for {:?}", all_commits.len(), repo.path);

    // Setup these object readers and entry readers to help pre-compute of latest commit for each file
    let mut object_readers: Vec<Arc<ObjectDBReader>> = Vec::new();
    for commit in &all_commits {
        log::debug!("Getting object reader for commit: {}", commit);
        object_readers.push(get_object_reader(repo, &commit.id)?);
    }

    log::debug!("Got {} object readers", object_readers.len());

    // Clear tree dir if exists (in order to run migration many times)
    let tree_dir = repo
        .path
        .join(constants::OXEN_HIDDEN_DIR)
        .join(constants::TREE_DIR);

    if tree_dir.exists() {
        println!("Tree dir already exists: {:?}", tree_dir);
        return Ok(());
    } else {
        // Create tree dir
        util::fs::create_dir_all(&tree_dir)?;
    }

    let bar = oxen_progress_bar(all_commits.len() as u64, ProgressBarType::Counter);
    // let commit_writer = CommitWriter::new(repo)?;
    log::debug!("Migrating {} commits", all_commits.len());
    for (commit_idx, _) in all_commits.iter().enumerate() {
        // Populate the global merkle tree from the old objects dir
        migrate_merkle_tree(
            repo,
            &commit_reader,
            &all_commits,
            commit_idx,
            &object_readers,
        )?;

        bar.inc(1);
    }

    // Set the oxen version to 0.19.0
    let mut config = RepositoryConfig::from_repo(repo)?;
    config.min_version = Some(MinOxenVersion::V0_19_0.as_str().to_string());
    let path = util::fs::config_filepath(&repo.path);
    config.save(&path)?;

    Ok(())
}

fn migrate_merkle_tree(
    repo: &LocalRepository,
    commit_reader: &CommitReader,
    commits: &[Commit],
    commit_idx: usize,
    object_readers: &Vec<Arc<ObjectDBReader>>,
) -> Result<(), OxenError> {
    let commit = &commits[commit_idx];
    let current_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    println!(
        "[{}] == START Migrating merkle tree for commit ({}/{}) {} ==",
        current_time,
        commit_idx,
        object_readers.len(),
        commit
    );
    let commit_dir = repo
        .path
        .join(constants::OXEN_HIDDEN_DIR)
        .join(constants::HISTORY_DIR)
        .join(&commit.id);

    log::debug!("Checking if commit dir exists: {:?}", commit_dir);
    if !commit_dir.exists() {
        log::warn!("Skipping commit {:?}, not downloaded", commit.id);
        return Ok(());
    }

    // Get the entry reader for the commit
    let entry_reader = CommitEntryReader::new(repo, commit)?;

    let dir_path = Path::new("");

    let mut commit_entry_readers: Vec<(Commit, CommitDirEntryReader)> = Vec::new();
    for (i, c) in commits.iter().enumerate() {
        log::debug!("Getting commit entry reader for commit {}", c);
        let reader = CommitDirEntryReader::new(repo, &c.id, dir_path, object_readers[i].clone())?;
        log::debug!("Got commit entry reader for commit {}", c);
        commit_entry_readers.push((c.clone(), reader));
        log::debug!("total commit entry readers: {}", commit_entry_readers.len());
    }

    log::debug!(
        "Got {} commit entry readers for commit {}",
        commit_entry_readers.len(),
        commit
    );
    // Write the initial commit db
    log::debug!("Migrating commit {}", commit);
    let commit_id = MerkleHash::from_str(&commit.id)?;
    let parent_ids = commit
        .parent_ids
        .iter()
        .map(|id| MerkleHash::from_str(id).unwrap())
        .collect();
    log::debug!("Parent ids for commit {} are {:?}", commit, parent_ids);

    // Create the root commit
    let node = CommitNode {
        hash: commit_id,
        parent_ids,
        message: commit.message.clone(),
        author: commit.author.clone(),
        email: commit.email.clone(),
        timestamp: commit.timestamp,
        ..Default::default()
    };

    println!(
        "Writing commit [{}/{}] node {}",
        commit_idx,
        commits.len(),
        commit
    );
    let mut commit_db = MerkleNodeDB::open_read_write(repo, &node, None)?;

    // Get the root hash
    let dir_hashes_dir = commit_dir.join(constants::DIR_HASHES_DIR);

    let dir_hashes_db: DBWithThreadMode<MultiThreaded> =
        DBWithThreadMode::open(&db::key_val::opts::default(), dir_hashes_dir)?;

    let hash: Option<String> = str_val_db::get(&dir_hashes_db, "")?;

    // This failure just means we won't migrate the dir, but the commit db still gets written
    // above so we're OK, I don't know how we get into this state
    let Some(hash) = hash else {
        log::warn!("No dir hash found for commit {}", commit);
        return Ok(());
    };

    let hash = hash.replace('"', "");
    log::debug!("OG Dir hash for commit {} is {}", commit, hash);
    let hash = MerkleHash::from_str(&hash)?;
    log::debug!("Dir hash for commit {} is {}", commit, hash);

    // Commit node has one child, the root dir
    let dir_node = write_dir_child(
        repo,
        commit_idx,
        &entry_reader,
        object_readers,
        commits,
        &mut commit_db,
        dir_path,
        &hash,
    )?;

    let mut dir_db = MerkleNodeDB::open_read_write(repo, &dir_node, Some(commit_id))?;
    migrate_dir(
        repo,
        commits,
        commit_idx,
        commit_reader,
        &entry_reader,
        object_readers,
        &mut dir_db,
        dir_path,
        &hash,
    )?;

    // Remove all the quotes from the db
    let vals: Vec<(String, String)> = str_val_db::list(&dir_hashes_db)?;
    for (key, val) in vals {
        let val = val.replace('"', "");
        str_val_db::put(&dir_hashes_db, key, &val)?;
    }

    Ok(())
}

#[allow(clippy::only_used_in_recursion)]
#[allow(clippy::too_many_arguments)]
fn migrate_dir(
    repo: &LocalRepository,
    commits: &[Commit],
    commit_idx: usize,
    commit_reader: &CommitReader,
    entry_reader: &CommitEntryReader,
    object_readers: &Vec<Arc<ObjectDBReader>>,
    dir_db: &mut MerkleNodeDB,
    dir_path: &Path, // full path to dir (path/to/dir)
    dir_hash: &MerkleHash,
) -> Result<(), OxenError> {
    // Read the values from the .oxen/objects/dirs db and write them
    // to the proper .oxen/tree/{path} with their hash as the key and type
    // and metadata as the value
    //
    println!(
        "Processing dir path [{:?}] hash [{}] for commit {}",
        dir_path, dir_hash, commits[commit_idx]
    );

    /*
    The number of VNodes is dynamic depending on the number of children in
    the directory.

    This helps us with reads/writes making them lean if we have many
    children in a directory.

    N = Number of Children
    M = Number of VNodes

    If we want each bucket to be ~10,000 entries

    Should be N / (2^M) <= 10,000, solve for M
    N / 10,000 = (2^M)
    M = log2(N / 10000)

    It's logarithmic, because we don't want too many vnodes per dir

    * log2(1,000,000 / 10,000)
        * 1,000,000,000 / (2^16) = 1,000,000,000 / 65,536 = 15,258
            * 65,536 VNodes
            * 15,258 Children Per VNode
        * 1,000,000 / (2^6) = 1,000,000 / 64 = 15,625
            * 64 VNodes
            * 15,625 Children Per VNode
        * 500,000 / (2^5) = 500,000 / 32 = 15,625
            * 32 VNodes
            * 15,258 Children Per VNode
        * 200,000 / (2^4) = 200,000 / 16 = 12,500
            * 16 VNodes
            * 12,5000 Children Per VNode
    */

    let obj_reader = &object_readers[commit_idx];
    let dir_obj = obj_reader.get_dir(&dir_hash.to_string())?;

    let mut commit_entry_readers: Vec<(Commit, CommitDirEntryReader)> = Vec::new();
    for (i, c) in commits.iter().enumerate() {
        let reader = CommitDirEntryReader::new(repo, &c.id, dir_path, object_readers[i].clone())?;
        commit_entry_readers.push((c.clone(), reader));
    }

    let Some(dir_obj) = dir_obj else {
        return Err(OxenError::basic_str(format!(
            "could not get dir objects for {}",
            dir_hash
        )));
    };

    let commit = &commits[commit_idx];

    log::debug!("MIGRATE_DIR: path {:?} for commit {}", dir_path, commit);

    // Write all the VNodes
    let mut children: Vec<TreeObjectChild> = Vec::new();
    for child in dir_obj.children() {
        if let TreeObjectChild::VNode { path: _, hash } = child {
            let vnode_obj = obj_reader
                .get_vnode(hash)?
                .expect("could not get vnode object");

            for child in vnode_obj.children() {
                children.push(child.clone());
            }
        }
    }

    // make sure to add all the dirs that are directly decendents
    // because it seems like there was some bug where we were missing
    // some dirs
    for (dir, hash) in obj_reader.list_dirs_w_hashes()? {
        log::debug!(
            "MIGRATE_DIR: path {:?} checking dir {:?} with hash {:?}",
            dir_path,
            dir,
            hash
        );
        if let Some(parent) = dir.parent() {
            if parent == dir_path || (parent == Path::new("") && dir_path == Path::new("")) {
                let seen_it = children.iter().any(|c| match c {
                    TreeObjectChild::Dir { path, hash: _ } => path == &dir,
                    _ => false,
                });
                if !seen_it {
                    log::debug!(
                        "MIGRATE_DIR: path {:?} adding dir {:?} with hash {:?}",
                        dir_path,
                        dir,
                        hash
                    );
                    children.push(TreeObjectChild::Dir {
                        path: dir.clone(),
                        hash: hash.clone(),
                    });
                }
            }
        }
    }

    log::debug!(
        "MIGRATE_DIR: path {:?} got {} children",
        dir_path,
        children.len()
    );

    // log2(N / 10000)
    let total_children = children.len();
    let vnode_size = 10_000;
    let num_vnodes = (total_children as f32 / vnode_size as f32).ceil() as u128;
    log::debug!(
        "MIGRATE_DIR: path {:?} {} VNodes for {} children",
        dir_path,
        num_vnodes,
        total_children
    );

    // Group the children into their buckets
    let mut buckets: Vec<Vec<TreeObjectChild>> = vec![Vec::new(); num_vnodes as usize];
    for child in children {
        let hash_int = u128::from_str_radix(child.hash(), 16).expect("Failed to parse hex string");
        let bucket = hash_int % num_vnodes;
        buckets[bucket as usize].push(child);
    }

    // Compute new hashes for each bucket
    // TODO: Make sure we make these unique like in the commit writer
    let mut bucket_hashes: Vec<u128> = vec![0; num_vnodes as usize];
    for (i, bucket) in buckets.iter().enumerate() {
        let mut hasher = xxhash_rust::xxh3::Xxh3::new();
        hasher.update(b"vnode");
        // generate a uuid for the vnode
        hasher.update(dir_path.to_str().unwrap().as_bytes());
        for child in bucket {
            // TODO: child.hash() is a string and we should just use
            //       the u128 hash for speed and consistency
            hasher.update(child.hash().as_bytes());
        }
        bucket_hashes[i] = hasher.digest128();
    }

    // Add all vnodes as children of the dir
    let mut vnode_nodes: Vec<VNode> = Vec::new();
    for (i, bhash) in bucket_hashes.iter().enumerate() {
        log::debug!("Bucket [{}] for {:x}", i, bhash);
        let node = VNode {
            hash: MerkleHash::new(*bhash),
            ..Default::default()
        };
        dir_db.add_child(&node)?;
        vnode_nodes.push(node);
    }

    // Re-Write the N vnodes
    log::debug!(
        "Writing {} buckets for path [{:?}] on commit {}",
        buckets.len(),
        dir_path,
        commit
    );
    for (i, bucket) in buckets.iter().enumerate() {
        let vnode = &vnode_nodes[i];

        // Write the children of the VNodes
        let mut node_db = MerkleNodeDB::open_read_write(repo, vnode, Some(*dir_hash))?;
        log::debug!(
            "Writing {} vnodes for bucket {} to path: {:?}",
            bucket.len(),
            i,
            node_db.path()
        );
        for (j, child) in bucket.iter().enumerate() {
            let (dtype, hash, path) = match child {
                TreeObjectChild::VNode { path, hash } => (MerkleTreeNodeType::VNode, hash, path),
                TreeObjectChild::File { path, hash } => (MerkleTreeNodeType::File, hash, path),
                TreeObjectChild::Dir { path, hash } => (MerkleTreeNodeType::Dir, hash, path),
                TreeObjectChild::Schema { path: _, hash: _ } => continue,
            };

            // if j % 1000 == 0 {
            log::debug!(
                "MIGRATE_DIR: path {:?} writing child {} {:?} {}",
                dir_path,
                j,
                dtype,
                path.display()
            );
            // }

            let child_hash = MerkleHash::from_str(hash)?;

            match dtype {
                MerkleTreeNodeType::Commit => {
                    // pass, we only write the commit once at the top
                    panic!("migrate_dir should not get to Commit");
                }
                MerkleTreeNodeType::VNode => {
                    // pass, we already wrote the vnode
                    panic!("migrate_dir should not get to VNode");
                }
                MerkleTreeNodeType::FileChunk => {
                    // pass, we do this in migrate_file
                    panic!("migrate_dir should not get to FileChunk");
                }
                MerkleTreeNodeType::File => {
                    // If it's a file, let's chunk it and make the chunk leaf nodes
                    let current_commit = &commits[commit_idx];
                    match write_file_node(
                        repo,
                        entry_reader,
                        &commit_entry_readers,
                        current_commit,
                        &mut node_db,
                        path,
                        &child_hash,
                    ) {
                        Ok(_) => (),
                        Err(e) => log::warn!("Error writing file node: {:?}", e),
                    }
                }
                MerkleTreeNodeType::Dir => {
                    let dir_node = write_dir_child(
                        repo,
                        commit_idx,
                        entry_reader,
                        object_readers,
                        commits,
                        &mut node_db,
                        path,
                        &child_hash,
                    )?;

                    if dir_node.last_commit_id == MerkleHash::new(0) {
                        log::warn!("No last commit id found for path {:?}", path);
                        return Ok(());
                    }

                    // Recurse if it's a directory
                    let mut dir_db =
                        MerkleNodeDB::open_read_write(repo, &dir_node, Some(vnode.hash))?;
                    migrate_dir(
                        repo,
                        commits,
                        commit_idx,
                        commit_reader,
                        entry_reader,
                        object_readers,
                        &mut dir_db,
                        path,
                        &child_hash,
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_dir_child(
    repo: &LocalRepository,
    commit_idx: usize,
    entry_reader: &CommitEntryReader,
    object_readers: &[Arc<ObjectDBReader>],
    commits: &[Commit],
    node_db: &mut MerkleNodeDB,
    path: &Path,
    hash: &MerkleHash,
) -> Result<DirNode, OxenError> {
    let commit = &commits[commit_idx];
    println!(
        "Write dir node for path [{:?}] and hash [{}/{}] on commit[{}] {}",
        path,
        hash,
        commit_idx,
        commits.len(),
        commit
    );
    let file_name = path.file_name().unwrap_or_default().to_str().unwrap();

    let mut num_bytes = 0;
    let mut last_commit_id = 0;
    let mut last_commit_timestamp: OffsetDateTime = OffsetDateTime::from_unix_timestamp(0).unwrap();
    let mut last_modified_seconds = 0;
    let mut last_modified_nanoseconds = 0;
    let mut data_type_counts: HashMap<String, u64> = HashMap::new();
    let mut data_type_sizes: HashMap<String, u64> = HashMap::new();

    let mut entries_processed = 0;

    // List dir children
    let dirs = entry_reader.list_dirs()?;

    let dirs: Vec<PathBuf> = dirs
        .iter()
        .filter(|dir| dir.starts_with(path) || Path::new("") == path)
        .map(|dir| dir.to_path_buf())
        .collect();

    // Compute total size and data type counts
    let progress_bar = spinner_with_msg(format!(
        "Processing {} subdirs for path [{:?}] {:?}",
        dirs.len(),
        path,
        dirs
    ));

    // This is an insane data structure, but it's the fastest way to get the
    // latest commit for each entry in the directory

    // If you make a joint key of dir path + commit id, you can save on loading all the readers
    // multiple times
    type DirReaderKey = (PathBuf, String);
    let mut dir_readers: HashMap<DirReaderKey, HashMap<PathBuf, CommitEntry>> = HashMap::new();
    for dir in &dirs {
        // let mut readers: Vec<(Commit, HashMap<PathBuf, CommitEntry>)> = Vec::new();
        for (i, c) in commits.iter().enumerate() {
            if dir_readers.contains_key(&(dir.clone(), c.id.clone())) {
                continue;
            }
            let reader = CommitDirEntryReader::new(repo, &c.id, dir, object_readers[i].clone())?;
            let entry_paths: HashMap<PathBuf, CommitEntry> = reader
                .list_entries_set()?
                .iter()
                .map(|e| (e.path.clone(), e.clone()))
                .collect();
            // readers.push((c.clone(), entry_paths));
            log::debug!("Got reader [{}] for commit: {}", i, c);
            dir_readers.insert((dir.clone(), c.id.clone()), entry_paths);
        }
    }

    for dir in dirs {
        log::debug!("processing path [{:?}] sub dir: {:?}", path, dir);
        let dir_entry_reader =
            CommitDirEntryReader::new(repo, &commit.id, &dir, object_readers[commit_idx].clone())?;
        log::debug!(
            "Got dir entry reader for path [{:?}] subdir [{:?}]",
            path,
            dir
        );
        // let mut readers: Vec<(Commit, HashMap<PathBuf, CommitEntry>)> = Vec::new();
        // for (i, (c, _)) in commit_entry_readers.iter().enumerate() {
        //     let reader = CommitDirEntryReader::new(repo, &c.id, &dir, object_readers[i].clone())?;
        //     let entry_paths: HashMap<PathBuf, CommitEntry> = reader
        //         .list_entries_set()?.iter().map(|e| (e.path.clone(), e.clone())).collect();
        //     readers.push((c.clone(), entry_paths));
        //     log::debug!("Got reader [{}] for commit: {}", i, c);
        // }

        let entries = dir_entry_reader.list_entries()?;
        log::debug!("Processing {} entries for path [{:?}]", entries.len(), path);
        for entry in entries {
            num_bytes += entry.num_bytes;

            let mut last_hash = "".to_string();
            log::debug!(
                "Getting latest commit for {:?} in subdir {:?} of {:?}",
                entry.path,
                dir,
                path
            );
            for c in commits {
                // let file_name = entry.path.file_name().unwrap().to_str().unwrap();
                let key = (dir.clone(), c.id.clone());
                let entries = dir_readers.get(&key).unwrap();
                // if let Some(ce) = entries.get_entry(file_name)? {
                if let Some(ce) = entries.get(&entry.path) {
                    // log::debug!("Got entry for {:?} in subdir {:?} of {:?} for commit {}", entry.path, dir, path, commit);
                    if commit.timestamp > last_commit_timestamp && ce.hash != last_hash {
                        // log::debug!("Updating last commit id for {:?} in subdir {:?} of {:?} to {} because it changed from {} to {}", entry.path, dir, path, commit.id, last_hash, ce.hash);
                        last_commit_id = u128::from_str_radix(&commit.id, 16)
                            .expect("Failed to parse hex string");
                        last_commit_timestamp = commit.timestamp;
                    }
                    last_hash = ce.hash.clone();
                }
            }

            entries_processed += 1;
            let version_path = util::fs::version_path(repo, &entry);

            if version_path.exists() {
                let mime_type = util::fs::file_mime_type(&version_path);
                let data_type = util::fs::datatype_from_mimetype(&version_path, &mime_type);
                let data_type_str = format!("{}", data_type);
                data_type_counts
                    .entry(data_type_str.clone())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);
                data_type_sizes
                    .entry(data_type_str.clone())
                    .and_modify(|size| *size += entry.num_bytes)
                    .or_insert(entry.num_bytes);
            } else {
                log::warn!("Version path does not exist: {:?}", version_path);
            }

            if last_modified_seconds < entry.last_modified_seconds {
                last_modified_seconds = entry.last_modified_seconds;
            }

            if last_modified_nanoseconds < entry.last_modified_nanoseconds {
                last_modified_nanoseconds = entry.last_modified_nanoseconds;
            }

            progress_bar.set_message(format!(
                "Processing {:?} children: {} ({})",
                path,
                entries_processed,
                bytesize::ByteSize::b(num_bytes)
            ));
        }
    }

    let mut node = DirNode {
        node_type: MerkleTreeNodeType::Dir,
        name: file_name.to_owned(),
        hash: *hash,
        num_bytes,
        last_commit_id: MerkleHash::new(last_commit_id),
        last_modified_seconds,
        last_modified_nanoseconds,
        data_type_counts,
        data_type_sizes,
    };
    if last_commit_id == 0 {
        log::warn!("No last commit id found for path {:?}", path);
        node.last_commit_id = MerkleHash::from_str(&commit.id)?;
    }
    println!("Writing dir node {} to {:?}", node, node_db.path());
    node_db.add_child(&node)?;
    Ok(node)
}

fn write_file_node(
    repo: &LocalRepository,
    entry_reader: &CommitEntryReader,
    commit_entry_readers: &[(Commit, CommitDirEntryReader)],
    current_commit: &Commit,
    node_db: &mut MerkleNodeDB,
    path: &Path,
    hash: &MerkleHash,
) -> Result<(), OxenError> {
    let obj_reader = entry_reader.get_obj_reader();
    // read other meta data from file object
    let file_obj = obj_reader
        .get_file(&hash.to_string())?
        .ok_or(OxenError::basic_str(format!(
            "could not get file object for {}",
            hash
        )))?;

    let (num_bytes, last_modified_seconds, last_modified_nanoseconds) = match file_obj {
        TreeObject::File {
            num_bytes,
            last_modified_seconds,
            last_modified_nanoseconds,
            ..
        } => (num_bytes, last_modified_seconds, last_modified_nanoseconds),
        _ => return Err(OxenError::basic_str("file object is not a file")),
    };

    // log::debug!("Getting latest commit for path {:?} with {:?} readers", path, commit_entry_readers.len());
    let last_commit_id = if let Some(latest_commit) =
        core::v0_10_0::entries::get_latest_commit_for_path(commit_entry_readers, path)?
    {
        MerkleHash::from_str(&latest_commit.id)?
    } else {
        log::warn!("No last commit id found for path {:?}", path);
        MerkleHash::from_str(&current_commit.id)?
    };

    let commit_entry = entry_reader
        .get_entry(path)?
        .ok_or(OxenError::basic_str(format!(
            "could not get file entry for {}",
            path.display()
        )))?;

    // Chunk the file into 16kb chunks
    /* TODO: This is hard / inefficient to read into Polars for now, ignore

    let chunker = FileChunker::new(repo);
    let mut csm = ChunkShardManager::new(repo)?;
    csm.open_for_write()?;
    let chunks = chunker.save_chunks(&commit_entry, &mut csm)?;
     */

    // For now, we just have one chunk per file
    let chunks: Vec<u128> = vec![hash.to_u128()];

    // Then start refactoring the commands into a "legacy" module so we can still make the old
    // dbs but start implementing them with the new merkle object
    let file_name = path.file_name().unwrap().to_str().unwrap();

    // TODO: Need to get mime_type and data type from the combination of the contents and the extension
    // Because we rename the file to drop the extension halfway through the migration

    let version_path = util::fs::version_path(repo, &commit_entry);
    if !version_path.exists() {
        log::warn!("Version path does not exist: {:?}", version_path);
        return Ok(());
    }

    let extension = file_name.split('.').last().unwrap_or_default().to_string();
    let mut mime_type = util::fs::file_mime_type_from_extension(&version_path, path);
    let mut data_type =
        util::fs::datatype_from_mimetype_from_extension(&version_path, path, &mime_type);
    log::debug!(
        "write_file_node {:?} version_path: {:?} extension: {:?} mime_type: {:?} data_type: {:?}",
        path,
        version_path,
        extension,
        mime_type,
        data_type
    );

    // Look up schema metadata
    let mut metadata = repositories::metadata::get_file_metadata_with_extension(
        &version_path,
        &data_type,
        &extension,
    )?;

    // Look up existing schema metadata if it is tabular
    if data_type == EntryDataType::Tabular {
        let schema_reader =
            core::v0_10_0::index::schema_reader::SchemaReader::new(repo, &current_commit.id)?;
        log::debug!("Getting schema for path {:?}", path);
        let schema_metadata = schema_reader.get_schema_for_file(path)?;
        if let Some(schema) = schema_metadata {
            match &mut metadata {
                Some(GenericMetadata::MetadataTabular(m)) => {
                    m.tabular.schema = schema;
                }
                m_metadata => {
                    log::warn!("Expected tabular metadata for path {:?}", path);
                    log::warn!("Got {:?}", m_metadata);
                    metadata = None;
                }
            }
        }
    };

    if metadata.is_none() && data_type == EntryDataType::Tabular {
        log::warn!("No metadata found for path {:?}", path);
        data_type = EntryDataType::Binary;
        mime_type = "application/octet-stream".to_string();
    }

    let metadata_hash = util::hasher::maybe_get_metadata_hash(&metadata.clone())?;
    let combined_hash = util::hasher::get_combined_hash(metadata_hash, hash.to_u128())?;
    let combined_hash = MerkleHash::new(combined_hash);
    let metadata_hash = metadata_hash.map(MerkleHash::new);

    // Rename the version path file name to drop the extension
    let new_version_path = version_path.with_extension("");
    util::fs::rename(&version_path, &new_version_path)?;

    let val = FileNode {
        name: file_name.to_owned(),
        hash: *hash,
        combined_hash,
        metadata_hash,
        num_bytes,
        chunk_type: FileChunkType::SingleFile,
        storage_backend: FileStorageType::Disk,
        last_commit_id,
        last_modified_seconds,
        last_modified_nanoseconds,
        chunk_hashes: chunks,
        data_type,
        mime_type,
        extension,
        metadata,
        node_type: MerkleTreeNodeType::File,
    };
    node_db.add_child(&val)?;

    // TODO
    // * Look at the oxen pack command and abstract out this logic
    // * Store the chunks in the .oxen/objects/chunks dir (next to .oxen/objects/schemas)
    // * The file node object will need to be different than the other tree node objects
    //     * file_idx -> chunk_hash
    //     * we will want to store the enum of the type at the top of the node file, so we know what to deserialize
    // * The chunk dir db (.oxen/objects/chunks) will need chunk_hash -> chunk
    Ok(())
}

pub fn create_merkle_trees_for_all_repos_down(_path: &Path) -> Result<(), OxenError> {
    println!("There are no operations to be run");
    Ok(())
}

pub fn create_merkle_trees_down(_repo: &LocalRepository) -> Result<(), OxenError> {
    println!("There are no operations to be run");
    Ok(())
}
