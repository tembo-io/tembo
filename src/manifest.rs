use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Packaged file
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PackagedFile {
    ControlFile {
        name: PathBuf,
    },
    SqlFile {
        name: PathBuf,
    },
    SharedObject {
        name: PathBuf,
        architecture: Option<String>,
    },
    Bitcode {
        name: PathBuf,
    },
    Extra {
        name: PathBuf,
    },
}

impl PackagedFile {
    pub fn from<P: AsRef<Path>>(path: P) -> Self {
        let extension = path.as_ref().extension();
        if let Some(ext) = extension {
            match ext.to_str() {
                Some("control") => PackagedFile::ControlFile {
                    name: path.as_ref().to_path_buf(),
                },
                Some("sql") => PackagedFile::SqlFile {
                    name: path.as_ref().to_path_buf(),
                },
                Some("so") => PackagedFile::SharedObject {
                    name: path.as_ref().to_path_buf(),
                    architecture: None,
                },
                Some("bc") => PackagedFile::Bitcode {
                    name: path.as_ref().to_path_buf(),
                },
                Some(_) | None => PackagedFile::Extra {
                    name: path.as_ref().to_path_buf(),
                },
            }
        } else {
            PackagedFile::Extra {
                name: path.as_ref().to_path_buf(),
            }
        }
    }
}

/// Package manifest
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "name")]
    pub extension_name: String,
    #[serde(rename = "version")]
    pub extension_version: String,
    pub sys: String,
    pub files: Option<Vec<PackagedFile>>,
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
                self.files.replace(Vec::new());
                self.files.as_mut().unwrap()
            }
            Some(ref mut files) => files,
        };
        files.push(PackagedFile::from(path));
        files.last_mut().unwrap()
    }
}
