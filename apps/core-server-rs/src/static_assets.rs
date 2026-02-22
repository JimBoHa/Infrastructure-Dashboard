use anyhow::Result;
use axum::extract::Request;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::middleware::{from_fn, Next};
use axum::response::Html;
use axum::response::Response;
use axum::routing::get_service;
use axum::{routing::get, Router};
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};

const DEFAULT_INDEX: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Farm Dashboard</title>
  </head>
  <body style="font-family: system-ui; padding: 24px">
    <h1>Farm Dashboard (Rust core-server)</h1>
    <p>This is a placeholder static root. Provide <code>--static-root</code> to serve the real dashboard build.</p>
  </body>
</html>
"#;

async fn apply_cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let mut response = next.run(req).await;

    if response.headers().contains_key(CACHE_CONTROL) {
        return response;
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let cache_value = if path.starts_with("/_next/static/") {
        // Next.js fingerprints these assets, so they are safe to cache indefinitely.
        "public, max-age=31536000, immutable"
    } else if content_type.starts_with("text/html") {
        // Prevent stale HTML shells/JS manifests from being cached on mobile browsers after upgrades.
        "no-store"
    } else {
        // Other static assets (favicon, vendor JS) can be cached, but we still want upgrades to propagate quickly.
        "public, max-age=86400"
    };

    if let Ok(value) = HeaderValue::from_str(cache_value) {
        response.headers_mut().insert(CACHE_CONTROL, value);
    }

    response
}

pub fn service(static_root: Option<PathBuf>) -> Result<Router> {
    let router = if let Some(root) = static_root {
        if !root.exists() {
            anyhow::bail!("static_root not found at {}", root.display());
        }
        let index = root.join("index.html");
        let dir = ServeDir::new(root)
            .append_index_html_on_directories(true)
            .not_found_service(ServeFile::new(index));
        Router::new()
            .fallback_service(get_service(dir))
            .layer(from_fn(apply_cache_headers))
    } else {
        async fn placeholder_handler() -> Html<&'static str> {
            Html(DEFAULT_INDEX)
        }

        Router::new()
            .route("/", get(placeholder_handler))
            .fallback(get(placeholder_handler))
    };
    Ok(router)
}
