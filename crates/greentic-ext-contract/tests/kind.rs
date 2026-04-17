use greentic_ext_contract::ExtensionKind;

#[test]
fn serializes_as_pascal_case_string() {
    assert_eq!(
        serde_json::to_string(&ExtensionKind::Design).unwrap(),
        "\"DesignExtension\""
    );
    assert_eq!(
        serde_json::to_string(&ExtensionKind::Bundle).unwrap(),
        "\"BundleExtension\""
    );
    assert_eq!(
        serde_json::to_string(&ExtensionKind::Deploy).unwrap(),
        "\"DeployExtension\""
    );
}

#[test]
fn round_trips_through_json() {
    for variant in [
        ExtensionKind::Design,
        ExtensionKind::Bundle,
        ExtensionKind::Deploy,
    ] {
        let s = serde_json::to_string(&variant).unwrap();
        let back: ExtensionKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn dir_name_matches_spec() {
    assert_eq!(ExtensionKind::Design.dir_name(), "design");
    assert_eq!(ExtensionKind::Bundle.dir_name(), "bundle");
    assert_eq!(ExtensionKind::Deploy.dir_name(), "deploy");
}
