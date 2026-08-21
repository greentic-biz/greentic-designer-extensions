//! Load-time report of the metadata a `greentic.ai/v2` extension's declared
//! tools leave out.
//!
//! For a v2 extension `describe.json` is the only source of tool metadata —
//! [`crate::ExtensionRuntime::list_tools`] never calls the wasm's `list-tools`
//! export on that contract — so an omitted field is absent for the tool's whole
//! life. That is a property of the loaded artifact, which is why it is reported
//! once when the artifact is loaded rather than every time its tools are
//! listed: `list_tools` runs per request, so warning from the mapper repeated
//! the whole burst on ordinary traffic.

use greentic_extension_sdk_contract::DescribeJson;

/// The contract version under which `describe.json` is the sole source of tool
/// metadata. Mirrors the check in [`crate::ExtensionRuntime::list_tools`].
const V2_API_VERSION: &str = "greentic.ai/v2";

/// Tool names spelled out per category before the rest are counted. Keeps a
/// 16-tool extension from producing an unreadable log line.
const MAX_NAMES_SHOWN: usize = 3;

/// The metadata gaps across one v2 extension's declared tools.
///
/// A tool can appear in more than one category; `tools` is the total the
/// extension declares, so the categories can be read as fractions of it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ToolMetadataGaps {
    pub tools: usize,
    pub no_description: Vec<String>,
    pub no_input_schema: Vec<String>,
    pub no_capabilities: Vec<String>,
}

/// Collect the metadata gaps in `describe`, or `None` when there is nothing to
/// report — either the extension is not on the v2 contract (v1 still reads its
/// tool metadata from the WIT export, so a silent v1 describe is not a defect)
/// or every declared tool is fully declared.
pub(crate) fn collect_tool_metadata_gaps(describe: &DescribeJson) -> Option<ToolMetadataGaps> {
    if describe.api_version != V2_API_VERSION {
        return None;
    }

    let tools = &describe.contributions.tools;
    let mut gaps = ToolMetadataGaps {
        tools: tools.len(),
        no_description: Vec::new(),
        no_input_schema: Vec::new(),
        no_capabilities: Vec::new(),
    };

    for t in tools {
        if t.description.as_ref().is_none_or(|d| d.trim().is_empty()) {
            gaps.no_description.push(t.name.clone());
        }
        if t.input_schema.as_ref().is_none_or(|s| s.trim().is_empty()) {
            gaps.no_input_schema.push(t.name.clone());
        }
        if t.capabilities.is_none() {
            gaps.no_capabilities.push(t.name.clone());
        }
    }

    let nothing_missing = gaps.no_description.is_empty()
        && gaps.no_input_schema.is_empty()
        && gaps.no_capabilities.is_empty();

    (!nothing_missing).then_some(gaps)
}

/// Emit at most one WARN for the whole extension.
///
/// Kept at WARN: an omission is a real defect in the loaded artifact, and the
/// two consequences differ. A missing `description`/`input_schema` leaves the
/// LLM unable to call the tool correctly; a missing `capabilities` defaults to
/// `["flow"]`, which withholds the tool from the agentic-worker surface. Either
/// way the symptom is silence, with nothing anywhere saying why.
pub(crate) fn report_tool_metadata_gaps(describe: &DescribeJson) {
    let Some(gaps) = collect_tool_metadata_gaps(describe) else {
        return;
    };

    tracing::warn!(
        extension = %describe.metadata.id,
        tools = gaps.tools,
        no_description = %format_names(&gaps.no_description),
        no_input_schema = %format_names(&gaps.no_input_schema),
        no_capabilities = %format_names(&gaps.no_capabilities),
        "v2 extension declares tools with missing metadata; without description or input_schema \
         the LLM cannot infer how to call them, and without capabilities they default to \
         [\"flow\"] and are NOT offered on the agentic-worker surface"
    );
}

/// Render one category as `"<count> (<first names>, +N more)"`, or `"0"` when
/// the category is clean.
fn format_names(names: &[String]) -> String {
    if names.is_empty() {
        return "0".to_string();
    }
    let shown = names.len().min(MAX_NAMES_SHOWN);
    let elided = names.len() - shown;
    let more = if elided == 0 {
        String::new()
    } else {
        format!(", +{elided} more")
    };
    format!("{} ({}{more})", names.len(), names[..shown].join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    use greentic_extension_sdk_contract::describe::contributions::Tool;
    use greentic_extension_sdk_contract::{DescribeJson, ExtensionKind};

    fn tool(name: &str, description: Option<&str>, input_schema: Option<&str>) -> Tool {
        Tool {
            name: name.into(),
            export: "greentic:extension-design/tools.invoke-tool".into(),
            runtime_ref: None,
            capabilities: None,
            secret_requirements: Vec::new(),
            description: description.map(Into::into),
            input_schema: input_schema.map(Into::into),
            output_schema: None,
            agentic_worker_metadata: None,
        }
    }

    fn complete_tool(name: &str) -> Tool {
        Tool {
            capabilities: Some(vec!["flow".into()]),
            ..tool(name, Some("Does a thing."), Some(r#"{"type":"object"}"#))
        }
    }

    /// A real v2 describe, straight from the fixture builder every other test
    /// in this crate uses, with only the fields under test replaced.
    fn describe_with(api_version: &str, tools: Vec<Tool>) -> DescribeJson {
        let fixture = greentic_extension_sdk_testing::ExtensionFixtureBuilder::new(
            ExtensionKind::Design,
            "greentic.sorla",
            "1.0.0",
        )
        .build()
        .expect("fixture build");
        let raw = std::fs::read_to_string(fixture.root().join("describe.json"))
            .expect("fixture describe.json");
        let mut describe: DescribeJson =
            serde_json::from_str(&raw).expect("fixture describe parses");
        describe.api_version = api_version.into();
        describe.contributions.tools = tools;
        describe
    }

    #[test]
    fn gaps_are_grouped_into_one_report_naming_the_affected_tools() {
        let describe = describe_with(
            "greentic.ai/v2",
            vec![
                tool("parse_sorla_yaml", None, None),
                tool("apply_sorla_patch", Some("Apply a patch."), None),
                complete_tool("render_sorla_view"),
            ],
        );

        let gaps = collect_tool_metadata_gaps(&describe).expect("gaps reported");

        assert_eq!(gaps.tools, 3);
        assert_eq!(gaps.no_description, ["parse_sorla_yaml"]);
        assert_eq!(
            gaps.no_input_schema,
            ["parse_sorla_yaml", "apply_sorla_patch"]
        );
        assert_eq!(
            gaps.no_capabilities,
            ["parse_sorla_yaml", "apply_sorla_patch"]
        );
    }

    #[test]
    fn a_fully_declared_v2_describe_reports_nothing() {
        let describe = describe_with(
            "greentic.ai/v2",
            vec![complete_tool("alpha"), complete_tool("beta")],
        );

        assert!(collect_tool_metadata_gaps(&describe).is_none());
    }

    /// v1 reads tool metadata from the WIT `list-tools` export, so a silent v1
    /// describe is the contract working as designed, not a defect.
    #[test]
    fn a_v1_describe_reports_nothing_even_when_the_describe_declares_nothing() {
        let describe = describe_with("greentic.ai/v1", vec![tool("bare", None, None)]);

        assert!(collect_tool_metadata_gaps(&describe).is_none());
    }

    /// The mapper treated blank strings as absent (`trim().is_empty()`); the
    /// report has to agree, or a whitespace description would silently pass.
    #[test]
    fn blank_strings_count_as_missing() {
        let describe = describe_with("greentic.ai/v2", vec![tool("blank", Some("  "), Some(""))]);

        let gaps = collect_tool_metadata_gaps(&describe).expect("gaps reported");

        assert_eq!(gaps.no_description, ["blank"]);
        assert_eq!(gaps.no_input_schema, ["blank"]);
    }

    #[test]
    fn an_extension_with_no_tools_reports_nothing() {
        let describe = describe_with("greentic.ai/v2", vec![]);

        assert!(collect_tool_metadata_gaps(&describe).is_none());
    }

    /// Collects everything [`report_tool_metadata_gaps`] emits on this thread,
    /// so "at most one WARN per extension" is asserted against real output
    /// rather than inferred from the gap computation alone.
    fn captured_report(describe: &DescribeJson) -> String {
        let buffer = SharedBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(buffer.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, || report_tool_metadata_gaps(describe));
        let bytes = buffer.0.lock().expect("log buffer lock").clone();
        String::from_utf8(bytes).expect("log output is utf-8")
    }

    #[derive(Clone, Default)]
    struct SharedBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("log buffer lock")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBuffer {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn a_gappy_v2_extension_emits_exactly_one_warn_naming_the_tools() {
        let describe = describe_with(
            "greentic.ai/v2",
            vec![
                tool("parse_sorla_yaml", None, None),
                tool("apply_sorla_patch", Some("Apply a patch."), None),
                complete_tool("render_sorla_view"),
            ],
        );

        let output = captured_report(&describe);

        assert_eq!(
            output.lines().count(),
            1,
            "one extension must produce one line, got:\n{output}"
        );
        assert!(output.contains("WARN"), "{output}");
        assert!(output.contains("extension=greentic.sorla"), "{output}");
        assert!(output.contains("tools=3"), "{output}");
        assert!(output.contains("parse_sorla_yaml"), "{output}");
        assert!(output.contains("apply_sorla_patch"), "{output}");
    }

    #[test]
    fn a_fully_declared_v2_extension_emits_nothing() {
        let describe = describe_with("greentic.ai/v2", vec![complete_tool("alpha")]);

        assert_eq!(captured_report(&describe), "");
    }

    #[test]
    fn a_v1_extension_emits_nothing() {
        let describe = describe_with("greentic.ai/v1", vec![tool("bare", None, None)]);

        assert_eq!(captured_report(&describe), "");
    }

    #[test]
    fn a_short_name_list_is_spelled_out_in_full() {
        let names = ["a".to_string(), "b".to_string(), "c".to_string()];

        assert_eq!(format_names(&names), "3 (a, b, c)");
    }

    #[test]
    fn a_long_name_list_is_truncated_so_the_line_stays_readable() {
        let names: Vec<String> = (0..16).map(|i| format!("tool_{i}")).collect();

        assert_eq!(
            format_names(&names),
            "16 (tool_0, tool_1, tool_2, +13 more)"
        );
    }

    #[test]
    fn an_empty_category_formats_as_a_bare_zero() {
        assert_eq!(format_names(&[]), "0");
    }
}
