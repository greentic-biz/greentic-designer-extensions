# SQL Gateway in the Runtime — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **ENV PRECONDITION:** this plan touches `greentic-runner` + `greentic-start`, which were inaccessible from the authoring environment (macOS TCC revoked git/FS access to the `~/Desktop` checkout mid-session — see memory `designer-tcc-desktop-git-block`). Execute only after a fresh session restores access. Integration tasks (1, 6, 7) reference the structure found during exploration; the implementer MUST `Read` the current file first and adapt to the live code (line numbers/fields may have shifted).

**Goal:** Add a SQL gateway inside `greentic-runner-host` exposing `GET /sql/<conn>/schema` + `POST /sql/<conn>/query` (read-only, SQLite/Postgres/MySQL via `sqlx`), configured from the bundle with secrets-resolved DSNs and a shared Bearer token — so `greentic.sql` works in one bundle with no separate service.

**Architecture:** A self-contained `src/sql/` module in `greentic-runner-host` (config, pool, introspect, execute, routes, state) wired into the existing axum router + `ServerState`. `greentic-start` parses the bundle `sql:` block, resolves secrets, builds the pools, and threads a `SqlGateway` into the host. Schema is introspected once and cached in runtime state.

**Tech Stack:** Rust, axum 0.8, `sqlx` 0.8 (features: runtime-tokio-rustls, sqlite, postgres, mysql), tokio.

**Spec:** [`../specs/2026-05-31-greentic-sql-gateway-runtime-design.md`](../specs/2026-05-31-greentic-sql-gateway-runtime-design.md)

**Repos touched:** `greentic-runner` (crate `greentic-runner-host`) and `greentic-start`.

**Reference points found during exploration (verify live):**
- Router: `greentic-runner/crates/greentic-runner-host/src/runner/mod.rs` — `Router::new().route(...).with_state(state)`; `ServerState` struct (~lines 77-84).
- Handler pattern: `greentic-runner/crates/greentic-runner-host/src/http/admin.rs` — `async fn handler(Guard, State(state): State<ServerState>) -> impl IntoResponse`.
- Config: `greentic-start/src/config.rs` (`DemoConfig`), `greentic-start/src/bundle_config.rs` (`load_demo_config`).
- Neither runtime crate currently depends on any DB driver.

---

## Conventions for every task

- Branch `feat/sql-gateway-runtime` in each touched repo (greentic-runner, greentic-start). Follow each repo's CLAUDE.md (branch off develop where applicable).
- TDD: write the failing test first where a test is given.
- `cargo test -p greentic-runner-host` (and `-p greentic-start`) must pass before each commit; `cargo clippy --workspace -- -D warnings` and `cargo fmt --all --check` per the repos' pre-commit norms.
- Conventional commits; no AI attribution.
- Use the **runtime `sqlx::query` API (not the compile-time `query!` macros)** so there is no `DATABASE_URL`/offline-prepared compile dependency.

---

## Task 0: Add `sqlx` dependency + empty `sql` module (compiles green)

**Files:**
- Modify: `greentic-runner/crates/greentic-runner-host/Cargo.toml`
- Create: `greentic-runner/crates/greentic-runner-host/src/sql/mod.rs`
- Modify: `greentic-runner/crates/greentic-runner-host/src/lib.rs` (add `pub mod sql;`)

- [ ] **Step 1: Read the current crate files**

`Read` `Cargo.toml`, `src/lib.rs`, `src/runner/mod.rs`, `src/http/admin.rs` to confirm the axum version, `ServerState` shape, and module layout before changing anything.

- [ ] **Step 2: Add `sqlx` to `Cargo.toml`** under `[dependencies]`:

```toml
sqlx = { version = "0.8", default-features = false, features = [
  "runtime-tokio-rustls", "sqlite", "postgres", "mysql", "chrono", "json",
] }
```

- [ ] **Step 3: Create `src/sql/mod.rs`** with the module skeleton + state type:

```rust
//! In-runtime SQL gateway: serves `/sql/<conn>/schema` + `/sql/<conn>/query`
//! over the runner's axum server, backed by read-only sqlx pools, so the
//! `greentic.sql` design extension can run text-to-data inside one bundle.

pub mod config;
pub mod execute;
pub mod introspect;
pub mod pool;
pub mod routes;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use introspect::Schema;
use pool::ConnectionPool;

/// One configured connection's live pool + engine.
pub struct SqlConnection {
    pub engine: config::Engine,
    pub pool: ConnectionPool,
}

/// Runtime state for the SQL gateway: connections, the cached schema per
/// connection, and the shared bearer token expected on `/sql` requests.
#[derive(Clone)]
pub struct SqlGateway {
    inner: Arc<SqlGatewayInner>,
}

struct SqlGatewayInner {
    connections: HashMap<String, SqlConnection>,
    schema_cache: RwLock<HashMap<String, Schema>>,
    auth_token: String,
}

impl SqlGateway {
    #[must_use]
    pub fn new(connections: HashMap<String, SqlConnection>, auth_token: String) -> Self {
        Self {
            inner: Arc::new(SqlGatewayInner {
                connections,
                schema_cache: RwLock::new(HashMap::new()),
                auth_token,
            }),
        }
    }

    #[must_use]
    pub fn connection(&self, name: &str) -> Option<&SqlConnection> {
        self.inner.connections.get(name)
    }

    #[must_use]
    pub fn check_token(&self, presented: &str) -> bool {
        // constant-time compare
        use subtle::ConstantTimeEq;
        presented.as_bytes().ct_eq(self.inner.auth_token.as_bytes()).into()
    }

    pub(crate) fn cache(&self) -> &RwLock<HashMap<String, Schema>> {
        &self.inner.schema_cache
    }
}
```

> Add `subtle = "2"` to `Cargo.toml` for the constant-time compare (or use a simple length-checked byte compare if `subtle` is undesired).

- [ ] **Step 4: Declare the module** in `src/lib.rs`: `pub mod sql;`

- [ ] **Step 5: Stub the sub-modules** so it compiles — create `config.rs`, `pool.rs`, `introspect.rs`, `execute.rs`, `routes.rs` each with the minimal types referenced above (filled in by later tasks). For now:
  - `config.rs`: `#[derive(...)] pub enum Engine { Postgres, Mysql, Sqlite }`
  - `pool.rs`: `pub enum ConnectionPool { Postgres(sqlx::PgPool), Mysql(sqlx::MySqlPool), Sqlite(sqlx::SqlitePool) }`
  - `introspect.rs`: the `Schema`/`Table`/`Column` structs (see Task 3)
  - `execute.rs`, `routes.rs`: empty for now.

- [ ] **Step 6: Build green**

Run: `cargo build -p greentic-runner-host`
Expected: `Finished`.

- [ ] **Step 7: Commit**

```bash
git commit -am "feat(sql-gateway): add sqlx dep + sql module skeleton"
```

---

## Task 1: Config types + bundle parsing (`config.rs` + greentic-start)

**Files:**
- Replace: `greentic-runner-host/src/sql/config.rs`
- Modify: `greentic-start/src/config.rs` (add `sql` to `DemoConfig`)

- [ ] **Step 1: `config.rs`** — config structs + engine enum + tests:

```rust
//! Bundle SQL config (the `sql:` block) — pure, host-testable.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Postgres,
    Mysql,
    Sqlite,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ConnectionConfig {
    pub engine: Engine,
    /// Secret URI holding the DSN/connection string for this connection.
    pub dsn_secret: String,
    #[serde(default = "default_true")]
    pub read_only: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct SqlConfig {
    /// Secret URI holding the shared bearer token the extension must present.
    pub auth_token_secret: String,
    #[serde(default)]
    pub connections: std::collections::BTreeMap<String, ConnectionConfig>,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sql_block() {
        let yaml = r#"
auth_token_secret: secret://sql/gateway_token
connections:
  analytics:
    engine: postgres
    dsn_secret: secret://sql/analytics/dsn
    read_only: true
  app:
    engine: sqlite
    dsn_secret: secret://sql/app/dsn
"#;
        let cfg: SqlConfig = serde_yaml_bw::from_str(yaml).unwrap();
        assert_eq!(cfg.connections.len(), 2);
        assert_eq!(cfg.connections["analytics"].engine, Engine::Postgres);
        assert!(cfg.connections["app"].read_only); // defaulted true
    }
}
```

> Use whatever YAML lib the runtime uses (`serde_yaml_bw` per the workspace). Adjust the test import to match.

- [ ] **Step 2: Run config test** — `cargo test -p greentic-runner-host sql::config`. Expected: PASS.

- [ ] **Step 3: Add `sql` to `greentic-start`'s `DemoConfig`**

`Read` `greentic-start/src/config.rs`, then add an optional field to the `DemoConfig` struct:
```rust
#[serde(default)]
pub sql: Option<greentic_runner_host::sql::config::SqlConfig>,
```
(or re-declare the type in greentic-start if importing the host crate's type is undesirable — match the existing dependency direction). Confirm `load_demo_config` deserializes the `sql:` block (it will, since it's part of `DemoConfig`).

- [ ] **Step 4: Commit** (in each repo as touched)

```bash
git commit -am "feat(sql-gateway): bundle sql: config types + DemoConfig field"
```

---

## Task 2: Pool builder (`pool.rs`) — read-only sqlx pools

**Files:**
- Replace: `greentic-runner-host/src/sql/pool.rs`

- [ ] **Step 1: `pool.rs`**

```rust
//! Build read-only sqlx pools per engine from a resolved DSN.

use crate::sql::config::Engine;

pub enum ConnectionPool {
    Postgres(sqlx::PgPool),
    Mysql(sqlx::MySqlPool),
    Sqlite(sqlx::SqlitePool),
}

/// Build a pool for `engine` from the resolved `dsn`. SQLite opens read-only
/// when `read_only`; Postgres/MySQL enforce read-only per-query (see execute.rs)
/// and the operator should also use a read-only DB role.
pub async fn build(engine: Engine, dsn: &str, read_only: bool) -> Result<ConnectionPool, String> {
    match engine {
        Engine::Postgres => sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(dsn)
            .await
            .map(ConnectionPool::Postgres)
            .map_err(|e| format!("postgres connect: {e}")),
        Engine::Mysql => sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(dsn)
            .await
            .map(ConnectionPool::Mysql)
            .map_err(|e| format!("mysql connect: {e}")),
        Engine::Sqlite => {
            let opts = dsn
                .parse::<sqlx::sqlite::SqliteConnectOptions>()
                .map_err(|e| format!("sqlite dsn: {e}"))?
                .read_only(read_only);
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect_with(opts)
                .await
                .map(ConnectionPool::Sqlite)
                .map_err(|e| format!("sqlite connect: {e}"))
        }
    }
}
```

- [ ] **Step 2: Build** — `cargo build -p greentic-runner-host`. Expected: `Finished`. (Pool building is exercised by the SQLite integration test in Task 5.)

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(sql-gateway): per-engine read-only sqlx pool builder"
```

---

## Task 3: Schema introspection (`introspect.rs`)

**Files:**
- Replace: `greentic-runner-host/src/sql/introspect.rs`

- [ ] **Step 1: `introspect.rs`** — `Schema` shape + per-engine introspection. Use the runtime `sqlx::query` API; map to the contract shape.

```rust
//! Per-engine schema introspection → the gateway `Schema` shape that matches the
//! greentic.sql extension's `/schema` contract.

use serde::Serialize;
use sqlx::Row;

use crate::sql::config::Engine;
use crate::sql::pool::ConnectionPool;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Schema {
    pub engine: String,
    pub tables: Vec<Table>,
}

/// Introspect tables+columns. Postgres/MySQL use information_schema (public/
/// current schema); SQLite walks sqlite_master + PRAGMA table_info.
pub async fn introspect(engine: Engine, pool: &ConnectionPool) -> Result<Schema, String> {
    let tables = match (engine, pool) {
        (Engine::Postgres, ConnectionPool::Postgres(p)) => {
            let rows = sqlx::query(
                "SELECT table_name, column_name, data_type \
                 FROM information_schema.columns \
                 WHERE table_schema = 'public' \
                 ORDER BY table_name, ordinal_position",
            )
            .fetch_all(p)
            .await
            .map_err(|e| format!("postgres introspect: {e}"))?;
            group_columns(rows.iter().map(|r| {
                (
                    r.get::<String, _>("table_name"),
                    r.get::<String, _>("column_name"),
                    r.get::<String, _>("data_type"),
                )
            }))
        }
        (Engine::Mysql, ConnectionPool::Mysql(p)) => {
            let rows = sqlx::query(
                "SELECT table_name, column_name, data_type \
                 FROM information_schema.columns \
                 WHERE table_schema = DATABASE() \
                 ORDER BY table_name, ordinal_position",
            )
            .fetch_all(p)
            .await
            .map_err(|e| format!("mysql introspect: {e}"))?;
            group_columns(rows.iter().map(|r| {
                (
                    r.get::<String, _>("table_name"),
                    r.get::<String, _>("column_name"),
                    r.get::<String, _>("data_type"),
                )
            }))
        }
        (Engine::Sqlite, ConnectionPool::Sqlite(p)) => {
            let names = sqlx::query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .fetch_all(p)
            .await
            .map_err(|e| format!("sqlite tables: {e}"))?;
            let mut tables = Vec::new();
            for row in names {
                let table: String = row.get("name");
                let cols = sqlx::query(&format!("PRAGMA table_info({table})"))
                    .fetch_all(p)
                    .await
                    .map_err(|e| format!("sqlite columns: {e}"))?;
                let columns = cols
                    .iter()
                    .map(|c| Column { name: c.get("name"), type_: c.get("type") })
                    .collect();
                tables.push(Table { name: table, columns });
            }
            tables
        }
        _ => return Err("engine/pool mismatch".to_string()),
    };
    Ok(Schema { engine: engine_str(engine).to_string(), tables })
}

fn engine_str(engine: Engine) -> &'static str {
    match engine {
        Engine::Postgres => "postgres",
        Engine::Mysql => "mysql",
        Engine::Sqlite => "sqlite",
    }
}

fn group_columns(rows: impl Iterator<Item = (String, String, String)>) -> Vec<Table> {
    let mut tables: Vec<Table> = Vec::new();
    for (table, column, type_) in rows {
        if tables.last().map(|t| &t.name) != Some(&table) {
            tables.push(Table { name: table.clone(), columns: Vec::new() });
        }
        tables.last_mut().unwrap().columns.push(Column { name: column, type_ });
    }
    tables
}
```

- [ ] **Step 2: Unit test `group_columns`** (pure, no DB):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_columns_by_table() {
        let rows = vec![
            ("users".into(), "id".into(), "integer".into()),
            ("users".into(), "email".into(), "text".into()),
            ("orders".into(), "id".into(), "integer".into()),
        ];
        let tables = group_columns(rows.into_iter());
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[0].columns.len(), 2);
    }
}
```

Run: `cargo test -p greentic-runner-host sql::introspect`. Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(sql-gateway): per-engine schema introspection"
```

---

## Task 4: Query execution + limits (`execute.rs`)

**Files:**
- Replace: `greentic-runner-host/src/sql/execute.rs`

- [ ] **Step 1: `execute.rs`** — run a query read-only with `max_rows`/`truncated`, map rows → JSON. Use a read-only transaction for PG/MySQL.

```rust
//! Read-only query execution with row limit + JSON mapping.

use serde::Serialize;
use serde_json::Value;
use sqlx::{Column as _, Row, TypeInfo};

use crate::sql::config::Engine;
use crate::sql::pool::ConnectionPool;

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: u64,
    pub truncated: bool,
}

/// Execute `sql` read-only, returning up to `max_rows` rows (+`truncated` if
/// more existed). Fetches `max_rows + 1` to detect truncation.
pub async fn run(
    engine: Engine,
    pool: &ConnectionPool,
    sql: &str,
    max_rows: u32,
) -> Result<QueryResult, String> {
    let cap = u64::from(max_rows);
    match (engine, pool) {
        (Engine::Sqlite, ConnectionPool::Sqlite(p)) => {
            let rows = sqlx::query(sql).fetch_all(p).await.map_err(map_err)?;
            map_rows_sqlite(&rows, cap)
        }
        (Engine::Postgres, ConnectionPool::Postgres(p)) => {
            let mut tx = p.begin().await.map_err(map_err)?;
            sqlx::query("SET TRANSACTION READ ONLY").execute(&mut *tx).await.map_err(map_err)?;
            let rows = sqlx::query(sql).fetch_all(&mut *tx).await.map_err(map_err)?;
            let out = map_rows_pg(&rows, cap);
            tx.rollback().await.ok();
            out
        }
        (Engine::Mysql, ConnectionPool::Mysql(p)) => {
            let mut tx = p.begin().await.map_err(map_err)?;
            sqlx::query("SET TRANSACTION READ ONLY").execute(&mut *tx).await.map_err(map_err)?;
            let rows = sqlx::query(sql).fetch_all(&mut *tx).await.map_err(map_err)?;
            let out = map_rows_mysql(&rows, cap);
            tx.rollback().await.ok();
            out
        }
        _ => Err("engine/pool mismatch".to_string()),
    }
}

fn map_err(e: sqlx::Error) -> String {
    // sqlx error messages do not include the connection string.
    format!("query error: {e}")
}

// NOTE: sqlx value->JSON conversion is engine-specific and verbose. The
// implementer should implement `map_rows_sqlite/pg/mysql` using each driver's
// Row/Column/ValueRef APIs, applying the `cap`/`truncated` logic below. A
// reasonable generic approach: try common types (i64, f64, bool, String) per
// column, falling back to NULL/string. Pure helper `apply_cap` is tested.

/// Drop rows beyond `cap`, return (kept_rows, truncated).
fn apply_cap<T>(mut rows: Vec<T>, cap: u64) -> (Vec<T>, bool) {
    let truncated = rows.len() as u64 > cap;
    if truncated {
        rows.truncate(cap as usize);
    }
    (rows, truncated)
}
```

> The plan deliberately stops at the engine-specific value→JSON mapping: it is mechanical but driver-specific (sqlx `ValueRef`/`decode`), and must be written against the live sqlx version. Implement `map_rows_*` to produce `Vec<Vec<Value>>` + `columns`, then call `apply_cap`. Keep `apply_cap` and column-name extraction unit-tested.

- [ ] **Step 2: Unit test `apply_cap`**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cap_truncates_and_flags() {
        let (kept, trunc) = apply_cap(vec![1, 2, 3], 2);
        assert_eq!(kept, vec![1, 2]);
        assert!(trunc);
        let (kept, trunc) = apply_cap(vec![1, 2], 5);
        assert_eq!(kept.len(), 2);
        assert!(!trunc);
    }
}
```

Run: `cargo test -p greentic-runner-host sql::execute`. Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(sql-gateway): read-only query execution + row-cap"
```

---

## Task 5: Routes + auth guard (`routes.rs`) + SQLite integration test

**Files:**
- Replace: `greentic-runner-host/src/sql/routes.rs`
- Test: `greentic-runner-host/tests/sql_gateway.rs` (or inline)

- [ ] **Step 1: `routes.rs`** — axum handlers + bearer guard, using `SqlGateway` from `ServerState`.

```rust
//! axum handlers for `/sql/<conn>/schema` + `/sql/<conn>/query`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::sql::{execute, introspect};

#[derive(Deserialize)]
pub struct QueryBody {
    pub sql: String,
    #[serde(default = "default_max_rows")]
    pub max_rows: u32,
}

fn default_max_rows() -> u32 {
    100
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}

/// Generic over the concrete state type so the handler can pull the `SqlGateway`.
/// Wire these in runner/mod.rs against the real `ServerState` (which must expose
/// the `SqlGateway`, added in Task 6).
pub async fn schema_handler(
    State(gateway): State<crate::sql::SqlGateway>,
    headers: HeaderMap,
    Path(conn): Path<String>,
) -> impl IntoResponse {
    if !auth_ok(&gateway, &headers) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"unauthorized"}))).into_response();
    }
    let Some(connection) = gateway.connection(&conn) else {
        return (StatusCode::NOT_FOUND, Json(json!({"error":"unknown connection"}))).into_response();
    };
    // serve from cache, else introspect + cache
    {
        let cache = gateway.cache().read().await;
        if let Some(schema) = cache.get(&conn) {
            return (StatusCode::OK, Json(serde_json::to_value(schema).unwrap())).into_response();
        }
    }
    match introspect::introspect(connection.engine, &connection.pool).await {
        Ok(schema) => {
            gateway.cache().write().await.insert(conn.clone(), schema.clone());
            (StatusCode::OK, Json(serde_json::to_value(&schema).unwrap())).into_response()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({"error": e}))).into_response(),
    }
}

pub async fn query_handler(
    State(gateway): State<crate::sql::SqlGateway>,
    headers: HeaderMap,
    Path(conn): Path<String>,
    Json(body): Json<QueryBody>,
) -> impl IntoResponse {
    if !auth_ok(&gateway, &headers) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"unauthorized"}))).into_response();
    }
    let Some(connection) = gateway.connection(&conn) else {
        return (StatusCode::NOT_FOUND, Json(json!({"error":"unknown connection"}))).into_response();
    };
    let max_rows = body.max_rows.clamp(1, 1000);
    match execute::run(connection.engine, &connection.pool, &body.sql, max_rows).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(&result).unwrap())).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({"error": e}))).into_response(),
    }
}

fn auth_ok(gateway: &crate::sql::SqlGateway, headers: &HeaderMap) -> bool {
    bearer(headers).is_some_and(|t| gateway.check_token(&t))
}
```

> If `ServerState` is a single struct (not the extractable `SqlGateway` directly), implement `FromRef<ServerState> for SqlGateway` so `State<SqlGateway>` works, OR change the handler to `State<ServerState>` and pull `state.sql`. Decide against the live `ServerState` in Task 6.

- [ ] **Step 2: SQLite end-to-end integration test** (no external server): create a temp SQLite file, seed a table, build a `SqlGateway` with one sqlite connection, drive `schema_handler` + `query_handler` (via `axum::Router` + `tower::ServiceExt::oneshot`, matching how other tests in the crate exercise routes — `Read` an existing test first). Assert `/schema` returns the table and `/query` returns rows; assert a wrong/missing token returns 401 and an unknown connection returns 404.

- [ ] **Step 3: Run** — `cargo test -p greentic-runner-host sql`. Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(sql-gateway): /schema + /query routes, bearer auth, sqlite e2e test"
```

---

## Task 6: Wire routes + state into the runner (`runner/mod.rs`)

**Files:**
- Modify: `greentic-runner-host/src/runner/mod.rs`

- [ ] **Step 1: Read `runner/mod.rs`** — confirm the current `Router` builder, `ServerState` fields, and how state is constructed/passed.

- [ ] **Step 2: Add the `SqlGateway` to `ServerState`** (an `Option<SqlGateway>` if the gateway may be absent) and implement `axum::extract::FromRef<ServerState> for SqlGateway` (or route via `State<ServerState>`).

- [ ] **Step 3: Register the routes** in the router builder:
```rust
.route("/sql/{conn}/schema", get(crate::sql::routes::schema_handler))
.route("/sql/{conn}/query", post(crate::sql::routes::query_handler))
```
(axum 0.8 path syntax is `{conn}`; verify against the existing routes' syntax.)

- [ ] **Step 4: Build + run the crate tests**
Run: `cargo test -p greentic-runner-host` and `cargo clippy -p greentic-runner-host -- -D warnings`. Expected: PASS, zero warnings.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(sql-gateway): register /sql routes + SqlGateway state in runner"
```

---

## Task 7: Build pools at startup from bundle config + secrets (greentic-start)

**Files:**
- Modify: `greentic-start/src/runtime.rs` (or wherever the host is constructed) + a small builder.

- [ ] **Step 1: Read the startup path** — `greentic-start/src/lib.rs` (`run_start`), `src/runtime.rs`, `src/bundle_config.rs` — find where `DemoConfig` is loaded, where secrets are resolved, and where the runner-host (`ServerState`) is built.

- [ ] **Step 2: Build the `SqlGateway`** before constructing the host: if `demo_config.sql` is `Some`, resolve `auth_token_secret` and each connection's `dsn_secret` via the existing secrets resolver, then `sql::pool::build(...)` per connection, collect into the `HashMap<String, SqlConnection>`, and `SqlGateway::new(map, token)`. Pass it into the host/`ServerState` (the field added in Task 6). Log how many connections were wired; on a connection build failure, log and skip that connection (don't crash startup).

- [ ] **Step 3: Build + smoke**
Run: `cargo build -p greentic-start`. Then a manual smoke: a tiny bundle `greentic.demo.yaml` with a `sql:` block pointing at a temp SQLite DB + the secrets, `gtc start`, and `curl -H "Authorization: Bearer <token>" localhost:<port>/sql/<conn>/schema`. (Document the smoke; it is manual.)

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(sql-gateway): build pools from bundle sql: config at startup"
```

---

## Task 8: Docs + network-allowlist note

- [ ] **Step 1: Document** in the bundle docs / a README: the `sql:` config block, the read-only role requirement, and that the `greentic.sql` extension's `secret://sql/connections` `base_url` must point at `http://<runner-host>:<port>/sql/<conn>` with the shared token at `secret://sql/<name>/token`. Note the **network allow-list**: the extension's `permissions.network` (or a runtime host-override) must permit the runner's own `/sql` host — confirm whether the runner can auto-allow its own endpoint.

- [ ] **Step 2: Commit**

```bash
git commit -am "docs(sql-gateway): bundle config + extension wiring + network note"
```

---

## Out of scope / follow-ups

- Engine value→JSON mapping is implemented per-driver in Task 4 against the live sqlx version (the plan tests the pure cap/grouping helpers; full row mapping is mechanical driver code).
- Postgres/MySQL integration tests behind docker-compose (greentic-integration already runs Postgres) — gated, not in the default unit suite.
- Per-tenant connection sets; schema cache manual-refresh endpoint; auto-deriving the extension registry from the bundle `sql:` block.
- Wiring the chat2data **flow** path (`requires_runtime_execution`) to reuse `/sql/<conn>/query`.
