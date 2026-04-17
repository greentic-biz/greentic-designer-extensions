# Greentic Designer Extension System — Design

**Date:** 2026-04-17
**Status:** Draft
**Authors:** Bimbim
**Repo:** `greentic-designer-extensions`
**Scope:** Module 4 (Design Extension) + Foundation shared with Modules 5, 6, 7
**Based on:** 16 April 2026 meeting with Maarten Ectors

---

## 1. Summary

Today, Greentic Designer has hardcoded domain knowledge (12 Adaptive Cards
tools, subprocess calls to `greentic-cards2pack` and `greentic-bundle`, no
deploy targets). Every new content type or deployment target requires a
designer release. This does not scale to "thousands of people filling the
Greentic Store with extensions."

This spec defines a WASM-based extension system that teaches the designer
new capabilities at runtime through signed `.gtxpack` archives distributed
via the Greentic Store. The system supports three extension kinds — design,
bundle, and deploy — sharing a common foundation (contract, runtime,
capability engine, CLI, store client) with kind-specific WIT sub-interfaces
and designer call-sites.

In scope for v1:

- Foundation (contract + runtime + CLI + store client)
- Module 4 (Design Extension kind) — full
- Module 5 + 6 (Bundle + Deploy kinds) — contract only, no reference impl
- Module 7 (Store) — client + OpenAPI spec, server in separate repo
- Reference implementation: `greentic.adaptive-cards@1.6.0`
- Designer refactor to consume the runtime

Out of scope for v1: bundle/deploy reference implementations, store server,
Digital Workers / Telco X / Green Tech X templates, messaging provider
extraction, MCP extension.

## 2. Goals

- G1. Designer has no hardcoded content-type knowledge. All domain logic
  lives in extensions.
- G2. Extensions are signed WASM components, distributed via the Greentic
  Store, versioned with semver.
- G3. Extensions declare capabilities they **offer** and **require**. A
  runtime matching engine resolves dependencies and fails gracefully when
  requirements are missing.
- G4. Three extension kinds — design, bundle, deploy — share a common
  foundation but plug into different designer call-sites.
- G5. Third parties (Osora, Telco X) can author extensions using a
  documented contract and a CLI scaffolding tool.
- G6. Cross-extension composition via a permission-gated host broker.
- G7. Zero regression in designer UX during migration (two-phase rollout).

## 3. Non-goals / Out of scope (v1)

- Multi-tenancy: extensions load per user, not per tenant
- Automatic dependency resolution on install (user installs dependencies
  manually in v1; `--with-deps` is future)
- Fine-grained sandboxing beyond wasmtime defaults + network/secrets
  allowlist
- Hot-swap preserving extension state (hot reload = drop + reload; stateful
  extensions persist via host storage interface)
- Nested extensions (extension containing another extension)
- Web-based Store UI (P3)
- OCI escape-hatch for Store artifact hosting (v1.1 if enterprise demand)
- Demo cleanup, `build-answer.json` schema, Codex fix (Modules 1-3 — other
  workstreams)
- Bundle-extension and deploy-extension reference implementations (Cycles
  3-4)
- Digital Workers / Telco X / Green Tech X design-extensions (follow-ups)

## 4. High-level architecture

```
   ┌─────────────────────────────────────────────────────────┐
   │              Greentic Store (store.greentic.ai)         │
   │   Developers upload · end-users discover + install      │
   └────────────────────────┬────────────────────────────────┘
                            │ HTTPS / OpenAPI
                            ▼
   ┌─────────────────────────────────────────────────────────┐
   │               greentic-ext-runtime                      │
   │                                                         │
   │   Registry trait ─ Store · OCI · Local · Git            │
   │   Discovery     ─ scan dirs · parse describe.json       │
   │   Installer     ─ verify sig · permission prompt · swap │
   │   Capability    ─ offered/required · semver match       │
   │    Registry       degraded state · cycle detect         │
   │   Broker        ─ cross-ext calls · permission gate     │
   │   Wasmtime      ─ Component instantiation · pool        │
   │   Watcher       ─ hot reload on FS events               │
   └────────────────────────┬────────────────────────────────┘
                            │
   ┌────────────────────────▼────────────────────────────────┐
   │                  Greentic Designer                      │
   │                                                         │
   │   ┌──────────┐   ┌───────────┐   ┌───────────┐          │
   │   │  Chat    │──▶│ Next      │──▶│  Deploy   │          │
   │   │  (LLM)   │   │ wizard    │   │  wizard   │          │
   │   └─────┬────┘   └─────┬─────┘   └─────┬─────┘          │
   │         │              │               │                │
   │   design-ext     bundle-ext      deploy-ext             │
   │   call surface   call surface    call surface           │
   └─────────┼──────────────┼───────────────┼────────────────┘
             │              │               │
             ▼              ▼               ▼
      greentic.adaptive-cards   greentic.hosted-webchat   greentic.aws-eks
      greentic.flows-ygtc-v2    greentic.openshift        greentic.desktop
      greentic.digital-workers  greentic.multi-channel    greentic.cisco-onprem
```

**Repo structure:**

```
greentic-designer-extensions/
├── Cargo.toml                          # workspace
├── rust-toolchain.toml                 # 1.91.0
├── CLAUDE.md
├── README.md
├── ci/local_check.sh
├── docs/
│   ├── concept.md                              # executive summary
│   ├── superpowers/specs/*.md                  # design specs
│   ├── how-to-write-a-design-extension.md
│   ├── how-to-write-a-bundle-extension.md
│   ├── how-to-write-a-deploy-extension.md
│   ├── describe-json-spec.md
│   ├── wit-reference.md
│   ├── capability-registry.md
│   ├── greentic-store-api.openapi.yaml
│   ├── cross-extension-communication.md
│   ├── permissions-and-trust.md
│   └── cli-reference.md
├── wit/
│   ├── extension-base.wit
│   ├── extension-design.wit
│   ├── extension-bundle.wit
│   ├── extension-deploy.wit
│   └── extension-host.wit
├── crates/
│   ├── greentic-ext-contract/
│   ├── greentic-ext-runtime/
│   ├── greentic-ext-cli/
│   └── greentic-ext-testing/
├── reference-extensions/
│   └── adaptive-cards/
└── templates/
    ├── design-kind/
    ├── bundle-kind/
    └── deploy-kind/
```

## 5. Extension contract

### 5.1 `describe.json` — manifest

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.adaptive-cards",
    "name": "Adaptive Cards",
    "version": "1.6.0",
    "summary": "Design and validate Microsoft Adaptive Cards v1.6",
    "description": "Full AdaptiveCards v1.6 support: schemas, host-compat, a11y, LLM prompts, knowledge base.",
    "author": {
      "name": "Greentic",
      "email": "team@greentic.ai",
      "publicKey": "ed25519:AAAAC3NzaC1lZDI1NTE5AAAAI..."
    },
    "license": "MIT",
    "homepage": "https://greentic.ai/extensions/adaptive-cards",
    "repository": "https://github.com/greenticai/greentic-designer-extensions",
    "keywords": ["adaptive-cards", "ui", "microsoft"],
    "icon": "assets/icon.svg",
    "screenshots": ["assets/screenshot-1.png"]
  },
  "engine": {
    "greenticDesigner": ">=0.6.0 <1.0.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "greentic:adaptive-cards/schema",      "version": "1.6.0" },
      { "id": "greentic:adaptive-cards/validate",    "version": "1.0.0" },
      { "id": "greentic:adaptive-cards/transform",   "version": "1.0.0" },
      { "id": "greentic:adaptive-cards/host-compat", "version": "1.0.0" }
    ],
    "required": [
      { "id": "greentic:host/logging", "version": "^1.0.0" }
    ]
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 64,
    "permissions": {
      "network": ["https://api.anthropic.com/*"],
      "secrets": ["OPENAI_API_KEY"],
      "callExtensionKinds": []
    }
  },
  "contributions": { /* kind-specific — see 5.2 */ },
  "signature": {
    "algorithm": "ed25519",
    "publicKey": "ed25519:...",
    "value": "base64..."
  }
}
```

**Required fields per kind:**

| Field | Required for | Notes |
|---|---|---|
| `apiVersion`, `kind`, `metadata.id`, `metadata.version` | all | |
| `engine` | all | Min designer version + runtime version |
| `capabilities.offered` | all | Kind-specific minimum (see 5.2) |
| `capabilities.required` | optional | Host or other extension caps |
| `runtime.component` | all | Path to WASM inside `.gtxpack` |
| `runtime.permissions` | all | Default deny; user prompt on install |
| `contributions` | kind-specific | See 5.2 |
| `signature` | published extensions | Unsigned dev builds allowed with `--trust-policy loose` |

### 5.2 Kind-specific `contributions`

**DesignExtension:**

```json
"contributions": {
  "schemas":   ["schemas/*.json"],
  "prompts":   ["prompts/*.md"],
  "knowledge": ["knowledge/*.json"],
  "tools": [
    { "name": "validate_card", "export": "greentic:extension-design/tools.validate-content" },
    { "name": "optimize_card", "export": "greentic:extension-design/tools.optimize" }
  ],
  "assets": ["assets/"],
  "i18n":   ["i18n/"]
}
```

**BundleExtension:**

```json
"contributions": {
  "recipes": [
    {
      "id": "hosted-webchat",
      "displayName": "Greentic-hosted with WebChat",
      "description": "Fully managed deployment with embedded web chat UI",
      "configSchema": "recipes/hosted-webchat.schema.json",
      "supportedCapabilities": ["greentic:adaptive-cards/*", "greentic:flows/*"]
    },
    {
      "id": "openshift",
      "displayName": "OpenShift on-prem",
      "configSchema": "recipes/openshift.schema.json",
      "supportedCapabilities": ["*"]
    }
  ],
  "assets": ["assets/"],
  "i18n":   ["i18n/"]
}
```

**DeployExtension:**

```json
"contributions": {
  "targets": [
    {
      "id": "aws-eks",
      "displayName": "AWS EKS",
      "credentialSchema": "targets/aws-eks.credentials.schema.json",
      "configSchema": "targets/aws-eks.config.schema.json",
      "iconPath": "assets/aws.svg"
    }
  ],
  "assets": ["assets/"],
  "i18n":   ["i18n/"]
}
```

Validator uses `oneOf` on `kind` to enforce the kind-specific schema:

```json
{
  "oneOf": [
    { "if": { "properties": { "kind": { "const": "DesignExtension" }}},
      "then": { "$ref": "#/$defs/designContributions" }},
    { "if": { "properties": { "kind": { "const": "BundleExtension" }}},
      "then": { "$ref": "#/$defs/bundleContributions" }},
    { "if": { "properties": { "kind": { "const": "DeployExtension" }}},
      "then": { "$ref": "#/$defs/deployContributions" }}
  ]
}
```

### 5.3 Base WIT — `greentic:extension-base@0.1.0`

```wit
package greentic:extension-base@0.1.0;

interface types {
  record extension-identity {
    id: string,
    version: string,
    kind: kind,
  }

  enum kind { design, bundle, deploy }

  record capability-ref {
    id: string,
    version: string,
  }

  record diagnostic {
    severity: severity,
    code: string,
    message: string,
    path: option<string>,
  }

  enum severity { error, warning, info, hint }

  variant extension-error {
    invalid-input(string),
    missing-capability(string),
    permission-denied(string),
    internal(string),
  }
}

interface manifest {
  use types.{extension-identity, capability-ref};
  get-identity: func() -> extension-identity;
  get-offered: func() -> list<capability-ref>;
  get-required: func() -> list<capability-ref>;
}

interface lifecycle {
  use types.{extension-error};
  init: func(config-json: string) -> result<_, extension-error>;
  shutdown: func();
}

world extension-base-world {
  export manifest;
  export lifecycle;
}
```

### 5.4 Host WIT — `greentic:extension-host@0.1.0`

```wit
package greentic:extension-host@0.1.0;

interface logging {
  enum level { trace, debug, info, warn, error }
  log: func(level: level, target: string, message: string);
  log-kv: func(level: level, target: string, message: string,
               fields: list<tuple<string, string>>);
}

interface i18n {
  t:  func(key: string) -> string;
  tf: func(key: string, args: list<tuple<string, string>>) -> string;
}

interface secrets {
  get: func(uri: string) -> result<string, string>;
}

interface broker {
  call-extension: func(
    kind: string,
    target-id: string,
    function: string,
    args-json: string
  ) -> result<string, string>;
}

interface http {
  record request {
    method: string,
    url: string,
    headers: list<tuple<string, string>>,
    body: option<list<u8>>,
  }
  record response {
    status: u16,
    headers: list<tuple<string, string>>,
    body: list<u8>,
  }
  fetch: func(req: request) -> result<response, string>;
}

world extension-host-world {
  import logging;
  import i18n;
  import secrets;
  import broker;
  import http;
}
```

**Permission model (denied by default):**

Extension declares permissions in `describe.json`. Runtime enforces at:
- Network: wasmtime-wasi outgoing-http allowlist per request
- Secrets: `secrets::get` checks URI allowlist
- Broker: `broker::call-extension` checks `callExtensionKinds`

On install, user gets a prompt listing requested permissions:

```
⚠️  Extension "adaptive-cards" v1.6.0 requests:
  - Network: openai.com, anthropic.com
  - Secrets: OPENAI_API_KEY, ANTHROPIC_API_KEY
  - Cross-extension: may call bundle-kind extensions
Install? [y/N]
```

First install only; subsequent updates re-prompt only for new permissions.

## 6. Kind-specific sub-WIT

### 6.1 `greentic:extension-design@0.1.0`

```wit
package greentic:extension-design@0.1.0;

use greentic:extension-base@0.1.0/types.{diagnostic, extension-error};

interface tools {
  record tool-definition {
    name: string,
    description: string,
    input-schema-json: string,
    output-schema-json: option<string>,
  }
  list-tools: func() -> list<tool-definition>;
  invoke-tool: func(name: string, args-json: string) -> result<string, extension-error>;
}

interface validation {
  record validate-result {
    valid: bool,
    diagnostics: list<diagnostic>,
  }
  validate-content: func(content-type: string, content-json: string) -> validate-result;
}

interface prompting {
  record prompt-fragment {
    section: string,
    content-markdown: string,
    priority: u32,
  }
  system-prompt-fragments: func() -> list<prompt-fragment>;
}

interface knowledge {
  record entry-summary { id: string, title: string, category: string, tags: list<string> }
  record entry         { id: string, title: string, category: string, tags: list<string>, content-json: string }

  list-entries: func(category-filter: option<string>) -> list<entry-summary>;
  get-entry: func(id: string) -> result<entry, extension-error>;
  suggest-entries: func(query: string, limit: u32) -> list<entry-summary>;
}

world design-extension {
  import greentic:extension-base@0.1.0/types;
  import greentic:extension-host@0.1.0/logging;
  import greentic:extension-host@0.1.0/i18n;
  import greentic:extension-host@0.1.0/secrets;
  import greentic:extension-host@0.1.0/broker;
  import greentic:extension-host@0.1.0/http;

  export greentic:extension-base@0.1.0/manifest;
  export greentic:extension-base@0.1.0/lifecycle;
  export tools;
  export validation;
  export prompting;
  export knowledge;
}
```

**Designer call-sites:**

| Designer file | Current | After refactor |
|---|---|---|
| `src/ui/prompt_builder.rs` | `adaptive_card_core::prompt::build_system_prompt` | `runtime.aggregate_prompt_fragments(kind=design)` across all loaded design-exts |
| `src/ui/tool_bridge/defs.rs` | Hardcoded 12 tool defs | Dynamic via `runtime.list_tools()` |
| `src/ui/tool_bridge/dispatch.rs` | 12 match arms → `adaptive_card_core::*` | `runtime.invoke_tool(ext_id, name, args)` |
| `src/ui/routes/validate.rs` | Direct `adaptive_card_core::validate` | `runtime.validate_content("adaptive-card", json)` |
| `src/ui/routes/examples.rs` | Direct `knowledge_base.*` | `runtime.design_knowledge().*` (multi-ext aggregator) |
| `src/knowledge.rs` | Wrapper over core | **Deleted** |

### 6.2 `greentic:extension-bundle@0.1.0`

```wit
package greentic:extension-bundle@0.1.0;

use greentic:extension-base@0.1.0/types.{diagnostic, extension-error};

interface recipes {
  record recipe-summary {
    id: string,
    display-name: string,
    description: string,
    icon-path: option<string>,
  }
  list-recipes: func() -> list<recipe-summary>;
  recipe-config-schema: func(recipe-id: string) -> result<string, extension-error>;
  supported-capabilities: func(recipe-id: string) -> result<list<string>, extension-error>;
}

interface bundling {
  record designer-session {
    flows-json: string,
    contents-json: string,
    assets: list<tuple<string, list<u8>>>,
    capabilities-used: list<string>,
  }
  record bundle-artifact {
    filename: string,
    bytes: list<u8>,
    sha256: string,
  }
  validate-config: func(recipe-id: string, config-json: string) -> list<diagnostic>;
  render: func(recipe-id: string, config-json: string, session: designer-session)
    -> result<bundle-artifact, extension-error>;
}

world bundle-extension {
  import greentic:extension-base@0.1.0/types;
  import greentic:extension-host@0.1.0/logging;
  import greentic:extension-host@0.1.0/i18n;
  import greentic:extension-host@0.1.0/broker;

  export greentic:extension-base@0.1.0/manifest;
  export greentic:extension-base@0.1.0/lifecycle;
  export recipes;
  export bundling;
}
```

**Designer call-sites** (new, in `src/ui/routes/wizard*.rs`):

```
POST /api/wizard/recipes               → list across all bundle-exts
GET  /api/wizard/recipes/{ext}/{id}/schema
POST /api/wizard/build                 → runtime.bundle.render(...)
GET  /api/wizard/build/{job_id}        → poll status
```

`orchestrate::cards2pack` and `orchestrate::deployer` are retired once a
bundle-ext reference implementation is shipped (Cycle 3).

### 6.3 `greentic:extension-deploy@0.1.0`

```wit
package greentic:extension-deploy@0.1.0;

use greentic:extension-base@0.1.0/types.{diagnostic, extension-error};

interface targets {
  record target-summary {
    id: string,
    display-name: string,
    description: string,
    icon-path: option<string>,
    supports-rollback: bool,
  }
  list-targets: func() -> list<target-summary>;
  credential-schema: func(target-id: string) -> result<string, extension-error>;
  config-schema: func(target-id: string) -> result<string, extension-error>;
  validate-credentials: func(target-id: string, credentials-json: string) -> list<diagnostic>;
}

interface deployment {
  record deploy-request {
    target-id: string,
    artifact-bytes: list<u8>,
    credentials-json: string,
    config-json: string,
    deployment-name: string,
  }
  enum deploy-status {
    pending, provisioning, configuring, starting, running, failed, rolled-back,
  }
  record deploy-job {
    id: string,
    status: deploy-status,
    message: string,
    endpoints: list<string>,
  }
  deploy: func(req: deploy-request) -> result<deploy-job, extension-error>;
  poll: func(job-id: string) -> result<deploy-job, extension-error>;
  rollback: func(job-id: string) -> result<_, extension-error>;
}

world deploy-extension {
  import greentic:extension-base@0.1.0/types;
  import greentic:extension-host@0.1.0/logging;
  import greentic:extension-host@0.1.0/i18n;
  import greentic:extension-host@0.1.0/secrets;
  import greentic:extension-host@0.1.0/http;

  export greentic:extension-base@0.1.0/manifest;
  export greentic:extension-base@0.1.0/lifecycle;
  export targets;
  export deployment;
}
```

Credentials never persist in designer — passed directly to the extension at
invocation. Extensions decide whether to store via `host::secrets`.

## 7. Runtime

### 7.1 `ExtensionRuntime` skeleton

```rust
pub struct ExtensionRuntime {
    config: RuntimeConfig,
    registries: Vec<Box<dyn ExtensionRegistry>>,
    wasmtime_engine: wasmtime::Engine,
    loaded: ArcSwap<HashMap<ExtensionId, Arc<LoadedExtension>>>,
    capability_registry: ArcSwap<CapabilityRegistry>,
    broker: Broker,
    watcher: notify::RecommendedWatcher,
    events: broadcast::Sender<RuntimeEvent>,
}

pub struct LoadedExtension {
    identity: ExtensionIdentity,
    describe: Arc<DescribeJson>,
    component: wasmtime::component::Component,
    instance_pool: InstancePool,
    kind: ExtensionKind,
    health: ExtensionHealth,
}

pub enum RuntimeEvent {
    ExtensionInstalled(ExtensionId),
    ExtensionUpdated(ExtensionId, PrevVersion),
    ExtensionRemoved(ExtensionId),
    CapabilityRegistryRebuilt,
}
```

**Patterns:**
- `ArcSwap` for lockless reads during hot reload (mirrors `greentic-runner`)
- `InstancePool` to amortize WASM cold start (~10-50 ms) across calls
- `wasmtime::Engine` shared across all extensions; `Component` is per-ext
- File watcher debounced at 500 ms for `~/.greentic/extensions/**`

### 7.2 Capability Registry + matching engine (DX-05, DX-06)

```rust
pub struct CapabilityRegistry {
    offered: HashMap<CapId, Vec<OfferedBinding>>,
    required: HashMap<ExtensionId, Vec<RequiredCap>>,
    resolutions: HashMap<ExtensionId, ResolutionPlan>,
}

pub struct OfferedBinding {
    extension_id: ExtensionId,
    version: semver::Version,
    kind: ExtensionKind,
    export_path: String,
}

pub struct RequiredCap {
    cap_ref: CapabilityRef,
    required_by: ExtensionId,
}

pub struct ResolutionPlan {
    resolved: HashMap<CapId, OfferedBinding>,
    unresolved: Vec<RequiredCap>,
}
```

**Matching rules:**

1. Required `{id, version: "^1.2"}` matches offered `{id, version}` if semver compatible
2. Multiple offerings → highest compatible semver wins (config pin overrides)
3. Unresolved → extension marked `Degraded(reason)`, not crashed
4. Degraded extension's offered caps are **not** registered (avoid cascading
   half-working state)
5. Cycle detection: reject install if A requires from B requires from A

**Degraded UI state:**

```
✅ adaptive-cards v1.6.0    — healthy, offers 4 caps
⚠️  openshift-bundle v0.2.0 — degraded (missing: greentic:flows/v2)
   → Install greentic.flows@^2 to enable
```

### 7.3 Discovery, install, update, uninstall, hot reload (DX-07)

**Startup:**

```
1. Load ~/.greentic/config.toml
2. Scan ~/.greentic/extensions/{design,bundle,deploy}/*/describe.json
3. Parse + JSON Schema validate
4. Verify signatures per trust policy
5. Parallel load (rayon) — wasmtime::Component::from_file
6. Build CapabilityRegistry
7. Run resolution — mark degraded where needed
8. Pre-instantiate N instances per ext
9. Emit Ready event → designer tool_bridge refreshes
```

**Install (`gtdx install adaptive-cards@^1.2`):**

```
1. Resolve via registry (Store / OCI / Local)
2. Fetch describe.json metadata only
3. Engine compat check
4. Cap resolution dry-run — show would-be-degraded
5. Permission prompt (first install only)
6. Fetch .gtxpack artifact
7. Verify Ed25519 signature
8. Stage to ~/.greentic/extensions/{kind}/{name}-{version}.tmp/
9. Atomic rename to ~/.greentic/extensions/{kind}/{name}-{version}/
10. Update registry.json
11. Watcher fires → runtime hot-reloads
```

**Hot reload:**

- `notify` watches extensions dir, debounced 500 ms
- Diff old vs new filesystem state
- For added/modified: load component → build candidate registry → validate → atomic swap
- For removed: drop loaded → rebuild registry → swap
- Failed reload: keep old state (rollback), log error, emit event
- Zero-downtime from designer POV (in-flight requests use old Arc)

**Storage layout:**

```
~/.greentic/
├── config.toml                         # registries, trust, pins
├── credentials.toml                    # per-registry tokens (600 perm)
├── extensions/
│   ├── design/
│   │   └── adaptive-cards-1.6.0/
│   ├── bundle/
│   └── deploy/
├── cache/
│   └── artifacts/{registry}/{name}-{version}.gtxpack
└── registry.json                       # installed index
```

**Config example:**

```toml
[default]
registry = "greentic-store"
trust-policy = "normal"                 # strict | normal | loose

[[registries]]
name = "greentic-store"
url = "https://store.greentic.ai"
token-env = "GREENTIC_STORE_TOKEN"

[[registries]]
name = "my-enterprise"
url = "https://ext.internal.corp"
token-env = "CORP_STORE_TOKEN"

[extensions]
"greentic.adaptive-cards"   = "^1.2"
"greentic.openshift-bundle" = "=0.3.0"
```

**Project-local override:**

```
./my-project/.greentic/extensions/design/my-dev-ext/   # shadows ~/.greentic/
```

Enables dev iteration without reinstall.

### 7.4 Host broker

```rust
impl Broker {
    fn call_extension(
        &self,
        caller: &ExtensionId,
        kind: ExtensionKind,
        target_id: &ExtensionId,
        function: &str,
        args_json: &str,
    ) -> Result<String, BrokerError> {
        // 1. Permission check: caller.describe.permissions.callExtensionKinds includes kind?
        // 2. Target resolution: loaded ext exists with that id + kind?
        // 3. Cap compatibility: function exists in target's WIT?
        // 4. Max-depth check: current call stack < limit (default 8)
        // 5. Deadline check: elapsed < limit (default 10s)
        // 6. Instantiate + call
    }
}
```

Extensions compose without knowing about each other. If dependency missing,
caller degrades gracefully (warning, not crash).

## 8. CLI — `gtdx`

```
gtdx new <kind> <name>            Scaffold NEW project in ./<name>/
gtdx init <kind>                  Initialize in existing project (CWD)
gtdx migrate <crate-path>         Wrap existing Rust lib as extension (v1.1)
gtdx build [--release]            Build WASM + package .gtxpack
gtdx validate [path]              Static check
gtdx login [--registry X]         Auth, save token
gtdx publish [--registry X]       Upload .gtxpack
gtdx search <query> [--kind]      Registry search
gtdx info <name>                  Registry metadata + versions
gtdx install <name>[@ver]         Install from registry or local path
gtdx uninstall <name>             Remove
gtdx list                         Installed exts + health
gtdx enable/disable <name>        Toggle
gtdx update [name]                Update to latest compatible
gtdx doctor                       Diagnose unresolved, degraded, conflicts
gtdx cache clean                  Purge artifact cache
gtdx registries {list|add|remove} Manage registries
gtdx config {get|set}             Read/write config
```

`gtc extension ...` is a thin alias (match greentic convention). Primary
binary is standalone `gtdx`.

**`gtdx init` behavior for existing project:**

1. Scan CWD — detect Cargo workspace/package, existing `src/lib.rs`, etc.
2. Interactive wizard (or `--non-interactive` flags) — kind, id, summary, caps
3. Apply patches **additively** (never overwrite):
   - `Cargo.toml`: add `[lib] crate-type = ["cdylib"]`, add deps
   - Add `wit/world.wit`, `describe.json`, `.cargo/config.toml`
   - Add Makefile, `.gitignore` entries
4. `src/lib.rs`: if present, add separate `src/ext_exports.rs` + instruct
   user to `mod ext_exports;`. Never modify existing user code.

## 9. Greentic Store integration

### 9.1 Registry trait

```rust
trait ExtensionRegistry {
    fn search(&self, query: SearchQuery) -> Result<Vec<ExtensionSummary>>;
    fn metadata(&self, name: &str, version: &str) -> Result<ExtensionMetadata>;
    fn fetch(&self, name: &str, version: &str) -> Result<ExtensionArtifact>;
    fn publish(&self, artifact: ExtensionArtifact, auth: &AuthToken) -> Result<()>;
}
```

Implementations shipped in v1:

| Registry | Purpose |
|---|---|
| `LocalFilesystemRegistry` | Dev / air-gapped |
| `GreenticStoreRegistry` | Default for end-users — HTTPS to store.greentic.ai |
| `OciRegistry` | Enterprise / mirror existing `.gtpack` pattern |

### 9.2 Store API — OpenAPI 3.1 contract

Ship `docs/greentic-store-api.openapi.yaml` frozen at v1 tag. Module 7 team
implements server to match.

Endpoints:

```
GET  /api/v1/extensions?kind=design&capability=X&q=whatsapp&page=1
GET  /api/v1/extensions/{name}
GET  /api/v1/extensions/{name}/{version}
GET  /api/v1/extensions/{name}/{version}/artifact
POST /api/v1/extensions              (multipart: .gtxpack + signature, auth)
POST /api/v1/auth/login              { email, password } → { token }
GET  /api/v1/search?q=X&kind=design
```

### 9.3 Upload model — store hosts WASM directly

Decision: v1 Store hosts the `.gtxpack` artifact, not just metadata.

**Rationale:** centralized search/analytics, yank + security recall,
offline capability, single source of truth. WASM artifacts are small
(1-10 MB typical) so storage cost is manageable.

OCI-backed extensions (store metadata, WASM in OCI) are an escape-hatch for
enterprise in v1.1 if demand exists. Not in v1.

### 9.4 Auth + trust

Developer onboarding:

```
gtdx login                  # authenticate, auto-generate Ed25519 keypair,
                            # upload pub key to account profile
gtdx publish                # sign .gtxpack + describe.json with priv key,
                            # POST to Store, Store verifies against account's pub key
```

Trust policies (configurable):

- `strict` — only signed + store-countersigned
- `normal` — signed (default; matches cargo)
- `loose` — unsigned allowed (dev mode, warn)

End-user install:

```
gtdx install adaptive-cards
  → fetch describe.json from Store
  → cap resolution dry-run
  → download artifact
  → verify Ed25519 signature
  → verify store countersignature (strict mode only)
  → atomic install
```

### 9.5 CI/CD example for developers

```yaml
on:
  release:
    types: [published]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-wasip2
      - run: cargo install gtdx
      - run: gtdx build
      - run: gtdx publish --token ${{ secrets.GREENTIC_STORE_TOKEN }}
```

## 10. Reference implementation — Adaptive Cards (DX-08)

### 10.1 Crate structure

```
reference-extensions/adaptive-cards/
├── Cargo.toml                  # depends on adaptive-card-core git
├── describe.json
├── wit/world.wit
├── src/lib.rs                  # thin WIT export layer
├── schemas/adaptive-card-v1.6.json
├── prompts/
│   ├── rules.md                # extracted from adaptive_card_core::prompt
│   └── examples.md
├── knowledge/                   # empty in v1; curated separately
├── assets/
│   └── icon.svg
└── i18n/en.json
```

**Cargo.toml:**

```toml
[package]
name = "greentic-adaptive-cards-extension"
version = "1.6.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.35"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
adaptive-card-core = { git = "https://github.com/greenticai/greentic-adaptive-card-mcp" }

[package.metadata.component]
package = "greentic:adaptive-cards-extension"
```

### 10.2 `describe.json`

```json
{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.adaptive-cards",
    "version": "1.6.0",
    "summary": "Design and validate Microsoft Adaptive Cards v1.6",
    "author": { "name": "Greentic", "publicKey": "ed25519:..." },
    "license": "MIT",
    "repository": "https://github.com/greenticai/greentic-designer-extensions"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "greentic:adaptive-cards/schema",      "version": "1.6.0" },
      { "id": "greentic:adaptive-cards/validate",    "version": "1.0.0" },
      { "id": "greentic:adaptive-cards/transform",   "version": "1.0.0" },
      { "id": "greentic:adaptive-cards/host-compat", "version": "1.0.0" }
    ],
    "required": [
      { "id": "greentic:host/logging", "version": "^1.0.0" }
    ]
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 64,
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] }
  },
  "contributions": {
    "schemas":   ["schemas/adaptive-card-v1.6.json"],
    "prompts":   ["prompts/rules.md", "prompts/examples.md"],
    "knowledge": ["knowledge/"],
    "tools": [
      { "name": "validate_card",      "export": "greentic:extension-design/validation.validate-content" },
      { "name": "analyze_card",       "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "check_accessibility","export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "optimize_card",      "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "transform_card",     "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "template_card",      "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "data_to_card",       "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "list_examples",      "export": "greentic:extension-design/knowledge.list-entries" },
      { "name": "get_example",        "export": "greentic:extension-design/knowledge.get-entry" },
      { "name": "suggest_layout",     "export": "greentic:extension-design/knowledge.suggest-entries" }
    ]
  }
}
```

### 10.3 Designer refactor — file-by-file

| File | Change |
|---|---|
| `Cargo.toml` | -`adaptive-card-core` +`greentic-ext-runtime` |
| `src/main.rs` | Init `ExtensionRuntime`, discover, store in `AppState` |
| `src/ui/state.rs` | Replace `knowledge_base` with `runtime: Arc<ExtensionRuntime>` |
| `src/ui/prompt_builder.rs` | Delegate to `runtime.aggregate_prompt_fragments("design")` |
| `src/ui/tool_bridge/defs.rs` | Static TOOL_DEFS → `runtime.list_tools()` (cached) |
| `src/ui/tool_bridge/dispatch.rs` | 12 match arms → `runtime.invoke_tool(ext_id, name, args)` |
| `src/ui/routes/validate.rs` | Direct call → `runtime.validate_content("adaptive-card", json)` |
| `src/ui/routes/examples.rs` | Direct KB → `runtime.design_knowledge().{list,get,suggest}()` |
| `src/knowledge.rs` | **Delete** |
| `src/orchestrate/*` | **Keep** — retired later by bundle-ext (Cycle 3) |

**Estimated LOC impact:** ~800 modified, ~200 deleted in designer; ~400 new
in AC extension crate.

### 10.4 Migration strategy — two-phase, zero downtime

**Phase A (1-2 days) — parallel paths:**

1. Add `greentic-ext-runtime` dep, keep `adaptive-card-core`
2. Bootstrap runtime in `main.rs`; routes still use direct core
3. Build AC extension standalone; validate with `gtdx` harness
4. Add env var `DESIGNER_USE_EXTENSIONS=1` that switches routes to runtime
5. Verify both paths produce identical output on test corpus

**Phase B (1 day) — cutover:**

1. Flip default to runtime path
2. Delete `adaptive-card-core` dep + old code paths
3. Ship AC extension to Store
4. Update startup: if no AC ext installed, show helpful error pointing to
   `gtdx install greentic.adaptive-cards`
5. Bundle AC extension in designer installer (first-run convenience —
   `include_bytes!` fallback, auto-install on first startup if missing)

## 11. Case study — flow-design extension

Demonstrates generalizability of the design-extension pattern. Not shipped
in v1 but informs the contract design.

**Store id:** `greentic.flows-ygtc-v2`
**Kind:** DesignExtension
**Content-type:** `flow-ygtc-v2`

Contributions:
- `schemas/ygtc-v2.json` (derived from `greentic-flow`)
- `prompts/authoring-rules.md`, `prompts/patterns.md`
- `knowledge/onboarding.json`, `knowledge/survey.json`, `knowledge/agentic-loop.json`
- Tools: `validate_flow`, `list_step_types`, `generate_skeleton`, `add_step`,
  `wire_branches`, `optimize_flow`, `suggest_flow`, `session_wait_guide`

**Cross-ext broker call** — flow-ext validates embedded cards via AC ext:

```rust
// inside flows-ygtc-v2.wasm
fn validate_content(ct: String, json: String) -> ValidateResult {
    let flow: Flow = serde_yaml::from_str(&json).unwrap();
    let mut diagnostics = schema_check(&flow);
    for step in flow.steps.iter().filter(|s| s.ty == "card") {
        let args = json!({ "content-type": "adaptive-card", "content-json": step.payload });
        match host::broker::call_extension("design", "greentic.adaptive-cards",
                                            "validate-content", &args.to_string()) {
            Ok(res) => merge_diagnostics(&mut diagnostics, res),
            Err(_) => diagnostics.push(Diagnostic::warning(
                "card-not-validated",
                "AdaptiveCards extension not installed; embedded cards not checked"
            )),
        }
    }
    ValidateResult { valid: diagnostics.is_empty(), diagnostics }
}
```

Graceful degradation when dependency missing.

**Tool aggregation rule:** when two design-exts offer tools with the same
name, designer resolves by content-type dispatch (polymorphic tools like
`validate_content` take content-type arg) or by prefixing with ext id
(`flows_ygtc_v2__add_step`).

## 12. Hand-offs

**→ Module 8 (Application Pack):** bundle-ext returns `bundle-artifact
{filename, bytes, sha256}`. Bytes are opaque to ext-runtime — format is
Module 8's domain. Contract freezes `.apack` extension + SHA256 required.

**→ Module 7 (Greentic Store):** ship `docs/greentic-store-api.openapi.yaml`.
Store server lives in separate repo, implements the contract. Contract
locked at v1 tag.

**→ Module 5 (Bundle Extension reference impls):** contract ships v1, no
reference impl in v1 scope. Cycle 3 brainstorm + plan separately.

**→ Module 6 (Deploy Extension reference impls):** contract ships v1, no
reference impl in v1. Cycle 4.

**→ `greentic-adaptive-card-mcp`:** `adaptive-card-core` stays put; AC
extension crate depends on it via git. No disruption to MCP server.

## 13. Testing strategy

**Unit tests:**
- `greentic-ext-contract` — schema validation, semver, WIT type roundtrip
- `greentic-ext-runtime` — discovery, resolution, broker, hot reload diff
- `greentic-ext-cli` — command parsing, config, atomic file ops

**Integration tests** (`tests/` in runtime crate):
- In-memory `TestRegistry` + 2-3 mock extensions
- install → discovery → resolution → tool invocation → expected output
- Broker call between extensions
- Hot reload (FS event → state swap)
- Permission denial paths
- Degraded state (missing required cap)

**WASM component tests** (in reference-extensions/adaptive-cards/):
- Native cargo tests for `adaptive-card-core` logic (already exist)
- WASM-level smoke: wasmtime run + fixture input → JSON snapshot
- Reuse 35 existing fixtures from `adaptive-card-mcp` repo

**Designer integration** (in `greentic-designer`):
- Golden path: start → install AC ext → POST /api/chat → snapshot
- Prompt aggregation snapshot
- Tool list snapshot (detect cap shuffling)

**CI** (`ci/local_check.sh` + GitHub Actions):
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- WASM build: `cargo component build --release`
- `gtdx validate` on built `.gtxpack`

## 14. Documentation deliverables (DX-04)

Format: mdBook in `docs/`, deployed to `designer-ext.greentic.ai`.

| Doc | Audience | Priority |
|---|---|---|
| `how-to-write-a-design-extension.md` | External devs (Osora, Telco X) | P0 — needed by week 2 |
| `how-to-write-a-bundle-extension.md` | External devs | P1 — needed before Module 5 Cycle 3 |
| `how-to-write-a-deploy-extension.md` | External devs | P1 — needed before Module 6 Cycle 4 |
| `describe-json-spec.md` | All | P0 |
| `wit-reference.md` | All | P0 |
| `capability-registry.md` | Contributors | P1 |
| `greentic-store-api.openapi.yaml` | Module 7 team | P0 |
| `cross-extension-communication.md` | Advanced devs | P1 |
| `permissions-and-trust.md` | All | P1 |
| `cli-reference.md` | All | P1 |
| `migration-from-adaptive-card-core.md` | Designer contributors | P2 |

## 15. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| wasmtime Component Model API churn | M | M | Pin wasmtime, upgrade in dedicated PRs |
| Store API contract drift vs Module 7 | M | H | OpenAPI freeze at v1, breaking change = major bump |
| AC extraction regresses designer UX | M | H | Two-phase migration, snapshot tests |
| Permission prompt UX annoying | H | M | First-install only, `--yes` for automation |
| Ed25519 key management friction | M | M | `gtdx login` auto-generates keypair |
| Ecosystem adoption slow | L | H | Ship AC + Store + docs; sync Osora / Telco X early (CO-01/02) |
| WASM overhead on hot path | L | M | Instance pool; benchmark; optimize if p50 > 10 ms |
| Broker overuse causes coupling | M | L | Permission gate + max-depth + docs warn |

## 16. Timeline — v1 (Cycle 0 + Cycle 1)

~9 weeks with 1 full-time dev, ~5-6 weeks with 2 devs or subagent-driven
pattern.

| Phase | Duration | Deliverable |
|---|---|---|
| Repo scaffolding + CI | 2 d | workspace, CLAUDE.md, ci green |
| Base WIT + contract types | 3 d | `greentic-ext-contract` crate |
| Runtime skeleton + wasmtime | 4 d | discovery, load, pool, broker stubs |
| Capability Registry + matching | 3 d | DX-05, DX-06 |
| Install/update lifecycle | 3 d | CLI wire-up, atomic ops, hot reload |
| CLI `gtdx` commands | 4 d | new/init/build/publish/install/list/doctor |
| Local + Store registry impls | 3 d | OpenAPI client; Store mocked via wiremock |
| Design sub-WIT + designer refactor Phase A | 3 d | two-path code, feature flag |
| AC extension crate (DX-08) | 3 d | WIT wrapper, describe.json, `.gtxpack` |
| Designer Phase B cutover | 2 d | flip default, delete old, bundled fallback |
| Bundle + Deploy sub-WIT contracts | 2 d | schema + WIT only |
| Store OpenAPI spec | 2 d | frozen reference |
| Docs (DX-04 + related) | 4 d | tutorial-heavy |
| Integration tests + fixtures | 3 d | e2e coverage |
| CI polish + release scripts | 2 d | husky, publish.yml, dependabot |
| Buffer / review / iteration | 3 d | — |

**Total:** ~44 working days ≈ 9 weeks.

### 16.1 Coordination with external teams (Module 9 — CO-01, CO-02)

- **Week 1-2**: Ship contract crate + describe.json spec + WIT + draft
  `how-to-write-a-design-extension.md`. Publish to GitHub (no runtime yet).
  Osora + Telco X can begin scaffolding manually using `cargo-component`.
- **Week 3**: CO-01 + CO-02 sync meetings. Present contract. Feedback.
  Adjust before freezing.
- **Week 4-5**: Runtime + CLI ready. Osora / Telco X can `gtdx validate`.
- **Week 6-8**: AC extraction + docs polish. Test install cycle.
- **Week 9**: Release v1. Osora / Telco X ship extensions against stable v1.

## 17. Decision log

| ID | Decision | Status |
|---|---|---|
| D-01 | 3 extension kinds (design, bundle, deploy) separate, shared foundation | Confirmed (matches Maarten quotes + diagram) |
| D-02 | Repo name `greentic-designer-extensions` (plural), covers all 3 kinds | Confirmed |
| D-03 | Extensions are WASM components targeting `wasm32-wasip2` | Confirmed |
| D-04 | `describe.json` k8s-style (apiVersion/kind/metadata + kind-specific) | Confirmed |
| D-05 | Permission model: declared in describe.json, prompt on install, default deny | Confirmed |
| D-06 | Host broker for cross-ext calls, permission-gated | Confirmed |
| D-07 | v1 Store hosts WASM artifact directly (not just metadata) | Confirmed |
| D-08 | OCI hybrid escape-hatch deferred to v1.1 | Confirmed |
| D-09 | `ArcSwap` for hot reload (match `greentic-runner` pattern) | Confirmed |
| D-10 | Pre-instantiated instance pool for WASM cold start | Confirmed |
| D-11 | CLI `gtdx` standalone binary, `gtc extension` alias | Confirmed |
| D-12 | Project-local `.greentic/extensions/` shadows user-global for dev | Confirmed |
| D-13 | Two-phase AC migration (parallel path then cutover) | Confirmed |
| D-14 | Bundled AC fallback in designer installer for first-run UX | Confirmed |
| BX-01 | Design + Bundle kept separate (not merged) | Recommended, needs Maarten final sign-off |
| BA-01 | `build-answer.json` vs `pack-answer` naming | Deferred (does not affect this spec) |
| DX-01 | Final ~25 minimum Designer capabilities list | Deferred (working session with Maarten) |

## 18. Appendix A — extension author workflow

1. `gtdx new design greentic.my-extension`
2. Edit `describe.json`, `wit/world.wit`, `src/lib.rs`
3. Add `schemas/`, `prompts/`, `knowledge/` as needed
4. `gtdx build` → produces `.gtxpack`
5. `gtdx validate` → static checks
6. `gtdx install ./target/*.gtxpack` → local test
7. `gtdx publish` → upload to Store
8. Users run `gtdx install greentic.my-extension` to consume

## 19. Appendix B — current designer state for reference

From `greentic-designer/CLAUDE.md` v0.5.0:
- 12 LLM tools hardcoded in `src/ui/tool_bridge/`
- `adaptive-card-core` as direct Rust dep
- `orchestrate::cards2pack` and `orchestrate::deployer` subprocess-based
- Wizard pipeline `/api/wizard/*` for deployment
- Knowledge base ships empty; curated separately
- Multi-session persistence via IndexedDB in frontend

Post-refactor:
- 0 tools hardcoded; all from extensions
- No `adaptive-card-core` direct dep
- `orchestrate::*` retired by bundle-ext (Cycle 3)
- Wizard pipeline routes to bundle-ext + deploy-ext
- Knowledge base is aggregated across extensions

---

End of document.
