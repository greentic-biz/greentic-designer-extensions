# Publishing an Extension — `gtdx publish`

Companion to `getting-started-scaffolding.md` and `getting-started-dev.md`. Once
you're ready to share your extension, `gtdx publish` validates the describe,
builds a release-profile WASM, packs it deterministically, and writes it into
your local filesystem registry.

## Quick start

```bash
gtdx new my-ext
cd my-ext
# ... implement your extension ...
gtdx publish
```

The artifact lands at
`~/.greentic/registries/local/<id>/<version>/<name>-<version>.gtxpack` along
with `manifest.json`, `artifact.sha256`, and (if `--sign`) `signature.json`.
A receipt is written to `./dist/publish-<id>-<version>.json`.

## Flags

| Flag                  | Purpose                                                       |
|-----------------------|---------------------------------------------------------------|
| `--registry <URI>`    | `local` (default) or `file://<path>`. Store/OCI are Phase 2.  |
| `--version <SEMVER>`  | Override `describe.json` version (CI version bumps).          |
| `--dry-run`           | Validate + build + pack, skip registry write.                 |
| `--force`             | Overwrite an existing version.                                |
| `--sign --key-id <ID>`| Sign describe.json via Wave 1 JCS (Ed25519).                  |
| `--verify-only`       | Check version conflict; skip build.                           |
| `--dist <DIR>`        | Also copy the artifact here. Default `./dist/`.               |
| `--release`           | Build with `--release` (default true for publish).            |
| `--format <FMT>`      | `human` (default) or `json`.                                  |

## Publishing to the Greentic Store

`gtdx publish --registry local` writes to the local filesystem. To push to a
Store HTTP server:

1. Register the Store URL once:

   ```bash
   gtdx registries add mystore https://store.example.com
   ```

2. Log in (saves a bearer token at `~/.greentic/credentials.toml` with
   mode 0600):

   ```bash
   gtdx login --registry mystore
   # paste the JWT when prompted
   ```

3. Publish:

   ```bash
   gtdx publish --registry mystore
   ```

Token resolution order on publish:

1. Env var named in the registry's `token-env` entry (configured via
   `gtdx registries add <name> <url> --token-env MYSTORE_TOKEN`).
2. `~/.greentic/credentials.toml` entry for the registry name.
3. None → `gtdx publish` aborts with an `AuthRequired` hint.

Publisher handles and allowed-prefix policies are enforced server-side;
you can only publish extensions whose `metadata.id` matches a prefix
allowed for your account.

## Determinism

Two `gtdx publish` invocations over identical sources produce byte-identical
`.gtxpack` archives. The writer sorts entries, zeros timestamps to the ZIP
epoch (1980-01-01), normalizes Unix permissions to 0644/0755, and normalizes
CRLF -> LF for text assets (json/md/wit/txt/toml/yaml).

## Non-goals in Phase 1

- Publishing to the Greentic Store HTTP registry (Phase 2, S5)
- Publishing to an OCI registry (Phase 2, S5)
- Passphrase-encrypted signing keys (Phase 2, S4)
- Strict trust policy + countersignatures (Phase 2, S4)
