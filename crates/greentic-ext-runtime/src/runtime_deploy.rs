//! `deployment` interface dispatch for deploy extensions (Mode B).
//!
//! Mirrors the export-walking pattern of the `targets` callers in
//! [`crate::runtime`] but lives in its own module to keep `runtime.rs`
//! under the workspace 500-line cap. First consumer: the designer's
//! wizard deploy step driving `greentic.deploy-github`.

use crate::error::RuntimeError;
use crate::loaded::ExtensionId;
use crate::runtime::ExtensionRuntime;
use crate::types::{DeployExtensionError, DeployJob, DeployRequest, DeployStatus};

use crate::host_bindings::deploy::exports::greentic::extension_deploy0_1_0::deployment::{
    DeployJob as WitDeployJob, DeployRequest as WitDeployRequest, DeployStatus as WitDeployStatus,
};
use crate::host_bindings::deploy::greentic::extension_base0_1_0::types::ExtensionError as WitExtensionError;
use crate::host_bindings::deploy_v02::exports::greentic::extension_deploy0_2_0::deployment::{
    DeployJob as WitDeployJobV2, DeployRequest as WitDeployRequestV2,
    DeployStatus as WitDeployStatusV2,
};
use crate::host_bindings::deploy_v02::greentic::extension_base0_2_0::types::ExtensionError as WitExtensionErrorV2;

const BASE_IFACE: &str = "greentic:extension-deploy/deployment";

impl ExtensionRuntime {
    /// Start a deployment inside the extension. Returns the initial job.
    ///
    /// Calls `greentic:extension-deploy/deployment::deploy`, resolving the
    /// interface newest-first across `@0.2.0`/`@0.1.0`. The matched version
    /// selects the `extension-error` ABI (6-variant base at `@0.2.0`,
    /// 4-variant base at `@0.1.0`). The call is synchronous and may take up
    /// to ~2 minutes for network-bound extensions (artifact upload); run it
    /// on a blocking thread.
    pub fn deploy(&self, ext_id: &str, req: DeployRequest) -> Result<DeployJob, RuntimeError> {
        let (mut store, instance) = self.deploy_instance(ext_id)?;
        let (iface_idx, version) = resolve_deployment_iface(&mut store, &instance)?;
        let func_idx = resolve_func(&mut store, &instance, &iface_idx, "deploy")?;
        if version == "0.2.0" {
            let func = instance
                .get_typed_func::<(WitDeployRequestV2,), (Result<WitDeployJobV2, WitExtensionErrorV2>,)>(
                    &mut store, &func_idx,
                )
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let wit_req = WitDeployRequestV2 {
                target_id: req.target_id,
                artifact_bytes: req.artifact_bytes,
                credentials_json: req.credentials_json,
                config_json: req.config_json,
                deployment_name: req.deployment_name,
            };
            let (result,) = func
                .call(&mut store, (wit_req,))
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            result
                .map(job_to_host_v2)
                .map_err(|e| RuntimeError::Deploy(err_to_host_v2(e)))
        } else {
            let func = instance
                .get_typed_func::<(WitDeployRequest,), (Result<WitDeployJob, WitExtensionError>,)>(
                    &mut store, &func_idx,
                )
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let wit_req = WitDeployRequest {
                target_id: req.target_id,
                artifact_bytes: req.artifact_bytes,
                credentials_json: req.credentials_json,
                config_json: req.config_json,
                deployment_name: req.deployment_name,
            };
            let (result,) = func
                .call(&mut store, (wit_req,))
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            result
                .map(job_to_host)
                .map_err(|e| RuntimeError::Deploy(err_to_host(e)))
        }
    }

    /// Poll a previously started deployment job.
    pub fn deploy_poll(&self, ext_id: &str, job_id: &str) -> Result<DeployJob, RuntimeError> {
        let (mut store, instance) = self.deploy_instance(ext_id)?;
        let (iface_idx, version) = resolve_deployment_iface(&mut store, &instance)?;
        let func_idx = resolve_func(&mut store, &instance, &iface_idx, "poll")?;
        if version == "0.2.0" {
            let func = instance
                .get_typed_func::<(String,), (Result<WitDeployJobV2, WitExtensionErrorV2>,)>(
                    &mut store, &func_idx,
                )
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (result,) = func
                .call(&mut store, (job_id.to_string(),))
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            result
                .map(job_to_host_v2)
                .map_err(|e| RuntimeError::Deploy(err_to_host_v2(e)))
        } else {
            let func = instance
                .get_typed_func::<(String,), (Result<WitDeployJob, WitExtensionError>,)>(
                    &mut store, &func_idx,
                )
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (result,) = func
                .call(&mut store, (job_id.to_string(),))
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            result
                .map(job_to_host)
                .map_err(|e| RuntimeError::Deploy(err_to_host(e)))
        }
    }

    /// Roll back a previously started deployment job.
    pub fn deploy_rollback(&self, ext_id: &str, job_id: &str) -> Result<(), RuntimeError> {
        let (mut store, instance) = self.deploy_instance(ext_id)?;
        let (iface_idx, version) = resolve_deployment_iface(&mut store, &instance)?;
        let func_idx = resolve_func(&mut store, &instance, &iface_idx, "rollback")?;
        if version == "0.2.0" {
            let func = instance
                .get_typed_func::<(String,), (Result<(), WitExtensionErrorV2>,)>(
                    &mut store, &func_idx,
                )
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (result,) = func
                .call(&mut store, (job_id.to_string(),))
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            result.map_err(|e| RuntimeError::Deploy(err_to_host_v2(e)))
        } else {
            let func = instance
                .get_typed_func::<(String,), (Result<(), WitExtensionError>,)>(&mut store, &func_idx)
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            let (result,) = func
                .call(&mut store, (job_id.to_string(),))
                .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
            result.map_err(|e| RuntimeError::Deploy(err_to_host(e)))
        }
    }

    /// Resolve a loaded extension into a fresh store + instance.
    ///
    /// Deliberate departure from the sibling dispatch modules (which inline
    /// these lines per method): three callers here made the duplication
    /// worth factoring, at the cost of naming wasmtime types in a private
    /// signature.
    fn deploy_instance(
        &self,
        ext_id: &str,
    ) -> Result<
        (
            wasmtime::Store<crate::host_state::HostState>,
            wasmtime::component::Instance,
        ),
        RuntimeError,
    > {
        let loaded = self
            .loaded()
            .get(&ExtensionId(ext_id.to_string()))
            .cloned()
            .ok_or_else(|| RuntimeError::NotFound(ext_id.to_string()))?;
        loaded
            .build_store_and_instance(
                self.engine(),
                self.host_overrides().clone(),
                &crate::host_ports::HostCallContext::default(),
            )
            .map_err(RuntimeError::Wasmtime)
    }
}

/// Resolve the `deployment` interface newest-first across the deploy version
/// table, returning the export index plus the matched bare version string so
/// callers can pick the correct typed signature.
fn resolve_deployment_iface(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
    instance: &wasmtime::component::Instance,
) -> Result<(wasmtime::component::ComponentExportIndex, &'static str), RuntimeError> {
    let (iface_idx, _name, version) = crate::runtime::resolve_iface_versions(
        store,
        instance,
        BASE_IFACE,
        crate::runtime::DEPLOY_VERSIONS,
    )?;
    Ok((iface_idx, version))
}

fn resolve_func(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
    instance: &wasmtime::component::Instance,
    iface_idx: &wasmtime::component::ComponentExportIndex,
    name: &str,
) -> Result<wasmtime::component::ComponentExportIndex, RuntimeError> {
    instance
        .get_export_index(&mut *store, Some(iface_idx), name)
        .ok_or_else(|| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "interface '{BASE_IFACE}' does not export '{name}'"
            ))
        })
}

fn job_to_host(j: WitDeployJob) -> DeployJob {
    DeployJob {
        id: j.id,
        status: match j.status {
            WitDeployStatus::Pending => DeployStatus::Pending,
            WitDeployStatus::Provisioning => DeployStatus::Provisioning,
            WitDeployStatus::Configuring => DeployStatus::Configuring,
            WitDeployStatus::Starting => DeployStatus::Starting,
            WitDeployStatus::Running => DeployStatus::Running,
            WitDeployStatus::Failed => DeployStatus::Failed,
            WitDeployStatus::RolledBack => DeployStatus::RolledBack,
        },
        message: j.message,
        endpoints: j.endpoints,
    }
}

fn err_to_host(e: WitExtensionError) -> DeployExtensionError {
    match e {
        WitExtensionError::InvalidInput(m) => DeployExtensionError::InvalidInput(m),
        WitExtensionError::MissingCapability(m) => DeployExtensionError::MissingCapability(m),
        WitExtensionError::PermissionDenied(m) => DeployExtensionError::PermissionDenied(m),
        WitExtensionError::Internal(m) => DeployExtensionError::Internal(m),
    }
}

fn job_to_host_v2(j: WitDeployJobV2) -> DeployJob {
    DeployJob {
        id: j.id,
        status: match j.status {
            WitDeployStatusV2::Pending => DeployStatus::Pending,
            WitDeployStatusV2::Provisioning => DeployStatus::Provisioning,
            WitDeployStatusV2::Configuring => DeployStatus::Configuring,
            WitDeployStatusV2::Starting => DeployStatus::Starting,
            WitDeployStatusV2::Running => DeployStatus::Running,
            WitDeployStatusV2::Failed => DeployStatus::Failed,
            WitDeployStatusV2::RolledBack => DeployStatus::RolledBack,
        },
        message: j.message,
        endpoints: j.endpoints,
    }
}

fn err_to_host_v2(e: WitExtensionErrorV2) -> DeployExtensionError {
    match e {
        WitExtensionErrorV2::InvalidInput(m) => DeployExtensionError::InvalidInput(m),
        WitExtensionErrorV2::MissingCapability(m) => DeployExtensionError::MissingCapability(m),
        WitExtensionErrorV2::PermissionDenied(m) => DeployExtensionError::PermissionDenied(m),
        WitExtensionErrorV2::NotFound(m) => DeployExtensionError::NotFound(m),
        WitExtensionErrorV2::SchemaInvalid(m) => DeployExtensionError::SchemaInvalid(m),
        WitExtensionErrorV2::Internal(m) => DeployExtensionError::Internal(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_returns_not_found_for_unknown_extension() {
        let rt = ExtensionRuntime::for_test();
        let req = DeployRequest {
            target_id: "github-repo".into(),
            artifact_bytes: vec![1, 2, 3],
            credentials_json: "{}".into(),
            config_json: "{}".into(),
            deployment_name: "demo".into(),
        };
        match rt.deploy("greentic.deploy-github", req) {
            Err(RuntimeError::NotFound(id)) => assert_eq!(id, "greentic.deploy-github"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn deploy_poll_returns_not_found_for_unknown_extension() {
        let rt = ExtensionRuntime::for_test();
        match rt.deploy_poll("greentic.deploy-github", "job-1") {
            Err(RuntimeError::NotFound(id)) => assert_eq!(id, "greentic.deploy-github"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn deploy_rollback_returns_not_found_for_unknown_extension() {
        let rt = ExtensionRuntime::for_test();
        match rt.deploy_rollback("greentic.deploy-github", "job-1") {
            Err(RuntimeError::NotFound(id)) => assert_eq!(id, "greentic.deploy-github"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
