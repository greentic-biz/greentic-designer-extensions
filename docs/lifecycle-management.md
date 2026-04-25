# Extension Lifecycle Management

Greentic extensions persist enable/disable state in
`~/.greentic/extensions-state.json` (schema 1.0). Default behavior: any
extension absent from the file is treated as **enabled**. First boot after
upgrade requires no migration.

## CLI

```bash
gtdx enable  <id>[@<version>]
gtdx disable <id>[@<version>]
gtdx list --status
```

`<id>@<version>` is required only when multiple versions of the same
extension are installed; otherwise the version is inferred from the
filesystem.

`gtdx disable` warns (does **not** block) when other installed extensions
declare a `capabilities.required` entry that matches a
`capabilities.offered` entry from the target. Cascade resolution is
intentionally out of scope for the MVP.

## State file format

```json
{
  "schema": "1.0",
  "default": {
    "enabled": {
      "greentic.llm-openai@0.1.0": true,
      "greentic.adaptive-cards@1.6.0": false
    }
  },
  "tenants": {}
}
```

The `tenants` map is reserved for the future designer-admin track and is
ignored by current readers. Extensions absent from `default.enabled`
default to enabled.

State file writes are atomic (`tmp + fsync + rename`) and gated by an
advisory `.lock` file with bounded retries; readers always see either the
old snapshot or the new, never a partial write.

## Hot reload

The designer (or any consumer of `greentic-ext-runtime`) subscribes to
`RuntimeEvent::StateFileChanged` via `ExtensionRuntime::subscribe()`. The
runtime's watcher emits this event when `~/.greentic/extensions-state.json`
is created or modified. Toggling extension state takes effect within
~1 second without restarting the consumer.

## Programmatic API

```rust
use greentic_ext_state::ExtensionState;

let mut state = ExtensionState::load(&home)?;
state.set_enabled("greentic.llm-openai", "0.1.0", false);
state.save_atomic(&home)?;
```

The library lives at `crates/greentic-ext-state/`.
