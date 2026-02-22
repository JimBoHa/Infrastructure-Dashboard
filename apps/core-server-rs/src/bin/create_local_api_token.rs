use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;

#[derive(Parser, Debug)]
#[command(
    about = "Create a local API token in the controller database (for automation/screenshots)."
)]
struct Args {
    #[arg(long, default_value = "/Users/Shared/FarmDashboard/setup/config.json")]
    config: String,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long, default_value = "playwright-screenshots")]
    name: String,
    #[arg(
        long,
        default_value = "config.write,users.manage,analytics.view,outputs.command,schedules.write,alerts.view,alerts.ack"
    )]
    capabilities: String,
    #[arg(long)]
    expires_in_days: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
}

fn generate_api_token() -> String {
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

fn api_token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn parse_caps(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|cap| cap.trim())
        .filter(|cap| !cap.is_empty())
        .map(|cap| cap.to_string())
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(days) = args.expires_in_days {
        anyhow::ensure!(days > 0, "expires_in_days must be > 0");
    }

    let config_db_url = if args.database_url.is_some() {
        None
    } else {
        let raw = std::fs::read_to_string(&args.config)
            .with_context(|| format!("failed to read config {}", args.config))?;
        let parsed: ConfigFile = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse config {}", args.config))?;
        parsed.database_url
    };

    let database_url = args
        .database_url
        .or(config_db_url)
        .context("database_url not provided and not found in config")?;

    let pool = PgPool::connect(&database_url)
        .await
        .context("failed to connect to database")?;

    let token = generate_api_token();
    let token_hash = api_token_hash(&token);
    let capabilities = parse_caps(&args.capabilities);
    let expires_at: Option<DateTime<Utc>> = args
        .expires_in_days
        .map(|days| Utc::now() + Duration::days(days));

    sqlx::query(
        r#"
        INSERT INTO api_tokens (name, token_hash, capabilities, created_at, expires_at)
        VALUES ($1, $2, $3, NOW(), $4)
        "#,
    )
    .bind(args.name)
    .bind(token_hash)
    .bind(SqlJson(capabilities))
    .bind(expires_at)
    .execute(&pool)
    .await
    .context("failed to insert api token")?;

    println!("{token}");
    Ok(())
}
