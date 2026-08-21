//! Guards that the SDK contract `greentic-ext-runtime` resolves exposes the
//! `dwProviders` contribution. This is a compile + parse regression guard: it
//! fails only if the pinned `greentic-extension-sdk-contract` is rolled back
//! below the version that introduced the field. No runtime logic is involved —
//! ext-runtime forwards `describe.contributions` verbatim.

use greentic_extension_sdk_contract::describe::Contributions;

#[test]
fn ext_runtime_sdk_exposes_dw_providers() {
    let c: Contributions = serde_json::from_value(serde_json::json!({
        "dwProviders": [{
            "providerId": "provider.llm.t.chat",
            "family": "llm",
            "displayName": "T",
            "version": "1.0.0",
            "channel": "research",
            "capabilityContractIds": ["cap://llm/chat"]
        }]
    }))
    .expect("Contributions with dwProviders must parse");
    assert_eq!(c.dw_providers.len(), 1);
    assert_eq!(c.dw_providers[0].provider_id, "provider.llm.t.chat");
}
