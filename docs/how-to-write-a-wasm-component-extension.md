# How to Surface a WASM Component as a Canvas Node

You have a WASM component implementing
`greentic:component/component-v0-v6-v0@0.6.0`, and you want it to appear as a
node in the designer's flow-editor palette. `--kind wasm-component` scaffolds
exactly that.

```
gtdx new my-node --kind wasm-component --id greentic.my-node \
    --component-ref oci://ghcr.io/greenticai/component/component-my-node@sha256:461c6a68…
```

> **Requires a `gtdx` carrying greentic-designer-sdk#106.** Before that commit
> this kind produced a project that did not build, and whose node pointed at a
> component that could not execute it. See
> [What #106 changed](#what-106-changed) if you are on an older build.

---

## What the scaffold produces

A single crate — the same layout as `--kind design` — plus a `describe.json`
declaring **two** components:

| component | what it is |
|---|---|
| `<key>` | this crate's design-time `extension.wasm`, shipped inside the `.gtxpack` |
| `<key>-node` | the component that **executes** the node, referenced by `oci_ref` |

```json
"runtime": {
  "components": {
    "my-node": {
      "gtpack": { "file": "extension.wasm", "sha256": "…", "pack_id": "greentic.my-node", "component_version": "0.1.0" },
      "sha256": "…",
      "world": "greentic:my-node/extension@1.0.0"
    },
    "my-node-node": {
      "oci_ref": "oci://ghcr.io/greenticai/component/component-my-node@sha256:461c6a68…",
      "sha256": "…",
      "world": "greentic:component/component-v0-v6-v0@0.6.0"
    }
  }
},
"contributions": {
  "nodeTypes": [{
    "type_id": "my_node",
    "label": "My Node",
    "category": "tools",
    "icon": "puzzle",
    "color": "#0d9488",
    "complexity": "simple",
    "config_schema": "{}",
    "output_ports": [
      { "name": "on_success", "label": "Success" },
      { "name": "on_error", "label": "Error" }
    ],
    "runtime_ref": "my-node-node",
    "operation": "my_node"
  }]
}
```

`contributions.nodeTypes[0].runtime_ref` points at the **node** component, not
at the extension's own wasm.

---

## The node component must be reachable by `oci_ref`

Not by a local `.gtpack`, and this is not a style preference — two layers
disagree with the local shape, both silently:

- the designer's flow compiler resolves a node through
  `runtime.components.<runtime_ref>.oci_ref` and **skips a `gtpack`-only
  component**, falling through to the catalog pin. Its own comment says so:
  *"If there is no oci_ref (gtpack-only), fall through to catalog pin."*
- `post_install_provider` relocates a nested `.gtpack` into the runner's pack
  directory **only when `kind == ExtensionKind::Provider`**.

So a node backed by an in-pack `.gtpack` builds, packs, installs — and runs
nothing.

Nor can the design extension execute the node itself: `greentic-runner-host`
accepts a component only if it exports `node@0.5`, `node@0.4`, or
`component-runtime@0.6`, and a design extension exports
`greentic:extension-design/tools@0.3.0`.

---

## Four things that fail late

- **Pin `oci_ref` by digest.** A built pack embeds the ref permanently, and
  these registries publish tags out of chronological order — the highest
  semver is frequently the oldest artifact.
- **`operation` is required** whenever the component exposes more than one.
  Without it the runner refuses the node with "expected
  node.component.operation to be set", while the palette, the flow builder and
  the pack build all report success first. The scaffold defaults it to the
  node's `type_id`; change it if your component names the operation
  differently.
- **One component backs many node types.** Ship one component and one
  `NodeType` per operation, differing only in `operation` and `config_schema`.
  Add the extra entries by hand — the scaffold writes one.
- **An extension node cannot be the first node of a flow.** Entry selection
  only picks a renderable node, so a non-render node at the head is stepped
  over. Lead with a card.

Omitting `--component-ref` is allowed: the scaffold writes an
`example.invalid` placeholder with a zero digest. It builds, and
`gtdx lint --publish` refuses it (`E_SHA256_ZERO`), so you cannot publish it
by accident.

---

## Building the node component itself

That is a separate crate, and `gtdx` does not build it. It may **not** import
`greentic:extension-host/http` or `extension-host/secrets` — those are
design-world imports. Use `greentic-interfaces-guest` with features
`["component-v0-6", "http-client-v1-1", "secrets"]`. A correct build shows
both in its world:

```
wasm-tools component wit <wasm>
# import greentic:http/http-client@1.1.0
# import greentic:secrets-store/secrets-store@1.0.0
```

Porting a tool to a flow node is also a **security** decision, not a
mechanical one: a worker tool sits behind that worker's guardrails and
credential gates, whereas a flow step is reachable from any flow against
whatever endpoint the node config names. A `confirm: true` argument
authorises nothing in a flow — it is a constant the flow author typed, with no
human present.

---

## What this crate's own wasm is for

Design-time only: validation, prompt fragments, knowledge entries, and any
design-time tools you declare in `contributions.tools`. It never executes the
node. A fresh scaffold stubs all of it — leave it as-is if the node needs no
authoring affordances.

---

## What #106 changed

Before greentic-designer-sdk#106 this kind was unusable, and worth recording
because the failure was invisible at every layer that could have caught it:

| Problem | Detail |
|---|---|
| Wrong runtime | `nodeTypes[0].runtime_ref` pointed at the **design** component, which the runner cannot execute |
| Unreachable premise | the generated `runtime/README.md` told you to drop a local `.gtpack` in — a shape neither the compiler nor the installer supports for a DesignExtension |
| WIT version | `extension/wit/world.wit` exported `tools@0.1.0` against a `@0.3.0` package |
| WIT layout | vendored deps sat at the project root while `extension/Cargo.toml` targeted `extension/wit` |
| Inline table | `[package.metadata.component.target]` was an inline table, so `target.dependencies` could not be appended |
| Stub signature | `invoke_tool -> Result<String, String>` where the contract returns `result<string, extension-error>` |

The two-crate workspace (`extension/` + `runtime/`) is gone; the kind now
overlays the design templates and overrides only `describe.json` and
`README.md`.

---

## See also

- [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md) —
  the full design-extension surface, including tools.
- [describe-json-spec.md](./describe-json-spec.md#tools-and-node-types-are-different-surfaces)
  — why tools and node types are different surfaces.
