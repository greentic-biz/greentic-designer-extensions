# Anchored verify (TOFU) + registry eviction — plan

Closes follow-ups **B** and **C** from
`greentic-admin/docs/superpowers/specs/2026-07-15-guardrail-followups-design.md`
(as corrected by greentic-admin #334). Both live in `greentic-ext-runtime`, so
they share one branch and one rev cascade.

## Why these two together

The cascade is the cost, not the code. Bumping `greentic-ext-runtime`'s rev
forces `greentic-runner` (aw-runtime + runner-host) → `greentic-dw-authoring`
(lockstep) → `greentic-designer`. Paying that once for both fixes is the whole
reason to pair them.

## What we found that the spec did not

Two things turned up while verifying the spec's claims. Both are in scope
because leaving either in place would make the fixes cosmetic.

### 1. The watcher path verifies nothing at all

`handle_added_or_modified` (`runtime.rs:447`) calls
`LoadedExtension::load_from_dir` directly. `verify_dir_signature` is never
called on that path — not the anchored check we are adding, not even the
existing integrity check. It is not dead code: `greentic-designer/src/ui/mod.rs:983`
calls `start_watcher()` and holds the guard for the whole server lifetime.

So anyone who can write to a watched extension dir gets code execution with no
signature check whatsoever, and does not need to re-sign anything. Adding TOFU
to `register_loaded_from_dir` alone would leave that door open — an attacker
would simply use it.

### 2. Follow-up C has a second, worse instance

`handle_added_or_modified` and `handle_removal` never touch
`capability_registry` at all. So a hot-reloaded extension's capabilities never
appear, and a **removed** extension's capabilities stay in the registry forever.
That is precisely the false positive that lets admin's guardrail preflight pass
a policy the runtime then fails closed on.

`register_loaded_from_dir` has a third variant of the same bug: it clones every
existing offering forward before appending, so re-registering one dir
**duplicates** that extension's offerings without bound.

## Design

### Trust model — TOFU, per extension id

Decided by the human (2026-07-16) over a single `GREENTIC_EXT_PUBLIC_KEY`
anchor. Rationale: a single global key is only correct if every extension is
signed by one publisher, which is false — the store carries third-party
extensions. TOFU needs no key-custody decision (that is D.5) and closes the
stated threat directly: swap a guardrail, re-sign with your own key, get
rejected because the pinned key differs.

`verify_dir_signature` gains two steps after the existing self-consistent check:

```
1. verify_describe_self_consistent(describe)     # integrity — unchanged
2. verify_describe_with_key(describe, key)       # NEW: authenticity
3. trust_store.pin_or_verify(id, key_b64)        # NEW: TOFU anchor
```

Step 2 must come before step 3 so a bad signature never poisons the store —
this mirrors `registry/src/verify.rs:52-56`, which documents that ordering for
the same reason.

The `dev-allow-unsigned` / `GREENTIC_EXT_ALLOW_UNSIGNED` bypass stays first and
keeps its skip-entirely semantics.

### Reuse, not reimplementation

`greentic-extension-sdk-registry::trust_store::TrustStore` already exists,
is already `pub`, is already tested, and already carries the file-locking and
atomic-rename care this needs. We depend on it rather than porting ~90 lines
that would drift.

It is a heavy dep for the job (it drags `oci-client`, `reqwest`, `dialoguer`,
`zip`, `tokio`, `sdk-state`). We accept that: it is compile-time weight only,
`ext-runtime` already carries `reqwest`/`tokio`/`wasmtime`, and the alternative
— relocating `trust_store` into `sdk-contract` — needs a new SDK tag and a
version bump in every consumer, which is the release-train problem this repo
has been bitten by before. `sdk-registry` ships in the tag consumers already
use (`v1.3.0-research.1`), so no SDK change and no new tag are needed.

We use `TrustStore` directly rather than `verify_authenticity`, which is
`pub(crate)` and would need an SDK change to reach.

### Trust root — resolve it the way gtdx does, not from DiscoveryPaths

The pins must land in the same store `gtdx` writes, or the check is theatre
against a store nobody else populates.

`gtdx` resolves `$GREENTIC_HOME`, else `~/.greentic` (`cli/src/main.rs:116-124`,
`main.rs:17`). Neither the designer nor the runner honors `GREENTIC_HOME`.

`DiscoveryPaths::home()` (`user.parent()`) happens to equal `~/.greentic` today,
but only by coincidence of three independent computations, and it goes
**wrong-but-plausible** under either override: `GREENTIC_HOME=/custom gtdx
install` pins to `/custom/trust/`, while the runtime would look in
`~/.greentic/trust/`; and the runner's `GREENTIC_EXTENSIONS_DIR` override makes
`home()` the parent of an arbitrary dir. In both cases a TOFU check would
silently re-pin into a *different* store instead of matching — a silent failure,
which is the exact bug class this whole epic exists to kill.

So: resolve the trust root as `$GREENTIC_HOME` else `~/.greentic`, mirroring
gtdx exactly. Do **not** derive it from `DiscoveryPaths`. The trust store keys
publisher keys by extension id; it has no relationship to where the extension
dir lives, so decoupling the two is correct, not a compromise.

`RuntimeConfig` gains `trust_root: Option<PathBuf>` (`None` = resolve as above)
plus a `with_trust_root()` builder, so tests can point at a temp dir.

### Registry — make it a pure function of `loaded`

The registry holds nothing that is not derivable from the loaded describes
(`extension_id`, `cap_id`, `version`, `kind`, and `export_path` which is always
`String::new()` here). So rather than patch eviction into three call sites,
rebuild it from the `loaded` map:

```rust
fn rebuild_registry(loaded: &HashMap<ExtensionId, LoadedExtensionRef>)
    -> Result<CapabilityRegistry, RuntimeError>
```

Every path that mutates `loaded` stores a registry rebuilt from the new map.
Eviction-on-replace, the removal leak, and the duplicate-on-reload bug all stop
existing rather than each being fixed. Less code than three patches.

## Tasks

**T1 — dep + patch.** Add `greentic-extension-sdk-registry = "=1.3.0-research.1"`
to `crates/greentic-ext-runtime/Cargo.toml`; add `-registry` (and `-state` if the
resolve needs it) to the workspace `[patch.crates-io]` alongside the existing
`-contract`/`-testing` path patches. Verify: `cargo check --workspace --locked`.

**T2 — trust root.** `RuntimeConfig.trust_root: Option<PathBuf>` +
`with_trust_root()`. A `resolve_trust_root(&self) -> PathBuf` that returns the
override, else `$GREENTIC_HOME`, else `~/.greentic`. Unit-test all three arms
(use the existing `EnvGuard` for the env arm).

**T3 — anchored verify.** `verify_dir_signature` becomes `&self` (to reach the
trust root). Insert steps 2 and 3 above. Parse the key with the same
`ed25519:`-prefix-tolerant handling as `registry/src/verify.rs:11-23` (the SDK's
own parser is private, so mirror it — and say so in a comment). Map both new
failures to `RuntimeError::SignatureInvalid { extension_id, reason }`; the
`PublisherKeyChanged` reason must name both the pinned and presented key so an
operator can tell a rotation from an attack.

**T4 — close the watcher hole.** `handle_added_or_modified` must call
`verify_dir_signature` before loading. Same gate as `register_loaded_from_dir`,
no exceptions.

**T5 — registry rebuild.** Add `rebuild_registry`; call it from
`register_loaded_from_dir`, `handle_added_or_modified`, and `handle_removal`.
Delete the clone-forward loop.

**T6 — tests.** Every one of these must fail before its fix and pass after:

| Test | Asserts |
|---|---|
| `tofu_pins_on_first_load` | first load of a signed fixture pins; `<root>/trust/publishers.json` names the key |
| `tofu_accepts_same_key_on_reload` | re-registering the same fixture succeeds |
| `tofu_rejects_different_key_same_id` | a second `signed_fixture` with the **same id** (fresh random key) is rejected `SignatureInvalid`; reason names both keys |
| `watcher_path_rejects_unsigned` | the `handle_added_or_modified` path rejects an unsigned fixture — **guards T4** |
| `watcher_path_rejects_tampered` | same, tampered |
| `registry_evicts_prior_offerings_on_replace` | re-register an id whose describe drops cap X → X is gone from `offerings()`, no duplicates |
| `registry_drops_offerings_on_removal` | `handle_removal` → that extension's caps leave `offerings()` |
| `trust_root_prefers_env_then_home` | the three arms of T2 |

`signed_fixture(kind, id, ver)` already generates a fresh random key per call and
returns it, so "same id, different key" needs no new helper.

## Global constraints

- `bash ci/local_check.sh` must pass (fmt + clippy `-D warnings` + test + release
  build + wit sync). Run it before declaring done; do not `--no-verify`.
- English only in source, tests, comments, tracing.
- No `unwrap()`/`panic!()` in production paths.
- Conventional Commits. **No AI attribution** in commits or PR bodies.
- Do not touch the main checkout at
  `/home/bima-pangestu/projects/Works/greentic/greentic-designer-extensions`
  (it sits on another branch). Work only in the worktree.

## Out of scope

- **`sign_describe`'s `key_id: None`.** The corrected spec called this a gap.
  Under TOFU it is not: the anchor selects a key by *extension id*, not by
  `key_id`. Wiring `key_id` would add a field nothing reads until D.5. Dropped
  deliberately — YAGNI, and say so if asked.
- **D.5** — KMS-rooted cert chain, key rotation, revocation. Still blocked on
  key custody; TOFU is explicitly the protection available without it.
- The rev cascade (runner → dw-authoring → designer) — separate PRs, after this
  merges.

## Rollout risk to state in the PR body

TOFU rejects an update signed by a key different from the first-seen one. If two
developers publish the same extension from their own
`~/.greentic/keys/dev-local.key`, the second publish will be **rejected** on a
runtime that pinned the first. This is the intended security property, and it
fails loudly (`PublisherKeyChanged` naming both keys) rather than silently — but
it is a behaviour change and the PR must say so plainly rather than let someone
discover it in the field.

Nothing pins today, so upgrading is safe: every install pins on its first load
after the upgrade.
