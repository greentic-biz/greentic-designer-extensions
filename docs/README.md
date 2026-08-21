# Documentation Index

This directory contains all documentation for `greentic-designer-extensions`.

---

## Reference

Precise specifications you look up while building or integrating.

| Document | Description |
|----------|-------------|
| [describe-json-spec.md](./describe-json-spec.md) | Full field reference for `describe.json` **v2** — all fields, types, defaults, cross-field invariants, the v1→v2 migration table, and complete validated examples. Start here: for a v2 extension, `contributions.tools[]` is the only source of tool metadata. |
| [wit-reference.md](./wit-reference.md) | Every WIT package and interface: record types, function signatures, and plain-English descriptions. |
| [capability-registry.md](./capability-registry.md) | Capability ID format, semver matching rules, degraded state, cycle detection, and host capabilities. |
| [cli-reference.md](./cli-reference.md) | Complete `gtdx` subcommand reference: synopsis, flags, descriptions, and example output. |

---

## Getting started

Quick walk-throughs for the inner-loop dev workflow.

| Document | Description |
|----------|-------------|
| [getting-started-scaffolding.md](./getting-started-scaffolding.md) | `gtdx new` scaffolding flow — all seven kinds, plus the **known issue**: every kind except `mcp` renders unresolvable WIT versions and needs a one-line fix before its first build. |
| [getting-started-dev.md](./getting-started-dev.md) | Inner-loop development with `gtdx dev` (auto-rebuild, watch, local install). |
| [getting-started-publish.md](./getting-started-publish.md) | Publishing flows with `gtdx publish --registry oci://…` or via the Greentic Store. |

---

## Tutorials

Step-by-step guides for building each extension kind from scratch.

| Document | Description |
|----------|-------------|
| [how-to-write-a-design-extension.md](./how-to-write-a-design-extension.md) | Build a `DesignExtension` — `gtdx new`, declaring tools in `describe.json`, implementing `invoke-tool`, permissions, publishing, and adding a flow-editor node. Uses `greentic.calendly` as the running example. |
| [how-to-write-a-bundle-extension.md](./how-to-write-a-bundle-extension.md) | Build a `BundleExtension` — recipes and bundling interfaces, the `render` contract, and the v2 `contributions.recipes` shape. |
| [how-to-write-a-deploy-extension.md](./how-to-write-a-deploy-extension.md) | Build a `DeployExtension` — targets and deployment interfaces, the async deploy/poll/rollback contract, and why `contributions.targets` no longer exists. |
| [how-to-write-a-provider-extension.md](./how-to-write-a-provider-extension.md) | Build a `ProviderExtension` — choosing among the six provider worlds, channel profiles and card tiers, and the separate runtime component. |
| [how-to-write-a-wasm-component-extension.md](./how-to-write-a-wasm-component-extension.md) | Surface an already-published WASM component as a canvas node — the two-component describe, why the node must be reachable by `oci_ref`, and the four things about it that fail late. |

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
