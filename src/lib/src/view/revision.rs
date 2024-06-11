use serde::{Deserialize, Serialize};

use crate::model::{Commit, Branch};

use super::StatusMessage;

#[derive(Serialize, Deserialize, Debug)]
pub struct ParseResourceResponse {
    pub status: StatusMessage,
    pub commit: Commit,
    pub branch: Option<Branch>,
    pub file_path: String,
}
