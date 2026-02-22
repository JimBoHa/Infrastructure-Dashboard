use anyhow::{bail, Context, Result};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use reqwest::blocking::Client;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tower_http::services::ServeFile;

use crate::cli::{NetbootArgs, NetbootCommands, NetbootPrepareArgs, NetbootServeArgs};

#[derive(Clone)]
struct NetbootState {
    root: PathBuf,
    http_path: String,
}

fn validate_http_path(value: &str) -> Result<String> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        bail!("http_path cannot be empty");
    }
    if trimmed.contains("..") || trimmed.contains('\\') {
        bail!("http_path must not contain '..' or backslashes");
    }
    Ok(trimmed.to_string())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))
}

fn download_to(client: &Client, url: &str, dest: &Path, force: bool) -> Result<()> {
    if dest.exists() {
        if force {
            fs::remove_file(dest).ok();
        } else {
            bail!(
                "Refusing to overwrite existing file: {} (use --force)",
                dest.display()
            );
        }
    }
    ensure_parent_dir(dest)?;

    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("Failed to download {url}"))?;
    if !response.status().is_success() {
        bail!("Download failed for {} ({})", url, response.status());
    }
    let mut file = fs::File::create(dest)?;
    if let Err(err) = io::copy(&mut response, &mut file) {
        let _ = fs::remove_file(dest);
        return Err(err.into());
    }
    Ok(())
}

fn prepare(args: NetbootPrepareArgs) -> Result<()> {
    let http_path = validate_http_path(&args.http_path)?;
    let root = args.output;
    fs::create_dir_all(&root).with_context(|| format!("Failed to create {}", root.display()))?;

    let client = Client::builder()
        .user_agent("farmctl-netboot")
        .build()
        .context("Failed to build HTTP client")?;

    let boot_img_dest = root.join(&http_path).join("boot.img");
    let boot_sig_dest = root.join(&http_path).join("boot.sig");
    let repo_dest = root.join("os_list_imagingutility_v4.json");

    download_to(&client, &args.boot_img_url, &boot_img_dest, args.force)?;
    download_to(&client, &args.boot_sig_url, &boot_sig_dest, args.force)?;
    download_to(&client, &args.imager_repo_url, &repo_dest, args.force)?;

    println!(
        "Netboot artifacts written:\n- {}\n- {}\n- {}",
        boot_img_dest.display(),
        boot_sig_dest.display(),
        repo_dest.display()
    );
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn index(State(state): State<NetbootState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "root": state.root.display().to_string(),
        "boot_img_path": format!("/{}/boot.img", state.http_path),
        "boot_sig_path": format!("/{}/boot.sig", state.http_path),
        "imager_repo_path": "/os_list_imagingutility_v4.json",
        "notes": [
            "This server intentionally serves only static netboot artifacts.",
            "It does not provide DHCP/TFTP; Pi bootloader EEPROM must point HTTP_HOST/HTTP_PORT/HTTP_PATH at this server.",
            "Changing HTTP_HOST may disable HTTPS in the Pi bootloader; use an isolated provisioning network or configure HTTPS with a custom CA per Raspberry Pi docs."
        ]
    }))
}

fn router(state: NetbootState) -> Router {
    let boot_img_path = format!("/{}/boot.img", state.http_path);
    let boot_sig_path = format!("/{}/boot.sig", state.http_path);

    let boot_img = ServeFile::new(state.root.join(&state.http_path).join("boot.img"));
    let boot_sig = ServeFile::new(state.root.join(&state.http_path).join("boot.sig"));
    let os_list = ServeFile::new(state.root.join("os_list_imagingutility_v4.json"));

    Router::new()
        .route("/", get(index))
        .route("/healthz", get(health))
        .route_service(boot_img_path.as_str(), boot_img)
        .route_service(boot_sig_path.as_str(), boot_sig)
        .route_service("/os_list_imagingutility_v4.json", os_list)
        .with_state(state)
}

async fn serve(args: NetbootServeArgs) -> Result<()> {
    let http_path = validate_http_path(&args.http_path)?;
    let addr: SocketAddr = format!("{}:{}", args.host.trim(), args.port)
        .parse()
        .context("Invalid host/port")?;

    let state = NetbootState {
        root: args.root,
        http_path,
    };

    let app = router(state.clone());
    println!("farmctl netboot server listening on http://{}", addr);
    println!("  - /{}/boot.img", state.http_path);
    println!("  - /{}/boot.sig", state.http_path);
    println!("  - /os_list_imagingutility_v4.json");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind {addr}"))?;
    axum::serve(listener, app)
        .await
        .context("Netboot server crashed")?;
    Ok(())
}

pub async fn handle(args: NetbootArgs) -> Result<()> {
    match args.command {
        NetbootCommands::Prepare(args) => prepare(args),
        NetbootCommands::Serve(args) => serve(args).await,
    }
}

#[cfg(test)]
mod tests {
    use super::validate_http_path;

    #[test]
    fn http_path_rejects_empty() {
        assert!(validate_http_path("").is_err());
    }

    #[test]
    fn http_path_trims_slashes() {
        assert_eq!(validate_http_path("/net_install/").unwrap(), "net_install");
    }

    #[test]
    fn http_path_rejects_traversal() {
        assert!(validate_http_path("../net_install").is_err());
        assert!(validate_http_path("net_install/..").is_err());
    }
}
