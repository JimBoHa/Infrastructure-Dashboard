use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    about = "Ops tool: purge raw metric points for a single sensor within a timestamp window (transactional)."
)]
struct Args {
    #[arg(long, default_value = "/Users/Shared/FarmDashboard/setup/config.json")]
    config: String,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long)]
    sensor_id: String,
    #[arg(long)]
    start: String,
    #[arg(long)]
    end: String,
    #[arg(long)]
    preserve_start: Option<String>,
    #[arg(long, default_value_t = false)]
    apply: bool,
    #[arg(long)]
    confirm_sensor_id: Option<String>,
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

fn parse_rfc3339_ts(raw: &str) -> Result<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(raw)
        .with_context(|| format!("invalid RFC3339 timestamp: {raw}"))?;
    Ok(parsed.with_timezone(&Utc))
}

#[derive(sqlx::FromRow)]
struct SensorIdentityRow {
    sensor_id: String,
    sensor_name: String,
    node_id: Uuid,
    node_name: String,
}

async fn load_sensor_identity(pool: &PgPool, sensor_id: &str) -> Result<SensorIdentityRow> {
    let row: Option<SensorIdentityRow> = sqlx::query_as(
        r#"
        SELECT
          sensors.sensor_id as sensor_id,
          sensors.name as sensor_name,
          sensors.node_id as node_id,
          nodes.name as node_name
        FROM sensors
        JOIN nodes ON nodes.id = sensors.node_id
        WHERE sensors.sensor_id = $1
        "#,
    )
    .bind(sensor_id)
    .fetch_optional(pool)
    .await
    .context("failed to query sensor identity")?;

    row.with_context(|| format!("sensor not found: {sensor_id}"))
}

async fn count_metrics_in_window(
    pool: &PgPool,
    sensor_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::bigint
        FROM metrics
        WHERE sensor_id = $1
          AND ts >= $2
          AND ts <= $3
        "#,
    )
    .bind(sensor_id)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await
    .context("failed to count metrics in window")?;
    Ok(count)
}

async fn metrics_min_max_count(
    pool: &PgPool,
    sensor_id: &str,
) -> Result<(i64, Option<DateTime<Utc>>, Option<DateTime<Utc>>)> {
    let (count, min_ts, max_ts): (i64, Option<DateTime<Utc>>, Option<DateTime<Utc>>) =
        sqlx::query_as(
            r#"
            SELECT
              COUNT(*)::bigint,
              MIN(ts),
              MAX(ts)
            FROM metrics
            WHERE sensor_id = $1
            "#,
        )
        .bind(sensor_id)
        .fetch_one(pool)
        .await
        .context("failed to query metrics min/max/count")?;
    Ok((count, min_ts, max_ts))
}

async fn delete_metrics_in_window(
    pool: &PgPool,
    sensor_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<u64> {
    let mut tx = pool.begin().await.context("failed to begin transaction")?;
    let res = sqlx::query(
        r#"
        DELETE FROM metrics
        WHERE sensor_id = $1
          AND ts >= $2
          AND ts <= $3
        "#,
    )
    .bind(sensor_id)
    .bind(start)
    .bind(end)
    .execute(&mut *tx)
    .await
    .context("failed to delete metrics in window")?;

    tx.commit().await.context("failed to commit transaction")?;
    Ok(res.rows_affected())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.apply {
        let confirm = args
            .confirm_sensor_id
            .as_deref()
            .context("--confirm-sensor-id is required with --apply")?;
        anyhow::ensure!(
            confirm == args.sensor_id,
            "--confirm-sensor-id must exactly match --sensor-id"
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

    let start = parse_rfc3339_ts(&args.start)?;
    let end = parse_rfc3339_ts(&args.end)?;
    anyhow::ensure!(end >= start, "end must be >= start");

    let identity = load_sensor_identity(&pool, args.sensor_id.trim()).await?;
    println!(
        "Target sensor: {} ({}) on node: {} ({})",
        identity.sensor_id, identity.sensor_name, identity.node_id, identity.node_name
    );
    println!(
        "Window (UTC): {} → {}",
        start.to_rfc3339(),
        end.to_rfc3339()
    );

    let (total_count, min_ts, max_ts) = metrics_min_max_count(&pool, &identity.sensor_id).await?;
    println!(
        "Metrics total: {} (min_ts={}, max_ts={})",
        total_count,
        min_ts
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "null".to_string()),
        max_ts
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "null".to_string())
    );

    let window_count = count_metrics_in_window(&pool, &identity.sensor_id, start, end).await?;
    println!("Metrics in window: {}", window_count);

    if let Some(preserve_start_raw) = args.preserve_start.as_deref() {
        let preserve_start = parse_rfc3339_ts(preserve_start_raw)?;
        let (count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)::bigint
            FROM metrics
            WHERE sensor_id = $1
              AND ts >= $2
            "#,
        )
        .bind(&identity.sensor_id)
        .bind(preserve_start)
        .fetch_one(&pool)
        .await
        .context("failed to count preserve-window metrics")?;
        println!(
            "Metrics in preserve window (ts >= {}): {}",
            preserve_start.to_rfc3339(),
            count
        );
    }

    if !args.apply {
        println!(
            "Dry run (no delete). Re-run with --apply --confirm-sensor-id <sensor-id> to execute."
        );
        return Ok(());
    }

    let deleted = delete_metrics_in_window(&pool, &identity.sensor_id, start, end).await?;
    println!("Deleted rows: {}", deleted);

    let remaining = count_metrics_in_window(&pool, &identity.sensor_id, start, end).await?;
    println!("Remaining rows in window: {}", remaining);
    anyhow::ensure!(
        remaining == 0,
        "window delete incomplete (remaining rows: {remaining})"
    );

    Ok(())
}
