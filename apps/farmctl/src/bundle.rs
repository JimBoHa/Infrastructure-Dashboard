use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use crate::bundle_node_overlay::build_node_agent_overlay;
use crate::cli::{BundleArgs, InstallerArgs};
use crate::config::{default_config, default_config_path, validate_bundle_path};
use crate::constants::{
    BUNDLE_ROOT_DIR, DEFAULT_SETUP_HOST, DEFAULT_SETUP_PORT, MANIFEST_NAME, MANIFEST_VERSION,
};
use crate::utils::{copy_dir, run_cmd, sha256_file, which};

const TIER_A_BUILDS_DIR: &str = "/Users/Shared/FarmDashboardBuilds";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub format_version: u32,
    pub bundle_version: String,
    pub created_at: String,
    pub components: Vec<BundleComponent>,
    pub files: Vec<BundleFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleComponent {
    pub name: String,
    pub version: String,
    pub path: String,
    pub entrypoint: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

pub fn bundle(args: BundleArgs) -> Result<()> {
    if is_tier_a_bundle_output(&args.output) {
        require_clean_git_worktree().context("Tier-A rebuild/refresh hard gate failed")?;
    }

    let temp_dir = tempfile::tempdir()?;
    let volume_root = temp_dir.path().join("FarmDashboardBundle");
    let root = volume_root.join(BUNDLE_ROOT_DIR);
    fs::create_dir_all(root.join("artifacts"))?;
    fs::create_dir_all(root.join("configs"))?;

    if !args.skip_build {
        build_core_server(&root.join("artifacts"))?;
        build_sidecar(&root.join("artifacts"))?;
        build_dashboard(&root.join("artifacts"))?;
    }

    let native_deps = args.native_deps.as_ref().context(
        "Native deps are required for controller bundle builds (run farmctl native-deps)",
    )?;
    let native_deps = native_deps.canonicalize().with_context(|| {
        format!(
            "Failed to resolve native deps path {} (avoid passing a symlink root)",
            native_deps.display()
        )
    })?;
    validate_native_deps(&native_deps)?;
    let native_dest = root.join("native");
    copy_dir(&native_deps, &native_dest).with_context(|| {
        format!(
            "Failed to copy native deps from {} to {}",
            native_deps.display(),
            native_dest.display()
        )
    })?;
    build_node_agent_overlay(&root.join("artifacts"), true)
        .context("Failed to build node-agent overlay")?;

    write_bundle_configs(&root).context("Failed to write bundle configs")?;
    let manifest =
        build_manifest(&root, &args.version).context("Failed to build bundle manifest")?;
    fs::write(
        root.join(MANIFEST_NAME),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    let output = args.output;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let status = Command::new("hdiutil")
        .arg("create")
        .arg("-volname")
        .arg(format!("FarmDashboardController-{}", args.version))
        .arg("-srcfolder")
        .arg(&volume_root)
        .arg("-ov")
        .arg("-format")
        .arg("UDZO")
        .arg(&output)
        .status()
        .with_context(|| "Failed to run hdiutil create")?;
    if !status.success() {
        bail!("Failed to create DMG at {}", output.display());
    }
    println!("Bundle created at {}", output.display());
    Ok(())
}

fn is_tier_a_bundle_output(path: &Path) -> bool {
    path.is_absolute() && path.starts_with(TIER_A_BUILDS_DIR)
}

fn require_clean_git_worktree() -> Result<()> {
    if which("git").is_none() {
        return Ok(());
    }

    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git rev-parse")?;
    if !output.status.success() {
        return Ok(());
    }

    let repo_root = String::from_utf8_lossy(&output.stdout);
    let repo_root = repo_root.trim();
    if repo_root.is_empty() {
        return Ok(());
    }

    let status = Command::new("git")
        .args(["status", "--porcelain=v1"])
        .current_dir(repo_root)
        .output()
        .context("Failed to run git status")?;
    if !status.status.success() {
        return Ok(());
    }

    let dirty = String::from_utf8_lossy(&status.stdout);
    if dirty.trim().is_empty() {
        return Ok(());
    }

    let mut disallowed_lines: Vec<&str> = Vec::new();
    for line in dirty.lines() {
        // Preserve porcelain-v1 leading spaces (they're part of the XY prefix).
        // Example: `" M reports/foo.log"` is a modified tracked file, where X=` `, Y=`M`.
        // If we `trim()` we drop the leading space and shift indices, breaking path parsing.
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let path = porcelain_v1_line_path(line);

        if is_tier_a_allowed_dirty_path(path) {
            continue;
        }

        disallowed_lines.push(line);
    }

    if disallowed_lines.is_empty() {
        return Ok(());
    }

    bail!(
        "Refusing to build a Tier-A controller bundle from a dirty worktree.\n\n\
Hard gate: no rebuild/refresh from uncommitted changes.\n\n\
Dirty status:\n{}\n\n\
Allowed for Tier-A builds:\n\
- reports/** (logs and local validation artifacts; not bundled)\n\n\
Runbook:\n\
- docs/runbooks/controller-rebuild-refresh-tier-a.md\n",
        disallowed_lines.join("\n")
    );
}

fn is_tier_a_allowed_dirty_path(path: &str) -> bool {
    path.starts_with("reports/")
}

fn porcelain_v1_line_path(line: &str) -> &str {
    // Porcelain v1 format is generally: `XY<space><path>`
    // For renames, <path> is `"old -> new"`. We treat the destination as the relevant path.
    let line = line.trim_end();
    let path = line.get(3..).unwrap_or("").trim();
    let path = path.split(" -> ").last().unwrap_or(path);
    path.trim_matches('"')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_v1_path_preserves_leading_space_status_prefix() {
        // Leading space is part of the XY prefix (X = ' ', Y = 'M').
        let path = porcelain_v1_line_path(" M reports/foo.log");
        assert_eq!(path, "reports/foo.log");
        assert!(is_tier_a_allowed_dirty_path(path));
    }

    #[test]
    fn porcelain_v1_path_handles_renames_by_using_destination() {
        let path = porcelain_v1_line_path("R  old_path.txt -> reports/new_path.txt");
        assert_eq!(path, "reports/new_path.txt");
        assert!(is_tier_a_allowed_dirty_path(path));
    }
}

pub fn installer(args: InstallerArgs) -> Result<()> {
    validate_bundle_path(&args.bundle)?;
    let temp_dir = tempfile::tempdir()?;
    let root = temp_dir.path().join("FarmDashboardInstaller");
    fs::create_dir_all(&root)?;

    let farmctl_binary = if let Some(path) = args.farmctl_binary {
        path
    } else if args.skip_build {
        std::env::current_exe().with_context(|| "Failed to resolve farmctl binary")?
    } else {
        build_farmctl_binary()?
    };
    build_installer_app(&root, &farmctl_binary, &args.bundle, &args.version)?;

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let status = Command::new("hdiutil")
        .arg("create")
        .arg("-volname")
        .arg(format!("FarmDashboardInstaller-{}", args.version))
        .arg("-srcfolder")
        .arg(&root)
        .arg("-ov")
        .arg("-format")
        .arg("UDZO")
        .arg(&args.output)
        .status()
        .with_context(|| "Failed to run hdiutil create")?;
    if !status.success() {
        bail!(
            "Failed to create installer DMG at {}",
            args.output.display()
        );
    }
    println!("Installer DMG created at {}", args.output.display());
    Ok(())
}

fn build_farmctl_binary() -> Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .current_dir("apps/farmctl");
    run_cmd(cmd)?;
    Ok(PathBuf::from("apps/farmctl/target/release/farmctl"))
}

fn build_installer_app(
    root: &Path,
    farmctl_binary: &Path,
    controller_dmg: &Path,
    version: &str,
) -> Result<()> {
    let app_name = "Farm Dashboard Installer.app";
    let app_path = root.join(app_name);
    let contents_dir = app_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;

    let farmctl_dest = resources_dir.join("farmctl");
    fs::copy(farmctl_binary, &farmctl_dest)?;
    fs::set_permissions(&farmctl_dest, fs::Permissions::from_mode(0o755))?;

    let dmg_name = format!("FarmDashboardController-{version}.dmg");
    let dmg_dest = resources_dir.join(dmg_name);
    fs::copy(controller_dmg, &dmg_dest)?;

    let launcher_src = PathBuf::from("apps/farmctl/installer_launcher/main.swift");
    if !launcher_src.exists() {
        bail!(
            "Missing Swift installer launcher source at {}",
            launcher_src.display()
        );
    }
    if which("swiftc").is_none() {
        bail!("swiftc not found; install Xcode command line tools to build the installer launcher");
    }

    let launcher_bin = macos_dir.join("FarmDashboardInstaller");
    let mut cmd = Command::new("swiftc");
    cmd.arg("-O")
        .arg("-framework")
        .arg("Cocoa")
        .arg(&launcher_src)
        .arg("-o")
        .arg(&launcher_bin);
    run_cmd(cmd)?;
    fs::set_permissions(&launcher_bin, fs::Permissions::from_mode(0o755))?;

    let config_path = default_config_path();
    let info_plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key>
  <string>Farm Dashboard Installer</string>
  <key>CFBundleName</key>
  <string>Farm Dashboard Installer</string>
  <key>CFBundleIdentifier</key>
  <string>com.farmdashboard.installer</string>
  <key>CFBundleExecutable</key>
  <string>FarmDashboardInstaller</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>{version}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>FarmSetupHost</key>
  <string>{host}</string>
  <key>FarmSetupPort</key>
  <integer>{port}</integer>
  <key>FarmSetupConfigPath</key>
  <string>{config_path}</string>
</dict>
</plist>
"#,
        version = version,
        host = DEFAULT_SETUP_HOST,
        port = DEFAULT_SETUP_PORT,
        config_path = config_path.display(),
    );
    fs::write(contents_dir.join("Info.plist"), info_plist)?;
    Ok(())
}

fn build_core_server(artifacts_root: &Path) -> Result<()> {
    let core_dir = artifacts_root.join("core-server");
    fs::create_dir_all(&core_dir)?;
    let bin_dir = core_dir.join("bin");
    fs::create_dir_all(&bin_dir)?;
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg("apps/core-server-rs/Cargo.toml");
    run_cmd(cmd)?;

    let bin_src = Path::new("apps/core-server-rs/target/release/core-server-rs");
    if !bin_src.exists() {
        bail!(
            "core-server-rs binary missing at {} (cargo build succeeded but binary not found)",
            bin_src.display()
        );
    }
    let bin_dest = bin_dir.join("core-server");
    fs::copy(&bin_src, &bin_dest).with_context(|| {
        format!(
            "Failed to copy core-server-rs binary from {} to {}",
            bin_src.display(),
            bin_dest.display()
        )
    })?;
    fs::set_permissions(&bin_dest, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn build_sidecar(artifacts_root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir("apps/telemetry-sidecar")
        .status()
        .with_context(|| "Failed to run cargo build for telemetry-sidecar")?;
    if !status.success() {
        bail!("telemetry-sidecar build failed");
    }
    let bin_src = Path::new("apps/telemetry-sidecar/target/release/telemetry-sidecar");
    let sidecar_dir = artifacts_root.join("telemetry-sidecar");
    fs::create_dir_all(sidecar_dir.join("bin"))?;
    let bin_dest = sidecar_dir.join("bin/telemetry-sidecar");
    fs::copy(&bin_src, &bin_dest)?;
    fs::set_permissions(&bin_dest, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn build_dashboard(artifacts_root: &Path) -> Result<()> {
    let dashboard_dir = artifacts_root.join("dashboard-web");
    fs::create_dir_all(&dashboard_dir)?;

    let temp_dir = tempfile::tempdir()?;
    let work_dir = temp_dir.path().join("dashboard-web");
    fs::create_dir_all(&work_dir)?;
    let root_src = Path::new("apps/dashboard-web");
    let items = [
        "package.json",
        "package-lock.json",
        "next-env.d.ts",
        "next.config.ts",
        "tsconfig.json",
        "postcss.config.mjs",
        "eslint.config.mjs",
        "scripts",
        "src",
        "public",
    ];
    for item in items {
        let src = root_src.join(item);
        if !src.exists() {
            continue;
        }
        let dest = work_dir.join(item);
        if src.is_dir() {
            copy_dir(&src, &dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dest)?;
        }
    }

    let npm_cache = std::env::var("NPM_CONFIG_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| work_dir.join(".npm-cache"));
    fs::create_dir_all(&npm_cache)?;

    let mut npm_install = Command::new("npm");
    npm_install
        .arg("install")
        .current_dir(&work_dir)
        .env("npm_config_cache", &npm_cache);
    run_cmd(npm_install)?;
    let mut npm_build = Command::new("npm");
    npm_build
        .arg("run")
        .arg("build")
        .current_dir(&work_dir)
        .env("FARM_DASHBOARD_STATIC", "1")
        .env("npm_config_cache", &npm_cache);
    run_cmd(npm_build)?;

    let export_root = work_dir.join("out");
    if !export_root.exists() {
        bail!(
            "dashboard-web static export output missing at {}",
            export_root.display()
        );
    }
    copy_dir(&export_root, &dashboard_dir.join("static"))
        .context("Failed to copy dashboard-web static export")?;
    Ok(())
}

fn write_bundle_configs(bundle_root: &Path) -> Result<()> {
    let defaults = default_config()?;
    let config_path = bundle_root.join("configs/setup-config.json");
    fs::write(config_path, serde_json::to_string_pretty(&defaults)?)?;

    if Path::new("infra/migrations").exists() {
        copy_dir(
            Path::new("infra/migrations"),
            &bundle_root.join("migrations"),
        )?;
    }

    if Path::new("apps/core-server-rs/.env.example").exists() {
        fs::copy(
            "apps/core-server-rs/.env.example",
            bundle_root.join("configs/core-server.env.example"),
        )?;
    }
    Ok(())
}

fn validate_native_deps(root: &Path) -> Result<()> {
    let required = [
        ("postgres", "postgres/bin/postgres"),
        ("postgres initdb", "postgres/bin/initdb"),
        ("redis", "redis/bin/redis-server"),
        ("qdrant", "qdrant/bin/qdrant"),
    ];
    for (label, rel) in required {
        let path = root.join(rel);
        if !path.exists() {
            bail!("Missing native dependency {} at {}", label, path.display());
        }
    }
    let mosquitto_bin = root.join("mosquitto/bin/mosquitto");
    let mosquitto_sbin = root.join("mosquitto/sbin/mosquitto");
    if !mosquitto_bin.exists() && !mosquitto_sbin.exists() {
        bail!(
            "Missing native dependency mosquitto at {} or {}",
            mosquitto_bin.display(),
            mosquitto_sbin.display()
        );
    }
    Ok(())
}

fn build_manifest(bundle_root: &Path, version: &str) -> Result<BundleManifest> {
    let mut files = Vec::new();
    for entry in WalkDir::new(bundle_root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(bundle_root)
                .unwrap_or(entry.path());
            let digest = sha256_file(entry.path())?;
            let size = entry.metadata()?.len();
            files.push(BundleFile {
                path: rel.to_string_lossy().to_string(),
                sha256: digest,
                size_bytes: size,
            });
        }
    }

    let component_defs = vec![
        (
            "core-server",
            "artifacts/core-server",
            "artifacts/core-server/bin/core-server",
        ),
        (
            "telemetry-sidecar",
            "artifacts/telemetry-sidecar",
            "artifacts/telemetry-sidecar/bin/telemetry-sidecar",
        ),
    ];

    let mut components = Vec::new();
    for (name, path, entrypoint) in component_defs {
        let entry_path = bundle_root.join(entrypoint);
        let digest = sha256_file(&entry_path)?;
        let size = entry_path.metadata()?.len();
        components.push(BundleComponent {
            name: name.to_string(),
            version: version.to_string(),
            path: path.to_string(),
            entrypoint: entrypoint.to_string(),
            sha256: digest,
            size_bytes: size,
        });
    }

    Ok(BundleManifest {
        format_version: MANIFEST_VERSION,
        bundle_version: version.to_string(),
        created_at: Utc::now().to_rfc3339(),
        components,
        files,
    })
}
