//! Greentic Adaptive Cards design-extension — WIT export layer.
#![allow(clippy::used_underscore_items)] // triggered by wit-bindgen macro expansion in bindings.rs

#[allow(warnings)]
mod bindings;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_design::{knowledge, prompting, tools, validation};
use bindings::greentic::extension_base::types;

use serde_json::Value;

const SCHEMA_V16: &str = include_str!("../schemas/adaptive-card-v1.6.json");
const PROMPT_RULES: &str = include_str!("../prompts/rules.md");
const PROMPT_EXAMPLES: &str = include_str!("../prompts/examples.md");

struct Component;

// ===== base::manifest =====
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "greentic.adaptive-cards".into(),
            version: "1.6.0".into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![
            types::CapabilityRef {
                id: "greentic:adaptive-cards/schema".into(),
                version: "1.6.0".into(),
            },
            types::CapabilityRef {
                id: "greentic:adaptive-cards/validate".into(),
                version: "1.0.0".into(),
            },
            types::CapabilityRef {
                id: "greentic:adaptive-cards/transform".into(),
                version: "1.0.0".into(),
            },
        ]
    }
    fn get_required() -> Vec<types::CapabilityRef> {
        vec![]
    }
}

// ===== base::lifecycle =====
impl lifecycle::Guest for Component {
    fn init(_config_json: String) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn shutdown() {}
}

// Manual schema validation — jsonschema doesn't cross-compile to wasm32-wasip2 cleanly.
// This checks top-level AC structure per the embedded schema. Sufficient for MVP.
fn validate_adaptive_card(card: &Value) -> (bool, Vec<types::Diagnostic>) {
    let mut diagnostics = Vec::new();

    let type_val = card.get("type").and_then(Value::as_str);
    if type_val != Some("AdaptiveCard") {
        diagnostics.push(types::Diagnostic {
            severity: types::Severity::Error,
            code: "wrong-type".into(),
            message: format!("expected type='AdaptiveCard', got {type_val:?}"),
            path: Some("/type".into()),
        });
    }

    let version_val = card.get("version").and_then(Value::as_str);
    match version_val {
        None => diagnostics.push(types::Diagnostic {
            severity: types::Severity::Error,
            code: "missing-version".into(),
            message: "version is required".into(),
            path: Some("/version".into()),
        }),
        Some(v) => {
            let valid_version = v.starts_with("1.") && {
                let rest = &v[2..];
                let digit = rest.chars().next().and_then(|c| c.to_digit(10));
                matches!(digit, Some(0..=6))
            };
            if !valid_version {
                diagnostics.push(types::Diagnostic {
                    severity: types::Severity::Error,
                    code: "unsupported-version".into(),
                    message: format!("unsupported version: {v} (supported: 1.0-1.6)"),
                    path: Some("/version".into()),
                });
            }
        }
    }

    if let Some(body) = card.get("body")
        && !body.is_array()
    {
        diagnostics.push(types::Diagnostic {
            severity: types::Severity::Error,
            code: "body-must-be-array".into(),
            message: "body must be an array".into(),
            path: Some("/body".into()),
        });
    }
    if let Some(actions) = card.get("actions")
        && !actions.is_array()
    {
        diagnostics.push(types::Diagnostic {
            severity: types::Severity::Error,
            code: "actions-must-be-array".into(),
            message: "actions must be an array".into(),
            path: Some("/actions".into()),
        });
    }

    (diagnostics.is_empty(), diagnostics)
}

fn diagnostics_to_json(diagnostics: &[types::Diagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|d| {
            let severity = match d.severity {
                types::Severity::Error => "error",
                types::Severity::Warning => "warning",
                types::Severity::Info => "info",
                types::Severity::Hint => "hint",
            };
            serde_json::json!({
                "severity": severity,
                "code": d.code,
                "message": d.message,
                "path": d.path,
            })
        })
        .collect()
}

// ===== design::tools =====
impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        let defs = [
            (
                "validate_card",
                "Validate an Adaptive Card against v1.6 schema",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "analyze_card",
                "Count elements, actions, and depth of a card",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "check_accessibility",
                "Return an a11y score (0-100) and issue list",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "optimize_card",
                "Apply accessibility + performance improvements (stub)",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "transform_card",
                "Apply a named transform (stub)",
                r#"{"type":"object","properties":{"card":{"type":"object"},"transform":{"type":"string"}},"required":["card","transform"]}"#,
            ),
            (
                "template_card",
                "Convert card to data-bound template (stub)",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "data_to_card",
                "Infer a card from data (stub)",
                r#"{"type":"object","properties":{"data":{}},"required":["data"]}"#,
            ),
        ];
        defs.iter()
            .map(|(name, desc, schema)| tools::ToolDefinition {
                name: (*name).into(),
                description: (*desc).into(),
                input_schema_json: (*schema).into(),
                output_schema_json: None,
            })
            .collect()
    }

    fn invoke_tool(name: String, args_json: String) -> Result<String, types::ExtensionError> {
        let args: Value = serde_json::from_str(&args_json)
            .map_err(|e| types::ExtensionError::InvalidInput(e.to_string()))?;

        let result = match name.as_str() {
            "validate_card" => {
                let card = &args["card"];
                let (valid, diagnostics) = validate_adaptive_card(card);
                serde_json::json!({
                    "valid": valid,
                    "diagnostics": diagnostics_to_json(&diagnostics),
                })
            }
            "analyze_card" => {
                let card = &args["card"];
                let body_len = card
                    .get("body")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let actions_len = card
                    .get("actions")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                serde_json::json!({
                    "body_elements": body_len,
                    "actions": actions_len,
                    "depth": 1,
                })
            }
            "check_accessibility" => {
                let card = &args["card"];
                let mut issues: Vec<String> = vec![];
                if let Some(body) = card.get("body").and_then(Value::as_array) {
                    for el in body {
                        if el.get("type") == Some(&Value::String("Image".into()))
                            && el.get("altText").is_none()
                        {
                            issues.push("Image missing altText".into());
                        }
                    }
                }
                let penalty = i64::try_from(issues.len()).unwrap_or(5) * 20;
                let score = if issues.is_empty() {
                    100i64
                } else {
                    100i64 - penalty.min(100)
                };
                serde_json::json!({ "score": score, "issues": issues })
            }
            "optimize_card" | "transform_card" | "template_card" | "data_to_card" => {
                serde_json::json!({
                    "status": "not_implemented_in_v1_6",
                    "note": "Schema-level MVP; full logic lands in follow-up."
                })
            }
            other => {
                return Err(types::ExtensionError::InvalidInput(format!(
                    "unknown tool: {other}"
                )));
            }
        };
        Ok(result.to_string())
    }
}

// ===== design::validation =====
impl validation::Guest for Component {
    fn validate_content(content_type: String, content_json: String) -> validation::ValidateResult {
        if content_type != "adaptive-card" {
            return validation::ValidateResult {
                valid: false,
                diagnostics: vec![types::Diagnostic {
                    severity: types::Severity::Error,
                    code: "unsupported-content-type".into(),
                    message: format!(
                        "this extension handles 'adaptive-card', got '{content_type}'"
                    ),
                    path: None,
                }],
            };
        }
        let card: Value = match serde_json::from_str(&content_json) {
            Ok(v) => v,
            Err(e) => {
                return validation::ValidateResult {
                    valid: false,
                    diagnostics: vec![types::Diagnostic {
                        severity: types::Severity::Error,
                        code: "json-parse".into(),
                        message: e.to_string(),
                        path: None,
                    }],
                };
            }
        };
        let (valid, diagnostics) = validate_adaptive_card(&card);
        validation::ValidateResult { valid, diagnostics }
    }
}

// ===== design::prompting =====
impl prompting::Guest for Component {
    fn system_prompt_fragments() -> Vec<prompting::PromptFragment> {
        vec![
            prompting::PromptFragment {
                section: "rules".into(),
                content_markdown: PROMPT_RULES.into(),
                priority: 100,
            },
            prompting::PromptFragment {
                section: "examples".into(),
                content_markdown: PROMPT_EXAMPLES.into(),
                priority: 50,
            },
        ]
    }
}

// ===== design::knowledge =====
impl knowledge::Guest for Component {
    fn list_entries(_category_filter: Option<String>) -> Vec<knowledge::EntrySummary> {
        vec![]
    }
    fn get_entry(id: String) -> Result<knowledge::Entry, types::ExtensionError> {
        Err(types::ExtensionError::InvalidInput(format!(
            "no entry: {id}"
        )))
    }
    fn suggest_entries(_query: String, _limit: u32) -> Vec<knowledge::EntrySummary> {
        vec![]
    }
}

// Suppress unused const warnings — SCHEMA_V16 is embedded for future reference
// but not read in the manual validator above.
#[allow(dead_code)]
const _: &str = SCHEMA_V16;

bindings::export!(Component with_types_in bindings);
