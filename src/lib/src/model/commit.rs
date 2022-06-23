use crate::util::oxen_date_format;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Commit {
    pub id: String,
    pub parent_ids: Vec<String>,
    pub message: String,
    pub author: String,
    #[serde(with = "oxen_date_format")]
    pub date: DateTime<Utc>,
}

// Hash on the id field so we can quickly look up
impl PartialEq for Commit {
    fn eq(&self, other: &Commit) -> bool {
        self.id == other.id
    }
}
impl Eq for Commit {}
impl Hash for Commit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Commit {
    pub fn to_uri_encoded(&self) -> String {
        serde_url_params::to_string(&self).unwrap()
    }

    pub fn date_to_str(&self) -> String {
        self.date.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    pub fn date_from_str(date: &str) -> DateTime<Utc> {
        let no_timezone = NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S").unwrap();
        DateTime::<Utc>::from_utc(no_timezone, Utc)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommitStats {
    pub commit: Commit,
    pub num_entries: usize, // this is how many entries are in our commit db
    pub num_synced_files: usize, // this is how many files are actually synced (in case we killed)
}

impl CommitStats {
    pub fn is_synced(&self) -> bool {
        self.num_entries == self.num_synced_files
    }
}
