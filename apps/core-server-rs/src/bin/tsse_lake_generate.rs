use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use clap::Parser;
use duckdb::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const METRICS_DATASET_V1: &str = "metrics/v1";

#[derive(Debug, Parser)]
#[command(about = "Generate a synthetic TSSE Parquet lake dataset (for benchmarks).")]
struct Args {
    /// Hot lake root (contains metrics/v1/date=YYYY-MM-DD/shard=NN/*.parquet)
    #[arg(long)]
    hot_path: Option<PathBuf>,

    /// Dataset root under the lake (default: metrics/v1)
    #[arg(long, default_value = METRICS_DATASET_V1)]
    dataset: String,

    /// Number of sensors to generate.
    #[arg(long, default_value_t = 1_000)]
    sensors: usize,

    /// Number of shards (must match CORE_ANALYSIS_LAKE_SHARDS when benchmarking the TSSE code).
    #[arg(long, default_value_t = 16)]
    shards: u32,

    /// Horizon length (days). Default: 90 (TSSE hot horizon).
    #[arg(long, default_value_t = 90)]
    horizon_days: u32,

    /// Sampling interval in seconds (controls dataset density).
    #[arg(long, default_value_t = 30)]
    interval_seconds: u32,

    /// End date (inclusive) for generated partitions (YYYY-MM-DD). Default: today (UTC).
    #[arg(long)]
    end_date: Option<String>,

    /// Overwrite existing shard-day parquet files.
    #[arg(long)]
    force: bool,
}

fn escape_single_quotes(input: String) -> String {
    input.replace('\'', "''")
}

fn parse_date(label: &str, value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .with_context(|| format!("invalid {} (expected YYYY-MM-DD): {}", label, value))
}

fn shard_for_sensor_id(sensor_id: &str, shards: u32) -> u32 {
    let shards = shards.max(1);
    (xxh3_64(sensor_id.as_bytes()) % shards as u64) as u32
}

fn partition_dir(root: &Path, dataset: &str, date: NaiveDate, shard: u32) -> PathBuf {
    root.join(dataset)
        .join(format!(
            "date={:04}-{:02}-{:02}",
            date.year(),
            date.month(),
            date.day()
        ))
        .join(format!("shard={:02}", shard))
}

fn main() -> Result<()> {
    let args = Args::parse();

    let hot_path = args
        .hot_path
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HOT_PATH));

    anyhow::ensure!(args.interval_seconds > 0, "interval_seconds must be > 0");
    anyhow::ensure!(args.horizon_days > 0, "horizon_days must be > 0");

    let end_date = if let Some(raw) = args.end_date.as_deref() {
        parse_date("end_date", raw)?
    } else {
        Utc::now().date_naive()
    };
    let start_date = end_date - Duration::days((args.horizon_days - 1) as i64);

    let shards = args.shards.max(1);
    let mut sensor_ids_by_shard: Vec<Vec<String>> = vec![Vec::new(); shards as usize];
    for idx in 0..args.sensors {
        let sensor_id = format!("sensor-{:06}", idx);
        let shard = shard_for_sensor_id(&sensor_id, shards) as usize;
        sensor_ids_by_shard[shard].push(sensor_id);
    }
    for shard_list in sensor_ids_by_shard.iter_mut() {
        shard_list.sort();
    }

    fs::create_dir_all(&hot_path)
        .with_context(|| format!("failed to create {}", hot_path.display()))?;

    let conn = Connection::open_in_memory()?;
    conn.execute(
        "CREATE TABLE sensors(sensor_id VARCHAR, shard UINTEGER)",
        [],
    )?;
    for (shard_idx, list) in sensor_ids_by_shard.iter().enumerate() {
        for sensor_id in list {
            conn.execute(
                &format!(
                    "INSERT INTO sensors VALUES ('{}', {})",
                    escape_single_quotes(sensor_id.clone()),
                    shard_idx
                ),
                [],
            )?;
        }
    }

    let run_id = format!("gen-{}", Utc::now().format("%Y%m%d_%H%M%S").to_string());

    let mut date = start_date;
    loop {
        for shard in 0..shards {
            let dir = partition_dir(&hot_path, &args.dataset, date, shard);
            fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;

            let parquet_path = dir.join(format!("part-{}.parquet", run_id));
            if parquet_path.exists() && !args.force {
                continue;
            }
            if parquet_path.exists() && args.force {
                let _ = fs::remove_file(&parquet_path);
            }

            // Generate deterministic synthetic data via DuckDB for this shard-day.
            //
            // Schema matches TSSE lake reads:
            // - sensor_id (VARCHAR)
            // - ts (TIMESTAMP)
            // - value (DOUBLE)
            // - quality (INTEGER)
            //
            // Note: this generator is intended for benchmarks, not production fidelity.
            let day_start = format!(
                "{:04}-{:02}-{:02} 00:00:00",
                date.year(),
                date.month(),
                date.day()
            );
            let interval_seconds = args.interval_seconds;

            let sql = format!(
                r#"
                COPY (
                  WITH t AS (
                    SELECT CAST(sec AS BIGINT) AS sec
                    FROM range(0, 86400, {interval_seconds})
                  )
                  SELECT
                    s.sensor_id,
                    (TIMESTAMP '{day_start}' + sec * INTERVAL '1 second') AS ts,
                    (
                      (1.0 + (abs(hash(s.sensor_id)) % 500)::DOUBLE / 100.0)
                      * sin(2*pi() * sec::DOUBLE / (300 + (abs(hash(s.sensor_id)) % 7200))::DOUBLE + (abs(hash(s.sensor_id)) % 6283)::DOUBLE / 1000.0)
                      + (((abs(hash(s.sensor_id || ':' || sec::VARCHAR)) % 2000)::DOUBLE) - 1000.0) / 10000.0)
                    ) AS value,
                    0 AS quality
                  FROM sensors s
                  CROSS JOIN t
                  WHERE s.shard = {shard}
                  ORDER BY sensor_id, ts
                ) TO '{parquet_path}' (FORMAT PARQUET)
                "#,
                interval_seconds = interval_seconds,
                day_start = day_start,
                shard = shard,
                parquet_path = escape_single_quotes(parquet_path.display().to_string()),
            );

            conn.execute(&sql, [])
                .with_context(|| format!("failed to write {}", parquet_path.display()))?;
        }
        if date == end_date {
            break;
        }
        date = date.succ_opt().context("failed to increment date cursor")?;
    }

    println!(
        "tsse_lake_generate: wrote synthetic lake under {} (dataset {}, sensors {}, shards {}, horizon {}d, interval {}s)",
        hot_path.display(),
        args.dataset,
        args.sensors,
        shards,
        args.horizon_days,
        args.interval_seconds
    );
    Ok(())
}
