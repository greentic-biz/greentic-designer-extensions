# describe.json — v1 Reference

## Overview

`describe.json` is the manifest for every Greentic Designer extension. It is
the single file the runtime reads before loading the WASM component. It
declares:

- Who the extension is (identity, version, license, author)
- What kind of extension it is (`DesignExtension`, `BundleExtension`, or
  `DeployExtension`)
- Which runtime versions it requires
- Which capabilities it offers and which it needs
- What runtime resources (memory, network, secrets) it is allowed to use
- What it contributes to the designer (tools, schemas, prompts, recipes,
  targets, etc.)
- An optional Ed25519 signature for trust verification

The runtime (`greentic-ext-runtime`) reads `describe.json` at install time
to resolve the capability graph and at load time to enforce permissions. The
CLI (`gtdx`) validates it before packing. The Greentic Store indexes it for
search.

**Schema location:** `crates/greentic-ext-contract/schemas/describe-v1.json`

**JSON Schema draft:** 2020-12

---

## Top-Level Structure

```json
{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": { ... },
  "engine": { ... },
  "capabilities": { ... },
  "runtime": { ... },
  "contributions": { ... },
  "signature": { ... }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `apiVersion` | const string | Yes | Always `"greentic.ai/v1"` |
| `kind` | enum | Yes | `DesignExtension`, `BundleExtension`, or `DeployExtension` |
| `metadata` | object | Yes | Identity, authorship, display info |
| `engine` | object | Yes | Compatible designer + runtime version ranges |
| `capabilities` | object | Yes | Offered and required capability refs |
| `runtime` | object | Yes | WASM component path + permissions |
| `contributions` | object | Yes | Kind-specific assets exposed to designer |
| `signature` | object | No | Ed25519 signature for trust verification |

---

## Field Reference

### `apiVersion`

```json
"apiVersion": "greentic.ai/v1"
```

Fixed constant. Future incompatible schema changes will increment this value.
The runtime rejects any `apiVersion` it does not recognize.

---

### `kind`

```json
"kind": "DesignExtension"
```

One of three values:

| Value | Purpose |
|-------|---------|
| `DesignExtension` | Teaches the designer to author a new content type. |
| `BundleExtension` | Packages designer output into a deployable Application Pack. |
| `DeployExtension` | Ships Application Packs to a deployment target. |

---

### `metadata`

```json
"metadata": {
  "id": "greentic.adaptive-cards",
  "name": "Adaptive Cards",
  "version": "1.6.0",
  "summary": "Design and validate Microsoft Adaptive Cards v1.6",
  "description": "Long-form description ...",
  "author": {
    "name": "Greentic",
    "email": "team@greentic.ai",
    "publicKey": "ed25519:AAAA..."
  },
  "license": "MIT",
  "homepage": "https://greentic.ai",
  "repository": "https://github.com/greenticai/...",
  "keywords": ["adaptive-cards", "ui"],
  "icon": "assets/icon.png",
  "screenshots": ["assets/shot1.png"]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `id` | string | Yes | Pattern: `^[a-z][a-z0-9.-]*\.[a-z0-9.-]+$` (e.g. `greentic.adaptive-cards`) |
| `name` | string | Yes | Display name, min length 1 |
| `version` | string | Yes | Semver: `MAJOR.MINOR.PATCH[-pre][+build]` |
| `summary` | string | Yes | One-liner, max 200 chars |
| `description` | string | No | Long-form Markdown |
| `author.name` | string | Yes | Publisher display name |
| `author.email` | string | No | Contact email |
| `author.publicKey` | string | No | Ed25519 public key (used for signature verification) |
| `license` | string | Yes | SPDX identifier (e.g. `MIT`, `Apache-2.0`) |
| `homepage` | string | No | URI |
| `repository` | string | No | URI |
| `keywords` | string[] | No | Search tags |
| `icon` | string | No | Relative path to icon inside the `.gtxpack` |
| `screenshots` | string[] | No | Relative paths to screenshots |

**ID pattern:** Reverse-domain style, lowercase. The prefix before the first
dot is the publisher namespace (`greentic`, `osora`, `myco`). The suffix is
the extension slug. Examples:

- `greentic.adaptive-cards`
- `osora.digital-workers`
- `myco.telco-x-forms`

---

### `engine`

```json
"engine": {
  "greenticDesigner": ">=0.6.0",
  "extRuntime": "^0.1.0"
}
```

Semver range expressions that the host must satisfy before loading the
extension. If either constraint is not met, the runtime refuses to load the
extension and surfaces an error in the designer.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `greenticDesigner` | string | Yes | Semver range for the Greentic Designer host application |
| `extRuntime` | string | Yes | Semver range for `greentic-ext-runtime` |

Use `>=` for a minimum-version floor, `^` for compatible-with semantics
(allows patch and minor bumps, locks major), and `*` to accept any version
(not recommended for production).

---

### `capabilities`

```json
"capabilities": {
  "offered": [
    { "id": "greentic:adaptive-cards/validate", "version": "1.0.0" },
    { "id": "greentic:adaptive-cards/schema",   "version": "1.6.0" }
  ],
  "required": [
    { "id": "greentic:host/logging", "version": "^1.0.0" }
  ]
}
```

Both `offered` and `required` are arrays of capability refs:

```json
{
  "id":      "greentic:adaptive-cards/validate",
  "version": "1.0.0"
}
```

| Field | Type | Constraints |
|-------|------|-------------|
| `id` | string | Pattern: `^[a-z][a-z0-9-]*:[a-z][a-z0-9/._-]*$` |
| `version` | string | Semver for offered; semver range for required |

**ID format:** `<namespace>:<path>` where namespace is a publisher identifier
and path is a slash-separated capability path. Examples:

- `greentic:adaptive-cards/validate`
- `greentic:host/logging`
- `osora:digital-workers/spec`

**Matching rules** are described in [capability-registry.md](./capability-registry.md).

---

### `runtime`

```json
"runtime": {
  "component": "extension.wasm",
  "memoryLimitMB": 64,
  "permissions": {
    "network": ["https://api.example.com"],
    "secrets": ["secrets://demo/default/my-ext/*"],
    "callExtensionKinds": ["design"]
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `component` | string | Yes | Relative path to the WASM binary inside the `.gtxpack` |
| `memoryLimitMB` | integer | No | Max Wasmtime memory in MB. Range: 1-1024. Default: 64 |
| `permissions.network` | string[] | No | HTTPS origin allowlist. Unspecified = no network access. |
| `permissions.secrets` | string[] | No | Secret URI patterns the extension may read via the host `secrets` interface. |
| `permissions.callExtensionKinds` | enum[] | No | Extension kinds this extension may call via the broker. Values: `design`, `bundle`, `deploy`. |

**Default-deny:** All permission fields default to empty — the extension is
denied until explicitly granted. An extension that leaves `network` empty
cannot make any outbound HTTP calls even if it imports `greentic:extension-host/http`.

**`component` path** must point to a valid WebAssembly Component Model
binary (WASI Preview 2, `wasm32-wasip2`). The path is relative to the root
of the unpacked `.gtxpack` archive.

---

### `contributions`

The `contributions` object is kind-specific. Its schema depends on the
`kind` field.

#### `DesignExtension` contributions

```json
"contributions": {
  "schemas": ["schemas/adaptive-card-v1.6.json"],
  "prompts": ["prompts/rules.md", "prompts/examples.md"],
  "knowledge": ["knowledge/"],
  "assets": ["assets/icon.png"],
  "i18n": ["i18n/en.json", "i18n/ja.json"],
  "tools": [
    {
      "name": "validate_card",
      "export": "greentic:extension-design/validation.validate-content"
    },
    {
      "name": "analyze_card",
      "export": "greentic:extension-design/tools.invoke-tool"
    }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `schemas` | string[] | JSON Schema files contributed to the designer's schema registry |
| `prompts` | string[] | Markdown files injected into LLM system prompt |
| `knowledge` | string[] | Paths to knowledge base files or directories |
| `assets` | string[] | Static assets (icons, images) bundled with the extension |
| `i18n` | string[] | Locale files for extension-provided UI strings |
| `tools` | object[] | Named tools exposed to the LLM + designer UI |

Each `tools` entry:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Stable tool identifier used by the LLM |
| `export` | string | Yes | WIT export path: `<package>/<interface>.<function>` |

Tools can point to any exported WIT function. Common exports:
- `greentic:extension-design/validation.validate-content`
- `greentic:extension-design/tools.invoke-tool`
- `greentic:extension-design/knowledge.list-entries`
- `greentic:extension-design/knowledge.get-entry`
- `greentic:extension-design/knowledge.suggest-entries`

#### `BundleExtension` contributions

```json
"contributions": {
  "recipes": [
    {
      "id": "hosted-webchat",
      "displayName": "Hosted WebChat",
      "configSchema": "schemas/hosted-webchat-config.json",
      "supportedCapabilities": [
        "greentic:adaptive-cards/render",
        "greentic:webchat/direct-line"
      ]
    }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `recipes` | object[] | Yes | List of bundle recipes this extension provides |

Each recipe:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Machine identifier for the recipe |
| `displayName` | string | Yes | Display name shown in the designer UI |
| `configSchema` | string | No | Path to JSON Schema for recipe configuration |
| `supportedCapabilities` | string[] | No | Capabilities the generated pack will include |

#### `DeployExtension` contributions

```json
"contributions": {
  "targets": [
    {
      "id": "aws-eks",
      "displayName": "AWS EKS",
      "credentialSchema": "schemas/aws-credentials.json",
      "configSchema": "schemas/aws-eks-config.json"
    }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `targets` | object[] | Yes | List of deploy targets this extension provides |

Each target:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Machine identifier for the target |
| `displayName` | string | Yes | Display name shown in the deploy wizard |
| `credentialSchema` | string | No | Path to JSON Schema for credentials |
| `configSchema` | string | No | Path to JSON Schema for deployment config |

---

### `signature`

```json
"signature": {
  "algorithm": "ed25519",
  "publicKey": "AAAA...base64url",
  "value":     "BBBB...base64url"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `algorithm` | const string | Yes | Always `"ed25519"` |
| `publicKey` | string | Yes | Base64url-encoded Ed25519 public key of the signer |
| `value` | string | Yes | Base64url-encoded signature over the canonical JSON of `describe.json` minus the `signature` field |

Presence of `signature` is optional. When absent, the extension is treated
as unsigned. Whether unsigned extensions are accepted depends on the
configured trust policy (see [permissions-and-trust.md](./permissions-and-trust.md)).

`gtdx publish` adds the signature automatically from the developer's stored
key. `gtdx install` verifies it against the trust policy before allowing
installation.

---

## JSON Schema Reference

The canonical JSON Schema is embedded in the `greentic-ext-contract` crate
at `crates/greentic-ext-contract/schemas/describe-v1.json`. It uses JSON
Schema draft 2020-12 and is identified by the `$id`:

```
https://store.greentic.ai/schemas/describe-v1.json
```

You can reference it from your `describe.json` for editor tooling:

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  ...
}
```

---

## Complete Examples

### DesignExtension — Adaptive Cards

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.adaptive-cards",
    "name": "Adaptive Cards",
    "version": "1.6.0",
    "summary": "Design and validate Microsoft Adaptive Cards v1.6",
    "description": "Schema-level validation, prompts, and tool exports for AdaptiveCards v1.6.",
    "author": {
      "name": "Greentic",
      "email": "team@greentic.ai"
    },
    "license": "MIT",
    "repository": "https://github.com/greenticai/greentic-designer-extensions",
    "keywords": ["adaptive-cards", "ui", "microsoft"]
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "greentic:adaptive-cards/schema",   "version": "1.6.0" },
      { "id": "greentic:adaptive-cards/validate",  "version": "1.0.0" },
      { "id": "greentic:adaptive-cards/transform", "version": "1.0.0" }
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
    "schemas": ["schemas/adaptive-card-v1.6.json"],
    "prompts": ["prompts/rules.md", "prompts/examples.md"],
    "knowledge": ["knowledge/"],
    "tools": [
      { "name": "validate_card",       "export": "greentic:extension-design/validation.validate-content" },
      { "name": "analyze_card",        "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "check_accessibility", "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "optimize_card",       "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "transform_card",      "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "template_card",       "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "data_to_card",        "export": "greentic:extension-design/tools.invoke-tool" },
      { "name": "list_examples",       "export": "greentic:extension-design/knowledge.list-entries" },
      { "name": "get_example",         "export": "greentic:extension-design/knowledge.get-entry" },
      { "name": "suggest_layout",      "export": "greentic:extension-design/knowledge.suggest-entries" }
    ]
  }
}
```

### BundleExtension — Hosted WebChat

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "BundleExtension",
  "metadata": {
    "id": "greentic.hosted-webchat",
    "name": "Hosted WebChat Bundle",
    "version": "1.0.0",
    "summary": "Package designer output as a hosted WebChat Application Pack",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "greentic:bundle/hosted-webchat", "version": "1.0.0" }
    ],
    "required": [
      { "id": "greentic:adaptive-cards/render", "version": "^1.0.0" }
    ]
  },
  "runtime": {
    "component": "extension.wasm",
    "memoryLimitMB": 128,
    "permissions": {
      "network": [],
      "secrets": [],
      "callExtensionKinds": ["design"]
    }
  },
  "contributions": {
    "recipes": [
      {
        "id": "hosted-webchat-standard",
        "displayName": "Hosted WebChat (Standard)",
        "configSchema": "schemas/webchat-standard-config.json",
        "supportedCapabilities": [
          "greentic:adaptive-cards/render",
          "greentic:webchat/direct-line"
        ]
      },
      {
        "id": "hosted-webchat-enterprise",
        "displayName": "Hosted WebChat (Enterprise, SSO)",
        "configSchema": "schemas/webchat-enterprise-config.json",
        "supportedCapabilities": [
          "greentic:adaptive-cards/render",
          "greentic:webchat/direct-line",
          "greentic:auth/saml"
        ]
      }
    ]
  }
}
```

### DeployExtension — Desktop

```json
{
  "$schema": "https://store.greentic.ai/schemas/describe-v1.json",
  "apiVersion": "greentic.ai/v1",
  "kind": "DeployExtension",
  "metadata": {
    "id": "greentic.desktop-deploy",
    "name": "Desktop Deploy",
    "version": "0.1.0",
    "summary": "Deploy an Application Pack to local desktop for testing",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "MIT"
  },
  "engine": {
    "greenticDesigner": ">=0.6.0",
    "extRuntime": "^0.1.0"
  },
  "capabilities": {
    "offered": [
      { "id": "greentic:deploy/desktop", "version": "0.1.0" }
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
        "id": "local-runner",
        "displayName": "Local Desktop Runner",
        "credentialSchema": null,
        "configSchema": "schemas/desktop-config.json"
      }
    ]
  }
}
```

---

## Validation

Run `gtdx validate` to check a `describe.json` against the schema:

```
gtdx validate ./my-extension/
```

The command reads `./my-extension/describe.json`, validates it against the
embedded JSON Schema, and deserializes it to confirm all required fields
parse correctly.

Example output on success:

```
✓ ./my-extension/describe.json valid
```

Example output on failure:

```
error: schema validation failed:
  /metadata/id: string does not match pattern '^[a-z][a-z0-9.-]*\.[a-z0-9.-]+'
```

You can also run `gtdx doctor` to validate all installed extensions at once.

For CI integration add the validate step to your build pipeline:

```bash
gtdx validate ./
```

`gtdx validate` exits with code 0 on success, non-zero on any validation
failure. It is safe to run without an internet connection.
