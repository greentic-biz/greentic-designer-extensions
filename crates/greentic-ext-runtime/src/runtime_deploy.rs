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

use crate::host_bindings::deploy::exports::greentic::extension_deploy::deployment::{
    DeployJob as WitDeployJob, DeployRequest as WitDeployRequest, DeployStatus as WitDeployStatus,
};
use crate::host_bindings::deploy::greentic::extension_base::types::ExtensionError as WitExtensionError;

const IFACE_NAME: &str = "greentic:extension-deploy/deployment@0.1.0";

impl ExtensionRuntime {
    /// Start a deployment inside the extension. Returns the initial job.
    ///
    /// Calls `greentic:extension-deploy/deployment@0.1.0::deploy`. The
    /// call is synchronous and may take up to ~2 minutes for network-bound
    /// extensions (artifact upload); run it on a blocking thread.
    pub fn deploy(&self, ext_id: &str, req: DeployRequest) -> Result<DeployJob, RuntimeError> {
        let (mut store, instance) = self.deploy_instance(ext_id)?;
        let func_idx = resolve_func(&mut store, &instance, "deploy")?;
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

    /// Poll a previously started deployment job.
    pub fn deploy_poll(&self, ext_id: &str, job_id: &str) -> Result<DeployJob, RuntimeError> {
        let (mut store, instance) = self.deploy_instance(ext_id)?;
        let func_idx = resolve_func(&mut store, &instance, "poll")?;
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

    /// Roll back a previously started deployment job.
    pub fn deploy_rollback(&self, ext_id: &str, job_id: &str) -> Result<(), RuntimeError> {
        let (mut store, instance) = self.deploy_instance(ext_id)?;
        let func_idx = resolve_func(&mut store, &instance, "rollback")?;
        let func = instance
            .get_typed_func::<(String,), (Result<(), WitExtensionError>,)>(&mut store, &func_idx)
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
        let (result,) = func
            .call(&mut store, (job_id.to_string(),))
            .map_err(|e| RuntimeError::Wasmtime(e.into()))?;
        result.map_err(|e| RuntimeError::Deploy(err_to_host(e)))
    }

    /// Resolve a loaded extension into a fresh store + instance.
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
            .build_store_and_instance(self.engine(), self.host_overrides().clone())
            .map_err(RuntimeError::Wasmtime)
    }
}

fn resolve_func(
    store: &mut wasmtime::Store<crate::host_state::HostState>,
    instance: &wasmtime::component::Instance,
    name: &str,
) -> Result<wasmtime::component::ComponentExportIndex, RuntimeError> {
    let iface_idx = instance
        .get_export_index(&mut *store, None, IFACE_NAME)
        .ok_or_else(|| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "extension does not export interface '{IFACE_NAME}'"
            ))
        })?;
    instance
        .get_export_index(&mut *store, Some(&iface_idx), name)
        .ok_or_else(|| {
            RuntimeError::Wasmtime(anyhow::anyhow!(
                "interface '{IFACE_NAME}' does not export '{name}'"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::DiscoveryPaths;
    use crate::runtime::RuntimeConfig;

    fn empty_runtime() -> (tempfile::TempDir, ExtensionRuntime) {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = RuntimeConfig::from_paths(DiscoveryPaths::new(tmp.path().to_path_buf()));
        (tmp, ExtensionRuntime::new(config).unwrap())
    }

    #[test]
    fn deploy_returns_not_found_for_unknown_extension() {
        let (_tmp, rt) = empty_runtime();
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
        let (_tmp, rt) = empty_runtime();
        match rt.deploy_poll("greentic.deploy-github", "job-1") {
            Err(RuntimeError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn deploy_rollback_returns_not_found_for_unknown_extension() {
        let (_tmp, rt) = empty_runtime();
        match rt.deploy_rollback("greentic.deploy-github", "job-1") {
            Err(RuntimeError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
