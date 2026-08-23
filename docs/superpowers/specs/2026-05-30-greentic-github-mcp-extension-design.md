# greentic.github-mcp — Remote GitHub MCP Proxy Extension

**Date:** 2026-05-30
**Status:** Design (approved for planning)
**Kind:** `DesignExtension` (WASM, `wasm32-wasip2`)

## Overview

`greentic.github-mcp` is a Greentic Designer **DesignExtension** that exposes
GitHub's tools to the **agentic worker** by proxying to GitHub's official
**remote MCP server** (`https://api.githubcopilot.com/mcp/`) over the host
`http` capability. Unlike `greentic.tavily` (which wraps a plain REST API), this
extension speaks the **MCP Streamable HTTP / JSON-RPC 2.0** protocol: it calls
the remote server's `tools/list` to enumerate GitHub's tools and proxies
`tools/call` to execute them. The GitHub Personal Access Token (PAT) is read
from the host `secrets` interface and never returned.

This is the second extension in the third-party-API family and the first to
proxy an MCP server. It deliberately does **not** reimplement GitHub REST
endpoints, and it does **not** require the local (stdio) GitHub MCP server,
which a WASM sandbox cannot run.

## Goals

- Ship a signed `.gtxpack` extension that surfaces GitHub MCP read tools to the
  agentic worker, each flagged `agentic_worker`.
- Establish the reusable MCP-over-HTTP client plumbing (JSON-RPC build/parse,
  SSE unwrapping, handshake orchestration) inside a design extension.
- Keep the extension a thin proxy: no GitHub business logic beyond MCP
  request/response translation and error normalisation.

## Non-Goals (v1)

- Write tools (create/update issues, PRs, etc.) — v1 runs `X-MCP-Readonly: true`.
- A generic, config-driven multi-MCP-server bridge — this v1 hardcodes the
  GitHub endpoint; generalisation is a later spec.
- The platform-level MCP→agentic-worker bridge in `aw-runtime` — separate,
  larger track.
- OAuth device/authorization-code flow — v1 uses a PAT only.
- Signing / `gtdx publish` flow — separate concern (see Tavily spec follow-ups).

## Architecture

### Sandbox constraints (carried from the Tavily build)

A DesignExtension runs as `wasm32-wasip2` with exactly five host imports:
`secrets`, `http`, `logging`, `i18n`, `broker`. No filesystem, no process spawn,
no stdio — which is why the **local** GitHub MCP server (Go binary, stdio) is
unusable here and we target the **remote** server over HTTP.

### Remote MCP transport (verified facts)

- Endpoint: `https://api.githubcopilot.com/mcp/`.
- Auth: header `Authorization: Bearer <PAT>`.
- Filtering headers: `X-MCP-Readonly: true` (read tools only),
  `X-MCP-Toolsets: <comma list>` (e.g. `repos,issues,pull_requests`).
- Transport: MCP Streamable HTTP — each JSON-RPC message is an HTTP `POST`;
  the server responds with **either** `application/json` **or**
  `text/event-stream` (SSE). The client sends
  `Accept: application/json, text/event-stream`.
- Protocol semantics (per the MCP Streamable HTTP spec): client sends
  `initialize` first; the server may return an `Mcp-Session-Id` header that the
  client echoes on subsequent requests; after `initialize` the client sends a
  `notifications/initialized` notification; later requests carry an
  `MCP-Protocol-Version` header.

> **Unverified-until-live:** the exact GitHub-server behaviour (whether
> `Mcp-Session-Id` is mandatory, whether responses are JSON or SSE in practice,
> whether `notifications/initialized` is required before `tools/call`) is taken
> from the MCP spec, not GitHub's own docs. The plan's first task is a live
> `curl` handshake against the real server with a PAT to confirm, before any
> Rust is written.

### Per-call statelessness

Each WASM `invoke_tool` / `list_tools` runs in a **fresh instance** with no
persistent state, so no MCP session survives across calls. This is fine: each
call performs the full handshake as a short sequence of sequential HTTP POSTs
within the one call — `initialize` → (`notifications/initialized`) →
`tools/list` or `tools/call` — threading the `Mcp-Session-Id` and protocol
version returned by `initialize` through the later POSTs in the same call. The
cost is ~2–3 round trips per call (acceptable for v1).

### Component shape

- New standalone crate `component-github-mcp-ext` (sibling of
  `component-tavily-ext`), copied from the Tavily crate and renamed
  (id `greentic.github-mcp`, world `greentic:github-mcp-extension`).
- Minimal world: exports `manifest` + `lifecycle` + `tools@0.2.0` only.
- Host imports used: `http`, `secrets`, `logging`.

### Modules (pure + glue split, mirroring Tavily)

- **`mcp_proto.rs`** (pure, host-testable, no WIT imports):
  - Build JSON-RPC request bodies: `initialize`, `notifications/initialized`,
    `tools/list`, `tools/call`.
  - Parse a response body into a JSON-RPC value, **unwrapping SSE**: if the
    body is `text/event-stream`, extract the `data:` line(s) and pick the
    JSON-RPC object whose `id` matches the request; otherwise parse as plain
    JSON.
  - Map an MCP `tools/list` result → `Vec<ToolMeta>` (name, description,
    `input_schema_json` from MCP `inputSchema`, `capabilities:["agentic_worker"]`,
    conservative `agentic_worker_metadata`: `side_effects:read`,
    `confirmation_required:false`, `cost:medium`).
  - Extract a `tools/call` result's `content`/`structuredContent` and `isError`.
  - Detect JSON-RPC `error` objects.
- **`lib.rs`** (thin WIT glue): orchestrate the HTTP handshake sequence, thread
  session id + protocol version, call host `http::fetch` + `secrets::get`, and
  map failures to `ExtensionError`.

### Configuration constants (v1, hardcoded)

```
ENDPOINT          = "https://api.githubcopilot.com/mcp/"
PAT_SECRET_REF    = "secret://github/token"
TOOLSETS          = "repos,issues,pull_requests"   // X-MCP-Toolsets
READONLY          = "true"                          // X-MCP-Readonly
PROTOCOL_VERSION  = "2025-06-18"                    // confirm in Task 1
```

### `list_tools()` behaviour

Handshake → `tools/list` → map each tool. **Graceful degradation:** if the PAT
secret is absent or the handshake/network fails, return an **empty list**
(logged via host `logging`) rather than erroring — so the DW Composer's tool
picker never breaks when GitHub isn't configured yet. Tool names are exposed
**as-is** from GitHub MCP (e.g. `get_issue`, `search_code`) so `invoke_tool`
forwards them unchanged; cross-extension name collisions are disambiguated by
`extension_id` in the agentic-worker allow-list.

### `invoke_tool(name, args_json)` behaviour

Handshake → `tools/call(name, arguments = args_json)` →
- JSON-RPC `error` → `ExtensionError::Internal` (or `InvalidInput` for
  `-32602` invalid params).
- result with `isError: true` → `ExtensionError::Internal` with the error text.
- success → return the result `content` (text) / `structuredContent` serialised
  as a JSON string.

### Permissions (`describe.json`)

```json
"runtime": {
  "permissions": {
    "network": ["https://api.githubcopilot.com/*"],
    "secrets": ["secret://github/"],
    "callExtensionKinds": []
  }
}
```

### Error handling

`result<string, extension-error>`, never panic. Categories: missing/denied
secret (`PermissionDenied`), HTTP transport failure (`Internal`), non-2xx
(`Internal`; 401 → `PermissionDenied` "PAT rejected"), unparseable JSON/SSE
(`Internal`), JSON-RPC error (`Internal`/`InvalidInput`), MCP `isError`
(`Internal`).

### Testing

- Unit tests (host): JSON-RPC request building; response parsing for **both**
  `application/json` and `text/event-stream` (SSE) fixtures; `tools/list` →
  `ToolMeta` mapping; `tools/call` content extraction + `isError`; JSON-RPC
  error detection. No live network in tests — captured fixtures only.
- A live handshake smoke step (manual, PAT-gated) documented in the plan's
  Task 1 to validate the protocol assumptions before implementation.
- `gtdx` validates `describe.json` in CI.

## Open items / risks (resolve during planning)

1. **Live protocol verification (Task 1, blocking):** confirm against the real
   server — JSON vs SSE responses, `Mcp-Session-Id` requirement,
   `notifications/initialized` necessity, exact `MCP-Protocol-Version`. Adjust
   the design if reality differs.
2. **`list_tools` per-agent-step cost:** the aw-runtime calls `list_tools` each
   loop step; fresh WASM instances cannot cache the `tools/list` result, so each
   call re-handshakes over the network. This is a **runtime caching concern**
   (out of the extension's control) and compounds the unimplemented Phase-4
   `ConfigProvider`. Flag for the runtime track; the DW Composer's one-shot
   `list_tools` call is unaffected.
3. **PAT provisioning:** operators must store the GitHub PAT under
   `secret://github/token`; without it the extension lists no tools and cannot
   invoke. Same provisioning gap noted for Tavily.
4. **Toolset surface size:** even read-only, `repos,issues,pull_requests` may
   expose many tools to the LLM. If it floods the agent, narrow `TOOLSETS` or
   add a curated allow-list filter in `list_tools`.

## End-to-end path (for reference)

build `.gtxpack` → operator catalog (published, tenant not blocked) → designer
install + enable → DW Composer "Extension Tools" picker (`list_tools` does a
live GitHub MCP handshake, needs the PAT) filters `capability=agentic_worker` →
selected tools snapshot into `DigitalWorkerManifest.extension_tools` →
`manifest_to_tool_refs` → `AgentConfig.tools` → AW loop → `invoke_tool` proxies
`tools/call` to GitHub MCP. (Live worker execution still gated on the Phase-4
`ConfigProvider`.)
