# How to Write a Deploy Extension

A `DeployExtension` teaches the Greentic Designer how to ship an Application
Pack to a deployment target. It appears as a set of selectable **targets** in
the designer's deploy wizard step.

This tutorial builds a minimal desktop deploy extension that writes a
marker file to a local directory. Real cloud targets (AWS EKS, GCP GKE,
Cisco on-prem) are planned for a future cycle.

The design extension tutorial covers prerequisites and the common steps.
Read [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md)
first if you are new to extension authoring.

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
cargo new --lib my-deploy-ext
cd my-deploy-ext
```

`Cargo.toml`:

```toml
[package]
name    = "my-deploy-ext"
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
package = "myco:my-deploy-ext"

[package.metadata.component.target]
path  = "wit"
world = "deploy-extension"

[package.metadata.component.target.dependencies]
"greentic:extension-base"   = { path = "../path/to/wit/extension-base.wit" }
"greentic:extension-host"   = { path = "../path/to/wit/extension-host.wit" }
"greentic:extension-deploy" = { path = "../path/to/wit/extension-deploy.wit" }
```

---

## Step 2 — `wit/world.wit`

```wit
package myco:my-deploy-ext;

world deploy-extension {
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;
  import greentic:extension-host/secrets@0.1.0;
  import greentic:extension-host/http@0.1.0;

  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export greentic:extension-deploy/targets@0.1.0;
  export greentic:extension-deploy/deployment@0.1.0;
}
```

Deploy extensions typically import `secrets` (for cloud credentials) and
`http` (for cloud API calls). This stub does not use them but declares the
imports to match the canonical deploy-extension world.

---

## Step 3 — `describe.json`

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DeployExtension",
  "metadata": {
    "id": "myco.desktop-deploy",
    "name": "Desktop Deploy",
    "version": "0.1.0",
    "summary": "Deploy an Application Pack to a local directory for testing",
    "author": { "name": "My Name", "email": "me@example.com" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "myco:deploy/desktop", "version": "0.1.0" }
    ],
    "required": []
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 32,
    "permissions": {
      "network": [],
      "secrets": [],
      "callExtensionKinds": []
    }
  },
  "contributions": {
    "targets": [
      {
        "id": "local-dir",
        "displayName": "Local Directory",
        "credentialSchema": null,
        "configSchema": "schemas/local-dir-config.json"
      }
    ]
  }
}
```

Create `schemas/local-dir-config.json`:

```json
{
  "type": "object",
  "properties": {
    "output_path": {
      "type": "string",
      "description": "Absolute path to the directory where the pack will be written"
    }
  },
  "required": ["output_path"]
}
```

---

## Step 4 — `src/lib.rs` — Implement the WIT exports

```rust
#![allow(clippy::used_underscore_items)]

#[allow(warnings)]
mod bindings;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_deploy::{deployment, targets};
use bindings::greentic::extension_base::types;

struct Component;

// ---- base::manifest ----

impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "myco.desktop-deploy".into(),
            version: "0.1.0".into(),
            kind: types::Kind::Deploy,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "myco:deploy/desktop".into(),
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

// ---- deploy::targets ----

impl targets::Guest for Component {
    fn list_targets() -> Vec<targets::TargetSummary> {
        vec![targets::TargetSummary {
            id: "local-dir".into(),
            display_name: "Local Directory".into(),
            description: "Write the pack to a local directory for testing".into(),
            icon_path: None,
            supports_rollback: false,
        }]
    }

    fn credential_schema(target_id: String)
        -> Result<String, types::ExtensionError>
    {
        match target_id.as_str() {
            "local-dir" => Ok(r#"{"type":"object","properties":{}}"#.into()),
            other => Err(types::ExtensionError::InvalidInput(
                format!("unknown target: {other}")
            )),
        }
    }

    fn config_schema(target_id: String)
        -> Result<String, types::ExtensionError>
    {
        match target_id.as_str() {
            "local-dir" => Ok(r#"{
                "type": "object",
                "properties": {
                    "output_path": {
                        "type": "string",
                        "description": "Absolute path to the output directory"
                    }
                },
                "required": ["output_path"]
            }"#.into()),
            other => Err(types::ExtensionError::InvalidInput(
                format!("unknown target: {other}")
            )),
        }
    }

    fn validate_credentials(
        _target_id: String,
        _credentials_json: String,
    ) -> Vec<types::Diagnostic> {
        // Desktop target needs no credentials.
        vec![]
    }
}

// ---- deploy::deployment ----

impl deployment::Guest for Component {
    fn deploy(req: deployment::DeployRequest)
        -> Result<deployment::DeployJob, types::ExtensionError>
    {
        if req.target_id != "local-dir" {
            return Err(types::ExtensionError::InvalidInput(
                format!("unknown target: {}", req.target_id)
            ));
        }
        let cfg: serde_json::Value = serde_json::from_str(&req.config_json)
            .map_err(|e| types::ExtensionError::InvalidInput(e.to_string()))?;
        let output_path = cfg["output_path"]
            .as_str()
            .ok_or_else(|| types::ExtensionError::InvalidInput(
                "output_path required".into()
            ))?;

        // Stub: write a marker file to the configured path.
        // A real implementation would write req.artifact_bytes as a .gtpack file.
        let marker = format!(
            "deployment: {}\nname: {}\nbytes: {}\n",
            req.target_id,
            req.deployment_name,
            req.artifact_bytes.len()
        );
        let job_id = format!("local-{}", req.deployment_name);
        let marker_path = format!("{output_path}/{}.deployed", req.deployment_name);

        // Note: WASM components cannot access the filesystem directly.
        // In a real deployment the host would write the file on behalf of the
        // extension. This stub stores the marker in the job message for
        // demonstration purposes.
        Ok(deployment::DeployJob {
            id: job_id,
            status: deployment::DeployStatus::Running,
            message: format!("would write to {marker_path}: {marker}"),
            endpoints: vec![format!("file://{output_path}")],
        })
    }

    fn poll(job_id: String) -> Result<deployment::DeployJob, types::ExtensionError> {
        Ok(deployment::DeployJob {
            id: job_id.clone(),
            status: deployment::DeployStatus::Running,
            message: "desktop deploy is synchronous".into(),
            endpoints: vec![],
        })
    }

    fn rollback(job_id: String) -> Result<(), types::ExtensionError> {
        Err(types::ExtensionError::InvalidInput(format!(
            "desktop target {job_id} does not support rollback"
        )))
    }
}

bindings::export!(Component with_types_in bindings);
```

---

## Step 5 — Build, package, and install

```bash
# Build
cargo component build --release

# Package
STAGE=$(mktemp -d)
cp target/wasm32-wasip2/release/my_deploy_ext.wasm "${STAGE}/extension.wasm"
cp describe.json "${STAGE}/"
cp -r schemas/ "${STAGE}/"
(cd "${STAGE}" && zip -r - .) > myco.desktop-deploy-0.1.0.gtxpack
rm -rf "${STAGE}"

# Validate and install
gtdx validate ./
gtdx install ./myco.desktop-deploy-0.1.0.gtxpack --trust loose

# Verify
gtdx list
```

---

## Notes on Real Cloud Deploy Extensions

The stub above calls `deploy` synchronously and returns `Running` immediately.
A real cloud deploy extension would:

1. Parse `req.credentials_json` and `req.config_json`.
2. Retrieve API keys from the host `secrets` interface (not from the config
   JSON directly — see [permissions-and-trust.md](./permissions-and-trust.md)).
3. Make an HTTP call via `greentic:extension-host/http` to start a cloud
   provisioning job. The URL must be listed in `runtime.permissions.network`.
4. Return a `DeployJob` in `pending` or `provisioning` state with a job ID.
5. Implement `poll` to call the cloud API and return the current status.
6. Implement `rollback` if the target supports it.

For examples of what real cloud deploy parameters look like, refer to
existing cloud provider SDKs. The extension contract imposes no opinion on
the shape of `credentials_json` or `config_json` beyond what your
`credentialSchema` / `configSchema` declare.
