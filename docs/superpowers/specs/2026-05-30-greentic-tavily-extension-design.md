# greentic.tavily — Web Search/Research Extension for the Agentic Worker

**Date:** 2026-05-30
**Status:** Design (approved for planning)
**Kind:** `DesignExtension` (WASM, `wasm32-wasip2`)

## Overview

`greentic.tavily` is a Greentic Designer **DesignExtension** that exposes
[Tavily](https://tavily.com) web-search capabilities to the **agentic worker**
as LLM-invocable tools. It is the first of a family of "third-party API"
extensions and doubles as the **reference template** for the rest (GitHub,
dwbase knowledge base, etc.): every such extension follows the same shape —
read an API key from the host `secrets` interface, make an outbound call via
the host `http` interface, map the response to a stable output schema, and
return it to the agent.

The defining requirement for a tool to surface in the agentic worker is the
`agentic_worker` capability flag on each `tool-definition` returned from
`list-tools()` (see `greentic-extension-sdk-contract::ToolCapability`).

## Goals

- Ship a signed `.gtxpack` extension exposing two agentic-worker tools:
  `tavily_search` and `tavily_extract`.
- Establish a reusable authoring pattern (secrets → http → map → return) that
  later third-party extensions copy verbatim.
- Keep the extension a **thin client**: no business logic beyond request
  construction, response mapping, and error normalisation.

## Non-Goals (separate specs later)

- GitHub direct-API extension.
- dwbase-backed knowledge-base extension.
- The **MCP→agentic-worker bridge** (the "proper" platform feature that would
  let any MCP server become agent tools) — its own strategic spec.
- Tavily `crawl` / `map` endpoints (beta) — deferred under YAGNI.

## Architecture

### Sandbox constraints (verified)

A DesignExtension runs as a `wasm32-wasip2` component with exactly five host
imports and nothing else:

| Host import | Signature | Use here |
|-------------|-----------|----------|
| `secrets` | `get(uri) -> result<string, string>` | read the Tavily API key |
| `http` | `fetch(request) -> result<response, string>` | call `api.tavily.com` |
| `logging` | `log` / `log-kv` | diagnostics |
| `i18n` | `t` / `tf` | (unused in v1) |
| `broker` | `call-extension(...)` | (unused in v1) |

No filesystem, no process spawn, no native libraries. This is why every
third-party integration reduces to "thin HTTP client + secret".

### Component shape

- Scaffold via `gtdx new tavily --kind design --id greentic.tavily`.
- WIT world `design-extension`. We genuinely implement only the `tools`
  interface (`list-tools` / `invoke-tool`); `validation`, `prompting`,
  `knowledge`, and `roles` are stubbed to return empty values so the world
  contract is satisfied.
- Build with `cargo-component` → `wasm32-wasip2`; package + sign with
  `gtdx publish` → `.gtxpack`.

### Tools

Both tools are returned by `list-tools()` with
`capabilities: ["agentic_worker"]` and an `agentic-worker-metadata` JSON blob
(decoded by `AgenticWorkerMetadata`).

#### `tavily_search`

Context-rich web search for the LLM.

Input (JSON Schema, Draft 2020-12):

| Field | Type | Req | Default | Notes |
|-------|------|-----|---------|-------|
| `query` | string | yes | — | search query |
| `max_results` | integer | no | 5 | 1–20 |
| `search_depth` | enum `basic`\|`advanced` | no | `basic` | |
| `topic` | enum `general`\|`news` | no | `general` | |
| `include_answer` | boolean | no | `true` | request Tavily's summarised answer |
| `include_domains` | string[] | no | — | allowlist |
| `exclude_domains` | string[] | no | — | denylist |
| `time_range` | string | no | — | e.g. `day`/`week`/`month`/`year` |

Output:

```json
{
  "answer": "optional summarised answer or null",
  "results": [
    { "title": "...", "url": "...", "content": "...", "score": 0.0 }
  ],
  "query": "echoed query"
}
```

`agentic-worker-metadata`: `usage_hint` = "Search the live web for current
facts or anything outside the model's knowledge", `side_effects: read`,
`cost: medium`, `confirmation_required: false`, plus 1–2 `examples`.

#### `tavily_extract`

Fetch clean content from already-known URLs.

Input:

| Field | Type | Req | Default |
|-------|------|-----|---------|
| `urls` | string[] | yes | — |
| `extract_depth` | enum `basic`\|`advanced` | no | `basic` |

Output:

```json
{
  "results": [ { "url": "...", "raw_content": "..." } ],
  "failed_results": [ { "url": "...", "error": "..." } ]
}
```

Metadata: `side_effects: read`, `cost: medium`, `confirmation_required: false`.

### `invoke-tool(name, args_json)` flow

1. Match `name` → `tavily_search` | `tavily_extract` | else `unknown tool`
   error.
2. Parse `args_json`; validate against the tool's input schema. Invalid →
   `extension-error` (invalid input) with a descriptive message.
3. Read the API key via `secrets.get(<uri>)`. Missing/denied → `extension-error`.
4. Build the `http::request`: `POST https://api.tavily.com/{search|extract}`,
   header `Authorization: Bearer <key>`, `Content-Type: application/json`,
   JSON body assembled from validated args.
5. `http.fetch`. Transport error → `extension-error`.
6. Inspect status: 401 → "invalid Tavily API key"; 429 → "Tavily rate limit
   exceeded"; other non-2xx → status + body excerpt. All mapped to
   `extension-error`.
7. Parse the Tavily JSON, project to the stable output schema (drop fields we
   do not expose), serialise, and return `Ok(json_string)`.

### Permissions (`describe.json`)

```json
"runtime": {
  "component": "extension.wasm",
  "permissions": {
    "network": ["https://api.tavily.com"],
    "secrets": ["secrets://*/tavily/*"],
    "callExtensionKinds": []
  }
}
```

Default-deny: only `api.tavily.com` is reachable; only the Tavily secret URI
pattern is readable.

### Error handling

All failure paths return `result<string, extension-error>`; the component
never panics. Error categories: unknown tool, invalid input, missing/denied
secret, HTTP transport failure, non-2xx upstream (401/429 called out
explicitly), and unparseable response.

### Testing

- Unit tests (`rlib`): argument parsing/validation, request construction,
  response mapping against captured Tavily fixture JSON, and error mapping for
  401/429/malformed bodies. **No live network calls** — HTTP is mocked.
- `gtdx` validates `describe.json` in CI.
- Where practical, use `greentic-extension-sdk-testing` to drive
  `list-tools` / `invoke-tool` through the runtime harness.

## Open items to resolve during planning

1. **Secret URI convention** — confirm the exact `secrets://…` scheme and how
   the host scopes it to the current tenant/env at `secrets.get` time. The
   `permissions.secrets` pattern above (`secrets://*/tavily/*`) is provisional
   pending the host `secrets` implementation.
2. **Operator key provisioning** — how a per-tenant Tavily API key is loaded
   into the configured secrets backend.
3. **Agentic-worker runtime wiring** — `ConfigProvider` (manifest
   `extension_tools` → `AgentConfig.tools`) has **no concrete implementation in
   `greentic-aw-runtime` yet** (Phase 4). Consequence: building the extension,
   installing it, and surfacing its tools in the DW Composer
   (`GET /api/extensions/tools?capability=agentic_worker`) is achievable now;
   *live tool execution inside a running worker* may remain blocked downstream
   until `ConfigProvider` lands. Verify this early so "end-to-end working in a
   live worker" expectations are accurate.

## End-to-end path (for reference)

build `.gtxpack` → operator catalog (`status=published`, tenant not blocked) →
designer install (`~/.greentic/extensions/design/...`, enabled) → DW Composer
"Extension Tools" picker filters `capability=agentic_worker` → selected tools
snapshot into `DigitalWorkerManifest.extension_tools` → `manifest_to_tool_refs`
→ `AgentConfig.tools` → AW loop `list_tools_for_llm` / `is_tool_allowed` →
`ExtensionRuntime::invoke_tool` dispatch.
