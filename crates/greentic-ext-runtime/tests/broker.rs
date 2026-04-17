use greentic_ext_contract::ExtensionKind;
use greentic_ext_runtime::{Broker, BrokerError};

#[test]
fn denies_call_without_permission() {
    let broker = Broker::new();
    let err = broker
        .check_permission("caller", &["design".to_string()], ExtensionKind::Bundle)
        .unwrap_err();
    assert!(matches!(err, BrokerError::PermissionDenied(_)));
}

#[test]
fn allows_call_when_kind_in_allowlist() {
    let broker = Broker::new();
    broker
        .check_permission(
            "caller",
            &["design".to_string(), "bundle".to_string()],
            ExtensionKind::Bundle,
        )
        .unwrap();
}

#[test]
fn enforces_max_depth() {
    let broker = Broker::new();
    let err = broker.check_depth(9).unwrap_err();
    assert!(matches!(err, BrokerError::MaxDepthExceeded));
}
