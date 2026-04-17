use std::io::Cursor;

use crate::error::RegistryError;
use crate::registry::ExtensionRegistry;
use crate::storage::Storage;
use crate::types::ExtensionArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustPolicy {
    Strict,
    Normal,
    Loose,
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub trust_policy: TrustPolicy,
    pub accept_permissions: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            trust_policy: TrustPolicy::Normal,
            accept_permissions: false,
        }
    }
}

pub struct Installer<'a, R: ExtensionRegistry + ?Sized> {
    storage: Storage,
    registry: &'a R,
}

impl<'a, R: ExtensionRegistry + ?Sized> Installer<'a, R> {
    pub fn new(storage: Storage, registry: &'a R) -> Self {
        Self { storage, registry }
    }

    pub async fn install(
        &self,
        name: &str,
        version: &str,
        opts: InstallOptions,
    ) -> Result<(), RegistryError> {
        let artifact = self.registry.fetch(name, version).await?;
        Self::verify_signature(&artifact, opts.trust_policy)?;
        self.install_artifact(&artifact, opts)
    }

    pub fn install_artifact(
        &self,
        artifact: &ExtensionArtifact,
        _opts: InstallOptions,
    ) -> Result<(), RegistryError> {
        let kind = artifact.describe.kind;
        let (staging, final_dir) =
            self.storage
                .begin_install(kind, &artifact.name, &artifact.version)?;

        let result = Self::extract_to_staging(artifact, &staging);
        if result.is_err() {
            self.storage.abort_install(&staging);
            result?;
        }
        self.storage.commit_install(&staging, &final_dir)?;
        tracing::info!(
            name = %artifact.name,
            version = %artifact.version,
            kind = ?kind,
            "extension installed"
        );
        Ok(())
    }

    fn extract_to_staging(
        artifact: &ExtensionArtifact,
        staging: &std::path::Path,
    ) -> Result<(), RegistryError> {
        let cursor = Cursor::new(&artifact.bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| RegistryError::Storage(format!("zip open: {e}")))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| RegistryError::Storage(format!("zip entry: {e}")))?;
            let out_path = staging.join(entry.mangled_name());
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
                continue;
            }
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
        Ok(())
    }

    fn verify_signature(
        artifact: &ExtensionArtifact,
        policy: TrustPolicy,
    ) -> Result<(), RegistryError> {
        match policy {
            TrustPolicy::Loose => Ok(()),
            TrustPolicy::Strict | TrustPolicy::Normal => {
                let Some(sig) = &artifact.describe.signature else {
                    return Err(RegistryError::SignatureInvalid("missing signature".into()));
                };
                let payload = serde_json::to_vec(&artifact.describe)?;
                greentic_ext_contract::verify_ed25519(&sig.public_key, &sig.value, &payload)
                    .map_err(|e| RegistryError::SignatureInvalid(e.to_string()))
            }
        }
    }

    pub fn uninstall(
        &self,
        kind: greentic_ext_contract::ExtensionKind,
        name: &str,
        version: &str,
    ) -> Result<(), RegistryError> {
        self.storage.remove_extension(kind, name, version)
    }
}
