# How to Write a Deploy Extension

A `DeployExtension` ships an Application Pack to a deployment target. It
contributes **targets** — the destinations the deploy wizard offers — and
implements the deploy / poll / rollback lifecycle against them.

Against the v2 describe contract. Field reference:
[describe-json-spec.md](./describe-json-spec.md).

---

## What a deploy extension is responsible for

| Interface | Exports | Purpose |
|---|---|---|
| `extension-deploy/targets` | `list-targets`, `credential-schema`, `config-schema`, `validate-credentials` | Advertise destinations and what they need from the operator |
| `extension-deploy/deployment` | `deploy`, `poll`, `rollback` | Run the deployment and report on it |

`deploy` is **asynchronous by contract**: it returns a `deploy-job` handle
immediately and the designer polls. A target that cannot roll back declares
`supports_rollback: false` in its summary rather than implementing `rollback`
as a silent no-op.

---

## Prerequisites

- Rust 1.95+, `rustup target add wasm32-wasip2`
- `cargo install --locked cargo-component`
- A `gtdx` matching the current contract — see
  [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md#check-your-gtdx-before-you-start)

---

## Step 1 — Scaffold

```
gtdx new my-deploy --kind deploy --id greentic.my-deploy
```

**Fix the generated `wit/world.wit` before the first build** — the template
renders `greentic:extension-host@0.2.0` while the vendored package is
`@0.1.0`:

```bash
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' wit/world.wit
```

Background:
[getting-started-scaffolding.md](./getting-started-scaffolding.md#known-issue--a-fresh-scaffold-does-not-build).

Also delete the deprecated `engine` block and set a real `metadata.id`.

---

## Step 2 — The WIT world

```wit
package greentic:my-deploy;

world extension {
  import greentic:extension-base/types@0.2.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;
  import greentic:extension-host/secrets@0.1.0;
  import greentic:extension-host/http@0.1.0;

  export greentic:extension-base/manifest@0.2.0;
  export greentic:extension-base/lifecycle@0.2.0;
  export greentic:extension-deploy/targets@0.2.0;
  export greentic:extension-deploy/deployment@0.2.0;
}
```

A deploy extension is the kind most likely to need `http` and `secrets`, and
both are **default-deny**: importing them here grants nothing until
`runtime.permissions` lists the origins and secret URIs.

---

## Step 3 — `describe.json`

```json
{
  "$schema": "https://store.greentic.cloud/schemas/describe-v2.json",
  "apiVersion": "greentic.ai/v2",
  "kind": "DeployExtension",
  "compat": {
    "min_designer_version": ">=1.2.0",
    "min_runner_version": "^1.3.0-research.1",
    "contract_version": "1.3.0-research.1"
  },
  "metadata": {
    "id": "greentic.my-deploy",
    "name": "My Deploy",
    "version": "0.1.0",
    "summary": "Deploy an Application Pack to my platform",
    "author": { "name": "My Name" },
    "license": "Apache-2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:deploy/my-platform", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 64,
    "permissions": {
      "network": ["https://api.my-platform.com/*"],
      "secrets": ["secret://my-platform/api_token"],
      "callExtensionKinds": []
    },
    "components": {
      "my-deploy": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "<64 lowercase hex>",
          "pack_id": "greentic.my-deploy",
          "component_version": "0.1.0"
        },
        "sha256": "<64 lowercase hex>",
        "world": "greentic:my-deploy/deploy-extension@1.0.0"
      }
    }
  },
  "contributions": {}
}
```

**Targets are not a `contributions` field.** Unlike a bundle extension's
recipes, the v2 `contributions` block has no `targets` list — the designer
discovers targets by calling `list-targets` on the component. `contributions`
stays empty for a plain deploy extension.

This is a real v1→v2 break: a v1 describe carrying `contributions.targets`
is refused outright, since `contributions` is `additionalProperties: false`
in the schema and `deny_unknown_fields` in the Rust contract.

```
$ gtdx validate ./my-deploy
Error: describe.json schema validation failed:
  /contributions: Additional properties are not allowed ('targets' was unexpected)
```

Mind the permission matchers, which are **different for the two lists**:

- `network` is pattern-based — exact host or `*.suffix`, with a trailing `/*`
  stripped for the path prefix. `https://api.my-platform.com/*` is idiomatic.
- `secrets` is **not** a glob. An entry permits a URI only when the URI equals
  it, or starts with the entry followed by `/`. `secret://my-platform/*`
  permits nothing, and a trailing slash breaks the prefix. See
  [permissions-and-trust.md](./permissions-and-trust.md#secrets).

---

## Step 4 — Implement the exports

```rust
impl targets::Guest for Component {
    fn list_targets() -> Vec<targets::TargetSummary> {
        vec![targets::TargetSummary {
            id: "my-platform-prod".into(),
            display_name: "My Platform (Production)".into(),
            description: "Deploy to the production cluster".into(),
            icon_path: None,
            supports_rollback: true,
        }]
    }

    fn credential_schema(target_id: String) -> Result<String, types::ExtensionError> {
        match target_id.as_str() {
            "my-platform-prod" => Ok(CREDENTIAL_SCHEMA.to_string()),
            other => Err(types::ExtensionError::InvalidInput(format!(
                "unknown target: {other}"
            ))),
        }
    }

    fn config_schema(target_id: String) -> Result<String, types::ExtensionError> { /* … */ }

    fn validate_credentials(
        _target_id: String,
        credentials_json: String,
    ) -> Vec<types::Diagnostic> {
        // Shape-check only. Do NOT call the platform here, and never echo the
        // credential into a diagnostic — these strings reach the operator's UI.
        match serde_json::from_str::<Creds>(&credentials_json) {
            Ok(_) => Vec::new(),
            Err(e) => vec![types::Diagnostic {
                severity: types::Severity::Error,
                code: "credentials-shape".into(),
                message: e.to_string(),
                path: None,
            }],
        }
    }
}

impl deployment::Guest for Component {
    fn deploy(req: deployment::DeployRequest) -> Result<deployment::DeployJob, types::ExtensionError> {
        let job_id = start_remote_deploy(&req)?;
        Ok(deployment::DeployJob {
            id: job_id,
            status: deployment::DeployStatus::Pending,
            message: "submitted".into(),
            endpoints: Vec::new(),
        })
    }

    fn poll(job_id: String) -> Result<deployment::DeployJob, types::ExtensionError> {
        // Report progress through the enum: pending → provisioning →
        // configuring → starting → running, or failed / rolled-back.
        fetch_status(&job_id)
    }

    fn rollback(job_id: String) -> Result<(), types::ExtensionError> {
        revert(&job_id)
    }
}
```

Three contract details that are easy to get wrong:

- **`deploy` must return quickly.** It gets a job handle back to the designer;
  the actual work is observed through `poll`. A `deploy` that blocks until the
  platform is live will look like a hung designer.
- **`endpoints` is how the operator reaches the thing.** Fill it in on `poll`
  once the deployment reports `running`; an empty list leaves the wizard with
  nothing to link to.
- **`validate_credentials` returns diagnostics, not a `Result`.** It runs while
  the operator is still filling the form.

`deploy-request.artifact_bytes` carries the pack itself. The credential and
config JSON arrive as strings validated against the schemas you published
above.

---

## Step 5 — Build, validate, publish

```
gtdx dev --once        # installs into ~/.greentic/extensions/deploy/
gtdx validate ./
gtdx lint --dir ./
gtdx publish --dry-run
```

---

## Notes on real cloud deploy extensions

The shipped `greentic.deploy-aws` / `-azure` / `-gcp` / `-single-vm` /
`-desktop` extensions all implement exactly the two interfaces above. What
differs between them is entirely inside `deploy`/`poll` — the contract does
not model cloud specifics, and there is no separate credential store: an
extension resolves what it needs through `extension-host/secrets`, gated by
its own declared `permissions.secrets`.
