# Designer Extension — Docs + Designer Refactor Plan (4 of 4)

> **For agentic workers:** Use superpowers:subagent-driven-development. Steps use checkbox syntax.

**Goal:** Complete the ecosystem:
- **Part A** (this session): ship core documentation — tutorials, references, spec files — so Osora + Telco X can build their own extensions against a documented contract.
- **Part B** (deferred to a dedicated session against the `greentic-designer` repo): refactor the designer to drop `adaptive-card-core` as a direct dep and consume the extension system runtime instead, shipping the AC extension as a bundled fallback for first-run UX.

**Why split:** Part B touches a different git repo (`greentic-designer`), requires coordinating branches across 2 repos, and involves real refactor risk. Part A is self-contained in this repo and directly unblocks external contributors.

**Prerequisites:** Plans 1-3 shipped (`v0.3.0-ac-reference` tag present).

---

## Part A — Documentation (executed in this session)

### Phase A1 — Core references

#### Task A1.1: `describe-json-spec.md` — full field reference

**Files:** `docs/describe-json-spec.md`

- [ ] Write a reference document covering every field of `describe.json` v1, including:
  - Top-level fields: `apiVersion`, `kind`, `metadata`, `engine`, `capabilities`, `runtime`, `contributions`, `signature`
  - Kind-specific `contributions`: design / bundle / deploy variants with concrete JSON examples
  - JSON Schema reference with `$ref` to the embedded schema
  - Field constraints (e.g., `metadata.id` regex, `metadata.version` semver)
  - A worked example for each of the 3 kinds

Target length: 500-800 lines markdown. Reference the actual schema at `crates/greentic-extension-sdk-contract/schemas/describe-v1.json`.

Commit: `docs: add describe.json v1 spec reference`

#### Task A1.2: `wit-reference.md` — WIT interfaces reference

**Files:** `docs/wit-reference.md`

- [ ] Document every WIT package + interface + record type shipped by this repo. Include:
  - Table of contents per package (base, host, design, bundle, deploy)
  - Each interface: exported functions with signatures + human prose
  - Each record: field names, types, semantics
  - A worked call-and-return example for the design tools interface

Reference the actual `wit/*.wit` files — format as literate prose around copy-paste of relevant WIT fragments.

Commit: `docs: add WIT interfaces reference`

#### Task A1.3: `capability-registry.md` — matching + degraded semantics

**Files:** `docs/capability-registry.md`

- [ ] Cover:
  - What a capability is (namespaced ID + semver)
  - `offered` vs `required` in describe.json
  - Matching rules (semver, highest wins, conflict resolution)
  - Degraded state (when + why, UX implications)
  - Cycle detection
  - Host capabilities (`greentic:host/*`)

Target: 200-300 lines. Include a small worked example with 2-3 extensions.

Commit: `docs: add capability registry + matching reference`

#### Task A1.4: `cli-reference.md` — every `gtdx` subcommand

**Files:** `docs/cli-reference.md`

- [ ] Table of contents. For each subcommand:
  - Synopsis
  - Arguments + flags
  - What it does
  - Example invocation + example output

11 subcommands: validate, list, install, uninstall, search, info, login, logout, registries, doctor, version.

Commit: `docs: add gtdx CLI reference`

### Phase A2 — Tutorials for external authors

#### Task A2.1: `how-to-write-a-design-extension.md`

**Files:** `docs/how-to-write-a-design-extension.md`

- [ ] Step-by-step tutorial walking someone through building a design extension from scratch. Use `greentic.adaptive-cards` as the worked example.
  - Prerequisites: Rust 1.91, `cargo-component`, `rustup target add wasm32-wasip1 wasm32-wasip2`
  - Scaffold a crate (by hand — `gtdx new` scaffolding is v1.1)
  - Write describe.json — kind=DesignExtension
  - Pick cap IDs + versions
  - WIT world setup
  - Implement the 4 exports (tools, validation, prompting, knowledge)
  - Build + package → .gtxpack
  - Test locally: `gtdx install ./your-extension-*.gtxpack`
  - Publish to Greentic Store: `gtdx publish`

Target: 300-600 lines, concrete + copy-pasteable commands.

Commit: `docs: add how-to-write-a-design-extension tutorial`

#### Task A2.2: `how-to-write-a-bundle-extension.md`

- [ ] Similar to A2.1 but for bundle extensions. No full reference implementation yet (Plan 5-like cycle), so this doc is framed as "contract + example" — include a minimal bundle-ext skeleton that returns a stub `bundle-artifact` from `render()`.

Commit: `docs: add how-to-write-a-bundle-extension tutorial`

#### Task A2.3: `how-to-write-a-deploy-extension.md`

- [ ] Same pattern for deploy-extensions. Include a desktop-target stub that pretends to deploy (writes a marker file). Note that full AWS/GCP/K8s deploy-exts come in later cycles.

Commit: `docs: add how-to-write-a-deploy-extension tutorial`

### Phase A3 — Supplementary docs

#### Task A3.1: `cross-extension-communication.md` — broker pattern

**Files:** `docs/cross-extension-communication.md`

- [ ] Cover:
  - When to use cross-ext calls vs keeping things in your own extension
  - The `host.broker.call-extension` WIT function
  - Permission gating via describe.json `runtime.permissions.callExtensionKinds`
  - Max-depth protection
  - Graceful degradation pattern (try broker, catch error, fall back to warning)
  - Worked example: flow-design-ext calling AC-ext for card validation

Target: 150-250 lines.

Commit: `docs: add cross-extension communication guide`

#### Task A3.2: `permissions-and-trust.md`

- [ ] Cover:
  - Declared permissions (network, secrets, callExtensionKinds)
  - Default-deny semantics
  - Trust policies (strict / normal / loose)
  - Ed25519 signing flow
  - `gtdx login` credential storage
  - `gtdx install` prompt UX
  - Trust escalation risks + mitigations

Target: 200-300 lines.

Commit: `docs: add permissions + trust reference`

### Phase A4 — README + docs index

#### Task A4.1: Update `README.md` with docs TOC

- [ ] Replace the existing README content with:
  - One-paragraph project summary
  - Installation: `cargo install --path crates/greentic-ext-cli` (or from crates.io when published)
  - Quickstart: `gtdx install <path-to-gtxpack>`
  - Link to each doc in `docs/`
  - Link to concept.md + spec + plans

Target: 80-150 lines.

Commit: `docs: update README with docs TOC + quickstart`

#### Task A4.2: Create `docs/README.md` docs index

- [ ] Index of all docs with one-line summaries. Groups:
  - Reference: describe-json-spec, wit-reference, capability-registry, cli-reference
  - Tutorials: how-to-write-a-{design,bundle,deploy}-extension
  - Guides: cross-extension-communication, permissions-and-trust
  - Architecture: concept.md, spec, plans

Commit: `docs: add docs index`

### Phase A5 — Milestone

- [ ] Run `ci/local_check.sh` — all green
- [ ] Tag `v0.4.0-docs` with message "Core documentation shipped; designer refactor pending in follow-up session"

---

## Part B — Designer refactor (DEFERRED to separate session)

This section is a **plan doc only** — execution happens in a fresh session working against `greentic-designer` repo with a dedicated branch there. Included here for completeness.

### Scope overview

**Target repo:** `/home/bimbim/works/greentic/greentic-designer/` (separate git repo, NOT part of the `greentic-designer-extensions` workspace).

**Deliverable:** `greentic-designer` no longer depends on `adaptive-card-core` directly. Instead, it consumes `greentic-ext-runtime` and dynamically loads the AC extension at startup. Bundled fallback `include_bytes!` ensures first-run users get AC support without manual install.

### Phase B1 — Two-path migration (no regression)

- **B1.1**: Add `greentic-ext-runtime` dep to designer's Cargo.toml (git dep, pin to v0.3.0-ac-reference or newer). Keep `adaptive-card-core` dep unchanged.
- **B1.2**: In `src/main.rs`, instantiate `ExtensionRuntime` at startup. Expose through `AppState`. Do not wire to routes yet.
- **B1.3**: Add env var `DESIGNER_USE_EXTENSIONS=1` — feature flag that switches routes from direct AC calls to runtime dispatch.
- **B1.4**: Rewrite `src/ui/tool_bridge/dispatch.rs` with dual path — if `USE_EXTENSIONS`, call runtime; else keep existing 12-arm match.
- **B1.5**: Rewrite `src/ui/routes/validate.rs` similarly.
- **B1.6**: Rewrite `src/ui/routes/examples.rs` to read from runtime knowledge aggregator when flag is set.
- **B1.7**: Rewrite `src/ui/prompt_builder.rs` to use runtime-aggregated prompt fragments when flag is set.
- **B1.8**: Snapshot test: with flag on + AC extension installed at `~/.greentic/extensions/design/greentic.adaptive-cards-1.6.0/`, produce the same LLM tool list + prompt as without the flag.

### Phase B2 — Cutover

- **B2.1**: Remove `DESIGNER_USE_EXTENSIONS` flag, make runtime path the only path.
- **B2.2**: Delete `adaptive-card-core` from Cargo.toml.
- **B2.3**: Delete `src/knowledge.rs` (obsolete wrapper).
- **B2.4**: Delete the 12-arm hardcoded match in `tool_bridge/dispatch.rs`.
- **B2.5**: Update CLAUDE.md.
- **B2.6**: Final CI green + ship.

### Phase B3 — Bundled fallback for first-run UX

- **B3.1**: During build, embed `greentic.adaptive-cards-1.6.0.gtxpack` via `include_bytes!`. If the designer starts and finds no AC extension installed, auto-install the bundled bytes to the user's `~/.greentic/extensions/design/`.
- **B3.2**: Add CLI flag `--no-bundled-fallback` to opt out.
- **B3.3**: Warn user clearly when bundled fallback activates ("Installing bundled greentic.adaptive-cards@1.6.0 for first-run. Run `gtdx uninstall greentic.adaptive-cards` to disable.").

### Known coordination points

- `greentic-designer` uses its own git workflow with `cargo-husky` pre-commit. Follow that repo's CLAUDE.md, do not impose patterns from this repo.
- `adaptive-card-core` stays published by `greentic-adaptive-card-mcp` repo — no changes there.
- After the cutover, the AC extension's actual behavior may differ from the old direct `adaptive-card-core` behavior (e.g., our MVP AC extension has stub `optimize_card` / `transform_card`). That regression is **intentional and documented** in Plan 3 — downstream consumers upgrade the AC extension independently of designer releases.

### Rough sizing

- B1: ~5 days
- B2: ~1-2 days
- B3: ~1 day

Total: ~1.5 weeks of focused work, ideally via subagent-driven-development against the designer repo.

---

## Execution Handoff (for Part A)

Part A (docs) is 9 files plus README + docs index — ~11 commits. Can be dispatched in one or two batches. Each doc is independent; no cross-file dependencies beyond link-checking.
