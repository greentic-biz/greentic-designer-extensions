# Greentic Designer Extensions — Concept

**Date:** 2026-04-17
**Status:** Draft for review (Maarten)
**Author:** Bimbim
**Scope:** Module 4 + 5 + 6 foundation (from 16 April 2026 meeting)

---

## Vision

Greentic Designer stops knowing anything domain-specific. Instead, it
learns at runtime from extensions that anyone can write, publish, and
share via the Greentic Store.

Three flavours of extension cover the lifecycle:

- **design-extension** teaches the designer to author new content types
  (Adaptive Cards, Digital Workers, Green Tech X / Telco X schemas, ...).
- **bundle-extension** packages the designer's output into deployable
  Application Packs, with a recipe per target scenario.
- **deploy-extension** ships the Application Pack to its target — desktop,
  AWS, GCP, on-prem Kubernetes, Cisco box — without the designer needing
  any hardcoded knowledge of that target.

A capability registry resolves dependencies and degrades gracefully when
something is missing. A host broker lets extensions compose with each
other under explicit permission gates.

---

## Problem — what we are replacing

Today the designer has **hardcoded knowledge**:

- 12 Adaptive Cards tools compiled into the binary
- Pack building via subprocess to `greentic-cards2pack`
- Bundle building via subprocess to `greentic-bundle`
- No deploy targets implemented (all stubs)
- No way for Osora, Telco X, or anyone else to add their own content types

Every new capability = a release of the designer binary. This does not scale
to "thousands of people filling a store."

---

## The three extension kinds

One unified system, three specialized roles. Each extension is a signed
WebAssembly component packaged as `.gtxpack` with a `describe.json` manifest.

| Kind | Teaches designer to... | Example extensions |
|---|---|---|
| **design-extension** | Author content (cards, flows, workers) | `greentic.adaptive-cards`, `greentic.flows-ygtc-v2`, `greentic.digital-workers`, `greentic.telco-x` |
| **bundle-extension** | Package content into an Application Pack | `greentic.hosted-webchat`, `greentic.openshift-bundle`, `greentic.multi-channel-bundle` |
| **deploy-extension** | Ship the pack to a target environment | `greentic.desktop`, `greentic.aws-eks`, `greentic.gcp-gke`, `greentic.cisco-onprem` |

A design-extension plugs into the **chat loop** — its tools become LLM tools,
its prompts teach the agent, its knowledge base seeds the few-shot memory.

A bundle-extension plugs into the **"Next" wizard step** — it offers recipes
for converting the authored flow + cards into a deployable Application Pack.

A deploy-extension plugs into the **deploy wizard step** — it offers targets,
collects credentials, and runs the deployment.

---

## How it fits together

```
   ┌─────────────────────────────────────────────────────────┐
   │              Greentic Store (store.greentic.ai)         │
   │    Developers upload — end-users discover + install     │
   └────────────────────────┬────────────────────────────────┘
                            │ HTTP API
                            ▼
   ┌─────────────────────────────────────────────────────────┐
   │               greentic-ext-runtime                      │
   │   discover · install · verify · cap-match · broker      │
   └────────────────────────┬────────────────────────────────┘
                            │
   ┌────────────────────────▼────────────────────────────────┐
   │                  Greentic Designer                      │
   │                                                         │
   │   ┌──────────┐      ┌───────────┐      ┌───────────┐    │
   │   │  Chat    │  →   │ "Next"    │  →   │  Deploy   │    │
   │   │  (LLM)   │      │  wizard   │      │  wizard   │    │
   │   └─────┬────┘      └─────┬─────┘      └─────┬─────┘    │
   │         │                 │                  │          │
   │    design-ext       bundle-ext         deploy-ext       │
   │         │                 │                  │          │
   │    tools + KB         recipes            targets        │
   └─────────┼─────────────────┼──────────────────┼──────────┘
             │                 │                  │
             ▼                 ▼                  ▼
      adaptive-cards     hosted-webchat        aws-eks
      flows-ygtc-v2      openshift-bundle      desktop
      digital-workers    multi-channel         gcp-gke
      telco-x            ...                   cisco-onprem
```

---

## What powers it — three mechanisms

### 1. `describe.json` — the manifest for everything in the Store

Every extension ships a `describe.json` declaring identity, version, the
capabilities it offers, the capabilities it requires, and permissions it
needs (network, secrets, cross-extension calls).

```json
{
  "apiVersion": "greentic.ai/v1",
  "kind": "DesignExtension",
  "metadata": {
    "id": "greentic.adaptive-cards",
    "version": "1.6.0"
  },
  "capabilities": {
    "offered":  [ { "id": "greentic:adaptive-cards/validate", "version": "1.0.0" } ],
    "required": [ { "id": "greentic:host/logging", "version": "^1.0.0" } ]
  },
  "runtime": {
    "component": "extension.wasm",
    "permissions": { "network": [], "secrets": [], "callExtensionKinds": [] }
  }
}
```

One schema, three `kind` values (DesignExtension / BundleExtension /
DeployExtension), kind-specific `contributions` section (schemas, prompts,
knowledge, tools, recipes, targets).

### 2. Capability Registry + matching engine

Extensions declare what they **offer** and what they **require**. At install
and at startup the registry resolves the graph.

- Semver-aware: required `^1.2` matches offered `1.2.5`, `1.3.0` but not `2.0.0`
- Missing requirement → extension marked **degraded**, not crashed — designer
  shows a friendly warning in the UI
- Cycle detection prevents circular dependencies
- Multiple offerings → highest compatible version wins; config can pin

This is what lets the designer say *"Oh, I have Digital Workers"* at startup.

### 3. Host broker — cross-extension collaboration

A flow-design extension that emits cards can call the adaptive-cards
extension to validate them — via a host broker that enforces permissions.

```
flows-ygtc-v2.wasm
     │
     │ call-extension("design", "greentic.adaptive-cards",
     │                 "validate-content", { card })
     ▼
runtime broker
     │ check: caller has permission to call this kind?
     │ check: target extension installed?
     │ check: cap compatible?
     ▼
adaptive-cards.wasm :: validate-content → result
```

Extensions compose without knowing about each other — the broker is the
contract. If the dependency is not installed, the caller degrades
gracefully (warning instead of crash).

---

## Greentic Store — distribution & discovery

The Store is a first-class piece of the system, not a bolt-on. It provides:

- **Upload** via `gtdx publish` (developers, automated via CI/CD)
- **Discovery** via `gtdx search` and web UI (end-users)
- **Versioning** — every version is a distinct record, semver-resolved
- **Signing** — Ed25519 developer signature, optional store countersign
- **Offline mirror** — self-hosted store for enterprise / air-gapped

The Store hosts the `.gtxpack` artifact directly (not just metadata). This
gives centralized analytics, yank + security recall, and offline
capability. An OCI escape hatch is possible later for enterprise mirrors
but is not in the v1 scope.

The Store API contract ships as an **OpenAPI 3.1 spec** in this repo. The
Store server itself lives in a separate repository and is implemented by
the Module 7 team against that contract.

---

## Reference extension — Adaptive Cards

To prove the system works and to retire hardcoded logic, we ship
`greentic.adaptive-cards@1.6.0` as the first canonical extension.

- Wraps the existing `adaptive-card-core` library (no rewrite)
- Exposes the 10 existing tools (validate, analyze, optimize, transform, ...)
- Contributes JSON Schema, prompts, knowledge base
- Bundled with the designer installer for first-run convenience

The designer drops its direct dependency on `adaptive-card-core`, becoming
a pure extension host. From that point on, adding support for a new
content type (Digital Workers, Telco X) is a matter of publishing an
extension, not releasing the designer.

---

## How this connects to current work

This concept builds on decisions from the 16 April meeting and depends on
P0 work already in flight:

| Dependency | Status | How we use it |
|---|---|---|
| Module 1 — demo cleanup | In progress | Not blocking — independent workstream |
| Module 2 — `build-answer.json` schema | In progress | Bundle-ext renders into Application Pack format defined there |
| Module 3 — Codex / GTC Wizard schema fix | In progress | Unrelated to extension contract |
| Module 8 — Application Pack spec | Needs spec | Bundle-ext output format — contract freeze needed |

We ship the extension contract + runtime + AC reference independently of
Modules 5, 6, 7 reference implementations. Those land in follow-up
cycles.

---

## Timeline — v1 scope

~9 weeks with one developer full-time, faster in parallel:

```
Week 1-2   Repo scaffolding · WIT · describe.json schema · runtime skeleton
Week 3-4   Capability registry · matching · install/update/scan · CLI
Week 5     Design sub-WIT · designer refactor (parallel path)
Week 6     AC extension crate · designer cutover · bundled fallback
Week 7     Bundle + Deploy sub-WIT contracts · Store OpenAPI spec
Week 8     Integration tests · docs (DX-04) · how-to-write guides
Week 9     Polish · release · hand-off to Osora + Telco X
```

Coordination sync-points with Osora and Telco X teams at week 2 (contract
preview) and week 5 (ready for them to scaffold their extensions).

Module 5, 6, 7 reference implementations are follow-up cycles after v1.

---

## What we ship in v1

| Deliverable | |
|---|---|
| New repo `greentic-designer-extensions` | 4 crates workspace + reference AC extension |
| `greentic-extension-sdk-contract` | Types · WIT · describe.json schema + validator |
| `greentic-ext-runtime` | wasmtime loader · registry · broker · lifecycle |
| `greentic-ext-cli` (`gtdx`) | ~20 subcommands for authoring + install |
| `greentic.adaptive-cards@1.6.0` | First extension, published to Store |
| `greentic-designer` refactor | Drops hardcoded AC, consumes runtime |
| OpenAPI spec | Contract for Module 7 Store team |
| Documentation | 10+ docs including tutorials for Osora / Telco X |

**Not in v1 scope** (follow-up cycles): bundle-ext reference impls,
deploy-ext reference impls, Greentic Store server, Digital Workers template,
Telco X template, MCP extension.

---

## Open decisions — needs sign-off

| ID | Question | Recommendation |
|---|---|---|
| **BX-01** | Design + Bundle merged into one extension, or kept separate? | **Keep separate** — matches your quote *"bundle extension, not extension to bundle"*. Shared foundation, different WIT sub-interfaces and call sites. |
| **BA-01** | Final naming: `bundle`, `build-answer`, or `pack-answer`? | Deferred — does not affect extension contract but affects docs phrasing. |
| **DX-01** | Final list of ~25 minimum Designer capabilities | Working session with you — capabilities currently drafted from AC v1.6 feature set; need vetting. |
| **Store v1** | Default = Store hosts WASM artifact, OCI escape hatch deferred to v1.1 | Matches the UX most end-users expect (one command install). Advanced enterprise option comes later. |

---

## What we need from you tonight

1. **Sign off on the three-kind model** (design / bundle / deploy as separate
   kinds over shared foundation).
2. **Sign off on the repo strategy** — new repo `greentic-designer-extensions`
   as umbrella for all three kinds.
3. **Unblock BX-01 and DX-01** or agree to timebox them into a follow-up
   session so implementation can start.
4. **Validate the v1 scope** — AC extraction + contract + runtime + CLI +
   Store OpenAPI — with bundle/deploy reference impls as follow-up cycles.

Everything else is engineering detail, captured in the full spec at
`docs/superpowers/specs/2026-04-17-designer-extension-system-design.md`.
