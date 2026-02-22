use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::sensor_visibility;
use crate::state::AppState;

const DEFAULT_ZOOM: f64 = 16.0;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MapSettingsResponse {
    active_save_id: i64,
    active_save_name: String,
    active_base_layer_id: Option<i64>,
    center_lat: f64,
    center_lng: f64,
    zoom: f64,
    bearing: f64,
    pitch: f64,
    updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapSettingsUpdateRequest {
    active_base_layer_id: Option<i64>,
    center_lat: f64,
    center_lng: f64,
    zoom: f64,
    bearing: Option<f64>,
    pitch: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MapSaveResponse {
    id: i64,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapSaveCreateRequest {
    name: String,
    active_base_layer_id: Option<i64>,
    center_lat: Option<f64>,
    center_lng: Option<f64>,
    zoom: Option<f64>,
    bearing: Option<f64>,
    pitch: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MapLayerResponse {
    id: i64,
    system_key: Option<String>,
    name: String,
    kind: String,
    source_type: String,
    config: JsonValue,
    opacity: f64,
    enabled: bool,
    z_index: i32,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapLayerUpsertRequest {
    name: String,
    kind: String,
    source_type: String,
    config: JsonValue,
    opacity: Option<f64>,
    enabled: Option<bool>,
    z_index: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MapFeatureResponse {
    id: i64,
    node_id: Option<String>,
    sensor_id: Option<String>,
    geometry: JsonValue,
    properties: JsonValue,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapFeatureUpsertRequest {
    node_id: Option<String>,
    sensor_id: Option<String>,
    geometry: JsonValue,
    properties: Option<JsonValue>,
}

#[derive(sqlx::FromRow)]
struct MapLayerRow {
    id: i64,
    system_key: Option<String>,
    name: String,
    kind: String,
    source_type: String,
    config: SqlJson<JsonValue>,
    opacity: f64,
    enabled: bool,
    z_index: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<MapLayerRow> for MapLayerResponse {
    fn from(row: MapLayerRow) -> Self {
        Self {
            id: row.id,
            system_key: row.system_key,
            name: row.name,
            kind: row.kind,
            source_type: row.source_type,
            config: row.config.0,
            opacity: row.opacity,
            enabled: row.enabled,
            z_index: row.z_index,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct MapFeatureRow {
    id: i64,
    node_id: Option<Uuid>,
    sensor_id: Option<String>,
    geometry: SqlJson<JsonValue>,
    properties: SqlJson<JsonValue>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    sensor_config: SqlJson<JsonValue>,
    sensor_node_config: SqlJson<JsonValue>,
}

impl From<MapFeatureRow> for MapFeatureResponse {
    fn from(row: MapFeatureRow) -> Self {
        Self {
            id: row.id,
            node_id: row.node_id.map(|id| id.to_string()),
            sensor_id: row.sensor_id,
            geometry: row.geometry.0,
            properties: row.properties.0,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct MapSettingsPointerRow {
    active_save_id: Option<i64>,
    active_base_layer_id: Option<i64>,
    center_lat: Option<f64>,
    center_lng: Option<f64>,
    zoom: Option<f64>,
    bearing: Option<f64>,
    pitch: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct MapSaveRow {
    id: i64,
    name: String,
    active_base_layer_id: Option<i64>,
    center_lat: Option<f64>,
    center_lng: Option<f64>,
    zoom: Option<f64>,
    bearing: Option<f64>,
    pitch: Option<f64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn ensure_default_layers(db: &PgPool) -> Result<(), sqlx::Error> {
    let streets = serde_json::json!({
        "url_template": "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
        "tile_size": 256,
        "max_zoom": 19,
        "attribution": "© OpenStreetMap contributors",
        "requires_internet": true
    });
    let satellite = serde_json::json!({
        "url_template": "https://services.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        "tile_size": 256,
        "max_zoom": 23,
        "attribution": "© Esri",
        "requires_internet": true
    });
    let topo = serde_json::json!({
        "url_template": "https://basemap.nationalmap.gov/ArcGIS/rest/services/USGSTopo/MapServer/tile/{z}/{y}/{x}",
        "tile_size": 256,
        "max_zoom": 16,
        "attribution": "USGS",
        "requires_internet": true
    });

    let offline_pack = "swanton_ca";
    let offline_streets = serde_json::json!({
        "url_template": format!("/api/map/tiles/{offline_pack}/streets/{{z}}/{{x}}/{{y}}"),
        "tile_size": 256,
        "max_zoom": 17,
        "attribution": "Offline map pack (Swanton, CA)",
        "offline_pack_id": offline_pack
    });
    let offline_satellite = serde_json::json!({
        "url_template": format!("/api/map/tiles/{offline_pack}/satellite/{{z}}/{{x}}/{{y}}"),
        "tile_size": 256,
        "max_zoom": 18,
        "attribution": "Offline map pack (Swanton, CA)",
        "offline_pack_id": offline_pack
    });
    let offline_topo = serde_json::json!({
        "url_template": format!("/api/map/tiles/{offline_pack}/topo/{{z}}/{{x}}/{{y}}"),
        "tile_size": 256,
        "max_zoom": 16,
        "attribution": "Offline map pack (Swanton, CA)",
        "offline_pack_id": offline_pack
    });
    let offline_terrain = serde_json::json!({
        "url_template": format!("/api/map/tiles/{offline_pack}/terrain/{{z}}/{{x}}/{{y}}"),
        "tile_size": 256,
        "max_zoom": 13,
        "encoding": "terrarium",
        "attribution": "Offline terrain (Swanton, CA)",
        "offline_pack_id": offline_pack
    });

    let _ = sqlx::query(
        r#"
        INSERT INTO map_layers (system_key, name, kind, source_type, config, opacity, enabled, z_index, created_at, updated_at)
        VALUES
            ('streets', 'Streets (OpenStreetMap)', 'base', 'xyz', $1::jsonb, 1.0, true, 0, NOW(), NOW()),
            ('satellite', 'Satellite (Esri World Imagery)', 'base', 'xyz', $2::jsonb, 1.0, true, 0, NOW(), NOW()),
            ('topo', 'Topo (USGS)', 'base', 'xyz', $3::jsonb, 1.0, true, 0, NOW(), NOW()),
            ('offline_streets', 'Streets (Offline pack)', 'base', 'xyz', $4::jsonb, 1.0, true, 0, NOW(), NOW()),
            ('offline_satellite', 'Satellite (Offline pack)', 'base', 'xyz', $5::jsonb, 1.0, true, 0, NOW(), NOW()),
            ('offline_topo', 'Topo (Offline pack)', 'base', 'xyz', $6::jsonb, 1.0, true, 0, NOW(), NOW()),
            ('offline_terrain', 'Terrain hillshade (Offline pack)', 'overlay', 'terrain', $7::jsonb, 0.7, false, 50, NOW(), NOW())
        ON CONFLICT (system_key) DO NOTHING
        "#,
    )
    .bind(SqlJson(streets))
    .bind(SqlJson(satellite))
    .bind(SqlJson(topo))
    .bind(SqlJson(offline_streets))
    .bind(SqlJson(offline_satellite))
    .bind(SqlJson(offline_topo))
    .bind(SqlJson(offline_terrain))
    .execute(db)
    .await?;

    // Keep offline-pack layer zoom caps aligned with the pack definition so the UI doesn't request tiles
    // the pack never downloads (prevents "blank at max zoom" surprises after upgrades).
    let _ = sqlx::query(
        r#"
        UPDATE map_layers
        SET config = config || jsonb_build_object(
            'max_zoom',
            CASE
                WHEN system_key = 'offline_streets' THEN 17
                WHEN system_key = 'offline_satellite' THEN 18
                WHEN system_key = 'offline_topo' THEN 16
                WHEN system_key = 'offline_terrain' THEN 13
                ELSE COALESCE((config->>'max_zoom')::int, 18)
            END
        ),
            updated_at = NOW()
        WHERE system_key IN ('offline_streets', 'offline_satellite', 'offline_topo', 'offline_terrain')
        "#,
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn default_center_from_weather_config(db: &PgPool) -> Option<(f64, f64)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        metadata: SqlJson<JsonValue>,
    }

    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT metadata
        FROM setup_credentials
        WHERE name = 'weather_forecast'
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()?;

    let meta = row?.metadata.0;
    let lat = meta.get("latitude").and_then(|v| v.as_f64())?;
    let lng = meta.get("longitude").and_then(|v| v.as_f64())?;
    Some((lat, lng))
}

async fn default_base_layer_id(db: &PgPool) -> Option<i64> {
    let offline_installed: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT status = 'installed'
        FROM map_offline_packs
        WHERE id = 'swanton_ca'
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if offline_installed.unwrap_or(false) {
        let id: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT id
            FROM map_layers
            WHERE system_key = 'offline_satellite'
            LIMIT 1
            "#,
        )
        .fetch_optional(db)
        .await
        .ok()
        .flatten();
        if id.is_some() {
            return id;
        }
    }

    sqlx::query_scalar(
        r#"
        SELECT id
        FROM map_layers
        WHERE system_key = 'streets'
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

async fn load_save_by_id(db: &PgPool, id: i64) -> Result<Option<MapSaveRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            active_base_layer_id,
            center_lat,
            center_lng,
            zoom,
            bearing,
            pitch,
            created_at,
            updated_at
        FROM map_saves
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

async fn ensure_active_save(db: &PgPool) -> Result<MapSaveRow, (StatusCode, String)> {
    ensure_default_layers(db).await.map_err(map_db_error)?;

    let pointer: Option<MapSettingsPointerRow> = sqlx::query_as(
        r#"
        SELECT
            active_save_id,
            active_base_layer_id,
            center_lat,
            center_lng,
            zoom,
            bearing,
            pitch
        FROM map_settings
        WHERE singleton = TRUE
        "#,
    )
    .fetch_optional(db)
    .await
    .map_err(map_db_error)?;

    if let Some(active_id) = pointer.as_ref().and_then(|row| row.active_save_id) {
        if let Some(save) = load_save_by_id(db, active_id).await.map_err(map_db_error)? {
            return Ok(save);
        }
    }

    let default_center = default_center_from_weather_config(db)
        .await
        .unwrap_or((0.0, 0.0));
    let base_id_default = default_base_layer_id(db).await;

    let active_base_layer_id = pointer
        .as_ref()
        .and_then(|row| row.active_base_layer_id)
        .or(base_id_default);
    let center_lat = pointer
        .as_ref()
        .and_then(|row| row.center_lat)
        .unwrap_or(default_center.0);
    let center_lng = pointer
        .as_ref()
        .and_then(|row| row.center_lng)
        .unwrap_or(default_center.1);
    let zoom = pointer
        .as_ref()
        .and_then(|row| row.zoom)
        .unwrap_or(DEFAULT_ZOOM);
    let bearing = pointer.as_ref().and_then(|row| row.bearing).unwrap_or(0.0);
    let pitch = pointer.as_ref().and_then(|row| row.pitch).unwrap_or(0.0);

    let save: MapSaveRow = sqlx::query_as(
        r#"
        INSERT INTO map_saves (
            name,
            active_base_layer_id,
            center_lat,
            center_lng,
            zoom,
            bearing,
            pitch,
            created_at,
            updated_at
        )
        VALUES ('Default', $1, $2, $3, $4, $5, $6, NOW(), NOW())
        RETURNING id, name, active_base_layer_id, center_lat, center_lng, zoom, bearing, pitch, created_at, updated_at
        "#,
    )
    .bind(active_base_layer_id)
    .bind(center_lat)
    .bind(center_lng)
    .bind(zoom)
    .bind(bearing)
    .bind(pitch)
    .fetch_one(db)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        INSERT INTO map_settings (singleton, active_save_id, created_at, updated_at)
        VALUES (TRUE, $1, NOW(), NOW())
        ON CONFLICT (singleton)
        DO UPDATE SET active_save_id = EXCLUDED.active_save_id, updated_at = NOW()
        "#,
    )
    .bind(save.id)
    .execute(db)
    .await
    .map_err(map_db_error)?;

    Ok(save)
}

fn validate_layer_kind(kind: &str) -> Result<(), (StatusCode, String)> {
    match kind {
        "base" | "overlay" => Ok(()),
        _ => Err((
            StatusCode::BAD_REQUEST,
            "kind must be 'base' or 'overlay'".to_string(),
        )),
    }
}

fn validate_layer_source_type(source_type: &str) -> Result<(), (StatusCode, String)> {
    match source_type {
        "xyz" | "wms" | "arcgis" | "geojson" | "terrain" => Ok(()),
        _ => Err((
            StatusCode::BAD_REQUEST,
            "source_type must be one of: xyz, wms, arcgis, geojson, terrain".to_string(),
        )),
    }
}

fn validate_layer_config(
    source_type: &str,
    config: &JsonValue,
) -> Result<(), (StatusCode, String)> {
    let obj = config.as_object().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "config must be a JSON object".to_string(),
        )
    })?;

    let required = |key: &str| -> Result<&str, (StatusCode, String)> {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("config.{key} is required")))
    };

    match source_type {
        "xyz" => {
            let _ = required("url_template")?;
        }
        "terrain" => {
            let _ = required("url_template")?;
        }
        "wms" => {
            let _ = required("base_url")?;
            let _ = required("layers")?;
        }
        "arcgis" => {
            let _ = required("base_url")?;
        }
        "geojson" => {
            let data = obj.get("data").ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "config.data is required".to_string(),
                )
            })?;
            if !data.is_object() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "config.data must be a GeoJSON object".to_string(),
                ));
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_geojson_geometry(value: &JsonValue) -> Result<&str, (StatusCode, String)> {
    let obj = value.as_object().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "geometry must be a GeoJSON geometry object".to_string(),
        )
    })?;
    let geo_type = obj
        .get("type")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "geometry.type is required".to_string(),
            )
        })?;
    Ok(geo_type)
}

#[utoipa::path(
    get,
    path = "/api/map/settings",
    tag = "map",
    responses((status = 200, description = "Map settings", body = MapSettingsResponse))
)]
pub(crate) async fn get_map_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<MapSettingsResponse>, (StatusCode, String)> {
    let default_center = default_center_from_weather_config(&state.db)
        .await
        .unwrap_or((0.0, 0.0));
    let base_id_default = default_base_layer_id(&state.db).await;
    let active = ensure_active_save(&state.db).await?;

    Ok(Json(MapSettingsResponse {
        active_save_id: active.id,
        active_save_name: active.name,
        active_base_layer_id: active.active_base_layer_id.or(base_id_default),
        center_lat: active.center_lat.unwrap_or(default_center.0),
        center_lng: active.center_lng.unwrap_or(default_center.1),
        zoom: active.zoom.unwrap_or(DEFAULT_ZOOM),
        bearing: active.bearing.unwrap_or(0.0),
        pitch: active.pitch.unwrap_or(0.0),
        updated_at: Some(active.updated_at.to_rfc3339()),
    }))
}

#[utoipa::path(
    put,
    path = "/api/map/settings",
    tag = "map",
    request_body = MapSettingsUpdateRequest,
    responses(
        (status = 200, description = "Updated map settings", body = MapSettingsResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_map_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<MapSettingsUpdateRequest>,
) -> Result<Json<MapSettingsResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if !(-90.0..=90.0).contains(&payload.center_lat) {
        return Err((
            StatusCode::BAD_REQUEST,
            "center_lat must be -90..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&payload.center_lng) {
        return Err((
            StatusCode::BAD_REQUEST,
            "center_lng must be -180..180".to_string(),
        ));
    }
    if !(0.0..=24.0).contains(&payload.zoom) {
        return Err((StatusCode::BAD_REQUEST, "zoom must be 0..24".to_string()));
    }

    let bearing = payload.bearing.unwrap_or(0.0);
    let pitch = payload.pitch.unwrap_or(0.0);

    let active = ensure_active_save(&state.db).await?;

    let updated: MapSaveRow = sqlx::query_as(
        r#"
        UPDATE map_saves
        SET active_base_layer_id = $2,
            center_lat = $3,
            center_lng = $4,
            zoom = $5,
            bearing = $6,
            pitch = $7,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, active_base_layer_id, center_lat, center_lng, zoom, bearing, pitch, created_at, updated_at
        "#,
    )
    .bind(active.id)
    .bind(payload.active_base_layer_id)
    .bind(payload.center_lat)
    .bind(payload.center_lng)
    .bind(payload.zoom)
    .bind(bearing)
    .bind(pitch)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(MapSettingsResponse {
        active_save_id: updated.id,
        active_save_name: updated.name,
        active_base_layer_id: updated.active_base_layer_id,
        center_lat: updated.center_lat.unwrap_or(payload.center_lat),
        center_lng: updated.center_lng.unwrap_or(payload.center_lng),
        zoom: updated.zoom.unwrap_or(payload.zoom),
        bearing: updated.bearing.unwrap_or(bearing),
        pitch: updated.pitch.unwrap_or(pitch),
        updated_at: Some(updated.updated_at.to_rfc3339()),
    }))
}

#[utoipa::path(
    get,
    path = "/api/map/saves",
    tag = "map",
    responses((status = 200, description = "Map saves", body = Vec<MapSaveResponse>))
)]
pub(crate) async fn list_map_saves(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<MapSaveResponse>>, (StatusCode, String)> {
    let rows: Vec<MapSaveRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            active_base_layer_id,
            center_lat,
            center_lng,
            zoom,
            bearing,
            pitch,
            created_at,
            updated_at
        FROM map_saves
        ORDER BY updated_at DESC, id DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| MapSaveResponse {
                id: row.id,
                name: row.name,
                created_at: row.created_at.to_rfc3339(),
                updated_at: row.updated_at.to_rfc3339(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/map/saves",
    tag = "map",
    request_body = MapSaveCreateRequest,
    responses(
        (status = 201, description = "Created map save", body = MapSaveResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_map_save(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<MapSaveCreateRequest>,
) -> Result<(StatusCode, Json<MapSaveResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".to_string()));
    }

    if let Some(center_lat) = payload.center_lat {
        if !(-90.0..=90.0).contains(&center_lat) {
            return Err((
                StatusCode::BAD_REQUEST,
                "center_lat must be -90..90".to_string(),
            ));
        }
    }
    if let Some(center_lng) = payload.center_lng {
        if !(-180.0..=180.0).contains(&center_lng) {
            return Err((
                StatusCode::BAD_REQUEST,
                "center_lng must be -180..180".to_string(),
            ));
        }
    }
    if let Some(zoom) = payload.zoom {
        if !(0.0..=24.0).contains(&zoom) {
            return Err((StatusCode::BAD_REQUEST, "zoom must be 0..24".to_string()));
        }
    }

    let active = ensure_active_save(&state.db).await?;
    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let row: MapSaveRow = sqlx::query_as(
        r#"
        INSERT INTO map_saves (
            name,
            active_base_layer_id,
            center_lat,
            center_lng,
            zoom,
            bearing,
            pitch,
            created_at,
            updated_at
        )
        SELECT
            $1,
            COALESCE($2, active_base_layer_id),
            COALESCE($3, center_lat),
            COALESCE($4, center_lng),
            COALESCE($5, zoom),
            COALESCE($6, bearing),
            COALESCE($7, pitch),
            NOW(),
            NOW()
        FROM map_saves
        WHERE id = $8
        RETURNING id, name, active_base_layer_id, center_lat, center_lng, zoom, bearing, pitch, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(payload.active_base_layer_id)
    .bind(payload.center_lat)
    .bind(payload.center_lng)
    .bind(payload.zoom)
    .bind(payload.bearing)
    .bind(payload.pitch)
    .bind(active.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        INSERT INTO map_features (save_id, node_id, sensor_id, geometry, properties, created_at, updated_at)
        SELECT $1, node_id, sensor_id, geometry, properties, NOW(), NOW()
        FROM map_features
        WHERE save_id = $2
        "#,
    )
    .bind(row.id)
    .bind(active.id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        INSERT INTO map_settings (singleton, active_save_id, created_at, updated_at)
        VALUES (TRUE, $1, NOW(), NOW())
        ON CONFLICT (singleton)
        DO UPDATE SET active_save_id = EXCLUDED.active_save_id, updated_at = NOW()
        "#,
    )
    .bind(row.id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;

    Ok((
        StatusCode::CREATED,
        Json(MapSaveResponse {
            id: row.id,
            name: row.name,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/map/saves/{id}/apply",
    tag = "map",
    params(("id" = i64, Path, description = "Save id")),
    responses(
        (status = 200, description = "Active map settings", body = MapSettingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn apply_map_save(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<MapSettingsResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let Some(save) = load_save_by_id(&state.db, id).await.map_err(map_db_error)? else {
        return Err((StatusCode::NOT_FOUND, "Map save not found".to_string()));
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO map_settings (singleton, active_save_id, created_at, updated_at)
        VALUES (TRUE, $1, NOW(), NOW())
        ON CONFLICT (singleton)
        DO UPDATE SET active_save_id = EXCLUDED.active_save_id, updated_at = NOW()
        "#,
    )
    .bind(save.id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    let default_center = default_center_from_weather_config(&state.db)
        .await
        .unwrap_or((0.0, 0.0));
    let base_id_default = default_base_layer_id(&state.db).await;

    Ok(Json(MapSettingsResponse {
        active_save_id: save.id,
        active_save_name: save.name,
        active_base_layer_id: save.active_base_layer_id.or(base_id_default),
        center_lat: save.center_lat.unwrap_or(default_center.0),
        center_lng: save.center_lng.unwrap_or(default_center.1),
        zoom: save.zoom.unwrap_or(DEFAULT_ZOOM),
        bearing: save.bearing.unwrap_or(0.0),
        pitch: save.pitch.unwrap_or(0.0),
        updated_at: Some(save.updated_at.to_rfc3339()),
    }))
}

#[utoipa::path(
    get,
    path = "/api/map/layers",
    tag = "map",
    responses((status = 200, description = "Map layers", body = Vec<MapLayerResponse>))
)]
pub(crate) async fn list_map_layers(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<MapLayerResponse>>, (StatusCode, String)> {
    ensure_default_layers(&state.db)
        .await
        .map_err(map_db_error)?;

    let rows: Vec<MapLayerRow> = sqlx::query_as(
        r#"
        SELECT id, system_key, name, kind, source_type, config, opacity, enabled, z_index, created_at, updated_at
        FROM map_layers
        ORDER BY
            CASE WHEN kind = 'base' THEN 0 ELSE 1 END,
            z_index ASC,
            id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(rows.into_iter().map(MapLayerResponse::from).collect()))
}

#[utoipa::path(
    post,
    path = "/api/map/layers",
    tag = "map",
    request_body = MapLayerUpsertRequest,
    responses(
        (status = 201, description = "Created map layer", body = MapLayerResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_map_layer(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<MapLayerUpsertRequest>,
) -> Result<(StatusCode, Json<MapLayerResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing name".to_string()));
    }
    let kind = payload.kind.trim().to_lowercase();
    let source_type = payload.source_type.trim().to_lowercase();

    validate_layer_kind(&kind)?;
    validate_layer_source_type(&source_type)?;
    validate_layer_config(&source_type, &payload.config)?;

    let opacity = payload.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let enabled = payload.enabled.unwrap_or(true);
    let z_index = payload.z_index.unwrap_or(0);

    let row: MapLayerRow = sqlx::query_as(
        r#"
        INSERT INTO map_layers (system_key, name, kind, source_type, config, opacity, enabled, z_index, created_at, updated_at)
        VALUES (NULL, $1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
        RETURNING id, system_key, name, kind, source_type, config, opacity, enabled, z_index, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(kind)
    .bind(source_type)
    .bind(SqlJson(payload.config))
    .bind(opacity)
    .bind(enabled)
    .bind(z_index)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok((StatusCode::CREATED, Json(MapLayerResponse::from(row))))
}

#[utoipa::path(
    put,
    path = "/api/map/layers/{id}",
    tag = "map",
    request_body = MapLayerUpsertRequest,
    params(("id" = i64, Path, description = "Layer id")),
    responses(
        (status = 200, description = "Updated map layer", body = MapLayerResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_map_layer(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(payload): Json<MapLayerUpsertRequest>,
) -> Result<Json<MapLayerResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing name".to_string()));
    }
    let kind = payload.kind.trim().to_lowercase();
    let source_type = payload.source_type.trim().to_lowercase();

    validate_layer_kind(&kind)?;
    validate_layer_source_type(&source_type)?;
    validate_layer_config(&source_type, &payload.config)?;

    let opacity = payload.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let enabled = payload.enabled.unwrap_or(true);
    let z_index = payload.z_index.unwrap_or(0);

    let row: Option<MapLayerRow> = sqlx::query_as(
        r#"
        UPDATE map_layers
        SET name = $2, kind = $3, source_type = $4, config = $5, opacity = $6, enabled = $7, z_index = $8, updated_at = NOW()
        WHERE id = $1
        RETURNING id, system_key, name, kind, source_type, config, opacity, enabled, z_index, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(kind)
    .bind(source_type)
    .bind(SqlJson(payload.config))
    .bind(opacity)
    .bind(enabled)
    .bind(z_index)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Layer not found".to_string()));
    };

    Ok(Json(MapLayerResponse::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/map/layers/{id}",
    tag = "map",
    params(("id" = i64, Path, description = "Layer id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_map_layer(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let deleted = sqlx::query(
        r#"
        DELETE FROM map_layers
        WHERE id = $1 AND system_key IS NULL
        "#,
    )
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?
    .rows_affected();

    if deleted == 0 {
        let exists: Option<(Option<String>,)> =
            sqlx::query_as("SELECT system_key FROM map_layers WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await
                .map_err(map_db_error)?;
        return match exists {
            None => Err((StatusCode::NOT_FOUND, "Layer not found".to_string())),
            Some((Some(_),)) => Err((
                StatusCode::BAD_REQUEST,
                "Default layers cannot be deleted; disable or edit instead".to_string(),
            )),
            Some((None,)) => Err((StatusCode::NOT_FOUND, "Layer not found".to_string())),
        };
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/map/features",
    tag = "map",
    responses((status = 200, description = "Map features", body = Vec<MapFeatureResponse>))
)]
pub(crate) async fn list_map_features(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<MapFeatureResponse>>, (StatusCode, String)> {
    let active = ensure_active_save(&state.db).await?;

    let rows: Vec<MapFeatureRow> = sqlx::query_as(
        r#"
        SELECT
            mf.id,
            mf.node_id,
            mf.sensor_id,
            mf.geometry,
            mf.properties,
            mf.created_at,
            mf.updated_at,
            COALESCE(s.config, '{}'::jsonb) as sensor_config,
            COALESCE(sn.config, '{}'::jsonb) as sensor_node_config
        FROM map_features mf
        LEFT JOIN nodes mn ON mn.id = mf.node_id
        LEFT JOIN sensors s ON s.sensor_id = mf.sensor_id
        LEFT JOIN nodes sn ON sn.id = s.node_id
        WHERE mf.save_id = $1
          AND (
            mf.node_id IS NULL
            OR (
              mn.id IS NOT NULL
              AND
              NOT (COALESCE(mn.config, '{}'::jsonb) @> '{"hidden": true}')
              AND NOT (COALESCE(mn.config, '{}'::jsonb) @> '{"poll_enabled": false}')
              AND NOT (COALESCE(mn.config, '{}'::jsonb) @> '{"deleted": true}')
            )
          )
          AND (
            mf.sensor_id IS NULL
            OR (
              s.sensor_id IS NOT NULL
              AND
              s.deleted_at IS NULL
              AND sn.id IS NOT NULL
            )
          )
        ORDER BY mf.id ASC
        "#,
    )
    .bind(active.id)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let visible_rows = rows
        .into_iter()
        .filter(|row| {
            if row.sensor_id.is_none() {
                return true;
            }
            sensor_visibility::evaluate_sensor_visibility(
                &row.sensor_config.0,
                &row.sensor_node_config.0,
            )
            .visible
        })
        .map(MapFeatureResponse::from)
        .collect();

    Ok(Json(visible_rows))
}

#[utoipa::path(
    post,
    path = "/api/map/features",
    tag = "map",
    request_body = MapFeatureUpsertRequest,
    responses(
        (status = 201, description = "Created map feature", body = MapFeatureResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_map_feature(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<MapFeatureUpsertRequest>,
) -> Result<(StatusCode, Json<MapFeatureResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let active = ensure_active_save(&state.db).await?;

    let node_id = payload
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let sensor_id = payload
        .sensor_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    if node_id.is_some() && sensor_id.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Specify only one of node_id or sensor_id".to_string(),
        ));
    }

    let geo_type = validate_geojson_geometry(&payload.geometry)?;
    if (node_id.is_some() || sensor_id.is_some()) && geo_type != "Point" {
        return Err((
            StatusCode::BAD_REQUEST,
            "Node/sensor features must use Point geometry".to_string(),
        ));
    }

    let properties = payload.properties.unwrap_or_else(|| serde_json::json!({}));
    let now = Utc::now();

    let row: MapFeatureRow = if let Some(node_id) = node_id {
        let uuid = Uuid::parse_str(node_id)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node_id".to_string()))?;
        sqlx::query_as(
            r#"
            INSERT INTO map_features (save_id, node_id, sensor_id, geometry, properties, created_at, updated_at)
            VALUES ($1, $2, NULL, $3, $4, $5, $5)
            ON CONFLICT (save_id, node_id) WHERE node_id IS NOT NULL
            DO UPDATE SET geometry = EXCLUDED.geometry, properties = EXCLUDED.properties, updated_at = EXCLUDED.updated_at
            RETURNING id, node_id, sensor_id, geometry, properties, created_at, updated_at,
                      '{}'::jsonb as sensor_config, '{}'::jsonb as sensor_node_config
            "#,
        )
        .bind(active.id)
        .bind(uuid)
        .bind(SqlJson(payload.geometry))
        .bind(SqlJson(properties))
        .bind(now)
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?
    } else if let Some(sensor_id) = sensor_id {
        let sensor_id = sensor_id.to_string();
        sqlx::query_as(
            r#"
            INSERT INTO map_features (save_id, node_id, sensor_id, geometry, properties, created_at, updated_at)
            VALUES ($1, NULL, $2, $3, $4, $5, $5)
            ON CONFLICT (save_id, sensor_id) WHERE sensor_id IS NOT NULL
            DO UPDATE SET geometry = EXCLUDED.geometry, properties = EXCLUDED.properties, updated_at = EXCLUDED.updated_at
            RETURNING id, node_id, sensor_id, geometry, properties, created_at, updated_at,
                      '{}'::jsonb as sensor_config, '{}'::jsonb as sensor_node_config
            "#,
        )
        .bind(active.id)
        .bind(sensor_id)
        .bind(SqlJson(payload.geometry))
        .bind(SqlJson(properties))
        .bind(now)
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?
    } else {
        sqlx::query_as(
            r#"
            INSERT INTO map_features (save_id, node_id, sensor_id, geometry, properties, created_at, updated_at)
            VALUES ($1, NULL, NULL, $2, $3, $4, $4)
            RETURNING id, node_id, sensor_id, geometry, properties, created_at, updated_at,
                      '{}'::jsonb as sensor_config, '{}'::jsonb as sensor_node_config
            "#,
        )
        .bind(active.id)
        .bind(SqlJson(payload.geometry))
        .bind(SqlJson(properties))
        .bind(now)
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?
    };

    Ok((StatusCode::CREATED, Json(MapFeatureResponse::from(row))))
}

#[utoipa::path(
    put,
    path = "/api/map/features/{id}",
    tag = "map",
    request_body = MapFeatureUpsertRequest,
    params(("id" = i64, Path, description = "Feature id")),
    responses(
        (status = 200, description = "Updated map feature", body = MapFeatureResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_map_feature(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Json(payload): Json<MapFeatureUpsertRequest>,
) -> Result<Json<MapFeatureResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let active = ensure_active_save(&state.db).await?;

    let _ = validate_geojson_geometry(&payload.geometry)?;
    let properties = payload.properties.unwrap_or_else(|| serde_json::json!({}));

    let row: Option<MapFeatureRow> = sqlx::query_as(
        r#"
        UPDATE map_features
        SET geometry = $2, properties = $3, updated_at = NOW()
        WHERE id = $1 AND save_id = $4
        RETURNING id, node_id, sensor_id, geometry, properties, created_at, updated_at,
                  '{}'::jsonb as sensor_config, '{}'::jsonb as sensor_node_config
        "#,
    )
    .bind(id)
    .bind(SqlJson(payload.geometry))
    .bind(SqlJson(properties))
    .bind(active.id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Feature not found".to_string()));
    };

    Ok(Json(MapFeatureResponse::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/map/features/{id}",
    tag = "map",
    params(("id" = i64, Path, description = "Feature id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_map_feature(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let active = ensure_active_save(&state.db).await?;

    let deleted = sqlx::query("DELETE FROM map_features WHERE id = $1 AND save_id = $2")
        .bind(id)
        .bind(active.id)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?
        .rows_affected();

    if deleted == 0 {
        return Err((StatusCode::NOT_FOUND, "Feature not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/map/settings",
            get(get_map_settings).put(update_map_settings),
        )
        .route("/map/saves", get(list_map_saves).post(create_map_save))
        .route("/map/saves/{id}/apply", post(apply_map_save))
        .route("/map/layers", get(list_map_layers).post(create_map_layer))
        .route(
            "/map/layers/{id}",
            put(update_map_layer).delete(delete_map_layer),
        )
        .route(
            "/map/features",
            get(list_map_features).post(create_map_feature),
        )
        .route(
            "/map/features/{id}",
            put(update_map_feature).delete(delete_map_feature),
        )
}
