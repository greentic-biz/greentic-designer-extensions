use greentic_ext_runtime::{CapabilityRegistry, OfferedBinding};
use greentic_extension_sdk_contract::{CapabilityRef, ExtensionKind};

fn offer(ext: &str, cap: &str, v: &str) -> OfferedBinding {
    OfferedBinding {
        extension_id: ext.into(),
        cap_id: cap.parse().unwrap(),
        version: v.parse().unwrap(),
        kind: ExtensionKind::Design,
        export_path: "e".into(),
    }
}

fn require(id: &str, v: &str) -> CapabilityRef {
    CapabilityRef {
        id: id.parse().unwrap(),
        version: v.to_string(),
    }
}

#[test]
fn detects_direct_cycle() {
    let mut r = CapabilityRegistry::new();
    r.add_offering(offer("a", "greentic:a/offered", "1.0.0"));
    r.add_offering(offer("b", "greentic:b/offered", "1.0.0"));
    let cycle = r.detect_cycle(&[
        ("a".to_string(), vec![require("greentic:b/offered", "^1.0")]),
        ("b".to_string(), vec![require("greentic:a/offered", "^1.0")]),
    ]);
    assert!(cycle.contains(&"a".to_string()));
    assert!(cycle.contains(&"b".to_string()));
}

#[test]
fn no_cycle_for_linear_dependency() {
    let mut r = CapabilityRegistry::new();
    r.add_offering(offer("a", "greentic:a/offered", "1.0.0"));
    r.add_offering(offer("b", "greentic:b/offered", "1.0.0"));
    let cycle = r.detect_cycle(&[
        ("a".to_string(), vec![require("greentic:b/offered", "^1.0")]),
        ("b".to_string(), vec![]),
    ]);
    assert!(cycle.is_empty());
}
