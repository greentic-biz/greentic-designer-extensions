# How to Surface a WASM Component as a Canvas Node

> **Status: `gtdx new --kind wasm-component` does not produce a buildable
> project.** Verified 2026-08-21 against `gtdx 1.3.0-research.3`, and still
> true after greentic-designer-sdk#105 — that fix repaired every *other* kind,
> and deliberately left this one, because its `nodeTypes` entry points
> `runtime_ref` at the design component, which the runner cannot execute at
> all. `gtdx new` now warns about this kind rather than letting the first
> build deliver the news. Use
> `--kind design` and add the node component as a separate crate — the shape
> shipped extensions such as `greentic.calendly` actually use. Details in
> [What is broken](#what-is-broken-in---kind-wasm-component) below.

The goal this document describes is still real and still supported: you have
a WASM component implementing `greentic:component/component-v0-v6-v0@0.6.0`,
and you want it to appear as a node in the designer's canvas palette. Only
the convenience scaffold for it is unusable.

---

## The shape that works

A node in the palette needs **two** things in one `describe.json`:

1. a `runtime.components` entry for the component that will execute the node —
   built as its own crate against the `component-v0-v6-v0@0.6.0` world and
   published to OCI, or shipped in-pack;
2. a `contributions.nodeTypes` entry pointing at it by `runtime_ref`, carrying
   the palette metadata and the `operation` to invoke.

```json
"runtime": {
  "components": {
    "my-ext-tool": {
      "gtpack": { "file": "extension.wasm", "sha256": "…", "pack_id": "greentic.my-ext", "component_version": "0.1.0" },
      "sha256": "…",
      "world": "greentic:my-ext/design-extension@1.0.0"
    },
    "my-ext-node": {
      "oci_ref": "oci://ghcr.io/greenticai/component/component-my-ext@sha256:461c6a68…",
      "sha256": "…",
      "world": "greentic:component/component-v0-v6-v0@0.6.0"
    }
  }
},
"contributions": {
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

Start it with `gtdx new --kind design`, delete the tool contributions you do
not need, and add the node component as a second `runtime.components` entry.
The full walk-through, including the four things about a node that fail
silently, is
[how-to-write-a-design-extension.md § Step 10](./how-to-write-a-design-extension.md#step-10--optional-add-a-flow-editor-node).

---

## Why a design extension cannot execute the node itself

`greentic-runner-host` accepts a component only if it exports `node@0.5`,
`node@0.4`, or `component-runtime@0.6`. A design extension exports
`greentic:extension-design/tools@0.3.0`, which the runner has no path to. So
the node component is genuinely a **separate artifact** — the two-component
describe above is not boilerplate, it is the contract.

The node crate also may not import `greentic:extension-host/http` or
`extension-host/secrets`; those are design-world imports. Use
`greentic-interfaces-guest` with features
`["component-v0-6", "http-client-v1-1", "secrets"]`. A correct build shows
both in its world:

```
wasm-tools component wit <wasm>
# import greentic:http/http-client@1.1.0
# import greentic:secrets-store/secrets-store@1.0.0
```

---

## What is broken in `--kind wasm-component`

The generated project is written against an older contract and does not build
even after the WIT version rewrite that fixes the other kinds:

| Problem | Detail |
|---|---|
| WIT version | `extension/wit/world.wit` exports `greentic:extension-design/tools@0.1.0`; the vendored package is `@0.3.0` |
| WIT layout | the vendored `wit/deps/` sits at the project root, while `extension/Cargo.toml` targets `extension/wit` and declares no `target.dependencies`, so the packages never resolve |
| Inline table | `[package.metadata.component.target]` is written as an inline table, so a `target.dependencies` section cannot be appended without rewriting it (`cannot extend value of type inline table with a dotted key`) |
| Stub signature | `extension/src/lib.rs` uses `wit_bindgen::generate!` with `invoke_tool -> Result<String, String>`, whereas `@0.3.0` returns `result<string, extension-error>` |
| v1 instructions | `runtime/README.md` tells you to set `runtime.gtpack.file` — a v1 path. In v2 it is `runtime.components.<id>.gtpack.file` |
| Wrong runtime | `contributions.nodeTypes[0].runtime_ref` points at the **design** component, which cannot execute a node at all |

The last row is the one that matters most: even if the build were fixed, the
generated wiring points the node at a component the runner will refuse.

Fixing the scaffold means regenerating its template against the current
contract, not patching a generated project. Until then, `--kind design`
is the supported route.

---

## What to do next

- [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md) —
  the working path, including the node section.
- [describe-json-spec.md](./describe-json-spec.md#tools-and-node-types-are-different-surfaces)
  — why tools and node types are different surfaces.
- [getting-started-scaffolding.md](./getting-started-scaffolding.md#known-issue--a-fresh-scaffold-does-not-build)
  — per-kind scaffold status.
