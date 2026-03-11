use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::{env, ffi::OsStr};
use walkdir::WalkDir;

use crate::bundle_node_overlay::build_node_agent_overlay;
use crate::cli::{BundleArgs, InstallerArgs};
use crate::config::{default_config, default_config_path, validate_bundle_path};
use crate::constants::{
    BUNDLE_ROOT_DIR, DEFAULT_SETUP_HOST, DEFAULT_SETUP_PORT, MANIFEST_NAME, MANIFEST_VERSION,
    PRODUCT_INSTALLER_NAME,
};
use crate::utils::{copy_dir, run_cmd, sha256_file, which};

const TIER_A_BUILDS_DIR: &str = "/Users/Shared/InfrastructureDashboardBuilds";

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
    let volume_root = temp_dir.path().join("InfrastructureDashboardBundle");
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
    validate_bundled_artifacts(&root).context("Failed to validate bundled artifacts")?;
    prepare_macos_bundle_payload(&root).context("Failed to sign macOS bundle payload")?;
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

    create_dmg(
        &format!("InfrastructureDashboardController-{}", args.version),
        &volume_root,
        &output,
    )?;
    codesign_container(&output)?;
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

    #[test]
    fn parse_otool_install_names_skips_header_line() {
        let output = "\
/tmp/core-server:
\t@rpath/libssl.3.dylib (compatibility version 3.0.0, current version 3.0.0)
\t/opt/homebrew/Cellar/openssl@3/3.6.1/lib/libcrypto.3.dylib (compatibility version 3.0.0, current version 3.0.0)
\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)
";
        let names = parse_otool_install_names(output);
        assert_eq!(
            names,
            vec![
                "@rpath/libssl.3.dylib".to_string(),
                "/opt/homebrew/Cellar/openssl@3/3.6.1/lib/libcrypto.3.dylib".to_string(),
                "/usr/lib/libSystem.B.dylib".to_string()
            ]
        );
    }

    #[test]
    fn classify_macos_signable_description_identifies_executables() {
        let path = Path::new("artifacts/core-server/bin/core-server");
        let description = "Mach-O 64-bit executable arm64";
        assert_eq!(
            classify_macos_signable_description(path, description),
            Some(SignableKind::Executable)
        );
    }

    #[test]
    fn classify_macos_signable_description_identifies_shared_libraries() {
        let path = Path::new("native/postgres/lib/postgresql/timescaledb.so");
        let description = "Mach-O 64-bit bundle arm64";
        assert_eq!(
            classify_macos_signable_description(path, description),
            Some(SignableKind::Library)
        );
    }

    #[test]
    fn classify_macos_signable_description_skips_non_macho_files() {
        let path = Path::new("configs/setup-config.json");
        let description = "JSON text data";
        assert_eq!(classify_macos_signable_description(path, description), None);
    }
}

pub fn installer(args: InstallerArgs) -> Result<()> {
    validate_bundle_path(&args.bundle)?;
    let temp_dir = tempfile::tempdir()?;
    let root = temp_dir.path().join("InfrastructureDashboardInstaller");
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
    let volname = env::var("FARM_DASHBOARD_INSTALLER_VOLNAME")
        .unwrap_or_else(|_| format!("InfrastructureDashboardInstaller-{}", args.version));
    create_dmg(&volname, &root, &args.output)?;
    codesign_container(&args.output)?;
    println!("Installer DMG created at {}", args.output.display());
    Ok(())
}

fn create_dmg(volname: &str, srcfolder: &Path, output: &Path) -> Result<()> {
    let mut last_error = None;
    let mut delay = Duration::from_millis(250);

    for attempt in 1..=3 {
        if output.exists() {
            let _ = fs::remove_file(output);
        }

        let result = Command::new("hdiutil")
            .arg("create")
            .arg("-volname")
            .arg(volname)
            .arg("-srcfolder")
            .arg(srcfolder)
            .arg("-ov")
            .arg("-format")
            .arg("UDZO")
            .arg(output)
            .output()
            .with_context(|| "Failed to run hdiutil create")?;

        if result.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        let detail = if stderr.trim().is_empty() {
            let stdout = stdout.trim();
            if stdout.is_empty() {
                "unknown error"
            } else {
                stdout
            }
        } else {
            stderr.trim()
        };
        last_error = Some(format!("attempt {attempt}: {detail}"));

        if attempt < 3 {
            thread::sleep(delay);
            delay = std::cmp::min(delay * 2, Duration::from_secs(2));
        }
    }

    bail!(
        "Failed to create DMG at {} ({})",
        output.display(),
        last_error.unwrap_or_else(|| "unknown error".to_string())
    )
}

fn build_farmctl_binary() -> Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .current_dir("apps/farmctl");
    run_cmd(cmd)?;
    resolve_release_binary(Path::new("apps/farmctl"), "farmctl")
}

fn resolve_release_binary(app_dir: &Path, binary_name: &str) -> Result<PathBuf> {
    let candidates = [
        app_dir.join("target/release").join(binary_name),
        PathBuf::from("target/release").join(binary_name),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    bail!(
        "{} binary missing after cargo build (checked {} and {})",
        binary_name,
        app_dir.join("target/release").join(binary_name).display(),
        PathBuf::from("target/release").join(binary_name).display()
    )
}

fn build_installer_app(
    root: &Path,
    farmctl_binary: &Path,
    controller_dmg: &Path,
    version: &str,
) -> Result<()> {
    let app_name = "Infrastructure Dashboard Installer.app";
    let app_path = root.join(app_name);
    let contents_dir = app_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;

    let farmctl_dest = resources_dir.join("farmctl");
    fs::copy(farmctl_binary, &farmctl_dest)?;
    fs::set_permissions(&farmctl_dest, fs::Permissions::from_mode(0o755))?;

    let dmg_name = format!("InfrastructureDashboardController-{version}.dmg");
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

    let launcher_bin = macos_dir.join("InfrastructureDashboardInstaller");
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
  <string>{display_name}</string>
  <key>CFBundleName</key>
  <string>{display_name}</string>
  <key>CFBundleIdentifier</key>
  <string>com.infrastructuredashboard.installer</string>
  <key>CFBundleExecutable</key>
  <string>InfrastructureDashboardInstaller</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>{version}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>InfrastructureSetupHost</key>
  <string>{host}</string>
  <key>InfrastructureSetupPort</key>
  <integer>{port}</integer>
  <key>InfrastructureSetupConfigPath</key>
  <string>{config_path}</string>
</dict>
</plist>
"#,
        version = version,
        display_name = PRODUCT_INSTALLER_NAME,
        host = DEFAULT_SETUP_HOST,
        port = DEFAULT_SETUP_PORT,
        config_path = config_path.display(),
    );
    fs::write(contents_dir.join("Info.plist"), info_plist)?;
    clear_extended_attributes(&app_path)?;
    codesign_bundled_binary(&farmctl_dest, true)?;
    codesign_bundled_binary(&launcher_bin, true)?;
    codesign_app_bundle(&app_path)?;
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

    let bin_src = resolve_release_binary(Path::new("apps/core-server-rs"), "core-server-rs")?;
    let bin_dest = bin_dir.join("core-server");
    fs::copy(&bin_src, &bin_dest).with_context(|| {
        format!(
            "Failed to copy core-server-rs binary from {} to {}",
            bin_src.display(),
            bin_dest.display()
        )
    })?;
    fs::set_permissions(&bin_dest, fs::Permissions::from_mode(0o755))?;
    bundle_core_server_openssl(&bin_dest, &core_dir)?;
    Ok(())
}

fn bundle_core_server_openssl(bin_path: &Path, core_dir: &Path) -> Result<()> {
    let lib_dir = core_dir.join("lib");
    fs::create_dir_all(&lib_dir)?;

    let openssl_lib_dir = resolve_openssl_lib_dir()?;
    let libssl_src = openssl_lib_dir.join("libssl.3.dylib");
    let libcrypto_src = openssl_lib_dir.join("libcrypto.3.dylib");
    if !libssl_src.exists() || !libcrypto_src.exists() {
        bail!(
            "OpenSSL dylibs not found in {} (missing libssl.3.dylib or libcrypto.3.dylib)",
            openssl_lib_dir.display()
        );
    }

    let libssl_dest = lib_dir.join("libssl.3.dylib");
    let libcrypto_dest = lib_dir.join("libcrypto.3.dylib");
    fs::copy(&libssl_src, &libssl_dest).with_context(|| {
        format!(
            "Failed to copy {} to {}",
            libssl_src.display(),
            libssl_dest.display()
        )
    })?;
    fs::copy(&libcrypto_src, &libcrypto_dest).with_context(|| {
        format!(
            "Failed to copy {} to {}",
            libcrypto_src.display(),
            libcrypto_dest.display()
        )
    })?;
    fs::set_permissions(&libssl_dest, fs::Permissions::from_mode(0o755))?;
    fs::set_permissions(&libcrypto_dest, fs::Permissions::from_mode(0o755))?;

    let rpath = "@loader_path/../lib";
    run_install_name_tool(&["-add_rpath", rpath], bin_path)?;
    rewrite_matching_install_names(bin_path, "libssl.3.dylib", "@rpath/libssl.3.dylib")?;
    rewrite_matching_install_names(bin_path, "libcrypto.3.dylib", "@rpath/libcrypto.3.dylib")?;

    run_install_name_tool(&["-id", "@rpath/libssl.3.dylib"], &libssl_dest)?;
    run_install_name_tool(&["-id", "@rpath/libcrypto.3.dylib"], &libcrypto_dest)?;
    rewrite_matching_install_names(
        &libssl_dest,
        "libcrypto.3.dylib",
        "@rpath/libcrypto.3.dylib",
    )?;
    codesign_bundled_binary(&libcrypto_dest, false)?;
    codesign_bundled_binary(&libssl_dest, false)?;
    codesign_bundled_binary(bin_path, true)?;

    Ok(())
}

fn validate_bundled_artifacts(root: &Path) -> Result<()> {
    let core_server = root.join("artifacts/core-server/bin/core-server");
    if !core_server.exists() {
        return Ok(());
    }

    let output = Command::new(&core_server)
        .arg("--help")
        .output()
        .with_context(|| {
            format!(
                "Failed to execute bundled core-server at {}",
                core_server.display()
            )
        })?;
    if !output.status.success() {
        bail!(
            "Bundled core-server failed to execute cleanly.\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn rewrite_matching_install_names(target: &Path, filename: &str, replacement: &str) -> Result<()> {
    for install_name in otool_install_names(target)? {
        if install_name == replacement {
            continue;
        }
        if install_name.ends_with(filename) {
            run_install_name_tool(&["-change", install_name.as_str(), replacement], target)?;
        }
    }
    Ok(())
}

fn codesign_bundled_binary(target: &Path, hardened_runtime: bool) -> Result<()> {
    let identity = env::var("FARM_BUNDLE_CODESIGN_IDENTITY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let keychain = env::var("FARM_BUNDLE_CODESIGN_KEYCHAIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let mut cmd = Command::new("codesign");
    cmd.arg("--force");
    if let Some(identity) = identity {
        cmd.arg("--timestamp");
        if hardened_runtime {
            cmd.arg("--options").arg("runtime");
        }
        cmd.arg("--sign").arg(identity);
    } else {
        cmd.arg("--sign").arg("-");
    }
    if let Some(keychain) = keychain {
        cmd.arg("--keychain").arg(keychain);
    }
    let output = cmd
        .arg(target)
        .output()
        .with_context(|| format!("Failed to run codesign on {}", target.display()))?;
    if !output.status.success() {
        bail!(
            "codesign failed for {}\nstdout:\n{}\nstderr:\n{}",
            target.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn codesign_app_bundle(app_path: &Path) -> Result<()> {
    let identity = env::var("FARM_BUNDLE_CODESIGN_IDENTITY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let keychain = env::var("FARM_BUNDLE_CODESIGN_KEYCHAIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let mut cmd = Command::new("codesign");
    cmd.arg("--force").arg("--deep");
    if let Some(identity) = identity {
        cmd.arg("--timestamp")
            .arg("--options")
            .arg("runtime")
            .arg("--sign")
            .arg(identity);
    } else {
        cmd.arg("--sign").arg("-");
    }
    if let Some(keychain) = keychain {
        cmd.arg("--keychain").arg(keychain);
    }
    let output = cmd
        .arg(app_path)
        .output()
        .with_context(|| format!("Failed to run codesign on {}", app_path.display()))?;
    if !output.status.success() {
        bail!(
            "codesign failed for app bundle {}\nstdout:\n{}\nstderr:\n{}",
            app_path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn codesign_container(target: &Path) -> Result<()> {
    let identity = env::var("FARM_BUNDLE_CODESIGN_IDENTITY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let keychain = env::var("FARM_BUNDLE_CODESIGN_KEYCHAIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let Some(identity) = identity else {
        return Ok(());
    };
    clear_extended_attributes(target)?;
    let mut cmd = Command::new("codesign");
    cmd.arg("--force")
        .arg("--timestamp")
        .arg("--sign")
        .arg(identity);
    if let Some(keychain) = keychain {
        cmd.arg("--keychain").arg(keychain);
    }
    let output = cmd
        .arg(target)
        .output()
        .with_context(|| format!("Failed to run codesign on {}", target.display()))?;
    if !output.status.success() {
        bail!(
            "codesign failed for {}\nstdout:\n{}\nstderr:\n{}",
            target.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn prepare_macos_bundle_payload(root: &Path) -> Result<()> {
    if env::consts::OS != "macos" {
        return Ok(());
    }
    clear_extended_attributes(root)?;

    let mut libraries = Vec::new();
    let mut executables = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.with_context(|| format!("Failed while walking {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        match classify_macos_signable_file(entry.path())? {
            Some(SignableKind::Library) => libraries.push(entry.into_path()),
            Some(SignableKind::Executable) => executables.push(entry.into_path()),
            None => {}
        }
    }

    libraries.sort();
    executables.sort();

    for path in libraries {
        codesign_bundled_binary(&path, false)?;
    }
    for path in executables {
        codesign_bundled_binary(&path, true)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignableKind {
    Library,
    Executable,
}

fn classify_macos_signable_file(path: &Path) -> Result<Option<SignableKind>> {
    let output = Command::new("file")
        .arg("-b")
        .arg(path)
        .output()
        .with_context(|| format!("Failed to run file on {}", path.display()))?;
    if !output.status.success() {
        bail!(
            "file failed for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(classify_macos_signable_description(
        path,
        &String::from_utf8_lossy(&output.stdout),
    ))
}

fn classify_macos_signable_description(path: &Path, description: &str) -> Option<SignableKind> {
    let description = description.trim();
    if !description.contains("Mach-O") {
        return None;
    }

    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if matches!(extension, "dylib" | "so")
        || description.contains("dynamically linked shared library")
        || description.contains("bundle")
    {
        Some(SignableKind::Library)
    } else {
        Some(SignableKind::Executable)
    }
}

fn clear_extended_attributes(path: &Path) -> Result<()> {
    if env::consts::OS != "macos" {
        return Ok(());
    }
    let Some(xattr) = which("xattr") else {
        return Ok(());
    };
    let output = Command::new(xattr)
        .arg("-cr")
        .arg(path)
        .output()
        .with_context(|| format!("Failed to run xattr -cr on {}", path.display()))?;
    if !output.status.success() {
        bail!(
            "xattr -cr failed for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn otool_install_names(target: &Path) -> Result<Vec<String>> {
    let output = Command::new("otool")
        .arg("-L")
        .arg(target)
        .output()
        .with_context(|| format!("Failed to run otool -L on {}", target.display()))?;
    if !output.status.success() {
        bail!(
            "otool -L failed for {}\nstdout:\n{}\nstderr:\n{}",
            target.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(parse_otool_install_names(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_otool_install_names(output: &str) -> Vec<String> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            Some(
                trimmed
                    .split_once(" (")
                    .map(|(name, _)| name)
                    .unwrap_or(trimmed)
                    .to_string(),
            )
        })
        .collect()
}

fn resolve_openssl_lib_dir() -> Result<PathBuf> {
    if let Ok(value) = env::var("OPENSSL_LIB_DIR") {
        let path = PathBuf::from(value);
        if path.join("libssl.3.dylib").exists() {
            return Ok(path);
        }
    }
    if let Ok(value) = env::var("OPENSSL_DIR") {
        let path = PathBuf::from(value).join("lib");
        if path.join("libssl.3.dylib").exists() {
            return Ok(path);
        }
    }

    let candidates = [
        "/opt/homebrew/opt/openssl@3/lib",
        "/usr/local/opt/openssl@3/lib",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.join("libssl.3.dylib").exists() {
            return Ok(path);
        }
    }

    if let Some(brew_path) = which("brew") {
        let output = Command::new(brew_path)
            .arg("--prefix")
            .arg("openssl@3")
            .output()
            .with_context(|| "Failed to query brew prefix for openssl@3")?;
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                let path = PathBuf::from(prefix).join("lib");
                if path.join("libssl.3.dylib").exists() {
                    return Ok(path);
                }
            }
        }
    }

    bail!("OpenSSL 3 dylibs not found. Install openssl@3 or set OPENSSL_LIB_DIR / OPENSSL_DIR.")
}

fn run_install_name_tool<S: AsRef<OsStr>>(args: &[S], target: &Path) -> Result<()> {
    let mut cmd = Command::new("install_name_tool");
    cmd.args(args).arg(target);
    run_cmd(cmd).with_context(|| {
        format!(
            "install_name_tool failed for {} with args {:?}",
            target.display(),
            args.iter()
                .map(|arg| arg.as_ref().to_string_lossy().to_string())
                .collect::<Vec<_>>()
        )
    })
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
    let bin_src = resolve_release_binary(Path::new("apps/telemetry-sidecar"), "telemetry-sidecar")?;
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
    if let Ok(prebuilt) = env::var("FARM_DASHBOARD_DASHBOARD_ARTIFACT") {
        let prebuilt = PathBuf::from(prebuilt);
        if !prebuilt.exists() {
            bail!(
                "Prebuilt dashboard artifact not found at {}",
                prebuilt.display()
            );
        }
        copy_dir(&prebuilt, &dashboard_dir).with_context(|| {
            format!(
                "Failed to copy prebuilt dashboard from {} to {}",
                prebuilt.display(),
                dashboard_dir.display()
            )
        })?;
        return Ok(());
    }

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
            if rel == Path::new(MANIFEST_NAME) {
                continue;
            }
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
