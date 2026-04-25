# Track A — Scaffold (`gtdx new --kind wasm-component`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `wasm-component` variant to `gtdx new` so authors can scaffold a node-providing WASM extension in one command.

**Architecture:** Extend the existing `Kind` enum and template machinery in `greentic-ext-cli/src/scaffold/`. Add a new `templates/wasm-component/` directory mirroring the structure of `templates/design/` plus extra files (workspace Cargo.toml, runtime/ subdir, rust-toolchain.toml). No `describe.json` schema change.

**Tech Stack:** Rust 1.94, edition 2024, clap, include_dir.

**Spec:** `docs/superpowers/specs/2026-04-25-designer-commercialization-backend-design.md` (Track A section)

**Branch / Worktree:**
```
git worktree add ~/works/greentic/gde-scaffold -b feat/scaffold-wasm-component main
```
PR target: `main`.

---

## File Structure

### Create

- `crates/greentic-ext-cli/templates/wasm-component/Cargo.toml.tmpl` — workspace manifest
- `crates/greentic-ext-cli/templates/wasm-component/extension/Cargo.toml.tmpl` — extension crate manifest
- `crates/greentic-ext-cli/templates/wasm-component/extension/src/lib.rs.tmpl` — `invoke-tool` impl with stubs
- `crates/greentic-ext-cli/templates/wasm-component/extension/wit/world.wit.tmpl` — WIT world
- `crates/greentic-ext-cli/templates/wasm-component/runtime/README.md.tmpl` — drop-pack instructions
- `crates/greentic-ext-cli/templates/wasm-component/describe.json.tmpl` — describe with `nodeTypes` + `runtime.gtpack`
- `crates/greentic-ext-cli/templates/wasm-component/README.md.tmpl` — quickstart
- `crates/greentic-ext-cli/templates/wasm-component/gitignore.tmpl` — `.gitignore`
- `crates/greentic-ext-cli/templates/wasm-component/rust-toolchain.toml.tmpl`
- `docs/wasm-component-tutorial.md` — author-facing tutorial

### Modify

- `crates/greentic-ext-cli/src/scaffold/mod.rs` — add `WasmComponent` variant to `Kind` enum
- `crates/greentic-ext-cli/src/commands/new.rs` — add `--node-type-id` and `--label` CLI args; wire context for new placeholders
- `crates/greentic-ext-cli/src/scaffold/template.rs` — confirm dir-rendering covers nested `extension/` and `runtime/` paths (likely already does; verify)
- `crates/greentic-ext-cli/src/scaffold/embedded.rs` — add `wasm-component` to `files_for_kind` if it includes WIT
- `crates/greentic-ext-cli/tests/cli_new.rs` — add snapshot + smoke test for `--kind wasm-component`
- `docs/gtdx-cli.md` — document new `--kind` value and new args

---

## Task 1: Add `WasmComponent` variant to `Kind` enum

**Files:**
- Modify: `crates/greentic-ext-cli/src/scaffold/mod.rs`

- [ ] **Step 1: Write failing test**

Add to `crates/greentic-ext-cli/src/scaffold/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_component_kind_str() {
        assert_eq!(Kind::WasmComponent.as_str(), "wasm-component");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-ext-cli scaffold::tests::wasm_component_kind_str`
Expected: FAIL, `WasmComponent` variant not found.

- [ ] **Step 3: Add the variant**

Modify `crates/greentic-ext-cli/src/scaffold/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Design,
    Bundle,
    Deploy,
    Provider,
    WasmComponent,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Design => "design",
            Kind::Bundle => "bundle",
            Kind::Deploy => "deploy",
            Kind::Provider => "provider",
            Kind::WasmComponent => "wasm-component",
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p greentic-ext-cli scaffold::tests::wasm_component_kind_str`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/src/scaffold/mod.rs
git commit -m "feat(cli): add WasmComponent variant to Kind enum"
```

---

## Task 2: Add `--node-type-id` and `--label` CLI args

**Files:**
- Modify: `crates/greentic-ext-cli/src/commands/new.rs`

- [ ] **Step 1: Write failing test**

Add to `crates/greentic-ext-cli/tests/cli_new.rs`:

```rust
#[test]
fn new_wasm_component_accepts_node_type_id_and_label() {
    let tmp = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args([
            "new",
            "--kind", "wasm-component",
            "--name", "greentic.test-tool",
            "--node-type-id", "test-tool",
            "--label", "Test Tool",
        ])
        .current_dir(tmp.path())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(tmp.path().join("greentic.test-tool/describe.json").exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p greentic-ext-cli --test cli_new new_wasm_component_accepts_node_type_id_and_label`
Expected: FAIL — unknown args, or templates dir missing for wasm-component.

- [ ] **Step 3: Add CLI args + context wiring**

In `crates/greentic-ext-cli/src/commands/new.rs`, add to `NewArgs`:

```rust
/// Node type ID (defaults to derived suffix of --name).
#[arg(long)]
pub node_type_id: Option<String>,

/// Display label for the node (defaults to humanized --name).
#[arg(long)]
pub label: Option<String>,
```

In the function that builds `Context` (around line 112):

```rust
let derived_id = args.name.split('.').next_back().unwrap_or(&args.name).to_string();
let node_type_id = args.node_type_id.clone().unwrap_or_else(|| derived_id.clone());
let label = args.label.clone().unwrap_or_else(|| {
    derived_id.replace('-', " ").split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
});
ctx.set("node_type_id", &node_type_id);
ctx.set("label", &label);
```

- [ ] **Step 4: Run test to verify the args parse (template files come next task)**

Run: `cargo test -p greentic-ext-cli --test cli_new new_wasm_component_accepts_node_type_id_and_label`
Expected: still FAIL but on missing template files (not arg parsing). If FAIL is on arg parsing, fix the clap setup.

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/src/commands/new.rs crates/greentic-ext-cli/tests/cli_new.rs
git commit -m "feat(cli): accept --node-type-id and --label for new --kind wasm-component"
```

---

## Task 3: Workspace `Cargo.toml` template

**Files:**
- Create: `crates/greentic-ext-cli/templates/wasm-component/Cargo.toml.tmpl`

- [ ] **Step 1: Write the template**

Create `crates/greentic-ext-cli/templates/wasm-component/Cargo.toml.tmpl`:

```toml
[workspace]
resolver = "2"
members = ["extension"]

[workspace.package]
edition = "2024"
license = "Apache-2.0"
authors = ["{{ author }}"]
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/wasm-component/Cargo.toml.tmpl
git commit -m "feat(cli): add wasm-component workspace Cargo.toml template"
```

---

## Task 4: Extension crate templates (`extension/`)

**Files:**
- Create: `crates/greentic-ext-cli/templates/wasm-component/extension/Cargo.toml.tmpl`
- Create: `crates/greentic-ext-cli/templates/wasm-component/extension/src/lib.rs.tmpl`
- Create: `crates/greentic-ext-cli/templates/wasm-component/extension/wit/world.wit.tmpl`

- [ ] **Step 1: Add `extension/Cargo.toml.tmpl`**

```toml
[package]
name = "{{ name_snake }}-extension"
version = "0.1.0"
edition.workspace = true
license.workspace = true
authors.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.36"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[package.metadata.component]
package = "{{ name_kebab }}:extension"
target = { path = "wit", world = "extension" }
```

- [ ] **Step 2: Add `extension/src/lib.rs.tmpl`**

```rust
//! Design-time WASM extension for {{ label }}.

wit_bindgen::generate!({
    world: "extension",
    path: "wit",
});

struct Component;

impl exports::greentic::extension_design::tools::Guest for Component {
    fn invoke_tool(name: String, args_json: String) -> Result<String, String> {
        match name.as_str() {
            "validate_config" => Ok(r#"{"valid":true}"#.to_string()),
            "describe_node" => Ok(serde_json::json!({
                "type_id": "{{ node_type_id }}",
                "label": "{{ label }}",
                "args_received": args_json,
            }).to_string()),
            other => Err(format!("unknown tool: {}", other)),
        }
    }
}

export!(Component);
```

- [ ] **Step 3: Add `extension/wit/world.wit.tmpl`**

```wit
package {{ name_kebab }}:extension;

world extension {
  export greentic:extension-design/tools@0.1.0;
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/greentic-ext-cli/templates/wasm-component/extension/
git commit -m "feat(cli): add wasm-component extension crate templates"
```

---

## Task 5: `describe.json.tmpl`

**Files:**
- Create: `crates/greentic-ext-cli/templates/wasm-component/describe.json.tmpl`

- [ ] **Step 1: Write template**

```json
{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "{{ name }}",
    "name": "{{ name_kebab }}",
    "version": "0.1.0",
    "author": "{{ author }}"
  },
  "engine": {
    "greenticDesigner": "^0.x",
    "extRuntime": "^0.10"
  },
  "contributions": {
    "nodeTypes": [
      {
        "type_id": "{{ node_type_id }}",
        "label": "{{ label }}",
        "category": "tools",
        "icon": "puzzle",
        "color": "#0d9488",
        "complexity": "simple",
        "config_schema": "{}",
        "output_ports": [
          { "name": "success", "label": "Success" },
          { "name": "error", "label": "Error" }
        ]
      }
    ]
  },
  "runtime": {
    "component": "extension.wasm",
    "gtpack": {
      "file": "runtime/REPLACE_ME.gtpack",
      "sha256": "REPLACE_AT_BUILD",
      "pack_id": "{{ name }}",
      "component_version": "0.1.0"
    }
  },
  "permissions": {
    "network": [],
    "secrets": [],
    "callExtensionKinds": []
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/greentic-ext-cli/templates/wasm-component/describe.json.tmpl
git commit -m "feat(cli): add wasm-component describe.json template"
```

---

## Task 6: `runtime/` subdir + top-level files

**Files:**
- Create: `crates/greentic-ext-cli/templates/wasm-component/runtime/README.md.tmpl`
- Create: `crates/greentic-ext-cli/templates/wasm-component/README.md.tmpl`
- Create: `crates/greentic-ext-cli/templates/wasm-component/gitignore.tmpl`
- Create: `crates/greentic-ext-cli/templates/wasm-component/rust-toolchain.toml.tmpl`

- [ ] **Step 1: `runtime/README.md.tmpl`**

```markdown
# Runtime artifact

Drop your pre-built `.gtpack` here, or run `gtdx dev` to have it built and
signed locally.

The expected filename is referenced from `../describe.json` under
`runtime.gtpack.file`. Update that field if you choose a different name.
```

- [ ] **Step 2: `README.md.tmpl`**

```markdown
# {{ label }}

A Greentic Designer Extension that wraps a WASM component as a node.

## Quickstart

1. Edit `describe.json` — fill in `category`, `icon`, `config_schema`, and
   optional `permissions` for your node.
2. Drop your pre-built `.gtpack` into `runtime/` and update the
   `runtime.gtpack.file` path in `describe.json`.
3. Run `gtdx dev` to build the design-time `extension.wasm`, sign, install
   locally, and watch for changes.
4. Open the designer — your node should appear in the palette.
5. When ready, `gtdx publish`.
```

- [ ] **Step 3: `gitignore.tmpl`**

```
target/
*.lock
*.wasm
.greentic/
```

- [ ] **Step 4: `rust-toolchain.toml.tmpl`**

```toml
[toolchain]
channel = "1.94.0"
targets = ["wasm32-wasip2"]
```

- [ ] **Step 5: Commit**

```bash
git add crates/greentic-ext-cli/templates/wasm-component/
git commit -m "feat(cli): add wasm-component runtime/ + top-level templates"
```

---

## Task 7: Verify scaffold renders end-to-end

**Files:**
- Modify: `crates/greentic-ext-cli/tests/cli_new.rs`

- [ ] **Step 1: Run the existing test from Task 2**

Run: `cargo test -p greentic-ext-cli --test cli_new new_wasm_component_accepts_node_type_id_and_label -- --nocapture`
Expected: PASS — scaffold succeeds, `describe.json` exists.

- [ ] **Step 2: If it fails, check `template.rs` covers nested dirs**

If failure mentions missing files in `extension/` or `runtime/`, verify `crates/greentic-ext-cli/src/scaffold/template.rs` walks subdirectories. If not, extend `load_templates_kind` to recurse. Show the diff in the actual file.

- [ ] **Step 3: Commit any template machinery fix**

```bash
git add crates/greentic-ext-cli/src/scaffold/template.rs
git commit -m "fix(cli): recurse into subdirs when loading wasm-component templates"
```

(Skip if no fix was needed.)

---

## Task 8: Snapshot test of full scaffold tree

**Files:**
- Modify: `crates/greentic-ext-cli/tests/cli_new.rs`

- [ ] **Step 1: Add snapshot test**

```rust
#[test]
fn new_wasm_component_produces_expected_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args([
            "new",
            "--kind", "wasm-component",
            "--name", "greentic.snap-test",
            "--author", "Test Author",
            "--node-type-id", "snap",
            "--label", "Snap",
        ])
        .current_dir(tmp.path())
        .status()
        .unwrap();
    assert!(status.success());

    let root = tmp.path().join("greentic.snap-test");
    let expected_files = [
        "Cargo.toml",
        "describe.json",
        "README.md",
        ".gitignore",
        "rust-toolchain.toml",
        "extension/Cargo.toml",
        "extension/src/lib.rs",
        "extension/wit/world.wit",
        "runtime/README.md",
    ];
    for f in expected_files {
        assert!(root.join(f).exists(), "missing: {}", f);
    }

    let describe: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("describe.json")).unwrap()).unwrap();
    assert_eq!(describe["metadata"]["id"], "greentic.snap-test");
    assert_eq!(describe["metadata"]["author"], "Test Author");
    assert_eq!(describe["contributions"]["nodeTypes"][0]["type_id"], "snap");
    assert_eq!(describe["contributions"]["nodeTypes"][0]["label"], "Snap");
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p greentic-ext-cli --test cli_new new_wasm_component_produces_expected_tree`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/tests/cli_new.rs
git commit -m "test(cli): add snapshot test for wasm-component scaffold tree"
```

---

## Task 9: Smoke test — scaffolded crate compiles

**Files:**
- Modify: `crates/greentic-ext-cli/tests/cli_new.rs`

- [ ] **Step 1: Add smoke test (gated to avoid running in CI by default)**

```rust
#[test]
#[ignore = "requires wasm32-wasip2 toolchain; run with `cargo test -- --ignored`"]
fn new_wasm_component_compiles_to_wasi_p2() {
    let tmp = tempfile::tempdir().unwrap();
    std::process::Command::new(env!("CARGO_BIN_EXE_gtdx"))
        .args([
            "new",
            "--kind", "wasm-component",
            "--name", "greentic.compile-test",
        ])
        .current_dir(tmp.path())
        .status()
        .unwrap();

    let crate_root = tmp.path().join("greentic.compile-test");
    let status = std::process::Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--manifest-path", "extension/Cargo.toml"])
        .current_dir(&crate_root)
        .status()
        .unwrap();
    assert!(status.success(), "scaffolded extension crate failed to compile");
}
```

- [ ] **Step 2: Run with `--ignored` locally**

Run: `cargo test -p greentic-ext-cli --test cli_new -- --ignored new_wasm_component_compiles_to_wasi_p2`
Expected: PASS (requires `wasm32-wasip2` target installed).

- [ ] **Step 3: Commit**

```bash
git add crates/greentic-ext-cli/tests/cli_new.rs
git commit -m "test(cli): add smoke test that scaffolded wasm-component compiles"
```

---

## Task 10: Documentation — author tutorial

**Files:**
- Create: `docs/wasm-component-tutorial.md`
- Modify: `docs/gtdx-cli.md`

- [ ] **Step 1: Write tutorial**

Create `docs/wasm-component-tutorial.md`:

```markdown
# Authoring a WASM-Component Extension

This tutorial walks through scaffolding a node-providing extension that
wraps a pre-built WASM component and surfaces it as a node in the
greentic-designer palette.

## Prerequisites

- Rust 1.94 with `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `gtdx` CLI installed: `cargo install greentic-ext-cli`
- A pre-built `.gtpack` runtime artifact (build separately via
  `greentic-pack`)

## Scaffold

```bash
gtdx new --kind wasm-component \
  --name greentic.my-tool \
  --author "Your Name" \
  --node-type-id my-tool \
  --label "My Tool"
cd greentic.my-tool
```

This creates a workspace with two parts: a design-time `extension/` crate
that compiles to `extension.wasm`, and a `runtime/` directory where you
drop your pre-built `.gtpack`.

## Configure

Edit `describe.json`:

- `contributions.nodeTypes[0].category` — palette category (e.g. `tools`,
  `ai`, `messaging`)
- `contributions.nodeTypes[0].icon` — icon name (e.g. `puzzle`, `robot`)
- `contributions.nodeTypes[0].config_schema` — stringified JSON Schema for
  node config
- `permissions.network` — HTTPS allowlist if your runtime calls external
  APIs
- `permissions.secrets` — secret URI patterns your runtime needs

## Build and install locally

```bash
cp /path/to/your-component.gtpack runtime/
# Update runtime.gtpack.file in describe.json to match the filename
gtdx dev
```

`gtdx dev` builds `extension.wasm`, signs it, installs to
`~/.greentic/extensions/design/`, and watches for changes.

## Test in designer

Open `greentic-designer`. The node should appear in your chosen
category. Drag it onto the canvas; the config form is generated from
your `config_schema`.

## Publish

```bash
gtdx publish
```

This signs and uploads the `.gtxpack` to your configured registry.
```

- [ ] **Step 2: Update `gtdx-cli.md`**

Add to the `--kind` reference (find existing section listing `design | bundle | deploy | provider`):

```markdown
- `wasm-component` — node-providing extension that wraps a pre-built
  `.gtpack` runtime as a designer canvas node. Adds two extra args:
  `--node-type-id` (defaults to suffix of `--name`) and `--label`
  (defaults to humanized `--name`). See
  [WASM-component tutorial](./wasm-component-tutorial.md).
```

- [ ] **Step 3: Commit**

```bash
git add docs/wasm-component-tutorial.md docs/gtdx-cli.md
git commit -m "docs(cli): add wasm-component tutorial + --kind reference"
```

---

## Task 11: Final CI check

- [ ] **Step 1: Run local CI**

Run: `ci/local_check.sh`
Expected: PASS (fmt + clippy + tests). Smoke test (Task 9) is `#[ignore]` and excluded.

- [ ] **Step 2: If failures, fix and add a follow-up commit**

Show diffs in actual files for any fixes.

- [ ] **Step 3: Push branch**

```bash
git push -u origin feat/scaffold-wasm-component
```

- [ ] **Step 4: Open PR**

```bash
gh pr create --title "feat(cli): scaffold wasm-component extension" \
  --base main \
  --body "$(cat <<'EOF'
## Summary

- Add `wasm-component` variant to `gtdx new --kind`
- New args: `--node-type-id`, `--label`
- New template tree: workspace + extension crate + runtime/ subdir + describe.json + tutorial
- Snapshot + smoke tests

## Test plan

- [x] Snapshot test verifies tree + describe.json fields
- [x] Smoke test compiles scaffolded crate to wasm32-wasip2 (manual `--ignored`)
- [x] `ci/local_check.sh` passes

Spec: `docs/superpowers/specs/2026-04-25-designer-commercialization-backend-design.md` (Track A)
EOF
)"
```

---

## Self-review checklist

- [x] Every task has exact file paths
- [x] Every step has runnable commands or full code blocks
- [x] No "TBD" / "implement later" / "add error handling"
- [x] Type names consistent (`Kind::WasmComponent`, `--kind wasm-component`)
- [x] Spec coverage:
  - CLI invocation form ✓ (Task 2)
  - Generated structure ✓ (Tasks 3–6)
  - Interpolated `describe.json` ✓ (Task 5)
  - Default `extension/src/lib.rs` stubs ✓ (Task 4)
  - Author workflow documented ✓ (Task 10)
  - No `describe.json` schema bump ✓ (uses existing v1 fields)
