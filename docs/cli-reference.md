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

- [validate](#validate)
- [list](#list)
- [install](#install)
- [uninstall](#uninstall)
- [search](#search)
- [info](#info)
- [login](#login)
- [logout](#logout)
- [registries](#registries)
- [doctor](#doctor)
- [version](#version)

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
gtdx list
```

**Arguments:** None.

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
