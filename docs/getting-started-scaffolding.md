# Scaffolding a new extension

`gtdx new` generates a ready-to-build Greentic Designer Extension project
against the v2 describe contract.

## Prerequisites

- Rust 1.95+ with the `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `cargo-component`: `cargo install --locked cargo-component`
- A `gtdx` no older than the contract you are targeting — see
  [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md#check-your-gtdx-before-you-start).
  The `1.3.0-research.1` prerelease sorts above `1.2.0` by semver and is
  older in content; build from a checkout rather than trusting the number.

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

## Known issue — a fresh scaffold does not build

> **Fixed upstream in greentic-designer-sdk#105 and #106** (merged to
> `research`, 2026-08-21). #105 fixed the WIT versions plus two stub bugs it
> was masking — `provider` used a non-existent error type, and `--kind llm`
> emitted a `describe.json` that did not parse at all. #106 fixed
> `--kind wasm-component`, and removed the deprecated `engine` block from
> every template. **All seven kinds now pass `gtdx validate`, `gtdx lint` and
> `cargo component build` as generated.**
>
> Everything below still applies to any `gtdx` predating those commits, which
> includes every released build. Check with `gtdx new` and read the rendered
> `wit/world.wit`: if it says `extension-host@0.1.0` you have the fix and can
> skip this section.

Verified 2026-08-21 against `gtdx 1.3.0-research.3`. **Every kind except `mcp`
fails `cargo component build` (and therefore `gtdx dev`) on a completely
untouched scaffold**, with:

```
failed to merge local target .../wit
package 'greentic:extension-host@0.2.0' not found. known packages:
    greentic:extension-host@0.1.0
    greentic:extension-base@0.2.0
    greentic:extension-design@0.3.0
```

The cause is in `templates/*/wit/world.wit.tmpl`: every import and export is
rendered with a single `{{contract_version}}` placeholder, but the embedded WIT
packages are versioned **independently** —

| package | version |
|---|---|
| `greentic:extension-base` | 0.2.0 |
| `greentic:extension-host` | **0.1.0** |
| `greentic:extension-design` | **0.3.0** |
| `greentic:extension-bundle` / `-deploy` / `-provider` | 0.2.0 |

so the rendered world asks for `extension-host@0.2.0`, which does not exist.
`scaffold::embedded::package_version` exists to read the real per-file version
and is not used by the renderer. No test builds a scaffold, so nothing catches
it.

### Status and workaround per kind

| `--kind` | Builds as generated | Fix |
|---|---|---|
| `mcp` | **yes** | none — it imports no greentic WIT package |
| `bundle` | no | rewrite `extension-host@0.2.0` → `@0.1.0` in `wit/world.wit` |
| `deploy` | no | same |
| `design` | no | same, **plus** `extension-design@0.2.0` → `@0.3.0` |
| `llm` | no | same as `design`, **plus** its `describe.json` carries an `id` key on the tool entry that `Tool` does not model — `deny_unknown_fields` fails the whole describe — and its stub targets the pre-0.3.0 design contract. Not worth patching by hand; take the fixed `gtdx` |
| `provider` | no | host version, **plus** `provider_types::Error` → `types::ExtensionError` in `src/lib.rs` (the WIT declares `extension-error`, from `extension-base/types`) |
| `wasm-component` | no | **not a one-line fix** — take the fixed `gtdx`; what was wrong is recorded in [how-to-write-a-wasm-component-extension.md](./how-to-write-a-wasm-component-extension.md#what-106-changed) |

For every kind but `provider` and `wasm-component`:

```bash
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' \
       -e 's|\(greentic:extension-design/[a-z0-9-]*\)@0\.2\.0|\1@0.3.0|g' \
       wit/world.wit
```

Then `cargo component build` succeeds. Confirmed for `design`, `bundle`,
`deploy`, and (with the extra `src/lib.rs` edit) `provider`.

## Set a real `metadata.id`

The default is `com.example.<name>`, which `gtdx lint` rejects
(`E_ID_PATTERN` requires `^greentic\.[a-z0-9][a-z0-9-]*$`). It does not block
`gtdx publish`, which never runs lint — but the rule means only
`greentic.*` ids lint clean.

On a `gtdx` predating #106 you must also **delete the `engine` block**: it is
deprecated, `compat` is the sole source of version constraints, and
`gtdx lint` errors on its presence (`E_ENGINE_DEPRECATED`). Templates no
longer emit it.

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
