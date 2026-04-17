use greentic_ext_contract::ExtensionKind;
use greentic_ext_registry::local::LocalFilesystemRegistry;
use greentic_ext_registry::{ExtensionRegistry, SearchQuery};
use greentic_ext_testing::{ExtensionFixtureBuilder, pack_directory};
use tempfile::TempDir;

#[tokio::test]
async fn local_registry_finds_and_fetches_packed_extension() {
    let tmp = TempDir::new().unwrap();
    let reg_root = tmp.path().to_path_buf();

    let fixture =
        ExtensionFixtureBuilder::new(ExtensionKind::Design, "greentic.local-demo", "0.1.0")
            .offer("greentic:demo/hi", "1.0.0")
            .with_wasm(b"not-a-real-wasm".to_vec())
            .build()
            .unwrap();
    let pack_path = reg_root.join("greentic.local-demo-0.1.0.gtxpack");
    pack_directory(fixture.root(), &pack_path).unwrap();

    let reg = LocalFilesystemRegistry::new("local", reg_root);

    let results = reg.search(SearchQuery::default()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "greentic.local-demo");

    let art = reg.fetch("greentic.local-demo", "0.1.0").await.unwrap();
    assert_eq!(art.version, "0.1.0");
    assert!(!art.bytes.is_empty());

    let versions = reg.list_versions("greentic.local-demo").await.unwrap();
    assert_eq!(versions, vec!["0.1.0"]);
}

#[tokio::test]
async fn local_registry_returns_not_found_for_missing() {
    let tmp = TempDir::new().unwrap();
    let reg = LocalFilesystemRegistry::new("local", tmp.path().to_path_buf());
    let err = reg.fetch("greentic.missing", "0.1.0").await.unwrap_err();
    match err {
        greentic_ext_registry::RegistryError::NotFound { name, version } => {
            assert_eq!(name, "greentic.missing");
            assert_eq!(version, "0.1.0");
        }
        other => panic!("unexpected error: {other}"),
    }
}
