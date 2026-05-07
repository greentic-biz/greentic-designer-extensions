//! `roles` interface dispatch for design extensions.
//!
//! Mirrors the export-walking pattern in [`crate::runtime`] (see
//! `render_bundle` / `validate_content`) but lives in its own module to
//! keep `runtime.rs` under the workspace 500-line cap.

use crate::error::RuntimeError;
use crate::loaded::ExtensionId;
use crate::runtime::ExtensionRuntime;
use crate::types::{
    CompileContext, Diagnostic, HostExtensionError, RoleError, RoleSpec, Severity, TargetKind,
};

const IFACE_NAME: &str = "greentic:extension-design/roles@0.2.0";

impl ExtensionRuntime {
    /// List all roles exposed by a loaded design extension.
    ///
    /// Calls `greentic:extension-design/roles@0.2.0::list-roles`.
    /// Returns an empty vec when the extension does not export the
    /// `roles` interface (older 0.1.0 extensions, for example) so
    /// callers can treat it as "no roles published" without reaching
    /// for `RuntimeError::Wasmtime`.
    pub fn list_roles(&self, ext_id: &str) -> Result<Vec<RoleSpec>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::roles::RoleSpec as WitRoleSpec;

        let loaded = self
            .loaded()
            .get(&ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(self.engine())
            .map_err(RuntimeError::Wasmtime)?;

        let Some(iface_idx) = instance.get_export_index(&mut store, None, IFACE_NAME) else {
            return Ok(Vec::new());
        };
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "list-roles")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{IFACE_NAME}' does not export 'list-roles'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(), (Vec<WitRoleSpec>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (roles,) = func
            .call(&mut store, ())
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(roles.into_iter().map(wit_role_spec_to_host).collect())
    }

    /// Run the cheap-path validator for a role's DSL entry.
    ///
    /// Calls `greentic:extension-design/roles@0.2.0::validate-role`.
    /// Returns the diagnostic list verbatim (empty = valid, mirrors the
    /// WIT contract). `RuntimeError` is reserved for host failures —
    /// missing extension, missing interface, wasmtime trap.
    pub fn validate_role(
        &self,
        ext_id: &str,
        name: &str,
        entry_json: &str,
    ) -> Result<Vec<Diagnostic>, RuntimeError> {
        use crate::host_bindings::exports::greentic::extension_design::roles::Diagnostic as WitDiagnostic;

        let loaded = self
            .loaded()
            .get(&ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;

        let (mut store, instance) = loaded
            .build_store_and_instance(self.engine())
            .map_err(RuntimeError::Wasmtime)?;

        let iface_idx = instance
            .get_export_index(&mut store, None, IFACE_NAME)
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "extension does not export interface '{IFACE_NAME}'"
                ))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "validate-role")
            .ok_or_else(|| {
                RuntimeError::Wasmtime(anyhow::anyhow!(
                    "interface '{IFACE_NAME}' does not export 'validate-role'"
                ))
            })?;

        let func = instance
            .get_typed_func::<(String, String), (Vec<WitDiagnostic>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        let (diags,) = func
            .call(&mut store, (name.to_string(), entry_json.to_string()))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;

        Ok(diags.into_iter().map(wit_diagnostic_to_host).collect())
    }

    /// Compile a single DSL entry to its target representation.
    ///
    /// Calls `greentic:extension-design/roles@0.2.0::compile-role`. A
    /// missing extension surfaces as `RoleError::UnknownRole(ext_id)`
    /// (the registry can't tell "no extension" apart from "no role"
    /// from the LLM's perspective and both should retry with a hint).
    /// Host failures (wasmtime trap, missing interface) surface as
    /// `RoleError::Host(HostExtensionError::Internal(_))` so the caller
    /// can match exhaustively without juggling two error types.
    pub fn compile_role(
        &self,
        ext_id: &str,
        name: &str,
        target: TargetKind,
        entry_json: &str,
        ctx: Option<&CompileContext>,
    ) -> Result<String, RoleError> {
        use crate::host_bindings::exports::greentic::extension_design::roles::{
            CompileContext as WitCompileContext, RoleError as WitRoleError,
            TargetKind as WitTargetKind,
        };

        let loaded = self
            .loaded()
            .get(&ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RoleError::UnknownRole(ext_id.to_string()))?;

        let (mut store, instance) =
            loaded
                .build_store_and_instance(self.engine())
                .map_err(|e| {
                    RoleError::Host(HostExtensionError::Internal(format!(
                        "instantiate '{ext_id}': {e}"
                    )))
                })?;

        let iface_idx = instance
            .get_export_index(&mut store, None, IFACE_NAME)
            .ok_or_else(|| {
                RoleError::Host(HostExtensionError::Internal(format!(
                    "extension '{ext_id}' does not export '{IFACE_NAME}'"
                )))
            })?;
        let func_idx = instance
            .get_export_index(&mut store, Some(&iface_idx), "compile-role")
            .ok_or_else(|| {
                RoleError::Host(HostExtensionError::Internal(format!(
                    "interface '{IFACE_NAME}' does not export 'compile-role'"
                )))
            })?;

        let func = instance
            .get_typed_func::<
                (
                    String,
                    WitTargetKind,
                    String,
                    Option<WitCompileContext>,
                ),
                (Result<String, WitRoleError>,),
            >(&mut store, &func_idx)
            .map_err(|e| RoleError::Host(HostExtensionError::Internal(e.to_string())))?;

        let wit_ctx = ctx.cloned().map(|c| WitCompileContext {
            flow_entries_json: c.flow_entries_json,
            flow_id: c.flow_id,
            locale: c.locale,
        });

        let (result,) = func
            .call(
                &mut store,
                (
                    name.to_string(),
                    target_to_wit(target),
                    entry_json.to_string(),
                    wit_ctx,
                ),
            )
            .map_err(|e| RoleError::Host(HostExtensionError::Internal(e.to_string())))?;

        result.map_err(wit_role_error_to_host)
    }
}

fn wit_role_spec_to_host(
    s: crate::host_bindings::exports::greentic::extension_design::roles::RoleSpec,
) -> RoleSpec {
    RoleSpec {
        name: s.name,
        description: s.description,
        json_schema: s.json_schema,
        target: target_from_wit(s.target),
        schema_version: s.schema_version,
        context_aware: s.context_aware,
    }
}

fn wit_diagnostic_to_host(
    d: crate::host_bindings::exports::greentic::extension_design::roles::Diagnostic,
) -> Diagnostic {
    use crate::host_bindings::greentic::extension_base::types::Severity as WitSeverity;
    Diagnostic {
        severity: match d.severity {
            WitSeverity::Error => Severity::Error,
            WitSeverity::Warning => Severity::Warning,
            WitSeverity::Info => Severity::Info,
            WitSeverity::Hint => Severity::Hint,
        },
        code: d.code,
        message: d.message,
        path: d.path,
    }
}

fn wit_role_error_to_host(
    e: crate::host_bindings::exports::greentic::extension_design::roles::RoleError,
) -> RoleError {
    use crate::host_bindings::exports::greentic::extension_design::roles::RoleError as WitRoleError;
    match e {
        WitRoleError::UnknownRole(s) => RoleError::UnknownRole(s),
        WitRoleError::InvalidInput(diags) => {
            RoleError::InvalidInput(diags.into_iter().map(wit_diagnostic_to_host).collect())
        }
        WitRoleError::CompileFailed(s) => RoleError::CompileFailed(s),
        WitRoleError::TargetNotSupported(t) => RoleError::TargetNotSupported(target_from_wit(t)),
        WitRoleError::VersionNotSupported(v) => RoleError::VersionNotSupported(v),
        WitRoleError::Host(ee) => RoleError::Host(wit_extension_error_to_host(ee)),
    }
}

fn wit_extension_error_to_host(
    e: crate::host_bindings::exports::greentic::extension_design::roles::ExtensionError,
) -> HostExtensionError {
    use crate::host_bindings::exports::greentic::extension_design::roles::ExtensionError as WitErr;
    match e {
        WitErr::InvalidInput(s) => HostExtensionError::InvalidInput(s),
        WitErr::MissingCapability(s) => HostExtensionError::MissingCapability(s),
        WitErr::PermissionDenied(s) => HostExtensionError::PermissionDenied(s),
        WitErr::Internal(s) => HostExtensionError::Internal(s),
    }
}

fn target_to_wit(
    t: TargetKind,
) -> crate::host_bindings::exports::greentic::extension_design::roles::TargetKind {
    use crate::host_bindings::exports::greentic::extension_design::roles::TargetKind as Wit;
    match t {
        TargetKind::AdaptiveCard => Wit::AdaptiveCard,
        TargetKind::SlackBlockKit => Wit::SlackBlockKit,
        TargetKind::TeamsCard => Wit::TeamsCard,
        TargetKind::PlainText => Wit::PlainText,
    }
}

fn target_from_wit(
    t: crate::host_bindings::exports::greentic::extension_design::roles::TargetKind,
) -> TargetKind {
    use crate::host_bindings::exports::greentic::extension_design::roles::TargetKind as Wit;
    match t {
        Wit::AdaptiveCard => TargetKind::AdaptiveCard,
        Wit::SlackBlockKit => TargetKind::SlackBlockKit,
        Wit::TeamsCard => TargetKind::TeamsCard,
        Wit::PlainText => TargetKind::PlainText,
    }
}
