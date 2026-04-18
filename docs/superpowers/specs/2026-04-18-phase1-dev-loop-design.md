# Phase 1 — Track B: Dev Loop (`gtdx dev`)

**Status:** Design, awaiting implementation plan
**Date:** 2026-04-18
**Parent:** `2026-04-18-dx-10-10-roadmap.md` (subsystem S2)
**Owner crate:** `greentic-ext-cli`

## 1. Goal

Give developers a one-command inner loop: edit source → automatic rebuild → automatic reinstall into a local registry → consumer (designer app) hot-reloads via the existing `greentic-ext-runtime` watcher. Target: edit-to-reinstall ≤ 5 s (debug, incremental).

## 2. CLI UX

```
gtdx dev [OPTIONS]

Options:
      --once              Build + install once, then exit (CI-friendly)
      --watch             Continuous mode (default)
      --install-to <DIR>  Target registry dir         [default: $GTDX_HOME/registries/local]
      --no-install        Build only, skip installation
      --release           cargo component build --release (default: debug for speed)
      --ext-id <ID>       Override describe.json id (for multi-variant dev)
      --debounce-ms <MS>  FS watch debounce           [default: 500, Windows: 1000]
      --log <LEVEL>       Log filter level             [default: info]
      --manifest <PATH>   Path to Cargo.toml          [default: ./Cargo.toml]
      --force-rebuild     Ignore cargo cache, full rebuild
      --format <FORMAT>   human | json                 [default: human]
```

## 3. State Machine

```
   ┌─────────────┐
   │   Idle      │◄───────────────────┐
   └──────┬──────┘                    │
          │ file change               │ debounce elapsed, no new events
          ▼                           │
   ┌─────────────┐                    │
   │  Debouncing │────────────────────┘
   └──────┬──────┘
          │ another change
          ▼ (reset debounce timer)
   ┌─────────────┐
   │  Building   │
   └──────┬──────┘
          │ build OK         build failed
          ▼                    │
   ┌─────────────┐             │
   │  Packing    │             │
   │  (.gtxpack) │             ▼
   └──────┬──────┘      ┌─────────────┐
          │             │  Error      │
          ▼             │  display    │
   ┌─────────────┐      └──────┬──────┘
   │ Installing  │             │
   └──────┬──────┘             │
          │                    │
          └────► Idle ◄────────┘
```

- All transitions emit a `DevEvent` (internal enum) for logging / JSON output.
- `Ctrl+C` triggers graceful shutdown: stop watcher, drain in-flight, exit 0.

## 4. Components (module breakdown)

| Module | Purpose | Target LOC |
|--------|---------|------------|
| `dev_cmd.rs` | orchestrator, state machine, signal handling | ~200 |
| `watcher.rs` | thin wrapper on `notify-debouncer-full`, filters paths | ~150 |
| `builder.rs` | invoke `cargo component build`, stream stdout/stderr | ~200 |
| `packer.rs` | shared with Track C `pack_writer` | (not counted twice) |
| `installer.rs` | wrap existing `greentic-ext-registry::Installer` | ~150 |
| **Total new dev-loop code** | | **~700 LOC** |

## 5. File Watching Rules

- **Watch:** `src/`, `wit/`, `describe.json`, `i18n/`, `schemas/`, `prompts/`, `Cargo.toml`.
- **Ignore:** `target/`, `.git/`, `dist/`, `*.swp`, `*.tmp`, `.DS_Store`, `~*`.
- Debounce default 500 ms (Linux/macOS), 1000 ms (Windows), overridable via `--debounce-ms`.
- Editor swap/backup patterns are excluded to avoid spurious rebuilds.

## 6. Output UX

Default human format:

```
$ gtdx dev
  gtdx dev v0.7.0 — watching ./demo
  registry: file:///home/user/.greentic/registries/local
  kind: design  id: com.example.demo@0.1.0

  [14:32:01] idle. watching 18 files.
  [14:32:14] change detected: src/lib.rs
  [14:32:14] debouncing (500ms)...
  [14:32:15] building (debug, incremental)...
  [14:32:17] ✓ build ok (2.1s)
  [14:32:17] ✓ packed com.example.demo-0.1.0.gtxpack (48 KB)
  [14:32:17] ✓ installed. ready.

  [14:33:52] change detected: describe.json
  [14:33:53] building...
  [14:33:55] ✗ describe.json validation failed:
             - metadata.version: "0.1" does not match semver pattern
             Fix the error and save to retry.
  [14:33:55] idle (last build failed).
```

`--format json` emits one JSON line per `DevEvent` for IDE consumption:

```json
{"ts":"2026-04-18T14:32:15Z","event":"build_start","profile":"debug"}
{"ts":"2026-04-18T14:32:17Z","event":"build_ok","duration_ms":2100,"wasm_size":48512}
{"ts":"2026-04-18T14:32:17Z","event":"install_ok","registry":"file:///.../local","version":"0.1.0"}
```

## 7. Error Semantics

| Error | Display | Exits? |
|-------|---------|--------|
| `cargo component` missing | `error: cargo-component not installed. run: cargo install --locked cargo-component` | yes (127) |
| WASM build compile error | print compiler output verbatim, tag `✗ build failed` | no (stay watching) |
| `describe.json` schema invalid | parse error + JSON pointer to field | no |
| WIT bindgen error | print wit-bindgen output, hint "check wit/deps/greentic/*" | no |
| Pack / install I/O error | show path + permission; fatal if registry not writable | no (retry on next change) |
| `Ctrl+C` | graceful: stop watcher, drain pending, exit 0 | yes (0) |

## 8. Incremental Speed Optimizations

1. Default to `debug` profile (cargo cache is hottest).
2. Skip re-pack if WASM + describe + assets hashes unchanged.
3. Skip install if computed `.gtxpack` SHA256 equals installed version.
4. Watcher filters exclude `target/` (the single biggest source of noise).
5. Use `cargo component build --offline` if lockfile is pinned and registry not reachable.

Target: single-file source edit → reinstall complete in **< 3 s** (debug, incremental, warm cache).

## 9. Hot-Reload Chain

`gtdx dev` does NOT embed a runtime. It writes to the filesystem registry; the consumer (designer app or any other tool) uses the existing `greentic-ext-runtime` debounced FS watcher + `ArcSwap` hot-reload to pick up changes.

```
gtdx dev ──writes──► ~/.greentic/registries/local/<id>/<ver>/
                                       │
                                       ▼
                  designer app ──watches──► runtime reload via ArcSwap
```

### 9.1 Consumer readiness

Phase 1 assumes the consumer (designer app) is already wired to watch the local registry directory. If not, developers can still verify the loop by running `gtdx info <id>` after each build — `info` re-reads the registry and shows the latest version. No extra consumer stub is shipped in Phase 1.

## 10. Platform Notes

- **Linux/macOS:** `inotify` / `FSEvents` via `notify`. Works out of the box.
- **Windows:** `ReadDirectoryChangesW`. Default debounce bumped to 1000 ms; docs note this.
- **WSL:** File watching from Windows-mounted paths is unreliable. Docs recommend `--debounce-ms 2000` or moving projects to native WSL FS (`~/`).

## 11. Testing

- **Unit** `dev_cmd::tests::state_machine` — feed mock watcher events, assert state transitions and counters.
- **Integration** `tests/cli_dev_once.rs`:
  - Scaffold via `gtdx new`, run `gtdx dev --once`, assert `.gtxpack` landed in target registry.
- **Smoke test** watch mode (`tests/cli_dev_watch.rs`):
  - Spawn `gtdx dev`, poll stdout for "ready", `touch src/lib.rs`, wait up to 10 s, assert new install shows up, kill process.
  - Gated behind `GTDX_RUN_SMOKE=1` env var (not on every CI run — flaky on slow CI hardware).

## 12. Error Handling: Deep Dive on Build Failures

When `cargo component build` fails, `builder.rs` does NOT try to reinterpret the compiler output. It streams stdout/stderr verbatim so the user sees familiar Cargo diagnostics. After the stream ends, `dev_cmd` logs a single summary line:

```
[14:32:17] ✗ build failed (2.1s). Fix errors above and save to retry.
```

This keeps Cargo errors authoritative and avoids introducing a new error vocabulary for WASM builds.

## 13. Acceptance Criteria

1. `gtdx dev --once` on a scaffolded project completes and produces an installed `.gtxpack`.
2. `gtdx dev --watch` detects a `src/lib.rs` save and reinstalls within 5 s (incremental, warm cache).
3. Build failures do NOT kill the watch process; next save triggers a retry.
4. `Ctrl+C` exits cleanly (0) with no orphaned child processes (no leaked `cargo component` runs).
5. `--format json` emits valid line-delimited JSON for all lifecycle events.
6. Works on Linux, macOS, Windows.

## 14. Non-Goals (Phase 1)

- ❌ Auto-open browser / designer app (developer opens separately)
- ❌ Multi-extension watch in one process (one dir = one `gtdx dev`)
- ❌ Remote dev / SSH / devcontainer (future, Phase 5)
- ❌ Integrated test runner `gtdx dev --test` (Phase 5)
- ❌ Providing a sample designer consumer stub (assume consumer already watches local registry)
