use std::path::{Path, PathBuf};
use std::sync::Arc;

use greentic_ext_contract::{DescribeJson, ExtensionKind};
use wasmtime::component::Component;

use crate::health::ExtensionHealth;
use crate::pool::InstancePool;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    #[must_use]
    pub fn from_describe(describe: &DescribeJson) -> Self {
        Self(describe.metadata.id.clone())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ExtensionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ExtensionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

pub struct LoadedExtension {
    pub id: ExtensionId,
    pub describe: Arc<DescribeJson>,
    pub kind: ExtensionKind,
    pub source_dir: PathBuf,
    pub component: Component,
    pub pool: InstancePool,
    pub health: ExtensionHealth,
}

impl LoadedExtension {
    pub fn load_from_dir(engine: &wasmtime::Engine, source_dir: &Path) -> anyhow::Result<Self> {
        let describe_path = source_dir.join("describe.json");
        let describe_bytes = std::fs::read(&describe_path)?;
        let describe_value: serde_json::Value = serde_json::from_slice(&describe_bytes)?;
        greentic_ext_contract::schema::validate_describe_json(&describe_value)
            .map_err(|e| anyhow::anyhow!("invalid describe.json: {e}"))?;
        let describe: DescribeJson = serde_json::from_value(describe_value)?;
        let id = ExtensionId::from_describe(&describe);
        let wasm_path = source_dir.join(&describe.runtime.component);
        let component = Component::from_file(engine, &wasm_path)?;
        let pool = InstancePool::new(2);
        let kind = describe.kind;
        Ok(Self {
            id,
            describe: Arc::new(describe),
            kind,
            source_dir: source_dir.to_path_buf(),
            component,
            pool,
            health: ExtensionHealth::Healthy,
        })
    }
}

pub type LoadedExtensionRef = Arc<LoadedExtension>;
