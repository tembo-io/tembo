use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Packaged file
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PackagedFile {
    ControlFile {},
    SqlFile {},
    SharedObject {},
    Bitcode {},
    Extra {},
}

impl PackagedFile {
    pub fn from<P: AsRef<Path>>(path: P) -> Self {
        let extension = path.as_ref().extension();
        if let Some(ext) = extension {
            match ext.to_str() {
                Some("control") => PackagedFile::ControlFile {},
                Some("sql") => PackagedFile::SqlFile {},
                Some("so") => PackagedFile::SharedObject {},
                Some("bc") => PackagedFile::Bitcode {},
                Some(_) | None => PackagedFile::Extra {},
            }
        } else {
            PackagedFile::Extra {}
        }
    }
}

/// Package manifest
#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    #[serde(rename = "name")]
    pub extension_name: String,
    #[serde(rename = "version")]
    pub extension_version: String,
    pub sys: String,
    pub architecture: String,
    pub files: Option<HashMap<PathBuf, PackagedFile>>,
}

impl Manifest {
    pub fn merge(&mut self, other: Self) {
        if let Some(files) = other.files {
            self.files.replace(files);
        }
    }

    pub fn add_file<P: AsRef<Path> + Into<PathBuf>>(&mut self, path: P) -> &mut PackagedFile {
        let files = match self.files {
            None => {
                self.files.replace(HashMap::new());
                self.files.as_mut().unwrap()
            }
            Some(ref mut files) => files,
        };
        files.insert(
            path.as_ref().to_path_buf(),
            PackagedFile::from(path.as_ref()),
        );
        files.get_mut(path.as_ref()).unwrap()
    }
}
