use std::fs;

use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::discovery::scan_kind_dir;
use greentic_ext_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

#[test]
fn scans_kind_directory_and_returns_extension_paths() {
    let tmp = TempDir::new().unwrap();
    let design_dir = tmp.path().join("design");
    fs::create_dir_all(&design_dir).unwrap();

    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.first",
        "0.1.0",
    )
    .offer("greentic:first/y", "1.0.0")
    .with_wasm(wat::parse_str("(component)").unwrap())
    .build()
    .unwrap();

    let target = design_dir.join("greentic.first-0.1.0");
    fs::create_dir_all(&target).unwrap();
    for entry in fs::read_dir(fixture.root()).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
    }

    let found = scan_kind_dir(&design_dir).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].ends_with("greentic.first-0.1.0"));
}
