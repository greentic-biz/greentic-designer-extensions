//! Greentic Adaptive Cards design-extension — WIT export layer.
//!
//! Wraps `adaptive-card-core` (from `greenticai/greentic-adaptive-card-mcp`)
//! and exposes its 8 tools through the design sub-WIT interfaces.
#![allow(clippy::used_underscore_items)] // triggered by wit-bindgen macro expansion in bindings.rs

#[allow(warnings)]
mod bindings;

use adaptive_card_core as core;
use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_design::{knowledge, prompting, tools, validation};
use bindings::greentic::extension_base::types;
use core::types::{CardVersion, DataToCardOpts, Host, OptimizeOpts, TransformTarget};
use serde_json::Value;

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
            types::CapabilityRef {
                id: "greentic:adaptive-cards/host-compat".into(),
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

fn parse_host_opt(v: &Value) -> Option<Host> {
    v.as_str().and_then(Host::from_str)
}

fn parse_card_version(s: &str) -> Option<CardVersion> {
    CardVersion::parse(s)
}

fn diagnostics_from_validation(report: &core::types::ValidationReport) -> Vec<types::Diagnostic> {
    let mut out: Vec<types::Diagnostic> = report
        .schema_errors
        .iter()
        .map(|e| types::Diagnostic {
            severity: types::Severity::Error,
            code: e.keyword.clone(),
            message: e.message.clone(),
            path: Some(e.path.clone()),
        })
        .collect();
    for issue in &report.accessibility.issues {
        out.push(types::Diagnostic {
            severity: types::Severity::Warning,
            code: format!("a11y:{}", issue.rule),
            message: issue.message.clone(),
            path: Some(issue.path.clone()),
        });
    }
    if let Some(hc) = &report.host_compat
        && !hc.compatible
    {
        for el in &hc.unsupported_elements {
            out.push(types::Diagnostic {
                severity: types::Severity::Warning,
                code: "host-compat:unsupported-element".into(),
                message: format!("element {el} not supported by {:?}", hc.host),
                path: None,
            });
        }
        for action in &hc.unsupported_actions {
            out.push(types::Diagnostic {
                severity: types::Severity::Warning,
                code: "host-compat:unsupported-action".into(),
                message: format!("action {action} not supported by {:?}", hc.host),
                path: None,
            });
        }
    }
    out
}

// ===== design::tools =====
impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        let defs = [
            (
                "validate_card",
                "Validate an Adaptive Card against v1.6 schema + a11y + optional host compat",
                r#"{"type":"object","properties":{"card":{"type":"object"},"host":{"type":"string","description":"generic|teams|outlook|webchat|windows|viva|webex"}},"required":["card"]}"#,
            ),
            (
                "analyze_card",
                "Element / action / depth counts + duplicate ID detection",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "check_accessibility",
                "WCAG-style a11y score (0-100) + issue list",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "optimize_card",
                "Apply accessibility / performance / modernize transforms",
                r#"{"type":"object","properties":{"card":{"type":"object"},"opts":{"type":"object","properties":{"accessibility":{"type":"boolean"},"performance":{"type":"boolean"},"modernize":{"type":"boolean"},"target_host":{"type":"string"}}}},"required":["card"]}"#,
            ),
            (
                "transform_card",
                "Apply version downgrade and/or host adaptation",
                r#"{"type":"object","properties":{"card":{"type":"object"},"version":{"type":"string","description":"1.0..1.6"},"host":{"type":"string"},"strict":{"type":"boolean"}},"required":["card"]}"#,
            ),
            (
                "template_card",
                "Convert static card into data-bound template with ${expr} bindings",
                r#"{"type":"object","properties":{"card":{"type":"object"}},"required":["card"]}"#,
            ),
            (
                "data_to_card",
                "Infer a card from input data (table / factset / list / chart / auto)",
                r#"{"type":"object","properties":{"data":{},"opts":{"type":"object"}},"required":["data"]}"#,
            ),
            (
                "check_host_compat",
                "Check whether a card is compatible with a target host",
                r#"{"type":"object","properties":{"card":{"type":"object"},"host":{"type":"string"}},"required":["card","host"]}"#,
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

        let result: Value = match name.as_str() {
            "validate_card" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let host = args.get("host").and_then(parse_host_opt);
                let report = core::validate_card(&card, host);
                serde_json::to_value(&report)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "analyze_card" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let analysis = core::analyze_card(&card);
                serde_json::to_value(&analysis)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "check_accessibility" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let report = core::check_accessibility(&card);
                serde_json::to_value(&report)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "optimize_card" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let opts: OptimizeOpts = args
                    .get("opts")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| types::ExtensionError::InvalidInput(format!("opts: {e}")))?
                    .unwrap_or_default();
                let report = core::optimize_card(card, &opts);
                serde_json::to_value(&report)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "transform_card" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let version = args.get("version").and_then(Value::as_str).and_then(parse_card_version);
                let host = args.get("host").and_then(parse_host_opt);
                let strict = args.get("strict").and_then(Value::as_bool).unwrap_or(false);
                let target = TransformTarget { version, host, strict };
                let report = core::transform_card(card, &target)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?;
                serde_json::to_value(&report)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "template_card" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let result = core::template_card(card);
                serde_json::to_value(&result)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "data_to_card" => {
                let data = args.get("data").cloned().unwrap_or(Value::Null);
                let opts: DataToCardOpts = args
                    .get("opts")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| types::ExtensionError::InvalidInput(format!("opts: {e}")))?
                    .unwrap_or(DataToCardOpts {
                        title: None,
                        presentation: None,
                        host: Host::Generic,
                    });
                core::data_to_card(&data, &opts)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
            }
            "check_host_compat" => {
                let card = args.get("card").cloned().unwrap_or(Value::Null);
                let host = args
                    .get("host")
                    .and_then(parse_host_opt)
                    .unwrap_or(Host::Generic);
                let report = core::check_compatibility(&card, host);
                serde_json::to_value(&report)
                    .map_err(|e| types::ExtensionError::Internal(e.to_string()))?
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
        let report = core::validate_card(&card, None);
        let diagnostics = diagnostics_from_validation(&report);
        validation::ValidateResult {
            valid: report.valid,
            diagnostics,
        }
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
// Knowledge base ships empty in v1; advanced samples curated separately.
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

bindings::export!(Component with_types_in bindings);
