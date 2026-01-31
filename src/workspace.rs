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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_load() {
        // Create a temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let root = temp_dir.path();

        // Create a Cargo.toml
        let cargo_toml_path = root.join("Cargo.toml");
        let mut file = File::create(&cargo_toml_path).expect("Failed to create Cargo.toml");
        writeln!(
            file,
            r#"
            [package]
            name = "test-package"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            serde = "1.0"
            "#
        )
        .expect("Failed to write to Cargo.toml");

        // Initialize git repo to avoid cargo warnings/errors about being outside a workspace
        // (optional, but good practice if tests run in weird environments)
        // Actually, cargo metadata should work fine without git.

        // Create src/main.rs so cargo build/metadata doesn't complain about missing sources if it checks
        std::fs::create_dir(root.join("src")).ok();
        let mut main_rs = File::create(root.join("src/main.rs")).expect("Failed to create main.rs");
        writeln!(main_rs, "fn main() {{}}").expect("Failed to write main.rs");

        // Test loading
        let workspace = Workspace::load(root).expect("Failed to load workspace");

        assert_eq!(workspace.root, root);
        assert!(workspace.packages.contains_key("test-package"));

        // Check dependencies
        let deps = workspace.get_dependencies();
        assert!(deps.iter().any(|p| p.name == "test-package"));
        // Note: `get_dependencies` returns workspace members (which are packages), not their dependencies.
        // Wait, looking at implementation: `self.packages.values().collect()`
        // `packages` is populated from `metadata.packages`.
        // metadata.packages includes dependencies too.

        assert!(workspace.packages.contains_key("serde"));
    }
}
