# greentic.sql Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `component-sql-ext`, a WASM design extension that gives the agentic worker a text-to-data tool: it owns NL→SQL (via a configurable OpenAI-compatible LLM), executes the SQL read-only through an operator-run SQL→HTTP gateway, and returns rows plus the generated SQL.

**Architecture:** Mirror the proven `component-tavily-ext` crate. Pure host-testable modules — `connections.rs` (registry), `guard.rs` (SELECT-only), `protocol.rs` (schema/LLM/query request+response shaping), `tool_meta.rs` (tool defs) — plus a thin `lib.rs` glue that reads config from host `secrets`, calls host `http` for `/schema` → LLM `/chat/completions` → `/query`, and maps failures to `ExtensionError`. The pure modules are added additively so the crate compiles at every task until the final `lib.rs` swap.

**Tech Stack:** Rust edition 2024, `cargo-component` → `wasm32-wasip2`, `wit-bindgen` 0.41, `serde_json`, OpenAI-compatible chat completions, packaged into `.gtxpack` by `build.sh`.

**Spec:** [`../specs/2026-05-30-greentic-sql-extension-design.md`](../specs/2026-05-30-greentic-sql-extension-design.md)

**Reference crate (copy this):** `/Users/bimapangestu/Desktop/Works/personal/greentic/component-tavily-ext`

---

## Conventions for every task

- Work in the new repo `/Users/bimapangestu/Desktop/Works/personal/greentic/component-sql-ext` on branch `feat/sql-extension`.
- Host tests (pure modules) run with `cargo test`. They MUST pass before each commit.
- WASM build (`cargo component build --release --target wasm32-wasip2`) is the integration check; run it in Tasks 0, 4, 6.
- Fresh repo has **no pre-commit hooks**; the implementer's `cargo test`/`cargo clippy` is the gate. Run `cargo fmt --all` before any `cargo component build` in a gate (the build regenerates `src/bindings.rs` unformatted).
- Conventional commits; no AI attribution.
- Gateway + LLM contracts used throughout:
  - `GET {base_url}/schema` (Bearer token) → `{ "engine":"postgres", "tables":[{"name","columns":[{"name","type"}]}] }`
  - `POST {base_url}/query` (Bearer token) `{ "sql", "max_rows" }` → `{ "columns":[…], "rows":[[…]], "row_count":N, "truncated":bool }`
  - `POST {llm.base_url}/chat/completions` (Bearer api_key) `{ model, temperature, messages:[…] }` → `{ "choices":[{"message":{"content":"<SQL>"}}] }`

---

## Task 0: Bootstrap the crate from component-tavily-ext

**Files:**
- Create dir: `component-sql-ext/` (copy of `component-tavily-ext/`)
- Modify: `Cargo.toml`, `wit/world.wit`, `describe.json`, `src/lib.rs` (identity only)

- [ ] **Step 1: Copy and strip** (also drop the inherited `.github`/`ci`)

```bash
cd /Users/bimapangestu/Desktop/Works/personal/greentic
cp -r component-tavily-ext component-sql-ext
cd component-sql-ext
rm -rf .git target Cargo.lock *.gtxpack .github ci
git init -q
git checkout -b feat/sql-extension
```

- [ ] **Step 2: `Cargo.toml`** — replace `name`, `description`, `[package.metadata.component] package`. Keep `version = "1.2.0-research"` (inherited):

```toml
[package]
name = "greentic-sql-extension"
version = "1.2.0-research"
edition = "2024"
rust-version = "1.95.0"
license = "MIT"
publish = false
description = "Text-to-data (NL->SQL) WASM design-extension for the Greentic agentic worker"

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
package = "greentic:sql-extension"

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

- [ ] **Step 3: `wit/world.wit`** — change only the package line (imports already include logging/http/secrets):

```wit
package greentic:sql-extension;

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

- [ ] **Step 4: `src/lib.rs` identity only** — change the `manifest::Guest` identity + offered capability; leave the inherited Tavily `tools::Guest` impl, `pub mod tool_meta/input/output`, `lifecycle`, and the gated `bindings::export!` untouched:

```rust
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "greentic.sql".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "greentic:sql/query".into(),
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
    "id": "greentic.sql",
    "name": "SQL (Text-to-Data)",
    "version": "1.2.0-research",
    "summary": "Ask databases in natural language; the extension writes read-only SQL and returns the data",
    "description": "Design extension giving the agentic worker a text-to-data tool. The agent asks a natural-language question against a named connection; the extension introspects the schema, uses a configurable OpenAI-compatible LLM to write a read-only SQL query, guards it (SELECT-only), executes it via an operator-run SQL->HTTP gateway, and returns rows plus the generated SQL. Multiple databases/engines are supported via the gateway contract. All access is read-only.",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "MIT",
    "repository": "https://github.com/greentic-biz/component-sql-ext",
    "keywords": ["sql", "text-to-sql", "text-to-data", "agentic-worker", "design-extension"]
  },
  "engine": {
    "greenticDesigner": ">=1.2.0",
    "extRuntime": "^1.2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:sql/query", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 64,
    "permissions": {
      "network": [
        "https://api.openai.com/*"
      ],
      "secrets": [
        "secret://sql/"
      ],
      "callExtensionKinds": []
    },
    "components": {
      "sql-tool": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
          "pack_id": "greentic.sql",
          "component_version": "1.2.0-research"
        },
        "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        "world": "greentic:sql-extension/design-extension@1.0.0"
      }
    }
  },
  "contributions": {
    "tools": [
      { "name": "sql_list_connections", "export": "greentic:extension-design/tools.invoke-tool", "runtime_ref": "sql-tool" },
      { "name": "sql_ask",              "export": "greentic:extension-design/tools.invoke-tool", "runtime_ref": "sql-tool" }
    ]
  }
}
```

> `network` lists only the default LLM host. The LLM `base_url` (if non-OpenAI) and every connection's gateway `base_url` are operator-configured at runtime and MUST be granted via runtime host-overrides — `describe.json` cannot enumerate them (see spec "Network allow-list").

- [ ] **Step 6: Regenerate bindings + build**

```bash
cargo component build --release --target wasm32-wasip2
```
Expected `Finished`. If it errors on a world/package mismatch (copied bindings still name `greentic:tavily-extension`):
```bash
rm -f src/bindings.rs && cargo component bindings && cargo component build --release --target wasm32-wasip2
```

- [ ] **Step 7: Host tests still pass**

Run: `cargo test`
Expected: inherited Tavily tests PASS (removed in Task 4).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore(sql): bootstrap design-extension crate from component-tavily-ext"
```

---

## Task 1: Connection registry (`connections.rs`, additive)

**Files:**
- Create: `src/connections.rs`
- Modify: `src/lib.rs` (add one module declaration line)

- [ ] **Step 1: Declare the module in `lib.rs`**

Add alongside the existing `pub mod` lines near the top of `src/lib.rs` (touch nothing else):
```rust
pub mod connections;
```

- [ ] **Step 2: Create `src/connections.rs`**

```rust
//! Pure parsing/lookup of the `secret://sql/connections` registry. No WIT
//! imports — host-testable.

use serde::Deserialize;

/// One configured database connection (points at a SQL->HTTP gateway base URL).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Connection {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub description: String,
}

/// Parse the registry JSON (a JSON array of connections).
pub fn parse_registry(json: &str) -> Result<Vec<Connection>, String> {
    serde_json::from_str::<Vec<Connection>>(json)
        .map_err(|error| format!("decode connections registry: {error}"))
}

/// Find a connection by exact name.
#[must_use]
pub fn find<'a>(connections: &'a [Connection], name: &str) -> Option<&'a Connection> {
    connections.iter().find(|connection| connection.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_with_defaults() {
        let json = r#"[
            {"name":"analytics","base_url":"https://gw/a","engine":"postgres","description":"warehouse"},
            {"name":"app","base_url":"https://gw/b"}
        ]"#;
        let conns = parse_registry(json).unwrap();
        assert_eq!(conns.len(), 2);
        assert_eq!(conns[0].engine, "postgres");
        assert_eq!(conns[1].engine, "");
        assert_eq!(conns[1].description, "");
    }

    #[test]
    fn find_hits_and_misses() {
        let conns = parse_registry(r#"[{"name":"x","base_url":"u"}]"#).unwrap();
        assert!(find(&conns, "x").is_some());
        assert!(find(&conns, "y").is_none());
    }

    #[test]
    fn malformed_registry_is_err() {
        assert!(parse_registry("{not an array").is_err());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib connections`
Expected: PASS (3 tests). Crate still compiles (Tavily code intact).

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/connections.rs
git commit -m "feat(sql): connection registry parse + lookup"
```

---

## Task 2: Read-only SQL guard (`guard.rs`, additive)

**Files:**
- Create: `src/guard.rs`
- Modify: `src/lib.rs` (add one module declaration line)

- [ ] **Step 1: Declare the module in `lib.rs`**

```rust
pub mod guard;
```

- [ ] **Step 2: Create `src/guard.rs`**

```rust
//! Pure best-effort read-only SQL guard. The gateway's read-only DB role is the
//! real safety boundary; this is the first line of defense over LLM-generated
//! SQL. No WIT imports — host-testable.

const FORBIDDEN: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "TRUNCATE", "GRANT",
    "REVOKE", "MERGE", "REPLACE", "CALL", "EXEC", "ATTACH", "PRAGMA",
];

/// Tokenize into uppercase identifier words (runs of `[A-Za-z0-9_]`).
fn words(sql: &str) -> Vec<String> {
    sql.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch.to_ascii_uppercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Ensure `sql` is a single read-only `SELECT`/`WITH` statement. Returns
/// `Err(reason)` for anything that is empty, multi-statement, commented, or
/// contains a forbidden DML/DDL keyword.
pub fn ensure_read_only(sql: &str) -> Result<(), String> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err("empty SQL".to_string());
    }
    if trimmed.contains("--") || trimmed.contains("/*") {
        return Err("SQL comments are not allowed".to_string());
    }
    let body = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();
    if body.contains(';') {
        return Err("multiple statements are not allowed".to_string());
    }
    let tokens = words(body);
    match tokens.first().map(String::as_str) {
        Some("SELECT" | "WITH") => {}
        Some(other) => return Err(format!("only SELECT/WITH queries are allowed, got: {other}")),
        None => return Err("no SQL keyword found".to_string()),
    }
    for keyword in FORBIDDEN {
        if tokens.iter().any(|token| token == keyword) {
            return Err(format!("forbidden keyword in SQL: {keyword}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_plain_select() {
        assert!(ensure_read_only("SELECT id, name FROM users WHERE id = 1").is_ok());
    }

    #[test]
    fn allows_lowercase_and_with_cte() {
        assert!(ensure_read_only("select * from t").is_ok());
        assert!(ensure_read_only("WITH x AS (SELECT 1) SELECT * FROM x").is_ok());
    }

    #[test]
    fn strips_single_trailing_semicolon() {
        assert!(ensure_read_only("SELECT 1;").is_ok());
    }

    #[test]
    fn rejects_dml_dll() {
        assert!(ensure_read_only("DELETE FROM users").is_err());
        assert!(ensure_read_only("INSERT INTO t VALUES (1)").is_err());
        assert!(ensure_read_only("DROP TABLE t").is_err());
        assert!(ensure_read_only("UPDATE t SET a=1").is_err());
    }

    #[test]
    fn rejects_select_then_mutation() {
        assert!(ensure_read_only("SELECT 1; DROP TABLE t").is_err());
    }

    #[test]
    fn rejects_comments_and_empty() {
        assert!(ensure_read_only("SELECT 1 -- comment").is_err());
        assert!(ensure_read_only("SELECT 1 /* x */").is_err());
        assert!(ensure_read_only("   ").is_err());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib guard`
Expected: PASS (6 tests).

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/guard.rs
git commit -m "feat(sql): read-only SELECT-only SQL guard"
```

---

## Task 3: Schema / LLM / query protocol (`protocol.rs`, additive)

**Files:**
- Create: `src/protocol.rs`
- Modify: `src/lib.rs` (add one module declaration line)

- [ ] **Step 1: Declare the module in `lib.rs`**

```rust
pub mod protocol;
```

- [ ] **Step 2: Create `src/protocol.rs`**

```rust
//! Pure request/response shaping for the gateway `/schema` + `/query` endpoints
//! and the OpenAI-compatible LLM. No WIT imports — host-testable.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    #[serde(default, rename = "type")]
    pub type_: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    #[serde(default)]
    pub columns: Vec<Column>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Schema {
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub tables: Vec<Table>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct QueryResult {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Value>,
    #[serde(default)]
    pub row_count: u64,
    #[serde(default)]
    pub truncated: bool,
}

/// Parse a gateway `/schema` response.
pub fn parse_schema(json: &str) -> Result<Schema, String> {
    serde_json::from_str::<Schema>(json).map_err(|error| format!("decode schema: {error}"))
}

/// Render a schema as compact prompt text: one `table(col type, ...)` per line.
#[must_use]
pub fn format_schema_prompt(schema: &Schema) -> String {
    let mut out = String::new();
    for table in &schema.tables {
        let cols: Vec<String> = table
            .columns
            .iter()
            .map(|column| format!("{} {}", column.name, column.type_))
            .collect();
        out.push_str(&format!("{}({})\n", table.name, cols.join(", ")));
    }
    out
}

/// Build the OpenAI-compatible chat-completions request body.
#[must_use]
pub fn build_llm_request(model: &str, engine: &str, schema_text: &str, question: &str) -> Value {
    let system = "You are a careful data analyst. Given a database schema and SQL dialect, \
                  write exactly ONE read-only SQL query (SELECT only) that answers the user's \
                  question. Return ONLY the SQL — no explanation and no markdown code fences.";
    let user = format!(
        "Dialect: {engine}\nSchema (one table per line as table(column type, ...)):\n{schema_text}\nQuestion: {question}"
    );
    json!({
        "model": model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    })
}

/// Extract the SQL string from a chat-completions response, stripping markdown
/// code fences if the model added them.
pub fn extract_sql(response_json: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(response_json).map_err(|error| format!("decode LLM response: {error}"))?;
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "LLM response missing choices[0].message.content".to_string())?;
    let sql = strip_fences(content);
    if sql.is_empty() {
        return Err("LLM returned empty SQL".to_string());
    }
    Ok(sql)
}

fn strip_fences(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with("```") {
        let after_first = trimmed.splitn(2, '\n').nth(1).unwrap_or("");
        let body = after_first.trim_end().strip_suffix("```").unwrap_or(after_first);
        body.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Build the gateway `/query` request body.
#[must_use]
pub fn build_query_request(sql: &str, max_rows: u32) -> Value {
    json!({ "sql": sql, "max_rows": max_rows })
}

/// Parse a gateway `/query` response.
pub fn parse_query_response(json: &str) -> Result<QueryResult, String> {
    serde_json::from_str::<QueryResult>(json).map_err(|error| format!("decode query response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_schema_and_formats_prompt() {
        let schema = parse_schema(
            r#"{"engine":"postgres","tables":[{"name":"users","columns":[{"name":"id","type":"integer"},{"name":"email","type":"text"}]}]}"#,
        )
        .unwrap();
        assert_eq!(schema.engine, "postgres");
        let prompt = format_schema_prompt(&schema);
        assert!(prompt.contains("users(id integer, email text)"));
    }

    #[test]
    fn builds_llm_request_with_question_and_engine() {
        let req = build_llm_request("gpt-4o-mini", "mysql", "users(id int)", "how many users?");
        assert_eq!(req["model"], "gpt-4o-mini");
        assert_eq!(req["temperature"], 0);
        assert_eq!(req["messages"][0]["role"], "system");
        let user = req["messages"][1]["content"].as_str().unwrap();
        assert!(user.contains("mysql"));
        assert!(user.contains("how many users?"));
    }

    #[test]
    fn extracts_sql_plain_and_fenced() {
        let plain = r#"{"choices":[{"message":{"content":"SELECT 1"}}]}"#;
        assert_eq!(extract_sql(plain).unwrap(), "SELECT 1");
        let fenced = "{\"choices\":[{\"message\":{\"content\":\"```sql\\nSELECT 2\\n```\"}}]}";
        assert_eq!(extract_sql(fenced).unwrap(), "SELECT 2");
    }

    #[test]
    fn extract_sql_missing_content_is_err() {
        assert!(extract_sql(r#"{"choices":[]}"#).is_err());
    }

    #[test]
    fn builds_query_request_and_parses_response() {
        let req = build_query_request("SELECT 1", 100);
        assert_eq!(req["sql"], "SELECT 1");
        assert_eq!(req["max_rows"], 100);
        let resp = parse_query_response(
            r#"{"columns":["id"],"rows":[[1],[2]],"row_count":2,"truncated":false}"#,
        )
        .unwrap();
        assert_eq!(resp.columns, vec!["id"]);
        assert_eq!(resp.row_count, 2);
        assert!(!resp.truncated);
    }

    #[test]
    fn parse_query_response_defaults_missing_fields() {
        let resp = parse_query_response(r#"{"columns":["a"]}"#).unwrap();
        assert!(resp.rows.is_empty());
        assert_eq!(resp.row_count, 0);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib protocol`
Expected: PASS (6 tests).

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/protocol.rs
git commit -m "feat(sql): schema/LLM/query protocol shaping"
```

---

## Task 4: Tool metadata + WIT glue (`tool_meta.rs` + `lib.rs`)

Replace the inherited Tavily `tool_meta.rs` with the two SQL tools, wire `lib.rs` to orchestrate the pipeline, and delete the inherited Tavily modules. This is the integration task — it ends green.

**Files:**
- Replace: `src/tool_meta.rs`
- Modify: `src/lib.rs`
- Delete: `src/input.rs`, `src/output.rs`

- [ ] **Step 1: Replace `src/tool_meta.rs`**

```rust
//! Static metadata for the two SQL tools. Pure (no WIT imports) so the
//! `agentic_worker` opt-in is asserted by a host test.

pub const SQL_LIST_CONNECTIONS_TOOL: &str = "sql_list_connections";
pub const SQL_ASK_TOOL: &str = "sql_ask";

const LIST_INPUT_SCHEMA: &str = r#"{ "type": "object", "properties": {} }"#;
const LIST_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["connections"],
  "properties": {
    "connections": {
      "type": "array",
      "items": { "type": "object", "properties": {
        "name": { "type": "string" }, "engine": { "type": "string" }, "description": { "type": "string" } } }
    }
  }
}"#;
const LIST_AW_META: &str = r#"{
  "usage_hint": "List the database connections this worker can query. Call this first to discover available databases (name, engine, description), then use sql_ask.",
  "side_effects": "read",
  "cost": "low",
  "confirmation_required": false
}"#;

const ASK_INPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["connection", "question"],
  "properties": {
    "connection": { "type": "string", "description": "A connection name from sql_list_connections" },
    "question": { "type": "string", "description": "The question to answer, in natural language" },
    "max_results": { "type": "integer", "minimum": 1, "maximum": 1000, "description": "Max rows to return (default 100)" }
  }
}"#;
const ASK_OUTPUT_SCHEMA: &str = r#"{
  "type": "object",
  "required": ["connection", "sql", "columns", "rows", "row_count"],
  "properties": {
    "connection": { "type": "string" },
    "sql": { "type": "string" },
    "columns": { "type": "array", "items": { "type": "string" } },
    "rows": { "type": "array" },
    "row_count": { "type": "integer" },
    "truncated": { "type": "boolean" }
  }
}"#;
const ASK_AW_META: &str = r#"{
  "usage_hint": "Answer a natural-language question against a named database connection. The tool writes read-only SQL for you, runs it, and returns the rows plus the SQL it used. Read-only; never mutates data.",
  "examples": [
    { "when": "the worker needs data from a connection", "input": { "connection": "analytics", "question": "how many active users signed up last week?" } }
  ],
  "side_effects": "read",
  "cost": "medium",
  "confirmation_required": false
}"#;

pub struct ToolMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema_json: &'static str,
    pub output_schema_json: &'static str,
    pub capabilities: Vec<String>,
    pub agentic_worker_metadata: &'static str,
}

#[must_use]
pub fn sql_list_connections_tool() -> ToolMeta {
    ToolMeta {
        name: SQL_LIST_CONNECTIONS_TOOL,
        description: "List the database connections available to this worker (name, engine, description).",
        input_schema_json: LIST_INPUT_SCHEMA,
        output_schema_json: LIST_OUTPUT_SCHEMA,
        capabilities: vec!["agentic_worker".into()],
        agentic_worker_metadata: LIST_AW_META,
    }
}

#[must_use]
pub fn sql_ask_tool() -> ToolMeta {
    ToolMeta {
        name: SQL_ASK_TOOL,
        description: "Ask a database a natural-language question; returns rows plus the read-only SQL the tool generated.",
        input_schema_json: ASK_INPUT_SCHEMA,
        output_schema_json: ASK_OUTPUT_SCHEMA,
        capabilities: vec!["agentic_worker".into()],
        agentic_worker_metadata: ASK_AW_META,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_tools_declare_agentic_worker() {
        for tool in [sql_list_connections_tool(), sql_ask_tool()] {
            assert!(tool.capabilities.iter().any(|cap| cap == "agentic_worker"));
        }
    }

    #[test]
    fn tool_names_match_constants() {
        assert_eq!(sql_list_connections_tool().name, "sql_list_connections");
        assert_eq!(sql_ask_tool().name, "sql_ask");
    }

    #[test]
    fn schemas_and_metadata_are_valid_json() {
        for tool in [sql_list_connections_tool(), sql_ask_tool()] {
            serde_json::from_str::<serde_json::Value>(tool.input_schema_json).unwrap();
            serde_json::from_str::<serde_json::Value>(tool.output_schema_json).unwrap();
            let meta: serde_json::Value =
                serde_json::from_str(tool.agentic_worker_metadata).unwrap();
            assert_eq!(meta["side_effects"], "read");
            assert_eq!(meta["confirmation_required"], false);
        }
    }
}
```

- [ ] **Step 2: Replace the body of `src/lib.rs`**

Keep the top `#![allow(...)]`, `pub mod bindings;`, and module declarations, but the module set is now `connections`, `guard`, `protocol`, `tool_meta` (NOT input/output). Replace the whole file with:

```rust
//! Greentic SQL (text-to-data) design extension — WIT export layer.
//!
//! Gives the agentic worker `sql_list_connections` + `sql_ask`. `sql_ask`
//! reads a connection from the `secret://sql/` registry, introspects the schema
//! via the gateway, asks a configurable OpenAI-compatible LLM to write read-only
//! SQL, guards it (SELECT-only), runs it via the gateway, and returns rows plus
//! the generated SQL. Protocol/guard/registry logic lives in the pure
//! [`connections`], [`guard`], [`protocol`], and [`tool_meta`] modules.
#![allow(clippy::used_underscore_items)]

#[allow(warnings)]
pub mod bindings;

pub mod connections;
pub mod guard;
pub mod protocol;
pub mod tool_meta;

pub use bindings::exports;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_design::tools;
use bindings::greentic::extension_base::types;
use bindings::greentic::extension_host::{http, secrets};
use serde_json::{json, Value};

pub struct Component;

const CONNECTIONS_REF: &str = "secret://sql/connections";
const LLM_REF: &str = "secret://sql/llm";
const DEFAULT_MAX_ROWS: u32 = 100;
const MAX_ROWS_CEILING: u32 = 1000;

// ===== base::manifest =====
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "greentic.sql".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef { id: "greentic:sql/query".into(), version: "1.0.0".into() }]
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

/// GET `url` with a Bearer token; return the 2xx body bytes.
fn http_get(url: &str, token: &str) -> Result<Vec<u8>, types::ExtensionError> {
    let response = http::fetch(&http::Request {
        method: "GET".into(),
        url: url.into(),
        headers: vec![("authorization".into(), format!("Bearer {token}"))],
        body: None,
    })
    .map_err(|error| types::ExtensionError::Internal(format!("http get {url}: {error}")))?;
    check_status(url, response)
}

/// POST JSON `body` to `url` with a Bearer token; return the 2xx body bytes.
fn http_post_json(url: &str, token: &str, body: &Value) -> Result<Vec<u8>, types::ExtensionError> {
    let bytes = serde_json::to_vec(body)
        .map_err(|error| types::ExtensionError::Internal(format!("encode body: {error}")))?;
    let response = http::fetch(&http::Request {
        method: "POST".into(),
        url: url.into(),
        headers: vec![
            ("authorization".into(), format!("Bearer {token}")),
            ("content-type".into(), "application/json".into()),
        ],
        body: Some(bytes),
    })
    .map_err(|error| types::ExtensionError::Internal(format!("http post {url}: {error}")))?;
    check_status(url, response)
}

fn check_status(url: &str, response: http::Response) -> Result<Vec<u8>, types::ExtensionError> {
    if (200..300).contains(&response.status) {
        return Ok(response.body);
    }
    let detail = String::from_utf8_lossy(&response.body);
    Err(if response.status == 401 {
        types::ExtensionError::PermissionDenied(format!("{url} rejected the token (401): {detail}"))
    } else {
        types::ExtensionError::Internal(format!("{url} failed ({}): {detail}", response.status))
    })
}

/// Read + parse the connection registry. Returns an empty vec when the secret
/// is absent (so `sql_list_connections` degrades gracefully).
fn load_connections() -> Vec<connections::Connection> {
    match secrets::get(CONNECTIONS_REF) {
        Ok(json) => connections::parse_registry(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Decoded `secret://sql/llm` config.
#[derive(serde::Deserialize)]
struct LlmConfig {
    base_url: String,
    model: String,
    api_key: String,
}

fn load_llm() -> Result<LlmConfig, types::ExtensionError> {
    let json = secrets::get(LLM_REF).map_err(|error| {
        types::ExtensionError::PermissionDenied(format!("resolve secret {LLM_REF}: {error}"))
    })?;
    serde_json::from_str::<LlmConfig>(&json)
        .map_err(|error| types::ExtensionError::Internal(format!("decode llm config: {error}")))
}

// ===== design::tools =====
impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        [tool_meta::sql_list_connections_tool(), tool_meta::sql_ask_tool()]
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
            tool_meta::SQL_LIST_CONNECTIONS_TOOL => {
                let list: Vec<Value> = load_connections()
                    .into_iter()
                    .map(|c| json!({ "name": c.name, "engine": c.engine, "description": c.description }))
                    .collect();
                serde_json::to_string(&json!({ "connections": list }))
                    .map_err(|error| types::ExtensionError::Internal(format!("encode output: {error}")))
            }
            tool_meta::SQL_ASK_TOOL => sql_ask(&args_json),
            other => Err(types::ExtensionError::InvalidInput(format!("unknown tool: {other}"))),
        }
    }
}

fn sql_ask(args_json: &str) -> Result<String, types::ExtensionError> {
    let args: Value = serde_json::from_str(args_json)
        .map_err(|error| types::ExtensionError::InvalidInput(format!("decode sql_ask input: {error}")))?;
    let connection_name = args["connection"].as_str().ok_or_else(|| {
        types::ExtensionError::InvalidInput("`connection` is required".to_string())
    })?;
    let question = args["question"].as_str().ok_or_else(|| {
        types::ExtensionError::InvalidInput("`question` is required".to_string())
    })?;
    let max_rows = args["max_results"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .map_or(DEFAULT_MAX_ROWS, |n| n.clamp(1, MAX_ROWS_CEILING));

    let conns = load_connections();
    let conn = connections::find(&conns, connection_name).ok_or_else(|| {
        types::ExtensionError::InvalidInput(format!("unknown connection: {connection_name}"))
    })?;
    let base = conn.base_url.trim_end_matches('/');
    let token = secrets::get(&format!("secret://sql/{connection_name}/token")).map_err(|error| {
        types::ExtensionError::PermissionDenied(format!("resolve connection token: {error}"))
    })?;

    // 1. Schema (gateway-cached).
    let schema_bytes = http_get(&format!("{base}/schema"), &token)?;
    let schema = protocol::parse_schema(&String::from_utf8_lossy(&schema_bytes))
        .map_err(types::ExtensionError::Internal)?;
    let engine = if schema.engine.is_empty() { conn.engine.clone() } else { schema.engine.clone() };
    let schema_text = protocol::format_schema_prompt(&schema);

    // 2. LLM -> SQL.
    let llm = load_llm()?;
    let llm_url = format!("{}/chat/completions", llm.base_url.trim_end_matches('/'));
    let llm_req = protocol::build_llm_request(&llm.model, &engine, &schema_text, question);
    let llm_bytes = http_post_json(&llm_url, &llm.api_key, &llm_req)?;
    let sql = protocol::extract_sql(&String::from_utf8_lossy(&llm_bytes))
        .map_err(types::ExtensionError::Internal)?;

    // 3. Guard.
    guard::ensure_read_only(&sql).map_err(types::ExtensionError::InvalidInput)?;

    // 4. Execute.
    let query_req = protocol::build_query_request(&sql, max_rows);
    let query_bytes = http_post_json(&format!("{base}/query"), &token, &query_req)?;
    let result = protocol::parse_query_response(&String::from_utf8_lossy(&query_bytes))
        .map_err(types::ExtensionError::Internal)?;

    serde_json::to_string(&json!({
        "connection": connection_name,
        "sql": sql,
        "columns": result.columns,
        "rows": result.rows,
        "row_count": result.row_count,
        "truncated": result.truncated,
    }))
    .map_err(|error| types::ExtensionError::Internal(format!("encode output: {error}")))
}

#[cfg(target_family = "wasm")]
bindings::export!(Component with_types_in bindings);
```

- [ ] **Step 3: Delete inherited Tavily modules**

```bash
git rm src/input.rs src/output.rs
```

- [ ] **Step 4: Verify host tests pass**

Run: `cargo test`
Expected: PASS — `connections` (3) + `guard` (6) + `protocol` (6) + `tool_meta` (3) = 18 tests.

- [ ] **Step 5: Build the WASM component**

Run: `cargo fmt --all && cargo component build --release --target wasm32-wasip2`
Expected: `Finished`.

- [ ] **Step 6: Lint**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: zero warnings. The `max_rows` line already uses `u32::try_from` (no `as` cast) to stay clippy-pedantic-clean. Fix any other clippy issues in the glue cleanly (mirror patterns the reference crates allow); report any fixes.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(sql): wire sql_list_connections + sql_ask pipeline (schema->LLM->guard->query)"
```

---

## Task 5: Packaging (`build.sh`) → `.gtxpack`

**Files:**
- Modify: `build.sh`

- [ ] **Step 1: Fix `build.sh`**

Change the wasm artifact path and the temp-zip prefix:
```bash
WASM_PATH="target/wasm32-wasip2/release/greentic_sql_extension.wasm"
```
```bash
TMP_ZIP="$STAGE/../greentic_sql_$$.zip"
```
Leave the rest unchanged.

- [ ] **Step 2: Build the package**

```bash
bash build.sh
```
Expected: ends with `==> built …/greentic.sql-1.2.0-research.gtxpack`.

- [ ] **Step 3: Verify package + digest**

```bash
unzip -l greentic.sql-1.2.0-research.gtxpack
unzip -p greentic.sql-1.2.0-research.gtxpack describe.json | jq '.runtime.components["sql-tool"].sha256'
jq '.runtime.components["sql-tool"].sha256' describe.json
```
Expected: archive lists `describe.json` + `extension.wasm`; the packaged sha256 is a real 64-hex digest; the on-disk source `describe.json` still shows the all-zeros placeholder.

- [ ] **Step 4: Commit**

```bash
git add build.sh
git commit -m "build(sql): package extension into .gtxpack"
```

---

## Task 6: README + final gate

**Files:**
- Replace: `README.md`

- [ ] **Step 1: Write `README.md`**

````markdown
# component-sql-ext

A Greentic Designer **design extension** (WASM, `wasm32-wasip2`) that gives the
agentic worker a **text-to-data** tool: the agent asks a database a
natural-language question and gets rows back. The extension writes the SQL
itself (via a configurable OpenAI-compatible LLM), guards it to **read-only**
(SELECT-only), and runs it through an operator-run **SQL→HTTP gateway**.

## Tools

- `sql_list_connections()` → `[{ name, engine, description }]`
- `sql_ask(connection, question, max_results?)` → `{ connection, sql, columns, rows, row_count, truncated }`

## Configuration (host secrets, under `secret://sql/`)

| Secret | Contents |
|--------|----------|
| `secret://sql/connections` | JSON array: `[{ "name", "base_url", "engine", "description" }]` |
| `secret://sql/<name>/token` | Bearer token for that connection's gateway |
| `secret://sql/llm` | `{ "provider":"openai", "base_url":"https://api.openai.com/v1", "model":"gpt-4o-mini", "api_key":"sk-…" }` |

## Gateway contract (you provide one HTTP endpoint per database)

- `GET {base_url}/schema` → `{ "engine", "tables":[{"name","columns":[{"name","type"}]}] }` (cached, read-only role)
- `POST {base_url}/query` `{ "sql", "max_rows" }` → `{ "columns", "rows", "row_count", "truncated" }`

The gateway is a separate companion (axum + sqlx) — see its own spec. It MUST
use a read-only DB role and SHOULD cache the schema.

## Network

The extension calls the LLM host and each connection's gateway host. Because
`describe.json`'s `network` allow-list is static, the operator must grant those
hosts via runtime host-overrides.

## Build

```sh
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo component build --release --target wasm32-wasip2
bash build.sh
```
````

- [ ] **Step 2: Run the full gate**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo component build --release --target wasm32-wasip2
bash build.sh
```
Expected: every command exits 0. (Run `cargo fmt --all` first if the check fails on a regenerated `bindings.rs`.)

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(sql): add README"
```

---

## Out of scope / follow-ups

- **Gateway reference impl** (axum + sqlx: Postgres/MySQL/SQLite → `/schema` + `/query`, read-only role, schema cache) — required companion, separate spec.
- **Write/mutation tools** — v1 is read-only.
- **Non-OpenAI-compatible LLM providers** (Anthropic-native shape) — `provider` field is the extension point.
- **Network host-override** documentation for operators (dynamic LLM/gateway hosts).
- **Large-schema handling** — v1 sends the full cached schema to the LLM; table-relevance pre-selection is a future refinement.
- **CI/CD + branch topology** — add `ci/local_check.sh` + `.github/workflows/{ci,release}.yml` (per-branch policy: research→-research, develop→-dev, main→stable) and create `develop`/`research`, mirroring the tavily/github-mcp repos.
- **Live worker execution** — gated on aw-runtime `ConfigProvider` (Phase 4).
