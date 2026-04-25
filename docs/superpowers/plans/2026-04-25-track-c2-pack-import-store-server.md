# Track C2 — Store-Server `.gtpack` Artifact Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `.gtpack` artifact upload + download + metadata endpoints to `greentic-store-server`, mirroring existing `.gtxpack` handling.

**Architecture:** Mirror the structure of `crates/greentic-store-api/src/handlers/extensions.rs` for `.gtpack`. Reuse `greentic-store-blob` for storage with a new path namespace `packs/<publisher>/<name>/<version>.gtpack`. Reuse existing publisher auth.

**Tech Stack:** Rust (match repo `rust-toolchain.toml`), axum, sqlx (matches existing crate setup).

**Spec:** `greentic-designer-extensions/docs/superpowers/specs/2026-04-25-designer-commercialization-backend-design.md` (Track C — store-server side)

**Branch / Worktree:**
```
git worktree add ~/works/greentic/gss-gtpack -b feat/gtpack-artifact-type main
```
PR target: `main` (verify default branch; some repos use `develop`).

---

## File Structure

### Create

- `crates/greentic-store-api/src/handlers/packs.rs` (~250 LOC) — handlers (publish, list, detail, version_metadata, download)
- `crates/greentic-store-db/src/packs.rs` (~150 LOC) — DB queries
- `crates/greentic-store-core/src/pack.rs` (~80 LOC) — domain type `Pack`
- `migrations/<timestamp>_add_packs_table.sql` (~60 LOC) — schema
- `crates/greentic-store-api/tests/packs_flow.rs` (~200 LOC) — integration tests mirroring `extensions_flow`

### Modify

- `crates/greentic-store-api/src/router.rs` — register `/api/v1/packs/...` routes
- `crates/greentic-store-api/src/handlers/mod.rs` — `pub mod packs;`
- `crates/greentic-store-db/src/lib.rs` — `pub mod packs;`
- `crates/greentic-store-core/src/lib.rs` — `pub mod pack;`
- `crates/greentic-store-blob/src/lib.rs` — verify path namespacing supports `packs/...` (likely already generic; otherwise extend)
- `openapi/openapi.yaml` (or wherever the OpenAPI spec lives) — add `/packs/...` endpoints
- `README.md` — document new endpoints

---

## Task 1: Database migration — `packs` table

**Files:**
- Create: `migrations/<timestamp>_add_packs_table.sql`

- [ ] **Step 1: Generate migration filename**

```bash
NAME="add_packs_table"
TS=$(date +%Y%m%d%H%M%S)
touch "migrations/${TS}_${NAME}.sql"
```

- [ ] **Step 2: Write migration**

Mirror the structure of the `extensions` table. Inspect existing extension migration first:

```bash
grep -l "CREATE TABLE extensions" migrations/
```

Then create the new migration with the same shape adapted for packs:

```sql
CREATE TABLE IF NOT EXISTS packs (
    id              BIGSERIAL PRIMARY KEY,
    publisher_id    BIGINT NOT NULL REFERENCES publishers(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    version         TEXT NOT NULL,
    sha256          TEXT NOT NULL,
    size_bytes      BIGINT NOT NULL,
    blob_key        TEXT NOT NULL,
    metadata_json   JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (publisher_id, name, version)
);

CREATE INDEX idx_packs_name ON packs (name);
CREATE INDEX idx_packs_created_at ON packs (created_at DESC);
```

(Verify `publishers` table column types via `migrations/` grep.)

- [ ] **Step 3: Run migration locally**

Run: `sqlx migrate run` (or repo's standard test-DB setup).
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add migrations/
git commit -m "feat(db): add packs table"
```

---

## Task 2: Domain type `Pack`

**Files:**
- Create: `crates/greentic-store-core/src/pack.rs`
- Modify: `crates/greentic-store-core/src/lib.rs`

- [ ] **Step 1: Add module**

`crates/greentic-store-core/src/pack.rs`:

```rust
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pack {
    pub id: i64,
    pub publisher_id: i64,
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub blob_key: String,
    pub metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSummary {
    pub publisher: String,
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub created_at: OffsetDateTime,
}
```

(Match existing `Extension` struct shape in `crates/greentic-store-core/src/extension.rs` for consistency.)

- [ ] **Step 2: Re-export**

`crates/greentic-store-core/src/lib.rs`:

```rust
pub mod pack;
pub use pack::{Pack, PackSummary};
```

- [ ] **Step 3: Build + commit**

Run: `cargo build -p greentic-store-core`
Expected: success.

```bash
git add crates/greentic-store-core/
git commit -m "feat(core): add Pack + PackSummary domain types"
```

---

## Task 3: DB layer queries

**Files:**
- Create: `crates/greentic-store-db/src/packs.rs`
- Modify: `crates/greentic-store-db/src/lib.rs`

- [ ] **Step 1: Add queries**

`crates/greentic-store-db/src/packs.rs`:

```rust
use greentic_store_core::Pack;
use sqlx::PgPool;

pub async fn insert_pack(
    pool: &PgPool,
    publisher_id: i64,
    name: &str,
    version: &str,
    sha256: &str,
    size_bytes: i64,
    blob_key: &str,
    metadata: &serde_json::Value,
) -> Result<Pack, sqlx::Error> {
    sqlx::query_as!(
        Pack,
        r#"
        INSERT INTO packs (publisher_id, name, version, sha256, size_bytes, blob_key, metadata_json)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, publisher_id, name, version, sha256, size_bytes, blob_key,
                  metadata_json AS "metadata: serde_json::Value",
                  created_at
        "#,
        publisher_id, name, version, sha256, size_bytes, blob_key, metadata
    )
    .fetch_one(pool)
    .await
}

pub async fn get_by_publisher_name_version(
    pool: &PgPool,
    publisher: &str,
    name: &str,
    version: &str,
) -> Result<Option<Pack>, sqlx::Error> {
    sqlx::query_as!(
        Pack,
        r#"
        SELECT p.id, p.publisher_id, p.name, p.version, p.sha256, p.size_bytes, p.blob_key,
               p.metadata_json AS "metadata: serde_json::Value",
               p.created_at
        FROM packs p
        JOIN publishers pub ON pub.id = p.publisher_id
        WHERE pub.handle = $1 AND p.name = $2 AND p.version = $3
        "#,
        publisher, name, version
    )
    .fetch_optional(pool)
    .await
}

pub async fn list_packs(pool: &PgPool, limit: i64) -> Result<Vec<Pack>, sqlx::Error> {
    sqlx::query_as!(
        Pack,
        r#"
        SELECT id, publisher_id, name, version, sha256, size_bytes, blob_key,
               metadata_json AS "metadata: serde_json::Value",
               created_at
        FROM packs
        ORDER BY created_at DESC
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await
}
```

- [ ] **Step 2: Re-export**

`crates/greentic-store-db/src/lib.rs`:

```rust
pub mod packs;
```

- [ ] **Step 3: DB integration test**

Mirror `crates/greentic-store-db/tests/extensions.rs`. Create `crates/greentic-store-db/tests/packs.rs`:

```rust
// Use the same test fixtures and DB setup pattern as extensions.rs
// Tests: insert_pack roundtrip, get_by_publisher_name_version found/missing,
// list_packs ordering and limit.
// Implementation pattern is identical to extensions.rs — copy the structure
// and substitute pack-specific fields.
```

- [ ] **Step 4: Run + commit**

Run: `cargo test -p greentic-store-db packs`
Expected: PASS (requires test DB; follow repo setup).

```bash
git add crates/greentic-store-db/
git commit -m "feat(db): pack queries (insert, get, list)"
```

---

## Task 4: Handler — publish (upload `.gtpack`)

**Files:**
- Create: `crates/greentic-store-api/src/handlers/packs.rs`
- Modify: `crates/greentic-store-api/src/handlers/mod.rs`

- [ ] **Step 1: Write the handler**

`crates/greentic-store-api/src/handlers/packs.rs`:

```rust
use axum::{
    Json,
    extract::{Path, State, Multipart},
    http::StatusCode,
    response::IntoResponse,
};
use sha2::{Digest, Sha256};

use crate::handlers::auth::AuthenticatedPublisher;
use crate::state::AppState;

const MAX_PACK_BYTES: usize = 100 * 1024 * 1024;
const PACK_BLOB_PREFIX: &str = "packs";

pub async fn publish(
    State(state): State<AppState>,
    auth: AuthenticatedPublisher,
    Path((name, version)): Path<(String, String)>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut bytes: Option<Vec<u8>> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("artifact") {
            match field.bytes().await {
                Ok(data) if data.len() <= MAX_PACK_BYTES => bytes = Some(data.to_vec()),
                Ok(_) => return (StatusCode::PAYLOAD_TOO_LARGE,
                    Json(serde_json::json!({"error": "pack exceeds 100 MB"}))).into_response(),
                Err(e) => return (StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e.to_string()}))).into_response(),
            }
        }
    }
    let Some(bytes) = bytes else {
        return (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing artifact field"}))).into_response();
    };

    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let blob_key = format!("{}/{}/{}/{}.gtpack",
        PACK_BLOB_PREFIX, auth.publisher.handle, name, version);

    if let Err(e) = state.blob.put(&blob_key, &bytes).await {
        return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("blob: {e}")}))).into_response();
    }

    match greentic_store_db::packs::insert_pack(
        &state.pool, auth.publisher.id, &name, &version, &sha256,
        bytes.len() as i64, &blob_key, &serde_json::json!({}),
    ).await {
        Ok(pack) => (StatusCode::CREATED, Json(pack)).into_response(),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            (StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "pack version already exists"}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}
```

(Adapt `AuthenticatedPublisher` extractor name to match existing `extensions.rs`.)

- [ ] **Step 2: Re-export module**

`crates/greentic-store-api/src/handlers/mod.rs`:

```rust
pub mod packs;
```

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-store-api/
git commit -m "feat(api): pack publish handler with sha256 + blob storage"
```

---

## Task 5: Handler — download + metadata + list

**Files:**
- Modify: `crates/greentic-store-api/src/handlers/packs.rs`

- [ ] **Step 1: Add handlers**

```rust
use axum::body::Body;

pub async fn download(
    State(state): State<AppState>,
    Path((publisher, name, version)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let pack = match greentic_store_db::packs::get_by_publisher_name_version(
        &state.pool, &publisher, &name, &version
    ).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "pack not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    match state.blob.get(&pack.blob_key).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                ("content-type", "application/zip"),
                ("content-length", &bytes.len().to_string()),
            ],
            Body::from(bytes),
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("blob: {e}")}))).into_response(),
    }
}

pub async fn version_metadata(
    State(state): State<AppState>,
    Path((publisher, name, version)): Path<(String, String, String)>,
) -> impl IntoResponse {
    match greentic_store_db::packs::get_by_publisher_name_version(
        &state.pool, &publisher, &name, &version
    ).await {
        Ok(Some(pack)) => (StatusCode::OK, Json(pack)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "pack not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match greentic_store_db::packs::list_packs(&state.pool, 100).await {
        Ok(packs) => (StatusCode::OK, Json(packs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-store-api/
git commit -m "feat(api): pack download + metadata + list handlers"
```

---

## Task 6: Wire routes

**Files:**
- Modify: `crates/greentic-store-api/src/router.rs`

- [ ] **Step 1: Register routes**

In `build_router`, add after the `/extensions` block:

```rust
.route(
    "/api/v1/packs/{publisher}/{name}/{version}",
    axum::routing::post(handlers::packs::publish)
        .layer(body_limit.clone()),
)
.route(
    "/api/v1/packs/{publisher}/{name}/{version}/download",
    get(handlers::packs::download),
)
.route(
    "/api/v1/packs/{publisher}/{name}/{version}/metadata",
    get(handlers::packs::version_metadata),
)
.route(
    "/api/v1/packs",
    get(handlers::packs::list),
)
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-store-api/src/router.rs
git commit -m "feat(api): register /api/v1/packs routes"
```

---

## Task 7: Integration tests

**Files:**
- Create: `crates/greentic-store-api/tests/packs_flow.rs`

- [ ] **Step 1: Mirror `publish_flow.rs` for packs**

Copy the test scaffolding from `crates/greentic-store-api/tests/publish_flow.rs` and substitute pack endpoints. Critical scenarios:

```rust
#[tokio::test]
async fn publish_then_download_roundtrip() {
    let (app, _ctx) = setup_test_app().await;
    let token = create_test_token(&_ctx, "testpub").await;

    let pack_bytes = b"PK\x03\x04...minimal zip...";

    // POST /api/v1/packs/testpub/my-pack/1.0.0
    let response = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/packs/testpub/my-pack/1.0.0")
            .header("authorization", format!("Bearer {}", token))
            .header("content-type", "multipart/form-data; boundary=----test")
            .body(Body::from(build_multipart(b"artifact", "x.gtpack", pack_bytes, "----test")))
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // GET /api/v1/packs/testpub/my-pack/1.0.0/download
    let download = app.clone().oneshot(
        Request::builder()
            .uri("/api/v1/packs/testpub/my-pack/1.0.0/download")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    let body = axum::body::to_bytes(download.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(&body[..], pack_bytes);
}

#[tokio::test]
async fn duplicate_version_returns_409() {
    // ... publish twice, second returns CONFLICT ...
}

#[tokio::test]
async fn unauthenticated_publish_returns_401() {
    // ... omit Authorization header ...
}

#[tokio::test]
async fn metadata_returns_pack_fields() {
    // ... publish, then GET /metadata, verify sha256 + size + name ...
}
```

(`build_multipart` and `setup_test_app` helpers exist in the existing test scaffolding; reuse them.)

- [ ] **Step 2: Run + commit**

Run: `cargo test -p greentic-store-api --test packs_flow`
Expected: PASS.

```bash
git add crates/greentic-store-api/tests/packs_flow.rs
git commit -m "test(api): pack publish/download/metadata flow"
```

---

## Task 8: OpenAPI spec update

**Files:**
- Modify: `openapi/openapi.yaml` (verify path; might be under `crates/greentic-store-api/openapi/`)

- [ ] **Step 1: Add pack endpoints**

Find existing `/extensions` paths in the spec and add parallel entries:

```yaml
  /api/v1/packs/{publisher}/{name}/{version}:
    post:
      summary: Publish a .gtpack
      security:
        - bearerAuth: []
      parameters:
        - { name: publisher, in: path, required: true, schema: { type: string } }
        - { name: name,      in: path, required: true, schema: { type: string } }
        - { name: version,   in: path, required: true, schema: { type: string } }
      requestBody:
        required: true
        content:
          multipart/form-data:
            schema:
              type: object
              properties:
                artifact: { type: string, format: binary }
      responses:
        '201': { $ref: '#/components/responses/PackCreated' }
        '409': { description: pack version already exists }

  /api/v1/packs/{publisher}/{name}/{version}/download:
    get:
      summary: Download a .gtpack
      parameters: [ ... same path params ... ]
      responses:
        '200':
          content:
            application/zip:
              schema: { type: string, format: binary }
        '404': { description: not found }

  /api/v1/packs/{publisher}/{name}/{version}/metadata:
    get:
      summary: Pack metadata
      responses:
        '200': { $ref: '#/components/responses/PackMetadata' }
        '404': { description: not found }

  /api/v1/packs:
    get:
      summary: List packs
      responses:
        '200':
          content:
            application/json:
              schema:
                type: array
                items: { $ref: '#/components/schemas/PackSummary' }
```

Also add `PackSummary` and `PackCreated` to `components.schemas` / `components.responses`.

- [ ] **Step 2: Verify OpenAPI lints**

If repo has `make lint-openapi` or similar:
Run: that command.
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add openapi/
git commit -m "docs(openapi): add /api/v1/packs/* endpoints"
```

---

## Task 9: README update

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add section**

```markdown
## Pack artifacts

In addition to extensions (`.gtxpack`), the store hosts application packs
(`.gtpack`) used by the greentic-designer pack import endpoint.

- `POST /api/v1/packs/{publisher}/{name}/{version}` — publish (auth required)
- `GET  /api/v1/packs/{publisher}/{name}/{version}/download`
- `GET  /api/v1/packs/{publisher}/{name}/{version}/metadata`
- `GET  /api/v1/packs` — list

Packs are stored in MinIO under `packs/{publisher}/{name}/{version}.gtpack`.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document /api/v1/packs/* endpoints"
```

---

## Task 10: CI + PR

- [ ] **Step 1: Run local CI**

Run: `ci/local_check.sh`
Expected: PASS (requires test DB + MinIO; follow `DEPLOY.md` for setup).

- [ ] **Step 2: Push branch**

```bash
git push -u origin feat/gtpack-artifact-type
```

- [ ] **Step 3: Open PR**

```bash
gh pr create --title "feat: .gtpack artifact type alongside .gtxpack" \
  --base main \
  --body "$(cat <<'EOF'
## Summary

- New `packs` table + DB queries (mirrors `extensions`)
- New `Pack` + `PackSummary` core types
- New handlers: publish (multipart), download, metadata, list
- New routes under `/api/v1/packs/...`
- Blob storage namespace `packs/{publisher}/{name}/{version}.gtpack`
- OpenAPI spec updated

## Test plan

- [x] Publish + download roundtrip preserves bytes + sha256
- [x] Duplicate version returns 409
- [x] Unauthenticated publish returns 401
- [x] Metadata returns publisher/name/version/sha256/size

## Backwards compatibility

Purely additive. Existing `.gtxpack` endpoints unchanged.

Spec: `greentic-designer-extensions/docs/superpowers/specs/2026-04-25-designer-commercialization-backend-design.md` (Track C — store-server side)
Companion PR (designer side): `feat/pack-import-backend` in `greentic-designer`
EOF
)"
```

---

## Self-review checklist

- [x] Migration adds `packs` table (Task 1)
- [x] Domain `Pack` + `PackSummary` (Task 2)
- [x] DB queries: insert, get_by_..., list (Task 3)
- [x] Publish handler: auth + multipart + sha256 + 100 MB cap + blob put (Task 4)
- [x] Download / metadata / list handlers (Task 5)
- [x] Routes registered (Task 6)
- [x] Integration tests for roundtrip + 409 + auth (Task 7)
- [x] OpenAPI spec updated (Task 8)
- [x] README updated (Task 9)
- [x] All identifiers consistent (`Pack`, `PackSummary`, `PACK_BLOB_PREFIX`, `MAX_PACK_BYTES`)
- [x] No "TBD" / placeholder steps; explicit instructions where existing helpers must be located/reused
