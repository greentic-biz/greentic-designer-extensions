# Changelog

## Unreleased

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

### Changed
- `InstallOptions` gained `force: bool` field (default `false`)
- `RegistryError::ProviderInstall` variant added
- `Storage::root()` accessor exposed
- Workspace version bumped 0.1.0 → 0.2.0 (additive — existing kinds unaffected)

### Fixed
- `describe-v1.json` schema: added `ProviderExtension` to kind enum
  (was missing since commit `4bf0e02`, blocked CLI install path)
- `wit_files_returns_all_embedded_packages` test count 6 → 7 after A4

### Notes
- Runner integration is zero-change: `greentic-runner` picks up extracted
  `.gtpack` files via existing 30s pack-index polling
