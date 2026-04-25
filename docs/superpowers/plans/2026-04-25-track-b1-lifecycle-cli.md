# Track B1 — Lifecycle CLI + State Lib + Runtime Watch API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `gtdx enable/disable` CLI commands backed by an atomic state file, plus a higher-level `ExtensionRuntime::watch()` API on top of the existing `watcher.rs` primitives.

**Architecture:** New crate `greentic-ext-state` owns the state file format and atomic write semantics, consumed by both `gtdx` and `greentic-ext-runtime`. The runtime's existing `watcher::watch()` (raw FS events) gets wrapped in a higher-level `RuntimeEvent` stream that emits `ExtensionAdded/Removed/Changed/StateFileChanged`. Listing extends with a status column.

**Tech Stack:** Rust 1.94, edition 2024, serde, serde_json, fs2 (advisory lock), notify-debouncer-full (already used).

**Spec:** `docs/superpowers/specs/2026-04-25-designer-commercialization-backend-design.md` (Track B — CLI side, state lib, watch API)

**Branch / Worktree:**
```
git worktree add ~/works/greentic/gde-lifecycle -b feat/extension-lifecycle-cli main
```
PR target: `main`.

---

## File Structure

### Create

- `crates/greentic-ext-state/Cargo.toml`
- `crates/greentic-ext-state/src/lib.rs` (~80 LOC) — public re-exports
- `crates/greentic-ext-state/src/state.rs` (~150 LOC) — `ExtensionState` struct + serde
- `crates/greentic-ext-state/src/atomic.rs` (~100 LOC) — `save_atomic` + advisory lock
- `crates/greentic-ext-state/src/error.rs` (~30 LOC) — `StateError` thiserror
- `crates/greentic-ext-state/tests/state_roundtrip.rs` (~150 LOC) — integration tests
- `crates/greentic-ext-cli/src/commands/enable.rs` (~80 LOC)
- `crates/greentic-ext-cli/src/commands/disable.rs` (~120 LOC) — includes capability scan
- `crates/greentic-ext-runtime/src/events.rs` (~120 LOC) — `RuntimeEvent` + adapter from `FsEvent`

### Modify

- `Cargo.toml` (workspace root) — add `greentic-ext-state` to members; add `fs2 = "0.4"` to workspace deps
- `crates/greentic-ext-cli/Cargo.toml` — add `greentic-ext-state` dep
- `crates/greentic-ext-cli/src/main.rs` — register `Enable` and `Disable` variants
- `crates/greentic-ext-cli/src/commands/mod.rs` — `pub mod enable; pub mod disable;`
- `crates/greentic-ext-cli/src/commands/list.rs` — add `--status` flag + STATUS column
- `crates/greentic-ext-runtime/Cargo.toml` — add `greentic-ext-state` dep
- `crates/greentic-ext-runtime/src/lib.rs` — `pub mod events;` + re-export
- `crates/greentic-ext-runtime/src/runtime.rs` — add `ExtensionRuntime::watch()` method

---

## Task 1: Create `greentic-ext-state` crate skeleton

**Files:**
- Create: `crates/greentic-ext-state/Cargo.toml`
- Create: `crates/greentic-ext-state/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add to workspace members + deps**

In root `Cargo.toml`, add to `[workspace] members`:

```toml
"crates/greentic-ext-state",
```

In root `[workspace.dependencies]`:

```toml
fs2 = "0.4"
```

- [ ] **Step 2: Create crate manifest**

`crates/greentic-ext-state/Cargo.toml`:

```toml
[package]
name = "greentic-ext-state"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
fs2 = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 3: Create `lib.rs` skeleton**

`crates/greentic-ext-state/src/lib.rs`:

```rust
//! Extension lifecycle state — persistent enable/disable per extension.

mod atomic;
mod error;
mod state;

pub use error::StateError;
pub use state::ExtensionState;
```

- [ ] **Step 4: Stub the modules so the crate compiles**

`crates/greentic-ext-state/src/error.rs`:

```rust
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("parse: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("lock contention after {0} retries")]
    LockContention(u32),
}
```

`crates/greentic-ext-state/src/state.rs`:

```rust
use std::path::Path;

#[derive(Debug, Default)]
pub struct ExtensionState;

impl ExtensionState {
    pub fn load(_home: &Path) -> Result<Self, crate::StateError> {
        Ok(Self)
    }
}
```

`crates/greentic-ext-state/src/atomic.rs`:

```rust
// Placeholder; filled in Task 5.
```

- [ ] **Step 5: Build + commit**

Run: `cargo build -p greentic-ext-state`
Expected: succeeds.

```bash
git add Cargo.toml crates/greentic-ext-state/
git commit -m "feat(state): scaffold greentic-ext-state crate"
```

---

## Task 2: Implement `ExtensionState::load` + JSON schema

**Files:**
- Modify: `crates/greentic-ext-state/src/state.rs`
- Create: `crates/greentic-ext-state/tests/state_roundtrip.rs`

- [ ] **Step 1: Write failing test**

`crates/greentic-ext-state/tests/state_roundtrip.rs`:

```rust
use greentic_ext_state::ExtensionState;
use tempfile::TempDir;

#[test]
fn load_returns_default_when_file_missing() {
    let tmp = TempDir::new().unwrap();
    let state = ExtensionState::load(tmp.path()).unwrap();
    // missing file = empty default = everything enabled
    assert!(state.is_enabled("anything", "1.0.0"));
}

#[test]
fn load_parses_existing_state_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("extensions-state.json");
    std::fs::write(
        &path,
        r#"{
            "schema": "1.0",
            "default": { "enabled": { "ext.a@1.0.0": false, "ext.b@2.0.0": true } },
            "tenants": {}
        }"#,
    )
    .unwrap();
    let state = ExtensionState::load(tmp.path()).unwrap();
    assert!(!state.is_enabled("ext.a", "1.0.0"));
    assert!(state.is_enabled("ext.b", "2.0.0"));
    assert!(state.is_enabled("ext.c", "1.0.0")); // default true when absent
}
```

- [ ] **Step 2: Run — should fail (compile error: `is_enabled` not defined)**

Run: `cargo test -p greentic-ext-state`
Expected: FAIL.

- [ ] **Step 3: Implement state.rs**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const STATE_FILENAME: &str = "extensions-state.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExtensionState {
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default)]
    pub default: ScopeState,
    #[serde(default)]
    pub tenants: HashMap<String, ScopeState>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ScopeState {
    #[serde(default)]
    pub enabled: HashMap<String, bool>,
}

fn default_schema() -> String { "1.0".to_string() }

impl ExtensionState {
    pub fn load(home: &Path) -> Result<Self, crate::StateError> {
        let path = state_path(home);
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(serde_json::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn is_enabled(&self, ext_id: &str, version: &str) -> bool {
        let key = format!("{}@{}", ext_id, version);
        self.default.enabled.get(&key).copied().unwrap_or(true)
    }

    pub fn set_enabled(&mut self, ext_id: &str, version: &str, enabled: bool) {
        let key = format!("{}@{}", ext_id, version);
        self.default.enabled.insert(key, enabled);
    }
}

pub(crate) fn state_path(home: &Path) -> PathBuf {
    home.join(STATE_FILENAME)
}
```

- [ ] **Step 4: Run — should pass**

Run: `cargo test -p greentic-ext-state`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-state/
git commit -m "feat(state): implement load + is_enabled with default-true fallback"
```

---

## Task 3: Implement `save_atomic` with advisory lock

**Files:**
- Modify: `crates/greentic-ext-state/src/atomic.rs`
- Modify: `crates/greentic-ext-state/src/state.rs`
- Modify: `crates/greentic-ext-state/tests/state_roundtrip.rs`

- [ ] **Step 1: Write failing test**

Append to `tests/state_roundtrip.rs`:

```rust
#[test]
fn save_atomic_writes_then_reload_returns_same_data() {
    let tmp = TempDir::new().unwrap();
    let mut state = ExtensionState::default();
    state.set_enabled("ext.x", "0.1.0", false);
    state.save_atomic(tmp.path()).unwrap();

    let reloaded = ExtensionState::load(tmp.path()).unwrap();
    assert!(!reloaded.is_enabled("ext.x", "0.1.0"));
    assert!(reloaded.is_enabled("ext.y", "0.1.0")); // default true
}

#[test]
fn save_atomic_does_not_leave_tmp_on_disk() {
    let tmp = TempDir::new().unwrap();
    let state = ExtensionState::default();
    state.save_atomic(tmp.path()).unwrap();

    let entries: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    let name = entries[0].as_ref().unwrap().file_name();
    assert_eq!(name, "extensions-state.json");
}
```

- [ ] **Step 2: Run — should fail (no `save_atomic`)**

Run: `cargo test -p greentic-ext-state save_atomic`
Expected: FAIL.

- [ ] **Step 3: Implement `atomic.rs`**

```rust
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::StateError;

const MAX_LOCK_RETRIES: u32 = 3;
const LOCK_BACKOFF_MS: u64 = 50;

pub(crate) fn write_atomic(target: &Path, content: &[u8]) -> Result<(), StateError> {
    let lock_path = target.with_extension("lock");
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_path)?;

    acquire_lock(&lock_file)?;

    let tmp_path = target.with_extension("json.tmp");
    {
        let mut f = File::create(&tmp_path)?;
        f.write_all(content)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp_path, target)?;

    let _ = lock_file.unlock();
    let _ = std::fs::remove_file(&lock_path);
    Ok(())
}

fn acquire_lock(file: &File) -> Result<(), StateError> {
    for _ in 0..MAX_LOCK_RETRIES {
        if file.try_lock_exclusive().is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(LOCK_BACKOFF_MS));
    }
    Err(StateError::LockContention(MAX_LOCK_RETRIES))
}
```

- [ ] **Step 4: Wire `save_atomic` on `ExtensionState`**

Add to `state.rs`:

```rust
impl ExtensionState {
    pub fn save_atomic(&self, home: &Path) -> Result<(), crate::StateError> {
        let path = state_path(home);
        let content = serde_json::to_vec_pretty(self)?;
        crate::atomic::write_atomic(&path, &content)
    }
}
```

- [ ] **Step 5: Run — should pass**

Run: `cargo test -p greentic-ext-state`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-state/
git commit -m "feat(state): implement save_atomic with tmp+rename + advisory lock"
```

---

## Task 4: Concurrent-write smoke test

**Files:**
- Modify: `crates/greentic-ext-state/tests/state_roundtrip.rs`

- [ ] **Step 1: Add concurrency test**

```rust
#[test]
fn concurrent_writers_do_not_corrupt_file() {
    use std::sync::Arc;
    let tmp = Arc::new(TempDir::new().unwrap());
    let mut handles = vec![];
    for i in 0..10 {
        let tmp = tmp.clone();
        handles.push(std::thread::spawn(move || {
            let mut state = ExtensionState::load(tmp.path()).unwrap();
            state.set_enabled(&format!("ext.{}", i), "0.1.0", i % 2 == 0);
            // best-effort save; some may fail with LockContention which is OK
            let _ = state.save_atomic(tmp.path());
        }));
    }
    for h in handles { h.join().unwrap(); }

    // file must parse cleanly after the dust settles
    let final_state = ExtensionState::load(tmp.path()).unwrap();
    let _ = final_state; // just confirm parse OK
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p greentic-ext-state concurrent_writers_do_not_corrupt_file`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-state/tests/state_roundtrip.rs
git commit -m "test(state): concurrent writers do not corrupt state file"
```

---

## Task 5: Add `gtdx enable` command

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/enable.rs`
- Modify: `crates/greentic-ext-cli/src/commands/mod.rs`
- Modify: `crates/greentic-ext-cli/src/main.rs`
- Modify: `crates/greentic-ext-cli/Cargo.toml`

- [ ] **Step 1: Add dep**

In `crates/greentic-ext-cli/Cargo.toml` `[dependencies]`:

```toml
greentic-ext-state = { path = "../greentic-ext-state" }
```

- [ ] **Step 2: Create command**

`crates/greentic-ext-cli/src/commands/enable.rs`:

```rust
use anyhow::{Context, Result, anyhow};
use clap::Args;
use greentic_ext_state::ExtensionState;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct EnableArgs {
    /// Extension id, optionally with @version (e.g. greentic.foo@0.1.0).
    pub target: String,

    /// Override greentic home (defaults to ~/.greentic).
    #[arg(long)]
    pub home: Option<PathBuf>,
}

pub fn run(args: EnableArgs) -> Result<()> {
    let home = resolve_home(args.home)?;
    let (id, version) = parse_target(&args.target, &home)?;

    verify_installed(&home, &id, &version)?;

    let mut state = ExtensionState::load(&home).context("loading state")?;
    state.set_enabled(&id, &version, true);
    state.save_atomic(&home).context("saving state")?;

    tracing::info!(ext_id = %id, version = %version, action = "enable", "extension state changed");
    println!("Enabled: {}@{} (designer will reload)", id, version);
    Ok(())
}

pub(crate) fn resolve_home(opt: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = opt { return Ok(p); }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    Ok(home.join(".greentic"))
}

pub(crate) fn parse_target(target: &str, home: &std::path::Path) -> Result<(String, String)> {
    if let Some((id, ver)) = target.split_once('@') {
        return Ok((id.to_string(), ver.to_string()));
    }
    let versions = installed_versions(home, target)?;
    match versions.len() {
        0 => Err(anyhow!("extension not installed: {}", target)),
        1 => Ok((target.to_string(), versions.into_iter().next().unwrap())),
        _ => Err(anyhow!(
            "ambiguous version for {}: installed = [{}]. Specify with @<version>.",
            target, versions.join(", ")
        )),
    }
}

pub(crate) fn installed_versions(home: &std::path::Path, id: &str) -> Result<Vec<String>> {
    let mut out = vec![];
    for kind in ["design", "deploy", "bundle", "provider"] {
        let dir = home.join("extensions").join(kind);
        if !dir.exists() { continue; }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().into_string().unwrap_or_default();
            if let Some(rest) = name.strip_prefix(&format!("{}-", id)) {
                out.push(rest.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub(crate) fn verify_installed(home: &std::path::Path, id: &str, version: &str) -> Result<()> {
    let suffix = format!("{}-{}", id, version);
    for kind in ["design", "deploy", "bundle", "provider"] {
        if home.join("extensions").join(kind).join(&suffix).exists() {
            return Ok(());
        }
    }
    Err(anyhow!("extension not installed: {}@{}", id, version))
}
```

- [ ] **Step 3: Wire into `commands/mod.rs` and `main.rs`**

`commands/mod.rs`: add `pub mod enable;`

`main.rs`: add to `Command` enum:

```rust
/// Enable an installed extension.
Enable(commands::enable::EnableArgs),
```

In match arm:

```rust
Command::Enable(args) => commands::enable::run(args),
```

- [ ] **Step 4: Add integration test**

Create `crates/greentic-ext-cli/tests/cli_enable.rs`:

```rust
use tempfile::TempDir;

#[test]
fn enable_writes_state_file() {
    let tmp = TempDir::new().unwrap();
    // create fake installed extension dir
    std::fs::create_dir_all(tmp.path().join("extensions/design/test.foo-0.1.0")).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["enable", "test.foo@0.1.0", "--home", tmp.path().to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(tmp.path().join("extensions-state.json")).unwrap();
    assert!(content.contains("\"test.foo@0.1.0\""));
    assert!(content.contains("true"));
}

#[test]
fn enable_errors_when_not_installed() {
    let tmp = TempDir::new().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["enable", "missing.ext@0.1.0", "--home", tmp.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not installed"));
}

#[test]
fn enable_errors_on_ambiguous_version() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("extensions/design/test.foo-0.1.0")).unwrap();
    std::fs::create_dir_all(tmp.path().join("extensions/design/test.foo-0.2.0")).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["enable", "test.foo", "--home", tmp.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ambiguous"));
}
```

- [ ] **Step 5: Run**

Run: `cargo test -p greentic-ext-cli --test cli_enable`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): add gtdx enable command"
```

---

## Task 6: Add `gtdx disable` command + capability dependency warning

**Files:**
- Create: `crates/greentic-ext-cli/src/commands/disable.rs`
- Modify: `crates/greentic-ext-cli/src/commands/mod.rs`
- Modify: `crates/greentic-ext-cli/src/main.rs`

- [ ] **Step 1: Create command (reuse helpers from enable.rs)**

`crates/greentic-ext-cli/src/commands/disable.rs`:

```rust
use anyhow::{Context, Result};
use clap::Args;
use greentic_ext_state::ExtensionState;
use std::path::PathBuf;

use super::enable::{installed_versions, parse_target, resolve_home, verify_installed};

#[derive(Debug, Args)]
pub struct DisableArgs {
    /// Extension id, optionally with @version.
    pub target: String,

    #[arg(long)]
    pub home: Option<PathBuf>,
}

pub fn run(args: DisableArgs) -> Result<()> {
    let home = resolve_home(args.home)?;
    let (id, version) = parse_target(&args.target, &home)?;
    verify_installed(&home, &id, &version)?;

    warn_dependents(&home, &id)?;

    let mut state = ExtensionState::load(&home).context("loading state")?;
    state.set_enabled(&id, &version, false);
    state.save_atomic(&home).context("saving state")?;

    tracing::info!(ext_id = %id, version = %version, action = "disable", "extension state changed");
    println!("Disabled: {}@{} (designer will reload)", id, version);
    Ok(())
}

fn warn_dependents(home: &std::path::Path, target_id: &str) -> Result<()> {
    let target_capabilities = read_offered_capabilities(home, target_id)?;
    if target_capabilities.is_empty() { return Ok(()); }

    for kind in ["design", "deploy", "bundle", "provider"] {
        let kind_dir = home.join("extensions").join(kind);
        if !kind_dir.exists() { continue; }
        for entry in std::fs::read_dir(&kind_dir)? {
            let entry = entry?;
            let describe_path = entry.path().join("describe.json");
            if !describe_path.exists() { continue; }
            let describe: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&describe_path)?)?;
            let id = describe["metadata"]["id"].as_str().unwrap_or("");
            if id == target_id { continue; }
            let required = describe["capabilities"]["required"]
                .as_array().cloned().unwrap_or_default();
            for cap in required {
                if let Some(cap_str) = cap.as_str() {
                    if target_capabilities.iter().any(|c| c == cap_str) {
                        eprintln!(
                            "warning: extension {} requires capability '{}' from {}. Disabling may break it.",
                            id, cap_str, target_id
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn read_offered_capabilities(home: &std::path::Path, target_id: &str) -> Result<Vec<String>> {
    let versions = installed_versions(home, target_id)?;
    if versions.is_empty() { return Ok(vec![]); }
    for kind in ["design", "deploy", "bundle", "provider"] {
        for v in &versions {
            let path = home.join("extensions").join(kind)
                .join(format!("{}-{}", target_id, v))
                .join("describe.json");
            if !path.exists() { continue; }
            let describe: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path)?)?;
            return Ok(describe["capabilities"]["offered"]
                .as_array().cloned().unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect());
        }
    }
    Ok(vec![])
}
```

- [ ] **Step 2: Wire into command registry**

Same as enable: `commands/mod.rs` + `main.rs` Command enum.

- [ ] **Step 3: Add tests**

Create `crates/greentic-ext-cli/tests/cli_disable.rs`:

```rust
use tempfile::TempDir;

#[test]
fn disable_sets_state_false() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("extensions/design/test.bar-0.1.0")).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["disable", "test.bar@0.1.0", "--home", tmp.path().to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(tmp.path().join("extensions-state.json")).unwrap();
    assert!(content.contains("\"test.bar@0.1.0\""));
    assert!(content.contains("false"));
}

#[test]
fn disable_warns_when_dependent_extension_present() {
    let tmp = TempDir::new().unwrap();
    let provider_dir = tmp.path().join("extensions/design/test.cap-provider-0.1.0");
    std::fs::create_dir_all(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("describe.json"), r#"{
        "metadata": { "id": "test.cap-provider" },
        "capabilities": { "offered": ["test:cap-x"] }
    }"#).unwrap();

    let dependent_dir = tmp.path().join("extensions/design/test.cap-consumer-0.1.0");
    std::fs::create_dir_all(&dependent_dir).unwrap();
    std::fs::write(dependent_dir.join("describe.json"), r#"{
        "metadata": { "id": "test.cap-consumer" },
        "capabilities": { "required": ["test:cap-x"] }
    }"#).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["disable", "test.cap-provider@0.1.0", "--home", tmp.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("test.cap-consumer"));
    assert!(stderr.contains("test:cap-x"));
}
```

- [ ] **Step 4: Run**

Run: `cargo test -p greentic-ext-cli --test cli_disable`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): add gtdx disable with capability dependency warning"
```

---

## Task 7: Extend `gtdx list --status`

**Files:**
- Modify: `crates/greentic-ext-cli/src/commands/list.rs`

- [ ] **Step 1: Add `--status` flag**

In `list.rs`, find the `ListArgs` struct and add:

```rust
/// Show enabled/disabled status column.
#[arg(long)]
pub status: bool,
```

- [ ] **Step 2: Modify the printing logic**

Find where extensions are printed; add a STATUS column when `args.status` is true:

```rust
if args.status {
    let state = greentic_ext_state::ExtensionState::load(&home).unwrap_or_default();
    let status = if state.is_enabled(id, version) { "enabled" } else { "disabled" };
    println!("{:<40} {:<12} {:<10} {}", id, version, kind, status);
} else {
    println!("{:<40} {:<12} {}", id, version, kind);
}
```

(Adapt to existing print format; keep column widths consistent.)

- [ ] **Step 3: Add test**

Append to `crates/greentic-ext-cli/tests/cli_list.rs` (create if absent):

```rust
use tempfile::TempDir;

#[test]
fn list_status_shows_disabled_extensions() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("extensions/design/test.qux-0.1.0")).unwrap();
    // disable it first
    std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["disable", "test.qux@0.1.0", "--home", tmp.path().to_str().unwrap()])
        .status().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args(["list", "--status", "--home", tmp.path().to_str().unwrap()])
        .output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test.qux"));
    assert!(stdout.contains("disabled"));
}
```

(If `list` doesn't currently accept `--home`, add it for testability matching enable/disable.)

- [ ] **Step 4: Run**

Run: `cargo test -p greentic-ext-cli --test cli_list`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): gtdx list --status shows enabled/disabled column"
```

---

## Task 8: Add `RuntimeEvent` enum + adapter from `FsEvent`

**Files:**
- Create: `crates/greentic-ext-runtime/src/events.rs`
- Modify: `crates/greentic-ext-runtime/src/lib.rs`

- [ ] **Step 1: Create events module**

`crates/greentic-ext-runtime/src/events.rs`:

```rust
//! High-level extension lifecycle events derived from raw FS events.

use std::path::Path;

use crate::watcher::FsEvent;

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    ExtensionAdded(String, String),    // (id, version)
    ExtensionRemoved(String, String),
    ExtensionChanged(String, String),
    StateFileChanged,
}

const STATE_FILE_NAME: &str = "extensions-state.json";

/// Convert a raw `FsEvent` into a `RuntimeEvent`, given the greentic home root.
/// Returns None for events outside the watched namespace.
pub fn classify(event: &FsEvent, home: &Path) -> Option<RuntimeEvent> {
    let path = match event {
        FsEvent::Added(p) | FsEvent::Modified(p) | FsEvent::Removed(p) => p,
    };
    if path.file_name().map(|n| n == STATE_FILE_NAME).unwrap_or(false)
        && path.parent() == Some(home) {
        return Some(RuntimeEvent::StateFileChanged);
    }

    let ext_root = home.join("extensions");
    let rel = path.strip_prefix(&ext_root).ok()?;
    let mut comps = rel.components();
    let _kind = comps.next()?;
    let dir_name = comps.next()?.as_os_str().to_str()?;

    let (id, version) = parse_dir_name(dir_name)?;
    let event = match event {
        FsEvent::Added(_) => RuntimeEvent::ExtensionAdded(id, version),
        FsEvent::Removed(_) => RuntimeEvent::ExtensionRemoved(id, version),
        FsEvent::Modified(_) => RuntimeEvent::ExtensionChanged(id, version),
    };
    Some(event)
}

fn parse_dir_name(s: &str) -> Option<(String, String)> {
    let dash = s.rfind('-')?;
    let id = &s[..dash];
    let version = &s[dash + 1..];
    Some((id.to_string(), version.to_string()))
}
```

- [ ] **Step 2: Re-export**

`crates/greentic-ext-runtime/src/lib.rs`:

```rust
pub mod events;
pub use events::{RuntimeEvent, classify};
```

- [ ] **Step 3: Test classifier**

Create `crates/greentic-ext-runtime/tests/events_classify.rs`:

```rust
use greentic_ext_runtime::events::{classify, RuntimeEvent};
use greentic_ext_runtime::watcher::FsEvent;
use std::path::PathBuf;

#[test]
fn state_file_change_classified_correctly() {
    let home = PathBuf::from("/home/test/.greentic");
    let event = FsEvent::Modified(home.join("extensions-state.json"));
    let r = classify(&event, &home);
    assert!(matches!(r, Some(RuntimeEvent::StateFileChanged)));
}

#[test]
fn extension_added_classified_correctly() {
    let home = PathBuf::from("/home/test/.greentic");
    let p = home.join("extensions").join("design").join("greentic.foo-0.1.0").join("describe.json");
    let event = FsEvent::Added(p);
    match classify(&event, &home) {
        Some(RuntimeEvent::ExtensionAdded(id, ver)) => {
            assert_eq!(id, "greentic.foo");
            assert_eq!(ver, "0.1.0");
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn out_of_namespace_event_returns_none() {
    let home = PathBuf::from("/home/test/.greentic");
    let event = FsEvent::Modified(PathBuf::from("/tmp/random.txt"));
    assert!(classify(&event, &home).is_none());
}
```

- [ ] **Step 4: Run**

Run: `cargo test -p greentic-ext-runtime --test events_classify`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-runtime/
git commit -m "feat(runtime): add RuntimeEvent enum + classify() adapter"
```

---

## Task 9: Add `ExtensionRuntime::watch()` method

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Modify: `crates/greentic-ext-runtime/Cargo.toml`

- [ ] **Step 1: Add `greentic-ext-state` dep**

`crates/greentic-ext-runtime/Cargo.toml` — add to `[dependencies]`:

```toml
greentic-ext-state = { path = "../greentic-ext-state" }
```

- [ ] **Step 2: Add `watch()` method**

In `crates/greentic-ext-runtime/src/runtime.rs`, add:

```rust
use crate::events::{classify, RuntimeEvent};
use crate::watcher::{watch as watch_fs, WatchHandle};
use std::sync::Arc;
use std::thread;

impl ExtensionRuntime {
    /// Start watching `~/.greentic/extensions/` and `extensions-state.json`,
    /// invoking `callback` for each higher-level RuntimeEvent.
    /// The returned `WatchHandle` keeps the watcher alive — drop it to stop.
    pub fn watch(
        &self,
        home: std::path::PathBuf,
        callback: Arc<dyn Fn(RuntimeEvent) + Send + Sync>,
    ) -> Result<WatchHandle, crate::error::RuntimeError> {
        let paths = vec![
            home.join("extensions"),
            home.clone(), // for extensions-state.json (file watch via parent dir)
        ];
        let (rx, handle) = watch_fs(&paths)?;

        let home_clone = home.clone();
        thread::spawn(move || {
            while let Ok(fs_event) = rx.recv() {
                if let Some(runtime_event) = classify(&fs_event, &home_clone) {
                    callback(runtime_event);
                }
            }
        });

        Ok(handle)
    }
}
```

- [ ] **Step 3: Add integration test**

Create `crates/greentic-ext-runtime/tests/runtime_watch.rs`:

```rust
use greentic_ext_runtime::events::RuntimeEvent;
use greentic_ext_runtime::runtime::ExtensionRuntime;
use greentic_ext_runtime::types::DiscoveryPaths;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[test]
fn watch_emits_state_file_changed_event() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("extensions/design")).unwrap();

    let runtime = ExtensionRuntime::new(greentic_ext_runtime::RuntimeConfig {
        paths: DiscoveryPaths { /* fields per existing struct; minimal */
            ..Default::default()
        },
    }).unwrap();

    let received: Arc<Mutex<Vec<RuntimeEvent>>> = Arc::new(Mutex::new(vec![]));
    let received_clone = received.clone();
    let _handle = runtime.watch(tmp.path().to_path_buf(), Arc::new(move |ev| {
        received_clone.lock().unwrap().push(ev);
    })).unwrap();

    // Trigger a state file change
    std::fs::write(tmp.path().join("extensions-state.json"), "{}").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(800)); // debounce 500ms

    let events = received.lock().unwrap();
    assert!(events.iter().any(|e| matches!(e, RuntimeEvent::StateFileChanged)));
}
```

(Adapt `RuntimeConfig::paths` field per the actual struct in `types.rs`.)

- [ ] **Step 4: Run**

Run: `cargo test -p greentic-ext-runtime --test runtime_watch`
Expected: PASS (allow up to 1.5s for debounce).

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-runtime/
git commit -m "feat(runtime): add ExtensionRuntime::watch() emitting RuntimeEvent"
```

---

## Task 10: Documentation

**Files:**
- Create: `docs/lifecycle-management.md`
- Modify: `docs/gtdx-cli.md`

- [ ] **Step 1: Write `docs/lifecycle-management.md`**

```markdown
# Extension Lifecycle Management

Greentic extensions persist enable/disable state in
`~/.greentic/extensions-state.json` (schema 1.0). Default behavior: any
extension absent from the file is treated as **enabled**. First boot after
upgrade requires no migration.

## CLI

```bash
gtdx enable  <id>[@<version>]
gtdx disable <id>[@<version>]
gtdx list --status
```

`<id>@<version>` is required when multiple versions of the same extension
are installed; otherwise version is inferred.

`disable` warns (but does not block) if other installed extensions declare
`capabilities.required` for any capability offered by the target.

## State file format

```json
{
  "schema": "1.0",
  "default": {
    "enabled": {
      "greentic.llm-openai@0.1.0": true,
      "greentic.adaptive-cards@1.6.0": false
    }
  },
  "tenants": {}
}
```

`tenants` is reserved for the future designer-admin track and is empty in
this release.

## Hot reload

The designer subscribes to `~/.greentic/extensions-state.json` and
`~/.greentic/extensions/` via `ExtensionRuntime::watch()`. Toggling
extension state takes effect within ~1 second without restarting the
designer.
```

- [ ] **Step 2: Update `docs/gtdx-cli.md`**

Add to the command reference:

```markdown
### `gtdx enable <target>`

Enable an installed extension. `<target>` is `<id>` or `<id>@<version>`.

### `gtdx disable <target>`

Disable an installed extension. Warns if other extensions depend on its
offered capabilities.

### `gtdx list [--status]`

When `--status` is given, adds an `enabled`/`disabled` column. Extensions
absent from the state file are reported as `enabled` (default).
```

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs(lifecycle): document enable/disable + state file format"
```

---

## Task 11: CI + PR

- [ ] **Step 1: Run local CI**

Run: `ci/local_check.sh`
Expected: PASS.

- [ ] **Step 2: Push branch**

```bash
git push -u origin feat/extension-lifecycle-cli
```

- [ ] **Step 3: Open PR**

```bash
gh pr create --title "feat: extension lifecycle CLI + state lib + runtime watch API" \
  --base main \
  --body "$(cat <<'EOF'
## Summary

- New crate `greentic-ext-state` — atomic JSON state file with advisory lock
- `gtdx enable <id>[@<version>]`
- `gtdx disable <id>[@<version>]` with capability dependency warning
- `gtdx list --status` adds enabled/disabled column
- `ExtensionRuntime::watch()` wraps existing `watcher.rs` and emits `RuntimeEvent`

## Test plan

- [x] State file round-trip + concurrency test
- [x] CLI enable/disable integration tests
- [x] Capability warning fires on dependent extension
- [x] Runtime watch emits StateFileChanged within debounce window

Spec: `docs/superpowers/specs/2026-04-25-designer-commercialization-backend-design.md` (Track B — CLI side)
Companion PR (designer side): `feat/extension-lifecycle-backend` in `greentic-designer`
EOF
)"
```

---

## Self-review checklist

- [x] State file schema v1.0 covered (Tasks 2–4)
- [x] Atomic write + advisory lock (Task 3)
- [x] `gtdx enable/disable` with version disambiguation (Tasks 5, 6)
- [x] Capability dependency warning (Task 6)
- [x] `gtdx list --status` (Task 7)
- [x] `ExtensionRuntime::watch()` API (Tasks 8, 9)
- [x] All identifiers consistent (`ExtensionState`, `RuntimeEvent`, `WatchHandle`)
- [x] No "TBD" / placeholder steps
- [x] Each task has runnable test + impl + commit
