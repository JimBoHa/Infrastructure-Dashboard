use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::cli::NativeDepsArgs;
use crate::utils::{copy_dir, run_cmd, which};

const DEFAULT_POSTGRES_APP_URL: &str =
    "https://github.com/PostgresApp/PostgresApp/releases/download/v2.9.2/Postgres-2.9.2-13-14-15-16-17-18.dmg";
const DEFAULT_REDIS_VERSION: &str = "7.2.6";
const DEFAULT_MOSQUITTO_VERSION: &str = "2.0.20";
const DEFAULT_TIMESCALEDB_VERSION: &str = "2.24.0";
const DEFAULT_QDRANT_VERSION: &str = "1.9.7";

pub fn build_native_deps(args: NativeDepsArgs) -> Result<()> {
    let output_root = resolve_output_root(&args.output)?;

    if output_root.exists() {
        if args.force {
            fs::remove_dir_all(&output_root)?;
        } else {
            bail!(
                "Output {} already exists (use --force to replace)",
                output_root.display()
            );
        }
    }
    fs::create_dir_all(&output_root)?;

    let temp_root = tempfile::tempdir().context("Failed to create temp workspace")?;
    let temp_path = if args.keep_temp {
        temp_root.keep()
    } else {
        temp_root.path().to_path_buf()
    };

    let postgres_version = if args.postgres_version.is_empty() {
        "17".to_string()
    } else {
        args.postgres_version
    };
    let redis_version = if args.redis_version.is_empty() {
        DEFAULT_REDIS_VERSION.to_string()
    } else {
        args.redis_version
    };
    let mosquitto_version = if args.mosquitto_version.is_empty() {
        DEFAULT_MOSQUITTO_VERSION.to_string()
    } else {
        args.mosquitto_version
    };
    let timescaledb_version = if args.timescaledb_version.is_empty() {
        DEFAULT_TIMESCALEDB_VERSION.to_string()
    } else {
        args.timescaledb_version
    };
    let qdrant_version = if args.qdrant_version.is_empty() {
        DEFAULT_QDRANT_VERSION.to_string()
    } else {
        args.qdrant_version
    };

    let postgres_root = output_root.join("postgres");
    let redis_root = output_root.join("redis");
    let mosquitto_root = output_root.join("mosquitto");
    let qdrant_root = output_root.join("qdrant");

    build_postgres_app(
        &postgres_root,
        &postgres_version,
        args.postgres_app_dmg.as_ref(),
        args.postgres_app_url.as_deref(),
        &temp_path,
    )?;
    build_redis(&redis_root, &redis_version, &temp_path)?;
    build_mosquitto(&mosquitto_root, &mosquitto_version, &temp_path)?;

    if !args.skip_timescaledb {
        install_timescaledb(&postgres_root, &timescaledb_version, &temp_path)?;
    }
    if !args.skip_qdrant {
        build_qdrant(&qdrant_root, &qdrant_version, &temp_path)?;
    }

    println!("Native dependencies prepared at {}", output_root.display());
    if args.keep_temp {
        println!(
            "Native deps build temp dir preserved at {}",
            temp_path.display()
        );
    }
    Ok(())
}

fn resolve_output_root(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .context("Failed to resolve working directory")?
        .join(path))
}

fn build_postgres_app(
    output: &Path,
    version: &str,
    local_dmg: Option<&PathBuf>,
    custom_url: Option<&str>,
    temp_root: &Path,
) -> Result<()> {
    let dmg_path = if let Some(path) = local_dmg {
        path.clone()
    } else {
        let dmg_url = custom_url.unwrap_or(DEFAULT_POSTGRES_APP_URL);
        let dmg_path = temp_root.join("postgres-app.dmg");
        download_to(dmg_url, &dmg_path)?;
        dmg_path
    };

    let mount = mount_dmg(&dmg_path)?;
    let app_root = mount
        .path()
        .join("Postgres.app")
        .join("Contents")
        .join("Versions");
    if !app_root.exists() {
        bail!(
            "Postgres.app not found in DMG at {}",
            mount.path().display()
        );
    }

    let version_dir = select_postgres_version(&app_root, version)?;
    copy_dir(&version_dir, output)?;

    let postgres_bin = output.join("bin").join("postgres");
    if !postgres_bin.exists() {
        bail!(
            "postgres binary missing after copy at {}",
            postgres_bin.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_root_resolves_relative_to_cwd() {
        let cwd = std::env::current_dir().expect("current_dir");
        let rel = Path::new("build/native-deps");
        let resolved = resolve_output_root(rel).expect("resolve_output_root");
        assert!(resolved.is_absolute());
        assert_eq!(resolved, cwd.join(rel));
    }

    #[test]
    fn output_root_preserves_absolute_paths() {
        let abs = std::env::current_dir()
            .expect("current_dir")
            .join("build/native-deps-abs");
        let resolved = resolve_output_root(&abs).expect("resolve_output_root");
        assert_eq!(resolved, abs);
    }
}

fn select_postgres_version(versions_root: &Path, version: &str) -> Result<PathBuf> {
    let direct = versions_root.join(version);
    if direct.exists() {
        return Ok(direct);
    }
    for entry in fs::read_dir(versions_root)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|v| v.to_str()) {
            if name.starts_with(version) {
                return Ok(path);
            }
        }
    }
    bail!(
        "Postgres version {} not found under {}",
        version,
        versions_root.display()
    )
}

fn build_redis(output: &Path, version: &str, temp_root: &Path) -> Result<()> {
    let archive = temp_root.join(format!("redis-{version}.tar.gz"));
    let url = format!("https://download.redis.io/releases/redis-{version}.tar.gz");
    download_to(&url, &archive)?;
    let src_root = extract_tarball(&archive, temp_root, "redis")?;

    let mut make_cmd = Command::new("make");
    make_cmd.current_dir(&src_root);
    run_cmd(make_cmd)?;

    let mut install_cmd = Command::new("make");
    install_cmd
        .current_dir(&src_root)
        .arg("install")
        .arg(format!("PREFIX={}", output.display()));
    run_cmd(install_cmd)?;
    Ok(())
}

fn build_mosquitto(output: &Path, version: &str, temp_root: &Path) -> Result<()> {
    if which("cmake").is_none() {
        bail!("cmake is required to build mosquitto (install with Xcode or Homebrew)");
    }
    let archive = temp_root.join(format!("mosquitto-{version}.tar.gz"));
    let url = format!("https://mosquitto.org/files/source/mosquitto-{version}.tar.gz");
    download_to(&url, &archive)?;
    let src_root = extract_tarball(&archive, temp_root, "mosquitto")?;
    let build_dir = src_root.join("build");
    fs::create_dir_all(&build_dir)?;

    let mut cmake_cmd = Command::new("cmake");
    cmake_cmd
        .current_dir(&build_dir)
        .arg("..")
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", output.display()))
        .arg("-DWITH_TLS=OFF")
        .arg("-DWITH_TLS_PSK=OFF")
        .arg("-DWITH_SRV=OFF")
        .arg("-DWITH_WEBSOCKETS=OFF")
        .arg("-DWITH_CJSON=OFF")
        .arg("-DWITH_DOCS=OFF")
        .arg("-DWITH_STATIC_LIBRARIES=OFF")
        .arg("-DWITH_STRIP=ON");
    run_cmd(cmake_cmd)?;

    let mut build_cmd = Command::new("cmake");
    build_cmd.current_dir(&build_dir).arg("--build").arg(".");
    run_cmd(build_cmd)?;

    let mut install_cmd = Command::new("cmake");
    install_cmd
        .current_dir(&build_dir)
        .arg("--install")
        .arg(".");
    run_cmd(install_cmd)?;
    Ok(())
}

fn build_qdrant(output: &Path, version: &str, temp_root: &Path) -> Result<()> {
    if which("cargo").is_none() {
        bail!("cargo is required to build qdrant (install Rust toolchain first)");
    }
    if which("git").is_none() {
        bail!("git is required to fetch qdrant source");
    }

    let tag = if version.trim().starts_with('v') {
        version.trim().to_string()
    } else {
        format!("v{}", version.trim())
    };

    let src_root = temp_root.join("qdrant-src");
    if src_root.exists() {
        fs::remove_dir_all(&src_root)?;
    }

    let mut clone_cmd = Command::new("git");
    clone_cmd
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--branch")
        .arg(&tag)
        .arg("https://github.com/qdrant/qdrant.git")
        .arg(&src_root);
    run_cmd(clone_cmd)?;

    let mut build_cmd = Command::new("cargo");
    build_cmd
        .current_dir(&src_root)
        .arg("build")
        .arg("--release")
        .arg("--bin")
        .arg("qdrant");
    run_cmd(build_cmd)?;

    let built = src_root.join("target/release/qdrant");
    if !built.exists() {
        bail!("qdrant binary not found after build at {}", built.display());
    }

    let bin_dir = output.join("bin");
    fs::create_dir_all(&bin_dir)?;
    let dest = bin_dir.join("qdrant");
    fs::copy(&built, &dest).with_context(|| {
        format!(
            "failed to copy qdrant {} -> {}",
            built.display(),
            dest.display()
        )
    })?;

    let mut perms = fs::metadata(&dest)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms)?;

    Ok(())
}

fn install_timescaledb(postgres_root: &Path, version: &str, temp_root: &Path) -> Result<()> {
    if which("cmake").is_none() {
        bail!("cmake is required to build timescaledb (install with Xcode or Homebrew)");
    }
    let archive = temp_root.join(format!("timescaledb-{version}.tar.gz"));
    let url =
        format!("https://github.com/timescale/timescaledb/archive/refs/tags/{version}.tar.gz");
    download_to(&url, &archive)?;
    let src_root = extract_tarball(&archive, temp_root, "timescaledb")?;

    let pg_config = postgres_root.join("bin/pg_config");
    if !pg_config.exists() {
        bail!("pg_config not found at {}", pg_config.display());
    }
    let pg_config_wrapper = build_pg_config_wrapper(&pg_config, postgres_root, temp_root)?;
    let libdir = pg_config_value(&pg_config, "--libdir")?;
    let includedir = pg_config_value(&pg_config, "--includedir-server")?;
    let ldflags = format!("-L{}", libdir.trim());
    let cppflags = format!("-I{}", includedir.trim());

    let mut bootstrap = Command::new("./bootstrap");
    bootstrap
        .current_dir(&src_root)
        .env("PG_CONFIG", &pg_config_wrapper)
        .env("LDFLAGS", &ldflags)
        .env("CPPFLAGS", &cppflags)
        .arg(format!("-DPG_CONFIG={}", pg_config_wrapper.display()))
        .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
        .arg("-DREGRESS_CHECKS=OFF")
        .arg("-DTAP_CHECKS=OFF")
        .arg("-DWARNINGS_AS_ERRORS=OFF")
        .arg("-DLINTER=OFF")
        .arg("-DUSE_OPENSSL=0");
    run_cmd(bootstrap)?;

    let build_dir = src_root.join("build");
    let mut make_cmd = Command::new("make");
    make_cmd
        .current_dir(&build_dir)
        .env("LDFLAGS", &ldflags)
        .env("CPPFLAGS", &cppflags);
    run_cmd(make_cmd)?;

    let stage_dir = temp_root.join("timescaledb-stage");
    let mut install_cmd = Command::new("make");
    install_cmd
        .current_dir(&build_dir)
        .env("LDFLAGS", &ldflags)
        .env("CPPFLAGS", &cppflags)
        .arg("install")
        .arg(format!("DESTDIR={}", stage_dir.display()));
    run_cmd(install_cmd)?;

    let lib_dest = postgres_root.join("lib/postgresql");
    let sharedir = pg_config_value(&pg_config, "--sharedir")?;
    let ext_dest = Path::new(sharedir.trim()).join("extension");
    fs::create_dir_all(&lib_dest)?;
    fs::create_dir_all(&ext_dest)?;

    for entry in WalkDir::new(&stage_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
        if name.starts_with("timescaledb") && name.ends_with(".dylib") {
            fs::copy(path, lib_dest.join(name))?;
        } else if name.starts_with("timescaledb")
            && (name.ends_with(".sql") || name.ends_with(".control"))
        {
            fs::copy(path, ext_dest.join(name))?;
        }
    }
    Ok(())
}

fn build_pg_config_wrapper(
    pg_config: &Path,
    postgres_root: &Path,
    temp_root: &Path,
) -> Result<PathBuf> {
    let output = Command::new(pg_config)
        .arg("--ldflags")
        .output()
        .with_context(|| format!("Failed to run {}", pg_config.display()))?;
    let ldflags = String::from_utf8_lossy(&output.stdout);
    let mut hardcoded_prefix = None;
    for token in ldflags.split_whitespace() {
        if let Some(path) = token.strip_prefix("-L") {
            if path.contains("Postgres.app/Contents/Versions") {
                hardcoded_prefix = Some(path.to_string());
                break;
            }
        }
    }
    let Some(prefix) = hardcoded_prefix else {
        return Ok(pg_config.to_path_buf());
    };

    let wrapper_path = temp_root.join("pg_config_wrapper");
    let script = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\nreal_pg_config=\"{}\"\nprefix_from=\"{}\"\nprefix_to=\"{}\"\noutput=\"$($real_pg_config \"$@\")\"\necho \"${{output//$prefix_from/$prefix_to}}\"\n",
        pg_config.display(),
        prefix,
        postgres_root.display(),
    );
    fs::write(&wrapper_path, script)?;
    fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755))?;
    Ok(wrapper_path)
}

fn pg_config_value(pg_config: &Path, flag: &str) -> Result<String> {
    let output = Command::new(pg_config)
        .arg(flag)
        .output()
        .with_context(|| format!("Failed to run {} {}", pg_config.display(), flag))?;
    if !output.status.success() {
        bail!("pg_config {} failed", flag);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn download_to(url: &str, dest: &Path) -> Result<()> {
    let client = Client::builder()
        .user_agent("farmctl-native-deps")
        .build()
        .context("Failed to build HTTP client")?;
    let mut last_err = None;
    for attempt in 1..=3 {
        if attempt > 1 {
            std::thread::sleep(std::time::Duration::from_secs(attempt * 2));
        }
        match download_once(&client, url, dest) {
            Ok(()) => return Ok(()),
            Err(err) => last_err = Some(err),
        }
    }
    if let Some(err) = last_err {
        bail!("Failed to download {}: {}", url, err);
    }
    bail!("Failed to download {}", url);
}

fn download_once(client: &Client, url: &str, dest: &Path) -> Result<()> {
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("Failed to download {url}"))?;
    if !response.status().is_success() {
        bail!("Download failed for {} ({})", url, response.status());
    }
    let mut file = fs::File::create(dest)?;
    if let Err(err) = io::copy(&mut response, &mut file) {
        let _ = fs::remove_file(dest);
        return Err(err.into());
    }
    Ok(())
}

fn extract_tarball(archive_path: &Path, temp_root: &Path, label: &str) -> Result<PathBuf> {
    let tar_gz = fs::File::open(archive_path)?;
    let decompressor = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(decompressor);
    let extract_root = temp_root.join(format!("{label}-src"));
    if extract_root.exists() {
        fs::remove_dir_all(&extract_root)?;
    }
    fs::create_dir_all(&extract_root)?;
    archive.unpack(&extract_root)?;

    let mut roots = Vec::new();
    for entry in fs::read_dir(&extract_root)? {
        let entry = entry?;
        if entry.path().is_dir() {
            roots.push(entry.path());
        }
    }
    if roots.len() == 1 {
        Ok(roots.remove(0))
    } else {
        bail!("Unexpected source layout for {}", archive_path.display());
    }
}

struct DmgMount {
    mount_dir: TempDir,
}

impl DmgMount {
    fn path(&self) -> &Path {
        self.mount_dir.path()
    }
}

impl Drop for DmgMount {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .arg("detach")
            .arg(self.mount_dir.path())
            .arg("-quiet")
            .status();
    }
}

fn mount_dmg(path: &Path) -> Result<DmgMount> {
    if !path.exists() {
        bail!("DMG not found at {}", path.display());
    }
    let mount_dir = tempfile::tempdir()?;
    let status = Command::new("hdiutil")
        .arg("attach")
        .arg(path)
        .arg("-nobrowse")
        .arg("-readonly")
        .arg("-mountpoint")
        .arg(mount_dir.path())
        .status()
        .context("Failed to run hdiutil attach")?;
    if !status.success() {
        bail!("Failed to mount DMG at {}", path.display());
    }
    Ok(DmgMount { mount_dir })
}
