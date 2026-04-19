# Changelog

## Unreleased

### Added
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
