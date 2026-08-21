# How to Write a Provider Extension

A `ProviderExtension` describes how Greentic talks to an outside system —
a messaging channel, an event source, an event sink — and ships the runtime
component that does the talking.

Against the v2 describe contract. Field reference:
[describe-json-spec.md](./describe-json-spec.md).

---

## Pick your world first

Unlike the other kinds, `greentic:extension-provider` declares **six** worlds.
Choose the one matching the surfaces you actually implement — a world you
export but do not implement will not compile, and one you implement but do not
export is invisible.

| World | Exports |
|---|---|
| `messaging-only-provider` | `messaging` |
| `event-source-only-provider` | `event-source` |
| `event-sink-only-provider` | `event-sink` |
| `messaging-and-event-source-provider` | `messaging` + `event-source` |
| `messaging-and-event-sink-provider` | `messaging` + `event-sink` |
| `full-provider` | all three |

`gtdx new --kind provider` scaffolds the **messaging-only** shape. Switching
later means editing `wit/world.wit` and adding the matching
`impl …::Guest for Component` blocks.

### What each interface owes

| Interface | Exports |
|---|---|
| `messaging` | `list-channels`, `describe-channel`, `secret-schema`, `config-schema`, `dry-run-encode` |
| `event-source` | `list-trigger-types`, `describe-trigger`, `trigger-schema` |
| `event-sink` | `list-event-types`, `describe-event`, `event-schema` |

A `channel-profile` carries `direction` (`inbound` / `outbound` /
`bidirectional`) and `tier-support` — the card tiers the channel can render:
`tier-a-native`, `tier-b-attachment`, `tier-c-fallback`, `tier-d-text-only`.
That tier list is how the designer decides whether a card survives the trip to
this channel, so declaring a tier you cannot actually render produces a
degraded message with no error anywhere.

---

## Prerequisites

- Rust 1.95+, `rustup target add wasm32-wasip2`
- `cargo install --locked cargo-component`
- A `gtdx` matching the current contract — see
  [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md#check-your-gtdx-before-you-start)

---

## Step 1 — Scaffold

```
gtdx new my-provider --kind provider --id greentic.my-provider
```

**On `gtdx` 1.2.0 or older, two fixes are required before the first build**
— this kind needs both. 1.2.1 needs neither:

```bash
# 1. The rendered world asks for a WIT package version that does not exist.
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' wit/world.wit

# 2. The stub names an error type that is not in the provider types module.
#    The WIT declares `extension-error`, which comes from extension-base/types.
sed -i 's/provider_types::Error/types::ExtensionError/g' src/lib.rs
```

Without (1), `cargo component build` fails with
`package 'greentic:extension-host@0.2.0' not found`. Without (2) it fails with
eight `cannot find type 'Error' in module 'provider_types'` errors. Both are
fixed in `gtdx` 1.2.1; background in
[getting-started-scaffolding.md](./getting-started-scaffolding.md#if-you-are-on-gtdx-120-or-older).

On those versions, also delete the deprecated `engine` block from
`describe.json`, and (on any version) set a real
`metadata.id`.

---

## Step 2 — The WIT world

The messaging-only shape, with versions matching the vendored packages:

```wit
package greentic:my-provider;

world extension {
  import greentic:extension-base/types@0.2.0;
  import greentic:extension-host/logging@0.1.0;
  import greentic:extension-host/i18n@0.1.0;

  export greentic:extension-base/manifest@0.2.0;
  export greentic:extension-base/lifecycle@0.2.0;
  export greentic:extension-provider/messaging@0.2.0;
}
```

Note what a provider world does **not** import: no `http`, no `secrets`, no
`broker`. This component supplies *metadata* — channel profiles and schemas.
The network work happens in the runtime component it ships (Step 4).

---

## Step 3 — `describe.json`

```json
{
  "$schema": "https://store.greentic.cloud/schemas/describe-v2.json",
  "apiVersion": "greentic.ai/v2",
  "kind": "ProviderExtension",
  "compat": {
    "min_designer_version": ">=1.2.0",
    "min_runner_version": "^1.3.0-research.1",
    "contract_version": "1.3.0-research.1"
  },
  "metadata": {
    "id": "greentic.my-provider",
    "name": "My Provider",
    "version": "0.1.0",
    "summary": "Talk to My Platform's chat channels",
    "author": { "name": "My Name" },
    "license": "Apache-2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:messaging/my-platform", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 64,
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] },
    "components": {
      "my-provider": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "<64 lowercase hex>",
          "pack_id": "greentic.my-provider",
          "component_version": "0.1.0"
        },
        "sha256": "<64 lowercase hex>",
        "world": "greentic:my-provider/messaging-only-provider@1.0.0"
      }
    }
  },
  "contributions": {}
}
```

Like a deploy extension, a provider contributes nothing through
`contributions` — channels and triggers are discovered by calling the
component. `contributions` stays `{}`.

**The v1 `runtime.gtpack` block moved.** It is now
`runtime.components.<id>.gtpack`; a top-level `runtime.gtpack` is refused
(`runtime` is `deny_unknown_fields`). Anything still telling you to set
`runtime.gtpack.file` — including the `--kind wasm-component` scaffold's own
`runtime/README.md` — is describing v1.

---

## Step 4 — The runtime component

The extension WASM answers questions about channels. Actually sending and
receiving messages is the job of a **runtime component**, built as its own
crate and shipped as a second entry under `runtime.components` (in-pack via
`gtpack`, or pinned by OCI digest via `oci_ref`).

When pinning by OCI, **pin by digest**: a built pack embeds the ref
permanently, and these registries publish tags out of chronological order, so
the highest semver is frequently the oldest artifact.

---

## Step 5 — Implement the exports

The scaffold stubs the messaging surface. The error type is
`types::ExtensionError` (from `extension-base/types`), not anything under the
provider's own `types` interface:

```rust
impl messaging::Guest for Component {
    fn list_channels() -> Vec<messaging::ChannelProfile> {
        vec![messaging::ChannelProfile {
            id: "my-platform-chat".into(),
            display_name: "My Platform Chat".into(),
            direction: provider_types::Direction::Bidirectional,
            tier_support: vec![
                provider_types::CardTier::TierBAttachment,
                provider_types::CardTier::TierDTextOnly,
            ],
            metadata: Vec::new(),
        }]
    }

    fn describe_channel(id: String) -> Result<messaging::ChannelProfile, types::ExtensionError> {
        Self::list_channels()
            .into_iter()
            .find(|c| c.id == id)
            .ok_or(types::ExtensionError::NotFound(id))
    }

    fn secret_schema(id: String) -> Result<String, types::ExtensionError> {
        // JSON Schema for the channel's credentials — rendered as the
        // operator's credential form. Never return a value here, only a shape.
        match id.as_str() {
            "my-platform-chat" => Ok(SECRET_SCHEMA.to_string()),
            other => Err(types::ExtensionError::NotFound(other.to_string())),
        }
    }

    fn config_schema(id: String) -> Result<String, types::ExtensionError> { /* … */ }

    fn dry_run_encode(
        id: String,
        sample: Vec<u8>,
    ) -> Result<Vec<u8>, types::ExtensionError> {
        // Encode `sample` as this channel's outbound envelope, without
        // sending anything. This is what lets the designer show an operator
        // what will go over the wire.
        encode_envelope(&id, &sample)
    }
}
```

`dry_run_encode` is the one export people skip, and it is the one that makes a
provider debuggable: it must perform the real encoding and no I/O.

---

## Step 6 — Build, validate, publish

```
gtdx dev --once        # installs into ~/.greentic/extensions/provider/
gtdx validate ./
gtdx lint --dir ./
gtdx publish --dry-run
gtdx publish --registry oci://ghcr.io/... --sign --key-id release-2026
```

`gtdx publish` builds and assembles the `.gtxpack` itself — there is no
hand-written staging-directory-and-zip step, and no manual sha256 computation
for the extension component.

---

## Common pitfalls

| Symptom | Cause |
|---|---|
| `package 'greentic:extension-host@0.2.0' not found` | Scaffold WIT versions — Step 1, fix (1) |
| `cannot find type 'Error' in module 'provider_types'` | Scaffold stub error type — Step 1, fix (2) |
| Cards arrive degraded on a real channel | `tier_support` claims a tier the runtime cannot render |
| `unknown field 'gtpack'` under `runtime` | v1 shape; it belongs inside `runtime.components.<id>` |
| Channel absent from the designer | `list-channels` does not return it — there is no `contributions` list to fall back on |
