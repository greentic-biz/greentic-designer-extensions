# describe.json — v2 Reference

## Overview

`describe.json` is the manifest for every Greentic Designer extension. It is
the single file the runtime reads before loading any WASM component. It
declares:

- Who the extension is (identity, version, license, author)
- What kind of extension it is (`DesignExtension`, `BundleExtension`,
  `DeployExtension`, `ProviderExtension`, `wasix:mcp/router`)
- Which designer / runner versions it is compatible with
- Which capabilities it offers and which it needs
- Which WASM components it ships, and where each one comes from
- What runtime resources (memory, network, secrets, LLM roles, OAuth
  providers) it is allowed to use
- What it contributes to the designer — **including the full definition of
  every tool it exposes**
- An optional Ed25519 signature for trust verification

**Schema location:** `crates/greentic-extension-sdk-contract/schemas/describe-v2.json`

**Canonical `$schema` URL:** `https://store.greentic.cloud/schemas/describe-v2.json`

**JSON Schema draft:** 2020-12

---

## Read this first: the JSON Schema is not the contract

The v2 JSON Schema types most of `contributions` as `{"type": "array",
"items": {}}` — it checks that `tools` is an array and nothing about what is
*in* it. The authoritative contract is the Rust type in
`greentic-extension-sdk-contract`, and **every struct in it is
`#[serde(deny_unknown_fields)]`**.

Two consequences that bite in practice:

- A single misspelled key anywhere fails the *whole* describe parse and takes
  the entire extension down — not just the field, and not just that tool.
- `gtdx validate` passing does **not** mean a field is well-formed. Only
  deserialization into the Rust type proves that, which is what the runtime
  and `gtdx install` do.

---

## Migrating from v1

If you have a v1 describe or a v1-era mental model, these are the breaking
differences:

| v1 | v2 | Notes |
|---|---|---|
| `apiVersion: "greentic.ai/v1"` | `apiVersion: "greentic.ai/v2"` | |
| *(none)* | `compat` **required** | `min_designer_version`, `min_runner_version`, `contract_version` |
| `engine` required | `engine` optional, **deprecated** | `gtdx lint` rejects it outright (`E_ENGINE_DEPRECATED`) |
| `runtime.component: "extension.wasm"` | `runtime.components: { "<id>": {…} }` | A map, **at least one entry required** |
| `runtime.gtpack` | `runtime.components["<id>"].gtpack` | Moved inside the component entry |
| `contributions.prompts: ["a.md"]` | `contributions.prompts: [{ "path": "a.md" }]` | Same for `schemas`, `knowledge` |
| `contributions.i18n: [...]` | *(removed)* | Use the top-level `localization` block |
| `tools: [{ name, export }]` | `tools: [{ name, export, description, input_schema, capabilities, … }]` | **This is the big one — see below** |
| tool metadata came from the WASM `list-tools` export | tool metadata comes from `describe.json` only | The runtime never calls `list-tools` for a v2 extension |

### The one that silently breaks extensions

For `apiVersion: "greentic.ai/v2"`, `ExtensionRuntime::list_tools`
short-circuits on the contract version and **never calls the WASM
`list-tools` export**. `contributions.tools[]` is the only source of a tool's
description, input schema, capabilities and agentic-worker metadata.

A field the describe omits is not filled in later — it is absent for the
tool's whole life. An extension that implements `list_tools()` in Rust and
leaves `contributions.tools` empty builds fine, installs fine, and exposes
**zero tools**, with no error at any layer.

You still implement `invoke-tool` in WASM: dispatch is a real call into the
component. Only *discovery* moved into the manifest.

At load time the runtime reports metadata gaps once per extension
(`tool_metadata_report`), at WARN. Those lines are truthful defect reports,
not noise — see [Tool metadata gaps](#tool-metadata-gaps).

---

## Top-Level Structure

```json
{
  "$schema": "https://store.greentic.cloud/schemas/describe-v2.json",
  "apiVersion": "greentic.ai/v2",
  "kind": "DesignExtension",
  "compat": { ... },
  "metadata": { ... },
  "capabilities": { ... },
  "runtime": { ... },
  "contributions": { ... },
  "localization": { ... },
  "signature": { ... }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `$schema` | string | No | Must equal the canonical URL exactly when present (`gtdx lint`: `E_SCHEMA_HOST`) |
| `apiVersion` | const string | Yes | `"greentic.ai/v2"` |
| `kind` | enum | Yes | See [`kind`](#kind) |
| `compat` | object | Yes | Minimum designer / runner versions + contract version |
| `metadata` | object | Yes | Identity, authorship, display info |
| `engine` | object | No | **Deprecated.** Superseded by `compat` |
| `capabilities` | object | Yes | Offered and required capability refs |
| `runtime` | object | Yes | Components, memory limit, permissions |
| `execution` | object | No | **Bundle extensions only** — rejected on any other kind |
| `contributions` | object | Yes | What the extension adds to the designer |
| `localization` | object | No | Locale catalogs for extension-provided strings |
| `signature` | object | No | Ed25519 signature for trust verification |
| `manifestSha256` | string | No | SHA-256 of the canonical `manifest.json`. Production packs MUST set it |
| `requiredSecrets` | array | No | Extension-wide secret requirements (vs. the per-tool list) |

---

## Field Reference

### `apiVersion`

```json
"apiVersion": "greentic.ai/v2"
```

The runtime rejects any `apiVersion` it does not recognize, and branches on
this value — notably for tool discovery (see above).

---

### `kind`

| Wire value | Install directory | Purpose |
|---|---|---|
| `DesignExtension` | `design/` | Teaches the designer to author a content type; contributes tools, node types, prompts, knowledge |
| `BundleExtension` | `bundle/` | Packages designer output into a deployable Application Pack |
| `DeployExtension` | `deploy/` | Ships Application Packs to a deployment target |
| `ProviderExtension` | `provider/` | Executes flow nodes against external services (messaging, webhooks, events) |
| `wasix:mcp/router` | `mcp/` | An MCP router artifact. Carries no `contributions` — its tools are discovered at runtime via `list-tools` |

---

### `compat`

```json
"compat": {
  "min_designer_version": ">=1.2.0",
  "min_runner_version": "^1.3.0-research.1",
  "contract_version": "1.3.0-research.1"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `min_designer_version` | semver **range** | Yes | Oldest Greentic Designer that can load this extension |
| `min_runner_version` | semver **range** | Yes | Oldest runner that can execute what it ships |
| `contract_version` | semver **version** | Yes | The exact contract release the descriptor was authored against |

All three are parsed eagerly, so an unparseable value fails at deserialize
time rather than when an installer tries to match.

`min_designer_version` is **not** the SDK version that generated the
extension. The floor for a v2 describe is designer `1.2.0` — the oldest
designer whose loader understands the v2 contract at all. Pinning it to the
generating SDK version declares a far stricter constraint than the extension
actually has.

---

### `metadata`

```json
"metadata": {
  "id": "greentic.adaptive-cards",
  "name": "Adaptive Cards",
  "version": "2.1.6-research",
  "summary": "Design and validate Microsoft Adaptive Cards",
  "description": "Long-form Markdown …",
  "author": { "name": "Greentic", "email": "team@greentic.ai" },
  "license": "Apache-2.0",
  "homepage": "https://greentic.ai",
  "repository": "https://github.com/greenticai/…",
  "keywords": ["adaptive-cards", "ui"],
  "icon": "assets/icon.png",
  "screenshots": ["assets/shot1.png"]
}
```

| Field | Type | Required | Constraints |
|-------|------|----------|-------------|
| `id` | string | Yes | Reverse-DNS, lowercase. `gtdx lint` additionally requires `^greentic\.[a-z0-9][a-z0-9-]*$` (`E_ID_PATTERN`) |
| `name` | string | Yes | Display name |
| `version` | string | Yes | Semver |
| `summary` | LocalizedString | Yes | One-liner |
| `description` | LocalizedString | No | Long-form Markdown |
| `author.name` | string | Yes | Publisher display name |
| `author.email` | string | No | Contact email |
| `author.publicKey` | string | No | Ed25519 public key |
| `license` | string | Yes | SPDX identifier |
| `homepage` / `repository` | string | No | URI |
| `keywords` | string[] | No | Search tags |
| `icon` | string | No | Path inside the `.gtxpack` |
| `screenshots` | string[] | No | Paths inside the `.gtxpack` |

**LocalizedString** accepts either form:

```json
"summary": "Design and validate Adaptive Cards"
```

```json
"summary": {
  "default": "Design and validate Adaptive Cards",
  "locales": { "ja": "Adaptive Cards を設計・検証", "id": "Rancang dan validasi Adaptive Cards" }
}
```

---

### `engine` — deprecated

```json
"engine": { "greenticDesigner": ">=1.2.0", "extRuntime": "^1.3.0-research.1" }
```

Still accepted by the parser for backward compatibility, and still emitted by
`gtdx new`. `compat` is the sole source of version constraints, and `gtdx
lint` errors on the block's mere presence (`E_ENGINE_DEPRECATED`). Delete it
from anything you author by hand.

---

### `capabilities`

```json
"capabilities": {
  "offered": [{ "id": "greentic:calendly/events", "version": "1.0.0" }],
  "required": [{ "id": "greentic:host/logging", "version": "^1.0.0" }]
}
```

| Field | Type | Constraints |
|-------|------|-------------|
| `id` | string | `^[a-z][a-z0-9-]*:[a-z][a-z0-9/._-]*$` |
| `version` | string | Exact semver for `offered`; a range for `required` |

Matching rules and cycle detection: [capability-registry.md](./capability-registry.md).

---

### `runtime`

```json
"runtime": {
  "memoryLimitMB": 64,
  "permissions": {
    "network": ["https://api.example.com"],
    "secrets": ["secret://my-ext"],
    "callExtensionKinds": ["design"],
    "llmRoles": ["sorla_composer"],
    "oauthProviders": ["hubspot"]
  },
  "components": {
    "my-ext-tool": {
      "gtpack": {
        "file": "extension.wasm",
        "sha256": "7312a747…",
        "pack_id": "com.example.my-ext",
        "component_version": "0.1.0"
      },
      "sha256": "7312a747…",
      "world": "com-example:my-ext/extension@1.0.0"
    }
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `memoryLimitMB` | integer | No | Wasmtime memory cap. **Must be in `[1, 1024]`**; default `64`. Enforced by the type, not only the schema |
| `permissions` | object | Yes | See below. Every list defaults to empty = denied |
| `world` | string | No | Top-level WIT world. Only `wasix:mcp/router` artifacts emit this; design extensions declare a world per component |
| `components` | map | Yes | **At least one entry.** Keys are component ids |

#### `runtime.permissions`

| Field | Type | Description |
|-------|------|-------------|
| `network` | string[] | HTTPS origin allowlist. Empty = no outbound HTTP, even if the component imports `greentic:extension-host/http` |
| `secrets` | string[] | Secret URIs readable via the host `secrets` interface. **Verbatim match or `/`-boundary prefix — not a glob.** `gtdx lint` rejects a bare key with no URI scheme (`E_PERMS_SECRETS_PLAIN_KEY`) |
| `callExtensionKinds` | string[] | Extension kinds callable via the broker: `design`, `bundle`, `deploy` |
| `llmRoles` | string[] | LLM role wire names requestable via `greentic:extension-host/llm`. Empty = the LLM import is denied. With exactly one role, `role-hint` may be omitted in requests; with several, it is required |
| `oauthProviders` | string[] | OAuth provider ids requestable via `greentic:oauth-broker/broker-v1`. Empty = the broker import is denied |

**Default-deny throughout.** Importing a host interface in your WIT world
grants nothing; the permission list is what grants it.

**`secrets` entries are not globs, and `network` entries are.** The two lists
use different matchers and the difference is easy to get backwards:

- `network` goes through the URL matcher, which strips a trailing `/*` — so
  `"https://api.calendly.com/*"` is the idiomatic form.
- `secrets` is a strict comparison: an entry permits a URI only when the URI
  **equals** it, or **starts with the entry followed by `/`**. A `*` is a
  literal asterisk and matches nothing.

That makes a trailing slash actively harmful. `"secret://notion/"` permits
`secret://notion//token`, not `secret://notion/token` — the code appends its
own separator. Declare either the full URIs:

```json
"secrets": ["secret://calendly/token", "secret://calendly/auth_mode"]
```

or a prefix with **no** trailing slash:

```json
"secrets": ["secret://notion"]
```

The scheme is `secret://`, singular.

#### `runtime.components.<id>`

Component ids are `[a-z0-9._-]+` — lowercase ASCII, digits, `.`, `_`, `-`.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `oci_ref` | string | see below | OCI reference to a component published outside this artifact. **Pin by digest** — a built pack embeds the ref permanently |
| `gtpack` | object | see below | A component shipped *inside* this `.gtxpack` |
| `sha256` | string | Yes | Lowercase hex, `^[0-9a-f]{64}$`. `gtdx publish` rejects an all-zero placeholder (`E_SHA256_ZERO`) |
| `world` | string | Yes | The WIT world the component exports |

**At least one of `oci_ref` / `gtpack` must be present**, or deserialization
fails.

`gtpack` fields: `file` (path inside the `.gtxpack`), `sha256`, `pack_id`,
`component_version`.

A single extension commonly declares **two** components with different
transports — one shipped in-pack for design-time tools, one pinned by OCI
digest for flow-node execution. See
[Tools and node types are different surfaces](#tools-and-node-types-are-different-surfaces).

---

### `execution`

Free-form object, permitted **only** when `kind` is `BundleExtension`.
Present on any other kind, deserialization fails with
`` `execution` is only allowed when kind=BundleExtension ``.

---

### `contributions`

Nine children, each its own typed list. All are optional and default to
empty; the JSON Schema requires the `contributions` object itself to exist.

Note the casing: the block's children are **camelCase** on the wire
(`nodeTypes`, `dwProviders`), except `connection_test`, which is snake_case
because that is what extensions and the designer already read.

| Field | Wire name | Type | Used by |
|---|---|---|---|
| Node types | `nodeTypes` | object[] | Design — flow-editor palette entries |
| Tools | `tools` | object[] | Design — tools offered to the LLM / agentic worker |
| Recipes | `recipes` | object[] | Bundle — packaging recipes |
| Knowledge | `knowledge` | object[] | Design — knowledge-base artifacts |
| Prompts | `prompts` | object[] | Design — Markdown prompt fragments |
| Schemas | `schemas` | object[] | Design — JSON Schema documents |
| DW providers | `dwProviders` | object[] | DW Composer provider cards |
| Guardrails | `guardrails` | object[] | Guardrail evaluators |
| Connection test | `connection_test` | object | The tool a consumer invokes to verify a live credential |

#### `contributions.tools[]`

```json
{
  "name": "calendly_me",
  "export": "greentic:extension-design/tools.invoke-tool",
  "runtime_ref": "calendly-tool",
  "description": "Look up the current Calendly user. The auth token is injected by the host and never returned.",
  "input_schema": "{\"type\":\"object\",\"required\":[\"operation\"],\"properties\":{\"operation\":{\"type\":\"string\",\"enum\":[\"get\"]}}}",
  "output_schema": "{\"type\":\"object\"}",
  "capabilities": ["agentic_worker"],
  "secret_requirements": [
    { "key": "calendly/token", "format": "text", "required": false,
      "description": "Personal Access Token, resolved by the host from secret://calendly/token." }
  ],
  "agentic_worker_metadata": "{\"side_effects\":\"read\",\"cost\":\"low\"}"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Stable tool id used by the LLM. `gtdx lint` requires `snake_case` and forbids near-duplicate names (`E_TOOL_NAMING`) |
| `export` | string | Yes | Fully-qualified `greentic:extension-design/<interface>.<member>` — never a bare member name (`E_EXPORT_FORM`) |
| `runtime_ref` | ComponentId | No | Which component to dispatch through. Absent ⇒ the sole declared component. Must exist in `runtime.components` |
| `description` | string | No | Shown to the LLM. **Absent ⇒ empty** — the model sees an unnamed function it cannot use |
| `input_schema` | string | No | JSON Schema **serialized as a string**, not an object. Surfaced as the function `parameters` |
| `output_schema` | string | No | JSON Schema, likewise a string |
| `capabilities` | string[] | No | `"flow"` and/or `"agentic_worker"`. **Absent ⇒ `["flow"]`** |
| `secret_requirements` | object[] | No | Credentials the tool needs. Each `key` is a path; the host URI is `secret://<key>` |
| `agentic_worker_metadata` | string | No | Serialized `AgenticWorkerMetadata` (see below) |

**`capabilities` is a product decision, not a formality.** Absent means
`["flow"]`, which *withholds* the tool from the agentic-worker surface — the
DW Composer tool picker and playbooks will not show it. Declare
`"agentic_worker"` explicitly when that is where the tool belongs.

**`input_schema` / `output_schema` are strings.** The v2 JSON Schema types
`tools` as `items: {}`, so passing a JSON *object* here is caught by nothing
until the Rust deserializer refuses it — and the refusal fails the whole
describe.

`agentic_worker_metadata` decodes to:

| Field | Type | Values |
|---|---|---|
| `usage_hint` | string | Free text for the planner |
| `examples` | object[] | `{ "when": "…", "input": { … } }` |
| `side_effects` | enum | `none`, `read`, `write`, `external` |
| `cost` | enum | `low`, `medium`, `high` |
| `confirmation_required` | bool | |

#### `contributions.nodeTypes[]`

```json
{
  "type_id": "calendly_me",
  "label": "Calendly Me",
  "category": "integration",
  "icon": "calendar",
  "color": "#6366f1",
  "complexity": "simple",
  "config_schema": "{\"type\":\"object\", …}",
  "output_ports": [{ "name": "default", "label": "Next" }],
  "runtime_ref": "calendly-node",
  "operation": "calendly_me"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type_id` | string | Yes | Stable palette id |
| `label` | LocalizedString | Yes | Display name |
| `category` | string | Yes | Palette grouping |
| `icon` | string | Yes | Icon id |
| `color` | string | Yes | Hex colour |
| `complexity` | string | Yes | e.g. `simple` |
| `config_schema` | string | Yes | JSON Schema **as a string** |
| `output_ports` | object[] | No | `{ name, label }` per outgoing port |
| `runtime_ref` | ComponentId | No | Component that executes the node. Must exist in `runtime.components` |
| `operation` | string | No | Named operation on that component |
| `deprecated` | object | No | Deprecation notice |

**`operation` fails late when it is wrong.** The designer compiles a node
type into a `component.exec` body of `{ component, operation, input }`, and
the runner *requires* `operation` — it refuses the node with "expected
node.component.operation to be set". Leaving it unset on a multi-operation
component therefore fails at execution time only: the palette, the flow
builder and the pack build all report success first. Omit it only for a
component that exposes exactly one operation.

One component backs many palette entries: ship one component and one
`NodeType` per operation, differing only in `operation` and `config_schema`.

#### `contributions.prompts[]`, `knowledge[]`, `schemas[]`

Each entry is an object, not a bare string:

```json
"prompts":   [{ "path": "prompts/rules.md" }],
"knowledge": [{ "path": "knowledge/" }],
"schemas":   [{ "path": "schemas/my-content.json" }]
```

`path` is relative to the `.gtxpack` root.

#### `contributions.recipes[]`

```json
{
  "id": "hosted-webchat-standard",
  "display_name": "Hosted WebChat (Standard)",
  "description": "Optional long form",
  "config_schema": "schemas/webchat-standard-config.json"
}
```

`display_name` and `description` are LocalizedStrings. `config_schema` is a
path, unlike `NodeType.config_schema`, which is inline JSON Schema text.

#### `contributions.guardrails[]`

```json
{ "export": "greentic:extension-design/guardrail.evaluate", "runtime_ref": "my-guardrail" }
```

`runtime_ref` absent ⇒ the runtime selects the sole declared component.

#### `contributions.dwProviders[]`

| Field | Type | Required |
|---|---|---|
| `provider_id` | string | Yes |
| `family` | string | Yes |
| `display_name` | string | Yes |
| `summary` | string | No |
| `version` | string | Yes |
| `channel` | string | Yes |
| `capability_contract_ids` | string[] | Yes |
| `question_blocks` | object[] | No |
| `template_compatibility` | string[] | No |

`question_blocks` entries are raw JSON objects (`{ block_id, kind, … }`),
matching the designer's untyped `question_blocks`.

#### `contributions.connection_test`

```json
"connection_test": { "tool": "calendly_me", "args": { "operation": "get" } }
```

Names a contributed tool the designer invokes behind its "Test connection"
button. `args` absent is treated as `{}`.

Two contract notes: the probe runs against the operator's **draft (unsaved)
secrets only**, so an admin-managed credential that was not re-typed resolves
empty; and `message` / `detail` are surfaced verbatim to the operator, so a
probe tool must never emit secret material into its output or error text.

---

### `localization`

Locale catalogs for extension-provided strings. Replaces v1's
`contributions.i18n` list.

---

### `signature`

```json
"signature": {
  "algorithm": "ed25519",
  "publicKey": "AAAA…base64url",
  "value": "BBBB…base64url",
  "keyId": "release-2026"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `algorithm` | const string | Yes | Always `"ed25519"` |
| `publicKey` | string | Yes | Base64url Ed25519 public key of the signer |
| `value` | string | Yes | Base64url signature over the canonical JSON of `describe.json` minus `signature` |
| `keyId` | string | No | Label identifying which key signed |

Absent ⇒ unsigned. Whether unsigned extensions install depends on the trust
policy — see [permissions-and-trust.md](./permissions-and-trust.md).

`manifestSha256` binds the whole-archive ledger into the signed describe.
Production packs must set it.

---

## Cross-field invariants

These are enforced at deserialize time, so violating one fails the parse
rather than surfacing later:

1. `runtime.components` must declare **at least one** entry.
2. `runtime.memoryLimitMB` must be in `[1, 1024]`.
3. `execution` is present only when `kind` is `BundleExtension`.
4. Every `runtime_ref` in `contributions.nodeTypes` must name a key in
   `runtime.components`.
5. Every `runtime_ref` in `contributions.tools` must name a key in
   `runtime.components`.
6. Every `RuntimeComponent` carries at least one of `oci_ref` / `gtpack`.
7. Every `sha256` is lowercase hex (uppercase `A-F` is refused, matching the
   schema pattern).

---

## Tools and node types are different surfaces

A design extension's tools reach the **agentic worker**, not the flow
runtime. `greentic-runner-host` accepts a component only if it exports
`node@0.5` / `node@0.4` / `component-runtime@0.6`; a design extension exports
`greentic:extension-design/tools@0.2.0`, which the runner has no path to. So
`contributions.tools` alone can never produce a flow step.

Shipping both from one extension means declaring two components:

```json
"runtime": {
  "components": {
    "calendly-tool": {
      "gtpack": { "file": "extension.wasm", "sha256": "…", "pack_id": "greentic.calendly", "component_version": "1.2.0-research" },
      "sha256": "…",
      "world": "greentic:calendly-extension/design-extension@1.0.0"
    },
    "calendly-node": {
      "oci_ref": "oci://ghcr.io/greenticai/component/component-calendly@sha256:461c6a68…",
      "sha256": "…",
      "world": "greentic:component/component-v0-v6-v0@0.6.0"
    }
  }
}
```

`contributions.tools[].runtime_ref` points at `calendly-tool`;
`contributions.nodeTypes[].runtime_ref` points at `calendly-node` and carries
an `operation`. The node component is a **separate crate**, built against the
`greentic:component/component-v0-v6-v0@0.6.0` world and published to OCI on
its own — it is not produced by `gtdx publish`.

---

## Tool metadata gaps

At load time the runtime scans a v2 extension's tools and emits one WARN per
extension naming the tools missing `description`, `input_schema` or
`capabilities`. The two consequences differ:

- missing `description` / `input_schema` → the LLM cannot call the tool
  correctly;
- missing `capabilities` → defaults to `["flow"]`, withholding the tool from
  the agentic-worker surface entirely.

Blank strings count as missing. These warnings are the only signal you get
for a class of defect that otherwise looks like a working install.

---

## Validation

Two commands, checking different things. Run both.

### `gtdx validate`

```
gtdx validate ./my-extension/
```

Reads `./my-extension/describe.json`, validates it against the embedded JSON
Schema, and deserializes it into the Rust contract. Exit code 0 on success,
non-zero on failure. Offline-safe.

Because the schema is permissive inside `contributions`, most real errors
surface from the deserialize step, as a serde message:

```
Error: unknown field `operation`, expected one of `type_id`, `label`, …
```

That particular message means **your `gtdx` is older than the describe you
are validating**, not that the field is wrong. `deny_unknown_fields` makes an
old CLI reject a newer contract. Rebuild `gtdx` from a matching SDK before
believing a field-level error.

`gtdx doctor` runs the same check across every installed extension.

### `gtdx lint`

```
gtdx lint --dir ./my-extension          # authoring rules
gtdx lint --dir ./my-extension --publish # + publish-only rules
```

Cross-field governance rules beyond the schema. **It is not wired into
`gtdx publish` or into the scaffold's `ci/local_check.sh`** — invoke it
yourself, or wire it into your own CI.

| Code | Rule |
|---|---|
| `E_SCHEMA_HOST` | `$schema` must equal `https://store.greentic.cloud/schemas/describe-v2.json` exactly |
| `E_VERSION_SEMVER` | `metadata.version` must be valid semver |
| `E_ID_PATTERN` | `metadata.id` must match `^greentic\.[a-z0-9][a-z0-9-]*$` |
| `E_ENGINE_DEPRECATED` | The `engine` block must be absent |
| `E_RUNTIME_REF` | Every `runtime_ref` must resolve to a declared component |
| `E_CAP_CYCLE` | Capability graph must be acyclic |
| `E_EXPORT_FORM` | Each tool `export` must be `greentic:extension-design/<interface>.<member>` |
| `E_TOOL_NAMING` | Tool names must be `snake_case`, with no near-duplicate prefixes |
| `E_PERMS_SECRETS_PLAIN_KEY` | `permissions.secrets` entries must be URIs, not bare keys |
| `E_SECRET_KEY_NOT_CANONICAL` | Secret keys must be in canonical form |
| `E_SHA256_ZERO` | *(publish only)* No placeholder all-zero component hash |
| `W_DESCRIBE_DIFF_BREAKING` | The describe changed incompatibly against the previous version |

Two of these fire on a **freshly scaffolded** project, because `gtdx new`
still emits an `engine` block and defaults `metadata.id` to
`com.example.<name>`: `E_ENGINE_DEPRECATED` and `E_ID_PATTERN`. Neither
blocks `gtdx publish`. Delete the `engine` block regardless; the id rule
only binds first-party `greentic.*` extensions.

---

## Complete Examples

### DesignExtension — tools + a flow node

```json
{
  "$schema": "https://store.greentic.cloud/schemas/describe-v2.json",
  "apiVersion": "greentic.ai/v2",
  "kind": "DesignExtension",
  "compat": {
    "min_designer_version": ">=1.2.0",
    "min_runner_version": "^1.3.0-research.1",
    "contract_version": "1.3.0-research.1"
  },
  "metadata": {
    "id": "greentic.calendly",
    "name": "Calendly",
    "version": "1.3.0",
    "summary": "Read Calendly users, event types and scheduled events",
    "author": { "name": "Greentic", "email": "team@greentic.ai" },
    "license": "Apache-2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:calendly/events", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 64,
    "permissions": {
      "network": ["https://api.calendly.com"],
      "secrets": ["secret://calendly/token"],
      "callExtensionKinds": []
    },
    "components": {
      "calendly-tool": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "7312a747e7c1fdb50fabaecd442f5592f3864be4cdb784a7c0d1dfbec1478dcb",
          "pack_id": "greentic.calendly",
          "component_version": "1.2.0-research"
        },
        "sha256": "7312a747e7c1fdb50fabaecd442f5592f3864be4cdb784a7c0d1dfbec1478dcb",
        "world": "greentic:calendly-extension/design-extension@1.0.0"
      },
      "calendly-node": {
        "oci_ref": "oci://ghcr.io/greenticai/component/component-calendly@sha256:461c6a68db12b1148465010589c7a8447bf5da9b9de358e4ae0758178801b959",
        "sha256": "0a9a6e931a531ae4814c33f68179a335cba7c82e77b3ceb27326418cba784f0e",
        "world": "greentic:component/component-v0-v6-v0@0.6.0"
      }
    }
  },
  "contributions": {
    "tools": [
      {
        "name": "calendly_me",
        "export": "greentic:extension-design/tools.invoke-tool",
        "runtime_ref": "calendly-tool",
        "description": "Look up the current Calendly user.",
        "input_schema": "{\"type\":\"object\",\"required\":[\"operation\"],\"properties\":{\"operation\":{\"type\":\"string\",\"enum\":[\"get\"]}}}",
        "capabilities": ["agentic_worker"],
        "secret_requirements": [
          { "key": "calendly/token", "format": "text", "required": false }
        ]
      }
    ],
    "nodeTypes": [
      {
        "type_id": "calendly_me",
        "label": "Calendly Me",
        "category": "integration",
        "icon": "calendar",
        "color": "#6366f1",
        "complexity": "simple",
        "config_schema": "{\"type\":\"object\",\"required\":[\"operation\"],\"properties\":{\"operation\":{\"type\":\"string\",\"enum\":[\"get\"]}}}",
        "output_ports": [{ "name": "default", "label": "Next" }],
        "runtime_ref": "calendly-node",
        "operation": "calendly_me"
      }
    ],
    "connection_test": { "tool": "calendly_me", "args": { "operation": "get" } }
  }
}
```

### BundleExtension

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
    "id": "greentic.bundle-standard",
    "name": "Standard Bundle",
    "version": "1.3.1-research",
    "summary": "Package designer output as a standard Application Pack",
    "author": { "name": "Greentic" },
    "license": "Apache-2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:bundle/standard", "version": "1.0.0" }],
    "required": [{ "id": "greentic:adaptive-cards/render", "version": "^1.0.0" }]
  },
  "runtime": {
    "memoryLimitMB": 128,
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": ["design"] },
    "components": {
      "bundle-standard": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "9d4e9a4068e11330af5b388de4fb30f1b50cd9771047a8e6b5f5edfccc694b4b",
          "pack_id": "greentic.bundle-standard",
          "component_version": "1.3.1-research"
        },
        "sha256": "9d4e9a4068e11330af5b388de4fb30f1b50cd9771047a8e6b5f5edfccc694b4b",
        "world": "greentic:extension-bundle/bundle-extension@1.0.0"
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

### DeployExtension

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
    "id": "greentic.deploy-desktop",
    "name": "Desktop Deploy",
    "version": "1.3.1-research",
    "summary": "Deploy an Application Pack to the local desktop for testing",
    "author": { "name": "Greentic" },
    "license": "Apache-2.0"
  },
  "capabilities": {
    "offered": [{ "id": "greentic:deploy/desktop", "version": "1.0.0" }],
    "required": []
  },
  "runtime": {
    "memoryLimitMB": 32,
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] },
    "components": {
      "deploy-desktop": {
        "gtpack": {
          "file": "extension.wasm",
          "sha256": "cfaf8746ecd1c357d13a6a0ae5832441034bc35e0d63e7f94f5d85daf9811ba3",
          "pack_id": "greentic.deploy-desktop",
          "component_version": "1.3.1-research"
        },
        "sha256": "cfaf8746ecd1c357d13a6a0ae5832441034bc35e0d63e7f94f5d85daf9811ba3",
        "world": "greentic:extension-deploy/deploy-extension@1.0.0"
      }
    }
  },
  "contributions": {}
}
```
