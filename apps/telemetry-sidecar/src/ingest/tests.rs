use super::TelemetryIngestor;
use crate::pipeline::{spawn_worker, BatchCommand, IngestStats, PipelineHandle};
use crate::telemetry::MetricRow;
use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::ingest::ingestor::node_health_sensor_id;

async fn setup_test_pool(database_url: &str, schema: &str) -> Result<PgPool> {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema))
        .execute(&admin_pool)
        .await?;
    drop(admin_pool);

    let schema_name = schema.to_string();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |conn, _meta| {
            let schema = schema_name.clone();
            Box::pin(async move {
                sqlx::query(&format!("SET search_path TO {}", schema))
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS nodes (
            id uuid primary key,
            status text not null,
            last_seen timestamptz null,
            uptime_seconds bigint default 0,
            cpu_percent real default 0,
            storage_used_bytes bigint default 0,
            memory_used_bytes bigint default 0,
            ping_ms double precision null,
            ping_p50_30m_ms double precision null,
            ping_jitter_ms double precision null,
            mqtt_broker_rtt_ms double precision null,
            mqtt_broker_rtt_jitter_ms double precision null,
            network_latency_ms double precision null,
            network_jitter_ms double precision null,
            uptime_percent_24h real null,
            config jsonb default '{}'::jsonb
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sensors (
            sensor_id varchar(24) primary key,
            node_id uuid not null,
            name text not null default '',
            type text not null default '',
            unit text not null default '',
            interval_seconds int not null,
            rolling_avg_seconds int null,
            config jsonb null,
            deleted_at timestamptz null
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS metrics (
            sensor_id varchar(24) not null,
            ts timestamptz not null,
            value double precision not null,
            quality smallint not null,
            primary key (sensor_id, ts)
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alarms (
            id serial primary key,
            name text not null,
            sensor_id varchar(24) null,
            node_id uuid null,
            rule jsonb null,
            status text not null,
            last_fired timestamptz null
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alarm_events (
            id serial primary key,
            alarm_id int not null,
            sensor_id varchar(24) null,
            node_id uuid null,
            status text not null,
            message text null,
            origin text null,
            created_at timestamptz not null default now()
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

#[tokio::test]
async fn test_sidecar_ingest_offline_alarm() -> Result<()> {
    if env::var("SIDECAR_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return Ok(());
    }
    let database_url = match env::var("SIDECAR_TEST_DATABASE_URL") {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };

    let schema = format!("sidecar_test_{}", std::process::id());
    let pool = setup_test_pool(&database_url, &schema).await?;

    let node_id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    let sensor_id = "feedfeed0000000000000001";
    sqlx::query("INSERT INTO nodes (id, status, last_seen) VALUES ($1::uuid, $2, $3)")
        .bind(node_id)
        .bind("online")
        .bind(Utc::now())
        .execute(&pool)
        .await?;
    sqlx::query(
        "INSERT INTO sensors (sensor_id, node_id, interval_seconds, rolling_avg_seconds, config) VALUES ($1, $2::uuid, $3, $4, '{}'::jsonb)",
    )
    .bind(sensor_id)
    .bind(node_id)
    .bind(60)
    .bind(0)
    .execute(&pool)
    .await?;

    let stats = Arc::new(IngestStats::new());
    let (tx, rx) = mpsc::channel::<BatchCommand>(8);
    let pipeline = PipelineHandle::new(tx, stats.clone());
    let _worker = spawn_worker(
        pool.clone(),
        rx,
        stats.clone(),
        5,
        Duration::from_millis(25),
        None,
    );
    let ingestor = TelemetryIngestor::new(
        pool.clone(),
        pipeline,
        std::time::Duration::from_secs(0),
        None,
        None,
    );

    ingestor
        .ingest_metric(MetricRow {
            sensor_id: sensor_id.to_string(),
            timestamp: Utc::now(),
            value: 12.0,
            quality: 0,
            source: None,
            seq: None,
            stream_id: None,
            backfill: false,
        })
        .await?;
    ingestor.flush().await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metrics WHERE sensor_id = $1")
        .bind(sensor_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 1);

    ingestor.check_offline().await?;

    let sensor_status: Option<String> =
        sqlx::query_scalar("SELECT config->>'status' FROM sensors WHERE sensor_id = $1")
            .bind(sensor_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(sensor_status.as_deref(), Some("offline"));

    let node_status: String = sqlx::query_scalar("SELECT status FROM nodes WHERE id = $1::uuid")
        .bind(node_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(node_status, "offline");

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM alarm_events WHERE status = 'firing'")
            .fetch_one(&pool)
            .await?;
    assert!(event_count > 0);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema))
        .execute(&admin_pool)
        .await;

    Ok(())
}

#[tokio::test]
async fn test_node_health_status_persists_metrics() -> Result<()> {
    if env::var("SIDECAR_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return Ok(());
    }
    let database_url = match env::var("SIDECAR_TEST_DATABASE_URL") {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    let schema = format!("sidecar_test_health_{}", std::process::id());
    let pool = setup_test_pool(&database_url, &schema).await?;

    let node_id = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
    sqlx::query("INSERT INTO nodes (id, status, last_seen) VALUES ($1::uuid, $2, $3)")
        .bind(node_id)
        .bind("online")
        .bind(Utc::now())
        .execute(&pool)
        .await?;

    let stats = Arc::new(IngestStats::new());
    let (tx, rx) = mpsc::channel::<BatchCommand>(8);
    let pipeline = PipelineHandle::new(tx, stats.clone());
    let _worker = spawn_worker(
        pool.clone(),
        rx,
        stats.clone(),
        5,
        Duration::from_millis(25),
        None,
    );
    let ingestor = TelemetryIngestor::new(
        pool.clone(),
        pipeline,
        std::time::Duration::from_secs(0),
        None,
        None,
    );

    let timestamp = Utc::now();
    ingestor
        .handle_node_status_payload(
            node_id,
            "online",
            timestamp,
            Some(timestamp),
            Some(120),
            Some(12.5),
            Some(2_048_000),
            Some(5.0),
            None,
            Some(vec![10.0, 25.0]),
            Some(42.0),
            Some((512 * 1024 * 1024) as i64),
            Some(15.0),
            Some(14.5),
            Some(3.5),
            Some(18.0),
            Some(2.5),
            Some(98.0),
            None,
            None,
            None,
        )
        .await?;
    ingestor.flush().await?;

    let memory_sensor = node_health_sensor_id(node_id, "memory_used_bytes");
    let mem_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metrics WHERE sensor_id = $1")
        .bind(&memory_sensor)
        .fetch_one(&pool)
        .await?;
    assert!(mem_count > 0);

    let jitter_sensor = node_health_sensor_id(node_id, "ping_jitter_ms");
    let jitter_value: f64 =
        sqlx::query_scalar("SELECT value FROM metrics WHERE sensor_id = $1 LIMIT 1")
            .bind(&jitter_sensor)
            .fetch_one(&pool)
            .await?;
    assert!((jitter_value - 3.5).abs() < f64::EPSILON);

    let node_row: (
        Option<i64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f32>,
    ) = sqlx::query_as(
        "SELECT memory_used_bytes, ping_ms, ping_jitter_ms, mqtt_broker_rtt_ms, mqtt_broker_rtt_jitter_ms, uptime_percent_24h FROM nodes WHERE id = $1::uuid",
    )
    .bind(node_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(node_row.0, Some(512 * 1024 * 1024));
    assert_eq!(node_row.1, Some(15.0));
    assert_eq!(node_row.2, Some(3.5));
    assert_eq!(node_row.3, Some(18.0));
    assert_eq!(node_row.4, Some(2.5));
    assert_eq!(node_row.5, Some(98.0));

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema))
        .execute(&admin_pool)
        .await;

    Ok(())
}
