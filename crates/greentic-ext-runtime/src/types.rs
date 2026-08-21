/// Host-side mirror of WIT `greentic:extension-design/tools@0.2.0::tool-definition`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
    pub output_schema_json: Option<String>,
}

/// Host-side mirror of WIT `greentic:extension-design/prompting@0.2.0::prompt-fragment`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptFragment {
    pub section: String,
    pub content_markdown: String,
    pub priority: u32,
}

/// Host-side mirror of WIT `greentic:extension-design/knowledge@0.2.0::entry-summary`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeEntrySummary {
    pub id: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// Host-side mirror of WIT `greentic:extension-design/knowledge@0.2.0::entry`.
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

/// Host-side mirror of WIT `greentic:extension-design/validation@0.2.0::validate-result`.
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

/// Host-side mirror of WIT
/// `greentic:extension-design/roles@0.2.0::target-kind`.
///
/// Output channel a compiled role targets. Closed enum: a new target
/// requires a WIT minor bump on the design package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetKind {
    AdaptiveCard,
    SlackBlockKit,
    TeamsCard,
    PlainText,
}

/// Host-side mirror of WIT
/// `greentic:extension-design/roles@0.2.0::role-spec`.
///
/// One role advertised by an extension. Aggregated across every loaded
/// design extension into a single registry keyed by `name`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoleSpec {
    pub name: String,
    pub description: String,
    pub json_schema: String,
    pub target: TargetKind,
    pub schema_version: u32,
    pub context_aware: bool,
}

/// Host-side mirror of WIT
/// `greentic:extension-design/roles@0.2.0::compile-context`.
///
/// Flow-level context handed to context-aware compilers. Empty for pure
/// roles. Designer fills this from the in-progress DSL document before
/// dispatch.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CompileContext {
    pub flow_entries_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_id: Option<String>,
    pub locale: String,
}

/// Host-side mirror of WIT
/// `greentic:extension-base/types@0.1.0::extension-error`.
///
/// Host-level failure that a role compiler may surface. Mirrored here
/// so [`RoleError::Host`] can carry the variant without dragging in
/// the bindgen-generated type at the public API boundary.
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "message")]
pub enum HostExtensionError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("missing capability: {0}")]
    MissingCapability(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// Host-side mirror of WIT
/// `greentic:extension-design/roles@0.2.0::role-error`.
///
/// Why a `compile-role` call failed. Distinct from
/// [`crate::error::RuntimeError`]: `RuntimeError` represents host /
/// runtime failures (extension-not-found, signature, IO), while
/// `RoleError` represents domain-level outcomes the LLM and designer
/// can act on (unknown role, invalid input, target not supported).
#[derive(Debug, Clone, thiserror::Error)]
pub enum RoleError {
    #[error("unknown role: {0}")]
    UnknownRole(String),
    #[error("invalid input: {0:?}")]
    InvalidInput(Vec<Diagnostic>),
    #[error("compile failed: {0}")]
    CompileFailed(String),
    #[error("target not supported: {0:?}")]
    TargetNotSupported(TargetKind),
    #[error("schema version not supported: {0}")]
    VersionNotSupported(u32),
    #[error("host: {0}")]
    Host(#[from] HostExtensionError),
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
