# SQL Gateway in the Runtime — Companion to greentic.sql

**Date:** 2026-05-31
**Status:** Design (approved for planning)
**Where it lives:** `greentic-runner-host` (new `src/sql/` module + axum routes + state) and `greentic-start` (bundle config parse + threading). Cross-repo runtime feature. Deploys as part of the single bundle — no separate service.

## Overview

The `greentic.sql` design extension (text-to-data) can only reach databases over
HTTP and depends on a gateway that exposes `GET /schema` + `POST /query`. To keep
everything in **one bundle** (no separate sidecar to deploy), this gateway is
implemented **inside the runtime** (`greentic-runner-host`): the runner's existing
axum server gains `/sql/<conn>/schema` + `/sql/<conn>/query` routes, backed by
native `sqlx` connection pools for SQLite/Postgres/MySQL. The `greentic.sql`
extension is unchanged — its connection `base_url` simply points at the runner's
own endpoint (e.g. `http://localhost:<port>/sql/<conn>`).

This is the **first real implementation** of the `requires_runtime_execution`
contract: prior art (`demo-chat2data`) declared it but nothing in the runtime
consumed it; SQL execution was entirely stubbed and no DB drivers exist in the
runtime today.

## Goals

- Add a SQL gateway module to `greentic-runner-host` exposing `/sql/<conn>/schema`
  + `/sql/<conn>/query` on the existing axum server.
- Support **SQLite + Postgres + MySQL** via a single `sqlx` dependency.
- Read-only execution (read-only sqlx connection/transaction + documented
  read-only DB role).
- **Schema introspection cached** in runtime state (the cache the extension relies
  on; the extension itself is stateless).
- Configure connections from the **bundle** (`databases:`/`sql:` block), with
  DSNs and the gateway auth token resolved **from secrets**.
- Shared **Bearer-token** auth on the `/sql` routes (server is network-exposed).

## Non-Goals (v1)

- Write/mutation queries — read-only only.
- Per-tenant connection sets — v1 is one connection set per bundle.
- Wiring the chat2data **flow** path (`requires_runtime_execution`) — this spec
  serves the agentic-worker SQL extension; the flow path can reuse the endpoint later.
- Auto-deriving the extension's `secret://sql/connections` registry from the bundle
  config — v1 keeps them as two operator-aligned configs (flagged as a follow-up).

## Architecture

### Module structure (`greentic-runner-host/src/sql/`)

- `config.rs` — `SqlGatewayConfig` / `ConnectionConfig { engine, dsn_secret, read_only }`.
- `pool.rs` — build an `sqlx` pool per connection (engine-specific, read-only).
- `introspect.rs` — per-engine schema introspection → the `Schema` shape.
- `execute.rs` — run a query with `max_rows` cap + timeout; map rows → JSON.
- `routes.rs` — axum handlers for `/sql/<conn>/schema` + `/sql/<conn>/query` + the
  Bearer-token guard.
- `mod.rs` — `SqlGateway` state: `HashMap<conn, { pool, engine }>` +
  `RwLock<HashMap<conn, Schema>>` schema cache + the expected auth token.

Routes register in the existing `HostServer::new()` router
(`greentic-runner-host/src/runner/mod.rs`); `SqlGateway` is added to `ServerState`.

### Bundle config (newly parsed; `greentic-start` `DemoConfig`)

```yaml
sql:
  auth_token_secret: secret://sql/gateway_token
  connections:
    analytics:
      engine: postgres            # postgres | mysql | sqlite
      dsn_secret: secret://sql/analytics/dsn
      read_only: true
    app:
      engine: sqlite
      dsn_secret: secret://sql/app/dsn
      read_only: true
```

`greentic-start` parses this, resolves `auth_token_secret` + each `dsn_secret`
from the secrets backend at startup, builds the pools, and threads the
`SqlGateway` into `greentic-runner-host`'s `ServerState` (via `RunnerConfig` /
`HostBuilder`). No DSN or token appears in plaintext YAML.

### Endpoints (match the `greentic.sql` extension's contract)

- **`GET /sql/<conn>/schema`** → `{ "engine", "tables":[{"name","columns":[{"name","type"}]}] }`
  — served from the cached introspection (populated on first request or startup).
- **`POST /sql/<conn>/query`** `{ "sql", "max_rows" }` →
  `{ "columns":[…], "rows":[[…]], "row_count":N, "truncated":bool }`.
- **Auth**: `Authorization: Bearer <token>` validated (constant-time) against the
  resolved `auth_token_secret`. Operator stores the same token at
  `secret://sql/<name>/token` for the extension. Missing/wrong → 401.
- Unknown `<conn>` → 404. Engine/driver error → 502 with a sanitized message
  (never leak the DSN/credentials).

### Read-only enforcement (layered)

- SQLite: open the pool with read-only mode (`SQLITE_OPEN_READ_ONLY` / `?mode=ro`).
- Postgres: run each query inside a `BEGIN TRANSACTION READ ONLY` (or set
  `default_transaction_read_only`).
- MySQL: `START TRANSACTION READ ONLY`.
- Documented requirement: the operator's DB user SHOULD be a read-only role
  (real backstop). The extension's own SELECT-only guard is the first line.

### Schema cache

Introspect once per connection (lazily on the first `/schema`, or eagerly at
startup), store in the `RwLock<HashMap<conn, Schema>>`. Subsequent `/schema`
calls (which the extension makes on every `sql_ask`) are served from cache. A
manual refresh path (re-introspect) is a future nicety, not v1.

### Query execution + limits

- Enforce `max_rows`: fetch up to `max_rows + 1`; if the extra row exists, drop it
  and set `truncated: true`.
- A per-query timeout (e.g. 30s, configurable).
- Map each row to a JSON array of values aligned with `columns`; NULL → JSON null.
- Cap total result bytes (defensive) — oversized → error rather than OOM.

## Error handling

Never leak DSN/credentials. Auth failure → 401; unknown connection → 404; SQL
or driver error → 502 with the DB's error message (no connection string); the
SELECT-only guarantee is the extension's job (this gateway also relies on the
read-only connection). All responses JSON.

## Testing

- Unit tests for: config parse, `max_rows`/`truncated` logic, row→JSON mapping,
  per-engine introspection SQL builders, auth-guard accept/reject.
- Integration tests against an **ephemeral SQLite** file (no external server
  needed) exercising `/schema` + `/query` end-to-end through the axum router.
- Postgres/MySQL integration behind a feature/`docker-compose` (greentic-integration
  already runs Postgres in CI) — gated, not required for the unit suite.

## Open items (resolve during planning)

1. **Config location** — `greentic-start` `DemoConfig` (add a `sql` field) vs a
   new type in `greentic-config`. Confirm against existing config plumbing.
2. **Network allow-list** — the extension calls `http://localhost:<port>/sql/...`;
   the runner-host should auto-allow its own SQL endpoint in the extension's
   network permissions (or document an operator host-override). Without it the
   extension's `http.fetch` to the gateway is denied.
3. **Registry alignment** — the extension's `secret://sql/connections` `base_url`
   must point at the runner's `/sql/<conn>`; v1 is operator-configured. Consider
   auto-deriving it from the bundle `sql:` block later.
4. **Sandbox/access** — `greentic-runner` is not directly accessible from the
   primary agent's tools (EPERM); implementation runs via subagents (which have
   access). Confirm the build environment at execution time.
5. **`sqlx` offline/compile** — `sqlx` macros need `DATABASE_URL` or offline
   prepared data at compile time if the query macros are used; prefer the runtime
   (non-macro) `sqlx::query` API to avoid a compile-time DB dependency.

## End-to-end (1 bundle)

`gtc start <bundle>` → greentic-start parses `sql:` config, resolves secrets,
builds sqlx pools, threads `SqlGateway` into the runner-host's `ServerState` →
the runner's axum server serves `/sql/<conn>/{schema,query}` → the `greentic.sql`
extension (loaded in the same runtime) calls those endpoints over localhost HTTP
with the shared Bearer token → text-to-data works inside one deployment.
