//! `contribution_tool_to_definition` is the whole tool surface of a
//! `greentic.ai/v2` extension: `list_tools` short-circuits on the contract
//! version and never calls the wasm's `list-tools` export. Anything this
//! mapper drops is unreachable in production, silently — which is exactly how
//! `agentic_worker_metadata` and `output_schema` went missing.

use greentic_ext_runtime::contribution_tool_to_definition;
use greentic_extension_sdk_contract::describe::contributions::Tool;
use greentic_extension_sdk_contract::{AgenticWorkerMetadata, Cost, SideEffects};

const AW_META: &str = r#"{"usage_hint":"Execute a playbook","examples":[{"when":"asked to run one","input":{"playbook_id":"p"}}],"side_effects":"external","cost":"medium","confirmation_required":false}"#;

fn full_tool() -> Tool {
    serde_json::from_value(serde_json::json!({
        "name": "run_telco_playbook",
        "export": "greentic:extension-design/tools.invoke-tool",
        "capabilities": ["flow", "agentic_worker"],
        "description": "Run a telco-x playbook.",
        "input_schema": r#"{"type":"object"}"#,
        "output_schema": r#"{"type":"object","properties":{"card":{"type":"object"}}}"#,
        "agentic_worker_metadata": AW_META,
    }))
    .expect("fixture parses")
}

#[test]
fn every_declared_field_reaches_the_tool_definition() {
    let def = contribution_tool_to_definition(&full_tool());
    assert_eq!(def.name, "run_telco_playbook");
    assert_eq!(def.description, "Run a telco-x playbook.");
    assert_eq!(def.input_schema_json, r#"{"type":"object"}"#);
    assert_eq!(
        def.output_schema_json.as_deref(),
        Some(r#"{"type":"object","properties":{"card":{"type":"object"}}}"#),
        "output_schema was hardcoded to None before this change"
    );
    assert_eq!(
        def.agentic_worker_metadata.as_deref(),
        Some(AW_META),
        "agentic_worker_metadata was hardcoded to None before this change"
    );
    assert_eq!(
        def.capabilities.as_deref(),
        Some(["flow".to_string(), "agentic_worker".to_string()].as_slice())
    );
}

/// The blob is opaque to the runtime, so "it arrived" is not enough — it has
/// to survive as something the planning layer can actually decode.
#[test]
fn forwarded_metadata_still_decodes_into_the_typed_contract() {
    let def = contribution_tool_to_definition(&full_tool());
    let meta = AgenticWorkerMetadata::decode(
        def.agentic_worker_metadata
            .as_deref()
            .expect("metadata forwarded"),
    )
    .expect("decodes");
    assert_eq!(meta.usage_hint.as_deref(), Some("Execute a playbook"));
    assert_eq!(meta.side_effects, Some(SideEffects::External));
    assert_eq!(meta.cost, Some(Cost::Medium));
    assert_eq!(meta.confirmation_required, Some(false));
    assert_eq!(meta.examples.map(|e| e.len()), Some(1));
}

/// A half-declared tool must still be offered rather than vanishing — the
/// degradation is reported once per extension at load time, not fatal here.
#[test]
fn a_minimal_tool_still_maps_without_panicking() {
    let minimal: Tool = serde_json::from_value(serde_json::json!({
        "name": "bare",
        "export": "greentic:extension-design/tools.invoke-tool",
    }))
    .expect("fixture parses");

    let def = contribution_tool_to_definition(&minimal);
    assert_eq!(def.name, "bare");
    assert!(def.description.is_empty());
    assert!(def.input_schema_json.is_empty());
    assert!(def.output_schema_json.is_none());
    assert!(def.agentic_worker_metadata.is_none());
    assert!(
        def.capabilities.is_none(),
        "capabilities stays None here; the ['flow'] default is applied by consumers"
    );
}
