use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use sqlx::PgPool;
use std::net::{IpAddr, Ipv4Addr};

use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ConnectionResponse {
    pub(crate) mode: String,
    pub(crate) local_address: String,
    pub(crate) cloud_address: String,
    pub(crate) status: String,
    pub(crate) last_switch: Option<String>,
    pub(crate) timezone: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ConnectionUpdate {
    mode: Option<String>,
    local_address: Option<String>,
    cloud_address: Option<String>,
    status: Option<String>,
}

fn extract_first_header(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    if first.is_empty() {
        return None;
    }
    Some(first.to_string())
}

fn scheme_from_headers(headers: &HeaderMap) -> &'static str {
    let proto = extract_first_header(headers, "x-forwarded-proto");
    if proto
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("https"))
    {
        return "https";
    }
    "http"
}

fn host_without_port(host: &str) -> &str {
    if host.starts_with('[') {
        if let Some(end) = host.find(']') {
            return &host[1..end];
        }
    }

    if let Some((name, port)) = host.rsplit_once(':') {
        if !name.is_empty() && !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()) {
            return name;
        }
    }

    host
}

fn is_tailscale_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

fn is_local_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.');
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if host.ends_with(".local") || host.ends_with(".lan") {
        return true;
    }

    let ip = match host.parse::<IpAddr>() {
        Ok(value) => value,
        Err(_) => return false,
    };
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || is_tailscale_ipv4(v4)
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

async fn db_online(db: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(db).await.is_ok()
}

pub(crate) async fn connection_for_request(
    state: &AppState,
    headers: &HeaderMap,
) -> ConnectionResponse {
    let timezone = iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string());
    let scheme = scheme_from_headers(headers);
    let host = extract_first_header(headers, "x-forwarded-host")
        .or_else(|| extract_first_header(headers, "host"))
        .unwrap_or_else(|| "localhost".to_string());
    let current_address = format!("{scheme}://{host}");

    let status = if db_online(&state.db).await {
        "online".to_string()
    } else {
        "degraded".to_string()
    };

    let is_local = is_local_host(host_without_port(&host));
    if is_local {
        ConnectionResponse {
            mode: "local".to_string(),
            local_address: current_address,
            cloud_address: "".to_string(),
            status,
            last_switch: None,
            timezone,
        }
    } else {
        ConnectionResponse {
            mode: "cloud".to_string(),
            local_address: "".to_string(),
            cloud_address: current_address,
            status,
            last_switch: None,
            timezone,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/connection",
    tag = "connection",
    responses((status = 200, description = "Connection status", body = ConnectionResponse))
)]
pub(crate) async fn get_connection(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
) -> Json<ConnectionResponse> {
    Json(connection_for_request(&state, &headers).await)
}

#[utoipa::path(
    put,
    path = "/api/connection",
    tag = "connection",
    request_body = ConnectionUpdate,
    responses(
        (status = 200, description = "Connection status", body = ConnectionResponse),
        (status = 501, description = "Not implemented")
    )
)]
pub(crate) async fn update_connection(
    axum::extract::State(_state): axum::extract::State<AppState>,
    Json(payload): Json<ConnectionUpdate>,
) -> Result<Json<ConnectionResponse>, (StatusCode, String)> {
    let mut requested: Vec<String> = Vec::new();
    if let Some(mode) = payload
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        requested.push(format!("mode={mode}"));
    }
    if let Some(local_address) = payload
        .local_address
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        requested.push(format!("local_address={local_address}"));
    }
    if let Some(cloud_address) = payload
        .cloud_address
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        requested.push(format!("cloud_address={cloud_address}"));
    }
    if let Some(status) = payload
        .status
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        requested.push(format!("status={status}"));
    }
    if requested.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No connection updates provided".to_string(),
        ));
    }
    Err((
        StatusCode::NOT_IMPLEMENTED,
        format!(
            "Connection update not implemented (requested: {})",
            requested.join(", ")
        ),
    ))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/connection", get(get_connection).put(update_connection))
}
