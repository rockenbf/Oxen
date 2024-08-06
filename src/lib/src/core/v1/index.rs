//! Reading and writing to the commit and merkle tree databases
//!

pub mod commit_db_reader;
pub mod commit_dir_entry_reader;
pub mod commit_entry_reader;
pub mod commit_entry_writer;
pub mod commit_metadata_db;
pub mod commit_reader;
pub mod commit_sync_status;
pub mod commit_validator;
pub mod commit_writer;
pub mod entry_indexer;
pub mod legacy_commit_dir_entry_reader;
pub mod legacy_commit_entry_reader;
pub mod legacy_schema_reader;
pub mod merge_conflict_db_reader;
pub mod merge_conflict_reader;
pub mod merge_conflict_writer;
pub mod merger;
pub mod object_db_reader;
pub mod oxenignore;
pub mod puller;
pub mod pusher;
pub mod ref_db_reader;
pub mod ref_reader;
pub mod ref_writer;
pub mod restore;
pub mod rm;
pub mod schema_reader;
pub mod schema_writer;
pub mod staged_dir_entry_db;
pub mod staged_dir_entry_reader;
pub mod stager;
pub mod tree_db_reader;
pub mod tree_object_reader;
pub mod versioner;
pub mod workspaces;

pub use crate::core::v1::index::commit_db_reader::CommitDBReader;
pub use crate::core::v1::index::commit_entry_writer::CommitEntryWriter;
pub use crate::core::v1::index::commit_reader::CommitReader;
pub use crate::core::v1::index::commit_writer::CommitWriter;
pub use crate::core::v1::index::entry_indexer::EntryIndexer;

pub use crate::core::v1::index::commit_dir_entry_reader::CommitDirEntryReader;
pub use crate::core::v1::index::commit_entry_reader::CommitEntryReader;
pub use crate::core::v1::index::legacy_commit_dir_entry_reader::LegacyCommitDirEntryReader;
pub use crate::core::v1::index::legacy_commit_entry_reader::LegacyCommitEntryReader;
pub use crate::core::v1::index::legacy_schema_reader::LegacySchemaReader;
pub use crate::core::v1::index::merge_conflict_db_reader::MergeConflictDBReader;
pub use crate::core::v1::index::merge_conflict_reader::MergeConflictReader;
pub use crate::core::v1::index::merger::Merger;
pub use crate::core::v1::index::object_db_reader::ObjectDBReader;
pub use crate::core::v1::index::ref_db_reader::RefDBReader;
pub use crate::core::v1::index::ref_reader::RefReader;
pub use crate::core::v1::index::ref_writer::RefWriter;
pub use crate::core::v1::index::restore::restore;
pub use crate::core::v1::index::rm::rm;
pub use crate::core::v1::index::schema_reader::SchemaReader;
pub use crate::core::v1::index::schema_writer::SchemaWriter;
pub use crate::core::v1::index::staged_dir_entry_db::StagedDirEntryDB;
pub use crate::core::v1::index::staged_dir_entry_reader::StagedDirEntryReader;
pub use crate::core::v1::index::stager::Stager;
pub use crate::core::v1::index::tree_object_reader::TreeObjectReader;

use crate::model::LocalRepository;

pub fn is_v1(repo: &LocalRepository) -> bool {
    // We added the tree db in v2, so if it doesn't exist, we are in v1
    !repo.path.join(".oxen").join("tree").exists()
}
