# Changelog

## Unreleased

### Added
- `gtdx search` accepts an optional QUERY (lists everything when omitted).
- `gtdx doctor` expands to four sections: toolchain (cargo / cargo-component /
  rustup / wasm32-wasip2 target), registries (reachability probe via /health),
  credentials (JWT expiry decode from token), and installed extensions
  (existing describe validation). `--offline` skips network probes.
- `gtdx publish` maps error kinds to numeric exit codes per spec §9
  (describe=2, build=70, version-exists=10, auth=20, registry=30,
  not-implemented=50, io=74, other=1).
- `gtdx publish --format json` emits a single JSON object per invocation
  (`event`: `dry_run` / `verify_only` / `published`) for IDE + CI consumers.
- `gtdx publish --registry oci://<host>/<namespace>[/<artifact>]` pushes the
  `.gtxpack` directly to any OCI Distribution v2-compatible registry (GHCR,
  Docker Hub, Harbor, Azure ACR). Artifact is a single layer with media type
  `application/vnd.greentic.gtxpack.v1`. Auth priority: `--oci-token` flag >
  `GHCR_TOKEN` > `GITHUB_TOKEN` > `OCI_TOKEN` > anonymous. 401/403 → hint to
  refresh `write:packages` scope; 409 → `VersionExists`.

## [0.2.0] - 2026-04-19

### Added
- `ExtensionKind::Provider` — 4th extension kind alongside Design/Bundle/Deploy
- `greentic:extension-provider@0.1.0` WIT contract with 3 sub-interfaces
  (messaging, event-source, event-sink) and 6 worlds for mixed capabilities
- `describe.json` `runtime.gtpack` field — required when `kind=ProviderExtension`,
  enforces kind↔gtpack invariant via `TryFrom<DescribeJsonRaw>`
- Lifecycle `install_provider` path: sha256 verification (constant-time), manual-pack
  conflict detection via CBOR `manifest.cbor.pack_id`, extraction to
  `~/.greentic/runtime/packs/providers/gtdx/`
- `ExtensionRegistry::list_by_kind` + `get_describe` trait methods (default impls)
- `gtdx list --kind <design|bundle|deploy|provider|all>` filter
- `gtdx info <name>` — local-first lookup, renders provider runtime pack +
  component version, uniform capabilities line
- `gtdx install <.gtxpack>` now routes `kind=Provider` through `post_install_provider`
- Shared provider fixture helpers in `greentic-ext-testing::provider_fixtures`
- `greentic-ext-contract::hex` — centralized hex encoder
- `gtdx new <name>` — scaffold a new design/bundle/deploy extension with
  vendored WIT contract and `.gtdx-contract.lock` (Phase 1 Track A, S1).
- `gtdx dev` subcommand: inner-loop build + pack + install for extension authors.
  Supports `--once` (CI-friendly one-shot), `--watch` (default continuous mode),
  `--no-install` (pack only), `--release`, `--debounce-ms`, and `--format json`
  for IDE integrations. File watcher filters `target/`, VCS dirs, editor swap
  files, and backup files automatically. Skip-unchanged logic avoids redundant
  installs when the pack's sha256 has not changed.
- `gtdx publish` subcommand: validate describe.json, build release WASM, pack
  into a deterministic `.gtxpack`, and publish into the filesystem registry at
  `$GREENTIC_HOME/registries/local/<id>/<version>/`. Supports `--dry-run`,
  `--force`, `--sign --key-id <id>`, `--version` override, and `--verify-only`.
  Writes a receipt at `./dist/publish-<id>-<version>.json`. Store and OCI
  registries return `NotImplemented` for now (Phase 2).
- `greentic-ext-contract::pack_writer` — deterministic ZIP writer (sorted
  entries, zeroed timestamps, LF normalization) shared by `gtdx dev` and
  `gtdx publish`.
- `gtdx publish --registry <name>` now uploads `.gtxpack` artifacts to a
  Greentic Store HTTP server via multipart POST to `/api/v1/extensions` with
  bearer-token auth. Registry URL is resolved from `~/.greentic/config.toml`
  (add with `gtdx registries add <name> <url>`); token is read from
  `~/.greentic/credentials.toml` (`gtdx login --registry <name>`) or the
  env-var named in the registry's `token-env` entry. 401 → `AuthRequired`
  with actionable hint; 409 → `VersionExists`; 2xx → parsed `PublishReceipt`.

### Changed
- `InstallOptions` gained `force: bool` field (default `false`)
- `RegistryError::ProviderInstall`, `VersionExists`, `NotImplemented` variants added
- `Storage::root()` accessor exposed
- `ExtensionRegistry::publish` signature: now takes `PublishRequest` and returns `PublishReceipt` (replaces prior `ExtensionArtifact + AuthToken` shape)
- Workspace version bumped 0.1.0 → 0.2.0 (additive — existing kinds unaffected)

### Fixed
- `describe-v1.json` schema: added `ProviderExtension` to kind enum
  (was missing since commit `4bf0e02`, blocked CLI install path)
- `wit_files_returns_all_embedded_packages` test count 6 → 7 after A4

### Notes
- Runner integration is zero-change: `greentic-runner` picks up extracted
  `.gtpack` files via existing 30s pack-index polling
