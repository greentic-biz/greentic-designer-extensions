# Phase 1 — Track C: Publish Local FS (`gtdx publish`)

**Status:** Design, awaiting implementation plan
**Date:** 2026-04-18
**Parent:** `2026-04-18-dx-10-10-roadmap.md` (subsystem S3)
**Owner crates:** `greentic-ext-cli`, `greentic-ext-registry`, `greentic-extension-sdk-contract`

## 1. Goal

Wire `Registry::publish()` end-to-end for the `LocalFilesystem` backend and expose it via `gtdx publish`. Produce deterministic `.gtxpack` artifacts and atomic registry writes. This is the minimum viable publish path; Store/OCI follow in Phase 2.

## 2. CLI UX

```
gtdx publish [OPTIONS]

Options:
  -r, --registry <URI>   Registry URI
                         Accepts: file://<path>, local (= $GTDX_HOME/registries/local)
                         [default: local]
      --version <SEMVER> Override describe.json version (for CI version bumps)
      --dry-run          Build + pack + validate; skip registry write
      --sign             Sign .gtxpack with local key from ~/.greentic/keys/
      --key-id <ID>      Signing key id (requires --sign)
      --trust <POLICY>   loose | normal | strict       [default: loose]
      --dist <DIR>       Also copy artifact to this dir [default: ./dist/]
      --force            Overwrite existing version in registry
      --release          cargo component build --release [default: true in publish]
      --verify-only      Check registry for version conflict, skip build
      --format <FORMAT>  human | json                  [default: human]
```

## 3. Pipeline

```
1. Load & validate describe.json (JSON Schema v1)
   └─ fail? exit 2 with JSON pointer to bad field
2. Preflight
   ├─ cargo component available
   ├─ version does not already exist in target registry (unless --force)
   └─ signing key available (if --sign)
3. Build WASM (cargo component build --release)
   └─ fail? exit 70 with compiler output
4. Assemble artifact tree in a tempdir
   ├─ copy target/wasm32-wasip2/release/*.wasm → extension.wasm
   ├─ canonicalize describe.json → describe.json
   ├─ copy schemas/ prompts/ i18n/ knowledge/ icons/ (if present)
   └─ (optional --sign) write signature.json
5. Pack deterministic .gtxpack (ZIP)
   ├─ entries sorted by path
   ├─ timestamps zeroed (1980-01-01 epoch — ZIP min)
   ├─ Unix modes normalized (0644 files, 0755 dirs)
   └─ compute SHA256 of final ZIP
6. Publish via Registry::publish() trait
   └─ LocalFilesystem: atomic rename (tempfile → target)
7. Write receipt JSON to ./dist/publish-<id>-<version>.json
   { artifact, sha256, registry, timestamp, trust_policy, signed: bool }
```

## 4. `Registry::publish()` Contract

Extend existing trait in `greentic-ext-registry`:

```rust
#[async_trait]
pub trait Registry: Send + Sync {
    // existing methods unchanged ...

    async fn publish(&self, req: PublishRequest) -> Result<PublishReceipt, RegistryError>;
}

pub struct PublishRequest {
    pub ext_id: String,
    pub version: Version,
    pub kind: ExtensionKind,
    pub artifact_bytes: Vec<u8>,          // .gtxpack contents
    pub artifact_sha256: String,
    pub describe: DescribeJson,
    pub signature: Option<SignatureBlob>, // None = unsigned
    pub force: bool,
}

pub struct PublishReceipt {
    pub url: String,     // file:///... for local fs
    pub sha256: String,
    pub published_at: DateTime<Utc>,
}

pub enum RegistryError {
    VersionExists { existing_sha: String },
    AuthRequired,
    NetworkError(String),
    SignatureRequired,      // when registry policy requires signed
    SchemaValidation(String),
    NotImplemented { hint: String },  // Phase 2 stubs return this
    Io(std::io::Error),
}
```

### 4.1 Phase 1 registry coverage

- `LocalFilesystem::publish()` — **implemented**.
- `GreenticStoreRegistry::publish()` — **returns** `RegistryError::NotImplemented { hint: "Store publish lands in Phase 2 (S5). Use --registry local for now." }`.
- `OciRegistry::publish()` — **returns** `RegistryError::NotImplemented { hint: "OCI publish lands in Phase 2 (S5). Use --registry local for now." }`.

Silent fallback is explicitly avoided.

## 5. LocalFilesystem Layout

```
<registry-root>/
├── index.json                   # top-level index, updated atomically
├── com.example.demo/
│   ├── metadata.json            # aggregated metadata for `gtdx info`
│   ├── 0.1.0/
│   │   ├── com.example.demo-0.1.0.gtxpack
│   │   ├── manifest.json        # canonicalized describe.json
│   │   └── signature.json       # optional
│   └── 0.1.1/
│       └── ...
└── another.ext/
    └── ...
```

### 5.1 Atomicity & concurrency

- All writes go via `tempfile-then-rename`. `rename(2)` is atomic on same filesystem.
- Concurrent `gtdx publish` processes coordinate via advisory file lock on `<registry-root>/.publish.lock` (`fs2` crate).
- Readers (`gtdx install`, `gtdx info`) use the same lock in shared mode for index.json reads.

## 6. Deterministic ZIP Packing (critical)

Shared pack writer lives in `greentic-extension-sdk-contract::pack_writer`, used by both Track B (`gtdx dev`) and Track C (`gtdx publish`):

```rust
pub fn build_gtxpack(entries: Vec<PackEntry>) -> Result<Vec<u8>> {
    let mut entries = entries;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let mut buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buf);
    for entry in entries {
        let opts = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(
                DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
                    .expect("1980-01-01 is a valid ZIP epoch"),
            )
            .unix_permissions(if entry.is_dir { 0o755 } else { 0o644 });
        zip.start_file(&entry.path, opts)?;
        zip.write_all(&entry.bytes)?;
    }
    zip.finish()?;
    Ok(buf.into_inner())
}
```

### 6.1 Line-ending normalization

Text assets (`*.json`, `*.md`, `*.wit`, `*.txt`) have CRLF converted to LF before hashing. Binary assets (`*.wasm`, `*.png`, `*.zip`) are passed through untouched. This keeps SHA256 stable across Windows contributors.

### 6.2 Determinism acceptance

`tests/deterministic_pack.rs`:

```rust
#[test]
fn publish_is_deterministic() {
    let tmp = tempdir().unwrap();
    let p1 = run_publish(&tmp, "demo");
    let p2 = run_publish(&tmp, "demo");
    assert_eq!(p1.sha256, p2.sha256);
}
```

Repeated 10× in CI (loop in script) to catch non-determinism from HashMap iteration or system clocks.

## 7. Signing at Phase 1 (Minimum Viable)

Phase 1 does NOT wire JCS RFC 8785 canonical signing (Phase 2 territory). What Phase 1 ships:

- `--sign` flag is opt-in (default: unsigned; `--trust loose`).
- If `--sign` is set:
  - Reuse the interim Ed25519 implementation from the `feat-signing` worktree.
  - Sign over `artifact_sha256` (the full ZIP hash), NOT over canonicalized describe.json.
  - Write `signature.json` alongside the `.gtxpack`:
    ```json
    {
      "alg": "ed25519",
      "key_id": "developer-01",
      "scheme": "phase1-artifact-sha256",
      "signature_b64": "...",
      "known_limitations": ["phase1-artifact-sig-only-not-jcs"]
    }
    ```
- `gtdx keygen <name>` minimal: generate Ed25519 keypair → `~/.greentic/keys/<name>.{pub,key}` (mode 0600). No passphrase encryption yet (Phase 2).
- Publish receipt sets `"signed": true` and includes `"signing_known_limitations"`.
- The Phase 1 happy-path runs **without** `--sign`; `--sign` exists for end-to-end loop testing and early adopters willing to accept the known gap.

`--sign` emits a loud warning on every invocation:

```
warning: Phase 1 signing signs only the artifact SHA256. Describe.json
         canonicalization (JCS, RFC 8785) lands in Phase 2. Do NOT rely
         on this signature for production trust decisions.
```

## 8. Pre-Publish Validation

`describe.json` must pass, in this order:

1. JSON Schema v1 (existing `schemas/describe-v1.json`).
2. `metadata.version` parses as semver.
3. `metadata.id` matches reverse-DNS regex `^[a-z][a-z0-9-]*(\.[a-z][a-z0-9-]*)+$`.
4. `engine` compatible with the WIT contract version in `.gtdx-contract.lock` (mismatch ≠ warning, is an error; `gtdx sync-wit` in Phase 4 fixes).
5. `capabilities.offered[].version` semver valid.
6. `permissions.network[]` entries are HTTPS origins (no wildcards).

Failure output aggregates all errors before exiting:

```
✗ describe.json validation failed (3 errors):
  • metadata.version: "0.1" — not a valid semver
  • capabilities.offered[0].version: "v1" — missing minor/patch
  • permissions.network[2]: "http://insecure.com" — must be https://

Fix these and re-run: gtdx publish
```

## 9. Error Exit Codes

| Error | Exit | User action |
|-------|------|-------------|
| describe.json invalid | 2 | fix file |
| WASM build failed | 70 | read cargo output |
| Version exists, no `--force` | 10 | bump version or pass `--force` |
| Signing key missing | 20 | `gtdx keygen <name>` |
| Registry not writable | 30 | `chmod` / `chown` |
| WIT contract mismatch | 40 | `gtdx sync-wit` (Phase 4) or fix manually |
| Registry `NotImplemented` (Store/OCI in Phase 1) | 50 | switch to `--registry local` |
| IO error | 74 | check disk / permissions |

## 10. Testing

- **Unit** `pack_writer::tests::determinism` — two builds, identical SHA256.
- **Unit** `local_fs::tests::publish_happy` — publish into tempdir, assert file layout + `index.json` contents.
- **Unit** `local_fs::tests::publish_conflict` — same version twice, assert `RegistryError::VersionExists` without `--force` and success with `--force`.
- **Unit** `local_fs::tests::concurrent_publish` — spawn two tokio tasks publishing different versions, assert no index corruption.
- **Unit** `validator::tests::*` — one test per validation rule in §8.
- **Integration** `tests/cli_publish.rs` — scaffold, publish, assert `gtdx install` + `gtdx info` round-trip.
- **E2E** integration gate (see roadmap §6.3).

## 11. Module Breakdown & LOC Budget

| Module | Purpose | Target LOC |
|--------|---------|------------|
| `publish_cmd.rs` | orchestration, flag handling | ~250 |
| `pack_writer.rs` (in `greentic-extension-sdk-contract`) | deterministic ZIP writer | ~200 |
| `local_fs_publish.rs` (extends `LocalFilesystem`) | impl `publish()` + lock + atomic rename | ~200 |
| `validator.rs` | pre-publish describe.json checks | ~150 |
| **Total new code** | | **~800 LOC** |

## 12. Acceptance Criteria

1. `gtdx publish --registry local` succeeds on a scaffolded project and writes the expected layout under `$GTDX_HOME/registries/local/`.
2. Two consecutive publishes of identical sources produce byte-identical `.gtxpack` (SHA256 equal), 10× verified.
3. Duplicate-version publish without `--force` fails with exit 10 and leaves registry unchanged.
4. `GreenticStoreRegistry::publish()` and `OciRegistry::publish()` return `NotImplemented` with an actionable hint.
5. `gtdx install` from the local filesystem registry round-trips; `gtdx info` shows the published version.
6. `describe.json` validation errors are aggregated (all 3+ errors shown in one invocation, not one-at-a-time).
7. `--sign` emits the "known limitations" warning on every run.

## 13. Non-Goals (Phase 1)

- ❌ JCS RFC 8785 canonical signing (Phase 2, S4)
- ❌ Store HTTP publish (Phase 2, S5)
- ❌ OCI publish (Phase 2, S5)
- ❌ Key passphrase encryption (Phase 2, S4)
- ❌ Strict trust policy + Store countersignature (Phase 2, S4)
- ❌ Auto version bump on conflict (explicit reject — too magical for Phase 1)
- ❌ SBOM emission inside `.gtxpack` (Phase 3 or later)
