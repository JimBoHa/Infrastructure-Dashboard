use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::constants::{DEFAULT_SETUP_HOST, DEFAULT_SETUP_PORT};
use crate::profile::InstallProfile;

#[derive(Parser)]
#[command(name = "farmctl", version, about = "Farm Dashboard installer CLI")]
pub struct Cli {
    #[arg(long, global = true, value_enum)]
    pub profile: Option<InstallProfile>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Install(InstallArgs),
    Upgrade(InstallArgs),
    Rollback(RollbackArgs),
    Uninstall(UninstallArgs),
    Status(StatusArgs),
    Health(HealthArgs),
    Diagnostics(DiagnosticsArgs),
    Db(DbArgs),
    DevActivity(DevActivityArgs),
    Bundle(BundleArgs),
    Installer(InstallerArgs),
    Dist(DistArgs),
    Serve(ServeArgs),
    NativeDeps(NativeDepsArgs),
    Netboot(NetbootArgs),
}

#[derive(Args)]
pub struct InstallArgs {
    #[arg(long)]
    pub bundle: PathBuf,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Args)]
pub struct RollbackArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub version: Option<String>,
}

#[derive(Args)]
pub struct UninstallArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub remove_roots: bool,
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Args)]
pub struct StatusArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Args)]
pub struct HealthArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args)]
pub struct DiagnosticsArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub include_secrets: bool,
}

#[derive(Args)]
pub struct DbArgs {
    #[command(subcommand)]
    pub command: DbCommands,
}

#[derive(Subcommand)]
pub enum DbCommands {
    Migrate(DbMigrateArgs),
    SeedDemo(DbSeedDemoArgs),
}

#[derive(Args)]
pub struct DbMigrateArgs {
    /// Optional Setup config.json path (used as a fallback to resolve database_url).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Override database URL (otherwise uses CORE_DATABASE_URL/DATABASE_URL or config.json).
    #[arg(long)]
    pub database_url: Option<String>,
    /// Directory containing SQL migration files.
    #[arg(long, default_value = "infra/migrations")]
    pub migrations_root: PathBuf,
}

#[derive(Args)]
pub struct DbSeedDemoArgs {
    /// Optional Setup config.json path (used as a fallback to resolve backup_root).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Override database URL (otherwise uses CORE_DATABASE_URL/DATABASE_URL).
    #[arg(long)]
    pub database_url: Option<String>,
    /// Override backup root where fixture backups are written.
    #[arg(long)]
    pub backup_root: Option<PathBuf>,
    /// Skip writing backup fixture JSON files under the backup root.
    #[arg(long, default_value_t = false)]
    pub skip_backup_fixtures: bool,
}

#[derive(Args)]
pub struct DevActivityArgs {
    #[command(subcommand)]
    pub command: DevActivityCommand,
    /// Override the core-server base URL (defaults to http://127.0.0.1:<core_port>).
    #[arg(long)]
    pub core_url: Option<String>,
}

#[derive(Subcommand)]
pub enum DevActivityCommand {
    /// Mark the dashboard as "under active development" for the configured TTL.
    Start(DevActivityStartArgs),
    /// Clear the "active development" marker immediately.
    Stop,
    /// Print the current marker status.
    Status(DevActivityStatusArgs),
}

#[derive(Args)]
pub struct DevActivityStartArgs {
    /// How long the banner should remain visible without another heartbeat (default: 600s).
    #[arg(long, default_value_t = 600)]
    pub ttl_seconds: u64,
    /// Optional message to show in the dashboard banner.
    #[arg(long)]
    pub message: Option<String>,
    /// Optional source label (e.g., "codex").
    #[arg(long)]
    pub source: Option<String>,
}

#[derive(Args)]
pub struct DevActivityStatusArgs {
    /// Print JSON instead of a human-readable string.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args)]
pub struct BundleArgs {
    #[arg(long)]
    pub version: String,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, default_value_t = false)]
    pub skip_build: bool,
    #[arg(long)]
    pub native_deps: Option<PathBuf>,
}

#[derive(Args)]
pub struct InstallerArgs {
    #[arg(long)]
    pub bundle: PathBuf,
    #[arg(long)]
    pub version: String,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, default_value_t = false)]
    pub skip_build: bool,
    #[arg(long)]
    pub farmctl_binary: Option<PathBuf>,
}

#[derive(Args)]
pub struct DistArgs {
    #[arg(long)]
    pub version: String,
    #[arg(long)]
    pub native_deps: PathBuf,
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub skip_build: bool,
    #[arg(long)]
    pub farmctl_binary: Option<PathBuf>,
}

#[derive(Args)]
pub struct ServeArgs {
    #[arg(long, default_value = DEFAULT_SETUP_HOST)]
    pub host: String,
    #[arg(long, default_value_t = DEFAULT_SETUP_PORT)]
    pub port: u16,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub static_root: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub no_auto_open: bool,
}

#[derive(Args)]
pub struct NativeDepsArgs {
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, default_value = "17")]
    pub postgres_version: String,
    #[arg(long)]
    pub postgres_app_dmg: Option<PathBuf>,
    #[arg(long)]
    pub postgres_app_url: Option<String>,
    #[arg(long, default_value = "7.2.6")]
    pub redis_version: String,
    #[arg(long, default_value = "2.0.20")]
    pub mosquitto_version: String,
    #[arg(long, default_value = "2.24.0")]
    pub timescaledb_version: String,
    #[arg(long, default_value = "1.9.7")]
    pub qdrant_version: String,
    #[arg(long, default_value_t = false)]
    pub skip_timescaledb: bool,
    #[arg(long, default_value_t = false)]
    pub skip_qdrant: bool,
    #[arg(long, default_value_t = false)]
    pub force: bool,
    #[arg(long, default_value_t = false)]
    pub keep_temp: bool,
}

#[derive(Args)]
pub struct NetbootArgs {
    #[command(subcommand)]
    pub command: NetbootCommands,
}

#[derive(Subcommand)]
pub enum NetbootCommands {
    Prepare(NetbootPrepareArgs),
    Serve(NetbootServeArgs),
}

#[derive(Args)]
pub struct NetbootPrepareArgs {
    /// Output directory where netboot artifacts will be written.
    #[arg(long)]
    pub output: PathBuf,
    /// Override the HTTP_PATH used by the Pi bootloader (directory under output).
    #[arg(long, default_value = "net_install")]
    pub http_path: String,
    /// URL for boot.img (defaults to official Raspberry Pi host).
    #[arg(
        long,
        default_value = "https://downloads.raspberrypi.org/net_install/boot.img"
    )]
    pub boot_img_url: String,
    /// URL for boot.sig (defaults to official Raspberry Pi host).
    #[arg(
        long,
        default_value = "https://downloads.raspberrypi.org/net_install/boot.sig"
    )]
    pub boot_sig_url: String,
    /// URL for the Raspberry Pi Imager OS list JSON.
    #[arg(
        long,
        default_value = "https://downloads.raspberrypi.com/os_list_imagingutility_v4.json"
    )]
    pub imager_repo_url: String,
    /// Overwrite existing files in the output directory.
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Args)]
pub struct NetbootServeArgs {
    /// Directory containing netboot artifacts produced by `farmctl netboot prepare`.
    #[arg(long)]
    pub root: PathBuf,
    /// Address to bind the HTTP server to.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Port to bind the HTTP server to.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    /// HTTP_PATH used by the Pi bootloader (directory under root).
    #[arg(long, default_value = "net_install")]
    pub http_path: String,
}
