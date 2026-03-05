use anyhow::{Context, Result};
use std::fs;

use crate::cli::{ConfigArgs, ConfigCommand, ConfigPatchArgs};
use crate::config::{patch_config, resolve_config_path, SetupConfigPatch};

pub fn handle(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Patch(args) => patch(args),
    }
}

fn patch(args: ConfigPatchArgs) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let patch_path = args.patch_file;
    let contents = fs::read_to_string(&patch_path)
        .with_context(|| format!("Failed to read patch file at {}", patch_path.display()))?;
    let patch: SetupConfigPatch = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse patch JSON at {}", patch_path.display()))?;

    let config = patch_config(&config_path, patch)?;
    let out = serde_json::to_string_pretty(&config)?;
    println!("{out}");
    Ok(())
}
