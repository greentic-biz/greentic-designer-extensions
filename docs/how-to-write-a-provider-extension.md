# How to Write a Provider Extension

This tutorial walks you through building a `ProviderExtension` from scratch
using a minimal messaging provider as the running example. By the end you will
have a signed `.gtxpack` that `gtdx install` extracts to the correct runtime
path for `greentic-runner` pickup.

Provider extensions consist of **two artifacts shipped together** inside one
`.gtxpack` file:

1. **Extension WASM component** — metadata queries (channels, schemas, i18n).
   Runs in `gtdx` / designer / wizards.
2. **Runtime pack** (`.gtpack`) — actual message-send or event-emit runtime.
   Runs in `greentic-runner`.

The reference implementation for this guide lives in
`greentic-biz/greentic-provider-extensions/` (a dedicated authoring repo
created during Wave A). The Telegram pilot will be the first shipped provider
(Wave B).

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
- **`greentic-pack`** — for building the runtime `.gtpack`:
  ```
  cargo install greentic-pack --locked
  ```

---

## Anatomy of a Provider Extension

Each `.gtxpack` contains three things:

1. **`describe.json`** — metadata: identity, capabilities offered, required
   runtime `.gtpack` hash + path
2. **Extension WASM** (`wasm32-wasip2`, `cdylib`) — exports metadata
   interfaces (e.g., `list-channels`, `describe-channel`)
3. **Embedded runtime `.gtpack`** — contains the actual provider component
   (e.g., Slack message-send logic). This is the same artifact format that
   `greentic-runner` loads today.

The extension layer handles **discovery and configuration**. The runtime pack
handles **execution**. The two are bundled so `gtdx install` can extract both
atomically.

---

## Step 1 — Create the extension crate

```
cargo new --lib my-provider-ext
cd my-provider-ext
```

Open `Cargo.toml` and configure for WASM component authoring:

```toml
[package]
name    = "my-provider-ext"
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

[package.metadata.component]
package = "myco:my-provider-ext"

[package.metadata.component.target]
path  = "wit"
world = "messaging-only-provider"

[package.metadata.component.target.dependencies]
"greentic:extension-base"     = { path = "../path/to/wit/extension-base.wit" }
"greentic:extension-host"     = { path = "../path/to/wit/extension-host.wit" }
"greentic:extension-provider" = { path = "../path/to/wit/extension-provider.wit" }
```

The `world` field selects which provider capability to implement. Choose one:

- `messaging-only-provider` — bidirectional messaging (send + receive)
- `event-source-only-provider` — inbound triggers (webhooks, cron, timers)
- `event-sink-only-provider` — outbound event emits
- `messaging-and-event-source-provider` — messaging + triggers
- `messaging-and-event-sink-provider` — messaging + emits
- `full-provider` — all three capabilities

For this guide, we'll use `messaging-only-provider`. Telegram v1 is a
messaging-only provider.

---

## Step 2 — WIT world in `wit/world.wit`

Create `wit/world.wit`:

```wit
package myco:my-provider-ext;

world messaging-only-provider {
  // Host services the extension may call.
  import greentic:extension-base/types@0.1.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;

  // Interfaces the extension must implement.
  export greentic:extension-base/manifest@0.1.0;
  export greentic:extension-base/lifecycle@0.1.0;
  export greentic:extension-provider/messaging@0.1.0;
}
```

The `messaging` interface requires you to implement:

- `list-channels() -> Vec<ChannelProfile>` — all supported channels
- `describe-channel(id) -> ChannelProfile` — metadata for one channel
- `secret-schema(id) -> JSON Schema` — what secrets are needed (API key, token)
- `config-schema(id) -> JSON Schema` — what config values (user IDs, settings)
- `dry-run-encode(id, sample) -> Vec<u8>` — test encoding a payload

The spec (`extension-provider.wit`) defines the full interface.

---

## Step 3 — Author `describe.json`

Create `describe.json` in the crate root:

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "ProviderExtension",
  "metadata": {
    "id": "myco.my-provider",
    "name": "My Provider",
    "version": "0.1.0",
    "summary": "Send messages via My Provider API",
    "author": { "name": "My Name", "email": "me@example.com" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": "*",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      {
        "id": "myco:messaging/channels",
        "version": "0.1.0"
      }
    ],
    "required": []
  },
  "runtime": {
    "component": "provider.wasm",
    "memoryLimitMB": 128,
    "permissions": {
      "network": ["https://api.myprovider.com"],
      "secrets": ["api-key", "webhook-secret"],
      "callExtensionKinds": []
    },
    "gtpack": {
      "file": "runtime/provider.gtpack",
      "sha256": "abc123def456...64lowercase hex chars...",
      "pack_id": "myco.my-provider-runtime@0.1.0",
      "component_version": "0.6.0"
    }
  },
  "contributions": {
    "schemas": [
      "schemas/channel-config.json"
    ],
    "prompts": [],
    "knowledge": [],
    "tools": []
  }
}
```

**Key fields:**

- `kind: "ProviderExtension"` — required invariant; deserialize will reject if
  missing or misspelled
- `engine.greenticDesigner: "*"` — required even if provider is not
  designer-specific (host enforces via `deny_unknown_fields`)
- `runtime.gtpack.sha256` — must be **exactly 64 lowercase hex characters**;
  computed via:
  ```bash
  sha256sum runtime/provider.gtpack | cut -d' ' -f1
  ```
- `runtime.gtpack.pack_id` — must match the `pack.yaml` or `manifest.cbor`
  inside the runtime `.gtpack`; checked at install time
- `capabilities.offered` — list what this provider exposes (channels, event
  sources, etc.); designers will query these

---

## Step 4 — Build the runtime `.gtpack`

The **runtime pack is separate from the extension**. It contains:

- `manifest.cbor` — pack metadata (name, version, pack_id)
- WASM components targeting `wasm32-wasip2` with `greentic:component@0.6.0`
  exports
- Optional Ed25519 signature

Build it using the standard `greentic-pack` tooling (see
[greentic-pack docs](https://github.com/greentic-biz/greentic-pack) or
[greentic-pack on crates.io](https://crates.io/crates/greentic-pack)):

```bash
# In your provider runtime crate directory
greentic-pack build \
  --manifest pack.yaml \
  --output ../provider-extension/runtime/provider.gtpack
```

**Important:** The runtime build happens **before** the extension build. The
extension zip must contain the pre-built `.gtpack` file.

After building, verify the `pack_id` inside matches what you declare in
`describe.json`:

```bash
# Inspect the runtime pack
greentic-pack inspect runtime/provider.gtpack
# Should output pack_id: "myco.my-provider-runtime@0.1.0"
```

---

## Step 5 — Implement extension metadata in `src/lib.rs`

Run `cargo component build` once to generate bindings, then implement the
traits:

```rust
#![allow(clippy::used_underscore_items)]

#[allow(warnings)]
mod bindings;

use bindings::exports::greentic::extension_base::{lifecycle, manifest};
use bindings::exports::greentic::extension_provider::messaging;
use bindings::greentic::extension_base::types;

struct Component;

impl manifest::Guest for Component {
    fn get_identity() -> types::ExtensionIdentity {
        types::ExtensionIdentity {
            id: "myco.my-provider".into(),
            version: "0.1.0".into(),
            kind: types::Kind::Provider,
        }
    }
    fn get_offered() -> Vec<types::CapabilityRef> {
        vec![types::CapabilityRef {
            id: "myco:messaging/channels".into(),
            version: "0.1.0".into(),
        }]
    }
    fn get_required() -> Vec<types::CapabilityRef> {
        vec![]
    }
}

impl lifecycle::Guest for Component {
    fn init(_config_json: String) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn shutdown() {}
}

impl messaging::Guest for Component {
    fn list_channels() -> Vec<messaging::ChannelProfile> {
        vec![
            messaging::ChannelProfile {
                id: "general".into(),
                display_name: "General Chat".into(),
                direction: messaging::Direction::Bidirectional,
                tier_support: vec![
                    messaging::CardTier::TierANative,
                    messaging::CardTier::TierBAttachment,
                    messaging::CardTier::TierDTextOnly,
                ],
                metadata: vec![],
            },
        ]
    }

    fn describe_channel(id: String) -> Result<messaging::ChannelProfile, messaging::Error> {
        if id == "general" {
            Ok(messaging::ChannelProfile {
                id: "general".into(),
                display_name: "General Chat".into(),
                direction: messaging::Direction::Bidirectional,
                tier_support: vec![
                    messaging::CardTier::TierANative,
                    messaging::CardTier::TierBAttachment,
                    messaging::CardTier::TierDTextOnly,
                ],
                metadata: vec![],
            })
        } else {
            Err(messaging::Error::NotFound(format!("channel {id} not found")))
        }
    }

    fn secret_schema(id: String) -> Result<String, messaging::Error> {
        if id == "general" {
            Ok(r#"{"type":"object","properties":{"api-key":{"type":"string"}},"required":["api-key"]}"#.to_string())
        } else {
            Err(messaging::Error::NotFound(format!("channel {id} not found")))
        }
    }

    fn config_schema(id: String) -> Result<String, messaging::Error> {
        if id == "general" {
            Ok(r#"{"type":"object","properties":{"channel-id":{"type":"string"}},"required":["channel-id"]}"#.to_string())
        } else {
            Err(messaging::Error::NotFound(format!("channel {id} not found")))
        }
    }

    fn dry_run_encode(id: String, sample: Vec<u8>) -> Result<Vec<u8>, messaging::Error> {
        if id == "general" {
            // Echo back; in real code, validate the payload shape
            Ok(sample)
        } else {
            Err(messaging::Error::NotFound(format!("channel {id} not found")))
        }
    }
}

bindings::export!(Component with_types_in bindings);
```

---

## Step 6 — Build the extension WASM

```
cargo component build --release
```

Output: `target/wasm32-wasip2/release/my_provider_ext.wasm`

---

## Step 7 — Assemble the `.gtxpack`

A `.gtxpack` is a ZIP archive. Required layout:

```
myco.my-provider-0.1.0.gtxpack
├── describe.json
├── provider.wasm           ← the compiled extension WASM
├── schemas/
│   └── channel-config.json
└── runtime/
    └── provider.gtpack     ← the pre-built runtime pack
```

Build script (`build.sh`):

```bash
#!/usr/bin/env bash
set -euo pipefail

NAME="myco.my-provider"
VERSION="0.1.0"
PACK="${NAME}-${VERSION}.gtxpack"

# Build extension WASM
cargo component build --release

# Build runtime pack (separately; assumes separate runtime crate)
pushd ../provider-runtime
greentic-pack build --manifest pack.yaml --output ../provider-ext/runtime/provider.gtpack
popd

# Compute runtime pack sha256 for describe.json
RUNTIME_SHA=$(sha256sum runtime/provider.gtpack | cut -d' ' -f1)
echo "Runtime pack SHA256: $RUNTIME_SHA"

# Update describe.json with computed sha256
# (Note: use a tool like jq for production; hardcoding here for clarity)
sed -i "s/\"sha256\": \"[^\"]*\"/\"sha256\": \"$RUNTIME_SHA\"/" describe.json

mkdir -p dist
rm -f "dist/${PACK}"

# Assemble staging directory
STAGE=$(mktemp -d)
cp target/wasm32-wasip2/release/my_provider_ext.wasm "${STAGE}/provider.wasm"
cp describe.json "${STAGE}/"
cp -r schemas/ "${STAGE}/"
cp -r runtime/ "${STAGE}/"

# Create the zip deterministically (sorted filenames)
(cd "${STAGE}" && zip -r -X - . | sort) > "dist/${PACK}"
rm -rf "${STAGE}"

echo "Built dist/${PACK}"
```

```
chmod +x build.sh
./build.sh
```

---

## Step 8 — Validate

```
gtdx validate ./
# Expected output:
# ✓ ./describe.json valid
```

You can also unpack and validate:

```
unzip dist/myco.my-provider-0.1.0.gtxpack -d /tmp/ext-check
gtdx validate /tmp/ext-check/
```

---

## Step 9 — Install and test

```
gtdx install ./dist/myco.my-provider-0.1.0.gtxpack --trust loose
```

Verify:

```bash
$ gtdx list --kind provider
[provider]
  myco.my-provider@0.1.0  Send messages via My Provider API

$ gtdx info myco.my-provider
ID:      myco.my-provider@0.1.0
Kind:    provider
Summary: Send messages via My Provider API
Capabilities offered: [myco:messaging/channels]

$ ls -la ~/.greentic/extensions/provider/myco.my-provider-0.1.0/
describe.json
provider.wasm
schemas/

$ ls -la ~/.greentic/runtime/packs/providers/gtdx/
myco.my-provider-runtime@0.1.0.gtpack
```

When `greentic-runner` starts, it polls `~/.greentic/runtime/packs/providers/`
every 30 seconds. The `.gtpack` will be hot-loaded into any tenant that
declares it in their bundle (via the bundle `.gmap` access rules).

---

## Step 10 — Sign and publish

Generate a keypair:

```
gtdx keygen --name "My Organization"
```

Sign the artifact:

```
gtdx sign ./dist/myco.my-provider-0.1.0.gtxpack
```

This creates:

- `myco.my-provider-0.1.0.gtxpack.sig` — the Ed25519 signature
- Updates `describe.json` with the signature metadata

For production installs, use `--trust normal` or `--trust strict`:

```
gtdx install ./dist/myco.my-provider-0.1.0.gtxpack --trust strict
```

Then publish to the Greentic Store (when available):

```
gtdx publish ./dist/myco.my-provider-0.1.0.gtxpack
```

---

## Reference: Where do provider extensions live?

Provider extensions are authored in the **dedicated repo**:

```
greentic-biz/greentic-provider-extensions/
├── crates/
│   ├── provider-telegram-ext/    ← Wave B pilot (messaging only)
│   ├── provider-slack-ext/       ← Wave B+ (messaging + event-sink)
│   ├── provider-teams-ext/       ← Wave C (messaging + event-sink)
│   └── provider-cron-ext/        ← Wave B (event-source only)
└── runtime/
    ├── provider-telegram-runtime/
    ├── provider-slack-runtime/
    └── ...
```

This is **separate** from `greentic-biz/greentic-messaging-providers`, which
ships the legacy runtime-only providers (still in use until retrofitted).

---

## Common pitfalls

1. **Missing `kind: ProviderExtension` in `describe.json`** — Installation
   deserializer requires it. Omit it → install fails with "invalid kind."

2. **`runtime.gtpack.sha256` wrong format** — Must be exactly **64 lowercase
   hex characters**. Mix of uppercase, or wrong length → deserialize fails.

3. **Stale runtime pack** — If you rebuild the runtime `.gtpack` but forget
   to update sha256 in `describe.json`, installation will fail with a hash
   mismatch error.

4. **Confusing extension WASM and runtime WASM** — The extension WASM is
   `wasm32-wasip2` via `cargo-component`; the runtime pack contains another
   WASM binary also targeting `wasm32-wasip2` via `greentic-pack`. Don't
   confuse the two. Extension runs in the designer/wizard; runtime runs in
   `greentic-runner`.

5. **`greenticDesigner: "*"` missing** — Even if your provider has nothing to
   do with designer (e.g., a pure event source), the `engine.greenticDesigner`
   field is **required**. Set it to `"*"` or a version constraint.

6. **Duplicate `pack_id` in manual directory** — If a `.gtpack` with the same
   `pack_id` already exists in `~/.greentic/runtime/packs/providers/manual/`,
   install refuses to proceed. Either `--force` override or pre-delete.

7. **Runner doesn't pick up the pack immediately** — Runner polls every 30
   seconds. Allow up to 30s for a fresh install to appear in active flows.

8. **Zip file not deterministic** — Use `zip -r -X` and sort filenames for
   reproducible builds (important if you share `.gtxpack` signatures).

---

## What to do next

- Add integration tests using `greentic-ext-testing` (see
  `greentic-designer-extensions` test suite).
- For multi-channel providers, expand `list_channels()` and add per-channel
  secret/config schemas.
- For event-source providers, implement the `event-source` interface
  (`list-trigger-types`, `describe-trigger`, `trigger-schema`).
- For event-sink providers (outbound emits), implement the `event-sink`
  interface (`list-event-types`, `describe-event`, `event-schema`).
- Add i18n support: populate `i18n/en.json` and reference in
  `contributions.i18n`.
- Publish to the Greentic Store and enable discovery via designer provider
  picker.

For trust policies and signing details, see
[permissions-and-trust.md](./permissions-and-trust.md).
