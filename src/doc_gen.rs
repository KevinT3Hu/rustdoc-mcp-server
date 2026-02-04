use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::File;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{info, instrument, warn};

pub struct DocGenerator;

impl DocGenerator {
    #[instrument(skip(cwd, target_dir))]
    pub async fn generate(
        package_name: &str,
        features: Option<&[String]>,
        cwd: &str,
        target_dir: &Path,
    ) -> Result<PathBuf> {
        let json_path = target_dir
            .join("doc")
            .join(format!("{}.json", package_name.replace('-', "_")));
        let lock_path = target_dir
            .join("doc")
            .join(format!("{}.lock", package_name.replace('-', "_")));

        info!(?json_path, "Checking for existing documentation");

        // Ensure doc dir exists
        if let Some(parent) = json_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let lock_file = File::create(&lock_path).context("Failed to create lock file")?;
        lock_file.lock_exclusive().context("Failed to lock file")?;

        if json_path.exists() {
            info!("Documentation already exists, skipping generation");
            lock_file.unlock().ok();
            return Ok(json_path);
        }

        info!("Generating documentation for package: {}", package_name);
        let mut cmd = Command::new("cargo");
        cmd.current_dir(cwd)
            .arg("+nightly")
            .arg("rustdoc")
            .arg("-p")
            .arg(package_name);

        if let Some(features) = features {
            cmd.arg("--no-default-features");
            if !features.is_empty() {
                cmd.arg("--features").arg(features.join(","));
            }
        }

        cmd.arg("--lib")
            .arg("--")
            .arg("-Z")
            .arg("unstable-options")
            .arg("--output-format")
            .arg("json");

        let output = cmd
            .output()
            .await
            .context("Failed to execute cargo rustdoc")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("cargo rustdoc failed: {}", stderr);
            lock_file.unlock().ok();
            anyhow::bail!("cargo rustdoc failed for {package_name}: {stderr}");
        }

        if !json_path.exists() {
            lock_file.unlock().ok();
            anyhow::bail!(
                "Documentation generated but file not found at expected path: {}",
                json_path.display()
            );
        }

        info!("Documentation generated successfully");
        lock_file.unlock().ok();
        Ok(json_path)
    }
}
