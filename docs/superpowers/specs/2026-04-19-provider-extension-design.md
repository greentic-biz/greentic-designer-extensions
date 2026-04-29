# Provider Extension System Design

**Spec date:** 2026-04-19
**Status:** Brainstorm complete, awaiting writing-plans
**Owner:** greentic-biz / maarten@
**Impacted repos:** `greentic-designer-extensions`, `greentic-messaging-providers`, `greentic-runner` (doc-only), `greentic-docs`

## 1. Summary

Introduce a fourth extension kind — `extension-provider` — alongside the existing `extension-design`, `extension-bundle`, and `extension-deploy` types. This extension kind provides a **metadata + catalog layer** over the existing `.gtpack` runtime artifact model: `.gtxpack` provider extensions carry a `describe.json` (capabilities, schemas, i18n, OAuth metadata) and embed the runtime `.gtpack` inline. Installation via `gtdx` drops the embedded `.gtpack` into the existing runner pack directory; the runner polls and hot-loads as it does today. No runtime path changes.

The goal is to make messaging and event providers **discoverable, dynamically selectable, signable, and author-able by third parties** via the same ecosystem already used for design/bundle/deploy extensions — while preserving 100% backwards compatibility with the existing `.gtpack`-only distribution model.

## 2. Goals

- G1. Messaging and event providers installable via `gtdx install`
- G2. Providers discoverable dynamically at design-time (Greentic Designer picker)
- G3. Providers selectable at build-time (Bundle wizard) and deploy-time (Deploy wizard)
- G4. Third parties can ship custom providers without forking `greentic-messaging-providers`
- G5. Single signing pipeline — reuse Wave 1 Ed25519 + JCS infrastructure
- G6. Zero runtime path changes in `greentic-runner` — extension is purely additive
- G7. Progressive migration — existing 7 providers stay shipping as `.gtpack` until retrofitted
- G8. Events providers (inbound triggers + outbound emits) covered by same contract

## 3. Non-goals

- NG1. Replacing the `.gtpack` runtime artifact format
- NG2. Wiring extension-provider runtime invocation into `greentic-runner` (runtime calls stay in `greentic:component@0.6.0` contract inside the pack)
- NG3. Per-tenant provider installation — installation is host-level; tenant scoping remains at the bundle `.gmap` layer
- NG4. Designer live-refresh when providers install out-of-process (cache bust on next query is sufficient for v1)
- NG5. Multi-tenant filtering in designer picker (bundle-level concern via `.gmap`)
- NG6. Provider revocation mechanism (no negative signature support v1; documented for future work)

## 4. Context

### 4.1 Current provider distribution

`greentic-messaging-providers` ships seven providers (Slack, Teams, Telegram, Webex, WhatsApp, WebChat, Email) as `.gtpack` archives. Packs contain `manifest.cbor`, runtime WASM components targeting `wasm32-wasip2`, and Ed25519 signatures. Runner loads packs from a pack index directory via 30s polling with atomic `ArcSwap` hot-reload.

Event providers (cron, webhook triggers, etc.) follow the same gtpack pattern.

### 4.2 Current extension ecosystem

`greentic-biz/greentic-designer-extensions` ships since 2026-04-17:
- `greentic-extension-sdk-contract` — types, describe.json validator, Ed25519 signatures
- `greentic-ext-runtime` — wasmtime Component loader, capability registry, hot reload
- `greentic-ext-registry` — registry traits + 3 implementations, installer lifecycle
- `greentic-ext-cli` (`gtdx`) — 11 subcommands
- WIT packages: extension-base, extension-host, extension-design, extension-bundle, extension-deploy, runtime-side

Wave 1 signing pipeline (2026-04-19): `gtdx keygen/sign/verify`, JCS canonicalization, runtime verify gate with `GREENTIC_EXT_ALLOW_UNSIGNED=1` escape hatch.

### 4.3 Drivers

Three motivations confirmed during brainstorming:
1. Custom third-party messaging/event providers (drop "fork the monorepo" friction)
2. Provider selection at bundle deploy/download time (designer queries registry, not hardcoded enum)
3. Events running inside flows (node picker populated from installed extensions)

## 5. Approach selection

### 5.1 Distribution model: Approach B (hybrid)

Extension = metadata/catalog layer. `.gtpack` = runtime artifact. They travel together as one `.gtxpack` file.

Alternatives rejected:
- **Approach A** (replace `.gtpack` with `.gtxpack` as runtime artifact): requires `greentic-ext-runtime` to expose host imports providers need (`state-store`, `outbound-http` per-channel, `telemetry`, OAuth callback). Massive runtime coupling, not scoped for v1. Forces re-deploy of all tenant providers.
- **Approach C** (status quo + `gtdx install .gtpack` tooling): keeps two parallel distribution paths forever. Community confusion. Designer must query two catalog sources.

### 5.2 Bundle strategy: B1 (inline wrap)

`.gtxpack` contains `describe.json` + embedded `.gtpack` in a single file. Alternatives rejected:
- **B2** (reference only): 2-fetch install, 2-signature-chain verification, cache invalidation pain, air-gap hostile.
- **B3** (hybrid inline+ref): double codepath complexity. Introduce when real demand exists.

### 5.3 WIT package shape: P1 (single package, 3 sub-interfaces)

One `extension-provider@0.1.0` package with three sub-interfaces:
- `messaging` — bi-directional (send + receive)
- `event-source` — inbound triggers
- `event-sink` — outbound emits

Provider declares which sub-interfaces it exports in its WIT `world`. Mirrors existing `extension-design`/`-bundle`/`-deploy` single-package-multi-interface pattern. Mixed-capability providers (e.g., Slack with both messaging and event-sink) ship as one extension with two capabilities.

Alternatives rejected:
- **P2** (2 separate packages): mixed providers need 2 installs — awkward UX.
- **P3** (3 separate packages): cron source + email sink always paired in practice — over-fragmentation.

### 5.4 Runtime handoff: R1 (drop directory)

`gtdx install` extracts embedded `.gtpack` into `~/.greentic/runtime/packs/providers/gtdx/`. Runner picks up via its existing 30s pack-index poll. Zero new HTTP surface area, zero new runner code.

Alternatives rejected:
- **R2** (runner HTTP admin API): requires runner running to install, adds auth surface.
- **R3** (shared manifest file + inotify): two state stores (disk + manifest) can drift.

### 5.5 Migration: M3 (progressive batched)

Four batches ship serially, independent PRs per batch:

| Batch | Providers | Complexity driver |
|-------|-----------|-------------------|
| 0 (Pilot) | Telegram | Simplest — no Adaptive Card tier, no OAuth |
| 1 | WebChat, Email, Cron (events) | Email validates 2-sub-interface pattern; Cron validates event-source contract independently |
| 2 | Slack, Webex | Validates OAuth field shape in describe.json |
| 3 | Teams, WhatsApp | Highest complexity — bot registration, Business API |

Within Batch 1, the three providers ship as independent parallel PRs that don't block each other.

## 6. Architecture

### 6.1 Data flow

```
[Author/Admin]
     │
     ▼
  gtdx install provider-slack.gtxpack
     │
     ├── verify Ed25519 signature (Wave 1 pipeline)
     ├── unwrap describe.json
     ├── verify embedded .gtpack sha256 vs describe.json declaration
     ├── load WASM extension component → call extension-base.describe() sanity check
     ├── extract describe + schemas + i18n → ~/.greentic/extensions/provider/<id>/<version>/
     ├── extract runtime.gtpack → ~/.greentic/runtime/packs/providers/gtdx/<id>-<version>.gtpack
     └── register in gtdx registry SQLite
                │
                ▼
        [runner pack-index poll, 30s]
                │
                ▼
        TenantRuntime auto-load → ArcSwap hot-reload
                │
                ▼
[Designer]          ← query gtdx registry → "available providers?" → messaging.list-channels()
[Bundle wizard]     ← read describe.json → secret-schema() → generate QA prompts
[Deploy wizard]     ← read describe.json → config-schema() → generate .env templates
```

### 6.2 Directory convention

```
~/.greentic/
├── extensions/                 # gtdx-managed extension metadata
│   ├── design/<id>/<version>/
│   ├── bundle/<id>/<version>/
│   ├── deploy/<id>/<version>/
│   └── provider/<id>/<version>/   # NEW
│       ├── describe.json
│       ├── schemas/
│       ├── i18n/
│       └── provider_ext.wasm
│
├── runtime/packs/              # runner-consumed packs
│   ├── applications/
│   ├── infrastructure/
│   └── providers/
│       ├── manual/             # manually-installed .gtpack (backcompat)
│       └── gtdx/               # NEW — gtdx-installed, extracted from .gtxpack
│
└── gtdx.db                     # SQLite registry — installed extension metadata, active/superseded flags
```

### 6.3 Conflict resolution

Installation refuses when a manually-installed `.gtpack` with the same `pack_id` already exists in `providers/manual/`. Override with `gtdx install --force` or pre-emptive manual cleanup.

Runtime: when duplicate `pack_id` across `manual/` and `gtdx/` directories (shouldn't happen post-install-check but defensive), runner picks highest semver via existing dedup path.

### 6.4 Tenant scoping

Installation is host-level. Per-tenant enablement lives in bundle `.gmap` access rules (existing mechanism). Secrets remain per-tenant via `greentic-secrets` URI pattern `secrets://{env}/{tenant}/{team}/{provider_id}/{key}`.

## 7. WIT contract — `extension-provider@0.1.0`

### 7.1 Package location

`greentic-biz/greentic-designer-extensions/wit/extension-provider.wit`.

Mirrored at build time into provider implementation repos via manual `cp` (consistent with `adaptive-card-extension` pattern). README in each implementing repo notes the vendored contract version.

### 7.2 World declarations

```wit
package greentic:extension-provider@0.1.0;

// Provider exports only messaging (e.g., Telegram v1)
world messaging-only-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
}

// Provider exports only event source (e.g., cron)
world event-source-only-provider {
  include greentic:extension-base/base@0.1.0;
  export event-source;
}

// Provider exports only event sink (e.g., generic webhook sender)
world event-sink-only-provider {
  include greentic:extension-base/base@0.1.0;
  export event-sink;
}

// Provider exports both (e.g., Email: send + inbox polling trigger)
world messaging-and-event-source-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
  export event-source;
}

// Future: full provider (e.g., Slack: messaging + channel-event sink)
world full-provider {
  include greentic:extension-base/base@0.1.0;
  export messaging;
  export event-sink;
}
```

### 7.3 Interface shapes

```wit
interface types {
  type channel-id = string;
  type trigger-id = string;
  type event-id = string;

  record channel-profile {
    id: channel-id,
    display-name: string,
    direction: direction,
    tier-support: list<card-tier>,
    metadata: list<tuple<string, string>>,
  }

  record trigger-profile {
    id: trigger-id,
    display-name: string,
    emit-shape: string,  // JSON pointer to schemas/<file>
  }

  record event-profile {
    id: event-id,
    display-name: string,
    payload-shape: string,  // JSON pointer to schemas/<file>
  }

  enum direction {
    inbound,
    outbound,
    bidirectional,
  }

  enum card-tier {
    tier-a-native,
    tier-b-attachment,
    tier-c-fallback,
    tier-d-text-only,
  }

  variant error {
    not-found(string),
    schema-invalid(string),
    internal(string),
  }
}

interface messaging {
  use types.{channel-id, channel-profile, error};

  list-channels: func() -> list<channel-profile>;
  describe-channel: func(id: channel-id) -> result<channel-profile, error>;
  secret-schema: func(id: channel-id) -> result<string, error>;   // JSON Schema string
  config-schema: func(id: channel-id) -> result<string, error>;   // JSON Schema string
  dry-run-encode: func(id: channel-id, sample: list<u8>) -> result<list<u8>, error>;  // optional
}

interface event-source {
  use types.{trigger-id, trigger-profile, error};

  list-trigger-types: func() -> list<trigger-profile>;
  describe-trigger: func(id: trigger-id) -> result<trigger-profile, error>;
  trigger-schema: func(id: trigger-id) -> result<string, error>;  // JSON Schema for flow config
}

interface event-sink {
  use types.{event-id, event-profile, error};

  list-event-types: func() -> list<event-profile>;
  describe-event: func(id: event-id) -> result<event-profile, error>;
  event-schema: func(id: event-id) -> result<string, error>;  // JSON Schema for payload
}
```

### 7.4 Semantics

All interfaces are **metadata-only** — invoked design-time and build-time. Runtime message send / event emit stays in `greentic:component@0.6.0` inside the embedded `.gtpack`. The sole overlap with runtime is `dry-run-encode` for design-time preview (optional per provider, no live external side effects).

JSON Schemas returned as strings use Draft 2020-12 dialect (matching `describe.json` validator).

## 8. Artifact format — `.gtxpack` for providers

### 8.1 Layout

```
greentic.provider.slack-0.1.0.gtxpack    (ZIP archive)
├── describe.json
├── describe.json.sig
├── wasm/
│   └── provider_slack_ext.wasm          (extension-provider WIT implementation, ~200-400 KB)
├── runtime/
│   └── provider.gtpack                  (embedded runtime pack, ~1-3 MB)
├── schemas/
│   ├── channel-im.schema.json
│   ├── channel-public.schema.json
│   └── event-channel-created.schema.json
└── i18n/
    ├── en.json
    ├── id.json
    ├── ja.json
    ├── zh.json
    ├── es.json
    └── de.json
```

Expected total size: 1.5–4 MB per provider. Upper acceptance bound: 5 MB.

### 8.2 `describe.json` additions for `kind=provider`

Provider extensions reuse the existing `DescribeJson` schema with ONE additive change: the existing `runtime` object gets an optional `gtpack` sub-field. When `kind=ProviderExtension`, `runtime.gtpack` is required; for other kinds it is forbidden (enforced via `TryFrom` validation at deserialize time).

```json
{
  "$schema": "https://greentic.ai/schemas/extension-describe.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "ProviderExtension",
  "metadata": {
    "id": "greentic.provider.slack",
    "name": "Slack Messaging Provider",
    "version": "0.1.0",
    "summary": "Send and receive Slack messages via Bot API",
    "author": {
      "name": "Greentic",
      "publicKey": "S6cnfmdoj3wKx3cxPsNsP8fErvK4S12a5AtzTExPrvQ="
    },
    "license": "Apache-2.0"
  },
  "engine": {
    "greenticDesigner": "*",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "greentic:provider.messaging", "version": "0.1.0" },
      { "id": "greentic:provider.event-sink", "version": "0.1.0" }
    ],
    "required": []
  },
  "runtime": {
    "component": "wasm/provider_slack_ext.wasm",
    "memoryLimitMB": 64,
    "permissions": {
      "network": [],
      "secrets": ["secrets://{env}/{tenant}/{team}/greentic.provider.slack/*"],
      "callExtensionKinds": []
    },
    "gtpack": {
      "file": "runtime/provider.gtpack",
      "sha256": "<64-char hex>",
      "pack_id": "greentic.provider.slack",
      "component_version": "0.6.0"
    }
  },
  "contributions": {
    "oauth": {
      "provider": "slack",
      "scopes_requested": ["chat:write", "channels:read"],
      "callback_url_pattern": "https://{deployment_host}/oauth/callback/slack",
      "token_storage": "per-tenant"
    }
  },
  "signature": {
    "algorithm": "ed25519",
    "publicKey": "<base64>",
    "value": "<base64>"
  }
}
```

**Schema integration notes:**
- `runtime.gtpack` is an *optional* field on the existing `Runtime` struct in `greentic-extension-sdk-contract::describe::Runtime`
- `runtime.component` still points to the extension WASM (metadata-query component, `wasm32-wasip1`). For providers, this is the same `provider_*_ext.wasm` that lives in `wasm/`
- Invariant enforced at deserialize: `kind == ProviderExtension ↔ runtime.gtpack.is_some()`
- Provider-specific OAuth/channel/trigger metadata goes under `contributions` (existing free-form field) rather than top-level — keeps schema additive

`oauth` field optional — only providers with OAuth flows populate it. Introduced in Batch 2 with Slack.

### 8.3 Signature coverage

Single signature (`describe.json.sig`) covers entire `describe.json` (JCS canonicalized). Since `describe.json.runtime.gtpack.sha256` is part of the signed payload, tampering with the embedded `.gtpack` invalidates the signature chain — no nested signing required.

### 8.4 Install lifecycle

```
1. Read .gtxpack → load describe.json
2. verify_describe() via JCS + Ed25519 (Wave 1 pipeline)
   (escape hatch: GREENTIC_EXT_ALLOW_UNSIGNED=1 for dev)
3. Hash runtime/provider.gtpack bytes → compare vs describe.json.runtime.gtpack.sha256
4. Load wasm/*.wasm → call extension-base.describe() for sanity check
5. Conflict check: read manifest.cbor from each file in ~/.greentic/runtime/packs/providers/manual/,
   compare manifest.pack_id vs describe.json.runtime.gtpack.pack_id
   → if match found, refuse with actionable error (unless --force)
6. Extract metadata → ~/.greentic/extensions/provider/<id>/<version>/
7. Extract runtime.gtpack → ~/.greentic/runtime/packs/providers/gtdx/<id>-<version>.gtpack
8. Register in gtdx registry (SQLite ~/.greentic/gtdx.db)
9. Exit 0 → next runner poll (≤30s) picks up gtpack → ArcSwap hot-reload
```

### 8.5 Uninstall lifecycle

```
1. gtdx uninstall <id> [--version X]
2. Remove registry entry (designer picker hides immediately on next query)
3. Delete ~/.greentic/runtime/packs/providers/gtdx/<id>-<version>.gtpack
4. Delete ~/.greentic/extensions/provider/<id>/<version>/
5. Next runner poll → detect missing pack → graceful unload via ArcSwap
```

### 8.6 Version update

```
gtdx install provider-slack-0.2.0.gtxpack   # while 0.1.0 is installed
→ 0.2.0 extracts parallel to 0.1.0
→ Registry marks 0.2.0 active, 0.1.0 superseded
→ Runner poll → runner picks highest semver (0.2.0), unloads 0.1.0
→ Rollback: gtdx rollback provider-slack 0.1.0 swaps active flag
```

## 9. Consumer integration

### 9.1 Shared registry API

```rust
// greentic-ext-runtime::registry
pub trait ExtensionRegistry {
    async fn list_by_kind(&self, kind: ExtensionKind) -> Result<Vec<ExtensionInfo>>;   // NEW
    async fn get_describe(&self, ext_id: &str, version: Option<&str>) -> Result<DescribeJson>;  // NEW
    async fn invoke_tool(&self, ext_id: &str, tool: &str, args: Value) -> Result<Value>;  // EXISTING
}
```

`ExtensionKind::Provider` variant added to `greentic-extension-sdk-contract`.

### 9.2 Greentic Designer

- Flow node picker for "send message" → `messaging.list-channels()` across all registered providers
- Flow node picker for "on trigger" → `event-source.list-trigger-types()`
- Flow node "emit event" → `event-sink.list-event-types()`
- Cache results 60s; bust on shared file-watch of `~/.greentic/gtdx.db` OR on explicit designer reload action
- Empty-registry fallback: display "Install providers via `gtdx install` or Greentic Store" with link

### 9.3 Bundle wizard (`greentic-bundle` + `greentic-setup`)

`bundle.yaml` references providers:
```yaml
providers:
  - id: greentic.provider.slack
    version: "0.1.0"           # optional — default = latest installed
    channels: [im, public-channel]   # validated against describe-channel()
```

Wizard flow:
1. Scan `bundle.yaml` → list provider references
2. Per provider: `messaging.secret-schema(channel)` via `invoke_tool` → JSON Schema
3. Generate wizard questions from schema (existing `greentic-setup` QA generator)
4. Persist answers as secret URIs
5. Validate via `messaging.describe-channel()` before bundle produces artifact

i18n: load from `~/.greentic/extensions/provider/<id>/<version>/i18n/<locale>.json` with `en.json` fallback.

### 9.4 Deploy wizard (`greentic-deployer` + `greentic-operator`)

- `messaging.config-schema()` → `.env.example` generation + docker-compose env
- `messaging.secret-schema()` → prompt user, persist to target secret store
- OAuth providers: `describe.json.oauth.callback_url_pattern` → generate deployment-specific callback URL
- Integrates with Phase B #2 `host::secrets` interface when deployer Mode B lands

## 10. Migration plan (M3)

### 10.1 Pre-flight work

1. `greentic-biz/greentic-designer-extensions`:
   - Publish `extension-provider.wit@0.1.0`
   - Add `ExtensionKind::Provider` to `greentic-extension-sdk-contract`
   - Bump package to v0.7.0
   - Extend `greentic-ext-registry::lifecycle::install` for `runtime.gtpack.{file, sha256}`
   - Add `list_by_kind` + `get_describe` to registry trait + all 3 implementations
2. `greentic-ext-cli`:
   - `gtdx list --kind provider`
   - `gtdx info <provider-id>`
   - describe.json validator for `kind=provider`
3. Docs:
   - `docs/how-to-write-a-provider-extension.md` using Telegram pilot as reference

### 10.2 Batch 0 — Pilot Telegram

Target: `greentic-messaging-providers`. Feature-gated (`provider-extension`, off by default).

Deliverables:
- New crate `crates/provider-telegram-extension/` (wasm32-wasip1 cdylib)
- `describe.json` declares 1 channel (`direct-message`)
- `schemas/direct-message-secret.schema.json` — Bot token + webhook secret
- `i18n/en.json` + id, ja, zh, es, de (sync coverage with existing provider i18n)
- `build.sh` produces `greentic.provider.telegram-<version>.gtxpack`
- CI signing with `EXT_SIGNING_KEY_PEM` secret

Acceptance gate:
1. `gtdx install ./greentic.provider.telegram-0.1.0.gtxpack` succeeds
2. `gtdx list --kind provider` shows Telegram
3. `gtdx info greentic.provider.telegram` lists 1 channel
4. Runner poll picks up gtpack
5. E2E Telegram webhook → message send unchanged from current behavior
6. `gtdx uninstall` clean; gtpack auto-unloads in runner
7. Signature verification green with `greenticDesigner: "*"` in Engine field
8. Existing test suite passes without `--features provider-extension`

### 10.3 Batch 1 — WebChat + Email (parallel PRs)

**WebChat:**
- Channel profiles: `direct-line-v3`, `websocket`
- Config: Direct Line secret key, allowed origins
- Tier profile: `tier-a-native` Adaptive Card support
- Existing WebChat E2E must pass unmodified

**Email:**
- Channel profiles: `smtp`, `imap-inbox`
- **First multi-interface provider**: messaging (SMTP egress) + event-source (inbox polling)
- Trigger: `new-email-matching-filter` with subject regex + sender pattern schema

**Cron events provider** ships parallel to Batch 1:
- Single event-source extension
- 1 trigger type: `scheduled-cron` with `{expression, timezone}` config schema
- Validates `extension-event-source` contract independent of messaging

### 10.4 Batch 2 — Slack + Webex

Validates OAuth field shape in `describe.json` with two providers. If Webex requires field extensions beyond Slack's shape, bump to `extension-provider@0.2.0` and cascade re-sign to Telegram + WebChat + Email.

Risk mitigation: prototype Slack + Webex describe.json in parallel before contract freeze; don't merge Slack until Webex validates the field shape.

### 10.5 Batch 3 — Teams + WhatsApp

**Teams:**
- Config: Azure tenant ID, client ID, client secret, bot framework endpoint
- Tier: `tier-b-attachment`
- Install wizard callout: Azure bot pre-registration required

**WhatsApp:**
- Config: Phone number ID, system user access token, business account ID
- Metadata: `"requires_approval": ["whatsapp_business_api"]`
- Template message pre-approval workflow surfaced in setup

### 10.6 Backwards compatibility guarantee

Existing `.gtpack` providers remain installable via manual pack directory drop. Runner loads both equally. The only visibility impact: non-extension-wrapped providers don't appear in designer picker until retrofitted. Users can still wire them via YAML.

### 10.7 Rollback

Per-batch feature flag `--features provider-extension` keeps runtime `.gtpack` shipping unchanged. Failed batch → mark registry entry deprecated → users fall back to manual install. No runtime regression risk in any batch.

### 10.8 Docs per batch

Each batch updates `greentic-docs`:
- `docs/guides/provider-extensions.md` — concept page (created in Batch 0)
- `docs/reference/providers/<name>.md` — per-provider page with install command + config schema reference

## 11. Testing strategy

### 11.1 Unit tests

**`greentic-designer-extensions`:**
- `greentic-extension-sdk-contract` — `ExtensionKind::Provider` round-trip, describe.json validator for `kind=provider`, `runtime.gtpack` required
- `greentic-ext-registry::lifecycle::install` — mock `.gtxpack` install cycle, sha256 check, conflict detection against `manual/`
- `greentic-ext-runtime::invoke_tool` — mock provider WASM dispatch

**`greentic-messaging-providers`:**
- Per-extension-crate unit tests for describe/schema/list-channels return shapes
- Schema validation tests in separate rlib crate (workaround for cdylib+WIT linker)

### 11.2 Integration tests

**`greentic-designer-extensions`:**
- `tests/provider_install.rs` — full install cycle for fixture provider
- `tests/provider_invoke.rs` — invoke_tool on installed fixture provider

**`greentic-messaging-providers`:**
- `tests/extension_build.rs` — drive `build.sh` programmatically, assert `.gtxpack` structure
- `tests/backcompat.rs` — runner with manual-only gtpack, then parallel `.gtxpack` install, assert coexistence

### 11.3 E2E tests (`greentic-e2e`)

New `scripts/run_provider_extension_e2e.sh`. Per batch:
- Batch 0: `gtdx install` + send Telegram message end-to-end
- Batch 1: WebChat Direct Line + Email SMTP + Email IMAP trigger
- Batch 2+: OAuth flow with sandbox credentials (30–60s timeout)

### 11.4 Signing validation

`EXT_SIGNING_KEY_PEM` GH secret added to `greentic-messaging-providers` (fourth repo, alongside the 3 existing extension repos).

`CI_REQUIRE_SIGNED=1` guardrail on main push.

### 11.5 Regression safeguard

Non-negotiable: `cargo test --workspace` (without `--features provider-extension`) in `greentic-messaging-providers` must pass identically to pre-feature baseline.

### 11.6 Per-batch pre-merge checklist

1. `cargo build --workspace --all-features` green (contract, messaging-providers, e2e)
2. `cargo test --workspace --all-features` green
3. `cargo fmt -- --check` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` green
4. `.gtxpack` size ≤ 5 MB
5. E2E scenario green in CI
6. Docs page published

### 11.7 Known gotchas from deploy-extension learnings

- `std::env::set_var` is unsafe under Rust 2024 — use RAII EnvGuard with Mutex
- `std::env::var("X").is_ok()` returns true for empty string — use `remove_var` for "unset" in tests
- `greenticDesigner: "*"` required in Engine struct (serde `deny_unknown_fields`)
- `serde_yaml_bw` aliased to `serde_yaml` via `use serde_yaml_bw as serde_yaml;`
- Zip output divergence: Ubuntu InfoZip vs Python wrapper — build.sh must handle both filename variants
- Security bot commits may break fmt/clippy — expect post-push fix cycles
- `[workspace]` empty table + root `exclude = ["crates/provider-*-extension"]` for WASM crate isolation

## 12. Open questions

- **Q1.** WIT vendor sync automation — currently manual `cp` from `greentic-designer-extensions/wit/` to `greentic-messaging-providers/wit/`. Acceptable for Batch 0; if friction emerges at Batch 2, consider a `sync-wit.sh` script or submodule.
- **Q2.** OAuth field shape — prototype Slack + Webex in parallel during Batch 2 pre-work to validate field generality. If insufficient, bump to `extension-provider@0.2.0` with cascading re-signs.
- **Q3.** Extension removal semantics when bundles reference an uninstalled provider — error out at bundle wizard time, or warn + disable that flow node? Current recommendation: error out (loud failure, easier debugging).
- **Q4.** Signature revocation — out of scope for v1. Document rotation procedure in `how-to-write-a-provider-extension.md`.

## 13. References

- Designer Extensions System spec: `docs/superpowers/specs/2026-04-17-designer-extension-system-design.md`
- Deploy Extension migration: memory `deploy-extension-migration.md`, `deploy-extension-next-steps.md`
- Bundle Extension migration: memory `bundle-extension-migration.md`
- Wave 1 signing pipeline: `greentic-designer-extensions` PR #6 (JCS canonicalization fix)
- Adaptive Card extension pattern: `greentic-adaptive-card-mcp/crates/adaptive-card-extension/`
- Greentic Runner pack-polling mechanism: project root `CLAUDE.md` §"Runtime Architecture"
