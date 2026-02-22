use anyhow::Result;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener as TokioTcpListener;
use tower_http::cors::{Any, CorsLayer};

use crate::cli::ServeArgs;
use crate::config::{
    env_flag, load_config, normalize_config, patch_config, resolve_config_path, save_config,
    SetupConfig, SetupConfigPatch,
};
use crate::launchd::{generate_plan, run_preflight};
use crate::privileged::run_farmctl_authorized;
use crate::profile::InstallProfile;
use crate::utils::{run_cmd_capture, CommandResult};

const SETUP_INDEX_HTML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../setup-app/static/index.html"
));
const SETUP_APP_JS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../setup-app/static/app.js"
));
const SETUP_APP_CSS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../setup-app/static/app.css"
));

#[derive(Clone)]
struct ApiState {
    config_path: PathBuf,
    static_root: Option<PathBuf>,
    profile_override: Option<InstallProfile>,
}

pub async fn serve(args: ServeArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let static_root = args.static_root.clone().or_else(|| {
        std::env::var("FARM_SETUP_STATIC_ROOT")
            .ok()
            .map(PathBuf::from)
    });
    let state = Arc::new(ApiState {
        config_path,
        static_root,
        profile_override,
    });

    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://127.0.0.1:3000"),
            HeaderValue::from_static("http://127.0.0.1:3005"),
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("http://localhost:3005"),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/healthz", get(healthz_handler))
        .route("/static/app.css", get(app_css_handler))
        .route("/static/app.js", get(app_js_handler))
        .route(
            "/api/config",
            get(get_config_handler).post(post_config_handler),
        )
        .route("/api/local-ip", get(local_ip_handler))
        .route("/api/preflight", get(preflight_handler))
        .route("/api/plan", post(plan_handler))
        .route("/api/status", get(status_handler))
        .route("/api/install", post(install_handler))
        .route("/api/upgrade", post(upgrade_handler))
        .route("/api/rollback", post(rollback_handler))
        .route("/api/health-report", get(health_report_handler))
        .route("/api/diagnostics", post(diagnostics_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = TokioTcpListener::bind(&addr).await?;
    if !args.no_auto_open {
        let url = format!("http://{}", addr);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            let _ = Command::new("open").arg(&url).status();
        });
    }
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_static(&state, "index.html", SETUP_INDEX_HTML) {
        Ok(body) => Html(body).into_response(),
        Err(err) => error_response(err),
    }
}

async fn app_css_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_static(&state, "app.css", SETUP_APP_CSS) {
        Ok(body) => text_response("text/css; charset=utf-8", body),
        Err(err) => error_response(err),
    }
}

async fn app_js_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_static(&state, "app.js", SETUP_APP_JS) {
        Ok(body) => text_response("application/javascript; charset=utf-8", body),
        Err(err) => error_response(err),
    }
}

async fn healthz_handler() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn local_ip_handler() -> impl IntoResponse {
    let candidates = crate::net::lan_ipv4_candidates();
    let recommended = candidates.first().cloned();
    Json(json!({
        "recommended": recommended,
        "candidates": candidates,
    }))
}

async fn get_config_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state) {
        Ok(config) => Json(config).into_response(),
        Err(err) => error_response(err),
    }
}

async fn post_config_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SetupConfigPatch>,
) -> impl IntoResponse {
    match patch_config(&state.config_path, payload) {
        Ok(config) => Json(config).into_response(),
        Err(err) => error_response(err),
    }
}

async fn preflight_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state).and_then(|config| run_preflight(&config)) {
        Ok(checks) => Json(json!({ "checks": checks })).into_response(),
        Err(err) => error_response(err),
    }
}

async fn plan_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SetupConfigPatch>,
) -> impl IntoResponse {
    match patch_config(&state.config_path, payload).and_then(|config| generate_plan(&config)) {
        Ok(plan) => Json(plan).into_response(),
        Err(err) => error_response(err),
    }
}

async fn status_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state)
        .and_then(|config| run_farmctl(&config, &state.config_path, &["status"], &[]))
    {
        Ok(result) => Json(json!({
            "ok": result.ok,
            "result": parse_farmctl_json(&result),
            "logs": vec![result],
        }))
        .into_response(),
        Err(err) => error_response(err),
    }
}

async fn install_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    run_install_action(&state, "install").await
}

async fn upgrade_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    run_install_action(&state, "upgrade").await
}

async fn rollback_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    run_install_action(&state, "rollback").await
}

async fn run_install_action(state: &ApiState, action: &str) -> impl IntoResponse {
    let config = match load_config_for_state(state) {
        Ok(config) => config,
        Err(err) => return error_response(err),
    };
    let bundle = match config.bundle_path.as_ref() {
        Some(path) => path.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": "bundle_path is required" })),
            )
                .into_response();
        }
    };

    let mut args = vec![action.to_string()];
    if action != "rollback" {
        args.push("--bundle".to_string());
        args.push(bundle);
    }

    let farmctl_result = match run_farmctl(
        &config,
        &state.config_path,
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &[],
    ) {
        Ok(result) => result,
        Err(err) => return error_response(err),
    };

    let launchd_results: Vec<CommandResult> = Vec::new();
    let ok = farmctl_result.ok;
    let handoff = ok && should_handoff_to_launchd(&config);
    if handoff {
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(750)).await;
            std::process::exit(0);
        });
    }
    Json(json!({
        "ok": ok,
        "handoff": handoff,
        "farmctl": vec![farmctl_result],
        "launchd": launchd_results,
    }))
    .into_response()
}

async fn health_report_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state)
        .and_then(|config| run_farmctl(&config, &state.config_path, &["health", "--json"], &[]))
    {
        Ok(result) => Json(json!({
            "ok": result.ok,
            "report": parse_farmctl_json(&result),
            "logs": vec![result],
        }))
        .into_response(),
        Err(err) => error_response(err),
    }
}

async fn diagnostics_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Option<serde_json::Value>>,
) -> impl IntoResponse {
    let config = match load_config_for_state(&state) {
        Ok(config) => config,
        Err(err) => return error_response(err),
    };
    let mut args = vec!["diagnostics".to_string()];
    if let Some(payload) = payload {
        if payload
            .get("include_secrets")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            args.push("--include-secrets".to_string());
        }
        if let Some(output) = payload.get("output").and_then(|value| value.as_str()) {
            args.push("--output".to_string());
            args.push(output.to_string());
        }
    }
    match run_farmctl(
        &config,
        &state.config_path,
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &[],
    ) {
        Ok(result) => Json(json!({
            "ok": result.ok,
            "logs": vec![result],
        }))
        .into_response(),
        Err(err) => error_response(err),
    }
}

fn load_static(state: &ApiState, name: &str, fallback: &str) -> Result<String> {
    if let Some(root) = &state.static_root {
        let path = root.join(name);
        if path.exists() {
            return Ok(fs::read_to_string(path)?);
        }
    }
    Ok(fallback.to_string())
}

fn text_response(content_type: &str, body: String) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    let value = HeaderValue::from_str(content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("text/plain; charset=utf-8"));
    headers.insert(header::CONTENT_TYPE, value);
    (headers, body).into_response()
}

fn error_response(err: anyhow::Error) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "ok": false, "error": err.to_string() })),
    )
        .into_response()
}

fn load_config_for_state(state: &ApiState) -> Result<SetupConfig> {
    let mut config = load_config(&state.config_path)?;
    normalize_config(&mut config, state.profile_override)?;
    save_config(&state.config_path, &config)?;
    Ok(config)
}

fn run_farmctl(
    config: &SetupConfig,
    config_path: &PathBuf,
    args: &[&str],
    env_overrides: &[(&str, &str)],
) -> Result<CommandResult> {
    let action = args.first().copied().unwrap_or("");
    let euid = unsafe { libc::geteuid() };
    let running_as_service_user = is_running_as_service_user(config, euid);
    let needs_admin = config.profile == InstallProfile::Prod
        && euid != 0
        && matches!(action, "uninstall" | "install" | "upgrade" | "rollback")
        && (action == "uninstall" || !running_as_service_user);
    if needs_admin {
        return run_farmctl_authorized(&config.farmctl_path, args, config_path, env_overrides);
    }

    let mut command = Command::new(&config.farmctl_path);
    command.args(args).arg("--config").arg(config_path);
    for (key, value) in env_overrides {
        command.env(key, value);
    }
    run_cmd_capture(command)
}

fn is_running_as_service_user(config: &SetupConfig, euid: libc::uid_t) -> bool {
    let service_user = config.service_user.trim();
    if service_user.is_empty() {
        return false;
    }
    match crate::service_user::lookup_uid_gid(service_user) {
        Ok((uid, _)) => uid as libc::uid_t == euid,
        Err(_) => false,
    }
}

fn parse_farmctl_json(result: &CommandResult) -> serde_json::Value {
    if result.stdout.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(&result.stdout).unwrap_or_else(|_| json!({ "raw": result.stdout }))
}

fn should_handoff_to_launchd(config: &SetupConfig) -> bool {
    if !env_flag("FARM_SETUP_BOOTSTRAP") {
        return false;
    }
    if config.profile != InstallProfile::Prod {
        return false;
    }
    true
}
