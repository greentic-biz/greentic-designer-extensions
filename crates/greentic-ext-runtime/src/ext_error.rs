//! WIT `extension-error` -> host normalization.
//!
//! Every bindgen module owns a distinct generated `ExtensionError` type;
//! these helpers map each of them onto the single host-side
//! [`crate::types::HostExtensionError`] so dispatch code never leaks
//! bindgen types and never collapses variants into anyhow strings.

use crate::types::HostExtensionError as H;

pub(crate) fn from_design_v01(
    e: crate::host_bindings::greentic::extension_base0_1_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::greentic::extension_base0_1_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::Internal(s) => H::Internal(s),
    }
}

pub(crate) fn from_deploy_v01(
    e: crate::host_bindings::deploy::greentic::extension_base0_1_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::deploy::greentic::extension_base0_1_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::Internal(s) => H::Internal(s),
    }
}

pub(crate) fn from_bundle_v01(
    e: crate::host_bindings::bundle::greentic::extension_base0_1_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::bundle::greentic::extension_base0_1_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::Internal(s) => H::Internal(s),
    }
}

pub(crate) fn from_design_v03(
    e: crate::host_bindings::design_v03::greentic::extension_base0_2_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::design_v03::greentic::extension_base0_2_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::NotFound(s) => H::NotFound(s),
        E::SchemaInvalid(s) => H::SchemaInvalid(s),
        E::Internal(s) => H::Internal(s),
    }
}

pub(crate) fn from_deploy_v02(
    e: crate::host_bindings::deploy_v02::greentic::extension_base0_2_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::deploy_v02::greentic::extension_base0_2_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::NotFound(s) => H::NotFound(s),
        E::SchemaInvalid(s) => H::SchemaInvalid(s),
        E::Internal(s) => H::Internal(s),
    }
}

pub(crate) fn from_bundle_v02(
    e: crate::host_bindings::bundle_v02::greentic::extension_base0_2_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::bundle_v02::greentic::extension_base0_2_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::NotFound(s) => H::NotFound(s),
        E::SchemaInvalid(s) => H::SchemaInvalid(s),
        E::Internal(s) => H::Internal(s),
    }
}

pub(crate) fn from_composer_v02(
    e: crate::host_bindings::dw_composer_v02::greentic::extension_base0_2_0::types::ExtensionError,
) -> H {
    use crate::host_bindings::dw_composer_v02::greentic::extension_base0_2_0::types::ExtensionError as E;
    match e {
        E::InvalidInput(s) => H::InvalidInput(s),
        E::MissingCapability(s) => H::MissingCapability(s),
        E::PermissionDenied(s) => H::PermissionDenied(s),
        E::NotFound(s) => H::NotFound(s),
        E::SchemaInvalid(s) => H::SchemaInvalid(s),
        E::Internal(s) => H::Internal(s),
    }
}

/// dw-composer@0.1.0 `compose` returns `result<string, string>` — the bare
/// error string has no category, so it lands in `internal`.
pub(crate) fn from_composer_v01(msg: String) -> H {
    H::Internal(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_v01_maps_lossless() {
        use crate::host_bindings::greentic::extension_base0_1_0::types::ExtensionError as V1;
        assert!(
            matches!(from_design_v01(V1::InvalidInput("a".into())), H::InvalidInput(s) if s == "a")
        );
        assert!(
            matches!(from_design_v01(V1::MissingCapability("b".into())), H::MissingCapability(s) if s == "b")
        );
        assert!(
            matches!(from_design_v01(V1::PermissionDenied("c".into())), H::PermissionDenied(s) if s == "c")
        );
        assert!(matches!(from_design_v01(V1::Internal("d".into())), H::Internal(s) if s == "d"));
    }

    #[test]
    fn design_v03_maps_all_six() {
        use crate::host_bindings::design_v03::greentic::extension_base0_2_0::types::ExtensionError as V2;
        assert!(
            matches!(from_design_v03(V2::InvalidInput("a".into())), H::InvalidInput(s) if s == "a")
        );
        assert!(
            matches!(from_design_v03(V2::MissingCapability("b".into())), H::MissingCapability(s) if s == "b")
        );
        assert!(
            matches!(from_design_v03(V2::PermissionDenied("c".into())), H::PermissionDenied(s) if s == "c")
        );
        assert!(matches!(from_design_v03(V2::NotFound("x".into())), H::NotFound(s) if s == "x"));
        assert!(
            matches!(from_design_v03(V2::SchemaInvalid("y".into())), H::SchemaInvalid(s) if s == "y")
        );
        assert!(matches!(from_design_v03(V2::Internal("z".into())), H::Internal(s) if s == "z"));
    }

    #[test]
    fn composer_v01_string_maps_to_internal() {
        assert!(matches!(from_composer_v01("boom".to_string()), H::Internal(s) if s == "boom"));
    }
}
