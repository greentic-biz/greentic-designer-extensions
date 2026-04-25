# How to Write a WASM Component Extension

A `wasm-component` extension is a node-providing flavor of `DesignExtension`
that wraps a **pre-built WASM runtime component** (`.gtpack`) and surfaces it
as a single node in the designer canvas palette.

Use this scaffold when you already have a working WASM component shipped as
a `.gtpack` and just want to expose it to designer authors — without writing
the full `DesignExtension` tutorial code by hand.

The general design extension tutorial
([how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md))
covers the full surface (validation, prompting, knowledge, multi-tool). This
document focuses on the narrower wasm-component case.

---

## When to use this

Use `--kind wasm-component` when **all** of the following are true:

- You already have (or will produce separately) a runtime `.gtpack` that
  implements `greentic:component@0.6.0`.
- You want that runtime to appear as **one** node in the designer palette.
- You do not need to teach the designer a new content type, prompt fragments,
  or knowledge entries.

If you need richer authoring affordances (validate-as-you-type, system
prompt fragments, multi-tool LLM integration), use `--kind design` instead
and embed your `.gtpack` via `runtime.gtpack` as documented in
[how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md)
(Step 4a).

---

## Difference from a full design extension

| Aspect | `--kind design` | `--kind wasm-component` |
|--------|-----------------|-------------------------|
| Primary purpose | Teach a new content type to the designer | Add a single node to the canvas palette |
| Tools / prompting / knowledge | Implements rich exports | Stub only |
| `contributions.nodeTypes` | Optional | **Required** (one entry) |
| `runtime.gtpack` | Optional | **Required** |
| Scaffold layout | Single crate | Workspace with `extension/` + `runtime/` |
| Time to first node in designer | Hours | Minutes |

The wasm-component scaffold is essentially a curated subset of the design
extension scaffold, pre-wired for the "I have a `.gtpack`, give me a node"
use case.

---

## Prerequisites

- **Rust 1.94 or later** (`rustup update stable`)
- **`cargo-component`** — WIT-aware build tool for WASM components:
  ```
  cargo install --locked cargo-component
  ```
- **`wasm32-wasip2` target:**
  ```
  rustup target add wasm32-wasip2
  ```
- **`gtdx`** — the Greentic Designer Extensions CLI:
  ```
  cargo install --path crates/greentic-ext-cli --locked
  ```
- **A pre-built runtime `.gtpack`** — produced separately via
  `greentic-pack` (or whatever build pipeline ships your component). The
  scaffold does **not** build the runtime for you; it only wraps it.

---

## Step 1 — Scaffold

```
gtdx new myco.my-tool \
  --kind wasm-component \
  --author "My Org" \
  --node-type-id my-tool \
  --label "My Tool" \
  --dir ./my-tool \
  -y \
  --no-git
```

Flags specific to this kind:

- `--node-type-id <id>` — the `contributions.nodeTypes[0].type_id` value.
  Defaults to the last `.`-separated segment of `<name>` (so
  `myco.my-tool` → `my-tool`).
- `--label <text>` — the palette label shown to authors. Defaults to a
  humanized form of the derived `node_type_id` (so `my-tool` → `My Tool`).

All other flags (`--id`, `--version`, `--license`, `--author`, `--dir`,
`-y`, `--no-git`, `--force`) behave exactly as for the other kinds — see
the [CLI reference](./cli-reference.md).

---

## Step 2 — Inspect what you got

```
my-tool/
├── Cargo.toml                # workspace root, members = ["extension"]
├── README.md                 # quickstart for the new project
├── describe.json             # extension manifest, pre-wired with one nodeType
├── rust-toolchain.toml
├── .gitignore
├── .gtdx-contract.lock
├── extension/
│   ├── Cargo.toml            # extension crate (cdylib, wit-bindgen)
│   ├── src/lib.rs            # WASM guest exports
│   └── wit/world.wit         # imports + exports
├── runtime/
│   └── README.md             # placeholder; drop your .gtpack here
└── wit/deps/greentic/...     # vendored WIT contract
```

Two notable differences from the `design` scaffold:

- The project is a **Cargo workspace** with the WASM crate under
  `extension/`, leaving room for sibling crates (e.g., a host-side test
  harness) without restructuring later.
- A **`runtime/`** subdirectory is created up front for the pre-built
  `.gtpack`.

---

## Step 3 — Configure `describe.json`

The generated `describe.json` ships with sensible defaults, but you almost
certainly want to edit:

```json
"contributions": {
  "nodeTypes": [
    {
      "type_id": "my-tool",
      "label": "My Tool",
      "category": "tools",
      "icon": "puzzle",
      "color": "#0d9488",
      "complexity": "simple",
      "config_schema": "{}",
      "output_ports": [
        { "name": "success", "label": "Success" },
        { "name": "error", "label": "Error" }
      ]
    }
  ]
}
```

Things to fill in:

- **`category`** — designer palette grouping (`tools`, `integration`,
  `transform`, etc.).
- **`icon`** — palette icon name. The default `puzzle` is fine for a
  generic node.
- **`config_schema`** — a stringified JSON Schema describing the node's
  configuration form. The default `"{}"` accepts anything; replace it with
  the real schema your runtime expects.
- **`output_ports`** — adjust to match your runtime's actual output ports.
- **`runtime.permissions`** — declare any `network`, `secrets`, or
  `callExtensionKinds` your runtime needs. These are surfaced to the user
  on install.

---

## Step 4 — Drop in the runtime `.gtpack`

Copy your pre-built artifact into `runtime/`:

```
cp /path/to/my-tool-0.1.0.gtpack ./runtime/my-tool.gtpack
```

Update `describe.json` to point at it:

```json
"runtime": {
  "component": "extension.wasm",
  "gtpack": {
    "file": "runtime/my-tool.gtpack",
    "sha256": "REPLACE_AT_BUILD",
    "pack_id": "myco.my-tool",
    "component_version": "0.1.0"
  }
}
```

The `sha256` is filled in at build time by `gtdx publish` (it computes the
hash, rewrites the manifest, and seals the `.gtxpack`). You do not need to
compute it manually for local development.

---

## Step 5 — Build, install, iterate

`gtdx dev` watches the project, rebuilds the design-time `extension.wasm`,
re-signs locally, and reinstalls into your local `~/.greentic/extensions/`.
The designer hot-reloads the new node into the palette:

```
cd my-tool
gtdx dev
```

Open the designer in another terminal — the new node should appear in the
palette under the category you configured. Drag it onto the canvas, fill in
the config form (driven by your `config_schema`), and connect it to other
nodes.

---

## Step 6 — Publish

When the node behaves correctly end-to-end:

```
gtdx publish
```

This signs the artifact (filling in the `sha256` for the embedded `.gtpack`)
and uploads it to your default registry. Other users can then install it
with:

```
gtdx install myco.my-tool --version 0.1.0
```

See [getting-started-publish.md](./getting-started-publish.md) for the
full publish flow, signing-key management, and registry configuration.

---

## What to do next

- Add a real `config_schema` so authors get a typed configuration form
  instead of free-form JSON.
- Add multiple `output_ports` if your runtime branches (e.g., `match`,
  `no_match`).
- If you find yourself needing validation, prompt fragments, or knowledge
  entries, graduate to `--kind design` — the directory layouts are close
  enough that porting takes minutes.
- For trust policies and signing details, see
  [permissions-and-trust.md](./permissions-and-trust.md).
