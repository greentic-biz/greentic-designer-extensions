use greentic_ext_runtime::{CapabilityRegistry, OfferedBinding};
use greentic_extension_sdk_contract::{CapabilityRef, ExtensionKind};

fn cap_ref(id: &str, v: &str) -> CapabilityRef {
    CapabilityRef {
        id: id.parse().unwrap(),
        version: v.to_string(),
    }
}

#[test]
fn matches_caret_version() {
    let mut r = CapabilityRegistry::new();
    r.add_offering(OfferedBinding {
        extension_id: "x.offerer".into(),
        cap_id: "greentic:x/y".parse().unwrap(),
        version: "1.2.5".parse().unwrap(),
        kind: ExtensionKind::Design,
        export_path: "ext/y.func".into(),
    });
    let plan = r.resolve("x.consumer", &[cap_ref("greentic:x/y", "^1.0")]);
    assert!(plan.unresolved.is_empty());
    assert_eq!(plan.resolved.len(), 1);
}

#[test]
fn degrades_on_missing_cap() {
    let r = CapabilityRegistry::new();
    let plan = r.resolve("x.consumer", &[cap_ref("greentic:nope/here", "^1.0")]);
    assert_eq!(plan.unresolved.len(), 1);
    assert!(plan.resolved.is_empty());
}

#[test]
fn picks_highest_compatible_semver() {
    let mut r = CapabilityRegistry::new();
    for v in ["1.0.0", "1.2.0", "1.5.0", "2.0.0"] {
        r.add_offering(OfferedBinding {
            extension_id: format!("x.offer-{v}"),
            cap_id: "greentic:x/y".parse().unwrap(),
            version: v.parse().unwrap(),
            kind: ExtensionKind::Design,
            export_path: "e".into(),
        });
    }
    let plan = r.resolve("x.consumer", &[cap_ref("greentic:x/y", "^1.0")]);
    let picked = plan.resolved.values().next().unwrap();
    assert_eq!(picked.version.to_string(), "1.5.0");
}
