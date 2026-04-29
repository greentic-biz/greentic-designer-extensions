# Designer Extension — Foundation Implementation Plan (1 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the extension foundation — Cargo workspace, WIT interfaces, contract crate (types + JSON Schema validator + WIT bindings), and runtime crate (wasmtime loader + capability registry + host broker + hot reload). End state: runtime can discover, load, invoke test WASM extensions via integration tests.

**Architecture:** Four-crate workspace: `greentic-extension-sdk-contract` defines pure types + `describe.json` schema + WIT bindgen re-exports; `greentic-ext-runtime` implements wasmtime-based loader, capability registry with semver matching, host broker with permission gates, and `notify`-based hot reload using `ArcSwap` for lockless reads; `greentic-extension-sdk-testing` provides test utilities (fixture builders, in-memory filesystem); `greentic-ext-cli` scaffolded here but implementation in Plan 2.

**Tech Stack:** Rust 1.91 edition 2024, wasmtime 43 + Component Model, wit-bindgen 0.35, cargo-component, `notify` + `debouncer`, `arc-swap`, `semver`, `ed25519-dalek`, `jsonschema`, `serde` + `serde_json`, `tokio`, `tracing`, `thiserror`/`anyhow`.

**Source spec:** `docs/superpowers/specs/2026-04-17-designer-extension-system-design.md` sections 4-7.

---

## Phase 1 — Workspace scaffolding

### Task 1.1: Initialize Cargo workspace

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`

- [ ] **Step 1: Create `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.91.0"
components = ["rustfmt", "clippy"]
targets = ["wasm32-wasip2"]
```

- [ ] **Step 2: Create `rustfmt.toml`**

```toml
edition = "2024"
max_width = 100
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
reorder_imports = true
```

- [ ] **Step 3: Create workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/greentic-extension-sdk-contract",
    "crates/greentic-ext-runtime",
    "crates/greentic-ext-cli",
    "crates/greentic-extension-sdk-testing",
]

[workspace.package]
edition = "2024"
version = "0.1.0"
license = "MIT"
repository = "https://github.com/greenticai/greentic-designer-extensions"
rust-version = "1.91"

[workspace.dependencies]
anyhow = "1"
arc-swap = "1.7"
base64 = "0.22"
clap = { version = "4.5", features = ["derive", "env"] }
ed25519-dalek = { version = "2.1", features = ["serde", "rand_core"] }
jsonschema = "0.30"
notify = "7"
notify-debouncer-full = "0.4"
rand = "0.8"
semver = { version = "1", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
tempfile = "3"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["io"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
wasmtime = { version = "43", features = ["component-model", "async"] }
wasmtime-wasi = "43"
wasmtime-wasi-http = "43"
wit-bindgen = "0.35"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
missing_errors_doc = "allow"
missing_panics_doc = "allow"
module_name_repetitions = "allow"
```

- [ ] **Step 4: Run `cargo check` to confirm workspace parses**

Run: `cargo check --workspace`
Expected: fails because members don't exist yet. That's OK for now.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml rustfmt.toml
git commit -m "chore: scaffold cargo workspace for ext foundation"
```

### Task 1.2: Local CI script

**Files:**
- Create: `ci/local_check.sh`
- Modify: `.gitignore`

- [ ] **Step 1: Create `ci/local_check.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "==> cargo fmt"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

echo "==> cargo test"
cargo test --workspace --all-features --locked

echo "==> cargo build (release)"
cargo build --workspace --locked --release

echo "All checks passed."
```

- [ ] **Step 2: Make executable**

```bash
chmod +x ci/local_check.sh
```

- [ ] **Step 3: Update `.gitignore`**

```
/target
**/target
/*.gtxpack
**/*.gtxpack
.cargo/credentials.toml
.greentic/
Cargo.lock.bak
```

- [ ] **Step 4: Commit**

```bash
git add ci/local_check.sh .gitignore
git commit -m "ci: add local check script"
```

### Task 1.3: GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/dependabot.yml`

- [ ] **Step 1: Create `.github/workflows/ci.yml`**

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@1.91.0
        with:
          components: rustfmt, clippy
          targets: wasm32-wasip2
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
      - run: cargo test --workspace --all-features --locked
      - run: cargo build --workspace --locked --release
```

- [ ] **Step 2: Create `.github/dependabot.yml`**

```yaml
version: 2
updates:
  - package-ecosystem: cargo
    directory: /
    schedule:
      interval: weekly
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
```

- [ ] **Step 3: Commit**

```bash
git add .github/
git commit -m "ci: add github actions + dependabot"
```

---

## Phase 2 — WIT interfaces

### Task 2.1: Write `wit/extension-base.wit`

**Files:**
- Create: `wit/extension-base.wit`

- [ ] **Step 1: Write the file exactly**

```wit
package greentic:extension-base@0.1.0;

interface types {
  record extension-identity {
    id: string,
    version: string,
    kind: kind,
  }

  enum kind {
    design,
    bundle,
    deploy,
  }

  record capability-ref {
    id: string,
    version: string,
  }

  record diagnostic {
    severity: severity,
    code: string,
    message: string,
    path: option<string>,
  }

  enum severity {
    error,
    warning,
    info,
    hint,
  }

  variant extension-error {
    invalid-input(string),
    missing-capability(string),
    permission-denied(string),
    internal(string),
  }
}

interface manifest {
  use types.{extension-identity, capability-ref};
  get-identity: func() -> extension-identity;
  get-offered: func() -> list<capability-ref>;
  get-required: func() -> list<capability-ref>;
}

interface lifecycle {
  use types.{extension-error};
  init: func(config-json: string) -> result<_, extension-error>;
  shutdown: func();
}

world extension-base-world {
  export manifest;
  export lifecycle;
}
```

- [ ] **Step 2: Commit**

```bash
git add wit/extension-base.wit
git commit -m "wit: add greentic:extension-base@0.1.0 interfaces"
```

### Task 2.2: Write `wit/extension-host.wit`

**Files:**
- Create: `wit/extension-host.wit`

- [ ] **Step 1: Write the file**

```wit
package greentic:extension-host@0.1.0;

interface logging {
  enum level {
    trace,
    debug,
    info,
    warn,
    error,
  }
  log: func(level: level, target: string, message: string);
  log-kv: func(level: level, target: string, message: string,
               fields: list<tuple<string, string>>);
}

interface i18n {
  t: func(key: string) -> string;
  tf: func(key: string, args: list<tuple<string, string>>) -> string;
}

interface secrets {
  get: func(uri: string) -> result<string, string>;
}

interface broker {
  call-extension: func(
    kind: string,
    target-id: string,
    function: string,
    args-json: string
  ) -> result<string, string>;
}

interface http {
  record request {
    method: string,
    url: string,
    headers: list<tuple<string, string>>,
    body: option<list<u8>>,
  }
  record response {
    status: u16,
    headers: list<tuple<string, string>>,
    body: list<u8>,
  }
  fetch: func(req: request) -> result<response, string>;
}

world extension-host-world {
  import logging;
  import i18n;
  import secrets;
  import broker;
  import http;
}
```

- [ ] **Step 2: Commit**

```bash
git add wit/extension-host.wit
git commit -m "wit: add greentic:extension-host@0.1.0 interfaces"
```

### Task 2.3: Write `wit/extension-design.wit`

**Files:**
- Create: `wit/extension-design.wit`

- [ ] **Step 1: Write the file**

```wit
package greentic:extension-design@0.1.0;

use greentic:extension-base/types@0.1.0.{diagnostic, extension-error};

interface tools {
  record tool-definition {
    name: string,
    description: string,
    input-schema-json: string,
    output-schema-json: option<string>,
  }
  list-tools: func() -> list<tool-definition>;
  invoke-tool: func(name: string, args-json: string) -> result<string, extension-error>;
}

interface validation {
  record validate-result {
    valid: bool,
    diagnostics: list<diagnostic>,
  }
  validate-content: func(content-type: string, content-json: string) -> validate-result;
}

interface prompting {
  record prompt-fragment {
    section: string,
    content-markdown: string,
    priority: u32,
  }
  system-prompt-fragments: func() -> list<prompt-fragment>;
}

interface knowledge {
  record entry-summary {
    id: string,
    title: string,
    category: string,
    tags: list<string>,
  }
  record entry {
    id: string,
    title: string,
    category: string,
    tags: list<string>,
    content-json: string,
  }
  list-entries: func(category-filter: option<string>) -> list<entry-summary>;
  get-entry: func(id: string) -> result<entry, extension-error>;
  suggest-entries: func(query: string, limit: u32) -> list<entry-summary>;
}

world design-extension {
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i@0.1.018n;
  import greentic:extension-host/secrets@0.1.0;
  import greentic:extension-host/broker@0.1.0;
  import greentic:extension-host/http@0.1.0;

  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export tools;
  export validation;
  export prompting;
  export knowledge;
}
```

- [ ] **Step 2: Commit**

```bash
git add wit/extension-design.wit
git commit -m "wit: add greentic:extension-design@0.1.0 interfaces"
```

### Task 2.4: Write `wit/extension-bundle.wit`

**Files:**
- Create: `wit/extension-bundle.wit`

- [ ] **Step 1: Write the file**

```wit
package greentic:extension-bundle@0.1.0;

use greentic:extension-base/types@0.1.0.{diagnostic, extension-error};

interface recipes {
  record recipe-summary {
    id: string,
    display-name: string,
    description: string,
    icon-path: option<string>,
  }
  list-recipes: func() -> list<recipe-summary>;
  recipe-config-schema: func(recipe-id: string) -> result<string, extension-error>;
  supported-capabilities: func(recipe-id: string) -> result<list<string>, extension-error>;
}

interface bundling {
  record designer-session {
    flows-json: string,
    contents-json: string,
    assets: list<tuple<string, list<u8>>>,
    capabilities-used: list<string>,
  }
  record bundle-artifact {
    filename: string,
    bytes: list<u8>,
    sha256: string,
  }
  validate-config: func(recipe-id: string, config-json: string) -> list<diagnostic>;
  render: func(recipe-id: string, config-json: string, session: designer-session)
    -> result<bundle-artifact, extension-error>;
}

world bundle-extension {
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i@0.1.018n;
  import greentic:extension-host/broker@0.1.0;

  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export recipes;
  export bundling;
}
```

- [ ] **Step 2: Commit**

```bash
git add wit/extension-bundle.wit
git commit -m "wit: add greentic:extension-bundle@0.1.0 interfaces"
```

### Task 2.5: Write `wit/extension-deploy.wit`

**Files:**
- Create: `wit/extension-deploy.wit`

- [ ] **Step 1: Write the file**

```wit
package greentic:extension-deploy@0.1.0;

use greentic:extension-base/types@0.1.0.{diagnostic, extension-error};

interface targets {
  record target-summary {
    id: string,
    display-name: string,
    description: string,
    icon-path: option<string>,
    supports-rollback: bool,
  }
  list-targets: func() -> list<target-summary>;
  credential-schema: func(target-id: string) -> result<string, extension-error>;
  config-schema: func(target-id: string) -> result<string, extension-error>;
  validate-credentials: func(target-id: string, credentials-json: string) -> list<diagnostic>;
}

interface deployment {
  record deploy-request {
    target-id: string,
    artifact-bytes: list<u8>,
    credentials-json: string,
    config-json: string,
    deployment-name: string,
  }
  enum deploy-status {
    pending,
    provisioning,
    configuring,
    starting,
    running,
    failed,
    rolled-back,
  }
  record deploy-job {
    id: string,
    status: deploy-status,
    message: string,
    endpoints: list<string>,
  }
  deploy: func(req: deploy-request) -> result<deploy-job, extension-error>;
  poll: func(job-id: string) -> result<deploy-job, extension-error>;
  rollback: func(job-id: string) -> result<_, extension-error>;
}

world deploy-extension {
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i@0.1.018n;
  import greentic:extension-host/secrets@0.1.0;
  import greentic:extension-host/http@0.1.0;

  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export targets;
  export deployment;
}
```

- [ ] **Step 2: Commit**

```bash
git add wit/extension-deploy.wit
git commit -m "wit: add greentic:extension-deploy@0.1.0 interfaces"
```

### Task 2.6: WIT syntax validation test

**Files:**
- Create: `tests/wit_parse.rs`
- Modify: `Cargo.toml` (add `[[bin]]` or dev workspace)

- [ ] **Step 1: Add `wit-parser` to workspace dev-dependencies**

Edit `Cargo.toml` in `[workspace.dependencies]`:

```toml
wit-parser = "0.221"
```

- [ ] **Step 2: Create a small helper package to host the test**

Create `crates/_wit-lint/Cargo.toml`:

```toml
[package]
name = "_wit-lint"
version.workspace = true
edition.workspace = true
publish = false

[dev-dependencies]
wit-parser = { workspace = true }
```

Create `crates/_wit-lint/src/lib.rs`:

```rust
// Empty — this crate exists only to host WIT linter tests.
```

Create `crates/_wit-lint/tests/parse.rs`:

```rust
use std::path::PathBuf;

use wit_parser::Resolve;

fn wit_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("wit")
}

#[test]
fn all_wit_files_parse() {
    let mut resolve = Resolve::new();
    let (_pkg_id, _) = resolve
        .push_dir(&wit_dir())
        .expect("all wit files should parse without error");
}
```

- [ ] **Step 3: Register crate in workspace members**

Add to `Cargo.toml` workspace `members`:

```toml
"crates/_wit-lint",
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p _wit-lint --test parse`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/_wit-lint/
git commit -m "test: add WIT parser lint test"
```

---

## Phase 3 — `greentic-extension-sdk-contract` crate — types + schemas

### Task 3.1: Scaffold contract crate

**Files:**
- Create: `crates/greentic-extension-sdk-contract/Cargo.toml`
- Create: `crates/greentic-extension-sdk-contract/src/lib.rs`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "greentic-extension-sdk-contract"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Contract types and describe.json schema for Greentic Designer Extensions"

[dependencies]
anyhow = { workspace = true }
base64 = { workspace = true }
ed25519-dalek = { workspace = true }
jsonschema = { workspace = true }
semver = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Create `src/lib.rs` with module skeleton**

```rust
//! Contract types + describe.json schema for Greentic Designer Extensions.

pub mod describe;
pub mod capability;
pub mod kind;
pub mod schema;
pub mod signature;
pub mod error;

pub use self::describe::DescribeJson;
pub use self::capability::{CapabilityId, CapabilityRef, CapabilityVersion};
pub use self::kind::ExtensionKind;
pub use self::error::ContractError;
```

- [ ] **Step 3: Build the crate**

Run: `cargo check -p greentic-extension-sdk-contract`
Expected: fails with "file not found" for the referenced modules. Fine — we build them next.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-extension-sdk-contract/
git commit -m "feat(contract): scaffold crate"
```

### Task 3.2: Implement `kind` module

**Files:**
- Create: `crates/greentic-extension-sdk-contract/src/kind.rs`
- Create: `crates/greentic-extension-sdk-contract/tests/kind.rs`

- [ ] **Step 1: Write the failing test `tests/kind.rs`**

```rust
use greentic_extension_sdk_contract::ExtensionKind;

#[test]
fn serializes_as_pascal_case_string() {
    assert_eq!(
        serde_json::to_string(&ExtensionKind::Design).unwrap(),
        "\"DesignExtension\""
    );
    assert_eq!(
        serde_json::to_string(&ExtensionKind::Bundle).unwrap(),
        "\"BundleExtension\""
    );
    assert_eq!(
        serde_json::to_string(&ExtensionKind::Deploy).unwrap(),
        "\"DeployExtension\""
    );
}

#[test]
fn round_trips_through_json() {
    for variant in [
        ExtensionKind::Design,
        ExtensionKind::Bundle,
        ExtensionKind::Deploy,
    ] {
        let s = serde_json::to_string(&variant).unwrap();
        let back: ExtensionKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn dir_name_matches_spec() {
    assert_eq!(ExtensionKind::Design.dir_name(), "design");
    assert_eq!(ExtensionKind::Bundle.dir_name(), "bundle");
    assert_eq!(ExtensionKind::Deploy.dir_name(), "deploy");
}
```

- [ ] **Step 2: Run test to confirm it fails**

Run: `cargo test -p greentic-extension-sdk-contract --test kind`
Expected: FAIL (missing types)

- [ ] **Step 3: Write minimal implementation `src/kind.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExtensionKind {
    #[serde(rename = "DesignExtension")]
    Design,
    #[serde(rename = "BundleExtension")]
    Bundle,
    #[serde(rename = "DeployExtension")]
    Deploy,
}

impl ExtensionKind {
    #[must_use]
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::Design => "design",
            Self::Bundle => "bundle",
            Self::Deploy => "deploy",
        }
    }
}
```

- [ ] **Step 4: Run test to confirm it passes**

Run: `cargo test -p greentic-extension-sdk-contract --test kind`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-extension-sdk-contract/src/kind.rs crates/greentic-extension-sdk-contract/tests/kind.rs
git commit -m "feat(contract): add ExtensionKind enum"
```

### Task 3.3: Implement `error` module

**Files:**
- Create: `crates/greentic-extension-sdk-contract/src/error.rs`

- [ ] **Step 1: Write `src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("describe.json schema validation failed: {0}")]
    SchemaInvalid(String),

    #[error("capability id is malformed: {0}")]
    MalformedCapabilityId(String),

    #[error("version is not semver: {0}")]
    MalformedVersion(String),

    #[error("signature verification failed: {0}")]
    SignatureInvalid(String),

    #[error("unsupported apiVersion: {0}")]
    UnsupportedApiVersion(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 2: Confirm crate compiles**

Run: `cargo check -p greentic-extension-sdk-contract`
Expected: succeeds with warnings for unused modules

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-extension-sdk-contract/src/error.rs
git commit -m "feat(contract): add ContractError enum"
```

### Task 3.4: Implement `capability` module

**Files:**
- Create: `crates/greentic-extension-sdk-contract/src/capability.rs`
- Create: `crates/greentic-extension-sdk-contract/tests/capability.rs`

- [ ] **Step 1: Write failing test `tests/capability.rs`**

```rust
use greentic_extension_sdk_contract::{CapabilityId, CapabilityRef};

#[test]
fn parses_canonical_cap_id() {
    let id: CapabilityId = "greentic:adaptive-cards/validate".parse().unwrap();
    assert_eq!(id.namespace(), "greentic");
    assert_eq!(id.type_path(), "adaptive-cards/validate");
}

#[test]
fn rejects_missing_colon() {
    let err = "greentic-adaptive-cards".parse::<CapabilityId>().unwrap_err();
    assert!(format!("{err}").contains("malformed"), "got {err}");
}

#[test]
fn capability_ref_version_req_is_semver() {
    let cr: CapabilityRef = serde_json::from_str(
        r#"{"id":"greentic:host/logging","version":"^1.0.0"}"#
    ).unwrap();
    assert!(cr.version_req().matches(&"1.5.0".parse().unwrap()));
    assert!(!cr.version_req().matches(&"2.0.0".parse().unwrap()));
}

#[test]
fn wildcard_all_version_matches_everything() {
    let cr: CapabilityRef = serde_json::from_str(
        r#"{"id":"x:y/z","version":"*"}"#
    ).unwrap();
    assert!(cr.version_req().matches(&"999.999.999".parse().unwrap()));
}
```

- [ ] **Step 2: Write `src/capability.rs`**

```rust
use std::fmt;
use std::str::FromStr;

use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn namespace(&self) -> &str {
        self.0.split_once(':').map_or(&self.0, |(ns, _)| ns)
    }

    pub fn type_path(&self) -> &str {
        self.0.split_once(':').map_or("", |(_, p)| p)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for CapabilityId {
    type Err = ContractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ns, path) = s
            .split_once(':')
            .ok_or_else(|| ContractError::MalformedCapabilityId(s.into()))?;
        if ns.is_empty() || path.is_empty() {
            return Err(ContractError::MalformedCapabilityId(s.into()));
        }
        if !ns
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ContractError::MalformedCapabilityId(s.into()));
        }
        Ok(Self(s.to_owned()))
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub type CapabilityVersion = semver::Version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRef {
    pub id: CapabilityId,
    pub version: String,
}

impl CapabilityRef {
    pub fn version_req(&self) -> VersionReq {
        VersionReq::parse(&self.version).unwrap_or(VersionReq::STAR)
    }
}
```

- [ ] **Step 3: Re-export in `lib.rs`** (already imported; confirm no change needed)

- [ ] **Step 4: Run tests**

Run: `cargo test -p greentic-extension-sdk-contract --test capability`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-extension-sdk-contract/src/capability.rs crates/greentic-extension-sdk-contract/tests/capability.rs
git commit -m "feat(contract): add CapabilityId + CapabilityRef"
```

### Task 3.5: `describe.json` Rust types

**Files:**
- Create: `crates/greentic-extension-sdk-contract/src/describe.rs`
- Create: `crates/greentic-extension-sdk-contract/tests/describe_roundtrip.rs`

- [ ] **Step 1: Write `src/describe.rs`**

```rust
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::capability::CapabilityRef;
use crate::kind::ExtensionKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescribeJson {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: ExtensionKind,
    pub metadata: Metadata,
    pub engine: Engine,
    pub capabilities: Capabilities,
    pub runtime: Runtime,
    pub contributions: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub author: Author,
    pub license: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "publicKey", default, skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    #[serde(rename = "greenticDesigner")]
    pub greentic_designer: String,
    #[serde(rename = "extRuntime")]
    pub ext_runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(default)]
    pub offered: Vec<CapabilityRef>,
    #[serde(default)]
    pub required: Vec<CapabilityRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub component: String,
    #[serde(rename = "memoryLimitMB", default = "default_memory")]
    pub memory_limit_mb: u32,
    pub permissions: Permissions,
}

const fn default_memory() -> u32 {
    64
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(rename = "callExtensionKinds", default)]
    pub call_extension_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    pub algorithm: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    pub value: String,
}

impl DescribeJson {
    pub fn identity_key(&self) -> String {
        format!("{}@{}", self.metadata.id, self.metadata.version)
    }

    pub fn other_keys_sorted<'a>(map: &'a BTreeMap<String, String>) -> Vec<&'a String> {
        map.keys().collect()
    }
}
```

- [ ] **Step 2: Write `tests/describe_roundtrip.rs`**

```rust
use greentic_extension_sdk_contract::DescribeJson;

const AC_FIXTURE: &str = r#"{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.adaptive-cards",
    "name": "Adaptive Cards",
    "version": "1.6.0",
    "summary": "Design AdaptiveCards v1.6",
    "author": { "name": "Greentic" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:adaptive-cards/validate", "version": "1.0.0" }],
    "required": [{ "id": "greentic:host/logging", "version": "^1.0.0" }]
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 64,
    "permissions": {}
  },
  "contributions": {}
}"#;

#[test]
fn ac_fixture_parses() {
    let d: DescribeJson = serde_json::from_str(AC_FIXTURE).unwrap();
    assert_eq!(d.metadata.id, "greentic.adaptive-cards");
    assert_eq!(d.identity_key(), "greentic.adaptive-cards@1.6.0");
    assert_eq!(d.capabilities.offered.len(), 1);
    assert_eq!(d.runtime.memory_limit_mb, 64);
}

#[test]
fn round_trips_without_data_loss() {
    let d: DescribeJson = serde_json::from_str(AC_FIXTURE).unwrap();
    let serialized = serde_json::to_string(&d).unwrap();
    let parsed_back: DescribeJson = serde_json::from_str(&serialized).unwrap();
    assert_eq!(parsed_back.metadata.id, d.metadata.id);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-extension-sdk-contract --test describe_roundtrip`
Expected: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-extension-sdk-contract/src/describe.rs \
        crates/greentic-extension-sdk-contract/tests/describe_roundtrip.rs
git commit -m "feat(contract): add DescribeJson types with serde round-trip"
```

### Task 3.6: `describe.json` JSON Schema file + validator

**Files:**
- Create: `crates/greentic-extension-sdk-contract/schemas/describe-v1.json`
- Create: `crates/greentic-extension-sdk-contract/src/schema.rs`
- Create: `crates/greentic-extension-sdk-contract/tests/schema_validate.rs`

- [ ] **Step 1: Write `schemas/describe-v1.json`**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://store.greentic.ai/schemas/describe-v1.json",
  "title": "Greentic Designer Extension describe.json v1",
  "type": "object",
  "required": ["apiVersion", "kind", "metadata", "engine", "capabilities", "runtime", "contributions"],
  "additionalProperties": false,
  "properties": {
    "$schema": { "type": "string" },
    "apiVersion": { "const": "greentic.ai/v1" },
    "kind": { "enum": ["DesignExtension", "BundleExtension", "DeployExtension"] },
    "metadata": { "$ref": "#/$defs/metadata" },
    "engine": { "$ref": "#/$defs/engine" },
    "capabilities": { "$ref": "#/$defs/capabilities" },
    "runtime": { "$ref": "#/$defs/runtime" },
    "contributions": { "type": "object" },
    "signature": { "$ref": "#/$defs/signature" }
  },
  "allOf": [
    {
      "if": { "properties": { "kind": { "const": "DesignExtension" } } },
      "then": { "properties": { "contributions": { "$ref": "#/$defs/designContributions" } } }
    },
    {
      "if": { "properties": { "kind": { "const": "BundleExtension" } } },
      "then": { "properties": { "contributions": { "$ref": "#/$defs/bundleContributions" } } }
    },
    {
      "if": { "properties": { "kind": { "const": "DeployExtension" } } },
      "then": { "properties": { "contributions": { "$ref": "#/$defs/deployContributions" } } }
    }
  ],
  "$defs": {
    "metadata": {
      "type": "object",
      "required": ["id", "name", "version", "summary", "author", "license"],
      "properties": {
        "id": { "type": "string", "pattern": "^[a-z][a-z0-9.-]*\\.[a-z0-9.-]+$" },
        "name": { "type": "string", "minLength": 1 },
        "version": { "type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+(-[a-zA-Z0-9.-]+)?(\\+[a-zA-Z0-9.-]+)?$" },
        "summary": { "type": "string", "maxLength": 200 },
        "description": { "type": "string" },
        "author": {
          "type": "object",
          "required": ["name"],
          "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" },
            "publicKey": { "type": "string" }
          }
        },
        "license": { "type": "string" },
        "homepage": { "type": "string", "format": "uri" },
        "repository": { "type": "string", "format": "uri" },
        "keywords": { "type": "array", "items": { "type": "string" } },
        "icon": { "type": "string" },
        "screenshots": { "type": "array", "items": { "type": "string" } }
      }
    },
    "engine": {
      "type": "object",
      "required": ["greenticDesigner", "extRuntime"],
      "properties": {
        "greenticDesigner": { "type": "string" },
        "extRuntime": { "type": "string" }
      }
    },
    "capabilities": {
      "type": "object",
      "properties": {
        "offered": { "type": "array", "items": { "$ref": "#/$defs/capRef" } },
        "required": { "type": "array", "items": { "$ref": "#/$defs/capRef" } }
      }
    },
    "capRef": {
      "type": "object",
      "required": ["id", "version"],
      "properties": {
        "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]*:[a-z][a-z0-9/._-]*$" },
        "version": { "type": "string" }
      }
    },
    "runtime": {
      "type": "object",
      "required": ["component", "permissions"],
      "properties": {
        "component": { "type": "string" },
        "memoryLimitMB": { "type": "integer", "minimum": 1, "maximum": 1024 },
        "permissions": {
          "type": "object",
          "properties": {
            "network": { "type": "array", "items": { "type": "string" } },
            "secrets": { "type": "array", "items": { "type": "string" } },
            "callExtensionKinds": {
              "type": "array",
              "items": { "enum": ["design", "bundle", "deploy"] }
            }
          }
        }
      }
    },
    "signature": {
      "type": "object",
      "required": ["algorithm", "publicKey", "value"],
      "properties": {
        "algorithm": { "const": "ed25519" },
        "publicKey": { "type": "string" },
        "value": { "type": "string" }
      }
    },
    "designContributions": {
      "type": "object",
      "properties": {
        "schemas": { "type": "array", "items": { "type": "string" } },
        "prompts": { "type": "array", "items": { "type": "string" } },
        "knowledge": { "type": "array", "items": { "type": "string" } },
        "tools": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["name", "export"],
            "properties": {
              "name": { "type": "string" },
              "export": { "type": "string" }
            }
          }
        },
        "assets": { "type": "array", "items": { "type": "string" } },
        "i18n": { "type": "array", "items": { "type": "string" } }
      }
    },
    "bundleContributions": {
      "type": "object",
      "properties": {
        "recipes": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["id", "displayName", "configSchema", "supportedCapabilities"],
            "properties": {
              "id": { "type": "string" },
              "displayName": { "type": "string" },
              "description": { "type": "string" },
              "configSchema": { "type": "string" },
              "supportedCapabilities": {
                "type": "array",
                "items": { "type": "string" }
              }
            }
          }
        },
        "assets": { "type": "array", "items": { "type": "string" } },
        "i18n": { "type": "array", "items": { "type": "string" } }
      }
    },
    "deployContributions": {
      "type": "object",
      "properties": {
        "targets": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["id", "displayName", "credentialSchema", "configSchema"],
            "properties": {
              "id": { "type": "string" },
              "displayName": { "type": "string" },
              "credentialSchema": { "type": "string" },
              "configSchema": { "type": "string" },
              "iconPath": { "type": "string" }
            }
          }
        },
        "assets": { "type": "array", "items": { "type": "string" } },
        "i18n": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}
```

- [ ] **Step 2: Write `src/schema.rs`**

```rust
use jsonschema::{Draft, ValidationError, Validator};
use once_cell::sync::Lazy;

use crate::error::ContractError;

const SCHEMA_V1: &str = include_str!("../schemas/describe-v1.json");

static SCHEMA: Lazy<Validator> = Lazy::new(|| {
    let schema: serde_json::Value =
        serde_json::from_str(SCHEMA_V1).expect("embedded schema must parse");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .expect("embedded schema must compile")
});

pub fn validate_describe_json(value: &serde_json::Value) -> Result<(), ContractError> {
    let errors: Vec<String> = SCHEMA
        .iter_errors(value)
        .map(|e: ValidationError| format!("{}: {}", e.instance_path, e))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ContractError::SchemaInvalid(errors.join("; ")))
    }
}
```

- [ ] **Step 3: Add `once_cell` to Cargo.toml**

Edit `crates/greentic-extension-sdk-contract/Cargo.toml` dependencies:

```toml
once_cell = "1"
```

- [ ] **Step 4: Write `tests/schema_validate.rs`**

```rust
use greentic_extension_sdk_contract::schema::validate_describe_json;

#[test]
fn accepts_valid_design_ext() {
    let v = serde_json::json!({
      "apiVersion": "greentic.ai/v1",
      "kind": "DesignExtension",
      "metadata": {
        "id": "greentic.adaptive-cards",
        "name": "AC",
        "version": "1.6.0",
        "summary": "x",
        "author": { "name": "G" },
        "license": "MIT"
      },
      "engine": { "greenticDesigner": ">=0.1", "extRuntime": "^0.1" },
      "capabilities": {
        "offered": [{ "id": "greentic:ac/validate", "version": "1.0.0" }],
        "required": []
      },
      "runtime": {
        "component": "ext.wasm",
        "permissions": {}
      },
      "contributions": { "schemas": [] }
    });
    validate_describe_json(&v).unwrap();
}

#[test]
fn rejects_missing_kind() {
    let v = serde_json::json!({
      "apiVersion": "greentic.ai/v1",
      "metadata": {
        "id": "x.y", "name": "x", "version": "1.0.0",
        "summary": "x", "author": { "name": "x" }, "license": "MIT"
      },
      "engine": { "greenticDesigner": "*", "extRuntime": "*" },
      "capabilities": {},
      "runtime": { "component": "e.wasm", "permissions": {} },
      "contributions": {}
    });
    assert!(validate_describe_json(&v).is_err());
}

#[test]
fn rejects_bad_capability_id() {
    let mut v: serde_json::Value = serde_json::from_str(BASE_OK).unwrap();
    v["capabilities"]["offered"][0]["id"] = serde_json::json!("NO_COLON_HERE");
    assert!(validate_describe_json(&v).is_err());
}

const BASE_OK: &str = r#"{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.x", "name": "X", "version": "1.0.0",
    "summary": "x", "author": { "name": "G" }, "license": "MIT"
  },
  "engine": { "greenticDesigner": "*", "extRuntime": "*" },
  "capabilities": { "offered": [{ "id": "greentic:x/y", "version": "1.0.0" }] },
  "runtime": { "component": "e.wasm", "permissions": {} },
  "contributions": {}
}"#;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p greentic-extension-sdk-contract --test schema_validate`
Expected: PASS (3 tests)

- [ ] **Step 6: Commit**

```bash
git add crates/greentic-extension-sdk-contract/schemas/ \
        crates/greentic-extension-sdk-contract/src/schema.rs \
        crates/greentic-extension-sdk-contract/tests/schema_validate.rs \
        crates/greentic-extension-sdk-contract/Cargo.toml
git commit -m "feat(contract): add describe.json JSON Schema validator"
```

### Task 3.7: Signature types + SHA256 canonicalization

**Files:**
- Create: `crates/greentic-extension-sdk-contract/src/signature.rs`
- Create: `crates/greentic-extension-sdk-contract/tests/signature_rt.rs`

- [ ] **Step 1: Write `src/signature.rs`**

```rust
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signature, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::error::ContractError;

/// Compute canonical SHA256 of artifact bytes (as-is, no normalization).
#[must_use]
pub fn artifact_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sign_ed25519(key: &SigningKey, payload: &[u8]) -> String {
    let sig: Signature = key.sign(payload);
    B64.encode(sig.to_bytes())
}

pub fn verify_ed25519(
    public_key_b64: &str,
    signature_b64: &str,
    payload: &[u8],
) -> Result<(), ContractError> {
    let public_key_bytes = B64
        .decode(strip_prefix(public_key_b64))
        .map_err(|e| ContractError::SignatureInvalid(format!("pubkey b64: {e}")))?;
    let sig_bytes = B64
        .decode(signature_b64)
        .map_err(|e| ContractError::SignatureInvalid(format!("sig b64: {e}")))?;
    let public_key_array: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::SignatureInvalid("pubkey length != 32".into()))?;
    let sig_array: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::SignatureInvalid("sig length != 64".into()))?;
    let key = VerifyingKey::from_bytes(&public_key_array)
        .map_err(|e| ContractError::SignatureInvalid(format!("pubkey parse: {e}")))?;
    let signature = Signature::from_bytes(&sig_array);
    key.verify(payload, &signature)
        .map_err(|e| ContractError::SignatureInvalid(format!("verify: {e}")))
}

fn strip_prefix(s: &str) -> &str {
    s.strip_prefix("ed25519:").unwrap_or(s)
}
```

- [ ] **Step 2: Uncomment `pub mod signature;` in lib.rs** (already declared; add exports)

Edit `src/lib.rs`:

```rust
pub use self::signature::{artifact_sha256, sign_ed25519, verify_ed25519};
```

- [ ] **Step 3: Write `tests/signature_rt.rs`**

```rust
use ed25519_dalek::SigningKey;
use greentic_extension_sdk_contract::{artifact_sha256, sign_ed25519, verify_ed25519};
use rand::rngs::OsRng;

#[test]
fn sha256_is_deterministic() {
    assert_eq!(artifact_sha256(b"hello"), artifact_sha256(b"hello"));
    assert_ne!(artifact_sha256(b"hello"), artifact_sha256(b"world"));
}

#[test]
fn round_trip_sign_verify() {
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    let pk_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        pk.to_bytes(),
    );
    let payload = b"arbitrary payload";
    let sig = sign_ed25519(&sk, payload);
    verify_ed25519(&pk_b64, &sig, payload).expect("signature must verify");
}

#[test]
fn tampered_payload_fails_verification() {
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    let pk_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        pk.to_bytes(),
    );
    let sig = sign_ed25519(&sk, b"original");
    let err = verify_ed25519(&pk_b64, &sig, b"tampered").unwrap_err();
    assert!(format!("{err}").contains("verify"));
}
```

Also add to dev-deps of contract crate:

```toml
[dev-dependencies]
rand = { workspace = true }
base64 = { workspace = true }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p greentic-extension-sdk-contract --test signature_rt`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-extension-sdk-contract/src/signature.rs \
        crates/greentic-extension-sdk-contract/src/lib.rs \
        crates/greentic-extension-sdk-contract/tests/signature_rt.rs \
        crates/greentic-extension-sdk-contract/Cargo.toml
git commit -m "feat(contract): add ed25519 signing + sha256 helpers"
```

---

## Phase 4 — `greentic-extension-sdk-testing` crate

### Task 4.1: Scaffold testing crate

**Files:**
- Create: `crates/greentic-extension-sdk-testing/Cargo.toml`
- Create: `crates/greentic-extension-sdk-testing/src/lib.rs`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "greentic-extension-sdk-testing"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Test utilities for Greentic Designer Extensions"

[dependencies]
anyhow = { workspace = true }
greentic-extension-sdk-contract = { path = "../greentic-extension-sdk-contract" }
serde = { workspace = true }
serde_json = { workspace = true }
tempfile = { workspace = true }
zip = { version = "2", default-features = false, features = ["deflate"] }

[lints]
workspace = true
```

- [ ] **Step 2: Create `src/lib.rs`**

```rust
//! Test utilities for Greentic Designer Extensions.
//!
//! Builders for synthetic extensions, in-memory registries, and filesystem
//! fixtures used across the runtime and CLI test suites.

mod fixture;
mod gtxpack;

pub use self::fixture::{ExtensionFixture, ExtensionFixtureBuilder};
pub use self::gtxpack::{pack_directory, unpack_to_dir};
```

- [ ] **Step 3: Create `src/fixture.rs`** (skeleton; fully implemented in 4.2)

```rust
use std::path::{Path, PathBuf};

use anyhow::Result;
use greentic_extension_sdk_contract::{DescribeJson, ExtensionKind};
use tempfile::TempDir;

pub struct ExtensionFixture {
    pub dir: TempDir,
    pub describe_path: PathBuf,
}

impl ExtensionFixture {
    pub fn root(&self) -> &Path {
        self.dir.path()
    }
}

pub struct ExtensionFixtureBuilder {
    kind: ExtensionKind,
    id: String,
    version: String,
    offered: Vec<(String, String)>,
    required: Vec<(String, String)>,
    wasm_bytes: Vec<u8>,
}

impl ExtensionFixtureBuilder {
    pub fn new(kind: ExtensionKind, id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
            version: version.into(),
            offered: Vec::new(),
            required: Vec::new(),
            wasm_bytes: Vec::new(),
        }
    }

    pub fn offer(mut self, id: impl Into<String>, v: impl Into<String>) -> Self {
        self.offered.push((id.into(), v.into()));
        self
    }

    pub fn require(mut self, id: impl Into<String>, v: impl Into<String>) -> Self {
        self.required.push((id.into(), v.into()));
        self
    }

    pub fn with_wasm(mut self, bytes: Vec<u8>) -> Self {
        self.wasm_bytes = bytes;
        self
    }

    pub fn build(self) -> Result<ExtensionFixture> {
        let dir = TempDir::new()?;
        let describe = DescribeJson {
            schema_ref: None,
            api_version: "greentic.ai/v1".into(),
            kind: self.kind,
            metadata: greentic_extension_sdk_contract::describe::Metadata {
                id: self.id.clone(),
                name: self.id.clone(),
                version: self.version.clone(),
                summary: "test".into(),
                description: None,
                author: greentic_extension_sdk_contract::describe::Author {
                    name: "test".into(),
                    email: None,
                    public_key: None,
                },
                license: "MIT".into(),
                homepage: None,
                repository: None,
                keywords: vec![],
                icon: None,
                screenshots: vec![],
            },
            engine: greentic_extension_sdk_contract::describe::Engine {
                greentic_designer: "*".into(),
                ext_runtime: "*".into(),
            },
            capabilities: greentic_extension_sdk_contract::describe::Capabilities {
                offered: self
                    .offered
                    .into_iter()
                    .map(|(id, v)| greentic_extension_sdk_contract::CapabilityRef {
                        id: id.parse().unwrap(),
                        version: v,
                    })
                    .collect(),
                required: self
                    .required
                    .into_iter()
                    .map(|(id, v)| greentic_extension_sdk_contract::CapabilityRef {
                        id: id.parse().unwrap(),
                        version: v,
                    })
                    .collect(),
            },
            runtime: greentic_extension_sdk_contract::describe::Runtime {
                component: "extension.wasm".into(),
                memory_limit_mb: 64,
                permissions: Default::default(),
            },
            contributions: serde_json::json!({}),
            signature: None,
        };
        let describe_path = dir.path().join("describe.json");
        std::fs::write(&describe_path, serde_json::to_vec_pretty(&describe)?)?;
        std::fs::write(dir.path().join("extension.wasm"), &self.wasm_bytes)?;
        Ok(ExtensionFixture {
            dir,
            describe_path,
        })
    }
}
```

- [ ] **Step 4: Create `src/gtxpack.rs`**

```rust
use std::io::{Read, Write};
use std::path::Path;

use anyhow::Result;
use zip::write::SimpleFileOptions;

pub fn pack_directory(src: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in walkdir(src) {
        let entry = entry?;
        let rel = entry.strip_prefix(src)?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if entry.is_dir() {
            continue;
        }
        zip.start_file(rel_str, opts)?;
        let mut f = std::fs::File::open(&entry)?;
        std::io::copy(&mut f, &mut zip)?;
    }
    zip.finish()?;
    Ok(())
}

pub fn unpack_to_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let file = std::fs::File::open(src)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = dest.join(entry.mangled_name());
        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)?;
            continue;
        }
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&outpath)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn walkdir(root: &Path) -> impl Iterator<Item = std::io::Result<std::path::PathBuf>> {
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    std::iter::from_fn(move || {
        while let Some(p) = stack.pop() {
            match std::fs::read_dir(&p) {
                Ok(rd) => {
                    for e in rd {
                        match e {
                            Ok(ent) => stack.push(ent.path()),
                            Err(err) => return Some(Err(err)),
                        }
                    }
                    if p != root {
                        return Some(Ok(p));
                    }
                }
                Err(_) if p.is_file() => return Some(Ok(p)),
                Err(err) => return Some(Err(err)),
            }
        }
        None
    })
}
```

- [ ] **Step 5: Compile and commit**

Run: `cargo check -p greentic-extension-sdk-testing`
Expected: clean

```bash
git add crates/greentic-extension-sdk-testing/
git commit -m "feat(testing): add ExtensionFixture builder + gtxpack helpers"
```

---

## Phase 5 — `greentic-ext-runtime` crate — core loader

### Task 5.1: Scaffold runtime crate

**Files:**
- Create: `crates/greentic-ext-runtime/Cargo.toml`
- Create: `crates/greentic-ext-runtime/src/lib.rs`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "greentic-ext-runtime"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Wasmtime-based runtime for Greentic Designer Extensions"

[dependencies]
anyhow = { workspace = true }
arc-swap = { workspace = true }
greentic-extension-sdk-contract = { path = "../greentic-extension-sdk-contract" }
notify = { workspace = true }
notify-debouncer-full = { workspace = true }
semver = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
wasmtime = { workspace = true }
wasmtime-wasi = { workspace = true }

[dev-dependencies]
greentic-extension-sdk-testing = { path = "../greentic-extension-sdk-testing" }
tempfile = { workspace = true }
tokio = { workspace = true }
tracing-subscriber = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Create `src/lib.rs` module skeleton**

```rust
//! Wasmtime-based runtime for Greentic Designer Extensions.

mod broker;
mod capability;
mod discovery;
mod error;
mod health;
mod loaded;
mod pool;
mod runtime;
mod watcher;

pub use self::broker::{Broker, BrokerError, BrokerResult};
pub use self::capability::{CapabilityRegistry, OfferedBinding, ResolutionPlan};
pub use self::discovery::DiscoveryPaths;
pub use self::error::RuntimeError;
pub use self::health::{ExtensionHealth, HealthReason};
pub use self::loaded::{LoadedExtension, LoadedExtensionRef};
pub use self::runtime::{ExtensionRuntime, RuntimeConfig, RuntimeEvent};
```

- [ ] **Step 3: Compile (will fail — modules empty)**

Run: `cargo check -p greentic-ext-runtime`
Expected: fails with missing module files. Proceed.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/
git commit -m "feat(runtime): scaffold crate with module skeleton"
```

### Task 5.2: Define `RuntimeError` + `ExtensionHealth`

**Files:**
- Create: `crates/greentic-ext-runtime/src/error.rs`
- Create: `crates/greentic-ext-runtime/src/health.rs`

- [ ] **Step 1: Write `src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("extension '{0}' already loaded")]
    AlreadyLoaded(String),

    #[error("extension '{0}' not found")]
    NotFound(String),

    #[error("contract error: {0}")]
    Contract(#[from] greentic_extension_sdk_contract::ContractError),

    #[error("wasmtime: {0}")]
    Wasmtime(#[from] anyhow::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("watcher: {0}")]
    Watcher(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),
}
```

- [ ] **Step 2: Write `src/health.rs`**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionHealth {
    Healthy,
    Degraded(HealthReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthReason {
    MissingRequiredCap(String),
    SignatureInvalid,
    LoadFailed(String),
    CycleDetected,
}

impl ExtensionHealth {
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}
```

- [ ] **Step 3: Compile**

Run: `cargo check -p greentic-ext-runtime`
Expected: still fails on other modules, but `error.rs` + `health.rs` should not be the cause.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/error.rs \
        crates/greentic-ext-runtime/src/health.rs
git commit -m "feat(runtime): add RuntimeError + ExtensionHealth"
```

### Task 5.3: `LoadedExtension` struct + loader

**Files:**
- Create: `crates/greentic-ext-runtime/src/loaded.rs`
- Create: `crates/greentic-ext-runtime/src/pool.rs`

- [ ] **Step 1: Write `src/loaded.rs`**

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;

use greentic_extension_sdk_contract::{DescribeJson, ExtensionKind};
use wasmtime::component::Component;

use crate::health::ExtensionHealth;
use crate::pool::InstancePool;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    #[must_use]
    pub fn from_describe(describe: &DescribeJson) -> Self {
        Self(describe.metadata.id.clone())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct LoadedExtension {
    pub id: ExtensionId,
    pub describe: Arc<DescribeJson>,
    pub kind: ExtensionKind,
    pub source_dir: PathBuf,
    pub component: Component,
    pub pool: InstancePool,
    pub health: ExtensionHealth,
}

impl LoadedExtension {
    pub fn load_from_dir(
        engine: &wasmtime::Engine,
        source_dir: &Path,
    ) -> anyhow::Result<Self> {
        let describe_path = source_dir.join("describe.json");
        let describe_bytes = std::fs::read(&describe_path)?;
        let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)?;
        greentic_extension_sdk_contract::schema::validate_describe_json(&describe_value)
            .map_err(|e| anyhow::anyhow!("invalid describe.json: {e}"))?;
        let describe: DescribeJson = serde_json::from_value(describe_value)?;
        let id = ExtensionId::from_describe(&describe);
        let wasm_path = source_dir.join(&describe.runtime.component);
        let component = Component::from_file(engine, &wasm_path)?;
        let pool = InstancePool::new(2);
        Ok(Self {
            id,
            describe: Arc::new(describe.clone()),
            kind: describe.kind,
            source_dir: source_dir.to_path_buf(),
            component,
            pool,
            health: ExtensionHealth::Healthy,
        })
    }
}

pub type LoadedExtensionRef = Arc<LoadedExtension>;
```

- [ ] **Step 2: Write `src/pool.rs`** (stub — real pool in Task 5.5)

```rust
#[derive(Debug)]
pub struct InstancePool {
    capacity: usize,
}

impl InstancePool {
    #[must_use]
    pub const fn new(capacity: usize) -> Self {
        Self { capacity }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}
```

- [ ] **Step 3: Compile + commit**

Run: `cargo check -p greentic-ext-runtime`
Expected: compiles with warnings on unused items.

```bash
git add crates/greentic-ext-runtime/src/loaded.rs \
        crates/greentic-ext-runtime/src/pool.rs
git commit -m "feat(runtime): add LoadedExtension + instance pool stub"
```

### Task 5.4: `ExtensionRuntime` skeleton

**Files:**
- Create: `crates/greentic-ext-runtime/src/runtime.rs`

- [ ] **Step 1: Write `src/runtime.rs`**

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use wasmtime::Engine;

use crate::capability::CapabilityRegistry;
use crate::discovery::DiscoveryPaths;
use crate::error::RuntimeError;
use crate::loaded::{ExtensionId, LoadedExtensionRef};

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub paths: DiscoveryPaths,
}

impl RuntimeConfig {
    #[must_use]
    pub fn from_paths(paths: DiscoveryPaths) -> Self {
        Self { paths }
    }
}

pub struct ExtensionRuntime {
    engine: Engine,
    config: RuntimeConfig,
    loaded: ArcSwap<HashMap<ExtensionId, LoadedExtensionRef>>,
    capability_registry: ArcSwap<CapabilityRegistry>,
    events: broadcast::Sender<RuntimeEvent>,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    ExtensionInstalled(ExtensionId),
    ExtensionUpdated { id: ExtensionId, prev_version: String },
    ExtensionRemoved(ExtensionId),
    CapabilityRegistryRebuilt,
}

impl ExtensionRuntime {
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let mut ec = wasmtime::Config::new();
        ec.async_support(true);
        ec.wasm_component_model(true);
        let engine = Engine::new(&ec).map_err(RuntimeError::Wasmtime)?;
        let (tx, _) = broadcast::channel(64);
        Ok(Self {
            engine,
            config,
            loaded: ArcSwap::from_pointee(HashMap::new()),
            capability_registry: ArcSwap::from_pointee(CapabilityRegistry::new()),
            events: tx,
        })
    }

    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }

    #[must_use]
    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    #[must_use]
    pub fn loaded(&self) -> Arc<HashMap<ExtensionId, LoadedExtensionRef>> {
        self.loaded.load_full()
    }

    #[must_use]
    pub fn capability_registry(&self) -> Arc<CapabilityRegistry> {
        self.capability_registry.load_full()
    }
}
```

- [ ] **Step 2: Write `src/discovery.rs` (skeleton)**

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    pub user: PathBuf,
    pub project: Option<PathBuf>,
}

impl DiscoveryPaths {
    #[must_use]
    pub fn new(user: PathBuf) -> Self {
        Self {
            user,
            project: None,
        }
    }

    pub fn with_project(mut self, project: PathBuf) -> Self {
        self.project = Some(project);
        self
    }

    pub fn all(&self) -> impl Iterator<Item = &PathBuf> {
        self.user.iter().chain(self.project.iter()).flat_map(|_| {
            std::iter::once(&self.user).chain(self.project.as_ref())
        }).take(2)
    }
}
```

Wait — fix the iterator:

```rust
impl DiscoveryPaths {
    pub fn all(&self) -> Vec<&PathBuf> {
        let mut v = vec![&self.user];
        if let Some(p) = &self.project {
            v.push(p);
        }
        v
    }
}
```

- [ ] **Step 3: Commit**

Run: `cargo check -p greentic-ext-runtime`
Expected: clean (broker + watcher modules still missing — placeholder empty module files)

Create empty placeholders:

```bash
: > crates/greentic-ext-runtime/src/broker.rs
: > crates/greentic-ext-runtime/src/watcher.rs
```

Write empty `src/broker.rs`:

```rust
//! Host broker — implemented in Task 6.4.

pub type BrokerResult<T> = Result<T, BrokerError>;

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("stub")]
    Stub,
}

pub struct Broker;
```

Write empty `src/watcher.rs`:

```rust
//! Filesystem watcher — implemented in Task 7.2.
```

Run `cargo check -p greentic-ext-runtime` again.
Expected: clean.

```bash
git add crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/src/discovery.rs \
        crates/greentic-ext-runtime/src/broker.rs \
        crates/greentic-ext-runtime/src/watcher.rs
git commit -m "feat(runtime): add ExtensionRuntime skeleton + DiscoveryPaths"
```

### Task 5.5: Instance pool — real implementation

**Files:**
- Modify: `crates/greentic-ext-runtime/src/pool.rs`

- [ ] **Step 1: Replace `src/pool.rs` with pooled instance semantics**

```rust
use std::collections::VecDeque;
use std::sync::Mutex;

use wasmtime::component::{Instance, Linker};
use wasmtime::{Engine, Store};

pub struct InstancePool {
    capacity: usize,
    free: Mutex<VecDeque<PooledInstance>>,
}

pub struct PooledInstance {
    pub instance: Instance,
    pub store: Store<()>,
}

impl InstancePool {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            free: Mutex::new(VecDeque::new()),
        }
    }

    pub fn acquire<F>(&self, make: F) -> anyhow::Result<PooledInstance>
    where
        F: FnOnce() -> anyhow::Result<PooledInstance>,
    {
        if let Some(inst) = self.free.lock().unwrap().pop_front() {
            return Ok(inst);
        }
        make()
    }

    pub fn release(&self, inst: PooledInstance) {
        let mut q = self.free.lock().unwrap();
        if q.len() < self.capacity {
            q.push_back(inst);
        }
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}
```

- [ ] **Step 2: Compile**

Run: `cargo check -p greentic-ext-runtime`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-runtime/src/pool.rs
git commit -m "feat(runtime): implement InstancePool with free list"
```

---

## Phase 6 — Capability Registry + matching

### Task 6.1: CapabilityRegistry + matching rules

**Files:**
- Create: `crates/greentic-ext-runtime/src/capability.rs`
- Create: `crates/greentic-ext-runtime/tests/capability_registry.rs`

- [ ] **Step 1: Write failing test `tests/capability_registry.rs`**

```rust
use greentic_extension_sdk_contract::{CapabilityRef, ExtensionKind};
use greentic_ext_runtime::{CapabilityRegistry, OfferedBinding};

fn cap_ref(id: &str, v: &str) -> CapabilityRef {
    CapabilityRef {
        id: id.parse().unwrap(),
        version: v.to_string(),
    }
}

#[test]
fn matches_caret_version() {
    let mut r = CapabilityRegistry::new();
    r.add_offering(OfferedBinding {
        extension_id: "x.offerer".into(),
        cap_id: "greentic:x/y".parse().unwrap(),
        version: "1.2.5".parse().unwrap(),
        kind: ExtensionKind::Design,
        export_path: "ext/y.func".into(),
    });
    let plan = r.resolve("x.consumer", &[cap_ref("greentic:x/y", "^1.0")]);
    assert!(plan.unresolved.is_empty());
    assert_eq!(plan.resolved.len(), 1);
}

#[test]
fn degrades_on_missing_cap() {
    let mut r = CapabilityRegistry::new();
    let plan = r.resolve("x.consumer", &[cap_ref("greentic:nope/here", "^1.0")]);
    assert_eq!(plan.unresolved.len(), 1);
    assert!(plan.resolved.is_empty());
}

#[test]
fn picks_highest_compatible_semver() {
    let mut r = CapabilityRegistry::new();
    for v in ["1.0.0", "1.2.0", "1.5.0", "2.0.0"] {
        r.add_offering(OfferedBinding {
            extension_id: format!("x.offer-{v}"),
            cap_id: "greentic:x/y".parse().unwrap(),
            version: v.parse().unwrap(),
            kind: ExtensionKind::Design,
            export_path: "e".into(),
        });
    }
    let plan = r.resolve("x.consumer", &[cap_ref("greentic:x/y", "^1.0")]);
    let picked = plan.resolved.values().next().unwrap();
    assert_eq!(picked.version.to_string(), "1.5.0");
}
```

- [ ] **Step 2: Run test to confirm fail**

Run: `cargo test -p greentic-ext-runtime --test capability_registry`
Expected: FAIL (missing types)

- [ ] **Step 3: Write `src/capability.rs`**

```rust
use std::collections::HashMap;

use greentic_extension_sdk_contract::{CapabilityId, CapabilityRef, ExtensionKind};
use semver::{Version, VersionReq};

#[derive(Debug, Clone)]
pub struct OfferedBinding {
    pub extension_id: String,
    pub cap_id: CapabilityId,
    pub version: Version,
    pub kind: ExtensionKind,
    pub export_path: String,
}

#[derive(Debug, Clone)]
pub struct ResolutionPlan {
    pub consumer: String,
    pub resolved: HashMap<CapabilityId, OfferedBinding>,
    pub unresolved: Vec<CapabilityRef>,
}

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    offerings: HashMap<CapabilityId, Vec<OfferedBinding>>,
}

impl CapabilityRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_offering(&mut self, offering: OfferedBinding) {
        self.offerings
            .entry(offering.cap_id.clone())
            .or_default()
            .push(offering);
    }

    pub fn resolve(&self, consumer: &str, required: &[CapabilityRef]) -> ResolutionPlan {
        let mut resolved = HashMap::new();
        let mut unresolved = Vec::new();
        for req in required {
            let vr = VersionReq::parse(&req.version).unwrap_or(VersionReq::STAR);
            let best = self
                .offerings
                .get(&req.id)
                .and_then(|offers| {
                    offers
                        .iter()
                        .filter(|o| vr.matches(&o.version))
                        .max_by(|a, b| a.version.cmp(&b.version))
                })
                .cloned();
            match best {
                Some(o) => {
                    resolved.insert(req.id.clone(), o);
                }
                None => unresolved.push(req.clone()),
            }
        }
        ResolutionPlan {
            consumer: consumer.to_string(),
            resolved,
            unresolved,
        }
    }

    pub fn offerings(&self) -> impl Iterator<Item = &OfferedBinding> {
        self.offerings.values().flat_map(|v| v.iter())
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p greentic-ext-runtime --test capability_registry`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-runtime/src/capability.rs \
        crates/greentic-ext-runtime/tests/capability_registry.rs
git commit -m "feat(runtime): add CapabilityRegistry with semver matching"
```

### Task 6.2: Cycle detection

**Files:**
- Modify: `crates/greentic-ext-runtime/src/capability.rs`
- Create: `crates/greentic-ext-runtime/tests/cycle_detection.rs`

- [ ] **Step 1: Write failing test `tests/cycle_detection.rs`**

```rust
use greentic_extension_sdk_contract::{CapabilityRef, ExtensionKind};
use greentic_ext_runtime::{CapabilityRegistry, OfferedBinding};

fn offer(ext: &str, cap: &str, v: &str) -> OfferedBinding {
    OfferedBinding {
        extension_id: ext.into(),
        cap_id: cap.parse().unwrap(),
        version: v.parse().unwrap(),
        kind: ExtensionKind::Design,
        export_path: "e".into(),
    }
}

fn require(id: &str, v: &str) -> CapabilityRef {
    CapabilityRef {
        id: id.parse().unwrap(),
        version: v.to_string(),
    }
}

#[test]
fn detects_direct_cycle() {
    // A requires cap_b, B requires cap_a; each offers the cap the other needs.
    let mut r = CapabilityRegistry::new();
    r.add_offering(offer("a", "greentic:a/offered", "1.0.0"));
    r.add_offering(offer("b", "greentic:b/offered", "1.0.0"));
    let cycle = r.detect_cycle(&[
        ("a".to_string(), vec![require("greentic:b/offered", "^1.0")]),
        ("b".to_string(), vec![require("greentic:a/offered", "^1.0")]),
    ]);
    assert!(cycle.contains(&"a".to_string()));
    assert!(cycle.contains(&"b".to_string()));
}

#[test]
fn no_cycle_for_linear_dependency() {
    let mut r = CapabilityRegistry::new();
    r.add_offering(offer("a", "greentic:a/offered", "1.0.0"));
    r.add_offering(offer("b", "greentic:b/offered", "1.0.0"));
    let cycle = r.detect_cycle(&[
        ("a".to_string(), vec![require("greentic:b/offered", "^1.0")]),
        ("b".to_string(), vec![]),
    ]);
    assert!(cycle.is_empty());
}
```

- [ ] **Step 2: Extend `src/capability.rs` with `detect_cycle`**

Append to `CapabilityRegistry`:

```rust
impl CapabilityRegistry {
    /// Returns extension IDs that participate in a dependency cycle.
    /// Empty vec if acyclic.
    pub fn detect_cycle(
        &self,
        extensions: &[(String, Vec<CapabilityRef>)],
    ) -> Vec<String> {
        let ext_map: HashMap<&str, &Vec<CapabilityRef>> = extensions
            .iter()
            .map(|(id, reqs)| (id.as_str(), reqs))
            .collect();

        let mut in_cycle = Vec::new();
        for (id, _) in extensions {
            let mut visited = std::collections::HashSet::new();
            if self.dfs_has_cycle(id, &ext_map, &mut visited) {
                in_cycle.push(id.clone());
            }
        }
        in_cycle
    }

    fn dfs_has_cycle(
        &self,
        ext_id: &str,
        ext_map: &HashMap<&str, &Vec<CapabilityRef>>,
        visited: &mut std::collections::HashSet<String>,
    ) -> bool {
        if !visited.insert(ext_id.to_string()) {
            return true;
        }
        let Some(reqs) = ext_map.get(ext_id) else {
            return false;
        };
        for req in reqs.iter() {
            let vr = VersionReq::parse(&req.version).unwrap_or(VersionReq::STAR);
            let Some(offers) = self.offerings.get(&req.id) else {
                continue;
            };
            for o in offers.iter().filter(|o| vr.matches(&o.version)) {
                if self.dfs_has_cycle(&o.extension_id, ext_map, visited) {
                    return true;
                }
            }
        }
        visited.remove(ext_id);
        false
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-runtime --test cycle_detection`
Expected: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/capability.rs \
        crates/greentic-ext-runtime/tests/cycle_detection.rs
git commit -m "feat(runtime): add dependency cycle detection"
```

### Task 6.3: Wire CapabilityRegistry rebuild into ExtensionRuntime

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Create: `crates/greentic-ext-runtime/tests/runtime_load.rs`

- [ ] **Step 1: Write failing test `tests/runtime_load.rs`**

```rust
use std::path::PathBuf;

use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_testing::ExtensionFixtureBuilder;

#[tokio::test]
async fn loads_extension_and_registers_caps() {
    let minimal_wasm = build_minimal_wasm();
    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.test-ext",
        "0.1.0",
    )
    .offer("greentic:test/ping", "1.0.0")
    .with_wasm(minimal_wasm)
    .build()
    .unwrap();

    let config = RuntimeConfig::from_paths(
        greentic_ext_runtime::DiscoveryPaths::new(PathBuf::from("/dev/null"))
    );
    let mut rt = ExtensionRuntime::new(config).unwrap();
    rt.register_loaded_from_dir(fixture.root()).await.unwrap();

    let registry = rt.capability_registry();
    assert!(registry.offerings().any(|o| o.extension_id == "greentic.test-ext"));
}

fn build_minimal_wasm() -> Vec<u8> {
    // A valid but empty WebAssembly component.
    // Hex: magic + version + layer + core module placeholder
    wat::parse_str(r#"(component)"#).expect("component must compile")
}
```

Add dev-dep `wat = "1"` to runtime crate.

- [ ] **Step 2: Add `register_loaded_from_dir` method to `ExtensionRuntime`**

In `src/runtime.rs`:

```rust
use std::path::Path;

use crate::capability::OfferedBinding;
use crate::loaded::{ExtensionId, LoadedExtension};

impl ExtensionRuntime {
    pub async fn register_loaded_from_dir(&mut self, dir: &Path) -> Result<(), RuntimeError> {
        let loaded = LoadedExtension::load_from_dir(&self.engine, dir)?;
        let id = loaded.id.clone();
        let mut new_registry = CapabilityRegistry::new();
        for existing in self.capability_registry.load().offerings() {
            new_registry.add_offering(existing.clone());
        }
        for cap in &loaded.describe.capabilities.offered {
            new_registry.add_offering(OfferedBinding {
                extension_id: id.as_str().to_string(),
                cap_id: cap.id.clone(),
                version: cap.version.parse().map_err(|e: semver::Error| {
                    RuntimeError::Wasmtime(anyhow::anyhow!("bad offered version: {e}"))
                })?,
                kind: loaded.kind,
                export_path: String::new(),
            });
        }
        let mut new_map = (**self.loaded.load()).clone();
        new_map.insert(id.clone(), std::sync::Arc::new(loaded));
        self.loaded.store(std::sync::Arc::new(new_map));
        self.capability_registry.store(std::sync::Arc::new(new_registry));
        let _ = self.events.send(RuntimeEvent::ExtensionInstalled(id));
        Ok(())
    }
}
```

- [ ] **Step 3: Run test**

Run: `cargo test -p greentic-ext-runtime --test runtime_load`
Expected: PASS (1 test)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/tests/runtime_load.rs \
        crates/greentic-ext-runtime/Cargo.toml
git commit -m "feat(runtime): wire register_loaded_from_dir + cap registry rebuild"
```

### Task 6.4: Host broker with permission gate

**Files:**
- Modify: `crates/greentic-ext-runtime/src/broker.rs`
- Create: `crates/greentic-ext-runtime/tests/broker.rs`

- [ ] **Step 1: Write failing test `tests/broker.rs`**

```rust
use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_runtime::{Broker, BrokerError};

#[test]
fn denies_call_without_permission() {
    let broker = Broker::new();
    let err = broker
        .check_permission("caller", &["design".to_string()], ExtensionKind::Bundle)
        .unwrap_err();
    assert!(matches!(err, BrokerError::PermissionDenied(_)));
}

#[test]
fn allows_call_when_kind_in_allowlist() {
    let broker = Broker::new();
    broker
        .check_permission(
            "caller",
            &["design".to_string(), "bundle".to_string()],
            ExtensionKind::Bundle,
        )
        .unwrap();
}

#[test]
fn enforces_max_depth() {
    let broker = Broker::new();
    let err = broker.check_depth(9).unwrap_err();
    assert!(matches!(err, BrokerError::MaxDepthExceeded));
}
```

- [ ] **Step 2: Replace `src/broker.rs`**

```rust
use greentic_extension_sdk_contract::ExtensionKind;

pub type BrokerResult<T> = Result<T, BrokerError>;

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("target extension not loaded: {0}")]
    TargetNotLoaded(String),

    #[error("function not found: {0}")]
    FunctionNotFound(String),

    #[error("max call depth exceeded")]
    MaxDepthExceeded,

    #[error("deadline exceeded")]
    Deadline,
}

pub const MAX_DEPTH: u32 = 8;

#[derive(Debug, Default)]
pub struct Broker;

impl Broker {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn check_permission(
        &self,
        caller_id: &str,
        allowlist: &[String],
        target_kind: ExtensionKind,
    ) -> BrokerResult<()> {
        if allowlist.iter().any(|k| k == target_kind.dir_name()) {
            Ok(())
        } else {
            Err(BrokerError::PermissionDenied(format!(
                "{caller_id} may not call {:?} extensions",
                target_kind
            )))
        }
    }

    pub fn check_depth(&self, depth: u32) -> BrokerResult<()> {
        if depth >= MAX_DEPTH {
            Err(BrokerError::MaxDepthExceeded)
        } else {
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-runtime --test broker`
Expected: PASS (3 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/broker.rs \
        crates/greentic-ext-runtime/tests/broker.rs
git commit -m "feat(runtime): add Broker with permission + depth checks"
```

---

## Phase 7 — Discovery + hot reload

### Task 7.1: Filesystem scan for describe.json

**Files:**
- Modify: `crates/greentic-ext-runtime/src/discovery.rs`
- Create: `crates/greentic-ext-runtime/tests/discovery.rs`

- [ ] **Step 1: Write failing test `tests/discovery.rs`**

```rust
use std::fs;

use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_runtime::discovery::scan_kind_dir;
use greentic_extension_sdk_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

#[test]
fn scans_kind_directory_and_returns_extension_paths() {
    let tmp = TempDir::new().unwrap();
    let design_dir = tmp.path().join("design");
    fs::create_dir_all(&design_dir).unwrap();

    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.first",
        "0.1.0",
    )
    .offer("greentic:first/y", "1.0.0")
    .with_wasm(wat::parse_str("(component)").unwrap())
    .build()
    .unwrap();

    let target = design_dir.join("greentic.first-0.1.0");
    fs::create_dir_all(&target).unwrap();
    for entry in fs::read_dir(fixture.root()).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
    }

    let found = scan_kind_dir(&design_dir).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].ends_with("greentic.first-0.1.0"));
}
```

Add dev-dep `wat = "1"` if not already.

- [ ] **Step 2: Expand `src/discovery.rs`**

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    pub user: PathBuf,
    pub project: Option<PathBuf>,
}

impl DiscoveryPaths {
    #[must_use]
    pub fn new(user: PathBuf) -> Self {
        Self {
            user,
            project: None,
        }
    }

    pub fn with_project(mut self, project: PathBuf) -> Self {
        self.project = Some(project);
        self
    }

    pub fn all(&self) -> Vec<&PathBuf> {
        let mut v = vec![&self.user];
        if let Some(p) = &self.project {
            v.push(p);
        }
        v
    }
}

/// Scan a single kind directory (e.g. `~/.greentic/extensions/design/`)
/// Returns absolute paths to each extension subdirectory that contains a
/// `describe.json`.
pub fn scan_kind_dir(kind_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !kind_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(kind_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if entry.path().join("describe.json").exists() {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-runtime --test discovery`
Expected: PASS (1 test)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/discovery.rs \
        crates/greentic-ext-runtime/tests/discovery.rs
git commit -m "feat(runtime): add kind directory scanner"
```

### Task 7.2: File watcher + hot reload

**Files:**
- Modify: `crates/greentic-ext-runtime/src/watcher.rs`
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`
- Create: `crates/greentic-ext-runtime/tests/hot_reload.rs`

- [ ] **Step 1: Write `src/watcher.rs`**

```rust
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent};

use crate::error::RuntimeError;

#[derive(Debug, Clone)]
pub enum FsEvent {
    Added(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
}

pub fn watch(paths: &[PathBuf]) -> Result<mpsc::Receiver<FsEvent>, RuntimeError> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
            if let Ok(events) = res {
                for ev in events {
                    for p in ev.event.paths {
                        let _ = tx.send(match ev.event.kind {
                            notify::EventKind::Create(_) => FsEvent::Added(p),
                            notify::EventKind::Modify(_) => FsEvent::Modified(p),
                            notify::EventKind::Remove(_) => FsEvent::Removed(p),
                            _ => continue,
                        });
                    }
                }
            }
        },
    )
    .map_err(|e| RuntimeError::Watcher(e.to_string()))?;

    for p in paths {
        if p.exists() {
            debouncer
                .watcher()
                .watch(p, RecursiveMode::Recursive)
                .map_err(|e| RuntimeError::Watcher(e.to_string()))?;
        }
    }
    std::mem::forget(debouncer);
    Ok(rx)
}
```

- [ ] **Step 2: Write `tests/hot_reload.rs`**

```rust
use std::fs;
use std::thread::sleep;
use std::time::Duration;

use greentic_ext_runtime::watcher::{watch, FsEvent};
use tempfile::TempDir;

#[test]
fn watcher_detects_new_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let rx = watch(&[root.clone()]).unwrap();

    // Allow watcher to settle
    sleep(Duration::from_millis(200));
    let new_file = root.join("newfile.txt");
    fs::write(&new_file, "hello").unwrap();

    let mut saw_event = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if let Ok(ev) = rx.recv_timeout(Duration::from_millis(200)) {
            if matches!(ev, FsEvent::Added(_) | FsEvent::Modified(_)) {
                saw_event = true;
                break;
            }
        }
    }
    assert!(saw_event, "expected FsEvent::Added/Modified");
}
```

Expose `watcher` module publicly in `lib.rs`:

```rust
pub mod watcher;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-runtime --test hot_reload`
Expected: PASS (1 test, takes ~1-3 seconds)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/watcher.rs \
        crates/greentic-ext-runtime/src/lib.rs \
        crates/greentic-ext-runtime/tests/hot_reload.rs
git commit -m "feat(runtime): add debounced filesystem watcher"
```

### Task 7.3: Wire hot reload into ExtensionRuntime

**Files:**
- Modify: `crates/greentic-ext-runtime/src/runtime.rs`

- [ ] **Step 1: Add `start_watcher` method**

Append to `src/runtime.rs`:

```rust
use tokio::task::JoinHandle;

impl ExtensionRuntime {
    /// Spawns a watcher task. Events trigger reload of the affected extension's
    /// parent directory. Returns a JoinHandle owned by the caller.
    pub fn start_watcher(self: std::sync::Arc<Self>) -> Result<JoinHandle<()>, RuntimeError> {
        let paths = self.config.paths.all().into_iter().cloned().collect::<Vec<_>>();
        let rx = crate::watcher::watch(&paths)?;
        let this = self.clone();
        let handle = tokio::task::spawn_blocking(move || {
            while let Ok(event) = rx.recv() {
                if let Err(e) = this.handle_fs_event(event) {
                    tracing::warn!(error = %e, "hot reload failed");
                }
            }
        });
        Ok(handle)
    }

    fn handle_fs_event(&self, event: crate::watcher::FsEvent) -> Result<(), RuntimeError> {
        use crate::watcher::FsEvent;
        let path = match &event {
            FsEvent::Added(p) | FsEvent::Modified(p) | FsEvent::Removed(p) => p.clone(),
        };
        // Find ancestor that contains describe.json — that's the extension dir.
        let ext_dir = find_extension_dir(&path);
        match event {
            FsEvent::Removed(_) => {
                if let Some(dir) = ext_dir {
                    self.handle_removal(&dir);
                }
            }
            FsEvent::Added(_) | FsEvent::Modified(_) => {
                if let Some(dir) = ext_dir {
                    self.handle_added_or_modified(&dir)?;
                }
            }
        }
        Ok(())
    }

    fn handle_removal(&self, dir: &std::path::Path) {
        let current = self.loaded.load();
        let Some((id, _)) = current.iter().find(|(_, v)| v.source_dir == dir) else {
            return;
        };
        let id = id.clone();
        let mut new_map = (**current).clone();
        new_map.remove(&id);
        self.loaded.store(std::sync::Arc::new(new_map));
        let _ = self.events.send(RuntimeEvent::ExtensionRemoved(id));
    }

    fn handle_added_or_modified(&self, dir: &std::path::Path) -> Result<(), RuntimeError> {
        let loaded = crate::loaded::LoadedExtension::load_from_dir(&self.engine, dir)?;
        let id = loaded.id.clone();
        let mut new_map = (**self.loaded.load()).clone();
        let prev_version = new_map.get(&id).map(|e| e.describe.metadata.version.clone());
        new_map.insert(id.clone(), std::sync::Arc::new(loaded));
        self.loaded.store(std::sync::Arc::new(new_map));
        let event = match prev_version {
            Some(prev) => RuntimeEvent::ExtensionUpdated { id, prev_version: prev },
            None => RuntimeEvent::ExtensionInstalled(id),
        };
        let _ = self.events.send(event);
        Ok(())
    }
}

fn find_extension_dir(p: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur = p;
    loop {
        if cur.join("describe.json").exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}
```

- [ ] **Step 2: Add integration test `tests/hot_reload_runtime.rs`**

```rust
use std::fs;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hot_reload_picks_up_new_extension() {
    let tmp = TempDir::new().unwrap();
    let user_root = tmp.path().join("user");
    let design_dir = user_root.join("design");
    fs::create_dir_all(&design_dir).unwrap();

    let config = RuntimeConfig::from_paths(DiscoveryPaths::new(user_root));
    let rt = Arc::new(ExtensionRuntime::new(config).unwrap());
    let _h = rt.clone().start_watcher().unwrap();

    sleep(Duration::from_millis(400));

    let fixture = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.hot",
        "0.1.0",
    )
    .offer("greentic:hot/ping", "1.0.0")
    .with_wasm(wat::parse_str("(component)").unwrap())
    .build()
    .unwrap();

    let target = design_dir.join("greentic.hot-0.1.0");
    fs::create_dir_all(&target).unwrap();
    for e in fs::read_dir(fixture.root()).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), target.join(e.file_name())).unwrap();
    }

    sleep(Duration::from_millis(1500));

    let loaded = rt.loaded();
    assert!(
        loaded.values().any(|e| e.id.as_str() == "greentic.hot"),
        "extension should be loaded; got: {:?}",
        loaded.keys().collect::<Vec<_>>()
    );
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-runtime --test hot_reload_runtime`
Expected: PASS (1 test)

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-runtime/src/runtime.rs \
        crates/greentic-ext-runtime/tests/hot_reload_runtime.rs
git commit -m "feat(runtime): integrate file watcher with runtime state"
```

---

## Phase 8 — End-to-end integration test

### Task 8.1: Full discovery + load + capability resolution round-trip

**Files:**
- Create: `crates/greentic-ext-runtime/tests/e2e_discovery.rs`

- [ ] **Step 1: Write test**

```rust
use std::fs;
use std::sync::Arc;

use greentic_extension_sdk_contract::ExtensionKind;
use greentic_ext_runtime::{DiscoveryPaths, ExtensionRuntime, RuntimeConfig};
use greentic_extension_sdk_testing::ExtensionFixtureBuilder;
use tempfile::TempDir;

async fn copy_fixture(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_to_end_discovery_and_capability_resolution() {
    let tmp = TempDir::new().unwrap();
    let user_root = tmp.path().join("user");
    fs::create_dir_all(user_root.join("design")).unwrap();
    fs::create_dir_all(user_root.join("bundle")).unwrap();

    // Offerer extension
    let offerer = ExtensionFixtureBuilder::new(
        ExtensionKind::Design,
        "greentic.offerer",
        "1.2.0",
    )
    .offer("greentic:x/service", "1.2.0")
    .with_wasm(wat::parse_str("(component)").unwrap())
    .build()
    .unwrap();

    // Consumer extension requires the offered cap
    let consumer = ExtensionFixtureBuilder::new(
        ExtensionKind::Bundle,
        "greentic.consumer",
        "0.1.0",
    )
    .require("greentic:x/service", "^1.0")
    .with_wasm(wat::parse_str("(component)").unwrap())
    .build()
    .unwrap();

    copy_fixture(offerer.root(), &user_root.join("design/greentic.offerer-1.2.0")).await;
    copy_fixture(consumer.root(), &user_root.join("bundle/greentic.consumer-0.1.0")).await;

    let mut rt = ExtensionRuntime::new(RuntimeConfig::from_paths(
        DiscoveryPaths::new(user_root.clone())
    ))
    .unwrap();

    for kind in ["design", "bundle"] {
        for path in greentic_ext_runtime::discovery::scan_kind_dir(
            &user_root.join(kind)
        ).unwrap() {
            rt.register_loaded_from_dir(&path).await.unwrap();
        }
    }

    let registry = rt.capability_registry();
    let plan = registry.resolve(
        "greentic.consumer",
        &[greentic_extension_sdk_contract::CapabilityRef {
            id: "greentic:x/service".parse().unwrap(),
            version: "^1.0".into(),
        }],
    );
    assert!(plan.unresolved.is_empty(), "unresolved: {:?}", plan.unresolved);
    assert_eq!(plan.resolved.len(), 1);
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p greentic-ext-runtime --test e2e_discovery`
Expected: PASS (1 test)

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-runtime/tests/e2e_discovery.rs
git commit -m "test(runtime): end-to-end discovery + resolution"
```

### Task 8.2: Full workspace CI check

**Files:** none

- [ ] **Step 1: Run local check**

Run: `ci/local_check.sh`
Expected: all green (fmt, clippy, tests, release build)

- [ ] **Step 2: Tag foundation milestone**

```bash
git tag -a v0.1.0-foundation -m "Foundation milestone: WIT + contract + runtime core"
git log --oneline v0.1.0-foundation
```

Expected: tag appears in log.

---

## Phase 9 — `greentic-ext-cli` crate skeleton (full CLI in Plan 2)

### Task 9.1: Scaffold CLI crate

**Files:**
- Create: `crates/greentic-ext-cli/Cargo.toml`
- Create: `crates/greentic-ext-cli/src/main.rs`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "greentic-ext-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "gtdx: CLI for Greentic Designer Extensions"

[[bin]]
name = "gtdx"
path = "src/main.rs"

[dependencies]
anyhow = { workspace = true }
clap = { workspace = true }
greentic-extension-sdk-contract = { path = "../greentic-extension-sdk-contract" }
greentic-ext-runtime = { path = "../greentic-ext-runtime" }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Create `src/main.rs` with stub commands**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gtdx", version, about = "Greentic Designer Extensions CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate an extension directory or .gtxpack file
    Validate {
        #[arg(default_value = ".")]
        path: String,
    },
    /// List installed extensions
    List,
    /// Placeholder — full CLI implemented in Plan 2
    Version,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Validate { path } => {
            let describe_path = std::path::Path::new(&path).join("describe.json");
            let bytes = std::fs::read(&describe_path)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            greentic_extension_sdk_contract::schema::validate_describe_json(&value)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("✓ {} valid", describe_path.display());
        }
        Command::List => {
            println!("Extension listing not yet implemented (see Plan 2)");
        }
        Command::Version => {
            println!("gtdx {}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build + run**

Run: `cargo build -p greentic-ext-cli`
Expected: clean build

Run: `./target/debug/gtdx version`
Expected: prints version

Run: `./target/debug/gtdx --help`
Expected: prints help with 3 subcommands

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-cli/
git commit -m "feat(cli): scaffold gtdx binary with validate/list/version stubs"
```

---

## Self-review checklist (run before handoff)

- [ ] Spec section 5 (describe.json, base WIT, host WIT) → implemented in Phases 2 + 3
- [ ] Spec section 6 (kind-specific sub-WIT) → Phase 2
- [ ] Spec section 7.1 (ExtensionRuntime skeleton) → Phase 5
- [ ] Spec section 7.2 (CapabilityRegistry + matching) → Phase 6
- [ ] Spec section 7.3 (discovery + hot reload) → Phase 7
- [ ] Spec section 7.4 (host broker) → Task 6.4
- [ ] Spec section 8 (CLI) → Phase 9 stub; full impl in Plan 2
- [ ] Spec section 9 (Store registry) → Plan 2
- [ ] Spec section 10 (AC reference extension) → Plan 3
- [ ] Spec section 10.3-10.4 (Designer refactor) → Plan 4

Gaps handled by follow-up plans: Plan 2 covers CLI completion + 3 registry
implementations + install lifecycle. Plan 3 covers AC extension crate. Plan 4
covers designer refactor + docs.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-17-foundation.md`.

Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session using executing-plans, batch with checkpoints

Plans 2, 3, 4 are follow-up cycles — brainstorm + write when Plan 1 ships
green.

Which approach?
