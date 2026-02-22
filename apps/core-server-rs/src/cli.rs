use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "core-server-rs",
    version,
    about = "Rust core server (migration)"
)]
pub struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    #[arg(long)]
    pub static_root: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub print_openapi: bool,
}
