# Cross-Extension Communication

Extensions can call each other through the host broker rather than coupling
directly. This page explains the broker API, permission gating, and
graceful degradation patterns.

---

## Why Composition

Two extensions that share a common need should not duplicate logic. A
flow-design extension that emits Adaptive Cards should validate them using
the AC extension's schema — without bundling the AC validation code itself.

The host broker makes this possible: one extension calls another by name,
and the host routes the call, enforcing permissions and depth limits.

---

## The Broker API

The broker is imported from `greentic:extension-host/broker`:

```wit
interface broker {
  call-extension: func(
    kind:      string,     // "design" | "bundle" | "deploy"
    target-id: string,     // e.g. "greentic.adaptive-cards"
    function:  string,     // e.g. "validate-content"
    args-json: string,     // JSON-encoded arguments
  ) -> result<string, string>;
}
```

**Parameters:**

| Name | Description |
|------|-------------|
| `kind` | Kind of the target extension. One of `"design"`, `"bundle"`, `"deploy"`. |
| `target-id` | The `metadata.id` of the target extension, e.g. `"greentic.adaptive-cards"`. |
| `function` | The function to invoke on the target. Maps to a WIT interface function. |
| `args-json` | JSON-encoded arguments for the function. |

**Return value:** `Ok(result_json)` on success, `Err(message)` on any
failure (permission denied, target not installed, depth exceeded, target
returned an error).

---

## Permission Gating

Before using the broker, declare permission in `describe.json`:

```json
"runtime": {
  "permissions": {
    "callExtensionKinds": ["design"]
  }
}
```

Without this declaration, any `call-extension` call returns
`Err("permission denied: caller not allowed to call design extensions")`.

The allowlist is a list of kinds. An extension that needs to call both
design and bundle extensions must declare both:

```json
"callExtensionKinds": ["design", "bundle"]
```

The broker does not restrict which specific extension IDs can be called —
only the kind category.

---

## Max-Depth Protection

The host enforces a maximum call depth to prevent runaway recursion. The
default limit is **8**. A call that would exceed this depth returns:

```
Err("call depth limit exceeded (max 8)")
```

Extensions must not rely on unbounded recursion. Design for a bounded call
graph: A calls B, B calls C, done.

---

## Graceful Degradation

A call through the broker will fail if the target extension is not installed
or is degraded. Always handle the error case without crashing:

```rust
fn invoke_tool(name: String, args_json: String)
    -> Result<String, types::ExtensionError>
{
    if name == "validate_with_schema" {
        match broker::call_extension(
            "design",
            "greentic.adaptive-cards",
            "validate-content",
            &args_json,
        ) {
            Ok(result_json) => return Ok(result_json),
            Err(e) => {
                // Degrade gracefully: return a partial result with a warning.
                logging::log(
                    logging::Level::Warn,
                    "myext::tools",
                    &format!("AC extension unavailable, skipping schema validation: {e}"),
                );
                return Ok(serde_json::json!({
                    "valid": null,
                    "warning": "adaptive-cards extension not installed; schema validation skipped",
                }).to_string());
            }
        }
    }
    Err(types::ExtensionError::InvalidInput(format!("unknown tool: {name}")))
}
```

The designer UI surfaces degraded state to the user as a warning — not as
a crash — so users understand why a feature is partial rather than seeing
an opaque error.

---

## Worked Example — Flow Designer Calling AC Validation

Extension: `osora.flow-designer` (a design extension for authoring flows)
Target: `greentic.adaptive-cards` (validates card embedded in a flow node)

1. `describe.json` for `osora.flow-designer`:
   ```json
   "runtime": {
     "permissions": {
       "callExtensionKinds": ["design"]
     }
   }
   ```

2. In `invoke_tool` when handling a flow that contains an Adaptive Card node:

   ```rust
   // Prepare the validation request for the AC extension.
   let ac_args = serde_json::json!({
       "content_type": "adaptive-card",
       "content_json": serde_json::to_string(&card_node).unwrap(),
   }).to_string();

   let validation_result = broker::call_extension(
       "design",
       "greentic.adaptive-cards",
       "validate-content",
       &ac_args,
   );

   match validation_result {
       Ok(json) => {
           // Parse the ValidateResult JSON and merge diagnostics
           // into the flow validation response.
           let vr: serde_json::Value = serde_json::from_str(&json)?;
           // ...
       }
       Err(_) => {
           // AC extension not installed — skip card validation.
           // Flow validation continues without it.
       }
   }
   ```

3. If `greentic.adaptive-cards` is installed, the broker dispatches the call:
   ```
   osora.flow-designer.wasm
        └─ broker::call-extension("design", "greentic.adaptive-cards",
                                  "validate-content", {...})
             └─ host checks permissions: OK
             └─ host resolves "greentic.adaptive-cards": found
             └─ host checks depth: 1 < 8: OK
             └─ greentic.adaptive-cards.wasm :: validate-content(...)
                  └─ returns ValidateResult JSON
             └─ host returns Ok(json) to caller
   ```

4. If `greentic.adaptive-cards` is not installed, the broker returns
   `Err("extension not found: greentic.adaptive-cards")` and the flow
   designer degrades gracefully.
