//! # Oxen Commands
//!
//! Top level commands you are likely to run on an Oxen repository
//!

use crate::api;
use crate::constants;
use crate::error::OxenError;
use crate::index::{
    CommitEntryReader, CommitReader, CommitWriter, Indexer, Merger, RefReader, RefWriter, Stager,
};
use crate::model::{Branch, Commit, LocalRepository, RemoteBranch, RemoteRepository, StagedData};
use crate::util;

use rocksdb::{IteratorMode, LogLevel, Options, DB};
use std::path::Path;
use std::str;

/// # Initialize an Empty Oxen Repository
///
/// ```
/// # use liboxen::command;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// let base_dir = Path::new("/tmp/repo_dir_init");
/// command::init(base_dir)?;
/// assert!(base_dir.join(".oxen").exists());
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
pub fn init(path: &Path) -> Result<LocalRepository, OxenError> {
    let hidden_dir = util::fs::oxen_hidden_dir(path);
    if hidden_dir.exists() {
        let err = format!("Oxen repository already exists: {:?}", path);
        return Err(OxenError::basic_str(err));
    }

    std::fs::create_dir_all(hidden_dir)?;
    let config_path = util::fs::config_filepath(path);
    let repo = LocalRepository::new(path)?;
    repo.save(&config_path)?;

    if let Ok(commit) = commit_with_no_files(&repo, constants::INITIAL_COMMIT_MSG) {
        println!("Initial commit {}", commit.id);
    }

    Ok(repo)
}

/// # Get status of files in repository
///
/// What files are tracked, added, untracked, etc
///
/// Empty Repository:
///
/// ```
/// use liboxen::command;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// let base_dir = Path::new("/tmp/repo_dir_status_1");
/// // Initialize empty repo
/// let repo = command::init(&base_dir)?;
/// // Get status on repo
/// let status = command::status(&repo)?;
/// assert!(status.is_clean());
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
///
/// Repository with files
/// ```
/// use liboxen::command;
/// use liboxen::util;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// let base_dir = Path::new("/tmp/repo_dir_status_2");
/// // Initialize empty repo
/// let repo = command::init(&base_dir)?;
///
/// // Write file to disk
/// let hello_file = base_dir.join("hello.txt");
/// util::fs::write_to_path(&hello_file, "Hello World");
///
/// // Get status on repo
/// let status = command::status(&repo)?;
/// assert_eq!(status.untracked_files.len(), 1);
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
pub fn status(repository: &LocalRepository) -> Result<StagedData, OxenError> {
    let hidden_dir = util::fs::oxen_hidden_dir(&repository.path);
    if !hidden_dir.exists() {
        return Err(OxenError::local_repo_not_found());
    }

    let reader = CommitEntryReader::new_from_head(repository)?;
    let stager = Stager::new(repository)?;
    let status = stager.status(&reader)?;
    Ok(status)
}

/// # Get status of files in repository
///
/// ```
/// use liboxen::command;
/// use liboxen::util;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// // Initialize the repository
/// let base_dir = Path::new("/tmp/repo_dir_add");
/// let repo = command::init(base_dir)?;
///
/// // Write file to disk
/// let hello_file = base_dir.join("hello.txt");
/// util::fs::write_to_path(&hello_file, "Hello World");
///
/// // Stage the file
/// command::add(&repo, &hello_file)?;
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
pub fn add<P: AsRef<Path>>(repo: &LocalRepository, path: P) -> Result<(), OxenError> {
    let stager = Stager::new_with_merge(repo)?;
    let commit = head_commit(repo)?;
    let reader = CommitEntryReader::new(repo, &commit)?;
    stager.add(path.as_ref(), &reader)?;
    Ok(())
}

/// # Commit the staged files in the repo
///
/// ```
/// use liboxen::command;
/// use liboxen::util;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// // Initialize the repository
/// let base_dir = Path::new("/tmp/repo_dir_commit");
/// let repo = command::init(base_dir)?;
///
/// // Write file to disk
/// let hello_file = base_dir.join("hello.txt");
/// util::fs::write_to_path(&hello_file, "Hello World");
///
/// // Stage the file
/// command::add(&repo, &hello_file)?;
///
/// // Commit staged
/// command::commit(&repo, "My commit message")?;
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
pub fn commit(repo: &LocalRepository, message: &str) -> Result<Option<Commit>, OxenError> {
    let status = status(repo)?;
    if !status.has_added_entries() {
        println!(
            "No files are staged, not committing. Stage a file or directory with `oxen add <file>`"
        );
        return Ok(None);
    }
    let commit = p_commit(repo, &status, message)?;
    Ok(Some(commit))
}

fn commit_with_no_files(repo: &LocalRepository, message: &str) -> Result<Commit, OxenError> {
    let status = StagedData::empty();
    let commit = p_commit(repo, &status, message)?;
    Ok(commit)
}

fn p_commit(
    repo: &LocalRepository,
    status: &StagedData,
    message: &str,
) -> Result<Commit, OxenError> {
    let stager = Stager::new(repo)?;
    let commit_writer = CommitWriter::new(repo)?;
    let commit = commit_writer.commit(status, message)?;
    stager.unstage()?;
    Ok(commit)
}

/// # Get a log of all the commits
///
/// ```
/// use liboxen::command;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// // Initialize the repository
/// let base_dir = Path::new("/tmp/repo_dir_log");
/// let repo = command::init(base_dir)?;
///
/// // Print     commit history
/// let history = command::log(&repo)?;
/// for commit in history.iter() {
///   println!("{} {}", commit.id, commit.message);
/// }
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
pub fn log(repo: &LocalRepository) -> Result<Vec<Commit>, OxenError> {
    let committer = CommitReader::new(repo)?;
    let commits = committer.history_from_head()?;
    Ok(commits)
}

/// # Get the history for a specific branch
pub fn log_branch_commit_history(
    repo: &LocalRepository,
    branch_name: &str,
) -> Result<Vec<Commit>, OxenError> {
    let committer = CommitReader::new(repo)?;
    if let Some(commit_id) = get_branch_commit_id(repo, branch_name)? {
        let commits = committer.history_from_commit_id(&commit_id)?;
        Ok(commits)
    } else {
        let err = format!("Branch does not exist: {}", branch_name);
        Err(OxenError::basic_str(err))
    }
}

/// # Create a new branch
/// This creates a new pointer to the current commit with a name,
/// it does not switch you to this branch, you still must call `checkout_branch`
pub fn create_branch(repo: &LocalRepository, name: &str) -> Result<Branch, OxenError> {
    let ref_writer = RefWriter::new(repo)?;
    let commit_reader = CommitReader::new(repo)?;
    let head_commit = commit_reader.head_commit()?;
    ref_writer.create_branch(name, &head_commit.id)
}

/// # Checkout a branch or commit id
/// This switches HEAD to point to the branch name or commit id,
/// it also updates all the local files to be from the commit that this branch references
pub fn checkout<S: AsRef<str>>(repo: &LocalRepository, value: S) -> Result<(), OxenError> {
    let value = value.as_ref();
    if branch_exists(repo, value) {
        if already_on_branch(repo, value) {
            println!("Already on branch {}", value);
            return Ok(());
        }

        println!("Checkout branch: {}", value);
        set_working_branch(repo, value)?;
        set_head(repo, value)?;
    } else {
        // If we are already on the commit, do nothing
        if already_on_commit(repo, value) {
            eprintln!("Commit already checked out {}", value);
            return Ok(());
        }

        println!("Checkout commit: {}", value);
        set_working_commit_id(repo, value)?;
        set_head(repo, value)?;
    }

    Ok(())
}

fn set_working_branch(repo: &LocalRepository, name: &str) -> Result<(), OxenError> {
    let commit_writer = CommitWriter::new(repo)?;
    commit_writer.set_working_repo_to_branch(name)
}

fn set_working_commit_id(repo: &LocalRepository, commit_id: &str) -> Result<(), OxenError> {
    let commit_writer = CommitWriter::new(repo)?;
    commit_writer.set_working_repo_to_commit_id(commit_id)
}

fn set_head(repo: &LocalRepository, value: &str) -> Result<(), OxenError> {
    let ref_writer = RefWriter::new(repo)?;
    ref_writer.set_head(value);
    Ok(())
}

fn get_branch_commit_id(repo: &LocalRepository, name: &str) -> Result<Option<String>, OxenError> {
    match RefReader::new(repo) {
        Ok(ref_reader) => ref_reader.get_commit_id_for_branch(name),
        _ => Err(OxenError::basic_str("Could not read reference for repo.")),
    }
}

fn branch_exists(repo: &LocalRepository, name: &str) -> bool {
    match RefReader::new(repo) {
        Ok(ref_reader) => ref_reader.has_branch(name),
        _ => false,
    }
}

fn already_on_branch(repo: &LocalRepository, name: &str) -> bool {
    match RefReader::new(repo) {
        Ok(ref_reader) => {
            if let Ok(Some(current_branch)) = ref_reader.get_current_branch() {
                // If we are already on the branch, do nothing
                if current_branch.name == name {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn already_on_commit(repo: &LocalRepository, commit_id: &str) -> bool {
    match RefReader::new(repo) {
        Ok(ref_reader) => {
            if let Ok(head_commit_id) = ref_reader.head_commit_id() {
                // If we are already on the branch, do nothing
                if head_commit_id == commit_id {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// # Create a branch and check it out in one go
/// This creates a branch with name,
/// then switches HEAD to point to the branch
pub fn create_checkout_branch(repo: &LocalRepository, name: &str) -> Result<(), OxenError> {
    println!("Create and checkout branch: {}", name);
    let head_commit = head_commit(repo)?;
    let ref_writer = RefWriter::new(repo)?;

    ref_writer.create_branch(name, &head_commit.id)?;
    ref_writer.set_head(name);
    Ok(())
}

/// # Merge a branch into the current branch
/// Checks for simple fast forward merge, or if current branch has diverged from the merge branch
/// it will perform a 3 way merge
/// If there are conflicts, it will abort and show the conflicts to be resolved in the `status` command
pub fn merge<S: AsRef<str>>(
    repo: &LocalRepository,
    branch_name: S,
) -> Result<Option<Commit>, OxenError> {
    let branch_name = branch_name.as_ref();
    if branch_exists(repo, branch_name) {
        if let Some(branch) = current_branch(repo)? {
            let merger = Merger::new(repo)?;
            if let Some(commit) = merger.merge(branch_name)? {
                println!(
                    "Successfully merged `{}` into `{}`",
                    branch_name, branch.name
                );
                println!("HEAD -> {}", commit.id);
                Ok(Some(commit))
            } else {
                eprintln!("Automatic merge failed; fix conflicts and then commit the result.");
                Ok(None)
            }
        } else {
            Err(OxenError::basic_str(
                "Must be on a branch to perform a merge.",
            ))
        }
    } else {
        Err(OxenError::local_branch_not_found(branch_name))
    }
}

/// # List local branches
pub fn list_branches(repo: &LocalRepository) -> Result<Vec<Branch>, OxenError> {
    let ref_reader = RefReader::new(repo)?;
    let branches = ref_reader.list_branches()?;
    Ok(branches)
}

/// # List remote branches
pub fn list_remote_branches(repo: &LocalRepository) -> Result<Vec<Branch>, OxenError> {
    if let Some(remote_repo) = api::remote::repositories::get_by_name(&repo.name)? {
        let branches = api::remote::branches::list(&remote_repo)?;
        Ok(branches)
    } else {
        Err(OxenError::remote_repo_not_found(&repo.name))
    }
}

/// # Get the current branch
pub fn current_branch(repo: &LocalRepository) -> Result<Option<Branch>, OxenError> {
    let ref_reader = RefReader::new(repo)?;
    let branch = ref_reader.get_current_branch()?;
    Ok(branch)
}

/// # Get the current commit
pub fn root_commit(repo: &LocalRepository) -> Result<Commit, OxenError> {
    let committer = CommitReader::new(repo)?;
    let commit = committer.root_commit()?;
    Ok(commit)
}

/// # Get the current commit
pub fn head_commit(repo: &LocalRepository) -> Result<Commit, OxenError> {
    let committer = CommitReader::new(repo)?;
    let commit = committer.head_commit()?;
    Ok(commit)
}

/// # Create a remote repository
/// Takes the current directory name, and creates a repository on the server we can sync to. Returns the remote URL.
pub fn create_remote(repo: &LocalRepository, host: &str) -> Result<RemoteRepository, OxenError> {
    api::remote::repositories::create(repo, host)
}

/// # Set the remote for a repository
/// Tells the CLI where to push the changes to
pub fn set_remote(
    repo: &mut LocalRepository,
    name: &str,
    url: &str,
) -> Result<RemoteRepository, OxenError> {
    repo.set_remote(name, url);
    repo.save_default()?;
    Ok(RemoteRepository::from_local(repo, url))
}

/// # Get a log of all the commits
///
/// ```
/// use liboxen::command;
/// use liboxen::util;
/// # use liboxen::error::OxenError;
/// # use std::path::Path;
/// # fn main() -> Result<(), OxenError> {
///
/// // Initialize the repository
/// let base_dir = Path::new("/tmp/repo_dir_push");
/// let mut repo = command::init(base_dir)?;
///
/// // Write file to disk
/// let hello_file = base_dir.join("hello.txt");
/// util::fs::write_to_path(&hello_file, "Hello World");
///
/// // Stage the file
/// command::add(&repo, &hello_file)?;
///
/// // Commit staged
/// command::commit(&repo, "My commit message")?;
///
/// // Set the remote server
/// command::set_remote(&mut repo, "origin", "http://hub.oxen.ai/repositories/hello");
///
/// // Push the file
/// command::push(&repo);
///
/// # std::fs::remove_dir_all(base_dir)?;
/// # Ok(())
/// # }
/// ```
pub fn push(repo: &LocalRepository) -> Result<RemoteRepository, OxenError> {
    let indexer = Indexer::new(repo)?;
    let rb = RemoteBranch::default();
    indexer.push(&rb)
}

/// Push to a specific remote
pub fn push_remote_branch(
    repo: &LocalRepository,
    remote: &str,
    branch: &str,
) -> Result<RemoteRepository, OxenError> {
    let indexer = Indexer::new(repo)?;
    let rb = RemoteBranch {
        remote: String::from(remote),
        branch: String::from(branch),
    };
    indexer.push(&rb)
}

/// Clone a repo from a url to a directory
pub fn clone(url: &str, dst: &Path) -> Result<LocalRepository, OxenError> {
    LocalRepository::clone_remote(url, dst)?.ok_or_else(|| OxenError::remote_repo_not_found(url))
}

/// Pull a repository's data from origin/main
pub fn pull(repo: &LocalRepository) -> Result<(), OxenError> {
    let indexer = Indexer::new(repo)?;
    let rb = RemoteBranch::default();
    indexer.pull(&rb)?;
    Ok(())
}

/// Pull a specific origin and branch
pub fn pull_remote_branch(
    repo: &LocalRepository,
    remote: &str,
    branch: &str,
) -> Result<(), OxenError> {
    let indexer = Indexer::new(repo)?;
    let rb = RemoteBranch {
        remote: String::from(remote),
        branch: String::from(branch),
    };
    indexer.pull(&rb)?;
    Ok(())
}

/// Inspect a key value database for debugging
pub fn inspect(path: &Path) -> Result<(), OxenError> {
    let mut opts = Options::default();
    opts.set_log_level(LogLevel::Fatal);
    let db = DB::open_for_read_only(&opts, path, false)?;
    let iter = db.iterator(IteratorMode::Start);
    for (key, value) in iter {
        if let (Ok(key), Ok(value)) = (str::from_utf8(&key), str::from_utf8(&value)) {
            println!("{}\t{}", key, value)
        }
    }
    Ok(())
}
