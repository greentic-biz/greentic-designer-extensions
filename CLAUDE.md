# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

## What this repo is

`greentic-ext-runtime` — the wasmtime-based host the Greentic Designer
uses to load and dispatch WebAssembly extensions. Three extension
kinds share a unified contract:

- **Design extension** (e.g. `greentic.adaptive-cards`) — exposes
  `tools`, `validation`, `prompting`, `knowledge` interfaces. The
  designer drives these from `/api/chat`, `/api/agent`, the
  inspector preview, and the LLM tool-calling loop.
- **Deploy extension** (e.g. `greentic.deploy-aws`) — exposes
  `targets`, `deployment`. The wizard's deploy step calls these to
  push `.gtbundle` artefacts to a target.
- **Bundle extension** (e.g. `greentic.bundle-standard`) — exposes
  `recipes`, `bundling.render`. Pack/Deploy renders the designer
  session into a `.gtpack` here. Wired through the runtime since
  `v0.12.0` (2026-05-01) — the previous out-of-band
  `greentic-bundle ext render` subprocess was retired (cf. designer
  PR #130 + plan
  `greentic-designer/docs/superpowers/plans/2026-04-30-bundle-dispatch-in-runtime.md`).

The runtime lives at `crates/greentic-ext-runtime`. The supporting
SDK crates (`-contract`, `-state`, `-cli`, `-registry`, `-testing`)
were split out to the public
[`greenticai/greentic-designer-sdk`](https://github.com/greenticai/greentic-designer-sdk)
repo and ship to crates.io as `greentic-extension-sdk-*` (#37 + #38).
Designer consumes them from crates.io while keeping `-runtime`
git-pinned here.

## Workspace layout

```toml
[workspace]
members = [
  "crates/greentic-ext-runtime",
  "crates/_wit-lint",
]
```

Reference extension repos (`greentic-bundle-extensions`,
`greentic-deployer-extensions`, `greentic-adaptive-card-mcp`) live
in their own GitHub orgs and consume this runtime via crates.io
(when the SDK pieces it depends on are published) or git tag.

## Public API surface

`greentic-ext-runtime` exposes (see `crates/greentic-ext-runtime/src/lib.rs`):

- `ExtensionRuntime::new(config)` — load + verify signed extensions
  from `~/.greentic/extensions/{design,deploy,bundle,provider}/`.
- `register_loaded_from_dir(path)` — explicit registration; designer
  calls this for design + deploy + bundle dirs at startup. Bundle
  registration is required for `render_bundle()` to find the
  recipe.
- `invoke_tool(ext_id, name, args_json)` — design extension tool
  dispatch.
- `validate_content(ext_id, content_type, content_json)` — design
  extension validator.
- `list_tools` / `prompt_fragments` / `knowledge_*` — design
  extension introspection helpers.
- `validate_credentials` / `credential_schema` / `list_targets` —
  deploy extension surface.
- `render_bundle(ext_id, recipe_id, config_json, session)` —
  bundle extension entry point. Takes a typed `BundleSession`
  (`flows_json`, `contents_json`, `assets`, `capabilities_used`) and
  returns a `BundleArtifact` (`filename`, `bytes`, `sha256`). Same
  lookup pattern as the design / deploy methods above; runs the
  WIT call on a sync wasmtime store, so callers in async contexts
  should wrap in `spawn_blocking`.

Wasmtime store + linker plumbing lives in `host_bindings.rs` (one
`bindgen!` per world to keep the `extension-base` / `extension-host`
shared types from colliding) and `host_state.rs` (capability +
broker + logging + i18n imports).

## Conventions

- **Rust 1.95.0**, edition 2024 (`rust-toolchain.toml` is canonical).
- **WASM target** for reference extensions: `wasm32-wasip2` —
  governed by the consuming repo (`greentic-bundle-extensions`,
  `greentic-deployer-extensions`).
- **Max 500 lines per source file.** Split modules before exceeding.
- **English only** in source, tests, comments, commit messages,
  tracing logs.
- **No Claude co-authorship** on commits.
- **Husky hooks** — pre-commit runs fmt + clippy; pre-push runs full
  `ci/local_check.sh`.
- **Feature branches + PRs** — never push directly to `main`.
- **Tag releases** — `v0.X.Y` workspace tags + `<crate>-vX.Y.Z` per-
  crate tags. Designer pins to the workspace tag.

## Adding a new world / interface

1. Vendor the WIT under `crates/greentic-ext-runtime/wit/deps/<package>/`
   (each kind gets its own subdir to dodge namespace collisions).
2. Add a sibling `mod <kind>` in `host_bindings.rs` with
   `wasmtime::component::bindgen!({ path: "wit", world: "..." })`.
3. Mirror the WIT records as Rust structs in `types.rs`; re-export
   from `lib.rs`.
4. Implement the entry point in `runtime.rs` (own `impl ExtensionRuntime`
   block to keep file sections small) — resolve the loaded extension,
   walk the `get_export_index` chain, call the typed function, map
   the WIT-level error into `RuntimeError::Wasmtime`.
5. Add a smoke test in the matching `#[cfg(test)]` module that
   exercises the `RuntimeError::NotFound` path against a tempdir.

The existing bundle path (`render_bundle`) is the most recent
example to mirror.

## External tool integration

- **`greentic-designer`** — primary consumer. Pins this crate via
  git tag (`v0.12.0+` for bundle dispatch).
- **`greentic-bundle-extensions`** — bundles the
  `bundle-standard` reference recipe + the OSS-side dispatcher stub
  (`greentic-bundle-extension-host::dispatcher::invoke_recipe`
  returns `ModeBNotImplemented` by design — designer goes through
  this runtime instead).
- **`greentic-adaptive-card-mcp`** — ships the `adaptive-cards`
  design extension (built against this runtime's WIT).
- **`greentic-store-server`** — distributes signed `.gtxpack`
  artefacts; the runtime's `verify_describe` checks signatures
  against the store's published key set.
