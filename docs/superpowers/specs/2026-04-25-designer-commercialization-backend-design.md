# Designer Commercialization Backend — Design

**Date:** 2026-04-25
**Status:** Draft (pending user review)
**Scope:** Backend-only. UI deferred to UI v2 phase.

## Context

Maarten and Bima aligned 2026-04-25 on the order of work to commercialize `greentic-designer`. Four original requirements (bundle download, any-component-included, app-pack-loader, demo-mode + designer-admin) were re-prioritized into:

1. Generic WASM-component extension authoring + extension lifecycle (enable/disable/uninstall)
2. Pack import / preload (start from existing asset; deep-link deferred)
3. **Deferred:** auto-discovery, proper demo-mode, designer-admin

This spec covers items (1) and (2), backend only. Frontend work waits until UI v2 lands so we don't build twice.

The existing extension system (`greentic-designer-extensions` v0.10.x) already ships:

- `describe.json` contract with `nodeTypes` + `runtime.gtpack` + `configSchema` + permissions
- `gtdx` CLI with `install/uninstall/list/info/search/new/dev/publish/sign/verify` (16 subcommands)
- `greentic-ext-runtime` discovery + WASM loader + `invoke_tool()` host
- Designer boot wiring that scans `~/.greentic/extensions/{design,deploy}/` and registers contributed `nodeTypes`
- Reference node-providing extension `greentic.llm-openai` (separate repo)

What is missing:

- A scaffold flavor for "WASM-component-as-node" authoring so Vahe / Osoro can produce `greentic-x` / `telco-x` / `retail-x` / `greentic-dw` / `greentic-sorla` extensions without bespoke wiring
- Enable / disable lifecycle ops (install / uninstall already shipped)
- Hot-reload mechanism so designer reflects state changes without restart
- Pack import endpoint so a `.gtpack` from upload or catalog can be loaded into the designer canvas
- `.gtpack` artifact type on `greentic-store-server` (today only `.gtxpack` is hosted)

## Goals

- Vahe / Osoro can scaffold a node-providing WASM extension and publish it within ~one developer day
- Operators can toggle extensions on/off via CLI without restarting the designer
- Designer hot-reloads palette + active extension list within ~one second of a state change
- A user can import a `.gtpack` (uploaded file or catalog ref) into the designer as a starting point
- Forward-compatible state model so the future designer-admin track can layer per-tenant policy without schema migration

## Non-goals

- Any UI work (panels, dialogs, frontend wiring) — deferred to a follow-up phase post-UI-v2
- Per-tenant enforcement at runtime (data model is forward-compatible; behavior is global per machine)
- Auto-discovery of arbitrary `.gtpack` components as palette nodes without an extension wrapper
- Proper demo-mode (mock fixtures, LLM-generated sample data) — manual workaround via `gtdx disable` is acceptable for MVP
- Designer-admin (RBAC, approval flow, multi-tenant policy)
- Cascade dependency resolution when disabling an extension other extensions require (warn only)
- `gtdx import-pack` CLI command
- Pack export from designer
- Auto-cleanup of imported packs

## Approach

Three parallel tracks behind two upfront sync points.

### Tracks

| Track | Repo(s) | Effort |
|-------|---------|--------|
| A — Scaffold (`gtdx new --kind wasm-component`) | `greentic-designer-extensions` | ~3 days |
| B — Lifecycle (CLI + state lib + designer boot filter + hot-reload + API) | `greentic-designer-extensions`, `greentic-designer` | ~8.5 days |
| C — Pack import (designer endpoint + validation + `.gtpack` on store-server) | `greentic-designer`, `greentic-store-server` | ~8 days |

Total: ~6 days wall-clock if all three tracks run in parallel (longest path = Track B designer backend); ~20 days sequential.

### Sync points (locked in this spec)

1. **State file schema v1.0** (Section: Data Model) — Track A reads, Track B reads + writes
2. **Trust policy abstraction** (Section: Track C — Trust Policy) — refactored out of existing `gtdx install`; consumed by Track B install path and Track C import path

`describe.json` schema requires no bump; existing v1 fields cover Track A's needs.

### Branch strategy

All work uses git worktrees so multiple agents can work in parallel without filesystem clash with the in-flight UI v2 branch in `greentic-designer`.

| Repo | Worktree path | Branch | Branch off | PR target |
|------|----------------|--------|-------------|-----------|
| `greentic-designer-extensions` | `~/works/greentic/gde-scaffold/` | `feat/scaffold-wasm-component` | `main` | `main` |
| `greentic-designer-extensions` | `~/works/greentic/gde-lifecycle/` | `feat/extension-lifecycle-cli` | `main` | `main` |
| `greentic-designer` | `~/works/greentic/gd-lifecycle/` | `feat/extension-lifecycle-backend` | `develop` | `develop` |
| `greentic-designer` | `~/works/greentic/gd-pack-import/` | `feat/pack-import-backend` | `develop` | `develop` |
| `greentic-store-server` | `~/works/greentic/gss-gtpack/` | `feat/gtpack-artifact-type` | `main` | `main` |

PRs in `greentic-designer` target `develop` (not `main`) per project convention.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ ~/.greentic/                                                        │
│   ├── extensions/                                                   │
│   │   ├── design/<id>-<version>/   (existing — installed exts)      │
│   │   ├── deploy/<id>-<version>/                                    │
│   │   └── ...                                                       │
│   ├── extensions-state.json        (NEW — Track B)                  │
│   ├── designer/                                                     │
│   │   └── imported-packs/<id>-<version>/  (NEW — Track C)           │
│   ├── registries.toml              (existing)                       │
│   └── runtime/packs/                (existing — runner pickup)      │
└─────────────────────────────────────────────────────────────────────┘

┌────────────────┐    ┌──────────────────────────┐    ┌──────────────┐
│ gtdx CLI       │    │ greentic-ext-runtime     │    │ designer     │
│ ─────────────  │    │ ───────────────────────  │    │ ──────────── │
│ new (Track A)  │───>│ discovery::scan_kind_dir │<───│ boot loads   │
│ enable (B)     │───>│ watch() (NEW — Track B)  │───>│ filters by   │
│ disable (B)    │    │   emits RuntimeEvent     │    │ state + SSE  │
│ install (exist)│    │                          │    │              │
└────────────────┘    │ greentic-ext-state (NEW) │    │ /api/...     │
                      │ greentic-trust (NEW)     │    │ (Tracks B+C) │
                      └──────────────────────────┘    └──────────────┘
                                                              │
                                            ┌─────────────────┴────────────┐
                                            │                              │
                                    catalog ref (Track C)         multipart upload
                                            │                              │
                                            v                              v
                                ┌──────────────────────┐       ┌────────────────┐
                                │ greentic-store-server│       │ disk           │
                                │ + .gtpack support    │       │ imported-packs/│
                                │ (Track C)            │       │                │
                                └──────────────────────┘       └────────────────┘
```

## Data Model — Extension State File

**Location:** `~/.greentic/extensions-state.json`

**Schema v1.0:**

```json
{
  "schema": "1.0",
  "default": {
    "enabled": {
      "greentic.llm-openai@0.1.0": true,
      "greentic.adaptive-cards@1.6.0": false
    }
  },
  "tenants": {}
}
```

**Rules:**

1. Key format: `<extension-id>@<version>`. Multiple versions of the same extension carry independent enable state.
2. Missing key implies `enabled: true`. First boot after upgrade has no state file → all installed extensions enabled. No migration script needed.
3. `tenants` is always empty in MVP. Reserved for the future designer-admin track. Reader code ignores this field.
4. `schema` field uses semver. Reader on unknown major version: log warning, treat as default state (all enabled), do not delete.
5. Atomic write: write to `extensions-state.json.tmp`, fsync, rename to `extensions-state.json`. Reader sees full snapshot or none.
6. Concurrent access: advisory file lock via `fs2` crate on write. Lock contention: 3 retries with backoff, then error. Read does not lock.
7. Corrupt-file recovery: on parse error, log error, fall back to empty state (all enabled). Do not delete or overwrite. User intervenes manually.

**Library:** new crate `greentic-ext-state` in `greentic-designer-extensions` workspace. Consumed by `gtdx` CLI (Track B commands) and `greentic-ext-runtime` (Track B boot filter + watcher).

**Public API:**

```rust
pub struct ExtensionState { /* private */ }

impl ExtensionState {
    pub fn load(home: &Path) -> Result<Self, StateError>;
    pub fn is_enabled(&self, ext_id: &str, version: &str) -> bool;
    pub fn set_enabled(&mut self, ext_id: &str, version: &str, enabled: bool);
    pub fn save_atomic(&self, home: &Path) -> Result<(), StateError>;
}
```

## Track A — Scaffold (`gtdx new --kind wasm-component`)

**Goal:** authors produce a node-providing WASM extension scaffold by running one command.

**CLI:**

```
gtdx new --kind wasm-component --name <id> [--author "<name>"] [--node-type-id <id>] [--label "<label>"]
```

**Generated structure:**

```
<name>/
├── Cargo.toml                 (workspace)
├── extension/
│   ├── Cargo.toml
│   ├── src/lib.rs             (impl invoke-tool with stub: validate_config, describe_node)
│   └── wit/world.wit
├── runtime/
│   ├── README.md              ("Drop your prebuilt .gtpack here, or build with gtdx dev")
│   └── .gitkeep
├── describe.json              (interpolated placeholders + sane defaults)
├── README.md                  (quickstart)
├── .gitignore
└── rust-toolchain.toml        (wasm32-wasip2 capable)
```

**Interpolated `describe.json` (post-scaffold):**

```jsonc
{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "<from --name>",
    "name": "<derived>",
    "version": "0.1.0",
    "author": "<from --author>"
  },
  "engine": {
    "greenticDesigner": "^0.x",
    "extRuntime": "^0.10"
  },
  "contributions": {
    "nodeTypes": [{
      "type_id": "<from --node-type-id>",
      "label": "<from --label>",
      "category": "tools",
      "icon": "puzzle",
      "color": "#0d9488",
      "complexity": "simple",
      "config_schema": "{}",
      "output_ports": [
        { "name": "success", "label": "Success" },
        { "name": "error", "label": "Error" }
      ]
    }]
  },
  "runtime": {
    "component": "extension.wasm",
    "gtpack": {
      "file": "runtime/REPLACE_ME.gtpack",
      "sha256": "REPLACE_AT_BUILD",
      "pack_id": "<from --name>",
      "component_version": "0.1.0"
    }
  },
  "permissions": {
    "network": [],
    "secrets": [],
    "callExtensionKinds": []
  }
}
```

**Default `extension/src/lib.rs` stubs:**

- `validate_config(args_json)` returns `{ "valid": true }`. Author replaces with real schema validation.
- `describe_node(args_json)` returns metadata mirrored from `describe.json`.

**Author workflow:**

1. `gtdx new --kind wasm-component --name greentic.my-tool`
2. Edit `describe.json` → fill `category`, `icon`, `config_schema`, optional `permissions`
3. Drop pre-built `.gtpack` into `runtime/`
4. `gtdx dev` → builds `extension.wasm`, signs, installs locally, watches for changes
5. Open designer → node appears in palette
6. `gtdx publish` when ready

**Implementation notes:**

- Reuse existing `gtdx new` scaffolding mechanism (subcommand #8). Add `wasm-component` to `--kind` enum: `design | bundle | deploy | wasm-component`.
- `wasm-component` is syntactic sugar for `DesignExtension` + `nodeTypes` + `runtime.gtpack` workflow.
- No `describe.json` schema change; all required fields exist in v1.
- Template files embedded in `gtdx` binary via `include_str!` or `rust-embed`.

## Track B — Lifecycle Backend

### CLI commands (`gtdx`)

```
gtdx enable  <id>[@<version>]    # set enabled=true
gtdx disable <id>[@<version>]    # set enabled=false
gtdx list    [--status]          # extend existing; add STATUS column
```

**Behavior:**

- `<id>@<version>` optional; if `<id>` only and multiple versions installed, error with list of available versions.
- Verify extension exists at `~/.greentic/extensions/<kind>/<id>-<version>/` before writing state. Not installed → error.
- Use `greentic-ext-state` lib (atomic write + advisory lock).
- Print "Enabled: <id>@<version> (designer will reload)" on success. Idempotent.
- `list --status`: extension absent from state file shows as `enabled` (default behavior).

**Disable warning:** scan other installed extensions whose `capabilities.required` references capabilities offered by the target. Print warning per dependent extension. Do not block (cascade resolution out of scope).

### Designer boot + filter

**Filter location:** designer-side (`greentic-designer/src/ui/mod.rs`), not `ext-runtime`. Runtime stays generic (loads what is on disk); designer applies policy.

**Updated boot sequence:**

1. `bundled::install_all()` (existing — first-run extensions)
2. `runtime.scan_kind_dir(...)` (existing — load all from disk)
3. **NEW:** `ExtensionState::load(home)`
4. **NEW:** filter `runtime.loaded()` — for each `(id, version)`, check `state.is_enabled(id, version)`. Pass-through populates `NodeTypeRegistry`. Filtered-out extensions stay in runtime memory but contribute no node types and receive no tool calls.
5. (existing) `tool_bridge` routes LLM tool calls to enabled extensions only.

**Disabled extension nodes in loaded flows:**

- Flow loads without crash. Nodes from disabled extensions render with badge `available: false`.
- Flow validation / deploy blocks until extension re-enabled.
- API: `NodeTypeRegistry::get_with_state(type_id) -> { descriptor, available: bool }`. Future UI consumes the `available` flag.

### Hot-reload watcher

**Location:** `greentic-ext-runtime` crate — runtime owns discovery, so it owns re-discovery.

**Public API:**

```rust
impl ExtensionRuntime {
    pub fn watch(&self, callback: Box<dyn Fn(RuntimeEvent) + Send + Sync>) -> WatchHandle;
}

pub enum RuntimeEvent {
    ExtensionAdded(ExtensionId, Version),
    ExtensionRemoved(ExtensionId, Version),
    ExtensionChanged(ExtensionId, Version),
    StateFileChanged,
}
```

**Implementation:**

- `notify` crate, cross-platform.
- Watch dirs: `~/.greentic/extensions/{design,deploy,bundle,provider}/` recursive + `~/.greentic/extensions-state.json` flat.
- Debounce 500ms.
- On filesystem event:
  - Add/Remove/Change → re-scan affected extension dir, update internal `loaded` map, fire callback.
  - State file change → fire `StateFileChanged` callback (designer re-applies filter).

**Designer subscribes** at boot:

```rust
runtime.watch(Box::new(move |event| {
    let new_registry = build_registry_from(&runtime, &state);
    designer_state.registry.store(Arc::new(new_registry));
    sse_broadcast(event);
}));
```

`ArcSwap<NodeTypeRegistry>` gives lock-free reads. In-flight requests see the old registry; new requests see the new one.

### HTTP API endpoints (designer)

```
GET    /api/extensions
  → 200 { extensions: [{ id, version, kind, enabled, available, describe }] }

POST   /api/extensions/:id/enable
  Body: { version: "0.1.0" }
  → 200 { id, version, enabled: true }
  → 404 if not installed

POST   /api/extensions/:id/disable
  Body: { version: "0.1.0" }
  → 200 { id, version, enabled: false }
  → 404 if not installed

GET    /api/extensions/events  (SSE)
  → stream of `data: { type, id, version }\n\n` events
```

**Authentication:** match existing designer endpoint pattern. If designer enforces 127.0.0.1 bind + bearer token (per security guidance for secrets-touching surfaces), these endpoints inherit the same.

**Error envelope:** match existing designer convention.

**Idempotency:** enable/disable are idempotent. Re-enabling an already-enabled extension returns 200 OK with no change.

## Track C — Pack Import Backend

### Endpoint

Single endpoint, two variants by `Content-Type`:

```
POST /api/packs/import
```

**Multipart upload:**

```
Content-Type: multipart/form-data
Body:
  gtpack=<binary .gtpack>
  trust=<strict|normal|loose>   (optional, default "normal")
```

**Catalog ref:**

```
Content-Type: application/json
Body: {
  "ref": "greentic.dentist-template@1.2.0",
  "registry": "default",     (optional)
  "trust": "normal"          (optional)
}
```

**Success response:**

```json
{
  "pack_id": "greentic.dentist-template",
  "version": "1.2.0",
  "stored_at": "/home/user/.greentic/designer/imported-packs/greentic.dentist-template-1.2.0/",
  "flow_path": "/home/user/.greentic/designer/imported-packs/greentic.dentist-template-1.2.0/flows/main.ygtc",
  "components": [
    { "id": "greentic.llm-openai", "version": "0.6.0" }
  ],
  "signature": { "verified": true, "key_id": "abcd...", "trust": "normal" }
}
```

**Error response:** designer standard `{ error: { code, message, details } }` envelope.

### Storage

**Path:** `~/.greentic/designer/imported-packs/<pack-id>-<version>/`

**Why separate from `~/.greentic/runtime/packs/`:**

- Runner hot-reload polls `~/.greentic/runtime/packs/`. Imported packs are designer-side source content, not auto-deployed to runtime.
- On user-initiated "Deploy", designer copies/symlinks to runtime path. That is a separate concern.
- Path separation is a hard guarantee against accidental run.

**Layout inside folder:** identical to extracted `.gtpack` ZIP — `manifest.cbor`, `sbom.cbor`, `flows/`, `components/`, `assets/`, optional `signatures/`.

**Cleanup:** out of scope MVP. Manual `rm -rf`. Future: TTL or LRU.

### Validation pipeline

Run in order, fail-fast:

1. **Size limit** — reject if `> 100 MB` (configurable via env). DoS guard.
2. **ZIP integrity** — open archive, parse manifest. Reject if corrupt.
3. **Path traversal** — every entry must be relative with no `..`. Reject otherwise.
4. **Schema** — `manifest.cbor` validates against `PackManifest` schema (share with `greentic-pack-lib`).
5. **Manifest version compat** — `manifest.format_version` in supported range. Reject with clear message.
6. **Signature** — apply trust policy (see below). With `loose` and unsigned, pass with warning in response.

**Example error:**

```json
{
  "error": {
    "code": "PACK_INVALID_SIGNATURE",
    "message": "Pack signature verification failed: untrusted key 'abcd...'",
    "details": { "trust_policy": "strict", "key_id": "abcd..." }
  }
}
```

### Catalog ref resolution

**Library:** new crate `greentic-pack-registry` in `greentic-designer-extensions` workspace, or extend existing `greentic-ext-registry` if structure fits (verify at implementation time).

**Public API:**

```rust
pub trait PackRegistryClient {
    fn fetch(&self, ref_str: &str, registry: Option<&str>) -> Result<Vec<u8>, RegistryError>;
}
```

**Default impl:** `StoreServerClient` — HTTP GET to store-server `.gtpack` endpoint (see Store-server section).

**Registry config:** reuse existing `~/.greentic/registries.toml` from `gtdx login/registries`.

### Trust policy (sync point with Track B)

**Location:** `greentic-ext-registry` crate (if it exists) or new `greentic-trust` crate in `greentic-designer-extensions` workspace. Pick whichever minimizes disruption.

**Definition:**

```rust
pub enum TrustPolicy {
    Strict,   // signed only, key must be in trust store
    Normal,   // signed only, prompt-on-first-use for new key
    Loose,    // accept unsigned (warning in response); reject corrupt signature
}

pub struct TrustVerifier { /* private */ }

impl TrustVerifier {
    pub fn verify(&self, signature: Option<&Signature>, pack_bytes: &[u8], policy: TrustPolicy) -> TrustResult;
}
```

**`Normal` behavior in designer (no interactive terminal):**

- API returns `409 TRUST_PROMPT_REQUIRED` with body `{ key_id, key_fingerprint }`.
- Frontend (future UI v2) prompts user. For MVP backend, user runs `gtdx trust add <key-id>` manually then retries the import. Alternatively, client sends `?accept_key=<fingerprint>` query param as explicit consent on retry.

**Migration:** existing `gtdx install` trust verification code moves to this shared crate. Track B PR carries the largest existing-code change here.

### Store-server side

**Repo:** `greentic-store-server`.

**Changes:**

1. Add `.gtpack` to artifact type allowlist (currently `.gtxpack` only).
2. New endpoints (or extend existing):
   - `POST /v1/packs/<publisher>/<name>/<version>` — upload `.gtpack` (mirror `.gtxpack` upload).
   - `GET  /v1/packs/<publisher>/<name>/<version>/download` — serve `.gtpack` binary.
   - `GET  /v1/packs/<publisher>/<name>/<version>/metadata` — JSON metadata.
3. Storage backend (Minio): add path namespace `packs/<publisher>/<name>/<version>.gtpack` alongside `extensions/<publisher>/<name>/<version>.gtxpack`.
4. Auth: reuse existing publisher auth. The `greentic` publisher is already registered.

**Backwards compat:** zero impact — purely additive.

## Cross-cutting

### Error handling

- Internal/library: `anyhow::Result<T>` + `.context()`.
- Domain errors crossing API boundaries: `thiserror`.
- API error envelope: `{ error: { code, message, details } }`.
- All logging via `tracing` crate.

**Log levels:**

- `info!` — state changes (enable/disable/import success), boot summary
- `warn!` — trust violations, ambiguous version, capability dependency conflict
- `error!` — IO failures, signature failures, schema parse errors

**Failure-tolerance pattern:** atomic writes (tmp+rename); readers tolerate missing or malformed files (fall back to default state, do not delete or overwrite).

### Testing strategy

**Track A:**

- Snapshot test: `gtdx new --kind wasm-component --name test.foo` vs fixture tree.
- Smoke test: scaffolded project compiles to `wasm32-wasip2`.
- Interpolation correctness: all placeholders filled.

**Track B:**

- `greentic-ext-state` unit tests: JSON round-trip, atomic write under concurrent writers, missing-file fallback, forward-compat preservation of unknown fields.
- `gtdx enable/disable` integration: state file content correct, error paths (not installed, ambiguous version), idempotency.
- Designer-side: boot with mixed enabled/disabled state populates registry correctly; hot-reload triggered by external state file edit propagates to registry within 1s; SSE events fire on state change; disabled-extension nodes in loaded flows expose `available: false`.

**Track C:**

- Validation pipeline unit tests: each fail mode (size, ZIP corrupt, traversal, schema, version, signature) × each trust policy × signed/unsigned/wrong-key.
- Endpoint integration: multipart happy path, catalog ref happy path (mock store-server), error code mapping, idempotency on duplicate import.
- Store-server: upload + download `.gtpack` round-trip, SHA256 integrity, auth required.

**E2E (manual for MVP):**

- Author scaffolds extension, drops dummy `.gtpack`, `gtdx dev`, installs, designer loads, `gtdx disable`, designer reloads, node hidden, `gtdx enable`, node returns.
- `curl POST /api/packs/import` (multipart) → file present in `~/.greentic/designer/imported-packs/`, metadata correct.

### Dependencies

**Hard (lock in this spec before branches are created):**

1. State file schema v1.0 — Track A reads (dev workflow), Track B reads + writes.
2. Trust policy abstraction — Track B existing `gtdx install` refactor consumes; Track C builds on.
3. `describe.json` — no schema bump. Track A uses existing v1.

**Soft (each track ships independently; E2E ordering):**

| Track | Ships independently? | E2E requires |
|-------|----------------------|--------------|
| A | Yes | — |
| B (CLI) | Yes | — |
| B (designer backend) | Yes | State file API stable from sync point |
| C (designer backend) | Yes | Mock store-server until store-server PR lands; multipart works without store-server |
| C (store-server) | Yes | — |

Cross-track integration test runs after all five PRs land — handled by `greentic-e2e`.

### Backwards compatibility & rollout

No breaking changes:

- Existing extensions without state file → enabled by default (new behavior matches old).
- Existing `gtdx install/uninstall/dev/publish` → unchanged.
- Existing designer flows → unchanged.
- Existing `.gtxpack` artifacts on store-server → unchanged; `.gtpack` is additive.

No feature flag. No data migration.

### Observability

- `gtdx enable/disable`: `tracing::info!(ext_id, version, action, "extension state changed")`
- Designer hot-reload: `info!(event = ?event, "registry rebuilt")`
- Pack import: `info!(pack_id, version, source, trust = ?policy, "pack imported")`
- Trust violation: `warn!(key_id, expected_policy = ?policy, "trust verification failed")`

No new metric/observability tooling. Reuse existing `tracing` setup per repo.

### Documentation deliverables

PRs include doc updates within the same repo:

- `greentic-designer-extensions/docs/lifecycle-management.md` (new, Track B PR)
- `greentic-designer-extensions/docs/wasm-component-tutorial.md` (new, Track A PR)
- `greentic-designer-extensions/docs/gtdx-cli.md` (update, Track B PR — add enable/disable to reference)
- `greentic-designer/README.md` + `CLAUDE.md` updates if user-facing API surface changes (Tracks B + C)
- `greentic-store-server/README.md` update for `.gtpack` endpoint (Track C)

After all PRs land:

- `greentic-docs/` update: expand "Designer Extensions" section with lifecycle + scaffold + pack import; auto-translate to 5 languages.
- Workspace `CLAUDE.md` update if commands or paths changed.
- `MEMORY.md` update with link to this spec + per-track PR status.

## Effort estimate

| Track | Code | Test | Docs | Total |
|-------|------|------|------|-------|
| A | 1–2 days | 0.5 day | 0.5 day | ~3 days |
| B (CLI + state lib) | 2 days | 1 day | 0.5 day | ~3.5 days |
| B (designer backend + watcher + API) | 3–4 days | 1–2 days | 0.5 day | ~5–6 days |
| C (designer backend + validation + endpoint) | 3 days | 1–2 days | 0.5 day | ~5 days |
| C (store-server `.gtpack` support) | 1–2 days | 1 day | 0.5 day | ~3 days |

**Full parallel (3 agents min):** ~6 days wall-clock (longest path = Track B designer backend).
**Sequential:** ~20 days.

## Open questions / future work

- Per-tenant state model: data model is forward-compatible, but enforcement (which tenant's state to use) requires tenant identity at designer boot — defer to designer-admin track.
- Cascade dependency resolver when disabling: warn-only in MVP. Future: explicit dependency graph + cascade prompt.
- Runtime extension hot-reload at `greentic-runner` side: out of scope (runner already polls pack dir every 30s; extensions are designer-side). If runner needs extension awareness later, separate spec.
- Pack export from designer: out of scope; separate spec when needed.
- Demo-mode infrastructure: deferred per Maarten alignment. Manual `gtdx disable` workaround acceptable for MVP.
