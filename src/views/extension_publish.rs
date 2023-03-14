//! This module handles the expected information an extension should have
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ExtensionUpload {
    pub name: String,
    pub vers: semver::Version,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
}
