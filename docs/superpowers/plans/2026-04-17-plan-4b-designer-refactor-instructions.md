# Plan 4B — Designer Refactor: Instructions for Fresh Session

> **Read this entire document before starting.** Self-contained brief for an agent or developer picking up Plan 4B from cold.

**Goal:** Refactor `greentic-biz/greentic-designer` to drop the direct
`adaptive-card-core` Rust dependency and instead consume the WASM
extension system from `greentic-biz/greentic-designer-extensions`. The
designer will load `greentic.adaptive-cards@1.6.0` as a real
`.gtxpack`, dispatch tool calls through wasmtime, and ship a bundled
fallback `.gtxpack` so first-run users don't have to install anything
manually.

**End state:** Identical user-visible behaviour to today, but
`adaptive_card_core::*` calls replaced with `runtime.invoke_tool(...)`,
and the designer ships with a working extension out of the box.

**Why this matters:** without Plan 4B, the entire extension system
(shipped 2026-04-17 across 2 repos and 6 milestones) is unused
infrastructure. This is the integration that makes it real.

---

## 0. Pre-flight checks

Before touching the designer:

1. **Verify upstream state.** Both feeder repos must be in sync at the
   tags below. `cd` into each and run `git fetch origin && git log -1 origin/main`.

   | Repo | Path | Required tag |
   |---|---|---|
   | greentic-designer-extensions | `/home/bimbim/works/greentic/greentic-designer-extensions/` | `v0.6.0` (commit on main) |
   | greentic-adaptive-card-mcp | `/home/bimbim/works/greentic/greentic-adaptive-card-mcp/` | `v0.2.0` (commit on main) |

2. **Build the AC extension `.gtxpack`** (needed both as a source-tree
   artifact and as bundled bytes):

   ```bash
   cd /home/bimbim/works/greentic/greentic-adaptive-card-mcp
   crates/adaptive-card-extension/build.sh
   ls -lh crates/adaptive-card-extension/greentic.adaptive-cards-1.6.0.gtxpack
   # → ~1 MB
   ```

3. **Verify designer repo is clean.** `cd /home/bimbim/works/greentic/greentic-designer` and run `git status`. **Important:** there is a known WIP CLAUDE.md edit on branch `docs/agentic-planner-spec` and possibly other uncommitted state. **Stash anything before branching off `main`** — see step 1 of Phase A.

4. **Verify tooling.** `rustc --version` should report `1.94.x` (matches both this repo and the extension repos). `cargo --version`, `cargo-component --version` (≥0.20).

---

## 1. Reference documents

| Doc | What it tells you |
|---|---|
| `/home/bimbim/works/greentic/greentic-designer-extensions/docs/superpowers/specs/2026-04-17-designer-extension-system-design.md` | Full architecture of the extension system (sections 6.1, 7, 8 most relevant for designer integration) |
| `/home/bimbim/works/greentic/greentic-designer-extensions/docs/superpowers/plans/2026-04-17-docs-and-designer-refactor.md` Part B | The original Plan 4B summary (this file expands it) |
| `/home/bimbim/works/greentic/greentic-designer-extensions/README.md` "Integration with greentic-designer" section | Code-level integration sketch (bootstrap + invoke_tool dispatch) |
| `/home/bimbim/works/greentic/greentic-designer/CLAUDE.md` | Designer repo conventions — read before touching |
| `/home/bimbim/works/greentic/greentic-designer/src/ui/tool_bridge/{defs,dispatch}.rs` | The 12 hardcoded match arms you'll be replacing |
| `/home/bimbim/works/greentic/greentic-designer/src/knowledge.rs` | Wrapper over `adaptive_card_core::KnowledgeBase` — to be deleted |
| `/home/bimbim/works/greentic/greentic-designer/src/ui/prompt_builder.rs` | Calls `adaptive_card_core::prompt::build_system_prompt` — to be replaced |

---

## 2. Architecture before vs after

### Today (state to migrate from)

```
greentic-designer/
├── Cargo.toml
│   └── adaptive-card-core = "0.1"               ← direct dep
└── src/
    ├── ui/tool_bridge/defs.rs                    # 12 hardcoded tool defs
    ├── ui/tool_bridge/dispatch.rs                # 12 match arms → adaptive_card_core::*
    ├── ui/prompt_builder.rs                      # → adaptive_card_core::prompt
    ├── knowledge.rs                              # wrapper around AC KB
    └── ui/routes/{validate,examples}.rs          # direct core calls
```

### After Plan 4B

```
greentic-designer/
├── Cargo.toml
│   ├── greentic-ext-runtime = { git = "...", tag = "v0.6.0" }
│   ├── greentic-ext-contract = { git = "...", tag = "v0.6.0" }
│   └── (NO adaptive-card-core dep — it's in the WASM ext now)
├── assets/
│   └── greentic.adaptive-cards-1.6.0.gtxpack    # bundled fallback (~1 MB)
└── src/
    ├── main.rs                                   # bootstrap ExtensionRuntime
    ├── ui/state.rs                               # AppState.runtime: Arc<ExtensionRuntime>
    ├── ui/tool_bridge/defs.rs                    # dynamic from runtime.list_tools()
    ├── ui/tool_bridge/dispatch.rs                # runtime.invoke_tool(ext_id, name, args)
    ├── ui/prompt_builder.rs                      # runtime.aggregate_prompt_fragments("design")
    ├── ui/routes/validate.rs                     # runtime.validate_content("adaptive-card", json)
    ├── ui/routes/examples.rs                     # runtime.design_knowledge().{list,get,suggest}()
    └── (knowledge.rs DELETED)
```

---

## 3. Phase A — Parallel paths, zero regression (1-2 days)

Goal: add the runtime alongside the existing path, gated by a feature
flag. Production behaviour unchanged unless `DESIGNER_USE_EXTENSIONS=1`
is set. This lets you snapshot-compare both paths before flipping the
default.

### Step A.1 — Branch + dependencies

```bash
cd /home/bimbim/works/greentic/greentic-designer
# Stash any WIP first if needed
git stash push -m "wip-pre-plan-4b" 2>/dev/null || true
git checkout main
git pull origin main
git checkout -b feat/extension-runtime-integration
```

Add to `Cargo.toml` (in alphabetical order with existing deps):

```toml
greentic-ext-runtime  = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.6.0" }
greentic-ext-contract = { git = "ssh://git@github.com/greentic-biz/greentic-designer-extensions", tag = "v0.6.0" }
```

Run `cargo build` to fetch + compile. Should succeed on first try
because both crates are pure Rust + don't bring in cargo-component.

Commit: `chore(deps): add greentic-ext-runtime + contract @v0.6.0`

### Step A.2 — Bootstrap runtime in `main.rs`

In `src/main.rs` (or wherever `AppState` is constructed), add after
`tracing_subscriber::fmt().init()`:

```rust
use std::sync::Arc;
use greentic_ext_runtime::{ExtensionRuntime, RuntimeConfig, DiscoveryPaths, discovery};

let greentic_home = std::env::var("GREENTIC_HOME")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|_| {
        dirs::home_dir()
            .expect("home directory")
            .join(".greentic")
    });

let mut runtime = ExtensionRuntime::new(
    RuntimeConfig::from_paths(DiscoveryPaths::new(greentic_home.clone()))
).expect("init runtime");

for kind in ["design", "bundle", "deploy"] {
    let kind_dir = greentic_home.join("extensions").join(kind);
    match discovery::scan_kind_dir(&kind_dir) {
        Ok(paths) => {
            for ext_dir in paths {
                if let Err(e) = runtime.register_loaded_from_dir(&ext_dir) {
                    tracing::warn!(?ext_dir, %e, "failed to load extension");
                }
            }
        }
        Err(e) => tracing::debug!(%e, "no extensions in {}", kind_dir.display()),
    }
}

let runtime = Arc::new(runtime);
tracing::info!(
    loaded_count = runtime.loaded().len(),
    "extension runtime ready"
);
```

Then in `AppState`:

```rust
pub struct AppState {
    // existing fields ...
    pub runtime: Arc<ExtensionRuntime>,
}
```

Add `runtime: runtime.clone()` to the construction site.

Run `cargo run` — designer should start as before; `tracing` logs show
"extension runtime ready · loaded_count=0" if no extensions installed.

Commit: `feat(designer): bootstrap ExtensionRuntime in AppState`

### Step A.3 — Feature flag + dual-path tool_bridge

Add helper at top of `src/ui/tool_bridge/mod.rs` (or wherever):

```rust
fn use_extensions() -> bool {
    std::env::var("DESIGNER_USE_EXTENSIONS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
```

Rewrite `dispatch::dispatch` (or equivalent entry) to branch:

```rust
pub async fn dispatch(
    state: &AppState,
    tool_name: &str,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    if use_extensions() {
        dispatch_via_runtime(state, tool_name, args).await
    } else {
        dispatch_via_core(state, tool_name, args).await   // existing path, unchanged
    }
}

async fn dispatch_via_runtime(
    state: &AppState,
    tool_name: &str,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let args_json = serde_json::to_string(&args)?;
    // Tools currently all live in greentic.adaptive-cards. When we add
    // more design extensions, this lookup becomes "find ext that owns
    // tool_name" via a tool registry built at startup.
    let ext_id = "greentic.adaptive-cards";
    let result_json = state.runtime
        .invoke_tool(ext_id, tool_name, &args_json)
        .map_err(|e| anyhow::anyhow!("invoke_tool({tool_name}) failed: {e}"))?;
    Ok(serde_json::from_str(&result_json)?)
}

async fn dispatch_via_core(/* existing 12-arm match */) { ... }
```

Commit: `feat(designer): dual-path tool_bridge gated by DESIGNER_USE_EXTENSIONS`

### Step A.4 — Mirror the same dual-path in `routes/validate.rs`, `routes/examples.rs`, `prompt_builder.rs`

Same pattern: keep existing function, add `_via_runtime` sibling, branch
on `use_extensions()`. For `examples.rs` you'll use
`runtime.design_knowledge()` aggregator (the runtime exposes a method
that walks all loaded design-exts and collects KB entries — verify
exact name in `greentic-ext-runtime` source).

Commit each file separately for clean history.

### Step A.5 — Manual smoke test

Build the AC ext + install into a test home, run designer with the flag:

```bash
# Build .gtxpack
(cd ../greentic-adaptive-card-mcp && crates/adaptive-card-extension/build.sh)

# Install into a fresh home
TEST_HOME=$(mktemp -d)
gtdx --home "$TEST_HOME" install \
  ../greentic-adaptive-card-mcp/crates/adaptive-card-extension/greentic.adaptive-cards-1.6.0.gtxpack \
  -y --trust loose

# Run designer with the flag
GREENTIC_HOME="$TEST_HOME" DESIGNER_USE_EXTENSIONS=1 cargo run -- ui

# In another terminal: hit /api/chat with a "validate this card" prompt
# and verify the response shape matches what you'd get without the flag.
```

If outputs match (or differ only in expected ways — see Phase 4 below),
proceed. If they diverge unexpectedly, debug before flipping the
default.

### Step A.6 — Snapshot test (CI guard)

Add an integration test under `tests/extension_parity.rs` that builds
the same fixture cards through both paths and asserts equality. Use
`insta` if the designer already uses snapshot testing.

Commit: `test(designer): assert tool dispatch parity between core and runtime paths`

---

## 4. Known intentional behaviour differences

The MVP AC extension does **not** delegate to `adaptive-card-core` for
every tool. Specifically these return a "not_implemented_in_v1_6" stub:

- `optimize_card`
- `transform_card`
- `template_card`
- `data_to_card`

If your snapshot test compares output for these tools, it will diverge.
That divergence is **intentional** and documented in
`greentic-adaptive-card-mcp/crates/adaptive-card-extension/src/lib.rs`.
Plan 4B accepts this regression for v1; the AC extension v2 cycle will
wire them through.

For Phase A snapshot tests, either:
- Skip those four tools entirely
- Assert "stub response shape" for runtime path and "real response shape" for core path

For Phase B cutover, **document the regression** in the designer
release notes.

---

## 5. Phase B — Cutover (1 day)

After Phase A snapshot tests are green:

### Step B.1 — Flip default

In `src/ui/tool_bridge/mod.rs`, change `use_extensions()` to return
`true` unconditionally (or remove the check entirely). Same for
`prompt_builder.rs`, `routes/validate.rs`, `routes/examples.rs`.

Commit: `feat(designer): runtime path is now the default`

### Step B.2 — Delete legacy code paths

Remove:
- `dispatch_via_core` and the 12-arm match from `tool_bridge/dispatch.rs`
- The non-runtime body of `prompt_builder.rs::build_system_prompt`
- The non-runtime body of `validate.rs`, `examples.rs`
- `src/knowledge.rs` (entire file)
- Any `use adaptive_card_core::*;` imports

Commit: `refactor(designer): remove dispatch_via_core + adaptive-card-core import sites`

### Step B.3 — Drop `adaptive-card-core` from `Cargo.toml`

```bash
# Remove the line
adaptive-card-core = "0.1"
```

Run `cargo build` — should compile cleanly without it.

Commit: `chore(deps): remove adaptive-card-core direct dep (now via WASM ext)`

### Step B.4 — Update `CLAUDE.md`

Designer's `CLAUDE.md` likely says it depends on `adaptive-card-core`.
Update to describe the new architecture: runtime + extension model,
no direct core dep, AC tools come from the loaded `.gtxpack`.

Commit: `docs(claude): update for extension runtime architecture`

---

## 6. Phase C — Bundled fallback (1 day)

Goal: first-run users (no `~/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/`)
get a working designer without manually installing anything.

### Step C.1 — Embed the `.gtxpack`

Copy the built artifact to designer's `assets/` (or `ext-bundles/`):

```bash
mkdir -p assets/ext-bundles
cp ../greentic-adaptive-card-mcp/crates/adaptive-card-extension/greentic.adaptive-cards-1.6.0.gtxpack \
   assets/ext-bundles/
```

Add to `.gitignore`? **No.** This is a release artifact that needs to
ship with the binary. Commit it.

In `src/main.rs` or a new `src/bundled_extensions.rs`:

```rust
const BUNDLED_AC_GTXPACK: &[u8] = include_bytes!(
    "../assets/ext-bundles/greentic.adaptive-cards-1.6.0.gtxpack"
);
const BUNDLED_AC_NAME: &str = "greentic.adaptive-cards";
const BUNDLED_AC_VERSION: &str = "1.6.0";
```

### Step C.2 — Auto-install on startup if missing

In the bootstrap block (after the discovery scan):

```rust
let ac_target = greentic_home
    .join("extensions/design")
    .join(format!("{BUNDLED_AC_NAME}-{BUNDLED_AC_VERSION}"));

if !ac_target.exists() && !std::env::var("DESIGNER_NO_BUNDLED_FALLBACK").is_ok() {
    tracing::info!(
        "installing bundled {BUNDLED_AC_NAME}@{BUNDLED_AC_VERSION} \
         (no extension found; set DESIGNER_NO_BUNDLED_FALLBACK=1 to disable)"
    );
    if let Err(e) = install_bundled_gtxpack(BUNDLED_AC_GTXPACK, &ac_target) {
        tracing::error!(%e, "bundled AC install failed; designer will run without AC ext");
    } else {
        // Re-scan to load the just-installed extension
        for path in discovery::scan_kind_dir(&greentic_home.join("extensions/design"))
            .unwrap_or_default()
        {
            runtime.register_loaded_from_dir(&path).ok();
        }
    }
}

fn install_bundled_gtxpack(bytes: &[u8], target_dir: &Path) -> anyhow::Result<()> {
    let staging = target_dir.with_extension("tmp");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let out_path = staging.join(entry.mangled_name());
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir)?;
    }
    if let Some(parent) = target_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&staging, target_dir)?;
    Ok(())
}
```

Add `zip = { version = "2", default-features = false, features = ["deflate"] }` to `Cargo.toml` deps if not already present.

Commit: `feat(designer): bundled fallback install of greentic.adaptive-cards@1.6.0 on first run`

### Step C.3 — `--no-bundled-fallback` clap flag (optional)

If designer uses `clap`, add a flag mirroring the env var. Otherwise the
env var alone is fine.

### Step C.4 — Update designer's `build.rs` (or CI) to refresh the bundle

Manual `cp` is OK for now. Document in `CLAUDE.md`:

> When the AC extension version bumps, refresh the bundle:
> ```bash
> cp ../greentic-adaptive-card-mcp/crates/adaptive-card-extension/greentic.adaptive-cards-*.gtxpack \
>    assets/ext-bundles/
> ```

A future enhancement is a `build.rs` that fetches the latest tag's
release asset, but that needs auth setup — not blocking.

Commit: `docs(claude): how to refresh bundled .gtxpack`

---

## 7. Phase D — Final verification + tag

### Step D.1 — Local CI green

```bash
# Whatever the designer's local CI script is — likely:
bash ci/local_check.sh   # or scripts/ci.sh — read CLAUDE.md
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

### Step D.2 — End-to-end manual run

```bash
# Clean home
TEST_HOME=$(mktemp -d)

# Run designer with bundled fallback enabled (default)
GREENTIC_HOME="$TEST_HOME" cargo run -- ui

# Visit http://localhost:<port>/ui in browser
# Send a chat message: "create a hello world adaptive card"
# Verify: tools execute, response includes a valid card,
# subsequent "validate this card" calls work.

# Optional: verify bundled install left files behind
ls "$TEST_HOME/extensions/design/greentic.adaptive-cards-1.6.0/"
# Should show: describe.json, extension.wasm, schemas/, prompts/, etc.
```

### Step D.3 — Open PR

```bash
git push -u origin feat/extension-runtime-integration
gh pr create --base main \
  --title "feat: consume extension system runtime + bundled AC fallback" \
  --body "..."   # see PR template below
```

### Step D.4 — Tag after merge

The next designer release captures the full migration. If designer uses
semver, this is a minor bump (no API break to designer's users; internal
refactor).

---

## 8. PR template

````markdown
## Summary

Drops the direct `adaptive-card-core` Rust dependency and consumes the
WASM extension system from greentic-designer-extensions instead. The
designer now loads `greentic.adaptive-cards@1.6.0` as a `.gtxpack`,
dispatches tool calls through wasmtime, and ships a bundled fallback
so first-run users don't need to install anything manually.

## Why

Without this PR the entire extension system shipped 2026-04-17 across
greentic-designer-extensions (v0.6.0) and greentic-adaptive-card-mcp
(v0.2.0) is unused infrastructure. This is the integration that makes
it real and unlocks third-party extensions (Digital Workers, Telco X,
etc.) that can be installed into the designer without designer
releases.

## Changes

- `Cargo.toml` — drop `adaptive-card-core`; add
  `greentic-ext-runtime = { git = "...", tag = "v0.6.0" }` and
  `greentic-ext-contract` similarly
- `src/main.rs` — bootstrap `ExtensionRuntime`, scan
  `~/.greentic/extensions/{design,bundle,deploy}/`, install bundled AC
  ext on first run
- `src/ui/state.rs` — add `runtime: Arc<ExtensionRuntime>` to `AppState`
- `src/ui/tool_bridge/dispatch.rs` — replace 12-arm core match with
  `runtime.invoke_tool(...)` dispatch
- `src/ui/prompt_builder.rs` — replace `adaptive_card_core::prompt::*`
  with `runtime.aggregate_prompt_fragments("design")`
- `src/ui/routes/{validate,examples}.rs` — same replacement pattern
- `src/knowledge.rs` — DELETED (KB now via runtime aggregator)
- `assets/ext-bundles/greentic.adaptive-cards-1.6.0.gtxpack` — bundled
  fallback embedded via `include_bytes!`
- `CLAUDE.md` — updated architecture notes and bundle refresh instructions

## Known intentional regressions

The MVP AC extension stubs four tools that previously returned full
results from `adaptive-card-core`:
- `optimize_card`, `transform_card`, `template_card`, `data_to_card`

These now return `{"status": "not_implemented_in_v1_6", "note": "..."}`.
The AC extension v2 cycle will wire them through.

## Test plan

- [ ] `bash ci/local_check.sh` green
- [ ] `cargo test --workspace` green
- [ ] Manual chat round-trip with `validate_card` returns valid v1.6 output
- [ ] Bundled fallback installs `~/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/`
      on first run when home is empty
- [ ] Setting `DESIGNER_NO_BUNDLED_FALLBACK=1` skips the auto-install
- [ ] Snapshot test `tests/extension_parity.rs` passes (asserts core + runtime
      paths produce identical output for 8 working tools, documents
      known divergence for 4 stub tools)

## Migration notes for downstream

- Users with older `~/.greentic/` who upgrade designer: no action needed.
  Their existing extension dir takes precedence over the bundled fallback.
- Users on a fresh home: the bundled AC ext auto-installs and they're up.
- CI / docker builds: set `DESIGNER_NO_BUNDLED_FALLBACK=1` if reproducible
  no-side-effect behaviour is required.

## Related

- `greentic-biz/greentic-designer-extensions@v0.6.0` (runtime + contract)
- `greentic-biz/greentic-adaptive-card-mcp@v0.2.0` (AC ext source)
- Plan doc: `greentic-biz/greentic-designer-extensions/docs/superpowers/plans/2026-04-17-docs-and-designer-refactor.md` Part B
````

---

## 9. Rollback plan

If Phase B reveals a non-recoverable regression after the AC ext path is
the default:

```bash
git revert <cutover commit SHA>
git push
```

This restores `dispatch_via_core` and the `adaptive-card-core` dep.
Bundled fallback can stay (harmless if the runtime path is gone — extra
dir on disk).

---

## 10. Estimated effort

- Phase A: 1-2 days
- Phase B: 1 day
- Phase C: 1 day
- Phase D + PR review iteration: 1-2 days

**Total: 4-6 working days of focused work.**

Subagent-driven-development pattern (per
`/home/bimbim/.claude/plugins/cache/claude-plugins-official/superpowers/`)
is recommended — dispatch a fresh subagent per phase, review between
each.

---

## 11. Hand-off checklist for the next session

When you start a fresh session against this plan, paste this prompt:

> Read `/home/bimbim/works/greentic/docs/2026-04-17-plan-4b-designer-refactor-instructions.md`
> end to end. Then execute Plan 4B against the `greentic-designer/` repo
> using the subagent-driven-development pattern. Phase A first
> (parallel paths + snapshot test), report back before flipping the
> default in Phase B. Branch off `main` as `feat/extension-runtime-integration`.
> Don't push until I review the snapshot diffs.

Good luck.
