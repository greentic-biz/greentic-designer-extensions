# `gtdx new --kind provider` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Add the 4th scaffold kind `provider` so `gtdx new my-ext --kind provider` produces a compilable provider-extension project. Closes the Wave A gap where the contract has `ExtensionKind::Provider` + 6 WIT worlds but the scaffold still only knows design/bundle/deploy.

**Architecture:** Follow the existing Track A pattern — add `Provider` to `scaffold::Kind`, register `TEMPLATES_PROVIDER`, and drop 4 templates (`describe.json.tmpl`, `Cargo.toml.tmpl`, `wit/world.wit.tmpl`, `src/lib.rs.tmpl`) under `templates/provider/`. Default generated world is `messaging-only-provider` (the most common provider shape: Slack/Telegram/WhatsApp). The scaffold's describe.json includes a placeholder `runtime.gtpack` block because the contract enforces `kind=ProviderExtension ↔ runtime.gtpack.is_some()`; the README-block inside `src/lib.rs` tells the author to replace `file`/`sha256` with their real `.gtpack` values before publish.

**Tech Stack:** Rust 1.94, existing `include_dir!` + template pipeline, cargo-component 0.21 + wit-bindgen 0.35 (same as other kinds).

---

## File Structure

### Create

- `crates/greentic-ext-cli/templates/provider/describe.json.tmpl`
- `crates/greentic-ext-cli/templates/provider/Cargo.toml.tmpl`
- `crates/greentic-ext-cli/templates/provider/wit/world.wit.tmpl`
- `crates/greentic-ext-cli/templates/provider/src/lib.rs.tmpl`

### Modify

- `crates/greentic-ext-cli/src/scaffold/mod.rs` — add `Provider` variant.
- `crates/greentic-ext-cli/src/scaffold/template.rs` — register `TEMPLATES_PROVIDER`, add `"provider"` match arm.
- `crates/greentic-ext-cli/src/scaffold/embedded.rs` — update test `wit_files_returns_all_embedded_packages` count (contract now has 7 `.wit` files including extension-provider); add a test for provider filter.
- `crates/greentic-ext-cli/tests/cli_new.rs` — add provider scaffold test; include provider in schema-conformance loop.
- `CHANGELOG.md` — note the new kind.

---

## Task 1: Kind::Provider enum + template loader wiring

**Files:**
- Modify: `crates/greentic-ext-cli/src/scaffold/mod.rs`
- Modify: `crates/greentic-ext-cli/src/scaffold/template.rs`

- [ ] **Step 1: Add Provider variant**

`scaffold/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Design,
    Bundle,
    Deploy,
    Provider,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Design => "design",
            Kind::Bundle => "bundle",
            Kind::Deploy => "deploy",
            Kind::Provider => "provider",
        }
    }
}
```

- [ ] **Step 2: Register provider templates in loader**

`scaffold/template.rs`: add alongside the other `TEMPLATES_*` statics:

```rust
static TEMPLATES_PROVIDER: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/provider");
```

Add match arm in `load_templates_kind`:

```rust
pub fn load_templates_kind(kind: &str) -> Vec<TemplateEntry> {
    match kind {
        "design" => collect(&TEMPLATES_DESIGN),
        "bundle" => collect(&TEMPLATES_BUNDLE),
        "deploy" => collect(&TEMPLATES_DEPLOY),
        "provider" => collect(&TEMPLATES_PROVIDER),
        _ => Vec::new(),
    }
}
```

- [ ] **Step 3: Create provider template dir with a placeholder file**

`include_dir!` fails at compile time if the path does not exist. Create the dir with a stub before the match arm compiles:

```bash
mkdir -p crates/greentic-ext-cli/templates/provider/wit crates/greentic-ext-cli/templates/provider/src
printf '{}\n' > crates/greentic-ext-cli/templates/provider/describe.json.tmpl
```

(Real content lands in Tasks 2–5. This stub just lets the crate compile right now so the next tasks can iterate.)

- [ ] **Step 4: Verify build**

Run: `cargo build -p greentic-ext-cli --quiet 2>&1 | tail -5`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/src/scaffold/mod.rs crates/greentic-ext-cli/src/scaffold/template.rs crates/greentic-ext-cli/templates/provider
git commit -m "feat(ext-cli): scaffold Kind::Provider + TEMPLATES_PROVIDER loader (stub templates)"
```

---

## Task 2: Provider describe.json template

**File:** `crates/greentic-ext-cli/templates/provider/describe.json.tmpl`

Provider extensions require `runtime.gtpack` per the contract invariant (`crates/greentic-ext-contract/src/describe/mod.rs: TryFrom<DescribeJsonRaw>` enforces `kind == ProviderExtension ↔ runtime.gtpack.is_some()`).

Replace the stub with the real template (scaffold-time placeholders — author edits before publish):

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "ProviderExtension",
  "metadata": {
    "id": "{{id}}",
    "name": "{{name}}",
    "version": "{{version}}",
    "summary": "A Greentic Designer provider extension.",
    "author": {
      "name": "{{author}}"
    },
    "license": "{{license}}"
  },
  "engine": {
    "greenticDesigner": "^{{contract_version}}",
    "extRuntime": "^{{contract_version}}"
  },
  "capabilities": {
    "offered": [],
    "required": []
  },
  "runtime": {
    "component": "extension.wasm",
    "permissions": {
      "network": [],
      "secrets": [],
      "callExtensionKinds": []
    },
    "gtpack": {
      "file": "REPLACE_WITH_YOUR.gtpack",
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
      "pack_id": "{{id}}",
      "component_version": "{{version}}"
    }
  },
  "contributions": {}
}
```

- [ ] **Step 1: Write the file.**

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/provider/describe.json.tmpl
git commit -m "feat(ext-cli): provider describe.json template (placeholder runtime.gtpack)"
```

---

## Task 3: Provider Cargo.toml + world.wit templates

**Files:**
- `crates/greentic-ext-cli/templates/provider/Cargo.toml.tmpl`
- `crates/greentic-ext-cli/templates/provider/wit/world.wit.tmpl`

### Step 1: Cargo.toml.tmpl

```toml
[package]
name = "{{name}}"
version = "{{version}}"
edition = "2024"
license = "{{license}}"
authors = ["{{author}}"]

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen-rt = { version = "0.35", features = ["bitflags"] }

[package.metadata.component]
package = "{{id_wit}}"

[package.metadata.component.target]
path = "wit"

[package.metadata.component.target.dependencies]
"greentic:extension-base" = { path = "wit/deps/greentic/extension-base" }
"greentic:extension-host" = { path = "wit/deps/greentic/extension-host" }
"greentic:extension-provider" = { path = "wit/deps/greentic/extension-provider" }
```

### Step 2: wit/world.wit.tmpl (messaging-only-provider shape)

```wit
package {{id_wit}};

world extension {
  import greentic:extension-base/types@{{contract_version}};
  import greentic:extension-host/logging@{{contract_version}};
  import greentic:extension-host/i18n@{{contract_version}};

  export greentic:extension-base/manifest@{{contract_version}};
  export greentic:extension-base/lifecycle@{{contract_version}};
  export greentic:extension-provider/messaging@{{contract_version}};
}
```

### Step 3: Commit

```bash
git add crates/greentic-ext-cli/templates/provider/Cargo.toml.tmpl crates/greentic-ext-cli/templates/provider/wit/world.wit.tmpl
git commit -m "feat(ext-cli): provider Cargo.toml + world.wit (messaging-only default)"
```

---

## Task 4: Provider src/lib.rs template

**File:** `crates/greentic-ext-cli/templates/provider/src/lib.rs.tmpl`

Messaging interface has 5 functions: `list_channels`, `describe_channel`, `secret_schema`, `config_schema`, `dry_run_encode`. Plus 3 base functions (`manifest::get-identity/get-offered/get-required`) and 2 lifecycle (`init/shutdown`). All TODO-stubs.

### Step 1: Write template

```rust
// Provider extension guest for {{id}}.
//
// This scaffold implements the messaging-only provider contract as TODO stubs.
// Two extra steps a provider author must complete before `gtdx publish`:
//
// 1. Replace the bundled `.gtpack` runtime: set
//    `runtime.gtpack.file`/`sha256` in `describe.json` to your real pack.
// 2. If your provider also sources events / sinks them, switch `wit/world.wit`
//    to one of the `*-and-*-provider` or `full-provider` worlds from
//    `wit/deps/greentic/extension-provider/world.wit`, and add the
//    corresponding `impl ... ::Guest for Component` blocks below.

#[allow(warnings)]
mod bindings;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_provider::messaging;
use bindings::greentic::extension_base::types;
use bindings::greentic::extension_provider::types as provider_types;

struct Component;

// ---- extension-base/manifest ----
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "{{id}}".to_string(),
            version: "{{version}}".to_string(),
            kind: types::Kind::Provider,
        }
    }

    fn get_offered() -> Vec<types::CapabilityRef> {
        Vec::new()
    }

    fn get_required() -> Vec<types::CapabilityRef> {
        Vec::new()
    }
}

// ---- extension-base/lifecycle ----
impl lifecycle::Guest for Component {
    fn init(_config_json: String) -> Result<(), types::ExtensionError> {
        // TODO: initialize any state the provider needs (clients, caches, etc.).
        Ok(())
    }

    fn shutdown() {
        // TODO: flush/close whatever `init` set up.
    }
}

// ---- extension-provider/messaging ----
impl messaging::Guest for Component {
    fn list_channels() -> Vec<messaging::ChannelProfile> {
        // TODO: return every channel this provider can talk to.
        Vec::new()
    }

    fn describe_channel(
        id: String,
    ) -> Result<messaging::ChannelProfile, provider_types::Error> {
        // TODO: look up and return the channel profile.
        Err(provider_types::Error::NotFound(id))
    }

    fn secret_schema(id: String) -> Result<String, provider_types::Error> {
        // TODO: return a JSON Schema for the channel's credentials.
        Err(provider_types::Error::NotFound(id))
    }

    fn config_schema(id: String) -> Result<String, provider_types::Error> {
        // TODO: return a JSON Schema for per-channel configuration.
        Err(provider_types::Error::NotFound(id))
    }

    fn dry_run_encode(
        id: String,
        _sample: Vec<u8>,
    ) -> Result<Vec<u8>, provider_types::Error> {
        // TODO: encode the sample as this channel's outbound envelope.
        Err(provider_types::Error::NotFound(id))
    }
}

bindings::export!(Component with_types_in bindings);
```

### Step 2: Verify the `types::Kind::Provider` enum variant exists in the vendored extension-base WIT

Cross-check `wit/extension-base.wit` — the `kind` enum already has `provider`. The Rust enum variant after wit-bindgen is `types::Kind::Provider`. Good.

### Step 3: Commit

```bash
git add crates/greentic-ext-cli/templates/provider/src/lib.rs.tmpl
git commit -m "feat(ext-cli): provider src/lib.rs stubs for manifest + lifecycle + messaging"
```

---

## Task 5: Update embedded.rs test count + add provider filter test

**File:** `crates/greentic-ext-cli/src/scaffold/embedded.rs`

The test `wit_files_returns_all_embedded_packages` asserted `files.len() == 6` originally; after Wave A's `extension-provider.wit` landed it became 7 (per Wave A fix note). Verify and add a provider filter test.

### Step 1: Update the count test

Find the `wit_files_returns_all_embedded_packages` test. If count is currently `6`, bump to `7`; if already `7`, leave it. Also assert `extension-provider.wit` is in the list.

Change the test body to (replace existing assertions with):

```rust
    #[test]
    fn wit_files_returns_all_embedded_packages() {
        let files = wit_files();
        assert!(files.iter().any(|f| f.name == "extension-base.wit"));
        assert!(files.iter().any(|f| f.name == "extension-host.wit"));
        assert!(files.iter().any(|f| f.name == "extension-design.wit"));
        assert!(files.iter().any(|f| f.name == "extension-bundle.wit"));
        assert!(files.iter().any(|f| f.name == "extension-deploy.wit"));
        assert!(files.iter().any(|f| f.name == "extension-provider.wit"));
        assert_eq!(files.len(), 7);
    }
```

### Step 2: Add provider filter test

Append inside the existing tests mod:

```rust
    #[test]
    fn files_for_kind_provider_includes_provider_not_design() {
        let files = files_for_kind("provider");
        let names: Vec<_> = files.iter().map(|f| f.name).collect();
        assert!(names.contains(&"extension-base.wit"));
        assert!(names.contains(&"extension-host.wit"));
        assert!(names.contains(&"extension-provider.wit"));
        assert!(!names.contains(&"extension-design.wit"));
        assert!(!names.contains(&"extension-bundle.wit"));
        assert!(!names.contains(&"extension-deploy.wit"));
    }
```

### Step 3: Run tests

Run: `cargo test -p greentic-ext-cli --bins scaffold::embedded 2>&1 | tail -10`
Expected: existing + new tests pass.

### Step 4: Commit

```bash
git add crates/greentic-ext-cli/src/scaffold/embedded.rs
git commit -m "test(ext-cli): scaffold embedded — verify 7 WIT files + provider per-kind filter"
```

---

## Task 6: Integration tests — cli_new covers provider

**File:** `crates/greentic-ext-cli/tests/cli_new.rs`

### Step 1: Read existing file structure

Current `cli_new.rs` has `scaffolds_design_extension_and_lock_file_matches_bytes`, `scaffolds_bundle_extension_with_correct_wit_deps`, `scaffolds_deploy_extension_with_correct_wit_deps`, the target-dir conflict tests, a schema-conformance loop over design/bundle/deploy, and the `cargo check` smoke.

### Step 2: Add provider scaffold test

Append:

```rust
#[test]
fn scaffolds_provider_extension_with_correct_wit_deps() {
    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("p");
    let (ok, _o, e) = run(Command::new(gtdx_bin())
        .arg("new")
        .arg("p")
        .arg("--kind")
        .arg("provider")
        .arg("--dir")
        .arg(&proj)
        .arg("-y")
        .arg("--no-git"));
    assert!(ok, "gtdx new provider failed: {e}");
    assert!(
        proj.join("wit/deps/greentic/extension-provider/world.wit")
            .exists()
    );
    assert!(!proj.join("wit/deps/greentic/extension-design/world.wit").exists());
    assert!(!proj.join("wit/deps/greentic/extension-bundle/world.wit").exists());
    assert!(!proj.join("wit/deps/greentic/extension-deploy/world.wit").exists());

    let describe = std::fs::read_to_string(proj.join("describe.json")).unwrap();
    assert!(describe.contains("\"kind\": \"ProviderExtension\""));
    assert!(describe.contains("\"gtpack\""));
    assert!(describe.contains("REPLACE_WITH_YOUR.gtpack"));
}
```

### Step 3: Extend schema-conformance loop to include provider

Find the existing `scaffolded_describe_json_validates_against_schema` test. Its loop iterates over `[("design", "design-demo"), ("bundle", "bundle-demo"), ("deploy", "deploy-demo")]`. Add provider:

```rust
    for (kind_flag, scaffold_name) in [
        ("design", "design-demo"),
        ("bundle", "bundle-demo"),
        ("deploy", "deploy-demo"),
        ("provider", "provider-demo"),
    ] {
        // existing body unchanged
    }
```

### Step 4: Run tests

Run: `cargo test -p greentic-ext-cli --test cli_new 2>&1 | tail -15`
Expected: all tests pass (existing + new provider + provider included in schema loop).

### Step 5: Commit

```bash
git add crates/greentic-ext-cli/tests/cli_new.rs
git commit -m "test(ext-cli): cli_new covers provider scaffold + schema conformance"
```

---

## Task 7: Author-run smoke — `gtdx new --kind provider` then `cargo component build`

Controller runs this directly.

- [ ] **Step 1:** `cargo build -p greentic-ext-cli --quiet`
- [ ] **Step 2:**

```bash
TMP=$(mktemp -d)
./target/debug/gtdx new p --kind provider \
  --id com.example.mytelegram \
  --dir "$TMP/p" \
  --author tester -y --no-git
(cd "$TMP/p" && cargo component build --target wasm32-wasip2 2>&1 | tail -8)
ls "$TMP/p/target/wasm32-wasip1/debug/"*.wasm 2>&1
```

Expected: `cargo component build` succeeds, `.wasm` lands under `target/wasm32-wasip1/debug/`.

- [ ] **Step 3:** Verify `gtdx publish --dry-run --manifest ...` accepts the scaffolded describe (schema + invariant validator):

```bash
GREENTIC_HOME="$TMP/home" ./target/debug/gtdx publish --dry-run \
  --manifest "$TMP/p/Cargo.toml" 2>&1 | tail -5
```

Expected: either prints `dry-run: would publish ...` OR (most likely) aborts with a describe-validation error naming the placeholder `runtime.gtpack.sha256`. If the latter, the placeholder sha256 (all zeros) is technically valid-format (64 hex chars) so the dry-run should proceed; the failure mode — if any — is informational for documentation.

- [ ] **Step 4:** No commit — record in PR.

---

## Task 8: Final gate + PR

Controller.

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0
- [ ] `cargo test --workspace --all-targets` → all green
- [ ] Commit stragglers if any
- [ ] Push + `gh pr create`

---

## Acceptance

1. `gtdx new <name> --kind provider` produces a scaffold whose layout matches design/bundle/deploy but with `wit/deps/greentic/extension-provider/world.wit` vendored (Task 6 test).
2. Scaffolded provider `cargo component build --target wasm32-wasip2` succeeds (Task 7).
3. Scaffolded `describe.json` passes the describe-v1 schema and the `kind ↔ gtpack` invariant (schema-conformance loop in Task 6 covers this).
4. `files_for_kind("provider")` returns base + host + provider and excludes the other per-kind WITs (Task 5 test).
5. Workspace fmt + clippy + test green (Task 8).
