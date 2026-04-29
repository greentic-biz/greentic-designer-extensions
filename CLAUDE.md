# CLAUDE.md

This file provides guidance to Claude Code when working in this repository.

## Status

**Specification stage.** No code has been written yet. The design is
captured in [`docs/superpowers/specs/2026-04-17-designer-extension-system-design.md`](docs/superpowers/specs/2026-04-17-designer-extension-system-design.md).
Before writing any code, read that spec first — it defines the architecture,
contract, WIT interfaces, and migration strategy.

The short-form executive summary for non-developers (including Maarten) is
in [`docs/concept.md`](docs/concept.md).

## What This Is

An extension system that teaches the Greentic Designer new domain
capabilities at runtime through signed WebAssembly components distributed
via the Greentic Store. Three extension kinds share a unified foundation:

- `DesignExtension` — teaches authoring of a content type
- `BundleExtension` — packages designer output into Application Packs
- `DeployExtension` — deploys Application Packs to targets

Extensions declare capabilities they offer + require. A capability registry
resolves the graph at startup and hot-reload, failing gracefully when
requirements are missing. A host broker mediates cross-extension calls with
permission gates.

## Conventions (once implementation begins)

- **Rust 1.94.0**, edition 2024 (match other greentic projects)
- **WASM target:** `wasm32-wasip2`
- **Max 500 lines per source file.** Split modules before exceeding.
- **English only** in source, tests, comments, commit messages, tracing logs.
- **No Claude co-authorship** on commits.
- **Husky hooks** — pre-commit runs fmt + clippy; pre-push runs full ci/local_check.sh.
- **Feature branches + PRs** — never push directly to `main`.
- **`serde_yaml_gtc`** (imported as `serde_yaml_bw`), not `serde_yaml`.

## Planned Workspace Layout

See the spec section 4 for the full structure. Workspace-level
`Cargo.toml` lists four crates plus reference extensions:

```toml
[workspace]
members = [
  "crates/greentic-extension-sdk-contract",
  "crates/greentic-ext-runtime",
  "crates/greentic-ext-cli",
  "crates/greentic-extension-sdk-testing",
  "reference-extensions/adaptive-cards",
]
```

## Dependencies To Pin Once Chosen

- `wasmtime` — Component Model runtime (pin version)
- `wit-bindgen` — Component WIT generation
- `cargo-component` — build tool for WASM components
- `arc-swap` — atomic hot reload swaps (match `greentic-runner`)
- `notify` — filesystem watcher (debounced)
- `semver` — version resolution
- `ed25519-dalek` — artifact signing

## External Tool Integration

- `greentic-designer` — consumer; refactored to drop direct
  `adaptive-card-core` dep and consume `greentic-ext-runtime`
- `greentic-adaptive-card-mcp` — `adaptive-card-core` stays there; the AC
  extension crate depends via git
- `greentic-store-server` — separate repo (not yet created); implements the
  OpenAPI spec shipped here

## Before Starting Implementation

1. Read `docs/superpowers/specs/2026-04-17-designer-extension-system-design.md`
2. Confirm Maarten sign-off on open decisions (BX-01, DX-01)
3. Use `superpowers:writing-plans` to produce a detailed implementation
   plan from the spec
4. Follow `superpowers:subagent-driven-development` pattern (match what
   worked for Plan A/B of `greentic-adaptive-card-mcp`)
