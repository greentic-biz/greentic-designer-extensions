# Scaffolding a new extension

`gtdx new` generates a ready-to-build Greentic Designer Extension project.

## Prerequisites

- Rust 1.94+ with `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `cargo-component`: `cargo install --locked cargo-component`

## Create

```
gtdx new my-ext --kind design --id com.example.my-ext
```

Supported kinds: `design`, `bundle`, `deploy`.

## What you get

```
my-ext/
├── .gtdx-contract.lock     WIT contract version + file hashes
├── Cargo.toml
├── README.md
├── build.sh
├── ci/local_check.sh
├── describe.json
├── i18n/en.json
├── src/lib.rs              WASM guest exports
└── wit/deps/greentic/...   Vendored WIT contract
```

## Next

```
cd my-ext
gtdx dev        # continuous rebuild + install (Track B, Phase 1)
gtdx publish    # produce and publish a .gtxpack (Track C, Phase 1)
```

See also: `docs/how-to-write-a-design-extension.md`.
