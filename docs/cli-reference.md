# CLI Reference — `gtdx`

`gtdx` is the Greentic Designer Extensions CLI. It manages extensions on
the local machine: validate, install, uninstall, search registries, and
diagnose problems.

**Install:**

```
cargo install --path crates/greentic-ext-cli --locked
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

- [new](#new)
- [validate](#validate)
- [list](#list)
- [install](#install)
- [uninstall](#uninstall)
- [enable](#enable)
- [disable](#disable)
- [search](#search)
- [info](#info)
- [login](#login)
- [logout](#logout)
- [registries](#registries)
- [doctor](#doctor)
- [version](#version)

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
| `-k`, `--kind KIND` | No | `design` | Extension kind. One of: `design`, `bundle`, `deploy`, `provider`, `wasm-component`. |
| `-i`, `--id ID` | No | `com.example.<NAME>` | Reverse-DNS extension id (e.g. `myco.my-tool`). |
| `-v`, `--version VERSION` | No | `0.1.0` | Initial semver. |
| `--author NAME` | No | `git config user.name` | Author name written into `describe.json` and Cargo metadata. |
| `--license SPDX` | No | `Apache-2.0` | SPDX license id. |
| `--dir PATH` | No | `./<NAME>` | Output directory. |
| `--node-type-id ID` | No | last `.`-separated segment of `<NAME>` | **`wasm-component` only.** Sets `contributions.nodeTypes[0].type_id` in `describe.json`. |
| `--label TEXT` | No | humanized form of derived `--node-type-id` | **`wasm-component` only.** Sets `contributions.nodeTypes[0].label`. |
| `--force` | No | false | Overwrite the target directory if it already exists. |
| `--no-git` | No | false | Skip `git init` after scaffolding. |
| `-y`, `--yes` | No | false | Skip interactive prompts. |

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
  runtime `.gtpack` as a single designer canvas node. Use this when you
  already have a working component and only need to surface it as a node.
  See
  [how-to-write-a-wasm-component-extension.md](./how-to-write-a-wasm-component-extension.md).

**Example — design extension (default):**

```
$ gtdx new my-ext --id com.example.my-ext
Scaffolded design extension at ./my-ext (12 files, contract 0.1.0).
```

**Example — wasm-component flavor:**

```
$ gtdx new myco.my-tool \
    --kind wasm-component \
    --node-type-id my-tool \
    --label "My Tool" \
    --dir ./my-tool \
    -y --no-git
Scaffolded wasm-component extension at ./my-tool (10 files, contract 0.1.0).
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
(`describe-v1.json`), and then deserializes it to confirm all required fields
are present and type-correct.

This command runs entirely offline — no network calls are made.

**Example:**

```
$ gtdx validate ./reference-extensions/adaptive-cards/
✓ ./reference-extensions/adaptive-cards/describe.json valid
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
  greentic-store  https://store.greentic.ai
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
gtdx 0.1.0
```
