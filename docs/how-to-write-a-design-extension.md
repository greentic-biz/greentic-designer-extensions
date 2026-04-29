# How to Write a Design Extension

This tutorial walks you through building a `DesignExtension` from scratch
using `greentic.adaptive-cards` as the running example. By the end you will
have a signed `.gtxpack` installed and visible in the designer.

The canonical source for everything shown here lives in
`reference-extensions/adaptive-cards/`.

---

## Prerequisites

- **Rust 1.94 or later** (`rustup update stable`)
- **`cargo-component`** — the WIT-aware build tool for WASM components:
  ```
  cargo install cargo-component --locked
  ```
- **`wasm32-wasip2` target:**
  ```
  rustup target add wasm32-wasip2
  ```
- **`gtdx`** — the Greentic Extensions CLI:
  ```
  cargo install --path crates/greentic-ext-cli --locked
  ```

---

## Step 1 — Create the crate

```
cargo new --lib my-extension
cd my-extension
```

Open `Cargo.toml` and set the crate type to expose a C-ABI entrypoint (for
the WASM component) while also providing an `rlib` for tests:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

---

## Step 2 — Dependencies in `Cargo.toml`

```toml
[package]
name    = "my-extension"
version = "0.1.0"
edition = "2024"
license = "MIT"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wit-bindgen    = "0.35"
wit-bindgen-rt = "0.35"
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"

# Tell cargo-component which WIT world to bind.
[package.metadata.component]
package = "myco:my-extension"

[package.metadata.component.target]
path  = "wit"
world = "design-extension"

# Paths to the shared WIT packages from the extensions repo.
[package.metadata.component.target.dependencies]
"greentic:extension-base"   = { path = "../path/to/wit/extension-base.wit" }
"greentic:extension-host"   = { path = "../path/to/wit/extension-host.wit" }
"greentic:extension-design" = { path = "../path/to/wit/extension-design.wit" }
```

Adjust the paths to `wit/*.wit` to match your directory layout relative to
the extension crate.

---

## Step 3 — WIT world in `wit/world.wit`

Create `wit/world.wit` declaring what the component imports from the host
and exports to the host:

```wit
package myco:my-extension;

world design-extension {
  // Host services the extension may call.
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;
  import greentic:extension-host/secrets@0.1.0;
  import greentic:extension-host/broker@0.1.0;
  import greentic:extension-host/http@0.1.0;

  // Interfaces the extension must implement.
  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export greentic:extension-design/tools@0.1.0;
  export greentic:extension-design/validation@0.1.0;
  export greentic:extension-design/prompting@0.1.0;
  export greentic:extension-design/knowledge@0.1.0;
}
```

This is identical to the AC extension's `wit/world.wit`. Only the package
name differs.

---

## Step 4 — `describe.json`

Create `describe.json` in the crate root:

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "myco.my-extension",
    "name": "My Extension",
    "version": "0.1.0",
    "summary": "Teaches the designer about my content type",
    "author": { "name": "My Name", "email": "me@example.com" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "myco:my-content/validate", "version": "0.1.0" }
    ],
    "required": []
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 64,
    "permissions": {
      "network": [],
      "secrets": [],
      "callExtensionKinds": []
    }
  },
  "contributions": {
    "schemas": ["schemas/my-content.json"],
    "prompts": ["prompts/rules.md"],
    "knowledge": [],
    "tools": [
      {
        "name": "validate_my_content",
        "export": "greentic:extension-design/validation.validate-content"
      },
      {
        "name": "analyze_my_content",
        "export": "greentic:extension-design/tools.invoke-tool"
      }
    ]
  }
}
```

Adjust `metadata.id`, capabilities, and contributions to match your
extension. The `component` path (`extension.wasm`) is where the WASM binary
will land inside the `.gtxpack`.

---

## Step 4a — (Optional) Node-providing design extensions

If your extension teaches the designer a new node type that is *executed at runtime* by a WASM component, you can embed the runtime `.gtpack` inside your `.gtxpack` so both install atomically.

Requirements:
- `kind` is still `DesignExtension`.
- `contributions.nodeTypes` must be a non-empty array describing the palette entry (type_id, label, category, color, complexity, config_schema, output_ports).
- `runtime.gtpack` must be set to the embedded pack (file path + sha256 + pack_id + component_version).

Skeleton:

```json
{
  "kind": "DesignExtension",
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 32,
    "permissions": { "network": [], "secrets": ["*"], "callExtensionKinds": [] },
    "gtpack": {
      "file": "runtime/my-component.gtpack",
      "sha256": "<computed at build>",
      "pack_id": "myco.my-node",
      "component_version": "0.6.0"
    }
  },
  "contributions": {
    "nodeTypes": [{
      "type_id": "my-node",
      "label": "My Node",
      "category": "integration",
      "color": "#6366f1",
      "complexity": "simple",
      "config_schema": "<stringified JSON Schema>",
      "output_ports": [{"name": "default", "label": "Next"}]
    }]
  }
}
```

`gtdx install` extracts the embedded `.gtpack` to the runner pack directory; the runner picks it up via its existing pack-index poll. Your extension's WASM handles design-time tools (validate, test, etc.); the embedded runtime handles flow execution.

---

## Step 5 — `src/lib.rs` — Implement the WIT exports

Run `cargo component build` once to generate the WIT bindings into
`src/bindings.rs`, then implement the traits:

```rust
#![allow(clippy::used_underscore_items)]

#[allow(warnings)]
mod bindings;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_design::{knowledge, prompting, tools, validation};
use bindings::greentic::extension_base::types;

const RULES_PROMPT: &str = include_str!("../prompts/rules.md");

struct Component;

// ---- base::manifest ----

impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "myco.my-extension".into(),
            version: "0.1.0".into(),
            kind: types::Kind::Design,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "myco:my-content/validate".into(),
            version: "0.1.0".into(),
        }]
    }
    fn get_required() -> Vec<types::CapabilityRef> {
        vec![]
    }
}

// ---- base::lifecycle ----

impl lifecycle::Guest for Component {
    fn init(_config_json: String) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn shutdown() {}
}

// ---- design::tools ----

impl tools::Guest for Component {
    fn list_tools() -> Vec<tools::ToolDefinition> {
        vec![tools::ToolDefinition {
            name: "analyze_my_content".into(),
            description: "Analyze my content type and return metadata".into(),
            input_schema_json: r#"{"type":"object","properties":{"content":{"type":"object"}},"required":["content"]}"#.into(),
            output_schema_json: None,
        }]
    }
    fn invoke_tool(name: String, args_json: String)
        -> Result<String, types::ExtensionError>
    {
        let args: serde_json::Value = serde_json::from_str(&args_json)
            .map_err(|e| types::ExtensionError::InvalidInput(e.to_string()))?;
        match name.as_str() {
            "analyze_my_content" => {
                let content = &args["content"];
                Ok(serde_json::json!({ "field_count": content.as_object().map_or(0, |o| o.len()) }).to_string())
            }
            other => Err(types::ExtensionError::InvalidInput(
                format!("unknown tool: {other}")
            )),
        }
    }
}

// ---- design::validation ----

impl validation::Guest for Component {
    fn validate_content(content_type: String, content_json: String)
        -> validation::ValidateResult
    {
        if content_type != "my-content" {
            return validation::ValidateResult {
                valid: false,
                diagnostics: vec![types::Diagnostic {
                    severity: types::Severity::Error,
                    code: "unsupported-content-type".into(),
                    message: format!("expected 'my-content', got '{content_type}'"),
                    path: None,
                }],
            };
        }
        match serde_json::from_str::<serde_json::Value>(&content_json) {
            Err(e) => validation::ValidateResult {
                valid: false,
                diagnostics: vec![types::Diagnostic {
                    severity: types::Severity::Error,
                    code: "json-parse".into(),
                    message: e.to_string(),
                    path: None,
                }],
            },
            Ok(_v) => {
                // Add your validation logic here.
                validation::ValidateResult { valid: true, diagnostics: vec![] }
            }
        }
    }
}

// ---- design::prompting ----

impl prompting::Guest for Component {
    fn system_prompt_fragments() -> Vec<prompting::PromptFragment> {
        vec![prompting::PromptFragment {
            section: "rules".into(),
            content_markdown: RULES_PROMPT.into(),
            priority: 100,
        }]
    }
}

// ---- design::knowledge ----

impl knowledge::Guest for Component {
    fn list_entries(_filter: Option<String>) -> Vec<knowledge::EntrySummary> {
        vec![]
    }
    fn get_entry(id: String) -> Result<knowledge::Entry, types::ExtensionError> {
        Err(types::ExtensionError::InvalidInput(format!("no entry: {id}")))
    }
    fn suggest_entries(_query: String, _limit: u32) -> Vec<knowledge::EntrySummary> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
```

Create `prompts/rules.md` with any rules you want injected into the LLM
system prompt, and `schemas/my-content.json` with a JSON Schema for your
content type.

---

## Step 6 — Build the WASM component

```
cargo component build --release
```

The output WASM binary is at:

```
target/wasm32-wasip2/release/my_extension.wasm
```

---

## Step 7 — Package as `.gtxpack`

A `.gtxpack` is a ZIP archive. The required layout:

```
my-extension-0.1.0.gtxpack
├── describe.json
├── extension.wasm          ← the compiled WASM binary (renamed)
├── schemas/
│   └── my-content.json
└── prompts/
    └── rules.md
```

Build script (`build.sh`):

```bash
#!/usr/bin/env bash
set -euo pipefail

NAME="myco.my-extension"
VERSION="0.1.0"
PACK="${NAME}-${VERSION}.gtxpack"

cargo component build --release

mkdir -p dist
rm -f "dist/${PACK}"

# Assemble staging directory
STAGE=$(mktemp -d)
cp target/wasm32-wasip2/release/my_extension.wasm "${STAGE}/extension.wasm"
cp describe.json "${STAGE}/"
cp -r schemas/ "${STAGE}/"
cp -r prompts/ "${STAGE}/"

# Create the zip
(cd "${STAGE}" && zip -r - .) > "dist/${PACK}"
rm -rf "${STAGE}"
echo "Built dist/${PACK}"
```

```
chmod +x build.sh
./build.sh
```

---

## Step 8 — Validate locally

```
# Validate describe.json in the source directory.
gtdx validate ./

# Expected output:
✓ ./describe.json valid
```

You can also unpack the `.gtxpack` to a temp directory and validate there:

```
unzip dist/myco.my-extension-0.1.0.gtxpack -d /tmp/ext-check
gtdx validate /tmp/ext-check/
```

---

## Step 9 — Install locally for testing

```
gtdx install ./dist/myco.my-extension-0.1.0.gtxpack --trust loose
```

`--trust loose` accepts unsigned extensions. This is appropriate for local
development. For production installs use `normal` or `strict`.

Verify installation:

```
$ gtdx list
[design]
  myco.my-extension@0.1.0  Teaches the designer about my content type
```

Run diagnostics:

```
$ gtdx doctor
✓ ~/.greentic/extensions/design/myco.my-extension-0.1.0/describe.json
1 total, 0 bad
```

---

## Step 10 — Publish to the Greentic Store

Log in first:

```
gtdx login
```

Then publish:

```
gtdx publish ./dist/myco.my-extension-0.1.0.gtxpack
```

`gtdx publish` signs the artifact with your stored key and uploads it to
the default registry. On success:

```
✓ published myco.my-extension@0.1.0
```

Other users can then install it with:

```
gtdx install myco.my-extension --version 0.1.0
```

---

## What to do next

- Add more tools in `invoke_tool` with real logic.
- Populate the knowledge base: implement `list_entries`, `get_entry`, and
  `suggest_entries` with meaningful data.
- Add i18n support: create `i18n/en.json` and reference it in
  `contributions.i18n`.
- Add integration tests using `greentic-extension-sdk-testing` (see
  `reference-extensions/adaptive-cards/tests/`).

For publishing with a permanent key and countersigning, see
[permissions-and-trust.md](./permissions-and-trust.md).
