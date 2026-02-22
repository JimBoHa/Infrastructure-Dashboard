use anyhow::{bail, Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Builder;
use walkdir::WalkDir;

use crate::node_deps::stage_offline_node_deps;
use crate::utils::{run_cmd, which};

pub fn build_node_agent_overlay(artifacts_root: &Path, include_offline_deps: bool) -> Result<()> {
    let node_agent_root = Path::new("apps/node-agent");
    if !node_agent_root.exists() {
        bail!(
            "Node agent source not found at {}",
            node_agent_root.display()
        );
    }

    let node_forwarder_bin = build_node_forwarder_binary(include_offline_deps)
        .context("Failed to build node-forwarder")?;

    let temp_dir = tempfile::tempdir()?;
    let overlay_root = temp_dir.path().join("node-agent-overlay");
    fs::create_dir_all(overlay_root.join("etc/systemd/system"))?;
    fs::create_dir_all(overlay_root.join("etc/logrotate.d"))?;
    fs::create_dir_all(overlay_root.join("usr/local/bin"))?;
    fs::create_dir_all(overlay_root.join("opt"))?;

    copy_node_agent_source(node_agent_root, &overlay_root.join("opt/node-agent"))?;
    write_node_agent_build_info(
        &overlay_root.join("opt/node-agent/app/build_info.py"),
        if include_offline_deps { "prod" } else { "test" },
    )?;
    export_requirements(
        node_agent_root,
        &overlay_root.join("opt/node-agent/requirements.txt"),
    )?;
    copy_systemd_units(node_agent_root, &overlay_root.join("etc/systemd/system"))?;
    copy_logrotate_configs(node_agent_root, &overlay_root.join("etc/logrotate.d"))?;
    copy_scripts(node_agent_root, &overlay_root.join("usr/local/bin"))?;
    copy_env_sample(node_agent_root, &overlay_root.join("etc/node-agent.env"))?;
    if let Some(bin_path) = node_forwarder_bin {
        let target = overlay_root.join("usr/local/bin/node-forwarder");
        fs::copy(&bin_path, &target).with_context(|| {
            format!(
                "Failed to copy node-forwarder binary from {} to {}",
                bin_path.display(),
                target.display()
            )
        })?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    }

    if include_offline_deps {
        fs::create_dir_all(overlay_root.join("opt/node-agent/vendor"))?;
        fs::create_dir_all(overlay_root.join("opt/node-agent/debs"))?;
        let cache_root = Path::new("build/cache");
        fs::create_dir_all(cache_root)?;
        stage_offline_node_deps(
            &overlay_root.join("opt/node-agent/vendor"),
            &overlay_root.join("opt/node-agent/debs"),
            cache_root,
        )?;
    }

    let dest_dir = artifacts_root.join("node-agent");
    fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join("node-agent-overlay.tar.gz");
    create_overlay_tar(&overlay_root, &dest)?;
    Ok(())
}

fn build_node_forwarder_binary(include: bool) -> Result<Option<PathBuf>> {
    if !include {
        return Ok(None);
    }

    let manifest = Path::new("apps/node-forwarder/Cargo.toml");
    if !manifest.exists() {
        bail!("node-forwarder source not found at {}", manifest.display());
    }

    // We ship a static aarch64 linux binary inside the node overlay.
    let target_triple = "aarch64-unknown-linux-musl";

    if which("zig").is_none() || which("cargo-zigbuild").is_none() {
        bail!(
            "Missing cross-compile tools for node-forwarder.\n\n\
Required:\n\
- zig (install via `brew install zig`)\n\
- cargo-zigbuild (install via `cargo install cargo-zigbuild`)\n\
- rust target {target_triple} (install via `rustup target add {target_triple}`)\n"
        );
    }

    let mut cmd = Command::new("cargo");
    cmd.args([
        "zigbuild",
        "--manifest-path",
        manifest.to_str().unwrap_or_default(),
        "--release",
        "--target",
        target_triple,
    ])
    .current_dir(Path::new("."));
    run_cmd(cmd).context("cargo zigbuild node-forwarder")?;

    let bin = Path::new("apps/node-forwarder/target")
        .join(target_triple)
        .join("release")
        .join("node-forwarder");
    if !bin.exists() {
        bail!(
            "node-forwarder build completed but binary missing at {}",
            bin.display()
        );
    }

    Ok(Some(bin))
}

fn write_node_agent_build_info(path: &Path, flavor: &str) -> Result<()> {
    if !matches!(flavor, "prod" | "dev" | "test") {
        bail!("Invalid node-agent build flavor {flavor:?}");
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        format!(
            "from __future__ import annotations\n\n\
from typing import Literal\n\n\
BUILD_FLAVOR: Literal[\"prod\", \"dev\", \"test\"] = \"{flavor}\"\n"
        ),
    )?;
    Ok(())
}

fn copy_node_agent_source(source: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest)?;
    }

    let ignore_dirs = [
        "__pycache__",
        ".pytest_cache",
        "tests",
        "systemd",
        "scripts",
    ];
    let mut walker = WalkDir::new(source).into_iter();
    while let Some(entry) = walker.next() {
        let entry = entry?;
        let rel = entry.path().strip_prefix(source).unwrap_or(entry.path());
        if rel.as_os_str().is_empty() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy();
        if entry.file_type().is_dir() {
            if ignore_dirs.iter().any(|dir| *dir == file_name) {
                walker.skip_current_dir();
            }
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if file_name.ends_with(".pyc") || file_name.ends_with(".pyo") {
            continue;
        }
        if rel == Path::new("storage/node_config.json") {
            continue;
        }
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &target)?;
    }
    Ok(())
}

fn copy_systemd_units(source_root: &Path, dest: &Path) -> Result<()> {
    let systemd_root = source_root.join("systemd");
    for entry in fs::read_dir(&systemd_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        let is_unit = (name.starts_with("node-agent")
            && (name.ends_with(".service") || name.ends_with(".timer") || name.ends_with(".path")))
            || name == "renogy-bt.service"
            || name == "node-forwarder.service";
        if !is_unit {
            continue;
        }
        fs::copy(&path, dest.join(name))?;
    }
    Ok(())
}

fn copy_logrotate_configs(source_root: &Path, dest: &Path) -> Result<()> {
    let logrotate_root = source_root.join("systemd/logrotate");
    if !logrotate_root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&logrotate_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().context("Missing logrotate filename")?;
            fs::copy(&path, dest.join(name))?;
        }
    }
    Ok(())
}

fn copy_scripts(source_root: &Path, dest: &Path) -> Result<()> {
    let scripts_root = source_root.join("scripts");
    let scripts_map = [
        ("node-agent-python.sh", "node-agent-python"),
        ("node-agent-logrotate.sh", "node-agent-logrotate"),
        (
            "node-agent-optional-services.py",
            "node-agent-optional-services",
        ),
        ("verify_backups.py", "node-agent-verify-backups"),
    ];
    for (src_name, dest_name) in scripts_map {
        let src = scripts_root.join(src_name);
        if !src.exists() {
            continue;
        }
        let target = dest.join(dest_name);
        fs::copy(&src, &target)?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn copy_env_sample(source_root: &Path, dest: &Path) -> Result<()> {
    let env_sample = source_root.join("systemd/node-agent.env.sample");
    if env_sample.exists() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(env_sample, dest)?;
    }
    Ok(())
}

fn export_requirements(project_root: &Path, dest: &Path) -> Result<()> {
    let pyproject_path = project_root.join("pyproject.toml");
    let contents = fs::read_to_string(&pyproject_path)
        .with_context(|| format!("Failed to read {}", pyproject_path.display()))?;
    let value: toml::Value = contents
        .parse()
        .with_context(|| format!("Failed to parse {}", pyproject_path.display()))?;
    let deps = value
        .get("project")
        .and_then(|v| v.get("dependencies"))
        .and_then(|v| v.as_array())
        .context("pyproject.toml missing [project].dependencies")?;
    let normalized: Vec<String> = deps
        .iter()
        .filter_map(|v| v.as_str())
        .map(normalize_requirement)
        .collect();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, format!("{}\n", normalized.join("\n")))?;
    Ok(())
}

fn normalize_requirement(requirement: &str) -> String {
    let trimmed = requirement.trim();
    let Some((name, rest)) = trimmed.split_once('(') else {
        return trimmed.to_string();
    };
    let Some((spec, tail)) = rest.split_once(')') else {
        return trimmed.to_string();
    };
    let spec = spec.replace(' ', "");
    let mut normalized = format!("{}{}", name.trim(), spec);
    let tail = tail.trim();
    if !tail.is_empty() {
        normalized = format!("{normalized} {tail}");
    }
    normalized
}

fn create_overlay_tar(source: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if dest.exists() {
        fs::remove_file(dest)?;
    }

    let tar_gz = fs::File::create(dest)?;
    let encoder = GzEncoder::new(tar_gz, Compression::default());
    let mut builder = Builder::new(encoder);
    builder.append_dir_all(".", source)?;
    let encoder = builder.into_inner()?;
    encoder.finish()?;
    Ok(())
}
