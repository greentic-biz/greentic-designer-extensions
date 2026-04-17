# greentic-designer-extensions

Extension system for [Greentic Designer](https://github.com/greenticai/greentic-designer).

Teaches the designer new capabilities at runtime through signed WebAssembly
extensions distributed via the Greentic Store. Three extension kinds share
a common foundation:

- **design-extension** — teaches the designer to author new content types
  (Adaptive Cards, Digital Workers, Telco X schemas, flow types)
- **bundle-extension** — packages designer output into deployable
  Application Packs (hosted, on-prem, OpenShift, Kubernetes, custom recipes)
- **deploy-extension** — ships Application Packs to deployment targets
  (desktop, AWS, GCP, Cisco on-prem)

## Status

**Draft specification.** Implementation has not started. See
[`docs/concept.md`](docs/concept.md) for the executive summary and
[`docs/superpowers/specs/2026-04-17-designer-extension-system-design.md`](docs/superpowers/specs/2026-04-17-designer-extension-system-design.md)
for the full technical design.

## Structure (planned)

```
greentic-designer-extensions/
├── wit/                    # WIT interfaces (single source of truth)
├── crates/
│   ├── greentic-ext-contract/   # Types + describe.json schema + WIT bindings
│   ├── greentic-ext-runtime/    # Wasmtime loader + registry + broker
│   ├── greentic-ext-cli/        # gtdx binary
│   └── greentic-ext-testing/    # Test utilities for extension authors
├── reference-extensions/
│   └── adaptive-cards/     # First canonical design-extension
├── templates/              # Scaffolds for gtdx new
└── docs/                   # Specs, how-to guides, API references
```

## Quick references

- [Concept doc (for sharing)](docs/concept.md)
- [Full design spec](docs/superpowers/specs/2026-04-17-designer-extension-system-design.md)

## Licensing

MIT (planned — not yet published).
