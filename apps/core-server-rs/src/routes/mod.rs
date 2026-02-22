pub mod action_logs;
pub mod alarm_rules;
pub mod alarms;
pub mod analysis;
pub mod analytics;
pub mod annotations;
pub mod api_tokens;
pub mod auth;
pub mod backups;
pub mod backups_exports;
pub mod battery;
pub mod connection;
pub mod controller_config;
pub mod dashboard;
pub mod deployments;
pub mod dev_activity;
pub mod discovery;
pub mod display_profiles;
pub mod external_devices;
pub mod forecast;
pub mod health;
pub mod indicators;
pub mod incidents;
pub mod map;
pub mod map_assets;
pub mod map_offline;
pub mod metrics;
pub mod node_sensors;
pub mod nodes;
pub mod outputs;
pub mod power_runway;
pub mod predictive;
pub mod renogy;
pub mod renogy_settings;
pub mod schedules;
pub mod sensors;
pub mod setup;
pub mod setup_daemon;
pub mod templates;
pub mod users;
pub mod weather_stations;

use axum::Router;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::router())
        .nest(
            "/api",
            Router::new()
                .merge(auth::router())
                .merge(api_tokens::router())
                .merge(users::router())
                .merge(nodes::router())
                .merge(node_sensors::router())
                .merge(display_profiles::router())
                .merge(sensors::router())
                .merge(outputs::router())
                .merge(schedules::router())
                .merge(alarm_rules::router())
                .merge(alarms::router())
                .merge(incidents::router())
                .merge(action_logs::router())
                .merge(analysis::router())
                .merge(annotations::router())
                .merge(metrics::router())
                .merge(map::router())
                .merge(map_assets::router())
                .merge(map_offline::router())
                .merge(backups::router())
                .merge(backups_exports::router())
                .merge(connection::router())
                .merge(controller_config::router())
                .merge(battery::router())
                .merge(dev_activity::router())
                .merge(forecast::router())
                .merge(external_devices::router())
                .merge(analytics::router())
                .merge(indicators::router())
                .merge(templates::router())
                .merge(setup::router())
                .merge(power_runway::router())
                .merge(predictive::router())
                .merge(discovery::router())
                .merge(dashboard::router())
                .merge(deployments::router())
                .merge(weather_stations::router())
                .merge(renogy::router())
                .merge(renogy_settings::router())
                .nest("/setup-daemon", setup_daemon::router())
                .merge(crate::openapi::router()),
        )
        .with_state(state)
}

#[cfg(test)]
mod auth_gaps_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use std::sync::OnceLock;
    use tower::ServiceExt;

    static STATE: OnceLock<AppState> = OnceLock::new();

    fn state() -> AppState {
        STATE.get_or_init(crate::test_support::test_state).clone()
    }

    #[tokio::test]
    async fn metrics_query_requires_bearer_auth() {
        let app = Router::new()
            .route("/api/metrics/query", get(metrics::query_metrics))
            .with_state(state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/metrics/query")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn backups_list_requires_bearer_auth() {
        let app = Router::new()
            .route("/api/backups", get(backups::list_backups))
            .with_state(state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/backups")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn setup_credentials_list_requires_bearer_auth() {
        let app = Router::new()
            .route("/api/setup/credentials", get(setup::list_credentials))
            .with_state(state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/setup/credentials")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn dashboard_state_requires_bearer_auth() {
        let app = Router::new()
            .route("/api/dashboard/state", get(dashboard::dashboard_state))
            .with_state(state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/dashboard/state")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn nodes_list_requires_bearer_auth() {
        let app = Router::new()
            .route("/api/nodes", get(nodes::list_nodes))
            .with_state(state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/nodes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn metrics_query_forbidden_without_view_caps() {
        let user = crate::test_support::test_user_with_caps(&[]);
        let result = metrics::query_metrics(
            axum::extract::State(state()),
            crate::auth::AuthUser(user),
            axum::extract::RawQuery(None),
        )
        .await;
        let err = match result {
            Ok(_) => panic!("expected forbidden"),
            Err(err) => err,
        };
        assert_eq!(err.0, axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn nodes_list_forbidden_without_view_caps() {
        let user = crate::test_support::test_user_with_caps(&[]);
        let result =
            nodes::list_nodes(axum::extract::State(state()), crate::auth::AuthUser(user)).await;
        let err = match result {
            Ok(_) => panic!("expected forbidden"),
            Err(err) => err,
        };
        assert_eq!(err.0, axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn setup_credentials_forbidden_without_view_caps() {
        let user = crate::test_support::test_user_with_caps(&[]);
        let result =
            setup::list_credentials(axum::extract::State(state()), crate::auth::AuthUser(user))
                .await;
        let err = match result {
            Ok(_) => panic!("expected forbidden"),
            Err(err) => err,
        };
        assert_eq!(err.0, axum::http::StatusCode::FORBIDDEN);
    }
}
