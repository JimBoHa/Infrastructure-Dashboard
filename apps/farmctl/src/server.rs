use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::net::TcpListener as TokioTcpListener;
use tower_http::cors::{Any, CorsLayer};

use crate::cli::ServeArgs;
use crate::config::{
    default_config, env_flag, load_config, normalize_config, patch_config, resolve_config_path,
    save_config, setup_state_dir, SetupConfig, SetupConfigPatch,
};
use crate::launchd::{generate_plan, run_preflight, LaunchdPlan, PreflightCheck};
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
    farmctl_path: PathBuf,
}

const SETUP_ACTIVITY_LOG_NAME: &str = "setup-assistant.log";

fn activity_log_path() -> PathBuf {
    setup_state_dir().join(SETUP_ACTIVITY_LOG_NAME)
}

fn append_activity_log(event: &str, payload: serde_json::Value) {
    let path = activity_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let entry = json!({
        "ts": Utc::now().to_rfc3339(),
        "event": event,
        "payload": payload,
    });

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}", entry);
    }
}

fn preflight_summary(checks: &[PreflightCheck]) -> serde_json::Value {
    let ready = checks.iter().filter(|check| check.status == "ok").count();
    let warnings = checks.iter().filter(|check| check.status == "warn").count();
    let blocked = checks
        .iter()
        .filter(|check| check.status == "error")
        .count();
    json!({
        "ready": ready,
        "warnings": warnings,
        "blocked": blocked,
        "install_ready": blocked == 0,
    })
}

fn plan_summary(plan: &LaunchdPlan) -> serde_json::Value {
    json!({
        "services": plan.plists.len(),
        "warnings": plan.warnings.len(),
        "staging_dir": plan.staging_dir,
        "target_dir": plan.target_dir,
    })
}

fn user_message_for_action(action: &str, ok: bool, handoff: bool) -> String {
    if ok && handoff {
        return format!(
            "{action} complete. Infrastructure Dashboard is switching to managed services."
        );
    }
    if ok {
        return format!("{action} complete.");
    }
    format!("{action} did not finish successfully. Detailed diagnostics were saved automatically.")
}

fn ui_error_response(event: &str, err: anyhow::Error) -> axum::response::Response {
    append_activity_log(
        event,
        json!({
            "ok": false,
            "error": err.to_string(),
        }),
    );
    error_response(err)
}

pub async fn serve(args: ServeArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let static_root = args.static_root.clone().or_else(|| {
        std::env::var("FARM_SETUP_STATIC_ROOT")
            .ok()
            .map(PathBuf::from)
    });
    let farmctl_path = std::env::current_exe().context("Failed to resolve farmctl binary path")?;
    let state = Arc::new(ApiState {
        config_path,
        static_root,
        profile_override,
        farmctl_path,
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
        .route("/api/uninstall", post(uninstall_handler))
        .route("/api/remove-failed-install", post(remove_failed_install_handler))
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
        Err(err) => ui_error_response("config.load", err),
    }
}

async fn post_config_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SetupConfigPatch>,
) -> impl IntoResponse {
    match patch_config(&state.config_path, payload.clone()) {
        Ok(config) => Json(config).into_response(),
        Err(err) => {
            if is_permission_denied(&err) {
                match patch_config_privileged(&state, payload) {
                    Ok(config) => Json(config).into_response(),
                    Err(priv_err) => ui_error_response("config.patch", priv_err),
                }
            } else {
                ui_error_response("config.patch", err)
            }
        }
    }
}

async fn preflight_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state).and_then(|config| run_preflight(&config)) {
        Ok(checks) => {
            let summary = preflight_summary(&checks);
            append_activity_log(
                "preflight",
                json!({
                    "ok": true,
                    "summary": summary.clone(),
                    "checks": checks.clone(),
                }),
            );
            Json(json!({
                "checks": checks,
                "summary": summary,
                "activity_log": activity_log_path(),
            }))
            .into_response()
        }
        Err(err) => ui_error_response("preflight", err),
    }
}

async fn plan_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<SetupConfigPatch>,
) -> impl IntoResponse {
    match patch_config(&state.config_path, payload).and_then(|config| generate_plan(&config)) {
        Ok(plan) => {
            let summary = plan_summary(&plan);
            append_activity_log(
                "plan",
                json!({
                    "ok": true,
                    "summary": summary.clone(),
                    "plan": plan.clone(),
                }),
            );
            Json(json!({
                "plan": plan,
                "summary": summary,
                "activity_log": activity_log_path(),
            }))
            .into_response()
        }
        Err(err) => ui_error_response("plan", err),
    }
}

async fn status_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state)
        .and_then(|config| run_farmctl(&config, &state.config_path, &["status"], &[]))
    {
        Ok(result) => {
            append_activity_log(
                "status",
                json!({
                    "ok": result.ok,
                    "result": parse_farmctl_json(&result),
                    "logs": [result.clone()],
                }),
            );
            Json(json!({
                "ok": result.ok,
                "result": parse_farmctl_json(&result),
                "activity_log": activity_log_path(),
            }))
            .into_response()
        }
        Err(err) => ui_error_response("status", err),
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

async fn uninstall_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Option<serde_json::Value>>,
) -> impl IntoResponse {
    let preserve_trends_and_sensors = payload
        .as_ref()
        .and_then(|value| value.get("preserve_trends_and_sensors"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    run_uninstall_action(&state, preserve_trends_and_sensors).await
}

async fn remove_failed_install_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    run_uninstall_action(&state, false).await
}

async fn run_uninstall_action(
    state: &ApiState,
    preserve_trends_and_sensors: bool,
) -> axum::response::Response {
    let config = match load_config_for_state(&state) {
        Ok(config) => config,
        Err(err) => return ui_error_response("install.cleanup.config", err),
    };

    let mut args = vec!["uninstall", "--remove-roots", "--yes"];
    if preserve_trends_and_sensors {
        args.push("--preserve-trends-and-sensors");
    }
    let farmctl_result = match run_farmctl(
        &config,
        &state.config_path,
        &args,
        &[],
    ) {
        Ok(result) => result,
        Err(err) => return ui_error_response("install.cleanup.run", err),
    };

    let message = if farmctl_result.ok {
        if preserve_trends_and_sensors {
            "Removed the install and preserved trend data + sensor names in a portable archive."
        } else {
            "Removed the failed install. Run readiness again before retrying."
        }
    } else {
        "Failed to remove the current install. Review the local setup activity log."
    };
    append_activity_log(
        "install.cleanup",
        json!({
            "ok": farmctl_result.ok,
            "preserve_trends_and_sensors": preserve_trends_and_sensors,
            "message": message,
            "farmctl": [farmctl_result.clone()],
        }),
    );
    Json(json!({
        "ok": farmctl_result.ok,
        "message": message,
        "activity_log": activity_log_path(),
        "farmctl": [farmctl_result],
    }))
    .into_response()
}

async fn run_install_action(state: &ApiState, action: &str) -> impl IntoResponse {
    let config = match load_config_for_state(state) {
        Ok(config) => config,
        Err(err) => return ui_error_response(&format!("install.{action}.config"), err),
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
        Err(err) => return ui_error_response(&format!("install.{action}.run"), err),
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
    let dashboard_url = format!("http://127.0.0.1:{}/", config.core_port);
    let message = user_message_for_action(action, ok, handoff);
    append_activity_log(
        &format!("install.{action}"),
        json!({
            "ok": ok,
            "handoff": handoff,
            "message": message.clone(),
            "dashboard_url": dashboard_url.clone(),
            "farmctl": [farmctl_result.clone()],
            "launchd": launchd_results.clone(),
        }),
    );
    Json(json!({
        "ok": ok,
        "handoff": handoff,
        "message": message,
        "dashboard_url": dashboard_url,
        "activity_log": activity_log_path(),
        "farmctl": [farmctl_result],
        "launchd": launchd_results,
    }))
    .into_response()
}

async fn health_report_handler(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match load_config_for_state(&state)
        .and_then(|config| run_farmctl(&config, &state.config_path, &["health", "--json"], &[]))
    {
        Ok(result) => {
            let report = parse_farmctl_json(&result);
            append_activity_log(
                "health",
                json!({
                    "ok": result.ok,
                    "report": report.clone(),
                    "logs": [result.clone()],
                }),
            );
            Json(json!({
                "ok": result.ok,
                "report": report,
                "activity_log": activity_log_path(),
            }))
            .into_response()
        }
        Err(err) => ui_error_response("health", err),
    }
}

async fn diagnostics_handler(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<Option<serde_json::Value>>,
) -> impl IntoResponse {
    let config = match load_config_for_state(&state) {
        Ok(config) => config,
        Err(err) => return ui_error_response("diagnostics.config", err),
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
        Ok(result) => {
            append_activity_log(
                "diagnostics",
                json!({
                    "ok": result.ok,
                    "logs": [result.clone()],
                }),
            );
            Json(json!({
                "ok": result.ok,
                "message": "Diagnostics were captured to the local setup activity log.",
                "activity_log": activity_log_path(),
            }))
            .into_response()
        }
        Err(err) => ui_error_response("diagnostics", err),
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
    let mut config = match load_config(&state.config_path) {
        Ok(config) => config,
        Err(err) if is_permission_denied(&err) => {
            let config = default_config()?;
            return write_config_privileged(state, &config);
        }
        Err(err) => return Err(err),
    };
    config.farmctl_path = state.farmctl_path.to_string_lossy().to_string();
    normalize_config(&mut config, state.profile_override)?;
    match save_config(&state.config_path, &config) {
        Ok(()) => Ok(config),
        Err(err) if is_permission_denied(&err) => write_config_privileged(state, &config),
        Err(err) => Err(err),
    }
}

fn patch_config_privileged(state: &ApiState, patch: SetupConfigPatch) -> Result<SetupConfig> {
    let mut temp = NamedTempFile::new().context("Failed to create temp file for config patch")?;
    let payload = serde_json::to_string(&patch)?;
    temp.write_all(payload.as_bytes())
        .context("Failed to write config patch payload")?;
    let temp_path = temp.path().to_path_buf();

    let temp_path_display = temp_path.to_string_lossy();
    let args = [
        "config",
        "patch",
        "--patch-file",
        temp_path_display.as_ref(),
    ];
    let result = run_farmctl_authorized(
        &state.farmctl_path.to_string_lossy(),
        &args,
        &state.config_path,
        &[],
    )?;
    if !result.ok {
        anyhow::bail!(result.stdout);
    }
    let parsed: SetupConfig =
        serde_json::from_str(&result.stdout).context("Failed to parse patched config response")?;
    Ok(parsed)
}

fn write_config_privileged(state: &ApiState, config: &SetupConfig) -> Result<SetupConfig> {
    let mut temp = NamedTempFile::new().context("Failed to create temp file for config write")?;
    let payload = serde_json::to_string_pretty(config)?;
    temp.write_all(payload.as_bytes())
        .context("Failed to write config payload")?;
    let temp_path = temp.path().to_path_buf();

    let temp_path_display = temp_path.to_string_lossy();
    let args = [
        "config",
        "write",
        "--config-file",
        temp_path_display.as_ref(),
    ];
    let result = run_farmctl_authorized(
        &state.farmctl_path.to_string_lossy(),
        &args,
        &state.config_path,
        &[],
    )?;
    if !result.ok {
        anyhow::bail!(result.stdout);
    }
    let parsed: SetupConfig =
        serde_json::from_str(&result.stdout).context("Failed to parse config write response")?;
    Ok(parsed)
}

fn is_permission_denied(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::PermissionDenied)
    })
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
        && (action == "install" || action == "uninstall" || !running_as_service_user);
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
