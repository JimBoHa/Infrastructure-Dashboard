use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::bundle;
use crate::cli::{BundleArgs, DistArgs, InstallerArgs};
use crate::utils::sha256_file;

fn default_output_dir(version: &str) -> PathBuf {
    PathBuf::from(format!("build/release-{version}"))
}

fn installer_filename(version: &str) -> String {
    format!("FarmDashboardInstaller-{version}.dmg")
}

fn ensure_public_release_dir(output_dir: &Path) -> Result<()> {
    if !output_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(output_dir)
        .with_context(|| format!("failed to read {}", output_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => continue,
        };
        if filename.starts_with("FarmDashboardController-") && filename.ends_with(".dmg") {
            anyhow::bail!(
                "Refusing to build a public release into {} because it already contains an internal controller DMG ({filename}). Use a clean output dir or delete the controller DMG; public releases must ship only FarmDashboardInstaller-<version>.dmg.",
                output_dir.display()
            );
        }
    }
    Ok(())
}

fn write_sha256_sums(path: &Path) -> Result<()> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("installer path missing filename")?;
    let sha = sha256_file(path)?;
    let output = path
        .parent()
        .context("installer path missing parent directory")?
        .join("SHA256SUMS.txt");
    fs::write(&output, format!("{sha}  {filename}\n"))
        .with_context(|| format!("failed to write {}", output.display()))?;
    Ok(())
}

pub fn dist(args: DistArgs) -> Result<()> {
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| default_output_dir(&args.version));
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    ensure_public_release_dir(&output_dir)?;

    let temp_dir =
        tempfile::tempdir().context("failed to create temp dir for controller bundle")?;
    let bundle_path = temp_dir
        .path()
        .join(format!("FarmDashboardController-{}.dmg", args.version));

    bundle::bundle(BundleArgs {
        version: args.version.clone(),
        output: bundle_path.clone(),
        skip_build: args.skip_build,
        native_deps: Some(args.native_deps.clone()),
    })?;

    let installer_path = output_dir.join(installer_filename(&args.version));
    bundle::installer(InstallerArgs {
        bundle: bundle_path,
        version: args.version.clone(),
        output: installer_path.clone(),
        skip_build: true,
        farmctl_binary: args.farmctl_binary,
    })?;

    write_sha256_sums(&installer_path)?;

    println!("Public installer artifact: {}", installer_path.display());
    Ok(())
}
