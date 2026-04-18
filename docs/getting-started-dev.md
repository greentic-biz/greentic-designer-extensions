# Developer Inner Loop — `gtdx dev`

This is the companion doc to `getting-started-scaffolding.md` (which covers `gtdx new`).
Once you have a scaffolded extension, `gtdx dev` watches the source tree,
rebuilds on every save, packs the output as a `.gtxpack`, and installs it into
`~/.greentic/` where the Greentic Designer picks up changes via its hot-reload
chain.

## Quick start

```bash
gtdx new my-ext
cd my-ext
gtdx dev
```

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
- **Install step fails with schema error.** The current Track A scaffold
  templates emit a `describe.json` shape that does not yet match
  `describe-v1.json`. Until those templates are updated, use
  `gtdx dev --once --no-install` to verify the build-pack loop, or edit the
  scaffolded `describe.json` manually to conform.
