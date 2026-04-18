# Phase 1 — Track A: Scaffolding (`gtdx new`)

**Status:** Design, awaiting implementation plan
**Date:** 2026-04-18
**Parent:** `2026-04-18-dx-10-10-roadmap.md` (subsystem S1)
**Owner crate:** `greentic-ext-cli`

## 1. Goal

Let a developer scaffold a compilable Greentic Designer Extension in one command. The generated project uses the WIT contract embedded in the `gtdx` binary and passes `cargo check` on all target platforms without manual edits.

## 2. CLI UX

```
gtdx new <NAME> [OPTIONS]

Arguments:
  <NAME>                 Project folder name (kebab-case). Also default id suffix.

Options:
  -k, --kind <KIND>      design | bundle | deploy       [default: design]
  -i, --id <ID>          Extension id (reverse-DNS)     [default: com.example.<NAME>]
  -v, --version <SEMVER> Initial version                [default: 0.1.0]
      --author <NAME>    Author metadata               [default: $USER from git config]
      --license <SPDX>   License id                    [default: Apache-2.0]
      --no-git           Skip `git init`
      --no-install       Skip cargo-component check + install suggestion
      --dir <PATH>       Output directory              [default: ./<NAME>]
  -y, --yes              Skip interactive prompts (use all defaults)
```

Interactive fallback: if invoked as `gtdx new` without `NAME`, enter `dialoguer` prompt mode (name, kind dropdown, id suggestion, version).

## 3. Generated Project Layout

```
demo/
├── .gitignore              # target/, *.gtxpack, dist/
├── .gtdx-contract.lock     # contract_version + WIT sha256s
├── Cargo.toml              # cdylib + wit-bindgen pinned to contract version
├── README.md               # stub with next-step commands (gtdx dev / gtdx publish)
├── build.sh                # cargo component build + zip → .gtxpack
├── ci/
│   └── local_check.sh      # fmt + clippy + test + build.sh
├── describe.json           # pre-filled, valid against schema-v1
├── i18n/
│   └── en.json             # label + description keys
├── prompts/
│   └── system.md           # placeholder (design ext only)
├── schemas/
│   └── .gitkeep            # for custom schemas (design ext only)
├── src/
│   └── lib.rs              # implements guest export of kind-specific world
└── wit/
    ├── world.wit           # pulls greentic:extension-<kind>@<version>
    └── deps/greentic/
        ├── extension-base/world.wit
        ├── extension-host/world.wit
        └── extension-<kind>/world.wit
```

## 4. Template Engine

- Templates live in `crates/greentic-ext-cli/templates/` and are embedded via `include_dir!`.
- Placeholders are literal tokens: `{{id}}`, `{{name}}`, `{{version}}`, `{{author}}`, `{{license}}`, `{{contract_version}}`.
- Rendering is manual string replacement (no `handlebars` dep — placeholder set is small and closed).
- Per `--kind`, the variant template under `templates/design/`, `templates/bundle/`, or `templates/deploy/` is selected.

### 4.1 Per-kind differences

- **design:** `src/lib.rs` implements `greentic:extension-design/design.validate-flow-node(...)` returning `{ok: true}` as a stub; README next-step guides the user to edit `validate_flow_node`.
- **bundle:** `src/lib.rs` implements `bundle.list-recipes()` returning a single placeholder recipe, plus `bundle.build-pack()` returning placeholder bytes.
- **deploy:** `src/lib.rs` implements `deploy.plan()` and `deploy.apply()` stubs; README explains lifecycle order.

## 5. WIT Vendor Mechanism

The `gtdx` binary ships with WIT files embedded under `crates/greentic-ext-cli/embedded-wit/<contract-version>/`:

- A CI step in `greentic-designer-extensions` runs before the release build: `cp -r wit/* crates/greentic-ext-cli/embedded-wit/<version>/` where `<version>` comes from the root `Cargo.toml` workspace version.
- At `gtdx new` time, the binary:
  1. Reads the embedded directory for the kind being scaffolded.
  2. Writes each file to `wit/deps/greentic/<package>/world.wit` in the target project.
  3. Computes SHA256 per file and writes `.gtdx-contract.lock`:
     ```toml
     contract_version = "0.7.0"
     generated_by = "gtdx 0.7.0"
     generated_at = "2026-04-18T12:34:56Z"
     [files]
     "wit/deps/greentic/extension-base/world.wit" = "sha256:abc..."
     "wit/deps/greentic/extension-host/world.wit"  = "sha256:def..."
     "wit/deps/greentic/extension-design/world.wit" = "sha256:ghi..."
     ```
- `Cargo.toml` in the generated project references WIT by package spec, not file path:
  ```toml
  [package.metadata.component]
  target = { path = "wit" }
  [package.metadata.component.target.dependencies]
  "greentic:extension-base"   = { path = "wit/deps/greentic/extension-base"   }
  "greentic:extension-host"   = { path = "wit/deps/greentic/extension-host"   }
  "greentic:extension-design" = { path = "wit/deps/greentic/extension-design" }
  ```

## 6. Preflight Checks

Run before writing any files:

1. `cargo` available on `PATH`. If missing: error exit 127 with `rustup` install hint.
2. `cargo component --version` available. If missing: warning + hint `cargo install --locked cargo-component` (non-fatal).
3. `rustup target list --installed | grep wasm32-wasip2`. If missing: warning + hint `rustup target add wasm32-wasip2` (non-fatal).
4. Target directory does not exist, or exists and is empty. If conflict without `--force`: error exit 2.

### 6.1 Preflight output

```
✓ cargo 1.91.0 found
✓ cargo-component 0.21.1 found
✓ wasm32-wasip2 target installed
✓ ./demo is clean

Scaffolding design extension at ./demo...
  written  15 files (wit, src, ci, docs)
  locked   contract version 0.7.0 (5 WIT files)

Next steps:
  cd demo
  gtdx dev        # watch & rebuild
  gtdx publish    # pack to dist/
```

Warnings are yellow, successes green, errors red. Uses `anstyle` for ANSI color handling (portable).

## 7. Error Handling

| Error | Exit code | Message |
|-------|-----------|---------|
| `cargo` missing | 127 | `error: cargo not on PATH. install via https://rustup.rs/` |
| Target dir conflict (no `--force`) | 2 | `error: ./demo already exists. pass --force to overwrite.` |
| Permission denied writing target | 30 | `error: cannot write to ./demo (permission denied)` |
| Embedded template missing kind | 70 | internal error — CI gate prevents this |
| Invalid `--id` format | 2 | `error: id must match reverse-DNS (got 'bad id')` |
| Invalid `--version` semver | 2 | `error: version '0.1' is not a valid semver` |

## 8. Testing

- **Unit** `new_cmd::tests::render_template` — render with sample params, assert each file exists and placeholders were substituted.
- **Unit** `new_cmd::tests::lock_file_integrity` — confirm `.gtdx-contract.lock` SHA256 matches embedded bytes.
- **Integration** `tests/cli_new.rs`:
  - Spawn `gtdx new` via `assert_cmd`, output to `tempdir()`.
  - Run `cargo check` in the generated project; assert exit 0.
  - Repeat for all three `--kind` values.
- **Platform matrix** (GitHub Actions): Linux + macOS + Windows.

## 9. Module Breakdown & Line Budget

| Module | Purpose | Target LOC |
|--------|---------|------------|
| `new_cmd.rs` | arg parsing, orchestration | ~250 |
| `template.rs` | placeholder substitution + file write | ~200 |
| `preflight.rs` | toolchain + target dir checks | ~150 |
| `embedded.rs` | `include_dir!` wrapper, hash helpers | ~50 |
| `templates/{design,bundle,deploy}/**` | file assets | (not code) |
| **Total new code** | | **~650 LOC** |

All modules stay well below 500 LOC per user preference.

## 10. Open Questions (resolved in brainstorming)

- Q: Handlebars vs manual string replace? → **manual** (smaller dep graph, placeholder set is closed).
- Q: Generate `.github/workflows/ci.yml` now or Phase 3? → **Phase 3**; Phase 1 scaffold only writes `ci/local_check.sh`.
- Q: `cargo-component` missing = error or warning? → **warning** (developer may install later).

## 11. Acceptance Criteria

1. `gtdx new demo` (all defaults) produces a directory that passes `cargo check` on Linux, macOS, Windows.
2. `gtdx new demo --kind bundle` and `--kind deploy` both produce compilable projects.
3. `.gtdx-contract.lock` hashes match what was actually written to `wit/deps/`.
4. Preflight errors are actionable (every error includes a suggested next command).
5. Re-running in an existing dir without `--force` fails with exit 2 (no files touched).
6. Both reference extensions (AC design ext, `bundle-standard`) can be rescaffolded and diffed ≤ 5 line changes vs generated output (acceptable customization, not a rewrite).

## 12. Non-Goals (Phase 1)

- ❌ Generating CI/Actions workflows (Phase 3)
- ❌ Generating signing keypair at init (Phase 2)
- ❌ Project upgrade / migration from non-`gtdx-new` projects (out of scope — separate tool)
- ❌ Custom user templates via `--template <url>` (future; Phase 5 if demanded)
