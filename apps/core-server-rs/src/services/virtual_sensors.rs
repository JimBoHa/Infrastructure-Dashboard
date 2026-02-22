use serde_json::{json, Map, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use uuid::Uuid;

use crate::ids;

pub const SENSOR_SOURCE_FORECAST_POINTS: &str = "forecast_points";

pub async fn ensure_forecast_point_sensor(
    db: &PgPool,
    node_id: Uuid,
    key: &str,
    name: &str,
    sensor_type: &str,
    unit: &str,
    interval_seconds: i32,
    provider: &str,
    kind: &str,
    subject_kind: &str,
    subject: &str,
    metric: &str,
    mode: &str,
) -> Result<String, sqlx::Error> {
    let sensor_id = ids::stable_hex_id(SENSOR_SOURCE_FORECAST_POINTS, key);
    let config = json!({
        "source": SENSOR_SOURCE_FORECAST_POINTS,
        "virtual": true,
        "read_only": true,
        "provider": provider,
        "kind": kind,
        "subject_kind": subject_kind,
        "subject": subject,
        "metric": metric,
        "mode": mode,
    });

    sqlx::query(
        r#"
        INSERT INTO sensors (
          sensor_id, node_id, name, type, unit, interval_seconds, rolling_avg_seconds, config, created_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,0,$7,NOW())
        ON CONFLICT (sensor_id) DO UPDATE
          SET node_id = EXCLUDED.node_id,
              name = EXCLUDED.name,
              type = EXCLUDED.type,
              unit = EXCLUDED.unit,
              interval_seconds = EXCLUDED.interval_seconds,
              config = EXCLUDED.config
        "#,
    )
    .bind(&sensor_id)
    .bind(node_id)
    .bind(name)
    .bind(sensor_type)
    .bind(unit)
    .bind(interval_seconds.max(1))
    .bind(SqlJson(config))
    .execute(db)
    .await?;

    Ok(sensor_id)
}

pub async fn ensure_read_only_virtual_sensor(
    db: &PgPool,
    node_id: Uuid,
    namespace: &str,
    key: &str,
    name: &str,
    sensor_type: &str,
    unit: &str,
    interval_seconds: i32,
    extra_config: JsonValue,
) -> Result<String, sqlx::Error> {
    let sensor_id = ids::stable_hex_id(namespace, key);

    let mut config_obj: Map<String, JsonValue> = extra_config.as_object().cloned().unwrap_or_default();
    config_obj.insert("source".to_string(), JsonValue::String(namespace.to_string()));
    config_obj.insert("virtual".to_string(), JsonValue::Bool(true));
    config_obj.insert("read_only".to_string(), JsonValue::Bool(true));

    sqlx::query(
        r#"
        INSERT INTO sensors (
          sensor_id, node_id, name, type, unit, interval_seconds, rolling_avg_seconds, config, created_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,0,$7,NOW())
        ON CONFLICT (sensor_id) DO UPDATE
          SET node_id = EXCLUDED.node_id,
              name = EXCLUDED.name,
              type = EXCLUDED.type,
              unit = EXCLUDED.unit,
              interval_seconds = EXCLUDED.interval_seconds,
              config = EXCLUDED.config
        "#,
    )
    .bind(&sensor_id)
    .bind(node_id)
    .bind(name)
    .bind(sensor_type)
    .bind(unit)
    .bind(interval_seconds.max(1))
    .bind(SqlJson(JsonValue::Object(config_obj)))
    .execute(db)
    .await?;

    Ok(sensor_id)
}
