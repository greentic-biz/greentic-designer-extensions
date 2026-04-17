use std::path::PathBuf;

use wit_parser::Resolve;

fn wit_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("wit")
}

#[test]
fn all_wit_files_parse() {
    // extension-base and extension-host have no cross-package dependencies and
    // must be loaded first. The three kind-specific packages depend on both.
    let ordered = [
        "extension-base.wit",
        "extension-host.wit",
        "extension-bundle.wit",
        "extension-deploy.wit",
        "extension-design.wit",
    ];
    let dir = wit_dir();
    let mut resolve = Resolve::new();
    for name in ordered {
        let path = dir.join(name);
        resolve
            .push_file(&path)
            .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
    }
}
