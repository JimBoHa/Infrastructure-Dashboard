use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use clap::Parser;
use duckdb::Connection;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

const DEFAULT_SETUP_CONFIG_PATH: &str = "/Users/Shared/FarmDashboard/setup/config.json";
const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const DEFAULT_TMP_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/tmp";
const DEFAULT_DATASET: &str = "metrics/v1";

#[derive(Debug, Parser)]
#[command(about = "Generate a synthetic TSSE benchmark dataset (Parquet lake + sensors in DB).")]
struct Args {
    /// Setup config path (used to read database_url when --database-url is not provided).
    #[arg(long, default_value = DEFAULT_SETUP_CONFIG_PATH)]
    config: String,

    /// Postgres connection string (overrides setup config).
    #[arg(long)]
    database_url: Option<String>,

    /// Analysis lake hot path (writes Parquet under <hot>/<dataset>/date=.../shard=...).
    #[arg(long)]
    hot_path: Option<PathBuf>,

    /// Analysis tmp path (used for CSV->Parquet conversion).
    #[arg(long)]
    tmp_path: Option<PathBuf>,

    /// Dataset name (must match core-server METRICS_DATASET_V1 to be readable by jobs).
    #[arg(long, default_value = DEFAULT_DATASET)]
    dataset: String,

    /// Number of shards (must match CORE_ANALYSIS_LAKE_SHARDS).
    #[arg(long, default_value_t = 16)]
    shards: u32,

    /// Number of sensors to generate.
    #[arg(long, default_value_t = 1_000)]
    sensors: u32,

    /// Horizon in days (data generated ending "today" in UTC).
    #[arg(long, default_value_t = 90)]
    horizon_days: u32,

    /// Min sensor interval (seconds).
    #[arg(long, default_value_t = 1)]
    min_interval_seconds: u32,

    /// Max sensor interval (seconds).
    #[arg(long, default_value_t = 30)]
    max_interval_seconds: u32,

    /// Density in [0,1] (fraction of interval buckets that contain points).
    #[arg(long, default_value_t = 0.02)]
    density: f64,

    /// Cap per-sensor total points (effective density is reduced to stay under this cap).
    #[arg(long, default_value_t = 20_000)]
    max_points_per_sensor: u32,

    /// RNG seed for deterministic generation.
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Cluster size for correlated sensors (higher = fewer clusters).
    #[arg(long, default_value_t = 10)]
    cluster_size: u32,

    /// Sensor id prefix (sensor_id becomes "<prefix>-0000", ...).
    #[arg(long, default_value = "bench-sensor")]
    sensor_id_prefix: String,

    /// Sensor type (stored in sensors.type).
    #[arg(long, default_value = "bench")]
    sensor_type: String,

    /// Sensor unit (stored in sensors.unit).
    #[arg(long, default_value = "unit")]
    unit: String,

    /// Node name to create for the bench sensors.
    #[arg(long, default_value = "TSSE Bench Node")]
    node_name: String,

    /// Do not write to Postgres; only generate the Parquet lake.
    #[arg(long)]
    skip_db: bool,

    /// Do not write Parquet; only create the node+sensors in Postgres.
    #[arg(long)]
    skip_parquet: bool,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LakeManifest {
    schema_version: u32,
    #[serde(default)]
    datasets: BTreeMap<String, DatasetManifest>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DatasetManifest {
    #[serde(default)]
    partitions: BTreeMap<String, PartitionManifest>,
    #[serde(default)]
    computed_through_ts: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PartitionManifest {
    location: String,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ReplicationState {
    schema_version: u32,
    #[serde(default)]
    last_inserted_at: Option<String>,
    #[serde(default)]
    computed_through_ts: Option<String>,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn ensure_dir_mode(path: &Path, mode: u32) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("failed to chmod {} to {:o}", path.display(), mode))?;
    }
    Ok(())
}

fn ensure_file_mode(path: &Path, mode: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("failed to chmod {} to {:o}", path.display(), mode))?;
    }
    Ok(())
}

fn escape_single_quotes(input: &str) -> String {
    input.replace('\'', "''")
}

fn write_parquet_from_csv(csv_path: &Path, parquet_path: &Path, tmp_root: &Path) -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let tmp_dir = tmp_root.join("duckdb");
    let _ = ensure_dir_mode(&tmp_dir, 0o700);
    let _ = conn.execute("PRAGMA threads=2", []);
    let _ = conn.execute(
        &format!(
            "SET temp_directory='{}'",
            escape_single_quotes(&tmp_dir.display().to_string())
        ),
        [],
    );

    let csv = escape_single_quotes(&csv_path.display().to_string());
    let out = escape_single_quotes(&parquet_path.display().to_string());
    let sql = format!(
        r#"
        COPY (
            SELECT sensor_id, ts::TIMESTAMP as ts, value::DOUBLE as value, quality::INTEGER as quality
            FROM read_csv('{csv}', columns={{'sensor_id':'VARCHAR','ts':'VARCHAR','value':'DOUBLE','quality':'INTEGER'}}, header=true)
            ORDER BY sensor_id, ts
        ) TO '{out}' (FORMAT PARQUET, COMPRESSION ZSTD);
        "#
    );
    conn.execute(&sql, [])?;
    Ok(())
}

fn read_database_url(args: &Args) -> Result<String> {
    if let Some(url) = args
        .database_url
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Ok(url.to_string());
    }
    let path = PathBuf::from(args.config.trim());
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let cfg: ConfigFile =
        serde_json::from_str(&contents).context("failed to parse setup config JSON")?;
    cfg.database_url
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .context("database_url missing in setup config; pass --database-url explicitly")
}

fn shard_for_sensor_id(sensor_id: &str, shards: u32) -> u32 {
    let shards = shards.max(1);
    (xxh3_64(sensor_id.as_bytes()) % shards as u64) as u32
}

fn dates_ending_today_utc(days: u32) -> Vec<NaiveDate> {
    let days = days.max(1) as i64;
    let today = Utc::now().date_naive();
    (0..days)
        .map(|offset| today - Duration::days(days - 1 - offset))
        .collect()
}

fn synth_value(cluster_id: u32, sensor_idx: u32, t: DateTime<Utc>, noise: f64) -> f64 {
    let seconds = t.timestamp() as f64;
    let day = 24.0 * 3600.0;
    let cluster_phase = (cluster_id as f64) * 0.2;
    let base = ((seconds / day) * std::f64::consts::TAU + cluster_phase).sin();
    let sensor_offset = (sensor_idx as f64) * 0.0005;
    base + sensor_offset + noise
}

fn points_for_day(rng: &mut StdRng, steps_per_day: u32, density: f64) -> Vec<u32> {
    if steps_per_day == 0 {
        return vec![];
    }
    let density = density.clamp(0.0, 1.0);
    if density >= 0.5 {
        return (0..steps_per_day)
            .filter(|_| rng.gen::<f64>() <= density)
            .collect();
    }
    let expected = (steps_per_day as f64 * density).round() as u32;
    let target = expected.min(steps_per_day);
    let mut out = Vec::with_capacity(target as usize);
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < target as usize {
        seen.insert(rng.gen_range(0..steps_per_day));
    }
    out.extend(seen.into_iter());
    out
}

async fn create_bench_node_and_sensors(
    db: &PgPool,
    args: &Args,
    sensor_ids: &[String],
    intervals: &[u32],
) -> Result<()> {
    anyhow::ensure!(
        sensor_ids.len() == intervals.len(),
        "sensor id / interval length mismatch"
    );

    let seed = args.seed;
    let mac_eth = format!(
        "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        (seed >> 0) as u8,
        (seed >> 8) as u8,
        (seed >> 16) as u8,
        (seed >> 24) as u8,
        (seed >> 32) as u8
    );
    let mac_wifi = format!(
        "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        (seed >> 4) as u8,
        (seed >> 12) as u8,
        (seed >> 20) as u8,
        (seed >> 28) as u8,
        (seed >> 36) as u8
    );

    let node_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO nodes (
            name,
            mac_eth,
            mac_wifi,
            ip_last,
            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            last_seen,
            config,
            ui_order
        )
        VALUES (
            $1,
            $2::macaddr,
            $3::macaddr,
            NULL,
            'online',
            0,
            0,
            0,
            now(),
            '{}'::jsonb,
            0
        )
        RETURNING id
        "#,
    )
    .bind(args.node_name.trim())
    .bind(mac_eth)
    .bind(mac_wifi)
    .fetch_one(db)
    .await
    .context("failed to insert bench node")?;

    let mut tx = db.begin().await?;
    for (idx, (sensor_id, interval)) in sensor_ids.iter().zip(intervals.iter()).enumerate() {
        let name = format!("Bench {}", sensor_id);
        let ui_order: i32 = idx as i32;
        let _ = sqlx::query(
            r#"
            INSERT INTO sensors (
                sensor_id,
                node_id,
                name,
                type,
                unit,
                interval_seconds,
                rolling_avg_seconds,
                deleted_at,
                config,
                ui_order
            )
            VALUES ($1, $2, $3, $4, $5, $6, 0, NULL, $7, $8)
            ON CONFLICT (sensor_id) DO UPDATE SET
                node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                rolling_avg_seconds = EXCLUDED.rolling_avg_seconds,
                deleted_at = NULL,
                config = EXCLUDED.config,
                ui_order = EXCLUDED.ui_order
            "#,
        )
        .bind(sensor_id)
        .bind(node_id)
        .bind(name)
        .bind(args.sensor_type.trim())
        .bind(args.unit.trim())
        .bind(*interval as i32)
        .bind(SqlJson(serde_json::json!({})))
        .bind(ui_order)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    println!("Created bench node: {}", node_id);
    Ok(())
}

fn write_manifest_and_state(hot_path: &Path, dataset: &str, dates: &[NaiveDate]) -> Result<()> {
    let state_dir = hot_path.join("_state");
    ensure_dir_mode(&state_dir, 0o750)?;

    let manifest_path = state_dir.join("manifest.json");
    let mut manifest: LakeManifest = if manifest_path.exists() {
        let bytes = fs::read(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        serde_json::from_slice(&bytes).unwrap_or_default()
    } else {
        LakeManifest {
            schema_version: 1,
            datasets: BTreeMap::new(),
        }
    };
    if manifest.schema_version == 0 {
        manifest.schema_version = 1;
    }

    let ds = manifest.datasets.entry(dataset.to_string()).or_default();
    ds.computed_through_ts = Some(Utc::now().to_rfc3339());
    for date in dates {
        let key = date.format("%Y-%m-%d").to_string();
        ds.partitions.insert(
            key,
            PartitionManifest {
                location: "hot".to_string(),
                updated_at: Some(Utc::now().to_rfc3339()),
            },
        );
    }
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    let _ = ensure_file_mode(&manifest_path, 0o600);

    let repl_path = state_dir.join("replication.json");
    let state = ReplicationState {
        schema_version: 1,
        last_inserted_at: Some(Utc::now().to_rfc3339()),
        computed_through_ts: Some(Utc::now().to_rfc3339()),
    };
    fs::write(&repl_path, serde_json::to_vec_pretty(&state)?)?;
    let _ = ensure_file_mode(&repl_path, 0o600);

    Ok(())
}

fn generate_parquet_dataset(
    args: &Args,
    hot_path: &Path,
    tmp_path: &Path,
    sensor_ids: &[String],
    intervals: &[u32],
) -> Result<()> {
    let shards = args.shards.max(1);
    let density = args.density.clamp(0.0, 1.0);
    let cluster_size = args.cluster_size.max(1);
    let dates = dates_ending_today_utc(args.horizon_days);

    ensure_dir_mode(hot_path, 0o750)?;
    ensure_dir_mode(tmp_path, 0o700)?;

    let run_id = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let run_root = tmp_path.join("tsse_bench_dataset").join(&run_id);
    ensure_dir_mode(&run_root, 0o700)?;

    // Pre-group sensors by shard for day+shard CSVs.
    let mut sensors_by_shard: BTreeMap<u32, Vec<(u32, String, u32)>> = BTreeMap::new();
    for (idx, (sensor_id, interval)) in sensor_ids.iter().zip(intervals.iter()).enumerate() {
        let shard = shard_for_sensor_id(sensor_id, shards);
        sensors_by_shard
            .entry(shard)
            .or_default()
            .push((idx as u32, sensor_id.clone(), *interval));
    }

    for date in dates.iter().copied() {
        for shard in 0..shards {
            let sensors = sensors_by_shard.get(&shard).cloned().unwrap_or_default();
            if sensors.is_empty() {
                continue;
            }

            let csv_dir = run_root
                .join(&args.dataset)
                .join(format!("date={}", date.format("%Y-%m-%d")))
                .join(format!("shard={:02}", shard));
            ensure_dir_mode(&csv_dir, 0o700)?;
            let csv_path = csv_dir.join("segment.csv");

            let mut file = fs::File::create(&csv_path)
                .with_context(|| format!("failed to create {}", csv_path.display()))?;
            writeln!(&mut file, "sensor_id,ts,value,quality")?;

            let day_start = Utc
                .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
                .unwrap();

            for (sensor_idx, sensor_id, interval_seconds) in sensors {
                let interval_seconds = interval_seconds.clamp(1, 30);
                let steps_per_day = 86_400u32 / interval_seconds;
                let total_steps = steps_per_day.saturating_mul(args.horizon_days.max(1));
                let mut effective_density = density;
                if total_steps > 0 {
                    let cap = args.max_points_per_sensor.max(3) as f64;
                    effective_density = effective_density.min(cap / (total_steps as f64));
                }

                let seed = args.seed
                    ^ (sensor_idx as u64).wrapping_mul(0x9E3779B97F4A7C15)
                    ^ (date.num_days_from_ce() as u64).wrapping_mul(0xD1B54A32D192ED03);
                let mut rng = StdRng::seed_from_u64(seed);
                let steps = points_for_day(&mut rng, steps_per_day, effective_density);
                let cluster_id = (sensor_idx / cluster_size) as u32;

                for step in steps {
                    let ts = day_start + Duration::seconds((step * interval_seconds) as i64);
                    let noise = rng.gen_range(-0.05..0.05);
                    let value = synth_value(cluster_id, sensor_idx, ts, noise);
                    writeln!(&mut file, "{},{},{},0", sensor_id, ts.to_rfc3339(), value)?;
                }
            }

            file.flush()?;

            let target_dir = hot_path
                .join(&args.dataset)
                .join(format!("date={}", date.format("%Y-%m-%d")))
                .join(format!("shard={:02}", shard));
            ensure_dir_mode(&target_dir, 0o750)?;

            let parquet_final = target_dir.join(format!("part-{}.parquet", run_id));
            let parquet_tmp = target_dir.join(format!("part-{}.parquet.tmp", run_id));
            write_parquet_from_csv(&csv_path, &parquet_tmp, tmp_path).with_context(|| {
                format!(
                    "failed to convert csv {} -> parquet {}",
                    csv_path.display(),
                    parquet_tmp.display()
                )
            })?;
            fs::rename(&parquet_tmp, &parquet_final).with_context(|| {
                format!(
                    "failed to finalize parquet {} -> {}",
                    parquet_tmp.display(),
                    parquet_final.display()
                )
            })?;
        }
    }

    write_manifest_and_state(hot_path, &args.dataset, &dates)?;

    let sensor_list_path = hot_path
        .join("_state")
        .join(format!("tsse_bench_sensors_{}.txt", run_id));
    ensure_dir_mode(sensor_list_path.parent().unwrap(), 0o750)?;
    fs::write(&sensor_list_path, sensor_ids.join("\n"))?;
    let _ = ensure_file_mode(&sensor_list_path, 0o600);

    println!("Wrote Parquet dataset under: {}", hot_path.display());
    println!("Dataset: {}", args.dataset);
    println!("Sensor list: {}", sensor_list_path.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let hot_path = args
        .hot_path
        .clone()
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_HOT_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HOT_PATH));
    let tmp_path = args
        .tmp_path
        .clone()
        .or_else(|| env_path("CORE_ANALYSIS_TMP_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TMP_PATH));

    let sensor_count = args.sensors.max(1);
    let mut sensor_ids = Vec::new();
    for idx in 0..sensor_count {
        sensor_ids.push(format!("{}-{:04}", args.sensor_id_prefix.trim(), idx));
    }

    let min_interval = args.min_interval_seconds.clamp(1, 30);
    let max_interval = args.max_interval_seconds.clamp(min_interval, 30);
    let mut rng = StdRng::seed_from_u64(args.seed ^ 0xA5A5_A5A5_A5A5_A5A5);
    let mut intervals = Vec::new();
    for _ in 0..sensor_count {
        intervals.push(rng.gen_range(min_interval..=max_interval));
    }

    if !args.skip_db {
        let database_url = read_database_url(&args)?;
        let db = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .with_context(|| format!("failed to connect to db at {}", database_url))?;
        create_bench_node_and_sensors(&db, &args, &sensor_ids, &intervals).await?;
    }

    if !args.skip_parquet {
        generate_parquet_dataset(&args, &hot_path, &tmp_path, &sensor_ids, &intervals)?;
    }

    println!("\nNext steps (typical):");
    println!(
        "  1) Start core-server-rs with CORE_ANALYSIS_LAKE_HOT_PATH='{}' CORE_ANALYSIS_TMP_PATH='{}' CORE_ANALYSIS_LAKE_SHARDS='{}'",
        hot_path.display(),
        tmp_path.display(),
        args.shards.max(1)
    );
    println!("  2) Run embeddings build job: POST /api/analysis/jobs {{\"job_type\":\"embeddings_build_v1\",\"params\":{{\"interval_seconds\":60}},\"dedupe\":false}}");
    println!("  3) Run bench harness: cargo run --bin tsse_bench -- --report reports/tsse-bench-YYYYMMDD_HHMM.md --auth-token-file <token> --focus-sensor-id {}-0000", args.sensor_id_prefix.trim());

    Ok(())
}
