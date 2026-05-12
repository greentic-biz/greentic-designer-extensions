# Documentation Index

This directory contains all documentation for `greentic-designer-extensions`.

---

## Reference

Precise specifications you look up while building or integrating.

| Document | Description |
|----------|-------------|
| [describe-json-spec.md](./describe-json-spec.md) | Full field reference for `describe.json` v1 — all fields, types, defaults, constraints, and complete JSON examples for all four kinds. |
| [wit-reference.md](./wit-reference.md) | Every WIT package and interface: record types, function signatures, and plain-English descriptions. |
| [capability-registry.md](./capability-registry.md) | Capability ID format, semver matching rules, degraded state, cycle detection, and host capabilities. |
| [cli-reference.md](./cli-reference.md) | Complete `gtdx` subcommand reference: synopsis, flags, descriptions, and example output. |

---

## Getting started

Quick walk-throughs for the inner-loop dev workflow.

| Document | Description |
|----------|-------------|
| [getting-started-scaffolding.md](./getting-started-scaffolding.md) | `gtdx new` scaffolding flow — pick a kind, point at a template, get a buildable crate. |
| [getting-started-dev.md](./getting-started-dev.md) | Inner-loop development with `gtdx dev` (auto-rebuild, watch, local install). |
| [getting-started-publish.md](./getting-started-publish.md) | Publishing flows with `gtdx publish --registry oci://…` or via the Greentic Store. |

---

## Tutorials

Step-by-step guides for building each extension kind from scratch.

| Document | Description |
|----------|-------------|
| [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md) | Build a `DesignExtension` — crate setup, WIT world, `describe.json`, `src/lib.rs`, build, package, and publish. Uses `greentic.adaptive-cards` as the running example. |
| [how-to-write-a-bundle-extension.md](./how-to-write-a-bundle-extension.md) | Build a `BundleExtension` — recipes interface, bundling interface, minimal stub that returns a pack artifact. |
| [how-to-write-a-deploy-extension.md](./how-to-write-a-deploy-extension.md) | Build a `DeployExtension` — targets interface, deployment interface, stub desktop deploy that writes a marker file. |
| [how-to-write-a-provider-extension.md](./how-to-write-a-provider-extension.md) | Build a `ProviderExtension` — flow-node execution against external services (messaging, webhooks, events). |
| [how-to-write-a-wasm-component-extension.md](./how-to-write-a-wasm-component-extension.md) | Lower-level: build a raw WASM Component extension when none of the specialized worlds apply. |

---

## Guides

Focused how-to articles for specific topics.

| Document | Description |
|----------|-------------|
| [cross-extension-communication.md](./cross-extension-communication.md) | How to call another extension via the host broker: permission setup, the `call-extension` function, depth limits, and graceful degradation. |
| [permissions-and-trust.md](./permissions-and-trust.md) | Declared permissions (network, secrets, broker), default-deny semantics, trust policies (strict/normal/loose), Ed25519 signing, and credential storage. |
| [lifecycle-management.md](./lifecycle-management.md) | Enable / disable / hot-reload contract — the state file format and watcher behavior the runtime exposes. |

---

## Architecture

Conceptual documents that explain the "why" behind the design.

| Document | Description |
|----------|-------------|
| [concept.md](./concept.md) | Non-technical executive summary — vision, the four extension kinds, capability registry, host broker, Greentic Store, and v1 scope. |
| [superpowers/specs/](./superpowers/specs/) | Full technical design specification. Start here before reading implementation code. |
| [superpowers/plans/](./superpowers/plans/) | Phased implementation plans (Plans 1-4 and beyond). |
