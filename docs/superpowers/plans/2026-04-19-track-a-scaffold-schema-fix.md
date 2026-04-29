# Track A Scaffold Schema Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the three Track A scaffold kinds (`design` / `bundle` / `deploy`) so a freshly-scaffolded project (`gtdx new demo`) produces a `describe.json` that validates against `describe-v1.json`, a `wit/world.wit` that matches the real vendored WIT contract, and a `src/lib.rs` that implements all required WIT exports as TODO stubs. End state: `gtdx new demo && gtdx dev --once` completes the full build → pack → install pipeline on Linux.

**Architecture:** Templates live in `crates/greentic-ext-cli/templates/{common,design,bundle,deploy}/` and are embedded via `include_dir!` (Track A wiring, unchanged). This PR only rewrites template content. No new dependencies. Tests in `crates/greentic-ext-cli/tests/cli_new.rs` and `rescaffold_reference_exts.rs` are updated to assert the new shapes, and a new schema-conformance assertion uses the already-available `jsonschema` workspace dep.

**Tech Stack:** Rust 1.94, edition 2024, cargo-component 0.21, wit-bindgen 0.35 (per Track A scaffold), `jsonschema` 0.30, existing Track A scaffold machinery.

**Parent:** Phase 1 Track B PR #9 acceptance gate (build blocker #1 + #2).

---

## File Structure

### Modify

- `crates/greentic-ext-cli/templates/design/describe.json.tmpl`
- `crates/greentic-ext-cli/templates/design/wit/world.wit.tmpl`
- `crates/greentic-ext-cli/templates/design/src/lib.rs.tmpl`
- `crates/greentic-ext-cli/templates/bundle/describe.json.tmpl`
- `crates/greentic-ext-cli/templates/bundle/wit/world.wit.tmpl`
- `crates/greentic-ext-cli/templates/bundle/src/lib.rs.tmpl`
- `crates/greentic-ext-cli/templates/deploy/describe.json.tmpl`
- `crates/greentic-ext-cli/templates/deploy/wit/world.wit.tmpl`
- `crates/greentic-ext-cli/templates/deploy/src/lib.rs.tmpl`
- `crates/greentic-ext-cli/Cargo.toml` — add `jsonschema` as dev-dep
- `crates/greentic-ext-cli/tests/cli_new.rs` — update kind-string assertions + add schema conformance test

### No change

- Cargo.toml workspace, scaffolding orchestration (`scaffold/{embedded,preflight,template,contract_lock}.rs`), `gtdx dev` code, or any other existing crate behavior.

---

## Task 1: Add jsonschema dev-dep to ext-cli

**Files:**
- Modify: `crates/greentic-ext-cli/Cargo.toml`

- [ ] **Step 1: Add jsonschema under `[dev-dependencies]`**

In `crates/greentic-ext-cli/Cargo.toml`, find the `[dev-dependencies]` block and add:

```toml
jsonschema = { workspace = true }
```

Keep existing entries untouched.

- [ ] **Step 2: Verify workspace still resolves**

Run: `cargo metadata --format-version 1 > /dev/null`
Expected: exit 0.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/Cargo.toml
git commit -m "chore(ext-cli): add jsonschema dev-dep for scaffold schema conformance tests"
```

---

## Task 2: Rewrite design describe.json template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/design/describe.json.tmpl`

The current describe emits `"kind": "design"` and uses pre-schema fields (`description`, `authors`, top-level `permissions`, `engine.contract`, no `contributions`). The schema (`crates/greentic-extension-sdk-contract/schemas/describe-v1.json`) requires: `kind ∈ {DesignExtension,BundleExtension,DeployExtension}`, `metadata.{id,name,version,summary,author:{name},license}`, `engine.{greenticDesigner,extRuntime}`, `runtime.{component,permissions}`, `contributions` (object).

- [ ] **Step 1: Replace file with schema-compliant template**

Content:

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "{{id}}",
    "name": "{{name}}",
    "version": "{{version}}",
    "summary": "A Greentic Designer design extension.",
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
    }
  },
  "contributions": {}
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/design/describe.json.tmpl
git commit -m "feat(ext-cli): design describe.json template conforms to describe-v1 schema"
```

---

## Task 3: Rewrite bundle describe.json template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/bundle/describe.json.tmpl`

- [ ] **Step 1: Replace file with schema-compliant template**

Same shape as design, but `kind = "BundleExtension"` and summary text updated:

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "BundleExtension",
  "metadata": {
    "id": "{{id}}",
    "name": "{{name}}",
    "version": "{{version}}",
    "summary": "A Greentic Designer bundle extension.",
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
    }
  },
  "contributions": {}
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/bundle/describe.json.tmpl
git commit -m "feat(ext-cli): bundle describe.json template conforms to describe-v1 schema"
```

---

## Task 4: Rewrite deploy describe.json template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/deploy/describe.json.tmpl`

- [ ] **Step 1: Replace file**

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DeployExtension",
  "metadata": {
    "id": "{{id}}",
    "name": "{{name}}",
    "version": "{{version}}",
    "summary": "A Greentic Designer deploy extension.",
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
    }
  },
  "contributions": {}
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/deploy/describe.json.tmpl
git commit -m "feat(ext-cli): deploy describe.json template conforms to describe-v1 schema"
```

---

## Task 5: Rewrite design world.wit template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/design/wit/world.wit.tmpl`

The contract world lives in `wit/deps/greentic/extension-design/world.wit` as `world design-extension`. It imports 6 interfaces (extension-base/types + 5 host facets) and exports 6 interfaces (manifest, lifecycle, tools, validation, prompting, knowledge). The scaffold world must match so cargo-component can merge targets.

- [ ] **Step 1: Replace file**

```wit
package {{id_wit}};

world extension {
  import greentic:extension-base/types@{{contract_version}};
  import greentic:extension-host/logging@{{contract_version}};
  import greentic:extension-host/i18n@{{contract_version}};
  import greentic:extension-host/secrets@{{contract_version}};
  import greentic:extension-host/broker@{{contract_version}};
  import greentic:extension-host/http@{{contract_version}};

  export greentic:extension-base/manifest@{{contract_version}};
  export greentic:extension-base/lifecycle@{{contract_version}};
  export greentic:extension-design/tools@{{contract_version}};
  export greentic:extension-design/validation@{{contract_version}};
  export greentic:extension-design/prompting@{{contract_version}};
  export greentic:extension-design/knowledge@{{contract_version}};
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/design/wit/world.wit.tmpl
git commit -m "feat(ext-cli): design world.wit imports/exports match real extension-design contract"
```

---

## Task 6: Rewrite bundle world.wit template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/bundle/wit/world.wit.tmpl`

Bundle contract world imports: extension-base/types + host/logging + host/i18n + host/broker. Exports: manifest, lifecycle, recipes, bundling.

- [ ] **Step 1: Replace file**

```wit
package {{id_wit}};

world extension {
  import greentic:extension-base/types@{{contract_version}};
  import greentic:extension-host/logging@{{contract_version}};
  import greentic:extension-host/i18n@{{contract_version}};
  import greentic:extension-host/broker@{{contract_version}};

  export greentic:extension-base/manifest@{{contract_version}};
  export greentic:extension-base/lifecycle@{{contract_version}};
  export greentic:extension-bundle/recipes@{{contract_version}};
  export greentic:extension-bundle/bundling@{{contract_version}};
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/bundle/wit/world.wit.tmpl
git commit -m "feat(ext-cli): bundle world.wit imports/exports match real extension-bundle contract"
```

---

## Task 7: Rewrite deploy world.wit template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/deploy/wit/world.wit.tmpl`

Deploy contract world imports: extension-base/types + host/logging + host/i18n + host/secrets + host/http. Exports: manifest, lifecycle, targets, deployment.

- [ ] **Step 1: Replace file**

```wit
package {{id_wit}};

world extension {
  import greentic:extension-base/types@{{contract_version}};
  import greentic:extension-host/logging@{{contract_version}};
  import greentic:extension-host/i18n@{{contract_version}};
  import greentic:extension-host/secrets@{{contract_version}};
  import greentic:extension-host/http@{{contract_version}};

  export greentic:extension-base/manifest@{{contract_version}};
  export greentic:extension-base/lifecycle@{{contract_version}};
  export greentic:extension-deploy/targets@{{contract_version}};
  export greentic:extension-deploy/deployment@{{contract_version}};
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/deploy/wit/world.wit.tmpl
git commit -m "feat(ext-cli): deploy world.wit imports/exports match real extension-deploy contract"
```

---

## Task 8: Rewrite design src/lib.rs template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/design/src/lib.rs.tmpl`

Must implement all 6 exported interfaces (manifest + lifecycle + tools + validation + prompting + knowledge) as TODO stubs that compile and produce a valid wasm component.

- [ ] **Step 1: Replace file**

```rust
// Design extension guest for {{id}}.
//
// This scaffold implements every export required by the extension-design
// contract as a TODO stub. Replace each body with real logic before shipping.

wit_bindgen::generate!({
    world: "extension",
    path: "wit",
});

use exports::greentic::extension_base::{lifecycle, manifest};
use exports::greentic::extension_design::{knowledge, prompting, tools, validation};
use greentic::extension_base::types;

struct Component;

// ---- extension-base/manifest ----
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "{{id}}".to_string(),
            version: "{{version}}".to_string(),
            kind: types::Kind::Design,
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
        // TODO: read configuration from `config_json` and initialize state.
        Ok(())
    }

    fn shutdown() {
        // TODO: release any resources the extension owns.
    }
}

// ---- extension-design/tools ----
impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        // TODO: return the list of tools the designer may invoke.
        Vec::new()
    }

    fn invoke_tool(
        name: String,
        _args_json: String,
    ) -> Result<String, types::ExtensionError> {
        // TODO: dispatch on `name` and return a JSON-encoded result.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown tool: {name}"
        )))
    }
}

// ---- extension-design/validation ----
impl validation::Guest for Component {
    fn validate_content(
        _content_type: String,
        _content_json: String,
    ) -> validation::ValidateResult {
        // TODO: run domain-specific validation and emit diagnostics.
        validation::ValidateResult {
            valid: true,
            diagnostics: Vec::new(),
        }
    }
}

// ---- extension-design/prompting ----
impl prompting::Guest for Component {
    fn system_prompt_fragments() -> Vec<prompting::PromptFragment> {
        // TODO: contribute prompt fragments for the designer LLM context.
        Vec::new()
    }
}

// ---- extension-design/knowledge ----
impl knowledge::Guest for Component {
    fn list_entries(
        _category_filter: Option<String>,
    ) -> Vec<knowledge::EntrySummary> {
        // TODO: return the knowledge entries this extension offers.
        Vec::new()
    }

    fn get_entry(id: String) -> Result<knowledge::Entry, types::ExtensionError> {
        // TODO: look up and return the knowledge entry.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown entry: {id}"
        )))
    }

    fn suggest_entries(
        _query: String,
        _limit: u32,
    ) -> Vec<knowledge::EntrySummary> {
        // TODO: rank knowledge entries for `query`.
        Vec::new()
    }
}

export!(Component);
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/design/src/lib.rs.tmpl
git commit -m "feat(ext-cli): design src/lib.rs implements all 6 required exports as TODO stubs"
```

---

## Task 9: Rewrite bundle src/lib.rs template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/bundle/src/lib.rs.tmpl`

- [ ] **Step 1: Replace file**

```rust
// Bundle extension guest for {{id}}.
//
// This scaffold implements every export required by the extension-bundle
// contract as a TODO stub. Replace each body with real logic before shipping.

wit_bindgen::generate!({
    world: "extension",
    path: "wit",
});

use exports::greentic::extension_base::{lifecycle, manifest};
use exports::greentic::extension_bundle::{bundling, recipes};
use greentic::extension_base::types;

struct Component;

// ---- extension-base/manifest ----
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "{{id}}".to_string(),
            version: "{{version}}".to_string(),
            kind: types::Kind::Bundle,
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
        // TODO: initialize any state the extension needs.
        Ok(())
    }

    fn shutdown() {
        // TODO: release any resources the extension owns.
    }
}

// ---- extension-bundle/recipes ----
impl recipes::Guest for Component {
    fn list_recipes() -> Vec<recipes::RecipeSummary> {
        // TODO: return every recipe this extension can bundle.
        Vec::new()
    }

    fn recipe_config_schema(
        recipe_id: String,
    ) -> Result<String, types::ExtensionError> {
        // TODO: return a JSON Schema for the recipe's config.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown recipe: {recipe_id}"
        )))
    }

    fn supported_capabilities(
        recipe_id: String,
    ) -> Result<Vec<String>, types::ExtensionError> {
        // TODO: list capabilities the recipe can satisfy.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown recipe: {recipe_id}"
        )))
    }
}

// ---- extension-bundle/bundling ----
impl bundling::Guest for Component {
    fn validate_config(
        _recipe_id: String,
        _config_json: String,
    ) -> Vec<types::Diagnostic> {
        // TODO: emit configuration diagnostics.
        Vec::new()
    }

    fn render(
        recipe_id: String,
        _config_json: String,
        _session: bundling::DesignerSession,
    ) -> Result<bundling::BundleArtifact, types::ExtensionError> {
        // TODO: render the recipe into a bundle artifact.
        Err(types::ExtensionError::Internal(format!(
            "render not implemented for recipe: {recipe_id}"
        )))
    }
}

export!(Component);
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/bundle/src/lib.rs.tmpl
git commit -m "feat(ext-cli): bundle src/lib.rs implements all 4 required exports as TODO stubs"
```

---

## Task 10: Rewrite deploy src/lib.rs template

**Files:**
- Modify: `crates/greentic-ext-cli/templates/deploy/src/lib.rs.tmpl`

- [ ] **Step 1: Replace file**

```rust
// Deploy extension guest for {{id}}.
//
// This scaffold implements every export required by the extension-deploy
// contract as a TODO stub. Replace each body with real logic before shipping.

wit_bindgen::generate!({
    world: "extension",
    path: "wit",
});

use exports::greentic::extension_base::{lifecycle, manifest};
use exports::greentic::extension_deploy::{deployment, targets};
use greentic::extension_base::types;

struct Component;

// ---- extension-base/manifest ----
impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "{{id}}".to_string(),
            version: "{{version}}".to_string(),
            kind: types::Kind::Deploy,
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
        // TODO: initialize any state the extension needs.
        Ok(())
    }

    fn shutdown() {
        // TODO: release any resources the extension owns.
    }
}

// ---- extension-deploy/targets ----
impl targets::Guest for Component {
    fn list_targets() -> Vec<targets::TargetSummary> {
        // TODO: return every deploy target this extension supports.
        Vec::new()
    }

    fn credential_schema(
        target_id: String,
    ) -> Result<String, types::ExtensionError> {
        // TODO: return the JSON Schema describing required credentials.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown target: {target_id}"
        )))
    }

    fn config_schema(target_id: String) -> Result<String, types::ExtensionError> {
        // TODO: return the JSON Schema describing required config.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown target: {target_id}"
        )))
    }

    fn validate_credentials(
        _target_id: String,
        _credentials_json: String,
    ) -> Vec<types::Diagnostic> {
        // TODO: emit credential diagnostics.
        Vec::new()
    }
}

// ---- extension-deploy/deployment ----
impl deployment::Guest for Component {
    fn deploy(
        _req: deployment::DeployRequest,
    ) -> Result<deployment::DeployJob, types::ExtensionError> {
        // TODO: launch the deployment and return a job handle.
        Err(types::ExtensionError::Internal("deploy not implemented".into()))
    }

    fn poll(job_id: String) -> Result<deployment::DeployJob, types::ExtensionError> {
        // TODO: fetch current status for `job_id`.
        Err(types::ExtensionError::InvalidInput(format!(
            "unknown job: {job_id}"
        )))
    }

    fn rollback(_job_id: String) -> Result<(), types::ExtensionError> {
        // TODO: roll back the deployment.
        Err(types::ExtensionError::Internal("rollback not implemented".into()))
    }
}

export!(Component);
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/deploy/src/lib.rs.tmpl
git commit -m "feat(ext-cli): deploy src/lib.rs implements all 4 required exports as TODO stubs"
```

---

## Task 11: Update cli_new.rs assertions

**Files:**
- Modify: `crates/greentic-ext-cli/tests/cli_new.rs`

The existing tests assert the OLD kind strings (`"kind": "design"`, `"kind": "bundle"`, `"kind": "deploy"`). Update each assertion to the new schema-compliant form.

- [ ] **Step 1: Identify existing assertions**

Run: `grep -n '"kind"' crates/greentic-ext-cli/tests/cli_new.rs`
Expected: three lines each for design / bundle / deploy kind-string asserts.

- [ ] **Step 2: Replace each occurrence**

For each of the three existing occurrences, change:
```rust
assert!(describe.contains("\"kind\": \"design\""));
```
to:
```rust
assert!(describe.contains("\"kind\": \"DesignExtension\""));
```
And similarly `bundle` → `BundleExtension`, `deploy` → `DeployExtension`. Preserve the rest of each test verbatim.

- [ ] **Step 3: Run tests**

Run: `cargo test -p greentic-ext-cli --test cli_new 2>&1 | tail -15`
Expected: all existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-cli/tests/cli_new.rs
git commit -m "test(ext-cli): cli_new asserts schema-compliant kind strings"
```

---

## Task 12: Add schema-conformance test

**Files:**
- Modify: `crates/greentic-ext-cli/tests/cli_new.rs`

A new test validates every scaffolded `describe.json` against `describe-v1.json` so future template drift is caught automatically.

- [ ] **Step 1: Append the test**

Append to the bottom of `crates/greentic-ext-cli/tests/cli_new.rs`:

```rust
#[test]
fn scaffolded_describe_json_validates_against_schema() {
    let schema_path = {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p.push("greentic-extension-sdk-contract/schemas/describe-v1.json");
        p
    };
    let schema_bytes = std::fs::read(&schema_path)
        .unwrap_or_else(|e| panic!("read schema at {}: {e}", schema_path.display()));
    let schema: serde_json::Value = serde_json::from_slice(&schema_bytes).unwrap();
    let compiled = jsonschema::validator_for(&schema).expect("compile schema");

    for (kind_flag, scaffold_name) in [
        ("design", "design-demo"),
        ("bundle", "bundle-demo"),
        ("deploy", "deploy-demo"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join(scaffold_name);
        let (ok, stdout, stderr) = run(Command::new(gtdx_bin())
            .arg("new")
            .arg(scaffold_name)
            .arg("--kind")
            .arg(kind_flag)
            .arg("--dir")
            .arg(&proj)
            .arg("-y")
            .arg("--no-git"));
        assert!(ok, "gtdx new --kind {kind_flag} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

        let describe_bytes = std::fs::read(proj.join("describe.json")).unwrap();
        let describe: serde_json::Value = serde_json::from_slice(&describe_bytes).unwrap();
        if let Err(errors) = compiled.validate(&describe) {
            let details: Vec<String> = errors.map(|e| format!("- {e}")).collect();
            panic!(
                "describe.json for kind={kind_flag} failed schema validation:\n{}",
                details.join("\n")
            );
        }
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p greentic-ext-cli --test cli_new -- scaffolded_describe_json_validates_against_schema 2>&1 | tail -15`
Expected: 1 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/tests/cli_new.rs
git commit -m "test(ext-cli): verify every scaffolded describe.json validates against describe-v1 schema"
```

---

## Task 13: Author-run smoke — cargo component build all 3 kinds

Controller runs this manually.

- [ ] **Step 1: Build the current gtdx**

```bash
cargo build -p greentic-ext-cli --quiet
```

- [ ] **Step 2: Scaffold + build each kind**

```bash
TMP=$(mktemp -d)
for kind in design bundle deploy; do
  ./target/debug/gtdx new "${kind}-demo" \
    --kind "$kind" \
    --dir "$TMP/$kind" \
    --author tester \
    -y --no-git

  (cd "$TMP/$kind" && cargo component build --quiet 2>&1 | tail -15)
  ls "$TMP/$kind/target/wasm32-wasip2/debug/"*.wasm
done
```

Expected: each invocation ends with a `.wasm` file on disk and no build errors.

- [ ] **Step 3: gtdx dev --once on design**

```bash
GREENTIC_HOME="$TMP/home" ./target/debug/gtdx dev \
  --once \
  --manifest "$TMP/design/Cargo.toml"
ls "$TMP/home/extensions/design/"
```

Expected: `InstallOk` event in stdout + `extensions/design/<id>-<version>/` dir with describe.json + extension.wasm.

- [ ] **Step 4: No commit**

Record the observed output in the PR description.

---

## Task 14: Final green gate + push

Controller runs this directly.

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Expected: no output.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -20`
Expected: exit 0.

- [ ] **Step 3: Full test**

Run: `cargo test --workspace --all-targets 2>&1 | tail -20`
Expected: all green.

- [ ] **Step 4: Commit any stragglers**

```bash
if ! git diff --quiet; then
  git add -A
  git commit -m "style: cargo fmt post-track-a-fix"
fi
```

- [ ] **Step 5: Push**

```bash
git push -u origin feat/track-a-scaffold-schema-fix
```

Expected: push succeeds. PR URL printed.

---

## Acceptance

1. `gtdx new demo` (and `--kind bundle` / `--kind deploy`) produces a project that `cargo component build --target wasm32-wasip2` builds without errors (Task 13).
2. Every scaffolded `describe.json` validates against `describe-v1.json` (Task 12).
3. `gtdx dev --once` on a scaffolded design extension installs into `~/.greentic/extensions/design/<id>-<version>/` (Task 13 Step 3).
4. Workspace `cargo fmt` + `cargo clippy -- -D warnings` + full `cargo test` green (Task 14).
5. No regressions in Track A tests (Task 11 keeps them green with updated assertions).

---

## Self-Review

**1. Coverage:** Track B PR #9 acknowledged two Track A blockers (schema mismatch + WIT import mismatch). Blocker #1 → Tasks 2/3/4 + schema test (Task 12). Blocker #2 → Tasks 5/6/7 (WIT) + 8/9/10 (lib.rs uses real interfaces). ✓

**2. Placeholder scan:** Every rust/json/wit snippet is complete. The inline `// TODO:` markers live INSIDE generated user code — that's intentional scaffolding (Task A already used this pattern). ✓

**3. Type consistency:** `ExtensionKind` enum variants (`Design`/`Bundle`/`Deploy`) serialize to `"DesignExtension"`/`"BundleExtension"`/`"DeployExtension"` per `crates/greentic-extension-sdk-contract/src/kind.rs`. Scaffolded describes use the serialized form. `types::Kind` inside wit-bindgen output mirrors the WIT enum variants. Consistency holds. ✓

**4. Risk notes:**
- `wit-bindgen 0.35` in Cargo.toml.tmpl is old but matches Track A as shipped; do not touch unless Task 13 build fails.
- Designer PromptFragment field names are `section`, `content-markdown`, `priority` — wit-bindgen renames `content-markdown` to `content_markdown` in the Rust surface. Not constructed in scaffold bodies (list empty), so no field mismatch risk.
- Deploy `rollback` returns `Result<_, extension-error>`; wit-bindgen generates `Result<(), types::ExtensionError>` — scaffold already uses `()` return. ✓
