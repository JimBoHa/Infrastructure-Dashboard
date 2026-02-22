mod bootstrap_admin;
mod bundle;
mod bundle_node_overlay;
mod cli;
mod config;
mod constants;
mod dev_activity;
mod dev_db;
mod dist;
mod health;
mod install;
mod launchd;
mod migrations;
mod native;
mod native_deps;
mod net;
mod netboot;
mod node_deps;
mod paths;
mod privileged;
mod processes;
mod profile;
mod server;
mod service_user;
mod sysv_ipc;
mod uninstall;
mod utils;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::install::InstallMode;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let profile = cli.profile;
    match cli.command {
        Commands::Install(args) => install::install_bundle(args, InstallMode::Install, profile),
        Commands::Upgrade(args) => install::install_bundle(args, InstallMode::Upgrade, profile),
        Commands::Rollback(args) => install::rollback(args, profile),
        Commands::Uninstall(args) => uninstall::uninstall(args, profile),
        Commands::Status(args) => install::status(args, profile),
        Commands::Health(args) => install::health(args, profile),
        Commands::Diagnostics(args) => install::diagnostics(args, profile),
        Commands::Db(args) => dev_db::handle(args),
        Commands::DevActivity(args) => dev_activity::handle(args),
        Commands::Bundle(args) => bundle::bundle(args),
        Commands::Installer(args) => bundle::installer(args),
        Commands::Dist(args) => dist::dist(args),
        Commands::Serve(args) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(server::serve(args, profile))
        }
        Commands::NativeDeps(args) => native_deps::build_native_deps(args),
        Commands::Netboot(args) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(netboot::handle(args))
        }
    }
}
