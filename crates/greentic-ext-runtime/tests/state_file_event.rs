//! Verifies the `StateFileChanged` event variant exists and round-trips.

use greentic_ext_runtime::RuntimeEvent;

#[test]
fn runtime_event_has_state_file_changed_variant() {
    let event = RuntimeEvent::StateFileChanged;
    // Smoke test: enum variant exists, is Debug + Clone.
    let _cloned = event.clone();
    let _ = format!("{event:?}");
}
