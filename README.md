# greentic-designer-extensions

WASM-based extension system for [Greentic Designer](https://github.com/greentic-biz/greentic-designer).
Teaches the designer new capabilities at runtime through signed
WebAssembly Component Model artifacts (`.gtxpack`) distributed via the
Greentic Store.

Three extension kinds share one foundation:

| Kind | Teaches the designer to... | Example reference impls |
|---|---|---|
| **design-extension** | Author content (cards, flows, digital workers, telco-x schemas) | [`greentic.adaptive-cards`](https://github.com/greentic-biz/greentic-adaptive-card-mcp), `greentic.flows-ygtc-v2` |
| **bundle-extension** | Package designer output into deployable Application Packs | `greentic.hosted-webchat`, `greentic.openshift-bundle` |
| **deploy-extension** | Ship Application Packs to a target environment | `greentic.aws-eks`, `greentic.cisco-onprem` |

Reference extensions live with their domain library — e.g. the AC
extension lives in
[`greentic-biz/greentic-adaptive-card-mcp`](https://github.com/greentic-biz/greentic-adaptive-card-mcp)
next to `adaptive-card-core`. This repo ships the **infrastructure**:
the WIT contract, runtime, capability engine, registry clients, and
`gtdx` CLI. It is **domain-agnostic** by design.

---

## Status

| Milestone | Tag | What's in it |
|---|---|---|
| Foundation | [`v0.1.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.1.0) | WIT contracts, contract crate, runtime core, capability registry, broker, hot reload |
| CLI + Registry | [`v0.2.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.2.0) | `gtdx` 11 subcommands · 3 registry impls (Local / Store / OCI) · install lifecycle · OpenAPI spec |
| AC Reference Extension (since moved) | [`v0.3.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.3.0) | First canonical design-extension shipping as `greentic.adaptive-cards@1.6.0` (now lives in `greentic-adaptive-card-mcp`) |
| Documentation | [`v0.4.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.4.0) | 11 docs (~3,600 lines): references, tutorials, guides |
| Runtime WASM Dispatch | [`v0.5.0`](https://github.com/greentic-biz/greentic-designer-extensions/releases/tag/v0.5.0) | Wasmtime Linker + 5 host imports + `invoke_tool` working end-to-end |

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
                              cargo install greentic-ext-cli
```

---

## Install `gtdx`

From source:

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

### 1. Build the AC reference extension (from its own repo)

The AC extension lives in
[`greentic-biz/greentic-adaptive-card-mcp`](https://github.com/greentic-biz/greentic-adaptive-card-mcp)
next to `adaptive-card-core`:

```bash
git clone git@github.com:greentic-biz/greentic-adaptive-card-mcp.git
cd greentic-adaptive-card-mcp
crates/adaptive-card-extension/build.sh
# → crates/adaptive-card-extension/greentic.adaptive-cards-1.6.0.gtxpack (~1 MB)
```

### 2. Validate, install, list

```bash
GTXPACK=$(pwd)/crates/adaptive-card-extension/greentic.adaptive-cards-1.6.0.gtxpack

gtdx install "$GTXPACK" -y --trust loose
# ✓ installed greentic.adaptive-cards@1.6.0

gtdx list
# [design]
#   greentic.adaptive-cards@1.6.0  Design and validate Microsoft Adaptive Cards v1.6

gtdx doctor
# ✓ ~/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/describe.json
# 1 total, 0 bad
```

### 3. Search the store (when live)

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
│   ├── greentic-extension-sdk-contract/        # Types, describe.json schema, signatures
│   ├── greentic-ext-runtime/         # Wasmtime loader, capability registry, broker
│   ├── greentic-ext-registry/        # 3 registry impls + lifecycle
│   ├── greentic-ext-cli/             # `gtdx` binary, 11 subcommands
│   ├── greentic-extension-sdk-testing/         # Test utilities for extension authors
│   └── _wit-lint/                    # WIT parser lint test
│
├── docs/                             # docs (see docs/README.md for index)
└── ci/local_check.sh                 # fmt + clippy + test + release build
```

Reference extensions live with their domain library, in their own repos:

- [`greentic-biz/greentic-adaptive-card-mcp`](https://github.com/greentic-biz/greentic-adaptive-card-mcp) — `crates/adaptive-card-extension/` ships `greentic.adaptive-cards@1.6.0`
- (future) `greentic-biz/greentic-digital-workers` — would ship `greentic.digital-workers@*`
- (future) `greentic-biz/greentic-telco-x` — would ship Telco X extensions

---

## Integration with greentic-designer

Designer (separate repo) consumes this extension system as a Cargo dependency:

```toml
# greentic-designer/Cargo.toml
[dependencies]
greentic-ext-runtime  = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.5.0" }
greentic-extension-sdk-contract = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.5.0" }
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
`greentic.adaptive-cards-1.6.0.gtxpack` (built from
`greentic-adaptive-card-mcp`) and auto-install on startup if no AC
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
| [`how-to-write-a-provider-extension.md`](docs/how-to-write-a-provider-extension.md) | Provider authors (messaging, events, webhooks) |

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

Prerequisites: Rust 1.94, no extra targets needed (this repo ships only host-side crates).

```bash
rustup install 1.94.0
rustup component add rustfmt clippy --toolchain 1.94.0
```

```bash
bash ci/local_check.sh
```

To run the optional end-to-end WASM dispatch test, build the AC
extension from `greentic-adaptive-card-mcp` first and point
`GTDX_TEST_GTXPACK` at the resulting `.gtxpack`:

```bash
export GTDX_TEST_GTXPACK=/path/to/greentic.adaptive-cards-1.6.0.gtxpack
cargo test -p greentic-ext-runtime --test ac_invoke -- --nocapture
```

The test self-skips if the env var is unset.

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
