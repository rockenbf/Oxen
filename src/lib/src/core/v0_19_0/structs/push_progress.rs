use crate::core::v0_19_0::structs::sync_progress::{SyncProgress, SyncType};
use std::{borrow::Cow, sync::Arc};

pub struct PushProgress {
    sync_progress: Arc<SyncProgress>,
}

impl PushProgress {
    pub fn new() -> Arc<Self> {
        Arc::new(PushProgress {
            sync_progress: SyncProgress::new(SyncType::Push),
        })
    }

    pub fn set_message(&self, message: impl Into<Cow<'static, str>>) {
        self.sync_progress.set_message(message);
    }

    pub fn update_message(&self) {
        self.sync_progress.update_message();
    }

    pub fn add_files(&self, files: u64) {
        self.sync_progress.add_files(files);
    }

    pub fn add_bytes(&self, bytes: u64) {
        self.sync_progress.add_bytes(bytes);
    }

    pub fn get_num_files(&self) -> u64 {
        self.sync_progress.get_num_files()
    }

    pub fn get_num_bytes(&self) -> u64 {
        self.sync_progress.get_num_bytes()
    }

    pub fn finish(&self) {
        self.sync_progress.finish();
    }
}
