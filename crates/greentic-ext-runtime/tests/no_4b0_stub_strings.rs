//! B.7 grep gate (in-tree). Pins the spec's success criterion that
//! no host-fn impl returns the historical "not implemented in 4B.0"
//! sentinel string.
//!
//! This test runs from the repo root via `CARGO_MANIFEST_DIR`. It
//! intentionally checks the source tree (not the compiled binary) so
//! it catches both `Err("...")` returns and `format!(...)` constructions
//! that re-introduce the literal.

use std::path::PathBuf;

const SENTINEL: &str = "not implemented in 4B.0";

#[test]
fn no_4b0_sentinel_in_runtime_sources() {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits: Vec<String> = Vec::new();
    walk(&src_dir, &mut |path, contents| {
        for (i, line) in contents.lines().enumerate() {
            if line.contains(SENTINEL) {
                hits.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    });
    assert!(
        hits.is_empty(),
        "B.7 grep gate failed — sentinel \"{SENTINEL}\" still present:\n  {}",
        hits.join("\n  ")
    );
}

fn walk(dir: &std::path::Path, on_file: &mut dyn FnMut(&std::path::Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, on_file);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs")
            && let Ok(contents) = std::fs::read_to_string(&path)
        {
            on_file(&path, &contents);
        }
    }
}
