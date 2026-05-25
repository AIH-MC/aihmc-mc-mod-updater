use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndex {
    pub game: String,
    pub format_version: i32,
    pub version_id: String,
    pub name: String,
    pub files: Vec<ModrinthFile>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

impl ModrinthIndex {
    pub fn get_sha512(&self, file_path: &str) -> Option<&str> {
        self.files.iter()
            .find(|f| f.path == file_path)
            .and_then(|f| f.hashes.get("sha512"))
            .map(|s| s.as_str())
    }
}
