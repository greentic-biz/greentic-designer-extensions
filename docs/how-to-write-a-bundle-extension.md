# How to Write a Bundle Extension

A `BundleExtension` turns a designer session into a deployable Application
Pack. It contributes **recipes** — named packaging strategies the operator
picks from — and implements the render that produces the artifact bytes.

Against the v2 describe contract. Field reference:
[describe-json-spec.md](./describe-json-spec.md).

---

## What a bundle extension is responsible for

| Interface | Exports | Purpose |
|---|---|---|
| `extension-bundle/recipes` | `list-recipes`, `recipe-config-schema`, `supported-capabilities` | Advertise what this extension can package and how it is configured |
| `extension-bundle/bundling` | `validate-config`, `render` | Check the operator's config, then produce the `.gtpack` bytes |

`render` is the whole job: it receives a `designer-session` (flows JSON,
contents JSON, assets, and the capability ids the session used) and returns a
`bundle-artifact` — `{ filename, bytes, sha256 }`.

The designer calls this **in-process** through
`ExtensionRuntime::render_bundle`; there is no subprocess. The previous
out-of-band `greentic-bundle ext render` path was retired in ext-runtime
`v0.12.0`.

---

## Prerequisites

- Rust 1.95+, `rustup target add wasm32-wasip2`
- `cargo install --locked cargo-component`
- A `gtdx` matching the current contract — see
  [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md#check-your-gtdx-before-you-start)

---

## Step 1 — Scaffold

```
gtdx new my-bundle --kind bundle --id greentic.my-bundle
```

**On any `gtdx` predating greentic-designer-sdk#105, fix the generated
`wit/world.wit` before your first build.** The
template renders `greentic:extension-host@0.2.0`, but the vendored package is
`@0.1.0`:

```bash
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' wit/world.wit
```

Without it, `cargo component build` and `gtdx dev` fail with
`package 'greentic:extension-host@0.2.0' not found`. Background and the
per-kind table:
[getting-started-scaffolding.md](./getting-started-scaffolding.md#known-issue--a-fresh-scaffold-does-not-build).

Also delete the deprecated `engine` block from `describe.json` and set a real
`metadata.id` — both are `gtdx lint` errors on an untouched scaffold.

---

## Step 2 — The WIT world

The corrected world matches the canonical `wit/extension-bundle.wit`:

```wit
package greentic:my-bundle;

world extension {
  import greentic:extension-base/types@0.2.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;
  import greentic:extension-host/broker@0.1.0;

  export greentic:extension-base/manifest@0.2.0;
  export greentic:extension-base/lifecycle@0.2.0;
  export greentic:extension-bundle/recipes@0.2.0;
  export greentic:extension-bundle/bundling@0.2.0;
}
```

The `broker` import is what lets a bundle extension call a **design**
extension — e.g. asking `greentic.adaptive-cards` to validate the cards it is
about to pack. That call is permission-gated by
`runtime.permissions.callExtensionKinds`; see
[cross-extension-communication.md](./cross-extension-communication.md).

---

## Step 3 — `describe.json`

```json
{
  "$schema": "https://store.greentic.cloud/schemas/describe-v2.json",
  "apiVersion": "greentic.ai/v2",
  "kind": "BundleExtension",
  "compat": {
    "min_designer_version": ">=1.2.0",
    "min_runner_version": "^1.3.0-research.1",
    "contract_version": "1.3.0-research.1"
  },
  "metadata": {
    "id": "greentic.my-bundle",
    "name": "My Bundle",
    "version": "0.1.0",
    "summary": "Package designer output as a hosted WebChat Application Pack",
    "author": { "name": "My Name" },
    "license": "Apache-2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:bundle/hosted-webchat", "version": "1.0.0" }],
    "required": [{ "id": "greentic:adaptive-cards/render", "version": "^1.0.0" }]
  },
  "runtime": {
    "memoryLimitMB": 128,
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": ["design"] },
    "components": {
      "my-bundle": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "<64 lowercase hex>",
          "pack_id": "greentic.my-bundle",
          "component_version": "0.1.0"
        },
        "sha256": "<64 lowercase hex>",
        "world": "greentic:my-bundle/bundle-extension@1.0.0"
      }
    }
  },
  "contributions": {
    "recipes": [
      {
        "id": "hosted-webchat-standard",
        "display_name": "Hosted WebChat (Standard)",
        "config_schema": "schemas/webchat-standard-config.json"
      }
    ]
  }
}
```

Two things specific to this kind:

- **`Recipe.config_schema` is a path**, unlike `NodeType.config_schema`,
  which is inline JSON Schema text. Same field name, different meaning.
- **`execution` is the only top-level field permitted exclusively on
  `BundleExtension`.** Setting it on any other kind fails deserialization.

`display_name` and the optional `description` are LocalizedStrings — a plain
string, or `{ "default": …, "locales": { … } }`.

Every recipe you list here should also be returned by `list-recipes`. Nothing
enforces the agreement: a recipe present in the manifest but absent from the
WASM export shows up in the designer and fails when selected.

---

## Step 4 — Implement the exports

The scaffold stubs all five. A minimal working shape:

```rust
impl recipes::Guest for Component {
    fn list_recipes() -> Vec<recipes::RecipeSummary> {
        vec![recipes::RecipeSummary {
            id: "hosted-webchat-standard".into(),
            display_name: "Hosted WebChat (Standard)".into(),
            description: "Static site + WebChat widget".into(),
            icon_path: None,
        }]
    }

    fn recipe_config_schema(recipe_id: String) -> Result<String, types::ExtensionError> {
        match recipe_id.as_str() {
            "hosted-webchat-standard" => Ok(CONFIG_SCHEMA.to_string()),
            other => Err(types::ExtensionError::InvalidInput(format!(
                "unknown recipe: {other}"
            ))),
        }
    }

    fn supported_capabilities(recipe_id: String) -> Result<Vec<String>, types::ExtensionError> {
        match recipe_id.as_str() {
            "hosted-webchat-standard" => Ok(vec!["greentic:adaptive-cards/render".into()]),
            other => Err(types::ExtensionError::InvalidInput(format!(
                "unknown recipe: {other}"
            ))),
        }
    }
}

impl bundling::Guest for Component {
    fn validate_config(_recipe_id: String, config_json: String) -> Vec<types::Diagnostic> {
        // Return diagnostics rather than failing — the designer renders these
        // beside the operator's form while they are still editing.
        match serde_json::from_str::<serde_json::Value>(&config_json) {
            Ok(_) => Vec::new(),
            Err(e) => vec![types::Diagnostic {
                severity: types::Severity::Error,
                code: "config-parse".into(),
                message: e.to_string(),
                path: None,
            }],
        }
    }

    fn render(
        recipe_id: String,
        config_json: String,
        session: bundling::DesignerSession,
    ) -> Result<bundling::BundleArtifact, types::ExtensionError> {
        let bytes = build_pack(&recipe_id, &config_json, &session)?;
        let sha256 = sha256_hex(&bytes);
        Ok(bundling::BundleArtifact {
            filename: format!("{recipe_id}.gtpack"),
            bytes,
            sha256,
        })
    }
}
```

`validate_config` returns diagnostics, not a `Result` — it is called while the
operator types, so a malformed config is feedback, not a failure.

`session.capabilities_used` tells you which capabilities the session actually
relies on. Checking it against `supported_capabilities` before rendering is
what turns "the pack builds and then misbehaves at run time" into a
diagnostic the operator sees at build time.

---

## Step 5 — Build, validate, publish

```
gtdx dev --once        # build + pack + install into ~/.greentic/extensions/bundle/
gtdx validate ./
gtdx lint --dir ./
gtdx publish --dry-run
gtdx publish --registry oci://ghcr.io/... --sign --key-id release-2026
```

`gtdx publish` builds the component and assembles the `.gtxpack` itself —
there is no hand-written zip step. Full flag reference:
[cli-reference.md](./cli-reference.md#publish).

---

## Notes on real bundle implementations

The shipped `greentic.bundle-standard` renders through the same `render`
export described here; the designer's pack pipeline calls
`runtime.render_bundle(ext_id, recipe_id, config_json, session)` and writes
the returned bytes out as a `.gtpack`. Downstream, `greentic-bundle wizard
apply` + `build` turn that pack into a `.gtbundle` — that part is a separate
binary and not something a bundle extension implements.
