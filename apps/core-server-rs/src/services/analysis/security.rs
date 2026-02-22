use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
pub fn apply_umask() {
    // Ensure newly created files are not world-readable.
    unsafe {
        libc::umask(0o027);
    }
}

#[cfg(not(unix))]
pub fn apply_umask() {}

pub fn ensure_dir_mode(path: &Path, mode: u32) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("failed to chmod {} to {:o}", path.display(), mode))?;
    }
    Ok(())
}

pub fn ensure_file_mode(path: &Path, mode: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("failed to chmod {} to {:o}", path.display(), mode))?;
    }
    Ok(())
}
