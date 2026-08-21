# greentic.tavily Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `component-tavily-ext`, a WASM design extension that exposes two agentic-worker tools — `tavily_search` and `tavily_extract` — over the host `http` + `secrets` capabilities.

**Architecture:** Mirror the proven `component-http-ext` crate exactly: a thin WIT-export layer (`src/lib.rs`) that resolves the API key via host `secrets::get`, POSTs to `api.tavily.com` via host `http::fetch`, and maps the response. All request/response shaping lives in pure, host-testable modules (`input.rs`, `output.rs`, `tool_meta.rs`) with no WIT imports. The `export!` macro is gated behind `#[cfg(target_family = "wasm")]` so the pure modules unit-test on the host via `cargo test`.

**Tech Stack:** Rust edition 2024, `cargo-component` → `wasm32-wasip2`, `wit-bindgen` 0.41, `serde`/`serde_json`, packaged into a `.gtxpack` by `build.sh`.

**Spec:** [`../specs/2026-05-30-greentic-tavily-extension-design.md`](../specs/2026-05-30-greentic-tavily-extension-design.md)

**Reference crate (copy this):** `/Users/bimapangestu/Desktop/Works/personal/greentic/component-http-ext`

---

## Conventions for every task

- Work in the new repo `/Users/bimapangestu/Desktop/Works/personal/greentic/component-tavily-ext` on branch `feat/tavily-extension`.
- **Host tests** (the pure modules) run with plain `cargo test` (host target). They MUST pass before each commit.
- **WASM build** (`cargo component build --release --target wasm32-wasip2`) is the integration check; run it in Task 1 and Task 6.
- Commit after each task with a conventional-commit message. No `Co-Authored-By` / AI attribution.
- Tavily API reference used throughout:
  - `POST https://api.tavily.com/search` — header `Authorization: Bearer <key>`, JSON body `{query, search_depth, topic, max_results, include_answer, include_domains?, exclude_domains?, time_range?}`. Response `{query, answer?, results:[{title,url,content,score}], ...}`.
  - `POST https://api.tavily.com/extract` — JSON body `{urls:[...], extract_depth}`. Response `{results:[{url,raw_content}], failed_results:[{url,error}]}`.

---

## Task 0: Bootstrap the crate from component-http-ext

Create a working copy of the reference crate, strip its artifacts, rename identifiers, and confirm it still builds to WASM with stub tools. This locks in the exact working `bindings.rs` + `wit/deps` + toolchain so later tasks only touch logic.

**Files:**
- Create dir: `component-tavily-ext/` (copy of `component-http-ext/`)
- Modify: `component-tavily-ext/Cargo.toml`
- Modify: `component-tavily-ext/wit/world.wit`
- Modify: `component-tavily-ext/describe.json`
- Modify: `component-tavily-ext/src/lib.rs` (identity strings only)

- [ ] **Step 1: Copy the reference crate and strip artifacts**

```bash
cd /Users/bimapangestu/Desktop/Works/personal/greentic
cp -r component-http-ext component-tavily-ext
cd component-tavily-ext
rm -rf .git target Cargo.lock greentic.http-0.1.0.gtxpack
git init -q
git checkout -b feat/tavily-extension
```

- [ ] **Step 2: Rename the crate in `Cargo.toml`**

Replace the `[package]` `name`, `description`, and the `[package.metadata.component]` `package` line. The WIT-deps paths stay unchanged. Final `Cargo.toml`:

```toml
[package]
name = "greentic-tavily-extension"
version = "0.1.0"
edition = "2024"
rust-version = "1.95.0"
license = "MIT"
publish = false
description = "Tavily web search/extract WASM design-extension for the Greentic agentic worker"

[lib]
crate-type = ["cdylib", "rlib"]
path = "src/lib.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
wit-bindgen = "0.41"
wit-bindgen-rt = "0.41"

[dev-dependencies]
serde_json = "1"

[package.metadata.component]
package = "greentic:tavily-extension"

[package.metadata.component.target]
path = "wit"
world = "design-extension"

[package.metadata.component.target.dependencies]
"greentic:extension-base" = { path = "wit/deps/extension-base/extension-base.wit" }
"greentic:extension-host" = { path = "wit/deps/extension-host/extension-host.wit" }
"greentic:extension-design" = { path = "wit/deps/extension-design/extension-design.wit" }

[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
missing_errors_doc = "allow"
missing_panics_doc = "allow"
module_name_repetitions = "allow"
used_underscore_items = "allow"
doc_markdown = "allow"
```

- [ ] **Step 3: Rename the WIT world package**

`wit/world.wit` — change only the package line; imports/exports are already exactly what Tavily needs:

```wit
package greentic:tavily-extension;

world design-extension {
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/http@0.1.0;
  import greentic:extension-host/secrets@0.1.0;

  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export greentic:extension-design/tools@0.2.0;
}
```

- [ ] **Step 4: Point `lib.rs` identity at the new id**

In `src/lib.rs`, change the `manifest::Guest` identity and offered capability (leave everything else for later tasks):

```rust
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "greentic.tavily".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "greentic:tavily/search".into(),
            version: "1.0.0".into(),
        }]
    }
    fn get_required() -> Vec<types::CapabilityRef> {
        vec![]
    }
}
```

- [ ] **Step 5: Rewrite `describe.json` for Tavily**

Replace the file with:

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v2.json",
  "apiVersion": "greentic.ai/v2",
  "kind": "DesignExtension",
  "compat": {
    "min_designer_version": ">=1.2.0",
    "min_runner_version": "^0.12.0",
    "contract_version": "1.2.0"
  },
  "metadata": {
    "id": "greentic.tavily",
    "name": "Tavily Web Search",
    "version": "0.1.0",
    "summary": "Tavily web search and content extraction tools for the agentic worker",
    "description": "Design extension exposing two agentic-worker tools: `tavily_search` (LLM-oriented web search with an optional summarised answer) and `tavily_extract` (clean content extraction from known URLs). The Tavily API key is injected by the host from a secret reference and never returned. Outbound calls are restricted to api.tavily.com.",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "MIT",
    "repository": "https://github.com/greentic-biz/component-tavily-ext",
    "keywords": ["tavily", "web-search", "research", "agentic-worker", "design-extension"]
  },
  "engine": {
    "greenticDesigner": ">=1.2.0",
    "extRuntime": "^1.2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:tavily/search", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 64,
    "permissions": {
      "network": [
        "https://api.tavily.com/*"
      ],
      "secrets": [
        "secret://tavily/"
      ],
      "callExtensionKinds": []
    },
    "components": {
      "tavily-tool": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
          "pack_id": "greentic.tavily",
          "component_version": "0.1.0"
        },
        "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        "world": "greentic:tavily-extension/design-extension@1.0.0"
      }
    }
  },
  "contributions": {
    "tools": [
      { "name": "tavily_search",  "export": "greentic:extension-design/tools.invoke-tool", "runtime_ref": "tavily-tool" },
      { "name": "tavily_extract", "export": "greentic:extension-design/tools.invoke-tool", "runtime_ref": "tavily-tool" }
    ]
  }
}
```

- [ ] **Step 6: Regenerate bindings + confirm the WASM build is green**

`cargo component build` regenerates `src/bindings.rs` from the renamed world (the file header says "Generated by wit-bindgen … DO NOT EDIT").

Run:
```bash
cargo component build --release --target wasm32-wasip2
```
Expected: `Finished` with no errors; produces `target/wasm32-wasip2/release/greentic_tavily_extension.wasm`. (The tools are still the inherited `http_request` stub at this point — that is replaced in Tasks 1–4.)

The copied `src/bindings.rs` describes the identical world (`manifest` + `lifecycle` + `tools@0.2.0`), so it compiles unchanged. If the build instead errors on a world/package mismatch (the committed bindings still name `greentic:http-extension`), force regeneration and rebuild:
```bash
rm -f src/bindings.rs && cargo component bindings && cargo component build --release --target wasm32-wasip2
```

- [ ] **Step 7: Confirm host tests still compile/run**

Run:
```bash
cargo test
```
Expected: the inherited `input`/`output`/`tool_meta` tests from http-ext PASS. (They are replaced wholesale in the next tasks.)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore(tavily): bootstrap design-extension crate from component-http-ext"
```

---

## Task 1: Tool metadata for both tools (`tool_meta.rs`)

Define the two tools' names, JSON schemas, and `agentic_worker` metadata. This is the file that makes the tools visible to the agentic worker (the `agentic_worker` capability flag).

**Files:**
- Replace: `component-tavily-ext/src/tool_meta.rs`
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Put this `tests` module at the bottom of the new `src/tool_meta.rs` (write the whole file in Step 2; this shows the tests first for TDD intent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_tools_declare_agentic_worker_capability() {
        for tool in [tavily_search_tool(), tavily_extract_tool()] {
            assert!(
                tool.capabilities.iter().any(|cap| cap == "agentic_worker"),
                "{} must opt into the agentic worker",
                tool.name
            );
        }
    }

    #[test]
    fn tool_names_match_constants() {
        assert_eq!(tavily_search_tool().name, "tavily_search");
        assert_eq!(tavily_extract_tool().name, "tavily_extract");
    }

    #[test]
    fn schemas_and_metadata_are_valid_json() {
        for tool in [tavily_search_tool(), tavily_extract_tool()] {
            serde_json::from_str::<serde_json::Value>(tool.input_schema_json)
                .expect("input schema is valid JSON");
            serde_json::from_str::<serde_json::Value>(tool.output_schema_json)
                .expect("output schema is valid JSON");
            let meta: serde_json::Value = serde_json::from_str(tool.agentic_worker_metadata)
                .expect("aw metadata is valid JSON");
            assert_eq!(meta["side_effects"], "read");
            assert_eq!(meta["confirmation_required"], false);
        }
    }
}
```

- [ ] **Step 2: Write the implementation (full file)**

Replace `src/tool_meta.rs` with:

```rust
//! Static metadata for the two Tavily tools: names, JSON schemas, capability
//! flags, and the `agentic_worker` metadata blob. Pure (no WIT imports) so the
//! `agentic_worker` opt-in is asserted by a host test.

pub const TAVILY_SEARCH_TOOL: &str = "tavily_search";
pub const TAVILY_EXTRACT_TOOL: &str = "tavily_extract";

const SEARCH_INPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["query"],
  "properties": {
    "query": { "type": "string", "description": "The search query" },
    "max_results": { "type": "integer", "minimum": 1, "maximum": 20, "description": "Number of results (default 5)" },
    "search_depth": { "type": "string", "enum": ["basic", "advanced"], "description": "Search depth (default basic)" },
    "topic": { "type": "string", "enum": ["general", "news"], "description": "Search topic (default general)" },
    "include_answer": { "type": "boolean", "description": "Include Tavily's summarised answer (default true)" },
    "include_domains": { "type": "array", "items": { "type": "string" }, "description": "Restrict to these domains" },
    "exclude_domains": { "type": "array", "items": { "type": "string" }, "description": "Exclude these domains" },
    "time_range": { "type": "string", "enum": ["day", "week", "month", "year"], "description": "Recency window" }
  }
}"#;

const SEARCH_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["results", "query"],
  "properties": {
    "answer": { "type": ["string", "null"] },
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "title": { "type": "string" },
          "url": { "type": "string" },
          "content": { "type": "string" },
          "score": { "type": "number" }
        }
      }
    },
    "query": { "type": "string" }
  }
}"#;

const SEARCH_AW_META: &str = r#"{
  "usage_hint": "Search the live web for current facts or anything outside the model's knowledge. Provide a query; optionally tune max_results, search_depth, topic, and domain filters. Returns a summarised answer plus ranked results with URLs and snippets.",
  "examples": [
    {
      "when": "the agentic worker needs current information from the web",
      "input": { "query": "latest stable Rust release version", "max_results": 5 }
    }
  ],
  "side_effects": "read",
  "cost": "medium",
  "confirmation_required": false
}"#;

const EXTRACT_INPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["urls"],
  "properties": {
    "urls": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "URLs to extract clean content from" },
    "extract_depth": { "type": "string", "enum": ["basic", "advanced"], "description": "Extraction depth (default basic)" }
  }
}"#;

const EXTRACT_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["results", "failed_results"],
  "properties": {
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "url": { "type": "string" },
          "raw_content": { "type": "string" }
        }
      }
    },
    "failed_results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "url": { "type": "string" },
          "error": { "type": "string" }
        }
      }
    }
  }
}"#;

const EXTRACT_AW_META: &str = r#"{
  "usage_hint": "Fetch clean, readable content from one or more known URLs (e.g. links returned by tavily_search). Returns extracted text per URL plus a list of URLs that failed.",
  "examples": [
    {
      "when": "the agentic worker has a URL and needs its full readable content",
      "input": { "urls": ["https://example.com/article"] }
    }
  ],
  "side_effects": "read",
  "cost": "medium",
  "confirmation_required": false
}"#;

/// Plain (non-WIT) description of one tool definition.
pub struct ToolMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema_json: &'static str,
    pub output_schema_json: &'static str,
    pub capabilities: Vec<String>,
    pub agentic_worker_metadata: &'static str,
}

#[must_use]
pub fn tavily_search_tool() -> ToolMeta {
    ToolMeta {
        name: TAVILY_SEARCH_TOOL,
        description: "Search the web with Tavily and return an optional summarised answer plus \
                      ranked results. The Tavily API key is injected by the host and never returned.",
        input_schema_json: SEARCH_INPUT_SCHEMA,
        output_schema_json: SEARCH_OUTPUT_SCHEMA,
        capabilities: vec!["agentic_worker".into()],
        agentic_worker_metadata: SEARCH_AW_META,
    }
}

#[must_use]
pub fn tavily_extract_tool() -> ToolMeta {
    ToolMeta {
        name: TAVILY_EXTRACT_TOOL,
        description: "Extract clean, readable content from known URLs using Tavily. The Tavily \
                      API key is injected by the host and never returned.",
        input_schema_json: EXTRACT_INPUT_SCHEMA,
        output_schema_json: EXTRACT_OUTPUT_SCHEMA,
        capabilities: vec!["agentic_worker".into()],
        agentic_worker_metadata: EXTRACT_AW_META,
    }
}
```
(Append the `tests` module from Step 1 to the end of this file.)

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib tool_meta`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit**

```bash
git add src/tool_meta.rs
git commit -m "feat(tavily): tool metadata + agentic_worker schemas for search/extract"
```

---

## Task 2: Request shaping (`input.rs`)

Pure module: decode tool args into typed inputs, validate, and build the Tavily request-body JSON for each endpoint.

**Files:**
- Replace: `component-tavily-ext/src/input.rs`
- Test: same file

- [ ] **Step 1: Write the failing tests**

These go in the `#[cfg(test)] mod tests` block (full file in Step 2):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_decodes_minimal_and_applies_defaults() {
        let input: SearchInput =
            serde_json::from_str(r#"{"query":"rust 2024 edition"}"#).unwrap();
        let body = build_search_body(&input).unwrap();
        assert_eq!(body["query"], "rust 2024 edition");
        assert_eq!(body["max_results"], 5);
        assert_eq!(body["search_depth"], "basic");
        assert_eq!(body["topic"], "general");
        assert_eq!(body["include_answer"], true);
        assert!(body.get("include_domains").is_none());
    }

    #[test]
    fn search_passes_through_optional_fields() {
        let input: SearchInput = serde_json::from_str(
            r#"{"query":"q","max_results":3,"search_depth":"advanced","topic":"news","include_answer":false,"include_domains":["a.com"],"time_range":"week"}"#,
        )
        .unwrap();
        let body = build_search_body(&input).unwrap();
        assert_eq!(body["max_results"], 3);
        assert_eq!(body["search_depth"], "advanced");
        assert_eq!(body["topic"], "news");
        assert_eq!(body["include_answer"], false);
        assert_eq!(body["include_domains"], json!(["a.com"]));
        assert_eq!(body["time_range"], "week");
    }

    #[test]
    fn search_rejects_empty_query() {
        let input: SearchInput = serde_json::from_str(r#"{"query":"   "}"#).unwrap();
        assert!(build_search_body(&input).is_err());
    }

    #[test]
    fn search_rejects_out_of_range_max_results() {
        let input: SearchInput =
            serde_json::from_str(r#"{"query":"q","max_results":0}"#).unwrap();
        assert!(build_search_body(&input).is_err());
        let input: SearchInput =
            serde_json::from_str(r#"{"query":"q","max_results":99}"#).unwrap();
        assert!(build_search_body(&input).is_err());
    }

    #[test]
    fn extract_decodes_and_applies_defaults() {
        let input: ExtractInput =
            serde_json::from_str(r#"{"urls":["https://x.com/a"]}"#).unwrap();
        let body = build_extract_body(&input).unwrap();
        assert_eq!(body["urls"], json!(["https://x.com/a"]));
        assert_eq!(body["extract_depth"], "basic");
    }

    #[test]
    fn extract_rejects_empty_urls() {
        let input: ExtractInput = serde_json::from_str(r#"{"urls":[]}"#).unwrap();
        assert!(build_extract_body(&input).is_err());
    }
}
```

- [ ] **Step 2: Write the implementation (full file)**

Replace `src/input.rs` with (then append the Step 1 tests):

```rust
//! Pure request-shaping logic for the Tavily tools.
//!
//! Free of any WIT/bindings/network imports so it unit-tests on the host via
//! `cargo test`. `lib.rs` calls these and supplies the actual transport
//! (`extension-host/http`) and secret resolution (`extension-host/secrets`).

use serde::Deserialize;
use serde_json::{json, Value};

/// Decoded `tavily_search` input.
#[derive(Debug, Deserialize)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub search_depth: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub include_answer: Option<bool>,
    #[serde(default)]
    pub include_domains: Option<Vec<String>>,
    #[serde(default)]
    pub exclude_domains: Option<Vec<String>>,
    #[serde(default)]
    pub time_range: Option<String>,
}

/// Decoded `tavily_extract` input.
#[derive(Debug, Deserialize)]
pub struct ExtractInput {
    pub urls: Vec<String>,
    #[serde(default)]
    pub extract_depth: Option<String>,
}

/// Build the JSON body for `POST /search`. Applies defaults and validates.
pub fn build_search_body(input: &SearchInput) -> Result<Value, String> {
    if input.query.trim().is_empty() {
        return Err("query must not be empty".to_string());
    }
    let max_results = input.max_results.unwrap_or(5);
    if !(1..=20).contains(&max_results) {
        return Err(format!("max_results must be between 1 and 20, got {max_results}"));
    }
    let mut body = json!({
        "query": input.query,
        "max_results": max_results,
        "search_depth": input.search_depth.clone().unwrap_or_else(|| "basic".to_string()),
        "topic": input.topic.clone().unwrap_or_else(|| "general".to_string()),
        "include_answer": input.include_answer.unwrap_or(true),
    });
    if let Some(domains) = &input.include_domains {
        body["include_domains"] = json!(domains);
    }
    if let Some(domains) = &input.exclude_domains {
        body["exclude_domains"] = json!(domains);
    }
    if let Some(range) = &input.time_range {
        body["time_range"] = json!(range);
    }
    Ok(body)
}

/// Build the JSON body for `POST /extract`. Applies defaults and validates.
pub fn build_extract_body(input: &ExtractInput) -> Result<Value, String> {
    if input.urls.is_empty() {
        return Err("urls must contain at least one URL".to_string());
    }
    Ok(json!({
        "urls": input.urls,
        "extract_depth": input.extract_depth.clone().unwrap_or_else(|| "basic".to_string()),
    }))
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib input`
Expected: PASS (6 tests).

- [ ] **Step 4: Commit**

```bash
git add src/input.rs
git commit -m "feat(tavily): typed inputs + request-body builders with validation"
```

---

## Task 3: Response mapping (`output.rs`)

Pure module: parse Tavily JSON responses into the stable output shapes.

**Files:**
- Replace: `component-tavily-ext/src/output.rs`
- Test: same file

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_search_response_with_answer_and_results() {
        let raw = br#"{
          "query": "rust",
          "answer": "Rust is a systems language.",
          "results": [
            {"title": "Rust", "url": "https://rust-lang.org", "content": "snippet", "score": 0.98}
          ],
          "response_time": 1.2
        }"#;
        let out = map_search_response(raw).unwrap();
        assert_eq!(out.query, "rust");
        assert_eq!(out.answer.as_deref(), Some("Rust is a systems language."));
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.results[0].url, "https://rust-lang.org");
        assert!((out.results[0].score - 0.98).abs() < f64::EPSILON);
    }

    #[test]
    fn maps_search_response_without_answer() {
        let raw = br#"{"query":"q","results":[]}"#;
        let out = map_search_response(raw).unwrap();
        assert!(out.answer.is_none());
        assert!(out.results.is_empty());
    }

    #[test]
    fn search_response_malformed_json_is_err() {
        assert!(map_search_response(b"{not json").is_err());
    }

    #[test]
    fn maps_extract_response_with_results_and_failures() {
        let raw = br#"{
          "results": [{"url": "https://x.com/a", "raw_content": "hello"}],
          "failed_results": [{"url": "https://x.com/b", "error": "timeout"}]
        }"#;
        let out = map_extract_response(raw).unwrap();
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.results[0].raw_content, "hello");
        assert_eq!(out.failed_results.len(), 1);
        assert_eq!(out.failed_results[0].error, "timeout");
    }

    #[test]
    fn extract_response_missing_failed_defaults_empty() {
        let raw = br#"{"results":[{"url":"u","raw_content":"c"}]}"#;
        let out = map_extract_response(raw).unwrap();
        assert!(out.failed_results.is_empty());
    }
}
```

- [ ] **Step 2: Write the implementation (full file)**

Replace `src/output.rs` with (then append the Step 1 tests):

```rust
//! Pure response-mapping logic for the Tavily tools.
//!
//! Free of WIT/bindings imports so it unit-tests on the host. We deserialize
//! the Tavily payloads with permissive structs (unknown fields ignored) and
//! re-serialize the stable shape the tool returns to the agent.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SearchOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default)]
    pub results: Vec<SearchResult>,
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ExtractResult {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub raw_content: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FailedResult {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ExtractOutput {
    #[serde(default)]
    pub results: Vec<ExtractResult>,
    #[serde(default)]
    pub failed_results: Vec<FailedResult>,
}

/// Parse a Tavily `/search` response body into [`SearchOutput`].
pub fn map_search_response(body: &[u8]) -> Result<SearchOutput, String> {
    serde_json::from_slice::<SearchOutput>(body)
        .map_err(|error| format!("decode tavily search response: {error}"))
}

/// Parse a Tavily `/extract` response body into [`ExtractOutput`].
pub fn map_extract_response(body: &[u8]) -> Result<ExtractOutput, String> {
    serde_json::from_slice::<ExtractOutput>(body)
        .map_err(|error| format!("decode tavily extract response: {error}"))
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib output`
Expected: PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git add src/output.rs
git commit -m "feat(tavily): typed response mapping for search/extract"
```

---

## Task 4: Wire the WIT export layer (`lib.rs`)

Replace the inherited `http_request` glue with the Tavily dispatch: `list_tools` returns both tools; `invoke_tool` dispatches to search/extract, resolves the API key, calls the host, and maps the response + status.

**Files:**
- Modify: `component-tavily-ext/src/lib.rs`

- [ ] **Step 1: Replace the `tools::Guest` impl and add the helpers**

In `src/lib.rs`, update the imports near the top to drop the `HttpToolInput` use and bring in the new modules:

```rust
use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_design::tools;
use bindings::greentic::extension_base::types;
use bindings::greentic::extension_host::{http, secrets};

use input::{ExtractInput, SearchInput};
```

Replace the entire `impl tools::Guest for Component { ... }` block with:

```rust
// ===== design::tools =====

/// Secret URI for the Tavily API key. The host resolves this via the configured
/// secrets backend; the value never leaves this component. Permission is
/// granted by the `secret://tavily/` prefix entry in describe.json.
const TAVILY_KEY_REF: &str = "secret://tavily/api_key";

impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        [tool_meta::tavily_search_tool(), tool_meta::tavily_extract_tool()]
            .into_iter()
            .map(|meta| tools::ToolDefinition {
                name: meta.name.into(),
                description: meta.description.into(),
                input_schema_json: meta.input_schema_json.into(),
                output_schema_json: Some(meta.output_schema_json.into()),
                capabilities: Some(meta.capabilities),
                agentic_worker_metadata: Some(meta.agentic_worker_metadata.into()),
            })
            .collect()
    }

    fn invoke_tool(name: String, args_json: String) -> Result<String, types::ExtensionError> {
        match name.as_str() {
            tool_meta::TAVILY_SEARCH_TOOL => {
                let parsed: SearchInput = serde_json::from_str(&args_json).map_err(|error| {
                    types::ExtensionError::InvalidInput(format!("decode tavily_search input: {error}"))
                })?;
                let body = input::build_search_body(&parsed)
                    .map_err(types::ExtensionError::InvalidInput)?;
                let raw = tavily_post("https://api.tavily.com/search", &body)?;
                let output = output::map_search_response(&raw)
                    .map_err(types::ExtensionError::Internal)?;
                serde_json::to_string(&output).map_err(|error| {
                    types::ExtensionError::Internal(format!("encode output: {error}"))
                })
            }
            tool_meta::TAVILY_EXTRACT_TOOL => {
                let parsed: ExtractInput = serde_json::from_str(&args_json).map_err(|error| {
                    types::ExtensionError::InvalidInput(format!("decode tavily_extract input: {error}"))
                })?;
                let body = input::build_extract_body(&parsed)
                    .map_err(types::ExtensionError::InvalidInput)?;
                let raw = tavily_post("https://api.tavily.com/extract", &body)?;
                let output = output::map_extract_response(&raw)
                    .map_err(types::ExtensionError::Internal)?;
                serde_json::to_string(&output).map_err(|error| {
                    types::ExtensionError::Internal(format!("encode output: {error}"))
                })
            }
            other => Err(types::ExtensionError::InvalidInput(format!(
                "unknown tool: {other}"
            ))),
        }
    }
}

/// Resolve the API key, POST `body` as JSON to `url`, and return the response
/// body bytes for a 2xx status. Non-2xx is mapped to an `ExtensionError` with a
/// message the agent can act on (401/429 called out explicitly).
fn tavily_post(url: &str, body: &serde_json::Value) -> Result<Vec<u8>, types::ExtensionError> {
    let api_key = secrets::get(TAVILY_KEY_REF).map_err(|error| {
        types::ExtensionError::PermissionDenied(format!("resolve secret {TAVILY_KEY_REF}: {error}"))
    })?;

    let body_bytes = serde_json::to_vec(body)
        .map_err(|error| types::ExtensionError::Internal(format!("encode request body: {error}")))?;

    let request = http::Request {
        method: "POST".into(),
        url: url.into(),
        headers: vec![
            ("content-type".into(), "application/json".into()),
            ("authorization".into(), format!("Bearer {api_key}")),
        ],
        body: Some(body_bytes),
    };

    let response = http::fetch(&request)
        .map_err(|error| types::ExtensionError::Internal(format!("http fetch: {error}")))?;

    if (200..300).contains(&response.status) {
        return Ok(response.body);
    }

    let detail = String::from_utf8_lossy(&response.body);
    let message = match response.status {
        401 => format!("Tavily rejected the API key (401): {detail}"),
        429 => format!("Tavily rate limit exceeded (429): {detail}"),
        status => format!("Tavily request failed ({status}): {detail}"),
    };
    Err(types::ExtensionError::Internal(message))
}
```

Leave the `manifest`/`lifecycle` impls (from Task 0) and the gated `bindings::export!` line at the end untouched.

- [ ] **Step 2: Verify host tests still pass**

The pure-module tests are unaffected; confirm nothing broke:

Run: `cargo test`
Expected: PASS (all `tool_meta` + `input` + `output` tests; lib.rs has no host tests since it imports WIT bindings).

- [ ] **Step 3: Verify the WASM component builds**

Run: `cargo component build --release --target wasm32-wasip2`
Expected: `Finished`; produces `target/wasm32-wasip2/release/greentic_tavily_extension.wasm`.

- [ ] **Step 4: Lint (zero warnings)**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat(tavily): wire list_tools + invoke_tool dispatch over host http/secrets"
```

---

## Task 5: Packaging (`build.sh`) → `.gtxpack`

Adapt the inherited build script (already correct in structure) and produce a signed-shaped `.gtxpack` with the real wasm digest substituted into `describe.json`.

**Files:**
- Modify: `component-tavily-ext/build.sh` (verify the wasm artifact filename)

- [ ] **Step 1: Fix the wasm artifact path in `build.sh`**

The inherited script references `greentic_http_extension.wasm`. Change that one line to the new crate's artifact:

```bash
WASM_PATH="target/wasm32-wasip2/release/greentic_tavily_extension.wasm"
```

Everything else in the script (staging `describe.json` + `extension.wasm`, sha256 substitution of the two `0000…` placeholders, deriving `${EXT_ID}-${EXT_VERSION}.gtxpack` via `jq`) is already correct.

- [ ] **Step 2: Build the package**

Run:
```bash
bash build.sh
```
Expected output ends with `==> built …/greentic.tavily-0.1.0.gtxpack` and a size line.

- [ ] **Step 3: Verify the package contents + digest substitution**

Run:
```bash
unzip -l greentic.tavily-0.1.0.gtxpack
unzip -p greentic.tavily-0.1.0.gtxpack describe.json | jq '.runtime.components["tavily-tool"].sha256'
```
Expected: archive lists `describe.json` and `extension.wasm`; the printed sha256 is a real 64-hex digest (NOT all zeros).

- [ ] **Step 4: Commit**

```bash
git add build.sh
git commit -m "build(tavily): package extension into .gtxpack"
```

---

## Task 6: README + final verification gate

Document the extension and run the full local gate.

**Files:**
- Replace: `component-tavily-ext/README.md`

- [ ] **Step 1: Write the README**

Replace `README.md` with:

````markdown
# component-tavily-ext

A Greentic Designer **design extension** (WASM, `wasm32-wasip2`) exposing two
agentic-worker tools backed by the [Tavily](https://tavily.com) API:

- `tavily_search` — LLM-oriented web search with an optional summarised answer.
- `tavily_extract` — clean content extraction from known URLs.

Both tools declare the `agentic_worker` capability, so the DW Composer surfaces
them to the agentic worker. The Tavily API key is read from the host secret
`secret://tavily/api_key` and is never returned to the caller. Outbound calls
are restricted to `api.tavily.com`.

## Build

```sh
cargo test                                                   # host unit tests
cargo clippy --all-targets -- -D warnings
cargo component build --release --target wasm32-wasip2       # wasm component
bash build.sh                                                # produces .gtxpack
```

## Secret provisioning

The operator must store the tenant's Tavily API key in the configured secrets
backend under a URI matching the `secret://tavily/` permission prefix — the
extension reads `secret://tavily/api_key`.

## Tools

### `tavily_search`
Input: `{ query, max_results?, search_depth?, topic?, include_answer?, include_domains?, exclude_domains?, time_range? }`
Output: `{ answer?, results: [{ title, url, content, score }], query }`

### `tavily_extract`
Input: `{ urls: [string], extract_depth? }`
Output: `{ results: [{ url, raw_content }], failed_results: [{ url, error }] }`
````

- [ ] **Step 2: Run the full gate**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo component build --release --target wasm32-wasip2
bash build.sh
```
Expected: every command exits 0; `build.sh` prints the `.gtxpack` path.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(tavily): add README"
```

---

## Out of scope / follow-ups (tracked, not built here)

- **Live worker execution** depends on the `aw-runtime` `ConfigProvider`
  (manifest `extension_tools` → `AgentConfig.tools`), which has no concrete
  implementation yet (Phase 4). Building, installing, and surfacing the tools in
  the DW Composer (`GET /api/extensions/tools?capability=agentic_worker`) works
  today; end-to-end execution in a running worker is blocked until that lands.
- **Operator key provisioning UX** (how a tenant's Tavily key reaches the
  secrets backend) — out of scope for the extension itself.
- **GitHub direct-API extension** and **dwbase knowledge-base extension** —
  separate specs, each copying this crate as the template.
- **Signing** — `describe.json` carries no `signature` block; `gtdx publish`
  signing flow is a separate concern from this build.
```
