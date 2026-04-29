# `gtdx` Finishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Close the remaining DX gaps so `gtdx` feels finished: search without a required query, an actionable `gtdx doctor` (toolchain + registries + credentials), mapped `gtdx publish` exit codes (spec §9), and `gtdx publish --format json` emitting JSONL. No new major features — this is polish so each command does what users expect.

**Architecture:** Three isolated changes: (1) `commands/search.rs` takes `Option<String>` query; (2) `commands/doctor.rs` extended with toolchain/registry/credential probes; (3) `publish/mod.rs` introduces a `PublishError` type mapping to numeric exit codes via a new `main.rs` exit-code layer, and the `run` dispatcher renders either human text or JSON lines per `--format`. No new crates or deps.

**Tech Stack:** Rust 1.94, `which`, `reqwest` (already present), `chrono` (already present for token-exp decode), `base64` (already present for JWT-payload decode).

---

## File Structure

### Modify
- `crates/greentic-ext-cli/src/commands/search.rs` — query becomes `Option<String>`.
- `crates/greentic-ext-cli/src/commands/doctor.rs` — expand to 4 check groups.
- `crates/greentic-ext-cli/src/commands/publish.rs` — wire `--format json` output + handle new `PublishError` exit codes.
- `crates/greentic-ext-cli/src/publish/mod.rs` — return a structured `PublishError` alongside `PublishOutcome`.
- `crates/greentic-ext-cli/src/main.rs` — translate `PublishError` to process exit codes.
- `CHANGELOG.md`, `docs/getting-started-publish.md` — entries.

### No new files.

---

## Task 1: `gtdx search` — optional query

**File:** `crates/greentic-ext-cli/src/commands/search.rs`

### Step 1: Make query optional

Replace the `pub query: String` field in `Args` with:

```rust
    /// Search term (partial-match on extension name). If omitted, lists everything the registry exposes.
    pub query: Option<String>,
```

Update the body: where it currently passes `query: Some(args.query)` to `SearchQuery`, change to `query: args.query` (already `Option<String>`).

### Step 2: Handle empty-result UX

After building `results`, if `results.is_empty()`, print an empty list hint:

```rust
    if results.is_empty() {
        println!("(no extensions match)");
        return Ok(());
    }
```

### Step 3: Verify

```bash
cargo build -p greentic-ext-cli --quiet
./target/debug/gtdx search --help
# Arguments: [QUERY]   (optional, square brackets)
```

### Step 4: Commit

```bash
git add crates/greentic-ext-cli/src/commands/search.rs
git commit -m "feat(ext-cli): gtdx search accepts optional QUERY (list all when omitted)"
```

---

## Task 2: `gtdx doctor` — toolchain + registries + credentials

**File:** `crates/greentic-ext-cli/src/commands/doctor.rs`

Current `doctor` only walks installed describes. Expand to four groups. Keep the existing describe-walk as one of the groups. Sections are labeled and emit `✓` / `⚠` / `✗` per item.

### Step 1: Replace file

```rust
use std::path::Path;

use chrono::{DateTime, Utc};
use clap::Args as ClapArgs;
use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_registry::credentials::Credentials;
use greentic_ext_registry::storage::Storage;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Skip network probes (offline mode).
    #[arg(long)]
    pub offline: bool,
}

pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    let mut failures = 0usize;
    println!("toolchain");
    failures += check_toolchain();
    println!();
    println!("registries ({home})", home = home.display());
    failures += check_registries(home, args.offline).await;
    println!();
    println!("credentials");
    failures += check_credentials(home);
    println!();
    println!("installed extensions");
    failures += check_installed(home)?;
    println!();
    if failures > 0 {
        println!("{failures} problem(s) found");
        std::process::exit(1);
    }
    println!("all checks passed");
    Ok(())
}

fn check_toolchain() -> usize {
    let mut fails = 0;
    for (name, hint) in [
        ("cargo", "install Rust from https://rustup.rs/"),
        (
            "cargo-component",
            "cargo install --locked cargo-component",
        ),
        ("rustup", "install Rust from https://rustup.rs/"),
    ] {
        match which::which(name) {
            Ok(path) => println!("  ✓ {name}  {}", path.display()),
            Err(_) => {
                println!("  ✗ {name} not found — {hint}");
                fails += 1;
            }
        }
    }
    match std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            if s.lines().any(|l| l.trim() == "wasm32-wasip2") {
                println!("  ✓ wasm32-wasip2 target installed");
            } else {
                println!(
                    "  ⚠ wasm32-wasip2 target missing — rustup target add wasm32-wasip2"
                );
                fails += 1;
            }
        }
        _ => {
            println!("  ⚠ cannot list rustup targets");
        }
    }
    fails
}

async fn check_registries(home: &Path, offline: bool) -> usize {
    let cfg = match greentic_ext_registry::config::load(&home.join("config.toml")) {
        Ok(c) => c,
        Err(e) => {
            println!("  ⚠ cannot read config.toml: {e}");
            return 1;
        }
    };
    if cfg.registries.is_empty() {
        println!("  ⚠ no registries configured — add one with: gtdx registries add <name> <url>");
        return 0;
    }
    let mut fails = 0;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("reqwest client");
    for entry in &cfg.registries {
        if offline {
            println!("  ⦿ {}  {}  (offline, not probed)", entry.name, entry.url);
            continue;
        }
        let health_url = format!("{}/health", entry.url.trim_end_matches('/'));
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                println!("  ✓ {}  {}", entry.name, entry.url);
            }
            Ok(resp) => {
                println!(
                    "  ⚠ {}  {}  (health={} at {})",
                    entry.name,
                    entry.url,
                    resp.status(),
                    health_url
                );
            }
            Err(e) => {
                println!("  ✗ {}  {}  ({e})", entry.name, entry.url);
                fails += 1;
            }
        }
    }
    fails
}

fn check_credentials(home: &Path) -> usize {
    let path = home.join("credentials.toml");
    let creds = match Credentials::load(&path) {
        Ok(c) => c,
        Err(e) => {
            println!("  ⚠ cannot read credentials.toml: {e}");
            return 1;
        }
    };
    if creds.tokens.is_empty() {
        println!("  ⦿ no tokens stored — run gtdx login --registry <name> when needed");
        return 0;
    }
    let mut fails = 0;
    for (name, token) in &creds.tokens {
        match jwt_exp(token) {
            Some(exp) if exp > Utc::now() => {
                let dur = exp - Utc::now();
                println!("  ✓ {name}  expires in {}h", dur.num_hours());
            }
            Some(_) => {
                println!("  ✗ {name}  token expired — run: gtdx login --registry {name}");
                fails += 1;
            }
            None => {
                println!(
                    "  ⦿ {name}  non-JWT token (cannot verify expiry)"
                );
            }
        }
    }
    fails
}

fn jwt_exp(token: &str) -> Option<DateTime<Utc>> {
    use base64::Engine as _;
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(payload))
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let exp = v.get("exp")?.as_i64()?;
    DateTime::from_timestamp(exp, 0)
}

fn check_installed(home: &Path) -> anyhow::Result<usize> {
    let storage = Storage::new(home);
    let mut total = 0usize;
    let mut bad = 0usize;
    for kind in [
        ExtensionKind::Design,
        ExtensionKind::Bundle,
        ExtensionKind::Deploy,
    ] {
        let dir = storage.kind_dir(kind);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            total += 1;
            let describe_path = entry.path().join("describe.json");
            if !describe_path.exists() {
                println!("  ✗ {} (no describe.json)", entry.path().display());
                bad += 1;
                continue;
            }
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    println!("  ✗ {}: invalid JSON: {e}", describe_path.display());
                    bad += 1;
                    continue;
                }
            };
            if let Err(e) = greentic_extension_sdk_contract::schema::validate_describe_json(&value) {
                println!("  ✗ {}: {e}", describe_path.display());
                bad += 1;
            } else {
                println!("  ✓ {}", describe_path.display());
            }
        }
    }
    if total == 0 {
        println!("  ⦿ no installed extensions");
    } else {
        println!("  {total} total, {bad} bad");
    }
    Ok(bad)
}
```

### Step 2: Update main.rs dispatch

Because `doctor::run` is now `async`, the match arm in `main.rs` must `.await`:

```rust
        Command::Doctor(args) => commands::doctor::run(args, &home).await,
```

(Change from `commands::doctor::run(args, &home)` if it was sync.)

### Step 3: Verify

```bash
cargo build -p greentic-ext-cli --quiet
./target/debug/gtdx doctor --help
./target/debug/gtdx doctor --offline   # local-only: fast smoke
```

Expected: three sections (toolchain, registries, credentials, installed extensions) emit checkmarks or hints.

### Step 4: Commit

```bash
git add crates/greentic-ext-cli/src/commands/doctor.rs crates/greentic-ext-cli/src/main.rs
git commit -m "feat(ext-cli): gtdx doctor expands to toolchain + registries + credentials + installed"
```

---

## Task 3: Publish exit codes

**Files:**
- Modify: `crates/greentic-ext-cli/src/publish/mod.rs`
- Modify: `crates/greentic-ext-cli/src/commands/publish.rs`
- Modify: `crates/greentic-ext-cli/src/main.rs`

Map Track C spec §9 exit codes:

| Error | Exit |
|-------|------|
| describe.json invalid | 2 |
| WASM build failed | 70 |
| Version exists, no --force | 10 |
| Auth required | 20 |
| Registry not writable | 30 |
| NotImplemented | 50 |
| IO error | 74 |
| Any other | 1 |

### Step 1: Define PublishError in publish/mod.rs

Add at the top of `publish/mod.rs` (below imports):

```rust
#[derive(Debug)]
pub enum PublishError {
    DescribeInvalid(String),      // exit 2
    Build(String),                // exit 70
    VersionExists(String),        // exit 10
    AuthRequired(String),         // exit 20
    RegistryNotWritable(String),  // exit 30
    NotImplemented(String),       // exit 50
    Io(String),                   // exit 74
    Other(anyhow::Error),         // exit 1
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublishError::DescribeInvalid(m)
            | PublishError::Build(m)
            | PublishError::VersionExists(m)
            | PublishError::AuthRequired(m)
            | PublishError::RegistryNotWritable(m)
            | PublishError::NotImplemented(m)
            | PublishError::Io(m) => write!(f, "{m}"),
            PublishError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PublishError {}

impl PublishError {
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            PublishError::DescribeInvalid(_) => 2,
            PublishError::VersionExists(_) => 10,
            PublishError::AuthRequired(_) => 20,
            PublishError::RegistryNotWritable(_) => 30,
            PublishError::NotImplemented(_) => 50,
            PublishError::Build(_) => 70,
            PublishError::Io(_) => 74,
            PublishError::Other(_) => 1,
        }
    }
}
```

### Step 2: Change run_publish return type

Replace `pub async fn run_publish(cfg: &PublishConfig) -> anyhow::Result<PublishOutcome>` with:

```rust
pub async fn run_publish(cfg: &PublishConfig) -> Result<PublishOutcome, PublishError> {
```

Convert internal `anyhow::bail!` calls to the right `PublishError` variant. Key conversion sites inside the existing body:

- After `validate_for_publish(&describe)` fails → `PublishError::DescribeInvalid(format_errors(&errors))`.
- After schema-validate fails → `PublishError::DescribeInvalid(format!("describe.json schema: {e}"))`.
- After `run_build` fails → `PublishError::Build(format!("{e}"))`.
- Mapping on the final `backend.publish(req).await` result:
  ```rust
  match backend.publish(req).await {
      Ok(r) => r,
      Err(greentic_ext_registry::RegistryError::VersionExists { existing_sha }) => {
          return Err(PublishError::VersionExists(format!(
              "version already exists (sha256={existing_sha})"
          )));
      }
      Err(greentic_ext_registry::RegistryError::AuthRequired(m)) => {
          return Err(PublishError::AuthRequired(m));
      }
      Err(greentic_ext_registry::RegistryError::NotImplemented { hint }) => {
          return Err(PublishError::NotImplemented(hint));
      }
      Err(greentic_ext_registry::RegistryError::Io(e)) => {
          return Err(PublishError::Io(e.to_string()));
      }
      Err(e) => return Err(PublishError::Other(anyhow::anyhow!("{e}"))),
  }
  ```
- Other internal I/O errors (e.g. `std::fs::read` / `std::fs::write` inside the orchestrator) → wrap in `PublishError::Io(err.to_string())` via a small helper:
  ```rust
  fn io_err<E: std::fmt::Display>(e: E) -> PublishError {
      PublishError::Io(e.to_string())
  }
  ```
  Use `.map_err(io_err)?` at the call sites that currently do `.map_err(|e| anyhow::anyhow!(...))?` on fs ops.

Any remaining catch-all → `PublishError::Other(anyhow::anyhow!("{e}"))`.

### Step 3: Update commands/publish.rs to route exit code

Change the `pub async fn run` signature to return `anyhow::Result<()>` still, but have it translate PublishError:

```rust
pub async fn run(args: Args, home: &Path) -> anyhow::Result<()> {
    if args.sign {
        eprintln!(
            "warning: Phase 1 signing reuses Wave 1 JCS sign_describe. ..."
        );
    }
    let project_dir = project_dir_from_manifest(&args.manifest)?;
    // ... build cfg ...
    match run_publish(&cfg).await {
        Ok(outcome) => {
            render_outcome(&args.format, outcome)?;
            Ok(())
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(err.exit_code());
        }
    }
}
```

Move the existing pretty-print into a new `fn render_outcome(format: &str, outcome: PublishOutcome) -> anyhow::Result<()>` (Task 4 will expand this to handle JSON format).

### Step 4: main.rs — no change required

`commands::publish::run` still returns `anyhow::Result<()>` from the caller's view. Exits happen inside via `std::process::exit`. No dispatcher change.

### Step 5: Verify build

```bash
cargo build -p greentic-ext-cli --quiet 2>&1 | tail -5
```

Expected: exit 0.

### Step 6: Commit

```bash
git add crates/greentic-ext-cli/src/publish/mod.rs crates/greentic-ext-cli/src/commands/publish.rs
git commit -m "feat(ext-cli): gtdx publish maps errors to numeric exit codes (spec §9)"
```

---

## Task 4: `gtdx publish --format json`

**File:** `crates/greentic-ext-cli/src/commands/publish.rs`

### Step 1: Expand render_outcome

Replace the `render_outcome` helper added in Task 3 with:

```rust
fn render_outcome(format: &str, outcome: PublishOutcome) -> anyhow::Result<()> {
    match format {
        "json" => render_json(&outcome),
        "human" => render_human(outcome),
        other => Err(anyhow::anyhow!("unknown --format: {other} (use human|json)")),
    }
}

fn render_human(outcome: PublishOutcome) -> anyhow::Result<()> {
    match outcome {
        PublishOutcome::DryRun { artifact, sha256, registry } => {
            println!(
                "dry-run: would publish {} to {}",
                artifact.display(),
                registry
            );
            println!("sha256: {sha256}");
        }
        PublishOutcome::VerifyOnly { ext_id, version, registry } => {
            println!("verify-only: {ext_id}@{version} slot free in {registry}");
        }
        PublishOutcome::Published {
            ext_id, version, sha256, artifact, receipt_path, signed, registry_url,
        } => {
            println!("\u{2713} published {ext_id}@{version}");
            println!("  artifact: {}", artifact.display());
            println!("  sha256:   {sha256}");
            println!("  registry: {registry_url}");
            println!("  signed:   {signed}");
            println!("  receipt:  {}", receipt_path.display());
        }
    }
    Ok(())
}

fn render_json(outcome: &PublishOutcome) -> anyhow::Result<()> {
    use serde_json::json;
    let v = match outcome {
        PublishOutcome::DryRun { artifact, sha256, registry } => json!({
            "event": "dry_run",
            "artifact": artifact.display().to_string(),
            "sha256": sha256,
            "registry": registry,
        }),
        PublishOutcome::VerifyOnly { ext_id, version, registry } => json!({
            "event": "verify_only",
            "ext_id": ext_id,
            "version": version,
            "registry": registry,
        }),
        PublishOutcome::Published {
            ext_id, version, sha256, artifact, receipt_path, signed, registry_url,
        } => json!({
            "event": "published",
            "ext_id": ext_id,
            "version": version,
            "sha256": sha256,
            "artifact": artifact.display().to_string(),
            "receipt_path": receipt_path.display().to_string(),
            "signed": signed,
            "registry_url": registry_url,
        }),
    };
    println!("{}", serde_json::to_string(&v)?);
    Ok(())
}
```

### Step 2: Verify with dry-run + JSON

```bash
cargo build -p greentic-ext-cli --quiet
TMP=$(mktemp -d); export GREENTIC_HOME="$TMP/home"
./target/debug/gtdx new demo --dir "$TMP/demo" --author tester -y --no-git
./target/debug/gtdx publish --manifest "$TMP/demo/Cargo.toml" --dry-run --format json | python3 -m json.tool
```

Expected: one JSON line with `event: "dry_run"`, `artifact`, `sha256`, `registry`.

### Step 3: Commit

```bash
git add crates/greentic-ext-cli/src/commands/publish.rs
git commit -m "feat(ext-cli): gtdx publish --format json emits JSON outcome lines"
```

---

## Task 5: Integration test — publish → install round-trip

**File:** `crates/greentic-ext-cli/tests/cli_publish.rs`

Append a gated test that exercises local publish + install via `gtdx install <name> --version <v> --registry file://<path>` against the same hierarchical layout the publish wrote.

### Step 1: Append test

```rust
#[test]
fn publish_to_local_then_install_round_trip() {
    if !gate() {
        eprintln!("skipped: set GTDX_RUN_BUILD=1 to enable");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("demo");
    let home = tmp.path().join("home");

    assert!(
        run(Command::new(gtdx_bin())
            .arg("new")
            .arg("demo")
            .arg("--dir")
            .arg(&proj)
            .arg("--author")
            .arg("tester")
            .arg("-y")
            .arg("--no-git"))
        .0
    );
    assert!(
        run(Command::new(gtdx_bin())
            .env("GREENTIC_HOME", &home)
            .arg("publish")
            .arg("--manifest")
            .arg(proj.join("Cargo.toml")))
        .0
    );

    // Install from the hierarchical registry via file:// URL.
    let reg_root = home.join("registries/local");
    let ext_dir = reg_root.join("com.example.demo/0.1.0");
    // install expects a direct .gtxpack path OR a registry that exposes the
    // pack via parse_pack_filename. Use the file path form.
    let pack_path = ext_dir.join("demo-0.1.0.gtxpack");
    assert!(pack_path.is_file(), "publish must write {}", pack_path.display());

    let home2 = tmp.path().join("home2");
    let (ok, o, e) = run(Command::new(gtdx_bin())
        .env("GREENTIC_HOME", &home2)
        .arg("install")
        .arg(pack_path.to_string_lossy().to_string())
        .arg("--trust")
        .arg("loose")
        .arg("-y"));
    assert!(ok, "gtdx install failed: {o}\n{e}");

    let installed = home2.join("extensions/design/com.example.demo-0.1.0");
    assert!(installed.exists(), "expected install at {}", installed.display());
    assert!(installed.join("describe.json").exists());
    assert!(installed.join("extension.wasm").exists());
}
```

### Step 2: Compile + default skip

```bash
cargo test -p greentic-ext-cli --test cli_publish 2>&1 | tail -5
```

Expected: 4 passed (3 existing + 1 new, all skip when gate off).

### Step 3: Commit

```bash
git add crates/greentic-ext-cli/tests/cli_publish.rs
git commit -m "test(ext-cli): gated publish → install round-trip integration"
```

---

## Task 6: CHANGELOG + doctor/search docs

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/getting-started-publish.md` (add exit-code + JSON format notes)

### Step 1: CHANGELOG

Append to Unreleased → Added:

```
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
```

### Step 2: docs/getting-started-publish.md

After the existing "Flags" table, add:

```markdown

## Exit codes

`gtdx publish` returns these exit codes:

| Code | Meaning                                     |
|------|---------------------------------------------|
| 0    | Published (or dry-run / verify-only OK).    |
| 2    | `describe.json` failed schema or business validation. |
| 10   | Version already exists; re-run with `--force`. |
| 20   | Auth required or token invalid; run `gtdx login`. |
| 30   | Registry not writable (permissions).        |
| 50   | Backend returns `NotImplemented` (e.g. OCI in Phase 1). |
| 70   | `cargo component build` failed — see compiler output. |
| 74   | I/O error (disk, network).                  |
| 1    | Any other failure.                          |

CI scripts can switch on these codes:

```bash
gtdx publish --registry mystore || {
  case $? in
    10) echo "skip: already published" ;;
    20) echo "need to refresh token" ;;
    *)  exit 1 ;;
  esac
}
```

## JSON output

`--format json` emits one JSON object per invocation on stdout:

```json
{"event":"published","ext_id":"gtdxsmoke2.mytest","version":"0.1.0","sha256":"089a1b56...","artifact":"./dist/mytest-0.1.0.gtxpack","receipt_path":"./dist/publish-gtdxsmoke2.mytest-0.1.0.json","signed":false,"registry_url":"http://.../api/v1/extensions/gtdxsmoke2.mytest/0.1.0"}
```

`event` is one of `dry_run` / `verify_only` / `published`.
```

### Step 3: Commit

```bash
git add CHANGELOG.md docs/getting-started-publish.md
git commit -m "docs: gtdx finishing — search/doctor/exit-codes/json-format entries"
```

---

## Task 7: Final gate + PR

Controller runs this directly.

- [ ] **Step 1:** `cargo fmt --all`
- [ ] **Step 2:** `cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -20` — exit 0.
- [ ] **Step 3:** `cargo test --workspace --all-targets 2>&1 | tail -20` — all green.
- [ ] **Step 4:** Commit stragglers if any.
- [ ] **Step 5:** Push + create PR.

---

## Acceptance

1. `gtdx search` with no QUERY lists all extensions from the default (or `--registry`) Store (Task 1 manual verify).
2. `gtdx doctor` prints four sections with actionable hints; exits 0 when all green, 1 with problem count otherwise (Task 2).
3. `gtdx publish` returns the exit code matching the spec §9 table for each error class (Task 3 + manual verify with `--sign --key-id missing` → 20, etc.).
4. `gtdx publish --format json` emits a single parseable JSON object (Task 4).
5. `gtdx install` round-trips a locally-published `.gtxpack` (Task 5).
6. Workspace fmt + clippy + test green (Task 7).
