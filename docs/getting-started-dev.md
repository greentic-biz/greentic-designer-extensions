# Developer Inner Loop — `gtdx dev`

This is the companion doc to `getting-started-scaffolding.md` (which covers `gtdx new`).
Once you have a scaffolded extension, `gtdx dev` watches the source tree,
rebuilds on every save, packs the output as a `.gtxpack`, and installs it into
`~/.greentic/` where the Greentic Designer picks up changes via its hot-reload
chain.

## Quick start

```bash
gtdx new my-ext --kind design --id greentic.my-ext
cd my-ext

# Required today: the scaffold renders unresolvable WIT versions.
sed -i -e 's|\(greentic:extension-host/[a-z0-9-]*\)@0\.2\.0|\1@0.1.0|g' \
       -e 's|\(greentic:extension-design/[a-z0-9-]*\)@0\.2\.0|\1@0.3.0|g' \
       wit/world.wit

gtdx dev
```

*(Fixed in greentic-designer-sdk#105; still applies to any released `gtdx`.)* Without that rewrite `gtdx dev` fails on its first build with
`package 'greentic:extension-host@0.2.0' not found` — see
[getting-started-scaffolding.md](./getting-started-scaffolding.md#known-issue--a-fresh-scaffold-does-not-build)
for the cause and the per-kind table.

The first build may take ~30–60 s (cold cargo cache). Subsequent incremental
rebuilds are typically below 5 seconds.

## Flags

| Flag                    | Purpose                                                         |
|-------------------------|-----------------------------------------------------------------|
| `--once`                | Build + install once, then exit. Good for CI smoke tests.       |
| `--watch`               | Continuous mode (default).                                      |
| `--no-install`          | Build and pack only — useful for offline/verify runs.           |
| `--release`             | Build with `--release`. Default is `debug` for speed.           |
| `--debounce-ms <MS>`    | File-watch debounce window. Default 500 ms (1000 ms on Windows).|
| `--format <FMT>`        | `human` (default) or `json` (one JSON line per lifecycle event).|
| `--manifest <PATH>`     | Path to the project's `Cargo.toml`. Default `./Cargo.toml`.     |
| `--force-rebuild`       | `cargo clean -p <crate>` first, then a full rebuild.             |
| `--mount <PATH>`        | Build + pack + install the extension at `<PATH>` once, exactly as `gtdx install` would. Conflicts with `--watch` / `--once`. |

## JSON output

`gtdx dev --format json` emits one JSON object per line. Each line has a `ts`
UTC ISO-8601 timestamp and an `event` tag (`build_start`, `build_ok`,
`pack_ok`, `install_ok`, `install_skipped`, `idle`, `error`, …). This makes it
trivial for editors and CI tools to consume the stream.

## Troubleshooting

- **Rebuilds keep firing on every save.** Confirm your editor isn't touching
  files inside `target/` or creating `*.swp` backups outside the scaffold.
  Bump `--debounce-ms` on slow filesystems (WSL, networked mounts).
- **`cargo-component` not installed.** Run
  `cargo install --locked cargo-component`.
- **No hot reload in the designer.** `gtdx dev` installs into
  `~/.greentic/extensions/<kind>/<id>-<version>/`. The designer must be
  configured to watch that directory (see the designer's integration guide).
- **Build fails with `package 'greentic:extension-host@0.2.0' not found`.**
  The scaffold's `wit/world.wit` versions are wrong; apply the rewrite in
  Quick start above. This is not a describe.json problem — the generated
  `describe.json` is valid v2 and passes `gtdx validate` as-is.
- **`gtdx lint` errors on an untouched scaffold.** Expected: it still emits a
  deprecated `engine` block and a `com.example.*` id. Neither blocks
  `gtdx publish`, which does not run lint.
- **Verifying the build/pack loop without installing.** `gtdx dev --once
  --no-install`.
