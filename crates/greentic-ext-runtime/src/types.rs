/// Host-side mirror of WIT `greentic:extension-design/tools@0.1.0::tool-definition`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
    pub output_schema_json: Option<String>,
}

/// Host-side mirror of WIT `greentic:extension-design/prompting@0.1.0::prompt-fragment`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptFragment {
    pub section: String,
    pub content_markdown: String,
    pub priority: u32,
}

/// Host-side mirror of WIT `greentic:extension-design/knowledge@0.1.0::entry-summary`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeEntrySummary {
    pub id: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// Host-side mirror of WIT `greentic:extension-design/knowledge@0.1.0::entry`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub content_json: String,
}

/// Host-side mirror of WIT `greentic:extension-base/types@0.1.0::severity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// Host-side mirror of WIT `greentic:extension-base/types@0.1.0::diagnostic`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Host-side mirror of WIT `greentic:extension-deploy/targets@0.1.0::target-summary`.
///
/// Returned by `ExtensionRuntime::list_targets` for each deploy target a
/// loaded extension declares.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TargetSummary {
    pub id: String,
    pub display_name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
    pub supports_rollback: bool,
}

/// Host-side mirror of WIT `greentic:extension-design/validation@0.1.0::validate-result`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidateResult {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
}

/// Host-side mirror of WIT
/// `greentic:extension-bundle/bundling@0.1.0::designer-session`.
///
/// The full payload the host hands to a bundle extension to render.
/// `flows_json` and `contents_json` are pre-serialised JSON blobs from
/// the designer; `assets` carries auxiliary file bytes (images, fonts,
/// vendored resources) keyed by their relative path inside the bundle.
#[derive(Debug, Clone, Default)]
pub struct BundleSession {
    pub flows_json: String,
    pub contents_json: String,
    pub assets: Vec<(String, Vec<u8>)>,
    pub capabilities_used: Vec<String>,
}

/// Host-side mirror of WIT
/// `greentic:extension-bundle/bundling@0.1.0::bundle-artifact`.
///
/// What `bundling.render` returns on success — the rendered artefact
/// bytes (typically a `.gtpack` zip) plus its filename and sha256 for
/// integrity checks. The host writes the bytes to disk verbatim.
#[derive(Debug, Clone)]
pub struct BundleArtifact {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

#[cfg(test)]
mod target_summary_tests {
    use super::*;

    #[test]
    fn target_summary_serializes_and_deserializes() {
        let t = TargetSummary {
            id: "aws-ecs-fargate-local".into(),
            display_name: "AWS ECS Fargate (local creds)".into(),
            description: "Deploy to AWS ECS Fargate using ambient credentials.".into(),
            icon_path: Some("icons/aws.svg".into()),
            supports_rollback: true,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TargetSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, t.id);
        assert!(back.supports_rollback);
    }
}
