# How to Write a Bundle Extension

A `BundleExtension` teaches the Greentic Designer how to package its output
(flows, cards, assets) into a deployable Application Pack. It appears as a
set of selectable **recipes** in the designer's "Next" wizard step.

This tutorial builds a minimal stub bundle extension. Full reference
implementations (hosted-webchat, openshift, multi-channel) are planned for
a future cycle.

The design extension tutorial
([how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md))
covers prerequisites and the common steps (crate setup, WIT bindings, build,
packaging). This document focuses on the parts specific to bundle extensions.

---

## Prerequisites

Same as the design extension tutorial:

- Rust 1.94+
- `cargo-component`
- `wasm32-wasip2` target
- `gtdx`

---

## Step 1 — Crate setup

```
cargo new --lib my-bundle-ext
cd my-bundle-ext
```

`Cargo.toml`:

```toml
[package]
name    = "my-bundle-ext"
version = "0.1.0"
edition = "2024"
license = "MIT"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wit-bindgen    = "0.35"
wit-bindgen-rt = "0.35"
serde_json     = "1"

[package.metadata.component]
package = "myco:my-bundle-ext"

[package.metadata.component.target]
path  = "wit"
world = "bundle-extension"

[package.metadata.component.target.dependencies]
"greentic:extension-base"   = { path = "../path/to/wit/extension-base.wit" }
"greentic:extension-host"   = { path = "../path/to/wit/extension-host.wit" }
"greentic:extension-bundle" = { path = "../path/to/wit/extension-bundle.wit" }
```

---

## Step 2 — `wit/world.wit`

```wit
package myco:my-bundle-ext;

world bundle-extension {
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;
  import greentic:extension-host/broker@0.1.0;

  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export greentic:extension-bundle/recipes@0.1.0;
  export greentic:extension-bundle/bundling@0.1.0;
}
```

Note: bundle extensions do not import `secrets` or `http` by default. Add
them if your bundle process needs to fetch assets or sign artifacts from an
external service.

---

## Step 3 — `describe.json`

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "BundleExtension",
  "metadata": {
    "id": "myco.my-bundle-ext",
    "name": "My Bundle Extension",
    "version": "0.1.0",
    "summary": "Packages designer output into my Application Pack format",
    "author": { "name": "My Name", "email": "me@example.com" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "myco:bundle/my-pack-format", "version": "0.1.0" }
    ],
    "required": []
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 128,
    "permissions": {
      "network": [],
      "secrets": [],
      "callExtensionKinds": []
    }
  },
  "contributions": {
    "recipes": [
      {
        "id": "my-standard-recipe",
        "displayName": "My Standard Pack",
        "configSchema": "schemas/my-pack-config.json",
        "supportedCapabilities": []
      }
    ]
  }
}
```

---

## Step 4 — `src/lib.rs` — Implement the WIT exports

```rust
#![allow(clippy::used_underscore_items)]

#[allow(warnings)]
mod bindings;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_bundle::{bundling, recipes};
use bindings::greentic::extension_base::types;

struct Component;

// ---- base::manifest ----

impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "myco.my-bundle-ext".into(),
            version: "0.1.0".into(),
            kind: types::Kind::Bundle,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "myco:bundle/my-pack-format".into(),
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

// ---- bundle::recipes ----

impl recipes::Guest for Component {
    fn list_recipes() -> Vec<recipes::RecipeSummary> {
        vec![recipes::RecipeSummary {
            id: "my-standard-recipe".into(),
            display_name: "My Standard Pack".into(),
            description: "Packages everything into a single .mypack archive".into(),
            icon_path: None,
        }]
    }

    fn recipe_config_schema(recipe_id: String)
        -> Result<String, types::ExtensionError>
    {
        match recipe_id.as_str() {
            "my-standard-recipe" => Ok(r#"{
                "type": "object",
                "properties": {
                    "pack_name": { "type": "string", "description": "Name of the output pack" }
                },
                "required": ["pack_name"]
            }"#.into()),
            other => Err(types::ExtensionError::InvalidInput(
                format!("unknown recipe: {other}")
            )),
        }
    }

    fn supported_capabilities(recipe_id: String)
        -> Result<Vec<String>, types::ExtensionError>
    {
        match recipe_id.as_str() {
            "my-standard-recipe" => Ok(vec![]),
            other => Err(types::ExtensionError::InvalidInput(
                format!("unknown recipe: {other}")
            )),
        }
    }
}

// ---- bundle::bundling ----

impl bundling::Guest for Component {
    fn validate_config(recipe_id: String, config_json: String)
        -> Vec<types::Diagnostic>
    {
        if recipe_id != "my-standard-recipe" {
            return vec![types::Diagnostic {
                severity: types::Severity::Error,
                code: "unknown-recipe".into(),
                message: format!("unknown recipe: {recipe_id}"),
                path: None,
            }];
        }
        match serde_json::from_str::<serde_json::Value>(&config_json) {
            Err(e) => vec![types::Diagnostic {
                severity: types::Severity::Error,
                code: "json-parse".into(),
                message: e.to_string(),
                path: None,
            }],
            Ok(cfg) => {
                if cfg.get("pack_name").is_none() {
                    vec![types::Diagnostic {
                        severity: types::Severity::Error,
                        code: "missing-pack-name".into(),
                        message: "pack_name is required".into(),
                        path: Some("/pack_name".into()),
                    }]
                } else {
                    vec![]
                }
            }
        }
    }

    fn render(
        recipe_id: String,
        config_json: String,
        _session: bundling::DesignerSession,
    ) -> Result<bundling::BundleArtifact, types::ExtensionError> {
        if recipe_id != "my-standard-recipe" {
            return Err(types::ExtensionError::InvalidInput(
                format!("unknown recipe: {recipe_id}")
            ));
        }
        let cfg: serde_json::Value = serde_json::from_str(&config_json)
            .map_err(|e| types::ExtensionError::InvalidInput(e.to_string()))?;
        let pack_name = cfg["pack_name"]
            .as_str()
            .ok_or_else(|| types::ExtensionError::InvalidInput(
                "pack_name missing or not a string".into()
            ))?;

        // Stub: produce a minimal marker file instead of a real pack.
        let content = format!("pack:{pack_name}\n");
        let bytes: Vec<u8> = content.into_bytes();
        let sha = format!("{:x}", {
            // Simple checksum for stub purposes.
            bytes.iter().fold(0u64, |acc, &b| acc.wrapping_add(b as u64))
        });
        Ok(bundling::BundleArtifact {
            filename: format!("{pack_name}.mypack"),
            bytes,
            sha256: sha,
        })
    }
}

bindings::export!(Component with_types_in bindings);
```

---

## Step 5 — Build, package, and install

Follow the same steps as the design extension tutorial:

```bash
# Build
cargo component build --release

# Package
STAGE=$(mktemp -d)
cp target/wasm32-wasip2/release/my_bundle_ext.wasm "${STAGE}/extension.wasm"
cp describe.json "${STAGE}/"
cp -r schemas/ "${STAGE}/"
(cd "${STAGE}" && zip -r - .) > myco.my-bundle-ext-0.1.0.gtxpack
rm -rf "${STAGE}"

# Validate
gtdx validate ./

# Install
gtdx install ./myco.my-bundle-ext-0.1.0.gtxpack --trust loose

# Verify
gtdx list
```

---

## Notes on Full Bundle Implementations

The stub `render` above creates a placeholder file. A real bundle extension
would:

1. Parse `session.flows_json` to get the designer flow graph.
2. Parse `session.contents_json` for card or content data.
3. Build a `.gtpack` (Application Pack) ZIP archive containing the manifest,
   flow files, WASM components, and assets.
4. Compute a real SHA-256 over the archive bytes.
5. Return the archive bytes in `BundleArtifact`.

The Application Pack format is defined by the `greentic-pack` project. See
the greentic repository map in the root `CLAUDE.md` for the full spec.
