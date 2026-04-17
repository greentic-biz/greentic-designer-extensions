# Capability Registry

The capability registry resolves which extensions are compatible with each
other and surfaces missing dependencies as degraded states rather than hard
crashes.

---

## What a Capability Is

A capability is a stable, named contract identified by a string ID and a
semver version:

```json
{ "id": "greentic:adaptive-cards/validate", "version": "1.0.0" }
```

The ID format is `<namespace>:<path>`:

- `namespace` — a lowercase publisher prefix (e.g. `greentic`, `osora`)
- `path` — a slash-separated path within that namespace

Capabilities are the unit of dependency: extension A says "I need X" and
extension B says "I provide X". The registry connects them.

---

## Declaring Capabilities in `describe.json`

```json
"capabilities": {
  "offered": [
    { "id": "greentic:adaptive-cards/validate", "version": "1.0.0" },
    { "id": "greentic:adaptive-cards/schema",   "version": "1.6.0" }
  ],
  "required": [
    { "id": "greentic:host/logging", "version": "^1.0.0" }
  ]
}
```

An extension that offers a capability tells other extensions "if you need
this, I can provide it." An extension that requires a capability declares a
dependency that must be satisfied for it to function fully.

The same values must be returned by the WASM component's
`greentic:extension-base/manifest` interface (`get-offered` and
`get-required`). The runtime cross-checks the two at install time.

---

## Matching Rules

The registry uses semver range matching for requirements:

1. A required range `^1.0.0` matches any offered `1.x.y` where
   `1.x.y >= 1.0.0`.
2. A required range `>=1.2.0` matches any offered version `>= 1.2.0` with
   no upper bound.
3. A required exact version `1.6.0` matches only exactly `1.6.0`.
4. When multiple extensions offer the same capability ID, the highest
   semver-compatible version wins.
5. If a pin is configured (e.g. via `gtdx` registry config), the pinned
   version takes precedence over the highest-wins rule.
6. Two extensions offering identical `(id, version)` pairs conflict —
   the second install is rejected with an explicit conflict error.

---

## Degraded State

When a required capability cannot be satisfied (the extension that would
provide it is not installed or is incompatible), the dependent extension
enters **degraded** state rather than failing to load.

In degraded state:

- The extension is loaded and partially functional.
- The specific feature that needs the missing capability returns
  `Err(MissingCapability("greentic:x/y version ^1.0"))`.
- The designer UI surfaces a warning banner listing degraded extensions
  and what they are missing.
- Log entries at `warn` level record the missing dependency.
- The extension can still serve all functionality that does not depend on
  the missing capability.

An extension should check for missing capabilities at the call site:

```rust
fn invoke_tool(name: String, args_json: String)
    -> Result<String, ExtensionError>
{
    if name == "cross_validate" {
        // Try broker; degrade if unavailable.
        match broker::call_extension("design", "greentic.schemas", "validate", &args_json) {
            Ok(result) => return Ok(result),
            Err(_) => {
                // Degrade: return partial result with warning.
                return Ok(r#"{"valid":null,"warning":"schema extension not installed"}"#.into());
            }
        }
    }
    ...
}
```

---

## Cycle Detection

The registry performs cycle detection on install. An install that would
create a circular dependency chain is rejected immediately:

- Extension A requires capability `X` provided by B.
- Extension B requires capability `Y` provided by A.
- Installing either A or B when the other is present triggers the cycle
  detector, which rejects the install with an error listing the cycle path.

Cycles are detected at install time, not at startup, so the installed set
is always cycle-free.

---

## Host Capabilities

The host (`greentic-ext-runtime`) provides a set of built-in capabilities
that are always available. Extensions may declare these in `required` to
signal the minimum host version they need.

| ID | Description |
|----|-------------|
| `greentic:host/logging` | Structured log output |
| `greentic:host/i18n` | Translation lookup |
| `greentic:host/secrets` | Secret URI resolution |
| `greentic:host/broker` | Cross-extension call dispatch |
| `greentic:host/http` | Outbound HTTPS |

Host capabilities follow the same semver matching rules. An extension that
requires `greentic:host/logging@^1.0.0` will not load on a runtime that only
provides `greentic:host/logging@0.9.0`.

---

## Worked Example

Three extensions installed:

| Extension | Offers | Requires |
|-----------|--------|----------|
| A — `greentic.adaptive-cards` | `greentic:ac/validate@1.2.0` | — |
| B — `osora.flow-designer` | — | `greentic:ac/validate@^1.0.0` |
| C — `myco.card-pro` | — | `greentic:ac/validate@^2.0.0` |

Resolution:

1. A offers `greentic:ac/validate@1.2.0`.
2. B requires `^1.0.0` — `1.2.0` satisfies `^1.0.0` (same major, higher
   patch). **B resolves successfully.**
3. C requires `^2.0.0` — `1.2.0` does not satisfy `^2.0.0` (different
   major). **C enters degraded state.** The designer shows:

   ```
   ⚠️  myco.card-pro is degraded
       Missing: greentic:ac/validate version ^2.0.0
       Installed: greentic:ac/validate@1.2.0
   ```

C remains loaded and can still serve any tool or validation that does not
require the `^2.0.0` feature. Only the specific call paths that depend on
it return `MissingCapability` errors.

To resolve C's degraded state, install an extension that provides
`greentic:ac/validate@2.0.0` or higher (and satisfies `^2.0.0`).
