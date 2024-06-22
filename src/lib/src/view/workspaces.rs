use serde::{Deserialize, Serialize};

use super::StatusMessage;

#[derive(Deserialize, Serialize, Debug)]
pub struct WorkspaceView {
    pub branch_name: String,
    pub identifier: String,
    pub resource_path: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct WorkspaceResponseView {
    #[serde(flatten)]
    pub status: StatusMessage,
    pub workspace: WorkspaceView,
}
