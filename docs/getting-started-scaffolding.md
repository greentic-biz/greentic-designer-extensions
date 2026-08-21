# Scaffolding a new extension

`gtdx new` generates a ready-to-build Greentic Designer Extension project
against the v2 describe contract.

## Prerequisites

- Rust 1.95+ with the `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `cargo-component`: `cargo install --locked cargo-component`
- **`gtdx` 1.2.1 or newer.** 1.2.1 is the first release whose scaffolds
  build: every earlier one, `1.2.0` included, generates a project that fails
  its first `cargo component build` for every kind except `mcp`. See
  [If you are on gtdx 1.2.0 or older](#if-you-are-on-gtdx-120-or-older).

  ```
  cargo install greentic-extension-sdk-cli --locked   # or: cargo binstall …
  gtdx --version
  ```

  Do not opt into `-research` prereleases on crates.io to get "newer":
  `1.3.0-research.1` sorts above `1.2.1` by semver and is **older** in
  content.

## Create

```
gtdx new my-ext --kind design --id greentic.my-ext
```

Run `gtdx new` with no name on a terminal for an interactive wizard, or pass
`-y` to resolve everything from flags and defaults.

Supported kinds: `design`, `bundle`, `deploy`, `provider`, `wasm-component`,
`mcp`, `llm`.

Seed a connector from an existing API description instead of the empty
skeleton:

```
gtdx new my-mcp --kind mcp --from-openapi ./spec.yaml
gtdx openapi ./spec.yaml --name my-connector
```

## What you get

```
my-ext/
├── .gtdx-contract.lock     WIT contract version + file hashes
├── Cargo.toml
├── README.md · CLAUDE.md · AGENTS.md
├── build.sh
├── ci/local_check.sh
├── describe.json           apiVersion greentic.ai/v2, contributions empty
├── i18n/en.json
├── prompts/system.md
├── src/lib.rs              every required WIT export, stubbed
└── wit/world.wit + wit/deps/greentic/...
```

## If you are on `gtdx` 1.2.0 or older

Upgrade — everything in this section is fixed in 1.2.1
(greentic-designer-sdk#105 and #106). It is kept because the failures are
opaque, and pinned toolchains are real.

**Every kind except `mcp` fails `cargo component build` (and therefore
`gtdx dev`) on a completely untouched scaffold**, with:

```
failed to merge local target .../wit
package 'greentic:extension-host@0.2.0' not found. known packages:
    greentic:extension-host@0.1.0
    greentic:extension-base@0.2.0
    greentic:extension-design@0.3.0
```

The cause was in `templates/*/wit/world.wit.tmpl`: every import and export
rendered with a single `{{contract_version}}` placeholder, while the embedded
WIT packages are versioned **independently** —

| package | version |
|---|---|
| `greentic:extension-base` | 0.2.0 |
| `greentic:extension-host` | **0.1.0** |
| `greentic:extension-design` | **0.3.0** |
| `greentic:extension-bundle` / `-deploy` / `-provider` | 0.2.0 |

so the rendered world asked for `extension-host@0.2.0`, which has never
existed.

### Per-kind status on ≤ 1.2.0

| `--kind` | Builds as generated | Workaround |
|---|---|---|
| `mcp` | **yes** | none — it imports no greentic WIT package |
| `bundle`, `deploy` | no | rewrite `extension-host@0.2.0` → `@0.1.0` in `wit/world.wit` |
| `design` | no | same, **plus** `extension-design@0.2.0` → `@0.3.0` |
| `llm` | no | same as `design`, **plus** its `describe.json` carries an `id` key on the tool entry that `Tool` does not model — `deny_unknown_fields` fails the whole describe. Upgrade rather than patch |
| `provider` | no | host version, **plus** `provider_types::Error` → `types::ExtensionError` in `src/lib.rs` (the WIT declares `extension-error`, from `extension-base/types`) |
| `wasm-component` | no | **no workaround** — its node pointed at a component the runner cannot execute. Upgrade; see [how-to-write-a-wasm-component-extension.md](./how-to-write-a-wasm-component-extension.md#what-121-changed) |

For every kind but `provider` and `wasm-component`:

```bash
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' \
       -e 's|\(greentic:extension-design/[a-z0-9-]*\)@0\.2\.0|\1@0.3.0|g' \
       wit/world.wit
```

Then `cargo component build` succeeds. Confirmed for `design`, `bundle`,
`deploy`, and (with the extra `src/lib.rs` edit) `provider`.

On these versions you must also **delete the `engine` block** from
`describe.json`: it is deprecated, `compat` is the sole source of version
constraints, and `gtdx lint` errors on its presence
(`E_ENGINE_DEPRECATED`). 1.2.1 templates no longer emit it.

## Set a real `metadata.id`

The default is `com.example.<name>`, which `gtdx lint` rejects
(`E_ID_PATTERN` requires `^greentic\.[a-z0-9][a-z0-9-]*$`). It does not block
`gtdx publish`, which never runs lint — but the rule means only
`greentic.*` ids lint clean.

## Then: declare your tools

The scaffold leaves `contributions` empty, and **an empty `contributions.tools`
means the extension exposes no tools at all**. For a v2 extension the runtime
never calls the WASM `list-tools` export — `describe.json` is the only source
of tool metadata. Implementing `list_tools()` in Rust and stopping there
produces an extension that builds, installs and loads with zero tools and no
error anywhere.

See [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md).

## Next

```
cd my-ext
gtdx dev               # watch, rebuild, reinstall into ~/.greentic
gtdx validate ./       # schema + contract deserialization
gtdx lint --dir ./     # cross-field governance rules
gtdx publish --dry-run # build + pack + validate, no registry write
```
