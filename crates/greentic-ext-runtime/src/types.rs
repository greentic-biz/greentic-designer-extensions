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
