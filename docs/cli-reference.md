# CLI Reference — `gtdx`

`gtdx` is the Greentic Designer Extensions CLI. It manages extensions on
the local machine: validate, install, uninstall, search registries, and
diagnose problems.

**Install:**

```
cargo install greentic-extension-sdk-cli --locked
```

---

## Global Flags

These flags apply to all subcommands.

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--home <PATH>` | `GREENTIC_HOME` | `~/.greentic` | Override the Greentic home directory. Extensions are stored under `<home>/extensions/`. Registry config is read from `<home>/config.toml`. Credentials are stored in `<home>/credentials.toml`. |

Example — use a project-local home for testing:

```
GREENTIC_HOME=./.gtdx-test gtdx list
```

---

## Table of Contents

**Authoring**

- [new](#new) · [openapi](#openapi) · [dev](#dev) · [validate](#validate) · [lint](#lint)

**Publishing**

- [publish](#publish) · [keygen](#keygen) · [sign](#sign) · [verify](#verify) · [yank](#yank) · [unyank](#unyank)

**Installing and operating**

- [install](#install) · [list](#list) · [info](#info) · [search](#search) · [uninstall](#uninstall)
- [enable](#enable) · [disable](#disable) · [outdated](#outdated) · [update](#update) · [doctor](#doctor)

**Registries and admin**

- [login](#login) · [logout](#logout) · [registries](#registries) · [component](#component) · [version](#version)

---

## `new`

Scaffold a new extension project from a built-in template.

**Synopsis:**

```
gtdx new <NAME> [--kind KIND] [--id ID] [--version VERSION] [--author NAME]
              [--license SPDX] [--dir PATH] [--node-type-id ID]
              [--label TEXT] [--force] [--no-git] [-y]
```

**Arguments:**

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `NAME` | Yes | — | Project folder name (kebab-case). Also used as the default suffix when `--id` is omitted. |
| `-k`, `--kind KIND` | No | `design` | Extension kind. One of: `design`, `bundle`, `deploy`, `provider`, `wasm-component`, `mcp`, `llm`. |
| `-i`, `--id ID` | No | `com.example.<NAME>` | Reverse-DNS extension id (e.g. `myco.my-tool`). |
| `-v`, `--version VERSION` | No | `0.1.0` | Initial semver. |
| `--author NAME` | No | `git config user.name` | Author name written into `describe.json` and Cargo metadata. |
| `--license SPDX` | No | `Apache-2.0` | SPDX license id. |
| `--dir PATH` | No | `./<NAME>` | Output directory. |
| `--node-type-id ID` | No | last `.`-separated segment of `<NAME>` | **`wasm-component` only.** Sets `contributions.nodeTypes[0].type_id` in `describe.json`. |
| `--label TEXT` | No | humanized form of derived `--node-type-id` | **`wasm-component` only.** Sets `contributions.nodeTypes[0].label`. |
| `--force` | No | false | Overwrite the target directory if it already exists. |
| `--no-git` | No | false | Skip `git init` after scaffolding. |
| `-y`, `--yes` | No | false | Skip the wizard; resolve everything from flags and defaults. |
| `-w`, `--wizard` | No | false | Force the interactive wizard even when a name/flags are given. Omitting `NAME` on a terminal also launches it. |
| `--from-openapi SPEC` | No | — | **`mcp` only.** Seed the router from an OpenAPI/Swagger spec instead of the echo skeleton. |
| `--icon PATH` | No | — | Icon (svg/png/jpg/webp, ≤ 1 MiB) copied into `assets/` and set as `metadata.icon`. |

**Kinds:**

- **`design`** — full design extension (validation, prompting, knowledge,
  multi-tool). See
  [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md).
- **`bundle`** — packages designer output into Application Packs. See
  [how-to-write-a-bundle-extension.md](./how-to-write-a-bundle-extension.md).
- **`deploy`** — deploys Application Packs to targets. See
  [how-to-write-a-deploy-extension.md](./how-to-write-a-deploy-extension.md).
- **`provider`** — messaging / event provider. See
  [how-to-write-a-provider-extension.md](./how-to-write-a-provider-extension.md).
- **`wasm-component`** — convenience flavor for wrapping a pre-built WASM
  runtime `.gtpack` as a single designer canvas node. **Does not currently
  produce a buildable project** — see
  [how-to-write-a-wasm-component-extension.md](./how-to-write-a-wasm-component-extension.md).
- **`mcp`** — a `wasix:mcp/router` artifact. It imports no greentic WIT
  package, and is the one kind that builds straight out of `gtdx new`.
- **`llm`** — an LLM provider flavor of the design world.

**Example — design extension (default):**

```
$ gtdx new my-ext --kind design --id greentic.my-ext -y
Scaffolded design extension at my-ext (19 files, contract 0.2.0).
```

> **The generated project does not build as-is** (every kind but `mcp`): the
> rendered `wit/world.wit` asks for `greentic:extension-host@0.2.0` while the
> vendored package is `@0.1.0`. See
> [getting-started-scaffolding.md](./getting-started-scaffolding.md#known-issue--a-fresh-scaffold-does-not-build)
> for the one-line fix and the per-kind table.

**Example — wasm-component flavor:**

```
$ gtdx new myco.my-tool \
    --kind wasm-component \
    --node-type-id my-tool \
    --label "My Tool" \
    --dir ./my-tool \
    -y --no-git
Scaffolded wasm-component extension at ./my-tool (17 files, contract 0.2.0).
```

The output directory contains a Cargo workspace, the extension WASM crate
under `extension/`, a `runtime/` subdirectory ready for your pre-built
`.gtpack`, and a pre-wired `describe.json` with one `nodeTypes` entry.

---

## `validate`

Validate an extension directory against the `describe.json` schema.

**Synopsis:**

```
gtdx validate [PATH]
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `PATH` | No | `.` | Path to the extension source directory. Must contain a `describe.json` file. |

**Description:**

Reads `<PATH>/describe.json`, validates it against the embedded JSON Schema
(`describe-v2.json`), and then deserializes it into the Rust contract to
confirm all required fields are present and type-correct.

The deserialize step is the one that matters: the v2 schema types
`contributions.tools` as `items: {}`, so it checks that `tools` is an array
and nothing about what is in it. Every contract struct is
`deny_unknown_fields`, so one misspelled key fails the whole describe.

A field-level error such as `unknown field 'operation'` usually means **your
`gtdx` is older than the describe you are validating**, not that the field is
wrong. See also [`lint`](#lint), which checks cross-field rules this command
does not.

This command runs entirely offline — no network calls are made.

**Example:**

```
$ gtdx validate ~/.greentic/extensions/design/greentic.calendly-1.3.0/
✓ ~/.greentic/extensions/design/greentic.calendly-1.3.0/describe.json valid
```

**Failure example:**

```
$ gtdx validate ./broken-ext/
Error: schema validation failed:
  /metadata/id: string does not match pattern '^[a-z][a-z0-9.-]*\.[a-z0-9.-]+'
```

**Exit codes:** 0 = valid, non-zero = validation failed.

---

## `list`

List installed extensions.

**Synopsis:**

```
gtdx list [--status]
```

**Arguments:**

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--status` | No | false | Append an `enabled`/`disabled` column derived from `<home>/extensions-state.json`. Extensions absent from the state file are reported as `enabled` (the default). |

**Description:**

Scans `<home>/extensions/` for installed extensions, reads each `describe.json`,
and prints a grouped summary. Extensions are grouped by kind (`design`,
`bundle`, `deploy`).

**Example:**

```
$ gtdx list
[design]
  greentic.adaptive-cards@1.6.0  Design and validate Microsoft Adaptive Cards v1.6

[bundle]
  greentic.hosted-webchat@1.0.0  Package designer output as a hosted WebChat pack

[deploy]
```

If no extensions are installed the output is empty.

**Example with `--status`:**

```
$ gtdx list --status
[design]
  greentic.adaptive-cards@1.6.0  enabled   Design and validate Microsoft Adaptive Cards v1.6
  greentic.llm-openai@0.1.0      disabled  OpenAI-backed LLM nodes

[bundle]
  greentic.hosted-webchat@1.0.0  enabled   Package designer output as a hosted WebChat pack

[deploy]
```

See [Lifecycle Management](./lifecycle-management.md) for the state file
format and atomic-write semantics.

---

## `install`

Install an extension from a registry or from a local `.gtxpack` file.

**Synopsis:**

```
gtdx install <TARGET> [--version VERSION] [--registry NAME] [-y] [--trust POLICY]
```

**Arguments:**

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `TARGET` | Yes | — | Extension name (registry install) OR path to a `.gtxpack` file (local install). |
| `--version VERSION` | Required for registry | — | Version to install. Must be an exact semver string (e.g. `1.6.0`). Ignored when `TARGET` is a local path. |
| `--registry NAME` | No | Config default | Name of the registry to use. Must be listed in `<home>/config.toml`. |
| `-y`, `--yes` | No | false | Skip the permission prompt. Accept all declared permissions automatically. |
| `--trust POLICY` | No | Config default | Trust policy override: `strict`, `normal`, or `loose`. See [permissions-and-trust.md](./permissions-and-trust.md). |

**Local path install:**

The file name must follow the convention `<name>-<version>.gtxpack`. The
installer derives name and version from the filename.

```
$ gtdx install ./greentic.adaptive-cards-1.6.0.gtxpack
✓ installed greentic.adaptive-cards@1.6.0
```

**Registry install:**

```
$ gtdx install greentic.adaptive-cards --version 1.6.0
✓ installed greentic.adaptive-cards@1.6.0
```

**Skip permission prompt:**

On first install (or when new permissions are declared), `gtdx` displays the
permissions the extension is requesting:

```
greentic.flow-designer@0.2.0 requests:
  network:
    https://api.greentic.ai
  secrets:
    secrets://*/default/flow-designer/*
  callExtensionKinds:
    design

Accept? [y/N]
```

Pass `-y` to accept automatically (useful in CI or scripted installs).

**Failure examples:**

```
$ gtdx install greentic.adaptive-cards --version 1.6.0
error: --version required for registry install

$ gtdx install ./not-a-pack.zip
error: not a .gtxpack file: not-a-pack.zip
```

---

## `uninstall`

Remove an installed extension.

**Synopsis:**

```
gtdx uninstall <NAME> [--version VERSION]
```

**Arguments:**

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `NAME` | Yes | — | Extension id, e.g. `greentic.adaptive-cards`. |
| `--version VERSION` | No | — | Remove only this version. If omitted, all versions of the extension are removed. |

**Example:**

```
$ gtdx uninstall greentic.adaptive-cards
✓ removed greentic.adaptive-cards@1.6.0

$ gtdx uninstall greentic.adaptive-cards --version 1.5.0
✓ removed greentic.adaptive-cards@1.5.0
```

If no matching installation is found:

```
$ gtdx uninstall greentic.adaptive-cards
nothing to remove for greentic.adaptive-cards
```

---

## `enable`

Enable an installed extension.

**Synopsis:**

```
gtdx enable <TARGET>
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `TARGET` | Yes | — | Extension id, optionally with `@<version>` (e.g. `greentic.adaptive-cards@1.6.0`). When the version is omitted and only one is installed, it is inferred. With multiple installed versions, the command errors and lists them. |

**Description:**

- Verifies the extension is installed under
  `<home>/extensions/<kind>/<id>-<version>/` before writing state.
- Writes `enabled: true` to `<home>/extensions-state.json` atomically.
- Idempotent — re-enabling an already-enabled extension prints the success
  message and exits 0.

**Example:**

```
$ gtdx enable greentic.adaptive-cards
✓ enabled greentic.adaptive-cards@1.6.0

$ gtdx enable greentic.llm-openai@0.1.0
✓ enabled greentic.llm-openai@0.1.0
```

See [Lifecycle Management](./lifecycle-management.md) for the state file
format.

---

## `disable`

Disable an installed extension. Disabled extensions stay installed but
contribute no palette nodes; consumers like `greentic-designer` skip them
at boot and on hot reload.

**Synopsis:**

```
gtdx disable <TARGET>
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `TARGET` | Yes | — | Same form as `enable`: extension id, optionally with `@<version>`. |

**Description:**

- Same install verification and atomic state write as `enable` (just
  `enabled: false`).
- Scans peer extensions and warns to stderr when any of them declare a
  `capabilities.required` entry that matches a `capabilities.offered`
  entry from the target. The warning is informational; the disable still
  proceeds. Cascade resolution is intentionally out of scope for the MVP.

**Example:**

```
$ gtdx disable greentic.adaptive-cards
✓ disabled greentic.adaptive-cards@1.6.0
```

**With dependency warning:**

```
$ gtdx disable greentic.adaptive-cards
warning: greentic.flow-designer@0.2.0 requires capability 'adaptive-cards.render' offered by greentic.adaptive-cards
✓ disabled greentic.adaptive-cards@1.6.0
```

See [Lifecycle Management](./lifecycle-management.md) for hot-reload
behavior and the state file format.

---

## `search`

Search a registry for extensions.

**Synopsis:**

```
gtdx search <QUERY> [--registry NAME] [--kind KIND] [--limit N]
```

**Arguments:**

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `QUERY` | Yes | — | Free-text search query. |
| `--registry NAME` | No | Config default | Registry to search. |
| `--kind KIND` | No | All kinds | Filter results by kind: `design`, `bundle`, or `deploy`. |
| `--limit N` | No | `20` | Maximum number of results to return. |

**Example:**

```
$ gtdx search adaptive cards --kind design
greentic.adaptive-cards               1.6.0  Design  Design and validate Microsoft Adaptive Cards v1.6
```

Output columns: name, latest version, kind, summary.

---

## `info`

Show metadata for an extension in a registry.

**Synopsis:**

```
gtdx info <NAME> [--version VERSION] [--registry NAME]
```

**Arguments:**

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `NAME` | Yes | — | Extension id. |
| `--version VERSION` | No | Latest | Specific version to show. If omitted, shows the latest published version. |
| `--registry NAME` | No | Config default | Registry to query. |

**Example:**

```
$ gtdx info greentic.adaptive-cards
name:     greentic.adaptive-cards
version:  1.6.0
kind:     Design
license:  MIT
summary:  Design and validate Microsoft Adaptive Cards v1.6
sha256:   sha256:abcdef1234...
versions:
  1.4.0
  1.5.0
  1.6.0
```

---

## `login`

Log in to a registry. Stores the token at `<home>/credentials.toml`
(mode 0600 on Unix).

**Synopsis:**

```
gtdx login [--registry NAME]
```

**Arguments:**

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--registry NAME` | No | Config default | Registry to log in to. |

**Description:**

Prompts for a token interactively (input is hidden). The token is stored in
`~/.greentic/credentials.toml`. Subsequent registry operations read the
token from this file automatically.

**Example:**

```
$ gtdx login
Token for greentic-store: ********
✓ logged in to greentic-store
```

**Tokens can also be supplied via environment variable.** Configure the
registry with a `token_env` field (see [registries](#registries)), then
set that environment variable in your CI environment.

---

## `logout`

Remove stored credentials for a registry.

**Synopsis:**

```
gtdx logout [--registry NAME]
```

**Arguments:**

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--registry NAME` | No | Config default | Registry to log out of. |

**Example:**

```
$ gtdx logout
✓ logged out of greentic-store
```

If no credentials are stored:

```
$ gtdx logout
no credentials for greentic-store
```

---

## `registries`

Show and modify configured registries.

**Synopsis:**

```
gtdx registries <SUBCOMMAND>
```

### `registries list`

```
gtdx registries list
```

List all configured registries and the current default.

```
$ gtdx registries list
default: greentic-store
  greentic-store  https://store.greentic.cloud
  local-mirror    https://registry.corp.example.com
```

### `registries add`

```
gtdx registries add <NAME> <URL> [--token-env ENV_VAR]
```

| Argument / Flag | Required | Description |
|-----------------|----------|-------------|
| `NAME` | Yes | Registry name used in other commands. |
| `URL` | Yes | Base URL of the registry API (no trailing slash). |
| `--token-env ENV_VAR` | No | Name of the environment variable that holds the auth token for this registry. |

**Example:**

```
$ gtdx registries add my-mirror https://registry.corp.example.com \
    --token-env CORP_REGISTRY_TOKEN
✓ added my-mirror
```

### `registries remove`

```
gtdx registries remove <NAME>
```

Remove a registry by name.

```
$ gtdx registries remove my-mirror
✓ removed my-mirror
```

### `registries set-default`

```
gtdx registries set-default <NAME>
```

Set the default registry used when `--registry` is not specified.

```
$ gtdx registries set-default my-mirror
✓ default = my-mirror
```

The name must already exist in the config.

---

## `doctor`

Validate all installed extensions and report problems.

**Synopsis:**

```
gtdx doctor
```

**Arguments:** None.

**Description:**

Scans all installed extensions across all kinds. For each extension:

1. Checks that `describe.json` exists.
2. Validates `describe.json` against the schema.
3. Prints `✓` (pass) or `✗` (fail) with a brief message.

Exits with code 1 if any extension fails validation.

**Example:**

```
$ gtdx doctor
✓ /home/user/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/describe.json
✗ /home/user/.greentic/extensions/design/broken-ext-0.1.0/describe.json: invalid JSON: ...

2 total, 1 bad
```

Use `doctor` after an upgrade or after manually editing an installed
extension to confirm the installation is intact.

---

## `version`

Print the `gtdx` version.

**Synopsis:**

```
gtdx version
```

**Example:**

```
$ gtdx version
gtdx 1.3.0-research.3
```

---

## `openapi`

Generate a `DesignExtension` connector from an OpenAPI 3.0 spec, instead of
hand-writing the tool dispatch.

**Synopsis:**

```
gtdx openapi <SPEC> [--name NAME] [--out DIR] [--base-url URL]
```

| Argument / Flag | Required | Default | Description |
|---|---|---|---|
| `SPEC` | Yes | — | Path to the OpenAPI 3.0 spec (JSON or YAML). |
| `-n`, `--name NAME` | No | the spec's `info.title` | Connector name override. |
| `-o`, `--out DIR` | No | `./<slugified-name>` | Output directory. |
| `--base-url URL` | No | the spec's first `servers[]` entry | Base URL override. |

To seed an MCP router from a spec instead, use `gtdx new --kind mcp
--from-openapi <SPEC>`.

---

## `dev`

The developer inner loop: rebuild, pack, and install on every source change.
Full walk-through: [getting-started-dev.md](./getting-started-dev.md).

**Synopsis:**

```
gtdx dev [--once | --watch | --mount PATH] [--release] [--no-install]
         [--debounce-ms MS] [--force-rebuild] [--format human|json]
         [--manifest PATH] [--log LEVEL]
```

| Flag | Default | Description |
|---|---|---|
| `--watch` | on | Continuous watch mode. |
| `--once` | — | Build + install once, then exit. CI-friendly. |
| `--mount PATH` | — | Build + pack + install the extension at `PATH` once, exactly as `gtdx install` would. Conflicts with `--watch` / `--once`. |
| `--no-install` | false | Build and pack only. |
| `--release` | false (debug) | Build with `--release`. |
| `--debounce-ms MS` | `500` | File-watch debounce window. |
| `--force-rebuild` | false | `cargo clean -p <crate>` first. |
| `--format FMT` | `human` | `human`, or `json` for one JSON object per lifecycle event. |
| `--manifest PATH` | `./Cargo.toml` | Path to the project's `Cargo.toml`. |
| `--log LEVEL` | `info` | Log filter level. |

`dev` invokes `cargo component build --target wasm32-wasip2` and then looks
for the artifact under `wasm32-wasip2/<profile>/`, falling back to
`wasm32-wasip1/<profile>/` (cargo-component 0.21 emits wasip2 only under some
toolchains).

---

## `lint`

Check a `describe.json` against cross-field governance rules that the JSON
Schema cannot express. **Not run by `gtdx publish`**, and not part of the
scaffold's `ci/local_check.sh` — invoke it yourself or wire it into your CI.

**Synopsis:**

```
gtdx lint [--dir DIR] [--publish]
```

| Flag | Default | Description |
|---|---|---|
| `--dir DIR` | `.` | Extension source directory containing `describe.json`. |
| `--publish` | false | Also run publish-only rules (e.g. `E_SHA256_ZERO`). |

Rule table: [describe-json-spec.md](./describe-json-spec.md#gtdx-lint).

**Example:**

```
$ gtdx lint --dir ./my-ext
error: E_ENGINE_DEPRECATED: engine block is deprecated; move all version
       constraints into compat (min_designer_version / min_runner_version)
       and delete engine
error: E_ID_PATTERN: metadata.id "com.example.my-ext" must match
       ^greentic\.[a-z0-9][a-z0-9-]*$
Error: 2 error(s)
```

Both of those fire on an untouched `gtdx new` scaffold.

---

## `publish`

Build the component, assemble the `.gtxpack`, and write it to a registry.
You do **not** hand it a path to a pre-built pack — it builds from the
project directory.

**Synopsis:**

```
gtdx publish [--registry URI] [--version VERSION] [--dry-run] [--sign]
             [--key PATH | --key-id ID | --key-env VAR] [--trust POLICY]
             [--dist DIR] [--force] [--verify-only] [--wasm PATH]
             [--manifest PATH] [--oci-token TOKEN] [--icon PATH]
             [--format human|json] [-w]
```

| Flag | Default | Description |
|---|---|---|
| `-r`, `--registry URI` | `local` | `local` (→ `$GREENTIC_HOME/registries/local`), `file://<path>`, `oci://<host>/<ns>[/<artifact>]`, or a named entry from `~/.greentic/config.toml`. |
| `--version VERSION` | describe's own | Override `describe.json`'s version for this run (CI version bumps). |
| `--dry-run` | false | Build + pack + validate; skip the registry write. |
| `--sign` | false | Sign the `.gtxpack`. Requires a key. |
| `--key PATH` | — | Explicit PKCS8 PEM signing key. Overrides `--key-id`. |
| `--key-id ID` | — | Loads `~/.greentic/keys/<ID>.key` and labels the signature with `<ID>`. |
| `--key-env VAR` | `GREENTIC_EXT_SIGNING_KEY_PEM` | Read the PKCS8 PEM key from this env var (CI / headless). |
| `--trust POLICY` | `loose` | `loose` \| `normal` \| `strict`. |
| `--dist DIR` | `./dist` | Also copy the artifact here. |
| `--force` | false | Overwrite an existing version. |
| `--verify-only` | false | Skip the build; only check the registry for a version conflict. |
| `--wasm PATH` | — | Pack this pre-built `wasm32-wasip2` component instead of running `cargo component build`. For externally produced components (e.g. a generated MCP router). The project's `describe.json` still drives the pack. |
| `--oci-token TOKEN` | — | Bearer token for `oci://` registries. Falls back to `GHCR_TOKEN`, `GITHUB_TOKEN`, `OCI_TOKEN`, then anonymous. |
| `--icon PATH` | — | Icon copied into `assets/` and written to `metadata.icon` — **updates `describe.json` on disk**, so commit the change. |
| `-w`, `--wizard` | false | Prompt for registry, mode, signing and trust instead of requiring the full flag string. |

---

## `keygen`

Generate an ed25519 keypair for signing extension artifacts.

**Synopsis:**

```
gtdx keygen [--out PATH]
```

`--out` writes the private key to a file with mode `0600`; the file must not
already exist. Without it, the key goes to stdout.

---

## `sign`

Sign a `describe.json` in place.

**Synopsis:**

```
gtdx sign <DESCRIBE_PATH> [--key PATH | --key-env VAR]
```

`--key` and `--key-env` are mutually exclusive; `--key-env` defaults to
`GREENTIC_EXT_SIGNING_KEY_PEM`.

---

## `verify`

Verify an extension's signature.

**Synopsis:**

```
gtdx verify <PATH> [--trusted-key KEY]
```

`PATH` accepts three shapes, checking progressively more:

| Input | Checks |
|---|---|
| a `describe.json` file | the inline signature |
| an extension directory | the `describe.json` inside it |
| a `.gtxpack` archive | the full chain: signature + manifest binding + ledger |

**`--trusted-key` decides what the result means.** With it (a base64 ed25519
public key, an `ed25519:` prefix is accepted), the signature must have been
produced by that exact key — this is an *authenticity* check. Without it,
only describe self-consistency is verified: that proves the describe is
unmodified, **not who signed it**.

---

## `outdated`

Check installed extensions for available updates.

**Synopsis:**

```
gtdx outdated [--registry NAME]
```

---

## `update`

Update installed extensions to the latest permitted version.

**Synopsis:**

```
gtdx update [TARGET] [--all] [--registry NAME] [-y]
```

| Argument / Flag | Description |
|---|---|
| `TARGET` | Extension id to update. Omit it and pass `--all` to update everything. |
| `--all` | Update every installed extension that has an update available. |
| `--registry NAME` | Registry name from config. |
| `-y`, `--yes` | Skip the permission prompt. |

---

## `yank`

Withdraw a published version. A yanked version is hidden from the version
list and is never selected as `latest`, but **stays downloadable for existing
pins** — it is a deprecation signal, not a deletion.

**Synopsis:**

```
gtdx yank <NAME> <VERSION> [--reason TEXT] [--registry NAME]
```

`--reason` is stored by the store and shown to anyone who inspects the
version. Worth the extra seconds.

---

## `unyank`

Reverse a yank, putting a version back in circulation.

**Synopsis:**

```
gtdx unyank <NAME> <VERSION> [--registry NAME]
```

---

## `component`

Register a **component-tool** by URL against greentic-designer-admin. This is
a different object from an extension: it registers an existing component so a
tenant's flow editor or agentic workers can call it, and it writes to the
admin, not to `~/.greentic`.

**Synopsis:**

```
gtdx component register --url URL --name NAME --tenant SLUG --user EMAIL
                        [--admin-url URL] [--admin-token KEY]
                        [--component-ref REF] [--component-version VERSION]
                        [--component-digest DIGEST]
                        [--allowed-ops OPS] [--role ROLE]
```

| Flag | Description |
|---|---|
| `--url URL` | Store / OCI / repo URL of the component to register. |
| `--name NAME` | Friendly name for the component-tool, unique per tenant. |
| `--tenant SLUG` | Sent as the `X-Greentic-Tenant` header. |
| `--user EMAIL` | Acting user, sent as `X-Greentic-User`. **Must be a tenant admin.** |
| `--admin-url URL` | Base URL of greentic-designer-admin, else `GREENTIC_ADMIN_URL`. |
| `--admin-token KEY` | `gts_` service key, else `GREENTIC_ADMIN_TOKEN`, else the `greentic-admin` key in `credentials.toml`. |
| `--component-ref` / `--component-version` / `--component-digest` | Optional identifying detail for the component. |
| `--allowed-ops OPS` | Restrict the registration to these operations (repeatable or comma-separated). Omitted = all operations allowed. |
| `--role ROLE` | Grant the component-tool to these roles (repeatable): `flow_editor`, `agentic_worker`. |
