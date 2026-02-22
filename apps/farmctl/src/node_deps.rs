use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::utils::run_cmd;

pub fn stage_offline_node_deps(
    vendor_dest: &Path,
    debs_dest: &Path,
    cache_root: &Path,
) -> Result<()> {
    fs::create_dir_all(vendor_dest)?;
    fs::create_dir_all(debs_dest)?;
    fs::create_dir_all(cache_root)?;

    let helper = Path::new("tools/build_node_offline_deps.py");
    if !helper.exists() {
        bail!(
            "Offline node deps helper missing at {} (run from repo root)",
            helper.display()
        );
    }

    let mut cmd = Command::new("python3");
    cmd.arg(helper)
        .arg("--vendor-dir")
        .arg(vendor_dest)
        .arg("--debs-dir")
        .arg(debs_dest)
        .arg("--cache-root")
        .arg(cache_root);
    run_cmd(cmd).context("Failed to stage offline Pi node dependencies")?;
    Ok(())
}
