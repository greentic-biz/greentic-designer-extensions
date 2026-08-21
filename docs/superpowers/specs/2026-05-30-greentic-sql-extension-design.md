# greentic.sql — Text-to-Data Extension for the Agentic Worker

**Date:** 2026-05-30
**Status:** Design (approved for planning)
**Kind:** `DesignExtension` (WASM, `wasm32-wasip2`)

## Overview

`greentic.sql` is a Greentic Designer **DesignExtension** that gives the agentic
worker a **text-to-data** capability: the agent asks a natural-language question
against a named database connection, and the extension returns the **data**. The
extension itself owns the natural-language → SQL translation (via a configurable
LLM) — SQL is a black box to the agent, which just gets rows back.

Because a WASM `wasm32-wasip2` component cannot open raw TCP, the extension never
talks to Postgres/MySQL/SQLite directly. Instead each connection points at an
operator-run **SQL→HTTP gateway** that speaks a small, fixed contract (`/schema`
+ `/query`). The gateway owns the actual database driver, the per-engine
introspection, and **schema caching**. "Several databases" = several gateway
connections in a registry, possibly different engines.

This is the third third-party-style extension after `greentic.tavily` and
`greentic.github-mcp`, and reuses the same crate template (pure modules + thin
WIT glue). It builds on the prior art in `demo-chat2data` (the 5-stage WASM NL→SQL
pipeline with whitelist/read-only security), collapsed into a single
agentic-worker tool.

## Goals

- Ship a `.gtxpack` extension exposing two `agentic_worker` tools:
  `sql_list_connections` and `sql_ask`.
- Own NL→SQL inside the extension using a **selectable, OpenAI-compatible LLM**.
- Support **multiple databases / engines** via a generic gateway contract — one
  extension code path, no per-engine logic in the extension.
- Enforce **read-only** access (SELECT-only guard + read-only gateway role).
- Return the generated SQL alongside the rows for transparency.

## Non-Goals (v1)

- The **gateway reference implementation** (a host service: axum + sqlx for
  Postgres/MySQL/SQLite implementing `/schema` + `/query` with schema caching and
  a read-only role). It is a REQUIRED companion but a separate stack and gets its
  own spec — without a gateway the extension has nothing to talk to.
- Write / mutation tools — v1 is read-only.
- Non-OpenAI-compatible LLM providers (e.g. Anthropic-native message shape) —
  the `provider` field is a designed extension point; v1 implements the
  `openai`-compatible chat-completions shape (which already covers OpenAI, Groq,
  OpenRouter, Together, local servers, etc. via `base_url`).
- Extension-managed schema caching — the gateway caches schema (the extension is
  stateless; see Architecture).
- Live worker execution wiring (gated on aw-runtime `ConfigProvider`, Phase 4).

## Architecture

### Sandbox + statelessness constraints (verified, carried from prior builds)

A DesignExtension has five host imports: `secrets`, `http`, `logging`, `i18n`,
`broker`. No filesystem, no raw TCP, **and no persistent memory across calls**
(`list_tools`/`invoke_tool` each run in a fresh component instance). Two
consequences shape this design:

1. The extension reaches databases only over **HTTP** → the **gateway** indirection.
2. The extension cannot hold an in-memory schema cache between calls → **the
   gateway caches schema** (introspects once on first load, serves `/schema` from
   cache). The extension fetches `/schema` per `sql_ask`; the gateway makes that
   cheap. (Alternative considered and rejected for v1: embedding a static schema
   snapshot in the registry config; rejected because it drifts from the live DB.)

### Gateway contract (operator-provided; engine-agnostic to the extension)

Each connection's `base_url` exposes:

- **`GET {base_url}/schema`** → introspected, **cached** schema:
  ```json
  { "engine": "postgres",
    "tables": [ { "name": "users",
                  "columns": [ { "name": "id", "type": "integer" },
                               { "name": "email", "type": "text" } ] } ] }
  ```
- **`POST {base_url}/query`** body `{ "sql": "SELECT …", "max_rows": 100 }` →
  ```json
  { "columns": ["id","email"], "rows": [[1,"a@x.com"]], "row_count": 1, "truncated": false }
  ```
- Auth: header `Authorization: Bearer <token>` (per-connection token from secrets).
- Non-2xx → mapped to an `ExtensionError` (401 → PermissionDenied).
- The gateway MUST connect with a **read-only** DB role (defense in depth).

### Configuration (all via the host `secrets` interface)

The extension's only config channel is `secrets`. Everything lives under the
`secret://sql/` prefix (one permission entry):

| Secret URI | Contents |
|------------|----------|
| `secret://sql/connections` | JSON array: `[{ "name", "base_url", "engine", "description" }]` |
| `secret://sql/<name>/token` | Bearer token for that connection's gateway |
| `secret://sql/llm` | `{ "provider": "openai", "base_url": "https://api.openai.com/v1", "model": "gpt-4o-mini", "api_key": "sk-…" }` |

`engine` in the registry is a display/fallback hint; the authoritative dialect
comes from `/schema`'s `engine` field.

### Tools exposed to the agent (both `agentic_worker`, read-only)

- **`sql_list_connections()`** → `[{ name, engine, description }]` from the
  registry. Lets the agent discover which databases it can query. Graceful-empty
  if the registry secret is absent.
- **`sql_ask(connection, question, max_results?)`** → runs the full pipeline and
  returns `{ connection, sql, columns, rows, row_count, truncated }`.

### `sql_ask` flow

1. Read `secret://sql/connections`; find the named connection (else `InvalidInput`).
2. Read `secret://sql/<name>/token` and `secret://sql/llm`.
3. `GET {base_url}/schema` (Bearer token) → schema + engine (gateway-cached).
4. Format the schema compactly and call the LLM
   (`POST {llm.base_url}/chat/completions`, Bearer `llm.api_key`): a system prompt
   instructing "write ONE read-only `{engine}` SQL query; return ONLY SQL" + a
   user message with engine + schema + question, `temperature: 0`.
5. Extract the SQL from the response (strip markdown fences if any).
6. **Guard SELECT-only** (see Safety). Reject → `InvalidInput` with the offending SQL.
7. `POST {base_url}/query` `{ sql, max_rows }` → rows.
8. Return `{ connection, sql, columns, rows, row_count, truncated }`.

`max_rows` = `max_results` if provided else a default (100), clamped to a ceiling
(e.g. 1000).

### Safety — read-only enforcement

Reuses `demo-chat2data`'s ideas, adapted to raw LLM-generated SQL:

- **SELECT-only guard** (extension, pure, host-tested): trim; strip a single
  trailing `;`; require the first keyword to be `SELECT` or `WITH`; reject if any
  forbidden statement keyword appears at a word boundary
  (`INSERT|UPDATE|DELETE|DROP|ALTER|CREATE|TRUNCATE|GRANT|REVOKE|MERGE|REPLACE|CALL|EXEC|ATTACH|PRAGMA`);
  reject multiple statements (an interior `;`); reject SQL comments (`--`, `/* */`).
  Best-effort first line of defense.
- **Read-only gateway role** — the gateway connects read-only, so even a guard
  bypass cannot mutate data. Documented as a hard requirement of the contract.
- `agentic-worker-metadata`: `side_effects: read`, `cost: medium`,
  `confirmation_required: false`.

### Network allow-list (open item — significant)

`permissions.network` in `describe.json` is a static, default-deny HTTPS-origin
allow-list (prefix match). This extension calls **dynamic hosts**: the LLM
`base_url` and every connection's gateway `base_url`, none known at build time.
Therefore the operator MUST grant these hosts via runtime **host-overrides**
(the runtime threads `HostOverrides` through `RuntimeConfig`) or a deployment-
specific allow-list. This is the same "host-global allow-list" limitation noted
for `greentic.http`. The `describe.json` will list a permissive default
(`https://api.openai.com/*` for the default LLM) and document that gateway hosts
must be added per deployment.

### Modules (pure + glue, mirroring the Tavily/github-mcp template)

- `connections.rs` (pure): `Connection { name, base_url, engine, description }`;
  parse the registry JSON; look up by name.
- `guard.rs` (pure): `ensure_read_only(sql) -> Result<(), String>`.
- `protocol.rs` (pure): format schema → prompt text; build the LLM chat request;
  extract SQL from the LLM response (fence-stripping); build the `/query` request;
  parse `/schema` and `/query` responses.
- `tool_meta.rs` (pure): the two tool definitions, JSON schemas, AW metadata.
- `lib.rs` (thin glue): orchestrate steps 1–8; read secrets; call host `http`.

### Error handling

`result<string, extension-error>`, never panic. Categories: missing/denied
secret (`PermissionDenied`), unknown connection / non-read-only SQL / bad args
(`InvalidInput`), HTTP transport failure (`Internal`), non-2xx from gateway or
LLM (401 → `PermissionDenied`, else `Internal`), unparseable JSON (`Internal`),
empty/missing SQL from the LLM (`Internal`). `sql_list_connections` degrades to
an empty list (logged) when the registry is absent.

### Testing

- Host unit tests (pure modules): registry parse/lookup; the SELECT-only guard
  (positive SELECT/WITH cases + each rejected category: DML/DDL, multi-statement,
  comments); schema→prompt formatting; LLM-response SQL extraction incl. fenced
  output; `/schema` and `/query` response parsing; request-body construction.
- No live network in tests — captured fixtures only.
- `gtdx` validates `describe.json`.

## Open items / risks (resolve during planning)

1. **Network allow-list for dynamic hosts** (above) — confirm the host-override
   path and document operator setup; without it the extension cannot reach the
   LLM or gateways.
2. **Gateway reference impl is a hard dependency** — the extension is untestable
   end-to-end until a gateway exists; ship its spec/impl in parallel.
3. **LLM cost/latency per `sql_ask`** — each call does `/schema` + one LLM
   completion + `/query`. Acceptable for v1; note token cost of sending schema.
4. **Schema size** — very large schemas may exceed the LLM context; v1 sends the
   full cached schema. A future refinement: table-relevance pre-selection.
5. **Secret-managed registry** — confirm operators can write JSON blobs to the
   configured secrets backend under `secret://sql/`.

## End-to-end path (for reference)

operator runs a gateway per DB (read-only role, schema cache) → stores
`secret://sql/connections`, `secret://sql/<name>/token`, `secret://sql/llm` →
build `.gtxpack` → install + enable → DW Composer surfaces `sql_list_connections`
+ `sql_ask` (`capability=agentic_worker`) → agent asks a question → extension:
schema → LLM → guard → query → data. (Live worker execution still gated on the
Phase-4 `ConfigProvider`.)
