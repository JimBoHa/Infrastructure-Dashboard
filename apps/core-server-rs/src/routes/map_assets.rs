use axum::extract::Path;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Router;
use reqwest::blocking::Client as BlockingClient;
use rusqlite::OptionalExtension;
use std::path::Path as FsPath;
use url::Url;

use crate::state::AppState;

const REMOTE_GLYPHS_BASE_URL: &str = "https://demotiles.maplibre.org/font";

fn safe_segment(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
        return None;
    }
    Some(trimmed)
}

#[utoipa::path(
    get,
    path = "/api/map/tiles/{pack}/{layer}/{z}/{x}/{y}",
    tag = "map",
    params(
        ("pack" = String, Path, description = "Offline pack id"),
        ("layer" = String, Path, description = "Layer key (streets/topo/satellite/terrain)"),
        ("z" = u32, Path, description = "Zoom"),
        ("x" = u32, Path, description = "Tile x"),
        ("y" = u32, Path, description = "Tile y")
    ),
    responses(
        (status = 200, description = "Tile bytes"),
        (status = 404, description = "Tile not found")
    )
)]
pub(crate) async fn get_offline_tile(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((pack, layer, z, x, y)): Path<(String, String, u32, u32, u32)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let pack =
        safe_segment(&pack).ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid pack".to_string()))?;
    let layer = safe_segment(&layer)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid layer".to_string()))?;

    let mbtiles = state
        .config
        .map_storage_path
        .join("tiles")
        .join(pack)
        .join(format!("{layer}.mbtiles"));

    let bytes = tokio::task::spawn_blocking(move || read_mbtiles_tile(&mbtiles, z, x, y))
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Tile not found".to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        CACHE_CONTROL,
        "public, max-age=31536000, immutable".parse().unwrap(),
    );
    headers.insert(
        CONTENT_TYPE,
        detect_content_type(&bytes)
            .parse()
            .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
    );

    Ok((headers, bytes))
}

#[utoipa::path(
    get,
    path = "/api/map/glyphs/{fontstack}/{range}",
    tag = "map",
    params(
        ("fontstack" = String, Path, description = "Font stack name"),
        ("range" = String, Path, description = "Unicode range filename (e.g. 0-255)")
    ),
    responses(
        (status = 200, description = "Glyph PBF bytes"),
        (status = 404, description = "Glyph not found")
    )
)]
pub(crate) async fn get_glyph_range(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((fontstack, range)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let fontstack = safe_segment(&fontstack)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid fontstack".to_string()))?
        .to_string();
    let range = safe_segment(&range)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid range".to_string()))?
        .to_string();

    let glyph_path = state
        .config
        .map_storage_path
        .join("glyphs")
        .join(&fontstack)
        .join(format!("{range}.pbf"));

    let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        if let Ok(bytes) = std::fs::read(&glyph_path) {
            return Ok(bytes);
        }

        // Best-effort: if the glyph isn't present yet, fetch it once (while internet is available)
        // and cache it under CORE_MAP_STORAGE_PATH so future runs remain offline.
        let mut url = Url::parse(REMOTE_GLYPHS_BASE_URL)?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("invalid glyph base URL path"))?;
            segments.push(&fontstack);
            segments.push(&format!("{range}.pbf"));
        }
        let url = url.to_string();

        if let Some(parent) = glyph_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let client = BlockingClient::builder()
            .user_agent("FarmDashboard glyph-cache/0.1")
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        let resp = client.get(url).send()?;
        if !resp.status().is_success() {
            anyhow::bail!("glyph download HTTP {}", resp.status());
        }
        let bytes = resp.bytes()?;
        std::fs::write(&glyph_path, &bytes)?;
        Ok(bytes.to_vec())
    })
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|_| (StatusCode::NOT_FOUND, "Glyph not found".to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        CACHE_CONTROL,
        "public, max-age=31536000, immutable".parse().unwrap(),
    );
    headers.insert(CONTENT_TYPE, "application/x-protobuf".parse().unwrap());
    Ok((headers, bytes))
}

fn read_mbtiles_tile(path: &FsPath, z: u32, x: u32, y_xyz: u32) -> anyhow::Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Ok(None);
    }
    let z_i32 = i32::try_from(z).unwrap_or(-1);
    let x_i32 = i32::try_from(x).unwrap_or(-1);
    let y_i32 = i32::try_from(y_xyz).unwrap_or(-1);
    if z_i32 < 0 || x_i32 < 0 || y_i32 < 0 {
        return Ok(None);
    }

    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let y_tms = (1_i64 << z_i32) as i64 - 1 - y_i32 as i64;
    let mut stmt = conn.prepare_cached(
        "SELECT tile_data FROM tiles WHERE zoom_level = ?1 AND tile_column = ?2 AND tile_row = ?3",
    )?;
    let row: Option<Vec<u8>> = stmt
        .query_row(rusqlite::params![z_i32, x_i32, y_tms], |row| row.get(0))
        .optional()?;
    Ok(row)
}

fn detect_content_type(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 4 && bytes[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        return "image/png";
    }
    if bytes.len() >= 2 && bytes[0..2] == [0xFF, 0xD8] {
        return "image/jpeg";
    }
    "application/octet-stream"
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/map/tiles/{pack}/{layer}/{z}/{x}/{y}",
            axum::routing::get(get_offline_tile),
        )
        .route(
            "/map/glyphs/{fontstack}/{range}",
            axum::routing::get(get_glyph_range),
        )
}
