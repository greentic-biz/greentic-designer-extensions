use greentic_ext_runtime::ToolDefinition;

#[test]
fn legacy_shape_deserializes_without_new_fields() {
    // Simulates what a JSON serialization of a legacy ToolDefinition
    // (pre-capability-flag) looks like. Both new fields absent.
    let json = r#"{
        "name": "validate_card",
        "description": "Validate an Adaptive Card.",
        "input_schema_json": "{}"
    }"#;
    let td: ToolDefinition = serde_json::from_str(json).expect("legacy decode");
    assert_eq!(td.name, "validate_card");
    assert!(td.capabilities.is_none());
    assert!(td.agentic_worker_metadata.is_none());
}

#[test]
fn new_shape_round_trip() {
    let td = ToolDefinition {
        name: "validate_card".into(),
        description: "Validate an Adaptive Card.".into(),
        input_schema_json: "{}".into(),
        output_schema_json: None,
        capabilities: Some(vec!["flow".into(), "agentic_worker".into()]),
        agentic_worker_metadata: Some(r#"{"usage_hint":"x"}"#.into()),
    };
    let json = serde_json::to_string(&td).unwrap();
    let back: ToolDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, td.name);
    assert_eq!(back.capabilities, td.capabilities);
    assert_eq!(back.agentic_worker_metadata, td.agentic_worker_metadata);
}

#[test]
fn legacy_default_capability_per_spec() {
    // Per spec section "Default semantics": when `capabilities` is None,
    // consumers MUST treat it as ["flow"]. This test asserts the policy
    // by simulating the consumer-side decision.
    let td = ToolDefinition {
        name: "legacy".into(),
        description: String::new(),
        input_schema_json: "{}".into(),
        output_schema_json: None,
        capabilities: None,
        agentic_worker_metadata: None,
    };
    let effective: Vec<String> = td
        .capabilities
        .clone()
        .unwrap_or_else(|| vec!["flow".into()]);
    assert_eq!(effective, vec!["flow".to_string()]);
}
