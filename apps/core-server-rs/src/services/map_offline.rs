use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use reqwest::blocking::Client as BlockingClient;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::state::AppState;
use url::Url;

const REMOTE_GLYPHS_BASE_URL: &str = "https://demotiles.maplibre.org/font";

#[derive(Debug, Clone)]
struct OfflinePackBounds {
    min_lat: f64,
    min_lng: f64,
    max_lat: f64,
    max_lng: f64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct OfflinePackRow {
    bounds: SqlJson<JsonValue>,
    min_zoom: i32,
    max_zoom: i32,
}

#[derive(Debug, Clone)]
struct TileLayerSpec {
    key: &'static str,
    name: &'static str,
    url_template: &'static str,
    file_name: &'static str,
    max_zoom: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct LayerKeyRow {
    system_key: String,
    id: i64,
}

static ACTIVE_INSTALLS: OnceLock<tokio::sync::Mutex<HashSet<String>>> = OnceLock::new();

fn active_installs() -> &'static tokio::sync::Mutex<HashSet<String>> {
    ACTIVE_INSTALLS.get_or_init(|| tokio::sync::Mutex::new(HashSet::new()))
}

pub fn spawn_install(state: AppState, pack_id: String) {
    tokio::spawn(async move {
        {
            let mut installs = active_installs().lock().await;
            if installs.contains(&pack_id) {
                tracing::info!(pack_id, "offline map pack install already running");
                return;
            }
            installs.insert(pack_id.clone());
        }

        let result = install_pack(&state, &pack_id).await;

        {
            let mut installs = active_installs().lock().await;
            installs.remove(&pack_id);
        }

        if let Err(err) = result {
            let message = format!("{err:#}");
            tracing::error!(pack_id, error = %message, "offline map pack install failed");
            let _ = mark_pack_failed(&state.db, &pack_id, &message).await;
        }
    });
}

pub async fn resume_installing_packs(state: AppState) -> Result<()> {
    let ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM map_offline_packs
        WHERE status = 'installing'
        ORDER BY id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .context("failed to list installing map packs")?;

    if ids.is_empty() {
        return Ok(());
    }

    tracing::info!(count = ids.len(), "resuming offline map pack installs");
    for id in ids {
        spawn_install(state.clone(), id);
    }

    Ok(())
}

async fn install_pack(state: &AppState, pack_id: &str) -> Result<()> {
    let pack = load_pack(&state.db, pack_id)
        .await?
        .ok_or_else(|| anyhow!("offline pack {pack_id} not found"))?;
    let bounds = parse_bounds(&pack.bounds.0)
        .with_context(|| format!("offline pack {pack_id} has invalid bounds"))?;

    let storage = state.config.map_storage_path.clone();
    fs::create_dir_all(&storage)
        .with_context(|| format!("failed to create map storage at {}", storage.display()))?;

    let tiles_dir = storage.join("tiles").join(pack_id);
    fs::create_dir_all(&tiles_dir)
        .with_context(|| format!("failed to create tiles dir {}", tiles_dir.display()))?;

    let glyphs_dir = storage.join("glyphs");
    fs::create_dir_all(&glyphs_dir)
        .with_context(|| format!("failed to create glyphs dir {}", glyphs_dir.display()))?;

    let started_at = Utc::now().to_rfc3339();
    update_progress_note(
        &state.db,
        pack_id,
        serde_json::json!({ "started_at": started_at }),
    )
    .await?;

    // Download glyphs for the default UI font stack used by the dashboard MapLibre layers.
    // Keep this scoped to the ranges we actually need for ASCII labels; additional ranges are fetched lazily.
    let fontstack = "Noto Sans Regular";
    download_glyph_range(&glyphs_dir, fontstack, "0-255.pbf").await?;
    download_glyph_range(&glyphs_dir, fontstack, "256-511.pbf").await?;
    download_glyph_range(&glyphs_dir, fontstack, "512-767.pbf").await?;
    download_glyph_range(&glyphs_dir, fontstack, "768-1023.pbf").await?;

    // Download tiles. This is intentionally conservative (sequential per layer) so it works reliably on the controller.
    let layers = build_tile_layers(pack.max_zoom);
    for layer in layers {
        download_tile_layer(
            &state.db,
            pack_id,
            &tiles_dir,
            &bounds,
            pack.min_zoom,
            layer,
        )
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE map_offline_packs
        SET status = 'installed',
            error = NULL,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(pack_id)
    .execute(&state.db)
    .await?;

    // If the operator installed offline tiles during setup, switch existing map saves/settings to the offline basemap
    // equivalents so the map remains usable after internet is removed.
    if let Err(err) = prefer_offline_baselayers(&state.db).await {
        tracing::warn!(pack_id, error = %err, "failed to switch map saves to offline baselayers");
    }

    Ok(())
}

pub async fn prefer_offline_baselayers(db: &PgPool) -> Result<()> {
    let rows: Vec<LayerKeyRow> = sqlx::query_as(
        r#"
        SELECT system_key, id
        FROM map_layers
        WHERE system_key IN (
            'streets',
            'satellite',
            'topo',
            'offline_streets',
            'offline_satellite',
            'offline_topo'
        )
        "#,
    )
    .fetch_all(db)
    .await
    .context("failed to load map layer ids")?;

    let mut ids: BTreeMap<String, i64> = BTreeMap::new();
    for row in rows {
        ids.insert(row.system_key, row.id);
    }

    let swaps = [
        ("streets", "offline_streets"),
        ("satellite", "offline_satellite"),
        ("topo", "offline_topo"),
    ];

    for (internet_key, offline_key) in swaps {
        let Some(&internet_id) = ids.get(internet_key) else {
            continue;
        };
        let Some(&offline_id) = ids.get(offline_key) else {
            continue;
        };

        sqlx::query(
            "UPDATE map_saves SET active_base_layer_id = $1 WHERE active_base_layer_id = $2",
        )
        .bind(offline_id)
        .bind(internet_id)
        .execute(db)
        .await?;

        sqlx::query(
            "UPDATE map_settings SET active_base_layer_id = $1 WHERE active_base_layer_id = $2",
        )
        .bind(offline_id)
        .bind(internet_id)
        .execute(db)
        .await?;
    }

    Ok(())
}

fn build_tile_layers(pack_max_zoom: i32) -> Vec<TileLayerSpec> {
    vec![
        TileLayerSpec {
            key: "streets",
            name: "Streets",
            url_template: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            file_name: "streets.mbtiles",
            max_zoom: std::cmp::min(pack_max_zoom, 17),
        },
        TileLayerSpec {
            key: "topo",
            name: "Topo",
            url_template: "https://basemap.nationalmap.gov/ArcGIS/rest/services/USGSTopo/MapServer/tile/{z}/{y}/{x}",
            file_name: "topo.mbtiles",
            max_zoom: std::cmp::min(pack_max_zoom, 16),
        },
        TileLayerSpec {
            key: "satellite",
            name: "Satellite",
            url_template:
                "https://services.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            file_name: "satellite.mbtiles",
            max_zoom: std::cmp::min(pack_max_zoom, 18),
        },
        TileLayerSpec {
            key: "terrain",
            name: "Terrain",
            url_template: "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png",
            file_name: "terrain.mbtiles",
            max_zoom: std::cmp::min(pack_max_zoom, 13),
        },
    ]
}

async fn load_pack(db: &PgPool, pack_id: &str) -> Result<Option<OfflinePackRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT bounds, min_zoom, max_zoom
        FROM map_offline_packs
        WHERE id = $1
        "#,
    )
    .bind(pack_id)
    .fetch_optional(db)
    .await
}

async fn mark_pack_failed(db: &PgPool, pack_id: &str, message: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE map_offline_packs
        SET status = 'failed',
            error = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(pack_id)
    .bind(message)
    .execute(db)
    .await?;
    Ok(())
}

async fn update_progress_note(
    db: &PgPool,
    pack_id: &str,
    patch: JsonValue,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE map_offline_packs
        SET progress = progress || $2::jsonb,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(pack_id)
    .bind(SqlJson(patch))
    .execute(db)
    .await?;
    Ok(())
}

async fn update_layer_progress(
    db: &PgPool,
    pack_id: &str,
    layer_key: &str,
    patch: JsonValue,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE map_offline_packs
        SET progress = jsonb_set(
            progress,
            ARRAY['layers', $2],
            COALESCE(progress->'layers'->$2, '{}'::jsonb) || $3::jsonb,
            true
        ),
        updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(pack_id)
    .bind(layer_key)
    .bind(SqlJson(patch))
    .execute(db)
    .await?;
    Ok(())
}

fn parse_bounds(value: &JsonValue) -> Result<OfflinePackBounds> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("bounds must be a JSON object"))?;
    let get = |key: &str| -> Result<f64> {
        obj.get(key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow!("bounds.{key} must be a number"))
    };
    let min_lat = get("min_lat")?;
    let min_lng = get("min_lng")?;
    let max_lat = get("max_lat")?;
    let max_lng = get("max_lng")?;
    if !(min_lat < max_lat) || !(min_lng < max_lng) {
        return Err(anyhow!(
            "bounds must be min<max (got lat {min_lat}..{max_lat}, lng {min_lng}..{max_lng})"
        ));
    }
    Ok(OfflinePackBounds {
        min_lat,
        min_lng,
        max_lat,
        max_lng,
    })
}

async fn download_glyph_range(root: &Path, fontstack: &str, range_file: &str) -> Result<()> {
    if fontstack.contains("..") || fontstack.contains('/') || fontstack.contains('\\') {
        return Err(anyhow!("unsafe fontstack {fontstack}"));
    }
    if range_file.contains("..") || range_file.contains('/') || range_file.contains('\\') {
        return Err(anyhow!("unsafe glyph range {range_file}"));
    }

    let dest_dir = root.join(fontstack);
    fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(range_file);
    if dest.exists() {
        return Ok(());
    }

    let mut url = Url::parse(REMOTE_GLYPHS_BASE_URL).context("invalid glyph base URL")?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("invalid glyph base URL path"))?;
        segments.push(fontstack);
        segments.push(range_file);
    }
    let url = url.to_string();
    tracing::info!(fontstack, range_file, "downloading glyph range");
    let body = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let client = BlockingClient::builder()
            .user_agent("FarmDashboard offline-map-pack/0.1")
            .build()
            .context("failed to build glyph download client")?;
        let resp = client.get(url).send().context("glyph request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("glyph download HTTP {}", resp.status()));
        }
        let bytes = resp.bytes().context("glyph read failed")?;
        Ok(bytes.to_vec())
    })
    .await??;

    fs::write(&dest, &body)?;
    Ok(())
}

async fn download_tile_layer(
    db: &PgPool,
    pack_id: &str,
    tiles_dir: &Path,
    bounds: &OfflinePackBounds,
    min_zoom: i32,
    layer: TileLayerSpec,
) -> Result<()> {
    let mbtiles_path = tiles_dir.join(layer.file_name);
    let db_clone = db.clone();
    let pack_id = pack_id.to_string();
    let bounds = bounds.clone();
    let tiles_dir = tiles_dir.to_path_buf();
    let max_zoom = layer.max_zoom;
    let layer_key = layer.key.to_string();
    let handle = tokio::runtime::Handle::current();

    tokio::task::spawn_blocking(move || -> Result<()> {
        fs::create_dir_all(&tiles_dir)?;
        let conn = Connection::open(&mbtiles_path)
            .with_context(|| format!("failed to open mbtiles {}", mbtiles_path.display()))?;

        init_mbtiles(
            &conn,
            layer.name,
            bounds.min_lng,
            bounds.min_lat,
            bounds.max_lng,
            bounds.max_lat,
            min_zoom,
            max_zoom,
        )
            .context("failed to init mbtiles schema")?;

        let client = BlockingClient::builder()
            .user_agent("FarmDashboard offline-map-pack/0.1")
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build tile download client")?;

        let mut totals_by_zoom: BTreeMap<i32, usize> = BTreeMap::new();
        for z in min_zoom..=max_zoom {
            totals_by_zoom.insert(z, estimate_tiles_for_bounds(&bounds, z));
        }
        let total: usize = totals_by_zoom.values().sum();

        let started = Instant::now();
        let mut downloaded: usize = 0;
        let mut failures: usize = 0;
        let mut last_progress = Instant::now();

        let mut exists_stmt = conn.prepare_cached(
            "SELECT 1 FROM tiles WHERE zoom_level = ?1 AND tile_column = ?2 AND tile_row = ?3 LIMIT 1",
        )?;

        for z in min_zoom..=max_zoom {
            let (x_min, x_max, y_min, y_max) = tile_range(&bounds, z);
            for x in x_min..=x_max {
                for y in y_min..=y_max {
                    let y_tms = (1_i64 << z) as i64 - 1 - y as i64;
                    let already_present: Option<i32> = exists_stmt
                        .query_row(params![z, x, y_tms], |row| row.get(0))
                        .optional()?;
                    if already_present.is_some() {
                        downloaded += 1;
                        continue;
                    }

                    let url = layer
                        .url_template
                        .replace("{z}", &z.to_string())
                        .replace("{x}", &x.to_string())
                        .replace("{y}", &y.to_string());
                    let resp = client.get(&url).send();
                    match resp {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                if let Ok(bytes) = resp.bytes() {
                                    insert_mbtiles_tile(&conn, z, x, y, &bytes)?;
                                } else {
                                    failures += 1;
                                }
                            } else {
                                failures += 1;
                            }
                        }
                        Err(err) => {
                            tracing::warn!(layer = layer.key, z, x, y, error = %err, "tile fetch failed");
                            failures += 1;
                        }
                    }
                    downloaded += 1;

                    if last_progress.elapsed() > Duration::from_secs(2) {
                        let rate = downloaded as f64 / started.elapsed().as_secs_f64().max(1.0);
                        let progress = serde_json::json!({
                            "downloaded": downloaded,
                            "total": total,
                            "failed": failures,
                            "zoom_min": min_zoom,
                            "zoom_max": max_zoom,
                            "tiles_per_sec": rate
                        });
                        let _ = handle.block_on(async {
                            let _ = update_layer_progress(&db_clone, &pack_id, &layer_key, progress).await;
                        });
                        last_progress = Instant::now();
                    }
                }
            }
        }

        let _ = handle.block_on(async {
            let _ = update_layer_progress(
                &db_clone,
                &pack_id,
                &layer_key,
                serde_json::json!({
                    "downloaded": downloaded,
                    "total": total,
                    "failed": failures,
                    "completed_at": Utc::now().to_rfc3339(),
                }),
            )
            .await;
        });

        Ok(())
    })
    .await??;

    Ok(())
}

fn init_mbtiles(
    conn: &Connection,
    name: &str,
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
    min_zoom: i32,
    max_zoom: i32,
) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS metadata (name TEXT PRIMARY KEY, value TEXT);
        CREATE TABLE IF NOT EXISTS tiles (
            zoom_level INTEGER,
            tile_column INTEGER,
            tile_row INTEGER,
            tile_data BLOB,
            PRIMARY KEY (zoom_level, tile_column, tile_row)
        );
        "#,
    )?;

    conn.execute(
        "INSERT OR REPLACE INTO metadata (name, value) VALUES ('name', ?1)",
        params![name],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (name, value) VALUES ('bounds', ?1)",
        params![format!("{min_lng},{min_lat},{max_lng},{max_lat}")],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (name, value) VALUES ('minzoom', ?1)",
        params![min_zoom.to_string()],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (name, value) VALUES ('maxzoom', ?1)",
        params![max_zoom.to_string()],
    )?;
    Ok(())
}

fn insert_mbtiles_tile(conn: &Connection, z: i32, x: i32, y_xyz: i32, data: &[u8]) -> Result<()> {
    let y_tms = (1_i64 << z) as i64 - 1 - y_xyz as i64;
    conn.execute(
        "INSERT OR REPLACE INTO tiles (zoom_level, tile_column, tile_row, tile_data) VALUES (?1, ?2, ?3, ?4)",
        params![z, x, y_tms, data],
    )?;
    Ok(())
}

fn estimate_tiles_for_bounds(bounds: &OfflinePackBounds, z: i32) -> usize {
    let (x_min, x_max, y_min, y_max) = tile_range(bounds, z);
    if x_max < x_min || y_max < y_min {
        return 0;
    }
    ((x_max - x_min + 1) as usize) * ((y_max - y_min + 1) as usize)
}

fn tile_range(bounds: &OfflinePackBounds, z: i32) -> (i32, i32, i32, i32) {
    let n = 2_f64.powi(z);
    let x_min = lng_to_x(bounds.min_lng, n);
    let x_max = lng_to_x(bounds.max_lng, n);
    let y_min = lat_to_y(bounds.max_lat, n);
    let y_max = lat_to_y(bounds.min_lat, n);
    (
        x_min.min(x_max),
        x_min.max(x_max),
        y_min.min(y_max),
        y_min.max(y_max),
    )
}

fn lng_to_x(lng: f64, n: f64) -> i32 {
    let x = ((lng + 180.0) / 360.0) * n;
    x.floor().clamp(0.0, n - 1.0) as i32
}

fn lat_to_y(lat: f64, n: f64) -> i32 {
    let lat_rad = lat.to_radians();
    let y = (1.0 - (lat_rad.tan() + (1.0 / lat_rad.cos())).ln() / std::f64::consts::PI) / 2.0 * n;
    y.floor().clamp(0.0, n - 1.0) as i32
}
