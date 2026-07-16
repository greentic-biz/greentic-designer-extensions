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
  artefacts. The runtime's verify chain (`verify_dir_signature`)
  checks the describe signature for self-consistency, that the
  describe is bound to the whole-archive `manifest.json`
  (`manifestSha256`), and that every manifest entry hash-matches —
  failing closed on a missing manifest (audit P5). It then anchors
  the signature (see below).

## Signature anchoring (TOFU)

`verify_dir_signature` runs three steps, in this order:

1. `verify_describe_self_consistent` — **describe integrity**. Any key
   passes; this only proves the describe is unchanged since signing.
2. `verify_dir_manifest` — **artifact integrity**: the describe is bound
   to the whole-archive ledger and every listed file hash-matches.
3. `TrustStore::pin_or_verify` — **the anchor**, and the only step that
   supplies authenticity. Trust-on-first-use: the publisher key is
   pinned per `extension.id` on first load, and every later load of that
   id must present the same key. Step 1 proved the signature verifies
   against that key, so pinning it is what makes the pair meaningful.

**The anchor must stay last.** Pinning is a *write*, into the store
`gtdx` shares — so a pin from a load that later fails permanently blocks
the genuine publisher for that id in both tools, recoverable only by
hand-editing `publishers.json`. An attacker who cannot complete a load
must not be able to squat an id that way. `gtdx` orders it the same, one
level up: `sdk-registry/src/lifecycle.rs` runs `verify_integrity` then
`verify_authenticity`.

There is deliberately **no `verify_describe_with_key` step**. Handing it
a key read out of the describe under verification compares that key
against itself — a tautology that cannot fail where step 1 passed. The
SDK's own doc says the key "must come from a trust anchor ... never from
the artifact alone"; here the trust anchor is the pin.

The store is `greentic-extension-sdk-registry`'s `TrustStore` — the
same one `gtdx install` writes, reused rather than reimplemented. It
lives at `<root>/trust/publishers.json` where root is `$GREENTIC_HOME`,
else `~/.greentic` (`RuntimeConfig::resolve_trust_root`, mirroring
gtdx's own resolution). It is deliberately **not** derived from
`DiscoveryPaths` — that diverges under `$GREENTIC_HOME` or the runner's
`GREENTIC_EXTENSIONS_DIR`, and would silently pin into a store gtdx
never reads.

This gate applies to **both** load paths — `register_loaded_from_dir`
and the watcher's `handle_added_or_modified`. The watcher path
previously verified nothing at all.

TOFU is what is available without a trust root. A **KMS-rooted cert
chain (D.5) is still blocked** on key custody; until then a first load
trusts whatever key it first sees. Consequence worth knowing: an update
signed by a different key than the first load is **rejected**
(`PublisherKeyChanged`, naming both keys) — intended, but it means two
developers publishing one extension from their own local keys will
collide.

`GREENTIC_EXT_ALLOW_UNSIGNED=1` (only under the `dev-allow-unsigned`
feature) still skips all three steps.

## Capability registry

`CapabilityRegistry` is derived wholesale from the loaded set by
`ExtensionRuntime::rebuild_registry`, never patched incrementally.
Every path that mutates `loaded` must store a registry rebuilt from
the new map. This is what makes eviction correct by construction —
a dropped capability, a removed extension, and a re-registered dir all
fall out automatically. Do not reintroduce per-call-site registry
mutation: the previous clone-forward-then-append got all three wrong,
and a stale offering is a live false positive for anything that reads
`offerings()` to decide what is resolvable.
