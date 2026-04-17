use std::path::PathBuf;

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
