# greentic-designer-extensions

Extension system for [Greentic Designer](https://github.com/greenticai/greentic-designer).
Teaches the designer new capabilities at runtime through signed WebAssembly
extensions distributed via the Greentic Store. Three extension kinds share a
common foundation: design (author content), bundle (package output), and
deploy (ship packs to targets).

**Status:** Foundation shipped. WIT contracts, `describe.json` schema,
`greentic-ext-contract`, `greentic-ext-runtime`, and `greentic-ext-cli`
are functional. The `greentic.adaptive-cards` reference extension is the
first canonical implementation.

---

## Install `gtdx`

```
cargo install --path crates/greentic-ext-cli --locked
```

---

## Quickstart

```
# List installed extensions.
gtdx list

# Install the Adaptive Cards extension from a local pack.
gtdx install ./reference-extensions/adaptive-cards/greentic.adaptive-cards-1.6.0.gtxpack

# Show info about an extension in the default registry.
gtdx info greentic.adaptive-cards
```

---

## Repository Layout

```
greentic-designer-extensions/
├── wit/                          # WIT packages (single source of truth)
│   ├── extension-base.wit        # Shared types, manifest, lifecycle
│   ├── extension-host.wit        # Host services (logging, i18n, secrets, broker, http)
│   ├── extension-design.wit      # Design-extension interfaces
│   ├── extension-bundle.wit      # Bundle-extension interfaces
│   └── extension-deploy.wit      # Deploy-extension interfaces
├── crates/
│   ├── greentic-ext-contract/    # Types, describe.json schema, WIT bindings
│   ├── greentic-ext-runtime/     # Wasmtime loader, capability registry, broker
│   ├── greentic-ext-cli/         # gtdx binary
│   └── greentic-ext-testing/     # Test utilities for extension authors
├── reference-extensions/
│   └── adaptive-cards/           # First canonical design-extension
└── docs/                         # Specs, references, tutorials
```

---

## Documentation

See [docs/README.md](docs/README.md) for a full index. Quick links:

| Document | Description |
|----------|-------------|
| [docs/concept.md](docs/concept.md) | Executive overview — what the system is and why |
| [docs/describe-json-spec.md](docs/describe-json-spec.md) | `describe.json` v1 field reference |
| [docs/wit-reference.md](docs/wit-reference.md) | WIT package + interface reference |
| [docs/cli-reference.md](docs/cli-reference.md) | `gtdx` subcommand reference |
| [docs/how-to-write-a-design-extension.md](docs/how-to-write-a-design-extension.md) | Tutorial: build a design extension |
| [docs/how-to-write-a-bundle-extension.md](docs/how-to-write-a-bundle-extension.md) | Tutorial: build a bundle extension |
| [docs/how-to-write-a-deploy-extension.md](docs/how-to-write-a-deploy-extension.md) | Tutorial: build a deploy extension |
| [docs/cross-extension-communication.md](docs/cross-extension-communication.md) | Broker API and composition patterns |
| [docs/permissions-and-trust.md](docs/permissions-and-trust.md) | Permission model, trust policies, signing |
| [docs/capability-registry.md](docs/capability-registry.md) | Capability matching and degraded state |

---

## Design

- [Concept doc](docs/concept.md) — non-technical executive summary
- [Full design spec](docs/superpowers/specs/2026-04-17-designer-extension-system-design.md)
- [Implementation plans](docs/superpowers/plans/)

---

## Contributing

Read [CLAUDE.md](CLAUDE.md) for the project conventions before starting.
Key points: Rust 1.94, edition 2024, max 500 lines per file, English only
in source and commits, feature branches + PRs.

Run the full CI check before opening a PR:

```
ci/local_check.sh
```

---

## License

MIT
