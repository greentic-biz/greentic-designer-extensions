# greentic-designer-extensions

WASM-based extension system for [Greentic Designer](https://github.com/greentic-biz/greentic-designer).
Teaches the designer new capabilities at runtime through signed
WebAssembly Component Model artifacts (`.gtxpack`) distributed via the
Greentic Store.

Three extension kinds share one foundation:

| Kind | Teaches the designer to... | Example |
|---|---|---|
| **design-extension** | Author content (cards, flows, digital workers, telco-x schemas) | `greentic.adaptive-cards`, `greentic.flows-ygtc-v2` |
| **bundle-extension** | Package designer output into deployable Application Packs | `greentic.hosted-webchat`, `greentic.openshift-bundle` |
| **deploy-extension** | Ship Application Packs to a target environment | `greentic.aws-eks`, `greentic.cisco-onprem` |

---

## Status

| Milestone | Tag | What's in it |
|---|---|---|
| Foundation | [`v0.1.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.1.0) | WIT contracts, contract crate, runtime core, capability registry, broker, hot reload |
| CLI + Registry | [`v0.2.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.2.0) | `gtdx` 11 subcommands · 3 registry impls (Local / Store / OCI) · install lifecycle · OpenAPI spec |
| AC Reference Extension | [`v0.3.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.3.0) | First canonical design-extension shipping as `greentic.adaptive-cards@1.6.0` |
| Documentation | [`v0.4.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.4.0) | 11 docs (~3,600 lines): references, tutorials, guides |
| Runtime WASM Dispatch | [`v0.5.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.5.0) | Wasmtime Linker + 5 host imports + `invoke_tool` working end-to-end |

End-to-end proof: `cargo test -p greentic-ext-runtime --test ac_invoke` loads the
real `.gtxpack`, instantiates the WASM Component, and gets back live
`adaptive-card-core` output (a11y score, schema errors, host-compat).

---

## Architecture

```
                ┌──────────────────────────────────────────────┐
                │   Greentic Store  (store.greentic.ai)         │
                │   Developers upload · end-users discover      │
                └────────────────────────┬──────────────────────┘
                                         │ HTTPS / OpenAPI
                                         ▼
                ┌──────────────────────────────────────────────┐
                │            greentic-ext-runtime               │
                │  Registry trait — Store · OCI · Local         │
                │  Wasmtime Component loader + Linker           │
                │  Capability registry (semver matching)        │
                │  Host broker (permission + depth gates)       │
                │  Debounced filesystem watcher (hot reload)    │
                └────────────────────────┬──────────────────────┘
                                         │
                ┌────────────────────────▼──────────────────────┐
                │          Greentic Designer (consumer)         │
                │  Chat — design-ext tools                      │
                │  "Next" wizard — bundle-ext recipes           │
                │  Deploy wizard — deploy-ext targets           │
                └────────────────────────┬──────────────────────┘
                                         │
                              gtdx CLI for users
                              cargo binstall greentic-ext-cli
```

---

## Install `gtdx`

From source (private repo today; `cargo binstall` once published):

```bash
git clone git@github.com:greentic-biz/greentic-designer-extensions.git
cd greentic-designer-extensions
cargo install --path crates/greentic-ext-cli --locked
```

Verify:

```bash
gtdx version
gtdx --help
```

---

## Quickstart

### 1. Build the AC reference extension

```bash
./reference-extensions/adaptive-cards/build.sh
# → reference-extensions/adaptive-cards/greentic.adaptive-cards-1.6.0.gtxpack (~1.1 MB)
```

### 2. Validate the manifest

```bash
gtdx validate reference-extensions/adaptive-cards
# ✓ reference-extensions/adaptive-cards/describe.json valid
```

### 3. Install into your local home

```bash
gtdx install ./reference-extensions/adaptive-cards/greentic.adaptive-cards-1.6.0.gtxpack \
  -y --trust loose
# ✓ installed greentic.adaptive-cards@1.6.0
```

Files land at `~/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/`.

### 4. List + diagnose

```bash
gtdx list
# [design]
#   greentic.adaptive-cards@1.6.0  Design and validate Microsoft Adaptive Cards v1.6

gtdx doctor
# ✓ ~/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/describe.json
# 1 total, 0 bad
```

### 5. Search the store (when live)

```bash
gtdx search digital-workers --kind design
gtdx info greentic.digital-workers
gtdx install greentic.digital-workers@^0.3
```

---

## Repository Layout

```
greentic-designer-extensions/
├── wit/                              # WIT packages (single source of truth)
│   ├── extension-base.wit            # Shared types, manifest, lifecycle
│   ├── extension-host.wit            # Host services (logging, i18n, secrets, broker, http)
│   ├── extension-design.wit          # Design-extension interfaces
│   ├── extension-bundle.wit          # Bundle-extension interfaces
│   ├── extension-deploy.wit          # Deploy-extension interfaces
│   └── runtime-side.wit              # Host-side world (bindgen target)
│
├── crates/
│   ├── greentic-ext-contract/        # Types, describe.json schema, signatures (15 tests)
│   ├── greentic-ext-runtime/         # Wasmtime loader, capability registry, broker (29 tests)
│   ├── greentic-ext-registry/        # 3 registry impls + lifecycle (14 tests)
│   ├── greentic-ext-cli/             # `gtdx` binary, 11 subcommands (5 e2e tests)
│   ├── greentic-ext-testing/         # Test utilities for extension authors
│   └── _wit-lint/                    # WIT parser lint test (1 test)
│
├── reference-extensions/
│   └── adaptive-cards/               # First canonical design-extension
│       ├── describe.json
│       ├── src/lib.rs                # WIT exports → adaptive-card-core
│       ├── schemas/, prompts/, knowledge/, i18n/
│       └── build.sh                  # cargo component build → .gtxpack
│
├── docs/                             # 11 docs (~3,600 lines), see docs/README.md
└── ci/local_check.sh                 # fmt + clippy + test + release build
```

**Stats:** 64 tests passing · 5 milestones tagged · ~80 commits · 1 working WASM Component

---

## How Adaptive Cards reference extension works

The `greentic.adaptive-cards@1.6.0` extension is built as a `cdylib` targeting
`wasm32-wasip1` via `cargo-component`. Its 8 tools delegate to
[`adaptive-card-core`](https://github.com/greentic-biz/greentic-adaptive-card-mcp)
(MIT, embedded as a git dep):

| Tool | Backed by |
|---|---|
| `validate_card` | `core::validate_card` (schema v1.6 + a11y + optional host compat) |
| `analyze_card` | `core::analyze_card` (CardAnalysis: elements, actions, depth, dup IDs) |
| `check_accessibility` | `core::check_accessibility` (WCAG-style score 0-100) |
| `optimize_card` | `core::optimize_card` (a11y / perf / modernize transforms) |
| `transform_card` | `core::transform_card` (version downgrade + host adapt) |
| `template_card` | `core::template_card` (`${expr}` bindings + sample data) |
| `data_to_card` | `core::data_to_card` (table / factset / list / chart) |
| `check_host_compat` | `core::check_compatibility` (HostCompatReport) |

Live invocation example (from the runtime e2e test):

```jsonc
// runtime.invoke_tool("greentic.adaptive-cards", "validate_card",
//                     r#"{"card":{"type":"AdaptiveCard","version":"1.6"}}"#)
{
  "valid": true,
  "accessibility": { "score": 95, "issues": [{"rule": "missing-speak", ...}] },
  "card_version": "1.6",
  "schema_errors": [],
  "suggestions": ["[a11y/missing-speak] Add a 'speak' field at the root..."]
}
```

---

## Integration with greentic-designer

Designer (separate repo) consumes this extension system as a Cargo dependency:

```toml
# greentic-designer/Cargo.toml
[dependencies]
greentic-ext-runtime  = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.5.0" }
greentic-ext-contract = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.5.0" }
```

Then in `src/main.rs`:

```rust
use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths, discovery};

let home = std::env::var("GREENTIC_HOME")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|_| dirs::home_dir().unwrap().join(".greentic"));

let mut runtime = ExtensionRuntime::new(
    RuntimeConfig::from_paths(DiscoveryPaths::new(home.clone()))
)?;

for kind in ["design", "bundle", "deploy"] {
    for ext_dir in discovery::scan_kind_dir(&home.join("extensions").join(kind))? {
        runtime.register_loaded_from_dir(&ext_dir).ok();
    }
}

let runtime = std::sync::Arc::new(runtime);
let app_state = AppState { runtime, /* existing */ };
```

In Axum handlers (or anywhere) replace direct `adaptive_card_core::*` calls with:

```rust
let result_json = state.runtime.invoke_tool(
    "greentic.adaptive-cards",
    "validate_card",
    &args_json,
)?;
```

For first-run UX, designer can `include_bytes!` the bundled
`greentic.adaptive-cards-1.6.0.gtxpack` and auto-install on startup if no AC
extension is detected. See
[`docs/superpowers/plans/2026-04-17-docs-and-designer-refactor.md`](docs/superpowers/plans/2026-04-17-docs-and-designer-refactor.md)
Part B for the full migration plan.

---

## Documentation

Full index at [docs/README.md](docs/README.md). Quick links:

### Reference
| Document | Description |
|----------|-------------|
| [`describe-json-spec.md`](docs/describe-json-spec.md) | `describe.json` v1 — every field, every kind, with examples |
| [`wit-reference.md`](docs/wit-reference.md) | All 5 WIT packages + interfaces with type signatures |
| [`capability-registry.md`](docs/capability-registry.md) | Semver matching, degraded state, cycle detection |
| [`cli-reference.md`](docs/cli-reference.md) | All 11 `gtdx` subcommands with examples |

### Tutorials
| Document | Audience |
|----------|----------|
| [`how-to-write-a-design-extension.md`](docs/how-to-write-a-design-extension.md) | External devs (Osora, Telco X) |
| [`how-to-write-a-bundle-extension.md`](docs/how-to-write-a-bundle-extension.md) | Devs adding deployment recipes |
| [`how-to-write-a-deploy-extension.md`](docs/how-to-write-a-deploy-extension.md) | Devs adding cloud / on-prem targets |

### Guides
| Document | Description |
|----------|-------------|
| [`cross-extension-communication.md`](docs/cross-extension-communication.md) | Host broker, permissions, composition |
| [`permissions-and-trust.md`](docs/permissions-and-trust.md) | Trust policies, Ed25519 signing |

### Architecture
| Document | Description |
|----------|-------------|
| [`concept.md`](docs/concept.md) | Executive summary for stakeholders |
| [Full design spec](docs/superpowers/specs/2026-04-17-designer-extension-system-design.md) | The complete technical design |
| [Implementation plans](docs/superpowers/plans/) | Per-milestone task breakdown |
| [Greentic Store API](docs/greentic-store-api.openapi.yaml) | OpenAPI 3.1 contract for Module 7 server |

---

## Building from source

Prerequisites: Rust 1.94, `cargo-component`, `wasm32-wasip1` + `wasm32-wasip2` targets.

```bash
rustup install 1.94.0
rustup component add rustfmt clippy --toolchain 1.94.0
rustup target add wasm32-wasip1 wasm32-wasip2 --toolchain 1.94.0
cargo install cargo-component --locked --version '^0.20'
```

Build + test the host-side workspace (excludes the wasm cdylib):

```bash
bash ci/local_check.sh
```

Build the AC reference extension as a WASM Component + package as `.gtxpack`:

```bash
./reference-extensions/adaptive-cards/build.sh
```

Run the end-to-end WASM dispatch test (loads the `.gtxpack`, invokes
`validate_card`, asserts on the real `adaptive-card-core` output):

```bash
cargo test -p greentic-ext-runtime --test ac_invoke -- --nocapture
```

---

## CI auth setup (one-time, repo admin)

`reference-extensions/adaptive-cards` depends on `adaptive-card-core` from
the private `greentic-biz/greentic-adaptive-card-mcp` repo. CI runners need
auth to fetch it. The `greentic-biz` org disables deploy keys, so we use a
**fine-grained Personal Access Token**.

1. Generate a fine-grained PAT at
   https://github.com/settings/personal-access-tokens/new:
   - **Resource owner**: `greentic-biz`
   - **Repository access**: only select `greentic-biz/greentic-adaptive-card-mcp`
   - **Repository permissions**: `Contents: Read-only`
   - Expiration: long-lived (e.g. 1 year — set a calendar reminder to rotate)

2. On this repo:
   - https://github.com/greentic-biz/greentic-designer-extensions/settings/secrets/actions/new
   - Name: `AC_CORE_PAT`
   - Value: paste the token

3. Re-run the failing CI job from the PR page.

The workflow uses `git config --global url.insteadOf` to rewrite the
`ssh://git@github.com/greentic-biz/...` URL Cargo sees into an HTTPS
URL with the PAT embedded, so Cargo can fetch the dep without further
configuration.

**Alternative**: make `greentic-biz/greentic-adaptive-card-mcp` public —
it's MIT licensed already — drop the auth step from the workflow, and
no secret is needed.

---

## What's next (roadmap)

- **Plan 4B** — Designer refactor: drop `adaptive-card-core` direct dep from
  `greentic-designer`, consume the runtime, bundled fallback for first-run
  UX. Cross-repo work in
  [`greentic-designer`](https://github.com/greentic-biz/greentic-designer).
- **Plan 5** — Bundle extension reference impls (`greentic.hosted-webchat`,
  `greentic.openshift`).
- **Plan 6** — Deploy extension reference impls (`greentic.desktop`,
  `greentic.aws-eks`, ...).
- **Plan 7** — `greentic-store-server` (Greentic Store HTTP backend) — see
  the OpenAPI spec for the contract.
- **AC v2** — Integrate `adaptive-card-core` even more deeply (richer
  knowledge base, transform rule expansions).

---

## Contributing

1. Read [CLAUDE.md](CLAUDE.md) for repo conventions before starting.
2. Pick an issue or open one to discuss.
3. Branch off `main`. Run `bash ci/local_check.sh` locally — it must be
   green before pushing.
4. Open a PR. CI must be green; address review comments; squash on merge.

Conventions:
- Rust 1.94, edition 2024
- Max 500 lines per source file
- English-only in source / commits / log messages / docs
- Conventional commit prefixes: `feat:`, `fix:`, `docs:`, `ci:`, `chore:`,
  `test:`, `refactor:`
- No Claude / AI co-author attribution

---

## License

MIT — see [LICENSE](LICENSE).

`adaptive-card-core` (consumed as a git dep) is also MIT — see
[greentic-biz/greentic-adaptive-card-mcp](https://github.com/greentic-biz/greentic-adaptive-card-mcp).
