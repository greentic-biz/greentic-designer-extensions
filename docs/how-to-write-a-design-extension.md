# How to Write a Design Extension

This tutorial builds a `DesignExtension` from an empty directory to a
published `.gtxpack`. It uses `greentic.calendly` as the running example,
because it exercises both surfaces a design extension can contribute: tools
for the agentic worker, and a palette node for the flow editor.

Everything below is against the **v2 describe contract**
(`apiVersion: "greentic.ai/v2"`). For the full field reference see
[describe-json-spec.md](./describe-json-spec.md).

---

## The one thing to read before anything else

**A design extension's tools are declared in `describe.json`, not in Rust.**

For a v2 extension, `ExtensionRuntime::list_tools` short-circuits on the
contract version and never calls the WASM `list-tools` export.
`contributions.tools[]` is the only source of a tool's name, description,
input schema and capabilities.

You still implement `invoke-tool` in Rust — dispatch is a real call into the
component. Only discovery moved into the manifest.

If you implement `list_tools()` in Rust and leave `contributions.tools`
empty, the extension builds, packs, installs and loads — and exposes **zero
tools**, with no error at any layer. This is the single most common way to
lose an afternoon here.

---

## Prerequisites

- **Rust 1.95 or later** (`rustup update stable`)
- **`wasm32-wasip2` target:**
  ```
  rustup target add wasm32-wasip2
  ```
- **`cargo-component`:**
  ```
  cargo install cargo-component --locked
  ```
- **`gtdx`** — the Greentic Extensions CLI.

### Check your `gtdx` before you start

`gtdx` embeds the describe contract, and every contract struct is
`deny_unknown_fields`. A `gtdx` older than the contract you are working
against rejects valid files with a field-level error that reads like your
mistake:

```
Error: unknown field `operation`, expected one of `type_id`, `label`, …
```

That message means the CLI is stale, not that the field is wrong. Verify
against a describe you know is current:

```
gtdx validate ~/.greentic/extensions/design/<any-installed-extension>/
```

Beware the version ordering: the `1.3.0-research.*` prerelease line branched
before several contract fields landed, so `1.3.0-research.1` sorts **above**
`1.2.0` by semver while being **older** in content. Build from a checkout you
can see rather than trusting the number:

```
cd greentic-designer-sdk && cargo install --path crates/greentic-extension-sdk-cli --force
```

---

## Step 1 — Scaffold

```
gtdx new my-extension --kind design --id greentic.my-extension
```

Omit the name (on a terminal) for an interactive wizard. Supported kinds:
`design`, `bundle`, `deploy`, `provider`, `wasm-component`, `mcp`, `llm`.

You get:

```
my-extension/
├── .gtdx-contract.lock     WIT contract version + file hashes
├── Cargo.toml
├── build.sh
├── ci/local_check.sh
├── describe.json           v2, contributions empty
├── i18n/en.json
├── prompts/system.md
├── src/lib.rs              every required export, stubbed
├── wit/world.wit
└── wit/deps/greentic/...   vendored WIT contract
```

*(Fixed in greentic-designer-sdk#105; still applies to any released `gtdx`.)* **The generated project does not build as-is.** Every kind but `mcp` renders a
`wit/world.wit` asking for `greentic:extension-host@0.2.0`, while the vendored
package is `@0.1.0` (and `extension-design` is `@0.3.0`, not `@0.2.0`). Rewrite
the versions before your first build:

```bash
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' \
       -e 's|\(greentic:extension-design/[a-z0-9-]*\)@0\.2\.0|\1@0.3.0|g' \
       wit/world.wit
```

Full analysis and the per-kind table:
[getting-started-scaffolding.md](./getting-started-scaffolding.md#known-issue--a-fresh-scaffold-does-not-build).

Two things to fix in the generated `describe.json` immediately:

- **Delete the `engine` block.** It is deprecated — `compat` is the sole
  source of version constraints — and `gtdx lint` errors on its presence
  (`E_ENGINE_DEPRECATED`).
- **Set a real `metadata.id`.** The default is `com.example.<name>`, which
  `gtdx lint` rejects (`E_ID_PATTERN` requires
  `^greentic\.[a-z0-9][a-z0-9-]*$`).

Neither blocks `gtdx publish`, which does not run lint — but both are real,
and the first one applies to every publisher, first-party or not.

### Seeding from an OpenAPI spec

For a REST integration, skip the hand-written dispatch entirely:

```
gtdx openapi ./calendly-openapi.yaml --name calendly
gtdx new my-mcp --kind mcp --from-openapi ./spec.yaml
```

---

## Step 2 — Know which surface you are building for

A design extension can contribute to two different runtimes, and they are
**not** interchangeable:

| You want | You declare | Executed by |
|---|---|---|
| A tool an Agentic Worker / playbook can call | `contributions.tools[]` | The design extension's own WASM, via `invoke-tool` |
| A node in the flow-editor palette | `contributions.nodeTypes[]` | A **separate** flow component, pinned by OCI digest |

`greentic-runner-host` accepts a component only if it exports `node@0.5`,
`node@0.4`, or `component-runtime@0.6`. A design extension exports
`greentic:extension-design/tools@0.2.0`, which the runner has no path to. So
`contributions.tools` alone can **never** produce a flow step, no matter how
it is declared.

Most of this tutorial covers the tool surface. Step 9 covers adding a node.

---

## Step 3 — Declare your tools in `describe.json`

Add entries under `contributions.tools`:

```json
"contributions": {
  "tools": [
    {
      "name": "calendly_me",
      "export": "greentic:extension-design/tools.invoke-tool",
      "runtime_ref": "my-extension",
      "description": "Look up the current Calendly user. The auth token is injected by the host and never returned.",
      "input_schema": "{\"type\":\"object\",\"required\":[\"operation\"],\"properties\":{\"operation\":{\"type\":\"string\",\"enum\":[\"get\"],\"description\":\"Which action to perform.\"}}}",
      "capabilities": ["agentic_worker"],
      "secret_requirements": [
        {
          "key": "calendly/token",
          "format": "text",
          "required": false,
          "description": "Personal Access Token. Resolved by the host from secret://calendly/token and never returned to the model."
        }
      ]
    }
  ]
}
```

Four fields decide whether the tool actually works:

- **`description`** — absent means the LLM sees a function with no
  explanation and cannot decide to call it.
- **`input_schema`** — a JSON Schema **serialized as a string**, not a JSON
  object. It becomes the function's `parameters`. Absent means the model
  cannot infer any arguments.
- **`capabilities`** — absent defaults to `["flow"]`, which **withholds the
  tool from the agentic-worker surface**: it will not appear in the DW
  Composer tool picker or in playbooks. If the tool is for a worker, say
  `["agentic_worker"]` explicitly.
- **`export`** — must be the fully-qualified
  `greentic:extension-design/<interface>.<member>`, never a bare
  `invoke-tool` (`E_EXPORT_FORM`).

`runtime_ref` names a key in `runtime.components`. With exactly one component
declared you may omit it; with more than one it is required, and a value that
does not match a declared component fails the parse.

Tool names must be `snake_case`, and `gtdx lint` also rejects near-duplicate
names where one is a prefix of another on a `_` boundary (`E_TOOL_NAMING`) —
`generate_pack` and `generate_pack_from_yaml` cannot coexist.

Optionally add planner hints:

```json
"agentic_worker_metadata": "{\"side_effects\":\"read\",\"cost\":\"low\",\"usage_hint\":\"Call before scheduling to resolve the acting user.\"}"
```

That field is a **string** too, holding a serialized object with
`usage_hint`, `examples`, `side_effects` (`none|read|write|external`), `cost`
(`low|medium|high`), and `confirmation_required`.

### Watch for the load-time warning

The runtime reports metadata gaps once per extension at load, at WARN,
naming the tools missing `description`, `input_schema` or `capabilities`.
Blank strings count as missing. Those lines are truthful defect reports — an
extension in that state installs cleanly and behaves as if the tools were not
there.

---

## Step 4 — Implement `invoke-tool`

The scaffold stubs every export. Only `invoke_tool` needs real logic for a
tool-only extension. Add `serde` / `serde_json` to `Cargo.toml` first — the
scaffold ships neither:

```toml
[dependencies]
wit-bindgen-rt = { version = "0.41", features = ["bitflags"] }
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
impl tools::Guest for Component {
    // v2: never called by the runtime. contributions.tools[] is the source
    // of truth. Kept because the WIT world requires the export.
    fn list_tools() -> Vec<tools::ToolDefinition> {
        Vec::new()
    }

    fn invoke_tool(name: String, args_json: String) -> Result<String, types::ExtensionError> {
        let args: serde_json::Value = serde_json::from_str(&args_json)
            .map_err(|e| types::ExtensionError::InvalidInput(e.to_string()))?;

        match name.as_str() {
            "calendly_me" => {
                // The host takes the full URI, and it must be permitted by
                // runtime.permissions.secrets. The error is a plain String.
                let token = bindings::greentic::extension_host::secrets::get(
                    "secret://calendly/token",
                )
                .map_err(|e| types::ExtensionError::Internal(format!("secret: {e}")))?;
                let body = fetch_me(&token, &args)?;
                Ok(serde_json::to_string(&body).unwrap_or_else(|_| "{}".into()))
            }
            other => Err(types::ExtensionError::InvalidInput(format!(
                "unknown tool: {other}"
            ))),
        }
    }
}
```

Keep the `name` strings in exact agreement with `contributions.tools[].name`.
Nothing checks that agreement: a mismatch surfaces as an `unknown tool` error
at call time, after the model has already chosen to call it.

**Never return secret material** from a tool. `message` and `detail` are
surfaced verbatim to operators, and tool output goes to the LLM.

---

## Step 5 — Permissions

Importing a host interface in `wit/world.wit` grants nothing. The permission
lists in `describe.json` are what grant it, and every one defaults to empty:

```json
"runtime": {
  "permissions": {
    "network": ["https://api.calendly.com/*"],
    "secrets": ["secret://calendly/token", "secret://calendly/auth_mode"],
    "callExtensionKinds": [],
    "llmRoles": [],
    "oauthProviders": []
  }
}
```

- `network` — HTTPS origin allowlist. Empty means no outbound HTTP even
  though the scaffold's world imports `greentic:extension-host/http`.
- `secrets` — must be **URIs**, not bare key names
  (`E_PERMS_SECRETS_PLAIN_KEY`), and the scheme is `secret://`, singular.
  **This list is not a glob**, unlike `network`: an entry permits a URI only
  when the URI equals it, or starts with the entry followed by `/`. A `*` is
  a literal asterisk and matches nothing, and a trailing slash breaks the
  prefix — `"secret://notion/"` permits `secret://notion//token`, not
  `secret://notion/token`. List the full URIs, or a prefix with no trailing
  slash (`"secret://notion"`).
- `llmRoles` — role wire names for `greentic:extension-host/llm`. With
  exactly one declared role, callers may omit `role-hint`; with several it is
  required.
- `oauthProviders` — provider ids for `greentic:oauth-broker/broker-v1`.

---

## Step 6 — Prompts, schemas, knowledge

These are `{ path }` objects in v2, not bare strings:

```json
"contributions": {
  "prompts":   [{ "path": "prompts/system.md" }],
  "schemas":   [{ "path": "schemas/my-content.json" }],
  "knowledge": [{ "path": "knowledge/" }]
}
```

Paths are relative to the `.gtxpack` root. Prompt fragments are also
returned from the WASM `prompting.system-prompt-fragments` export, which
**is** still called; the `contributions.prompts` list ships the files.

Locale catalogs go in the top-level `localization` block — v1's
`contributions.i18n` no longer exists.

---

## Step 7 — Build, install, iterate

```
gtdx dev
```

(If you have not applied the `wit/world.wit` version fix from Step 1, this is
where it fails — `gtdx dev` shells out to `cargo component build`.)

Watches the source, rebuilds, packs, and installs into
`~/.greentic/extensions/design/`. The designer's filesystem watcher picks the
new directory up and rebuilds its node-type registry in under a second — no
restart.

Useful variants:

```
gtdx dev --once            # build + install once (CI-friendly)
gtdx dev --no-install      # build and pack only
gtdx dev --force-rebuild   # cargo clean -p <crate> first
```

To install a one-off artifact by hand:

```
gtdx install ./dist/greentic.my-extension-0.1.0.gtxpack --trust loose
gtdx list
gtdx doctor
```

`--trust loose` accepts unsigned artifacts and is appropriate for local
development only.

---

## Step 8 — Validate and lint

Run both. They check different things.

```
gtdx validate ./          # JSON Schema + deserialize into the Rust contract
gtdx lint --dir ./        # cross-field governance rules
gtdx lint --dir ./ --publish   # + publish-only rules (rejects placeholder hashes)
```

`gtdx validate` is the one that catches real structural errors, because the
v2 JSON Schema types `contributions.tools` as `items: {}` — it checks that
`tools` is an array and nothing about what is in it. Only deserialization
into the Rust type validates a tool entry, and because every struct is
`deny_unknown_fields`, one misspelled key fails the **whole** describe and
takes the extension down.

`gtdx lint` is **not** wired into `gtdx publish`, nor into the scaffold's
`ci/local_check.sh`. Add it to your own CI if you want it enforced. Rule
table: [describe-json-spec.md](./describe-json-spec.md#gtdx-lint).

---

## Step 9 — Publish

```
gtdx publish --dry-run                       # build + pack + validate, no registry write
gtdx publish --registry local                # $GREENTIC_HOME/registries/local
gtdx publish --registry oci://ghcr.io/... --sign --key-id release-2026
```

`gtdx publish` builds the component itself, assembles the `.gtxpack`, and
writes it to `--dist` (default `./dist`). You do not hand it a path to a
pre-built pack. Signing keys come from `--key`, `--key-id`
(`~/.greentic/keys/<id>.key`), or `--key-env`
(`GREENTIC_EXT_SIGNING_KEY_PEM`) for CI.

For an externally produced component (e.g. one generated by the MCP path),
skip the build step and pack the given wasm:

```
gtdx publish --wasm ./target/wasm32-wasip2/release/my_component.wasm
```

Other users then install with:

```
gtdx install greentic.my-extension --version 0.1.0
```

---

## Step 10 — (Optional) Add a flow-editor node

A palette node needs a **second component**, built against
`greentic:component/component-v0-v6-v0@0.6.0` and published to OCI
independently. `gtdx publish` does not produce it.

Declare both components, then point each contribution at the right one:

```json
"runtime": {
  "components": {
    "my-ext-tool": {
      "gtpack": { "file": "extension.wasm", "sha256": "…", "pack_id": "greentic.my-extension", "component_version": "0.1.0" },
      "sha256": "…",
      "world": "greentic:my-extension/design-extension@1.0.0"
    },
    "my-ext-node": {
      "oci_ref": "oci://ghcr.io/greenticai/component/component-my-ext@sha256:461c6a68…",
      "sha256": "…",
      "world": "greentic:component/component-v0-v6-v0@0.6.0"
    }
  }
},
"contributions": {
  "tools": [{ "name": "my_op", "runtime_ref": "my-ext-tool", "export": "greentic:extension-design/tools.invoke-tool", "…": "…" }],
  "nodeTypes": [{
    "type_id": "my_op",
    "label": "My Operation",
    "category": "integration",
    "icon": "bolt",
    "color": "#6366f1",
    "complexity": "simple",
    "config_schema": "{\"type\":\"object\", …}",
    "output_ports": [{ "name": "default", "label": "Next" }],
    "runtime_ref": "my-ext-node",
    "operation": "my_op"
  }]
}
```

Four things about that node, each of which fails silently or late:

- **Pin `oci_ref` by digest.** A built pack embeds the ref permanently, and
  these registries do not publish tags in chronological order — the highest
  semver is frequently the oldest artifact.
- **`operation` is required by the runner** whenever the component exposes
  more than one. Without it the node is refused at execution time with
  "expected node.component.operation to be set", while the palette, the flow
  builder and the pack build all report success first.
- **One component backs many node types.** Ship one component and one
  `NodeType` per operation, differing only in `operation` and
  `config_schema`.
- **An extension node cannot be the first node of a flow.** Entry selection
  only picks a renderable node, so a non-render node at the head is stepped
  over. Lead with a card.

The node component may **not** import `greentic:extension-host/http` or
`extension-host/secrets` — those are design-world imports. Use
`greentic-interfaces-guest` with features
`["component-v0-6", "http-client-v1-1", "secrets"]` instead. A correct build
shows both in its world:

```
wasm-tools component wit <wasm>
# import greentic:http/http-client@1.1.0
# import greentic:secrets-store/secrets-store@1.0.0
```

Porting a tool to a flow node is also a **security** decision, not a
mechanical one: a worker tool sits behind that worker's guardrails and
credential gates, whereas a flow step is reachable from any flow against
whatever endpoint the node config names. A `confirm: true` argument
authorises nothing in a flow — it is a constant the flow author typed, with
no human present.

---

## Troubleshooting

| Symptom | Cause |
|---|---|
| Extension installs, exposes no tools | `contributions.tools` is empty; the WASM `list_tools()` is not read in v2 |
| Tool missing from the DW Composer picker | `capabilities` absent ⇒ defaults to `["flow"]`; declare `["agentic_worker"]` |
| LLM never calls the tool | `description` and/or `input_schema` absent — check the load-time WARN |
| `unknown field 'X'` from `gtdx validate` | Your `gtdx` is older than the contract; rebuild it from the SDK checkout |
| Whole extension fails to load after adding one field | `deny_unknown_fields` — one typo kills the entire describe |
| `unknown tool: X` at call time | `contributions.tools[].name` disagrees with the `match` arm in `invoke_tool` |
| Node runs nowhere / "expected node.component.operation to be set" | `operation` missing on a multi-operation component |
| No outbound HTTP despite importing the host `http` interface | `runtime.permissions.network` is empty (default-deny) |
| `permission denied for secret: …` at call time | `permissions.secrets` uses a `*` glob or a trailing slash; it is a verbatim / `/`-boundary prefix match |
| `gtdx lint` fails on a freshly scaffolded project | The scaffold still emits `engine` and `com.example.*` — delete the block, set a real id |
| `package 'greentic:extension-host@0.2.0' not found` | The scaffold's rendered `wit/world.wit` versions are wrong — see Step 1 |

---

## What to do next

- Add integration tests with `greentic-extension-sdk-testing`.
- Declare a `contributions.connection_test` so operators get a working "Test
  connection" button — it names a contributed tool and its arguments.
- For publishing with a permanent key and countersigning, see
  [permissions-and-trust.md](./permissions-and-trust.md).
- For the full manifest field reference, see
  [describe-json-spec.md](./describe-json-spec.md).
