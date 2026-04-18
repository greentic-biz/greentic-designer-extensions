//! File-system watcher wrapper + path filter.

use std::path::Path;

/// Returns `true` when `path` (relative to the project root) is a file the dev
/// loop should rebuild on. Filters out `target/`, VCS metadata, editor swap
/// files, and OS droppings.
pub fn should_watch(path: &Path) -> bool {
    let comps: Vec<_> = path.components().collect();
    for c in &comps {
        let s = match c.as_os_str().to_str() {
            Some(s) => s,
            None => return false,
        };
        if matches!(
            s,
            "target" | ".git" | ".idea" | ".vscode" | "dist" | "node_modules" | ".DS_Store"
        ) {
            return false;
        }
    }
    let name = match path.file_name().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };
    if name.starts_with('~') || name.starts_with('.') && name.ends_with(".swp") {
        return false;
    }
    if name.ends_with(".swp") || name.ends_with(".tmp") || name.ends_with('~') {
        return false;
    }
    // Positive patterns: match any file under watched roots, or a known root-level file.
    let first = comps.first().and_then(|c| c.as_os_str().to_str()).unwrap_or("");
    matches!(first, "src" | "wit" | "i18n" | "schemas" | "prompts")
        || matches!(name, "Cargo.toml" | "describe.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn watches_src_wit_describe_cargo() {
        assert!(should_watch(&p("src/lib.rs")));
        assert!(should_watch(&p("wit/world.wit")));
        assert!(should_watch(&p("wit/deps/greentic/extension-base/world.wit")));
        assert!(should_watch(&p("describe.json")));
        assert!(should_watch(&p("Cargo.toml")));
        assert!(should_watch(&p("i18n/en.json")));
        assert!(should_watch(&p("schemas/input.json")));
        assert!(should_watch(&p("prompts/system.md")));
    }

    #[test]
    fn ignores_target_git_ide_dirs() {
        assert!(!should_watch(&p("target/debug/foo.wasm")));
        assert!(!should_watch(&p(".git/HEAD")));
        assert!(!should_watch(&p(".idea/workspace.xml")));
        assert!(!should_watch(&p(".vscode/settings.json")));
        assert!(!should_watch(&p("dist/out.zip")));
        assert!(!should_watch(&p("node_modules/x/index.js")));
    }

    #[test]
    fn ignores_editor_swap_and_backup_files() {
        assert!(!should_watch(&p("src/.lib.rs.swp")));
        assert!(!should_watch(&p("src/lib.rs.tmp")));
        assert!(!should_watch(&p("src/lib.rs~")));
        assert!(!should_watch(&p("~backup.rs")));
    }

    #[test]
    fn ignores_out_of_scope_root_files() {
        assert!(!should_watch(&p("README.md")));
        assert!(!should_watch(&p("LICENSE")));
        assert!(!should_watch(&p("build.sh")));
    }
}
