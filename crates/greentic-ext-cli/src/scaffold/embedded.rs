//! Embedded WIT resources accessor.

use include_dir::{Dir, include_dir};

#[allow(dead_code)] // consumed by Task 16 (orchestration) for contract-lock metadata
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/embedded-wit/$CARGO_PKG_VERSION");

#[allow(dead_code)] // fields consumed by Task 5 (per-kind filter + sha256) and Task 16
pub struct WitFile {
    pub name: &'static str,
    pub bytes: &'static [u8],
}

#[allow(dead_code)] // consumed by Task 5 and Task 16
pub fn wit_files() -> Vec<WitFile> {
    EMBEDDED
        .files()
        .map(|f| WitFile {
            name: f
                .path()
                .file_name()
                .and_then(|s| s.to_str())
                .expect("embedded wit filename"),
            bytes: f.contents(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wit_files_returns_all_embedded_packages() {
        let files = wit_files();
        assert!(files.iter().any(|f| f.name == "extension-base.wit"));
        assert!(files.iter().any(|f| f.name == "extension-host.wit"));
        assert!(files.iter().any(|f| f.name == "extension-design.wit"));
        assert!(files.iter().any(|f| f.name == "extension-bundle.wit"));
        assert!(files.iter().any(|f| f.name == "extension-deploy.wit"));
        assert_eq!(files.len(), 6);
    }
}
