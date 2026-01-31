use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand, Package};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub metadata: Metadata,
    /// Map of package name to Package
    pub packages: HashMap<String, Package>,
}

impl Workspace {
    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let metadata = MetadataCommand::new()
            .manifest_path(root.join("Cargo.toml"))
            .exec()
            .context("Failed to load cargo metadata")?;

        let mut packages = HashMap::new();
        for pkg in &metadata.packages {
            packages.insert(pkg.name.to_string(), pkg.clone());
        }

        Ok(Self {
            root: root.to_path_buf(),
            metadata,
            packages,
        })
    }

    pub fn has_nightly_toolchain() -> bool {
        Command::new("rustc")
            .arg("+nightly")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Returns a list of all dependencies (direct and transitive) for the workspace members.
    pub fn get_dependencies(&self) -> Vec<&Package> {
        self.packages.values().collect()
    }
}
