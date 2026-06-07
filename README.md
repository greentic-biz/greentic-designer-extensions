# greentic-designer-extensions

The wasmtime-based **runtime engine** that hosts WebAssembly extensions for
[Greentic Designer](https://github.com/greentic-biz/greentic-designer). This
repo ships the WIT contract, the host runtime crate (`greentic-ext-runtime`),
the capability/permission broker, and the watcher that hot-reloads
extensions at runtime.

> **Authoring an extension?** Go to the public SDK repo
> [`greenticai/greentic-designer-sdk`](https://github.com/greenticai/greentic-designer-sdk),
> which ships the `gtdx` CLI, the `describe.json` contract, registry client,
> and test utilities to crates.io as `greentic-extension-sdk-*`. This repo
> is for runtime maintainers and designer integrators.

## What lives where

The extension system used to be a single repo; it now spans three:

| Repo | Role | Crates / artefacts |
|---|---|---|
| `greentic-biz/greentic-designer-extensions` (this repo) | Runtime + WIT contract | `greentic-ext-runtime` (git-pinned), `wit/*.wit`, `_wit-lint` |
| `greenticai/greentic-designer-sdk` | Public SDK + CLI | `greentic-extension-sdk-{contract,state,registry,testing,cli}` — published to crates.io |
| `greentic-biz/greentic-designer` | Consumer (the designer binary) | Pins this repo via git tag, the SDK crates via crates.io |

Reference extensions live in their own domain repos (e.g.
`greentic-adaptive-card-mcp`, `greentic-bundle-extensions`,
`greentic-deployer-extensions`) and consume this runtime's WIT.

---

## Extension Kinds

Four kinds share the unified runtime contract:

| Kind | Teaches the designer to... | Example reference impls |
|---|---|---|
| **design** | Author content — cards, flows, digital workers, telco-x schemas. Exposes `tools`, `validation`, `prompting`, `knowledge`. | [`greentic.adaptive-cards`](https://github.com/greentic-biz/greentic-adaptive-card-mcp), `greentic.flows-ygtc-v2` |
| **bundle** | Package designer output into deployable `.gtpack` archives. Exposes `recipes`, `bundling.render`. | `greentic.bundle-standard` |
| **deploy** | Ship Application Packs to a target environment. Exposes `targets`, `deployment`, credential schemas. | `greentic.deploy-aws`, `greentic.deploy-azure` |
| **provider** | Run flow nodes against external services (messaging, webhooks, events). | shipped via store |

The capability registry semver-matches extensions at load time; the host
broker gates cross-extension calls behind declared permissions and a depth
limit.

---

## Architecture

```
                ┌──────────────────────────────────────────────┐
                │   Greentic Store  (store.greentic.cloud)      │
                │   Developers upload · end-users discover      │
                └────────────────────────┬──────────────────────┘
                                         │ HTTPS / OpenAPI
                                         ▼
                ┌──────────────────────────────────────────────┐
                │            greentic-ext-runtime               │  ◀── this repo
                │  Wasmtime Component loader + Linker           │
                │  Capability registry (semver matching)        │
                │  Host broker (permission + depth gates)       │
                │  Debounced filesystem watcher (hot reload)    │
                └────────────────────────┬──────────────────────┘
                                         │
                ┌────────────────────────▼──────────────────────┐
                │          Greentic Designer (consumer)         │
                │  Chat — design-ext tools                      │
                │  Pack wizard — bundle-ext recipes             │
                │  Deploy wizard — deploy-ext targets           │
                │  Flow runtime — provider-ext nodes            │
                └────────────────────────┬──────────────────────┘
                                         │
                              gtdx CLI (from SDK repo)
                       cargo install greentic-extension-sdk-cli
```

---

## Install `gtdx` (extension CLI)

`gtdx` is published from the
[`greentic-designer-sdk`](https://github.com/greenticai/greentic-designer-sdk)
workspace as the `greentic-extension-sdk-cli` crate.

```bash
cargo install greentic-extension-sdk-cli --locked
gtdx version
gtdx --help
```

Source install (from the SDK repo) is also supported — see that repo's
README for the full inner-loop dev guide.

---

## Quickstart (consumer integration)

### 1. Build a reference extension

The AC extension lives in
[`greentic-biz/greentic-adaptive-card-mcp`](https://github.com/greentic-biz/greentic-adaptive-card-mcp):

```bash
git clone git@github.com:greentic-biz/greentic-adaptive-card-mcp.git
cd greentic-adaptive-card-mcp
crates/adaptive-card-extension/build.sh
# → crates/adaptive-card-extension/greentic.adaptive-cards-1.6.0.gtxpack
```

### 2. Install, list, validate

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

### 3. Search the store

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
│   ├── extension-provider.wit        # Provider-extension interfaces
│   └── runtime-side.wit              # Host-side world (bindgen target)
│
├── crates/
│   ├── greentic-ext-runtime/         # Wasmtime loader, capability registry, broker,
│   │                                 # host bindings — the heart of this repo
│   └── _wit-lint/                    # WIT parser lint test
│
├── docs/                             # Authoring & integration guides (see docs/README.md)
└── ci/local_check.sh                 # fmt + clippy + test + release build
```

The SDK crates (`-contract`, `-state`, `-cli`, `-registry`, `-testing`)
that used to live here moved to
[`greentic-designer-sdk`](https://github.com/greenticai/greentic-designer-sdk)
and now publish to crates.io.

---

## Integration with greentic-designer

The designer (separate repo) consumes this crate as a git-pinned dependency
and the SDK crates from crates.io:

```toml
# greentic-designer/Cargo.toml
[dependencies]
greentic-ext-runtime = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.12.0" }
greentic-extension-sdk-contract = "0.4"
greentic-extension-sdk-state    = "0.4"
```

Then in startup code:

```rust
use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths, discovery};

let home = std::env::var("GREENTIC_HOME")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|_| dirs::home_dir().unwrap().join(".greentic"));

let mut runtime = ExtensionRuntime::new(
    RuntimeConfig::from_paths(DiscoveryPaths::new(home.clone()))
)?;

for kind in ["design", "bundle", "deploy", "provider"] {
    for ext_dir in discovery::scan_kind_dir(&home.join("extensions").join(kind))? {
        runtime.register_loaded_from_dir(&ext_dir).ok();
    }
}

let runtime = std::sync::Arc::new(runtime);
```

In request handlers (or anywhere), invoke design-ext tools:

```rust
let result_json = state.runtime.invoke_tool(
    "greentic.adaptive-cards",
    "validate_card",
    &args_json,
)?;
```

Or render a bundle (since `v0.12.0`):

```rust
let artifact = state.runtime.render_bundle(
    "greentic.bundle-standard",
    "default",
    &config_json,
    &session,
)?;
// → BundleArtifact { filename, bytes, sha256 }
```

`render_bundle` runs the WIT call on a sync wasmtime store, so wrap it in
`tokio::task::spawn_blocking` when called from async code.

---

## Public API surface

`greentic-ext-runtime` exposes (see `crates/greentic-ext-runtime/src/lib.rs`):

| Function | Purpose |
|---|---|
| `ExtensionRuntime::new(config)` | Load + verify signed extensions from `~/.greentic/extensions/{design,bundle,deploy,provider}/` |
| `register_loaded_from_dir(path)` | Explicit registration; required for bundle/deploy lookup |
| `invoke_tool(ext_id, name, args_json)` | Design-extension tool dispatch |
| `validate_content(ext_id, content_type, content_json)` | Design-extension validator |
| `list_tools` / `prompt_fragments` / `knowledge_*` | Design-extension introspection |
| `validate_credentials` / `credential_schema` / `list_targets` | Deploy-extension surface |
| `render_bundle(ext_id, recipe_id, config_json, session)` | Bundle-extension entry point (v0.12.0+) |
| `subscribe()` | `broadcast::Receiver<RuntimeEvent>` for install/remove/state-change events |
| `start_watcher()` | Debounced filesystem watcher for hot-reload |

Wasmtime store + linker plumbing lives in `host_bindings.rs` (one
`bindgen!` per world to keep `extension-base` / `extension-host` shared
types from colliding) and `host_state.rs` (capability + broker + logging +
i18n imports).

---

## Documentation

Full index at [docs/README.md](docs/README.md). Quick links:

### Reference
| Document | Description |
|----------|-------------|
| [`describe-json-spec.md`](docs/describe-json-spec.md) | `describe.json` v1 — every field, every kind, with examples |
| [`wit-reference.md`](docs/wit-reference.md) | All WIT packages + interfaces with type signatures |
| [`capability-registry.md`](docs/capability-registry.md) | Semver matching, degraded state, cycle detection |
| [`cli-reference.md`](docs/cli-reference.md) | `gtdx` subcommands with examples |

### Tutorials
| Document | Audience |
|----------|----------|
| [`how-to-write-a-design-extension.md`](docs/how-to-write-a-design-extension.md) | Authors of authoring/validation extensions |
| [`how-to-write-a-bundle-extension.md`](docs/how-to-write-a-bundle-extension.md) | Devs adding packaging recipes |
| [`how-to-write-a-deploy-extension.md`](docs/how-to-write-a-deploy-extension.md) | Devs adding cloud / on-prem targets |
| [`how-to-write-a-provider-extension.md`](docs/how-to-write-a-provider-extension.md) | Provider authors (messaging, events, webhooks) |
| [`how-to-write-a-wasm-component-extension.md`](docs/how-to-write-a-wasm-component-extension.md) | Generic WASM Component extensions |

### Guides
| Document | Description |
|----------|-------------|
| [`getting-started-scaffolding.md`](docs/getting-started-scaffolding.md) | `gtdx new` walk-through |
| [`getting-started-dev.md`](docs/getting-started-dev.md) | Inner-loop dev (`gtdx dev`) |
| [`getting-started-publish.md`](docs/getting-started-publish.md) | `gtdx publish --registry oci://…` |
| [`cross-extension-communication.md`](docs/cross-extension-communication.md) | Host broker, permissions, composition |
| [`permissions-and-trust.md`](docs/permissions-and-trust.md) | Trust policies, Ed25519 signing |
| [`lifecycle-management.md`](docs/lifecycle-management.md) | Enable / disable / hot-reload contract |

### Architecture
| Document | Description |
|----------|-------------|
| [`concept.md`](docs/concept.md) | Executive summary for stakeholders |
| [Greentic Store API](docs/greentic-store-api.openapi.yaml) | OpenAPI 3.1 contract for the registry server |

---

## Building from source

Prerequisites: **Rust 1.95.0** (pinned via `rust-toolchain.toml`). No extra
targets needed for this repo — only host-side crates ship from here. The
WASM target (`wasm32-wasip2`) is the consuming repos' concern.

```bash
rustup install 1.95.0
rustup component add rustfmt clippy --toolchain 1.95.0
bash ci/local_check.sh
```

To run the optional end-to-end WASM dispatch test, build the AC extension
from `greentic-adaptive-card-mcp` first and point `GTDX_TEST_GTXPACK` at
the resulting `.gtxpack`:

```bash
export GTDX_TEST_GTXPACK=/path/to/greentic.adaptive-cards-1.6.0.gtxpack
cargo test -p greentic-ext-runtime --test ac_invoke -- --nocapture
```

The test self-skips if the env var is unset.

A second, v2-contract variant (`ac_invoke_v2`) exercises the same dispatch
path against an extension built with extension-base@0.2.0 /
extension-design@0.3.0 (the 6-variant `extension-error` ABI). It reads
`GTDX_TEST_GTXPACK_V2` and covers both a success invoke and a typed
`RuntimeError::Extension` error path. The v2 fixture (AC-MCP PR #74,
`2.0.4-research`) ships unsigned, so it needs the dev escape hatch:

```bash
GREENTIC_EXT_ALLOW_UNSIGNED=1 \
  GTDX_TEST_GTXPACK_V2=/path/to/ac-v2.gtxpack \
  cargo test -p greentic-ext-runtime --features dev-allow-unsigned \
    --test ac_invoke_v2 -- --nocapture
```

Both real-wasm tests self-skip when their env var is unset, so CI's
`cargo test --workspace --all-features` run (which does not set them)
passes them as no-ops — they are opt-in dev-local checks only.

---

## Releases

Two tag schemes coexist:

- **Workspace tags** (`v0.X.Y`) — designer pins to these for the runtime
  crate.
- **Per-crate tags** (`<crate>-vX.Y.Z`) — used when an individual crate
  needs to publish out of sync with the workspace.

Recent milestones:

| Tag | What's in it |
|---|---|
| `v0.5.0` | Wasmtime Linker + 5 host imports + `invoke_tool` end-to-end |
| `v0.6.0` | AC reference extension moved out + CI cleanup |
| `v0.7.0` | Design-interface methods (`list_tools`, `prompt_fragments`, `knowledge_*`) |
| `v0.8.0` | Release polish |
| `v0.9.0` | `validate_content` for design extensions |
| `v0.10.0` | Deploy-extension host bindings |
| `v0.11.0` | Release polish |
| `v0.12.0` | `render_bundle()` for in-process bundle WASM dispatch |
| `v1.2.x` (research lane) | `roles@0.2.0` WIT interface + runtime wrappers |

See `git tag --sort=-v:refname` for the full list.

---

## Contributing

1. Read [CLAUDE.md](CLAUDE.md) for repo conventions before starting.
2. Pick an issue or open one to discuss.
3. Branch off `research` (or `main` for hotfixes). Run
   `bash ci/local_check.sh` locally — it must be green before pushing.
4. Open a PR. CI must be green; address review comments; squash on merge.

Conventions:
- **Rust 1.95.0**, edition 2024
- **Max 500 lines** per source file
- **English-only** in source / commits / log messages / docs
- **Conventional commit prefixes** (`feat:`, `fix:`, `docs:`, `ci:`, `chore:`, `test:`, `refactor:`)
- **No Claude / AI co-author attribution**

---

## License

MIT — see [LICENSE](LICENSE).
