use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    pub user: PathBuf,
    pub project: Option<PathBuf>,
}

impl DiscoveryPaths {
    #[must_use]
    pub fn new(user: PathBuf) -> Self {
        Self {
            user,
            project: None,
        }
    }

    #[must_use]
    pub fn with_project(mut self, project: PathBuf) -> Self {
        self.project = Some(project);
        self
    }

    #[must_use]
    pub fn all(&self) -> Vec<&PathBuf> {
        let mut v = vec![&self.user];
        if let Some(p) = &self.project {
            v.push(p);
        }
        v
    }
}

/// Scan a single kind directory (e.g. `~/.greentic/extensions/design/`).
/// Returns absolute paths to each extension subdirectory that contains a
/// `describe.json`. Returns empty vec if the directory doesn't exist.
pub fn scan_kind_dir(kind_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !kind_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(kind_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if entry.path().join("describe.json").exists() {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}
