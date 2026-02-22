use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    about = "Ops tool: audit sensors for invalid history rows (e.g., forecast_points/derived sensors with metrics rows) and optionally purge contamination."
)]
struct Args {
    #[arg(long, default_value = "/Users/Shared/FarmDashboard/setup/config.json")]
    config: String,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[arg(long, default_value_t = false)]
    apply: bool,
    #[arg(long)]
    confirm: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
}

fn normalize_database_url(url: String) -> String {
    if let Some(stripped) = url.strip_prefix("postgresql+psycopg://") {
        return format!("postgresql://{stripped}");
    }
    url
}

#[derive(Debug, sqlx::FromRow)]
struct BadSensorRow {
    sensor_id: String,
    sensor_name: String,
    node_id: Uuid,
    node_name: String,
    source: Option<String>,
}

async fn load_rows(pool: &PgPool, query: &str, limit: i64) -> Result<Vec<BadSensorRow>> {
    let rows: Vec<BadSensorRow> = sqlx::query_as(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("failed to query sensors")?;
    Ok(rows)
}

async fn load_ids(pool: &PgPool, query: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(query)
        .fetch_all(pool)
        .await
        .context("failed to query sensor ids")?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

fn print_rows(title: &str, rows: &[BadSensorRow]) {
    println!("\n== {title} ==");
    if rows.is_empty() {
        println!("ok (none found)");
        return;
    }
    for row in rows {
        println!(
            "- {} ({}) node={} ({}) source={}",
            row.sensor_id,
            row.sensor_name,
            row.node_id,
            row.node_name,
            row.source.as_deref().unwrap_or(""),
        );
    }
}

async fn purge_metrics(pool: &PgPool, sensor_ids: &[String]) -> Result<u64> {
    if sensor_ids.is_empty() {
        return Ok(0);
    }
    let res = sqlx::query("DELETE FROM metrics WHERE sensor_id = ANY($1)")
        .bind(sensor_ids)
        .execute(pool)
        .await
        .context("failed to delete metrics contamination")?;
    Ok(res.rows_affected())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.apply {
        let confirm = args
            .confirm
            .as_deref()
            .context("--confirm is required with --apply")?;
        anyhow::ensure!(
            confirm == "DELETE_CONTAMINATION",
            "--confirm must be exactly DELETE_CONTAMINATION"
        );
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
    let database_url = normalize_database_url(database_url);

    let pool = PgPool::connect(&database_url)
        .await
        .context("failed to connect to database")?;

    let forecast_with_metrics_q = r#"
        SELECT
          sensors.sensor_id as sensor_id,
          sensors.name as sensor_name,
          nodes.id as node_id,
          nodes.name as node_name,
          NULLIF(TRIM(COALESCE(sensors.config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        JOIN nodes ON nodes.id = sensors.node_id
        WHERE sensors.deleted_at IS NULL
          AND NULLIF(TRIM(COALESCE(sensors.config, '{}'::jsonb)->>'source'), '') = 'forecast_points'
          AND EXISTS (SELECT 1 FROM metrics WHERE metrics.sensor_id = sensors.sensor_id LIMIT 1)
        ORDER BY sensors.sensor_id
        LIMIT $1
    "#;

    let derived_with_metrics_q = r#"
        SELECT
          sensors.sensor_id as sensor_id,
          sensors.name as sensor_name,
          nodes.id as node_id,
          nodes.name as node_name,
          NULLIF(TRIM(COALESCE(sensors.config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        JOIN nodes ON nodes.id = sensors.node_id
        WHERE sensors.deleted_at IS NULL
          AND NULLIF(TRIM(COALESCE(sensors.config, '{}'::jsonb)->>'source'), '') = 'derived'
          AND EXISTS (SELECT 1 FROM metrics WHERE metrics.sensor_id = sensors.sensor_id LIMIT 1)
        ORDER BY sensors.sensor_id
        LIMIT $1
    "#;

    let forecast_with_metrics = load_rows(&pool, forecast_with_metrics_q, args.limit).await?;
    let derived_with_metrics = load_rows(&pool, derived_with_metrics_q, args.limit).await?;

    print_rows(
        "forecast_points sensors with metrics rows (invalid: forecast sensors must not store metrics)",
        &forecast_with_metrics,
    );
    print_rows(
        "derived sensors with metrics rows (invalid: derived sensors must not store metrics)",
        &derived_with_metrics,
    );

    if !args.apply {
        println!(
            "\nDry-run only. Re-run with --apply --confirm DELETE_CONTAMINATION to purge detected contamination."
        );
        return Ok(());
    }

    let forecast_ids_q = r#"
        SELECT sensors.sensor_id
        FROM sensors
        WHERE sensors.deleted_at IS NULL
          AND NULLIF(TRIM(COALESCE(sensors.config, '{}'::jsonb)->>'source'), '') = 'forecast_points'
          AND EXISTS (SELECT 1 FROM metrics WHERE metrics.sensor_id = sensors.sensor_id LIMIT 1)
        ORDER BY sensors.sensor_id
    "#;
    let derived_ids_q = r#"
        SELECT sensors.sensor_id
        FROM sensors
        WHERE sensors.deleted_at IS NULL
          AND NULLIF(TRIM(COALESCE(sensors.config, '{}'::jsonb)->>'source'), '') = 'derived'
          AND EXISTS (SELECT 1 FROM metrics WHERE metrics.sensor_id = sensors.sensor_id LIMIT 1)
        ORDER BY sensors.sensor_id
    "#;

    let mut metrics_ids = load_ids(&pool, forecast_ids_q).await?;
    metrics_ids.extend(load_ids(&pool, derived_ids_q).await?);
    metrics_ids.sort();
    metrics_ids.dedup();

    let deleted_metrics = purge_metrics(&pool, &metrics_ids).await?;

    println!("\nPurge complete: deleted_metrics_rows={}", deleted_metrics);
    Ok(())
}
