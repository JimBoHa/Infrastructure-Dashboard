use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use zip::write::FileOptions;

#[derive(Debug, Clone, Serialize)]
pub struct CommandResult {
    pub command: String,
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub returncode: i32,
}

pub fn which<S: AsRef<OsStr>>(cmd: S) -> Option<PathBuf> {
    let cmd_ref = cmd.as_ref();
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(cmd_ref);
            if candidate.exists() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

pub fn port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn allocate_local_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

pub fn writable_dir(path: &Path) -> bool {
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let test_path = path.join(".write-test");
    if fs::write(&test_path, "ok").is_err() {
        return false;
    }
    let _ = fs::remove_file(test_path);
    true
}

pub fn run_cmd(mut command: Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        bail!("Command failed: {command:?}");
    }
    Ok(())
}

pub fn run_cmd_capture(mut command: Command) -> Result<CommandResult> {
    let mut command_display = command.get_program().to_string_lossy().to_string();
    for arg in command.get_args() {
        command_display.push(' ');
        command_display.push_str(&arg.to_string_lossy());
    }
    let output = command.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Ok(CommandResult {
        command: command_display,
        ok: output.status.success(),
        stdout,
        stderr,
        returncode: output.status.code().unwrap_or(-1),
    })
}

pub fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(dst) {
        let file_type = meta.file_type();
        if file_type.is_dir() {
            fs::remove_dir_all(dst)
                .with_context(|| format!("Failed to remove {}", dst.display()))?;
        } else {
            fs::remove_file(dst).with_context(|| format!("Failed to remove {}", dst.display()))?;
        }
    }
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create directory {}", dst.display()))?;
    for entry in WalkDir::new(src) {
        let entry = entry.with_context(|| format!("Failed while walking {}", src.display()))?;
        let rel = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)
                .with_context(|| format!("Failed to create directory {}", target.display()))?;
        } else if entry.file_type().is_symlink() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory {}", parent.display()))?;
            }
            let link_target = fs::read_link(entry.path())
                .with_context(|| format!("Failed to read symlink {}", entry.path().display()))?;
            if target.exists() {
                fs::remove_file(&target)
                    .with_context(|| format!("Failed to remove {}", target.display()))?;
            }
            symlink(link_target, &target)
                .with_context(|| format!("Failed to create symlink {}", target.display()))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory {}", parent.display()))?;
            }
            fs::copy(entry.path(), &target).with_context(|| {
                format!(
                    "Failed to copy file {} -> {}",
                    entry.path().display(),
                    target.display()
                )
            })?;
            let mode = entry.metadata()?.permissions().mode();
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                .with_context(|| format!("Failed to chmod {} to {:o}", target.display(), mode))?;
        }
    }
    Ok(())
}

pub fn symlink_force(target: &Path, link: &Path) -> Result<()> {
    if link.exists() || link.is_symlink() {
        fs::remove_file(link)?;
    }
    symlink(target, link)?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn redact_secrets(contents: &str) -> String {
    let mut redacted = String::new();
    for line in contents.lines() {
        if line.contains("password") || line.contains("token") || line.contains("secret") {
            let key = line.split('=').next().unwrap_or("secret");
            redacted.push_str(&format!("{key}=REDACTED\n"));
        } else {
            redacted.push_str(line);
            redacted.push('\n');
        }
    }
    redacted
}

pub fn write_zip_entry<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    contents: &[u8],
    options: FileOptions,
) -> Result<()> {
    zip.start_file(name, options)?;
    zip.write_all(contents)?;
    Ok(())
}
