# greentic.github-mcp Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `component-github-mcp-ext`, a WASM design extension that proxies the agentic worker to GitHub's remote MCP server (`https://api.githubcopilot.com/mcp/`) over host `http`, exposing GitHub's read tools via MCP `tools/list` / `tools/call`.

**Architecture:** Mirror the proven `component-tavily-ext` crate. A pure, host-testable module `mcp_proto.rs` builds JSON-RPC 2.0 requests and parses responses (unwrapping SSE), and maps `tools/list` → tool definitions. A thin `lib.rs` glue layer resolves the PAT via host `secrets`, runs the per-call MCP handshake (`initialize` → `notifications/initialized` → `tools/list`/`tools/call`) over host `http`, and maps failures to `ExtensionError`. `mcp_proto.rs` is added additively so the crate compiles at every task until the final `lib.rs` swap.

**Tech Stack:** Rust edition 2024, `cargo-component` → `wasm32-wasip2`, `wit-bindgen` 0.41, `serde_json`, MCP Streamable HTTP / JSON-RPC 2.0, packaged into `.gtxpack` by `build.sh`.

**Spec:** [`../specs/2026-05-30-greentic-github-mcp-extension-design.md`](../specs/2026-05-30-greentic-github-mcp-extension-design.md)

**Reference crate (copy this):** `/Users/bimapangestu/Desktop/Works/personal/greentic/component-tavily-ext`

---

## Conventions for every task

- Work in the new repo `/Users/bimapangestu/Desktop/Works/personal/greentic/component-github-mcp-ext` on branch `feat/github-mcp-extension`.
- Host tests (the pure module) run with plain `cargo test`. They MUST pass before each commit.
- WASM build (`cargo component build --release --target wasm32-wasip2`) is the integration check; run it in Tasks 0, 5, 7.
- The fresh repo has **no pre-commit hooks**; the implementer's `cargo test` / `cargo clippy` is the real gate. Run `cargo fmt --all` before any `cargo component build` in a gate (the build regenerates `src/bindings.rs` unformatted).
- Conventional commits; no AI attribution.
- MCP protocol reference (Streamable HTTP / JSON-RPC 2.0):
  - Endpoint `https://api.githubcopilot.com/mcp/`, header `Authorization: Bearer <PAT>`.
  - Filtering headers `X-MCP-Readonly: true`, `X-MCP-Toolsets: repos,issues,pull_requests`.
  - `Accept: application/json, text/event-stream`; responses may be JSON or SSE.
  - Flow: `initialize` (id 1) → `notifications/initialized` → `tools/list` / `tools/call` (id 2). Server may return `Mcp-Session-Id` to echo on later requests.

---

## Task 0: Bootstrap the crate from component-tavily-ext

Copy the Tavily crate, strip artifacts, rename to github-mcp, confirm the WASM build is green with the inherited (Tavily) tools still in place. Later tasks add `mcp_proto.rs` and swap `lib.rs`.

**Files:**
- Create dir: `component-github-mcp-ext/` (copy of `component-tavily-ext/`)
- Modify: `Cargo.toml`, `wit/world.wit`, `describe.json`, `src/lib.rs` (identity only)

- [ ] **Step 1: Copy and strip**

```bash
cd /Users/bimapangestu/Desktop/Works/personal/greentic
cp -r component-tavily-ext component-github-mcp-ext
cd component-github-mcp-ext
rm -rf .git target Cargo.lock *.gtxpack
git init -q
git checkout -b feat/github-mcp-extension
```

- [ ] **Step 2: `Cargo.toml`** — replace `name`, `description`, and `[package.metadata.component] package`:

```toml
[package]
name = "greentic-github-mcp-extension"
version = "0.1.0"
edition = "2024"
rust-version = "1.95.0"
license = "MIT"
publish = false
description = "GitHub remote-MCP proxy WASM design-extension for the Greentic agentic worker"

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
package = "greentic:github-mcp-extension"

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

- [ ] **Step 3: `wit/world.wit`** — change only the package line (imports/exports already correct: the world already imports `logging`, `http`, `secrets`):

```wit
package greentic:github-mcp-extension;

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

> If the copied Tavily `world.wit` does NOT already import `logging`, add the `import greentic:extension-host/logging@0.1.0;` line shown above — `lib.rs` will need it in Task 5.

- [ ] **Step 4: `src/lib.rs` identity only** — change the `manifest::Guest` identity + offered capability; leave the inherited Tavily `tools::Guest` impl, modules, and `lifecycle` untouched for now:

```rust
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "greentic.github-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "greentic:github-mcp/tools".into(),
            version: "1.0.0".into(),
        }]
    }
    fn get_required() -> Vec<types::CapabilityRef> {
        vec![]
    }
}
```

- [ ] **Step 5: `describe.json`** — replace with:

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
    "id": "greentic.github-mcp",
    "name": "GitHub (MCP)",
    "version": "0.1.0",
    "summary": "GitHub tools for the agentic worker, proxied to GitHub's official remote MCP server",
    "description": "Design extension that proxies the agentic worker to GitHub's remote MCP server (api.githubcopilot.com/mcp). It speaks MCP Streamable HTTP / JSON-RPC: list_tools enumerates GitHub's tools via tools/list and invoke_tool proxies tools/call. Runs read-only by default. The GitHub PAT is injected by the host from a secret reference and never returned.",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "MIT",
    "repository": "https://github.com/greentic-biz/component-github-mcp-ext",
    "keywords": ["github", "mcp", "agentic-worker", "design-extension"]
  },
  "engine": {
    "greenticDesigner": ">=1.2.0",
    "extRuntime": "^1.2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:github-mcp/tools", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 64,
    "permissions": {
      "network": [
        "https://api.githubcopilot.com/*"
      ],
      "secrets": [
        "secret://github/"
      ],
      "callExtensionKinds": []
    },
    "components": {
      "github-mcp-tool": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
          "pack_id": "greentic.github-mcp",
          "component_version": "0.1.0"
        },
        "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        "world": "greentic:github-mcp-extension/design-extension@1.0.0"
      }
    }
  },
  "contributions": {
    "tools": []
  }
}
```

> `contributions.tools` is intentionally empty: this extension's tools are discovered dynamically at runtime via the WASM `list_tools()` export, not statically from `describe.json`. If `gtdx`/validation rejects an empty array in a later task, add a single generic entry `{ "name": "github", "export": "greentic:extension-design/tools.invoke-tool", "runtime_ref": "github-mcp-tool" }` and note it.

- [ ] **Step 6: Regenerate bindings + build**

```bash
cargo component build --release --target wasm32-wasip2
```
Expected: `Finished`. If it errors on a world/package mismatch (copied bindings still name `greentic:tavily-extension`):
```bash
rm -f src/bindings.rs && cargo component bindings && cargo component build --release --target wasm32-wasip2
```

- [ ] **Step 7: Host tests still pass**

Run: `cargo test`
Expected: the inherited Tavily `tool_meta`/`input`/`output` tests PASS (they are removed in Task 5).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore(github-mcp): bootstrap design-extension crate from component-tavily-ext"
```

---

## Task 1: Live protocol verification (PAT-gated, controller-run)

Confirm the real GitHub MCP server behaviour before trusting the spec's assumptions. **This needs a real GitHub PAT** and is run by the controller/user, not an offline subagent. It produces knowledge + (optionally) real fixtures; the offline Tasks 2–5 use spec-shaped fixtures and are reconciled against whatever this task finds.

**Files:** none (verification only; optionally save captured responses under `tests/fixtures/`).

- [ ] **Step 1: Ping**

```bash
curl -sS -I https://api.githubcopilot.com/mcp/ -H "Authorization: Bearer $GITHUB_PAT" | head -5
```
Expected: a 2xx/4xx HTTP response (server reachable).

- [ ] **Step 2: Initialize + observe response framing and session header**

```bash
curl -sS -D - https://api.githubcopilot.com/mcp/ \
  -H "Authorization: Bearer $GITHUB_PAT" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "MCP-Protocol-Version: 2025-06-18" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"greentic-github-mcp","version":"0.1.0"}}}'
```
Record: the response `Content-Type` (is it `application/json` or `text/event-stream`?), whether a `Mcp-Session-Id` response header is present, and the `result.protocolVersion` the server reports.

- [ ] **Step 3: tools/list (echo the session id if step 2 returned one)**

```bash
curl -sS https://api.githubcopilot.com/mcp/ \
  -H "Authorization: Bearer $GITHUB_PAT" -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" -H "MCP-Protocol-Version: 2025-06-18" \
  -H "X-MCP-Readonly: true" -H "X-MCP-Toolsets: repos,issues,pull_requests" \
  -H "Mcp-Session-Id: <from step 2 if any>" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```
Record: a sample tool entry shape (`name`, `description`, `inputSchema`), and whether a prior `notifications/initialized` was required (if `tools/list` fails without it, send `{"jsonrpc":"2.0","method":"notifications/initialized"}` first and retry).

- [ ] **Step 4: Record findings**

Write the confirmed values into the plan's reconciliation note (below) and, if convenient, save the raw `initialize` and `tools/list` response bodies to `tests/fixtures/initialize.json|sse` and `tests/fixtures/tools_list.json|sse` for use as test fixtures in Tasks 3–4.

**Reconciliation:** if the server's real `protocolVersion`, response framing, or session requirement differs from the spec assumptions, update the `PROTOCOL_VERSION` constant (Task 5) and the SSE/JSON fixtures (Tasks 3–4) accordingly. No code is written in this task.

> If no PAT is available now, SKIP this task and proceed; Tasks 2–5 build against spec-shaped fixtures and the `mcp_proto` parser handles BOTH JSON and SSE, so the implementation is resilient either way. Re-run this task before first real use.

---

## Task 2: MCP request builders (`mcp_proto.rs`, additive)

Add the pure protocol module with JSON-RPC request builders. Additive — the crate keeps compiling (Tavily tools still wired).

**Files:**
- Create: `src/mcp_proto.rs`
- Modify: `src/lib.rs` (add one module declaration line)

- [ ] **Step 1: Declare the module in `lib.rs`**

Add this line alongside the existing `pub mod` declarations near the top of `src/lib.rs` (do not touch anything else):

```rust
pub mod mcp_proto;
```

- [ ] **Step 2: Create `src/mcp_proto.rs` with builders + tests**

```rust
//! Pure MCP (Streamable HTTP / JSON-RPC 2.0) protocol helpers for the GitHub
//! remote MCP server. No WIT/bindings/network imports — host-testable.

use serde_json::{json, Value};

/// Conservative agentic-worker metadata for read-only GitHub MCP tools.
pub const READONLY_AW_META: &str = r#"{
  "usage_hint": "Read-only GitHub operation proxied to GitHub's official MCP server (repositories, issues, pull requests). Provide the tool's documented arguments.",
  "side_effects": "read",
  "cost": "medium",
  "confirmation_required": false
}"#;

/// One tool surfaced by the remote MCP `tools/list`, projected to the fields the
/// design extension needs.
#[derive(Debug, PartialEq, Eq)]
pub struct McpToolMeta {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
}

/// Build a JSON-RPC `initialize` request.
#[must_use]
pub fn build_initialize(id: u64, protocol_version: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": { "name": "greentic-github-mcp", "version": "0.1.0" }
        }
    })
}

/// Build the `notifications/initialized` notification (no id).
#[must_use]
pub fn build_initialized() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

/// Build a JSON-RPC `tools/list` request.
#[must_use]
pub fn build_tools_list(id: u64) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list", "params": {} })
}

/// Build a JSON-RPC `tools/call` request.
#[must_use]
pub fn build_tools_call(id: u64, name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initialize_has_protocol_and_client_info() {
        let req = build_initialize(1, "2025-06-18");
        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["id"], 1);
        assert_eq!(req["method"], "initialize");
        assert_eq!(req["params"]["protocolVersion"], "2025-06-18");
        assert_eq!(req["params"]["clientInfo"]["name"], "greentic-github-mcp");
    }

    #[test]
    fn initialized_is_a_notification_without_id() {
        let req = build_initialized();
        assert_eq!(req["method"], "notifications/initialized");
        assert!(req.get("id").is_none());
    }

    #[test]
    fn tools_list_shape() {
        let req = build_tools_list(2);
        assert_eq!(req["id"], 2);
        assert_eq!(req["method"], "tools/list");
    }

    #[test]
    fn tools_call_carries_name_and_arguments() {
        let req = build_tools_call(2, "get_issue", &json!({ "owner": "o", "repo": "r", "issue_number": 1 }));
        assert_eq!(req["method"], "tools/call");
        assert_eq!(req["params"]["name"], "get_issue");
        assert_eq!(req["params"]["arguments"]["repo"], "r");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib mcp_proto`
Expected: PASS (4 tests). The whole crate still compiles (Tavily code untouched).

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/mcp_proto.rs
git commit -m "feat(github-mcp): JSON-RPC request builders (initialize/tools-list/tools-call)"
```

---

## Task 3: MCP response parsing — JSON + SSE (`mcp_proto.rs`)

Add response parsing that unwraps SSE framing and surfaces JSON-RPC errors.

**Files:**
- Modify: `src/mcp_proto.rs` (append functions + tests)

- [ ] **Step 1: Append the parsing functions to `src/mcp_proto.rs`** (before the `#[cfg(test)]` block)

```rust
/// Parse a JSON-RPC response from an HTTP body, unwrapping SSE framing when the
/// response is `text/event-stream`. Returns the JSON-RPC envelope whose `id`
/// matches `expected_id` (falling back to the first envelope carrying a result
/// or error if no id matches).
pub fn parse_jsonrpc_response(
    content_type: &str,
    body: &[u8],
    expected_id: u64,
) -> Result<Value, String> {
    let text = std::str::from_utf8(body)
        .map_err(|error| format!("response body is not UTF-8: {error}"))?;

    if content_type.to_ascii_lowercase().contains("text/event-stream") {
        let mut fallback: Option<Value> = None;
        for line in text.lines() {
            let Some(payload) = line.trim_start().strip_prefix("data:") else {
                continue;
            };
            let payload = payload.trim();
            let Ok(value) = serde_json::from_str::<Value>(payload) else {
                continue;
            };
            let has_body = value.get("result").is_some() || value.get("error").is_some();
            if value.get("id").and_then(Value::as_u64) == Some(expected_id) && has_body {
                return Ok(value);
            }
            if fallback.is_none() && has_body {
                fallback = Some(value);
            }
        }
        fallback.ok_or_else(|| "no JSON-RPC response found in SSE stream".to_string())
    } else {
        serde_json::from_str::<Value>(text)
            .map_err(|error| format!("decode JSON-RPC response: {error}"))
    }
}

/// Return the `result` object, or an `Err` describing a JSON-RPC `error`.
pub fn extract_result(envelope: &Value) -> Result<Value, String> {
    if let Some(error) = envelope.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("JSON-RPC error {code}: {message}"));
    }
    envelope
        .get("result")
        .cloned()
        .ok_or_else(|| "JSON-RPC response has neither result nor error".to_string())
}
```

- [ ] **Step 2: Append tests inside the existing `#[cfg(test)] mod tests` block**

```rust
    #[test]
    fn parses_plain_json_response() {
        let body = br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}"#;
        let value = parse_jsonrpc_response("application/json", body, 2).unwrap();
        assert_eq!(value["id"], 2);
        assert!(value["result"]["tools"].is_array());
    }

    #[test]
    fn unwraps_sse_response_and_matches_id() {
        let body = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":true}}\n\n";
        let value = parse_jsonrpc_response("text/event-stream; charset=utf-8", body, 2).unwrap();
        assert_eq!(value["result"]["ok"], true);
    }

    #[test]
    fn sse_picks_matching_id_among_multiple_events() {
        let body = b"data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"first\":true}}\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"second\":true}}\n\n";
        let value = parse_jsonrpc_response("text/event-stream", body, 2).unwrap();
        assert_eq!(value["result"]["second"], true);
    }

    #[test]
    fn extract_result_returns_result() {
        let env = json!({ "jsonrpc": "2.0", "id": 2, "result": { "x": 1 } });
        assert_eq!(extract_result(&env).unwrap()["x"], 1);
    }

    #[test]
    fn extract_result_surfaces_error() {
        let env = json!({ "jsonrpc": "2.0", "id": 2, "error": { "code": -32602, "message": "bad params" } });
        let err = extract_result(&env).unwrap_err();
        assert!(err.contains("-32602"));
        assert!(err.contains("bad params"));
    }

    #[test]
    fn malformed_json_body_is_err() {
        assert!(parse_jsonrpc_response("application/json", b"{not json", 2).is_err());
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib mcp_proto`
Expected: PASS (10 tests total).

- [ ] **Step 4: Commit**

```bash
git add src/mcp_proto.rs
git commit -m "feat(github-mcp): JSON-RPC response parsing with SSE unwrap + error extraction"
```

---

## Task 4: MCP tool mapping (`mcp_proto.rs`)

Add `tools/list` → metadata mapping and `tools/call` output extraction.

**Files:**
- Modify: `src/mcp_proto.rs` (append functions + tests)

- [ ] **Step 1: Append the mapping functions** (before the `#[cfg(test)]` block)

```rust
/// Map a `tools/list` result into the tool metadata the extension surfaces.
#[must_use]
pub fn map_tools_list(result: &Value) -> Vec<McpToolMeta> {
    let Some(tools) = result.get("tools").and_then(Value::as_array) else {
        return Vec::new();
    };
    tools
        .iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?.to_string();
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let input_schema_json = tool
                .get("inputSchema")
                .map_or_else(|| "{}".to_string(), std::string::ToString::to_string);
            Some(McpToolMeta { name, description, input_schema_json })
        })
        .collect()
}

/// Extract a `tools/call` result as a JSON string for the agent. Honors the MCP
/// `isError` flag (mapped to `Err`). Prefers `structuredContent`; otherwise
/// concatenates text `content` blocks.
pub fn extract_tool_output(result: &Value) -> Result<String, String> {
    let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);

    if let Some(structured) = result.get("structuredContent") {
        let text = structured.to_string();
        return if is_error { Err(text) } else { Ok(text) };
    }

    let mut parts: Vec<String> = Vec::new();
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        for block in content {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                parts.push(text.to_string());
            }
        }
    }
    let joined = parts.join("\n");
    if is_error {
        Err(if joined.is_empty() {
            "tool call returned isError".to_string()
        } else {
            joined
        })
    } else {
        Ok(joined)
    }
}
```

- [ ] **Step 2: Append tests inside the `#[cfg(test)] mod tests` block**

```rust
    #[test]
    fn maps_tools_list_with_input_schema() {
        let result = json!({
            "tools": [
                { "name": "get_issue", "description": "Get an issue",
                  "inputSchema": { "type": "object", "properties": { "repo": { "type": "string" } } } },
                { "name": "search_code", "description": "Search code", "inputSchema": { "type": "object" } }
            ]
        });
        let metas = map_tools_list(&result);
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].name, "get_issue");
        assert!(metas[0].input_schema_json.contains("properties"));
        assert!(serde_json::from_str::<Value>(&metas[0].input_schema_json).is_ok());
    }

    #[test]
    fn map_tools_list_missing_tools_returns_empty() {
        assert!(map_tools_list(&json!({})).is_empty());
    }

    #[test]
    fn extract_tool_output_joins_text_content() {
        let result = json!({ "content": [ { "type": "text", "text": "line1" }, { "type": "text", "text": "line2" } ] });
        assert_eq!(extract_tool_output(&result).unwrap(), "line1\nline2");
    }

    #[test]
    fn extract_tool_output_prefers_structured_content() {
        let result = json!({ "structuredContent": { "number": 7 }, "content": [ { "type": "text", "text": "ignored" } ] });
        let out = extract_tool_output(&result).unwrap();
        assert!(out.contains("\"number\":7"));
    }

    #[test]
    fn extract_tool_output_is_error_maps_to_err() {
        let result = json!({ "isError": true, "content": [ { "type": "text", "text": "not found" } ] });
        assert_eq!(extract_tool_output(&result).unwrap_err(), "not found");
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib mcp_proto`
Expected: PASS (15 tests total).

- [ ] **Step 4: Commit**

```bash
git add src/mcp_proto.rs
git commit -m "feat(github-mcp): tools/list mapping + tools/call output extraction"
```

---

## Task 5: Wire the WIT glue (`lib.rs`) + remove inherited Tavily modules

Replace `lib.rs` with the GitHub-MCP handshake/proxy glue and delete the inherited Tavily modules.

**Files:**
- Modify: `src/lib.rs`
- Delete: `src/tool_meta.rs`, `src/input.rs`, `src/output.rs`

- [ ] **Step 1: Confirm the logging Level variant**

The glue logs failures via the host `logging` import. Confirm the enum variant name:
```bash
grep -n "pub enum Level" -A6 src/bindings.rs
```
Use the `Warn` variant in Step 2; if the generated enum spells it differently (e.g. `Warning`), use that spelling instead.

- [ ] **Step 2: Replace the entire `src/lib.rs`** with:

```rust
//! Greentic GitHub-MCP design extension — WIT export layer.
//!
//! Proxies the agentic worker to GitHub's remote MCP server
//! (`https://api.githubcopilot.com/mcp/`) over the host `http` capability,
//! speaking MCP Streamable HTTP / JSON-RPC 2.0. `list_tools` enumerates GitHub's
//! tools via `tools/list`; `invoke_tool` proxies `tools/call`. The GitHub PAT is
//! read from the host secret and never returned. Protocol shaping lives in the
//! pure [`mcp_proto`] module.
#![allow(clippy::used_underscore_items)]

#[allow(warnings)]
pub mod bindings;

pub mod mcp_proto;

pub use bindings::exports;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_design::tools;
use bindings::greentic::extension_base::types;
use bindings::greentic::extension_host::{http, logging, secrets};
use serde_json::Value;

pub struct Component;

const ENDPOINT: &str = "https://api.githubcopilot.com/mcp/";
const PAT_SECRET_REF: &str = "secret://github/token";
const TOOLSETS: &str = "repos,issues,pull_requests";
const READONLY: &str = "true";
const PROTOCOL_VERSION: &str = "2025-06-18";

// ===== base::manifest =====
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "greentic.github-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "greentic:github-mcp/tools".into(),
            version: "1.0.0".into(),
        }]
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

/// An initialized MCP session: the resolved PAT and any server-assigned session id.
struct Session {
    api_key: String,
    session_id: Option<String>,
}

/// Find a response header value (case-insensitive).
fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Common MCP request headers. `session_id` is echoed when present.
fn mcp_headers(api_key: &str, session_id: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("authorization".into(), format!("Bearer {api_key}")),
        ("content-type".into(), "application/json".into()),
        ("accept".into(), "application/json, text/event-stream".into()),
        ("mcp-protocol-version".into(), PROTOCOL_VERSION.into()),
        ("x-mcp-readonly".into(), READONLY.into()),
        ("x-mcp-toolsets".into(), TOOLSETS.into()),
    ];
    if let Some(id) = session_id {
        headers.push(("mcp-session-id".into(), id.into()));
    }
    headers
}

/// POST a JSON-RPC request; return (parsed envelope, response headers).
fn post_rpc(
    api_key: &str,
    session_id: Option<&str>,
    request: &Value,
    expected_id: u64,
) -> Result<(Value, Vec<(String, String)>), types::ExtensionError> {
    let body = serde_json::to_vec(request)
        .map_err(|error| types::ExtensionError::Internal(format!("encode request: {error}")))?;

    let response = http::fetch(&http::Request {
        method: "POST".into(),
        url: ENDPOINT.into(),
        headers: mcp_headers(api_key, session_id),
        body: Some(body),
    })
    .map_err(|error| types::ExtensionError::Internal(format!("http fetch: {error}")))?;

    if !(200..300).contains(&response.status) {
        let detail = String::from_utf8_lossy(&response.body);
        return Err(if response.status == 401 {
            types::ExtensionError::PermissionDenied(format!("GitHub rejected the PAT (401): {detail}"))
        } else {
            types::ExtensionError::Internal(format!("MCP request failed ({}): {detail}", response.status))
        });
    }

    let content_type = header(&response.headers, "content-type").unwrap_or("");
    let envelope = mcp_proto::parse_jsonrpc_response(content_type, &response.body, expected_id)
        .map_err(types::ExtensionError::Internal)?;
    Ok((envelope, response.headers))
}

/// Send the `notifications/initialized` notification (status-only, no JSON-RPC body).
fn send_initialized(session: &Session) -> Result<(), types::ExtensionError> {
    let body = serde_json::to_vec(&mcp_proto::build_initialized())
        .map_err(|error| types::ExtensionError::Internal(format!("encode notification: {error}")))?;
    let response = http::fetch(&http::Request {
        method: "POST".into(),
        url: ENDPOINT.into(),
        headers: mcp_headers(&session.api_key, session.session_id.as_deref()),
        body: Some(body),
    })
    .map_err(|error| types::ExtensionError::Internal(format!("http fetch: {error}")))?;
    if !(200..300).contains(&response.status) {
        return Err(types::ExtensionError::Internal(format!(
            "MCP initialized notification failed ({})",
            response.status
        )));
    }
    Ok(())
}

/// Resolve the PAT and perform the MCP handshake (initialize + initialized).
fn handshake() -> Result<Session, types::ExtensionError> {
    let api_key = secrets::get(PAT_SECRET_REF).map_err(|error| {
        types::ExtensionError::PermissionDenied(format!("resolve secret {PAT_SECRET_REF}: {error}"))
    })?;

    let (envelope, headers) =
        post_rpc(&api_key, None, &mcp_proto::build_initialize(1, PROTOCOL_VERSION), 1)?;
    mcp_proto::extract_result(&envelope).map_err(types::ExtensionError::Internal)?;

    let session_id = header(&headers, "mcp-session-id").map(str::to_string);
    let session = Session { api_key, session_id };
    send_initialized(&session)?;
    Ok(session)
}

// ===== design::tools =====
impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        let session = match handshake() {
            Ok(session) => session,
            Err(error) => {
                logging::log(
                    logging::Level::Warn,
                    "greentic.github-mcp",
                    &format!("list_tools handshake failed: {error:?}"),
                );
                return Vec::new();
            }
        };

        let envelope = match post_rpc(
            &session.api_key,
            session.session_id.as_deref(),
            &mcp_proto::build_tools_list(2),
            2,
        ) {
            Ok((envelope, _)) => envelope,
            Err(error) => {
                logging::log(
                    logging::Level::Warn,
                    "greentic.github-mcp",
                    &format!("tools/list failed: {error:?}"),
                );
                return Vec::new();
            }
        };

        let result = match mcp_proto::extract_result(&envelope) {
            Ok(result) => result,
            Err(error) => {
                logging::log(
                    logging::Level::Warn,
                    "greentic.github-mcp",
                    &format!("tools/list error: {error}"),
                );
                return Vec::new();
            }
        };

        mcp_proto::map_tools_list(&result)
            .into_iter()
            .map(|meta| tools::ToolDefinition {
                name: meta.name,
                description: meta.description,
                input_schema_json: meta.input_schema_json,
                output_schema_json: None,
                capabilities: Some(vec!["agentic_worker".into()]),
                agentic_worker_metadata: Some(mcp_proto::READONLY_AW_META.into()),
            })
            .collect()
    }

    fn invoke_tool(name: String, args_json: String) -> Result<String, types::ExtensionError> {
        let arguments: Value = if args_json.trim().is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_str(&args_json).map_err(|error| {
                types::ExtensionError::InvalidInput(format!("decode arguments: {error}"))
            })?
        };

        let session = handshake()?;
        let (envelope, _) = post_rpc(
            &session.api_key,
            session.session_id.as_deref(),
            &mcp_proto::build_tools_call(2, &name, &arguments),
            2,
        )?;
        let result = mcp_proto::extract_result(&envelope).map_err(types::ExtensionError::Internal)?;
        mcp_proto::extract_tool_output(&result).map_err(types::ExtensionError::Internal)
    }
}

#[cfg(target_family = "wasm")]
bindings::export!(Component with_types_in bindings);
```

- [ ] **Step 3: Delete the inherited Tavily modules**

```bash
git rm src/tool_meta.rs src/input.rs src/output.rs
```

- [ ] **Step 4: Verify host tests pass**

Run: `cargo test`
Expected: PASS — the 15 `mcp_proto` tests (lib.rs has no host tests; it imports WIT bindings).

- [ ] **Step 5: Build the WASM component**

Run: `cargo fmt --all && cargo component build --release --target wasm32-wasip2`
Expected: `Finished`, produces `target/wasm32-wasip2/release/greentic_github_mcp_extension.wasm`.

- [ ] **Step 6: Lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: zero warnings. Fix any clippy issues in the glue (mirror patterns the reference crates already allow).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(github-mcp): wire MCP handshake + list_tools/invoke_tool proxy over host http/secrets"
```

---

## Task 6: Packaging (`build.sh`) → `.gtxpack`

**Files:**
- Modify: `build.sh` (wasm artifact filename)

- [ ] **Step 1: Fix the wasm artifact path in `build.sh`**

Change the `WASM_PATH` line to the new crate's artifact:
```bash
WASM_PATH="target/wasm32-wasip2/release/greentic_github_mcp_extension.wasm"
```
Also rename the internal temp-zip prefix if it still says `greentic_tavily_`:
```bash
TMP_ZIP="$STAGE/../greentic_github_mcp_$$.zip"
```
Leave the rest of the script unchanged.

- [ ] **Step 2: Build the package**

```bash
bash build.sh
```
Expected: ends with `==> built …/greentic.github-mcp-0.1.0.gtxpack`.

- [ ] **Step 3: Verify package + digest**

```bash
unzip -l greentic.github-mcp-0.1.0.gtxpack
unzip -p greentic.github-mcp-0.1.0.gtxpack describe.json | jq '.runtime.components["github-mcp-tool"].sha256'
```
Expected: archive lists `describe.json` + `extension.wasm`; the printed sha256 is a real 64-hex digest (not zeros). Confirm the on-disk source `describe.json` still has the zero placeholder: `jq '.runtime.components["github-mcp-tool"].sha256' describe.json` → all zeros.

- [ ] **Step 4: Commit**

```bash
git add build.sh
git commit -m "build(github-mcp): package extension into .gtxpack"
```

---

## Task 7: README + final verification gate

**Files:**
- Replace: `README.md`

- [ ] **Step 1: Write `README.md`**

````markdown
# component-github-mcp-ext

A Greentic Designer **design extension** (WASM, `wasm32-wasip2`) that proxies the
agentic worker to GitHub's official **remote MCP server**
(`https://api.githubcopilot.com/mcp/`) over the host `http` capability.

- `list_tools` enumerates GitHub's tools via MCP `tools/list`.
- `invoke_tool` proxies MCP `tools/call`.

Runs **read-only** by default (`X-MCP-Readonly: true`) over the
`repos,issues,pull_requests` toolsets. The GitHub PAT is read from the host
secret `secret://github/token` and never returned. Outbound calls are restricted
to `api.githubcopilot.com`. All surfaced tools declare the `agentic_worker`
capability so the DW Composer presents them to the agentic worker.

If no PAT is configured (or the MCP handshake fails), `list_tools` returns an
empty list rather than erroring.

## Build

```sh
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo component build --release --target wasm32-wasip2
bash build.sh
```

## Secret provisioning

Store a GitHub Personal Access Token in the configured secrets backend under a
URI matching the `secret://github/` permission prefix — the extension reads
`secret://github/token`.

## Notes

- Protocol: MCP Streamable HTTP / JSON-RPC 2.0. Responses may be `application/json`
  or `text/event-stream` (SSE); both are handled.
- Each `list_tools` / `invoke_tool` performs a fresh `initialize` handshake
  (WASM instances are stateless across calls).
````

- [ ] **Step 2: Run the full gate**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo component build --release --target wasm32-wasip2
bash build.sh
```
Expected: every command exits 0; `build.sh` prints the `.gtxpack` path. (Run `cargo fmt --all` first if the check fails on a regenerated `bindings.rs`.)

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(github-mcp): add README"
```

---

## Out of scope / follow-ups

- **Write tools + confirmation** — v1 is read-only; enabling writes means
  `X-MCP-Readonly: false` and `confirmation_required: true` metadata.
- **Generic multi-MCP bridge** — config-driven server URL/auth so one extension
  proxies any remote MCP server.
- **`list_tools` caching** — the aw-runtime calls `list_tools` per step and fresh
  WASM cannot cache; a runtime-side cache is the fix (runtime track).
- **Live worker execution** — gated on the aw-runtime `ConfigProvider` (Phase 4).
- **CI/CD** — add `ci/local_check.sh` + `.github/workflows/{ci,release}.yml`
  mirroring `component-tavily-ext` (see that repo's PR #1).
- **Signing** — `gtdx publish` signing flow is a separate concern.
