use anyhow::{anyhow, Result};
use base64::Engine;
use chrono::Utc;
use rand::RngCore;
use sqlx::types::Json as SqlJson;
use sqlx::{PgPool, Postgres, Transaction};

use super::types::DeploymentUserRef;

pub(super) async fn find_registered_node(
    db: &PgPool,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
) -> Result<Option<String>> {
    if mac_eth.is_none() && mac_wifi.is_none() {
        return Ok(None);
    }
    let record: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT id::text as id, name
        FROM nodes
        WHERE ($1::macaddr IS NOT NULL AND mac_eth = $1::macaddr)
           OR ($2::macaddr IS NOT NULL AND mac_wifi = $2::macaddr)
        LIMIT 1
        "#,
    )
    .bind(mac_eth)
    .bind(mac_wifi)
    .fetch_optional(db)
    .await?;
    Ok(record.map(|(id, name)| format!("{name} ({id})")))
}

pub(super) async fn issue_adoption_token(
    db: &PgPool,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
    user: &DeploymentUserRef,
) -> Result<String> {
    let mac_eth = normalize_mac_opt(mac_eth);
    let mac_wifi = normalize_mac_opt(mac_wifi);

    if mac_eth.is_none() && mac_wifi.is_none() {
        return Err(anyhow!(
            "Unable to determine MAC address for adoption token."
        ));
    }
    let expires_at = Utc::now() + chrono::Duration::seconds(900);
    let metadata = serde_json::json!({
        "issued_by": user.id,
        "issued_by_email": user.email,
        "issued_by_role": user.role,
        "source": "pi5-remote-deploy",
    });

    let mut tx = db.begin().await?;

    lock_adoption_token_macs(&mut tx, mac_eth.as_deref(), mac_wifi.as_deref()).await?;
    cleanup_unused_tokens(&mut tx, mac_eth.as_deref(), mac_wifi.as_deref(), None).await?;

    let service_name = "pi5-remote-deploy";
    let mut generated: Option<String> = None;
    for _ in 0..5 {
        let candidate = generate_token();
        let inserted = sqlx::query(
            r#"
            INSERT INTO adoption_tokens (token, mac_eth, mac_wifi, service_name, metadata, expires_at)
            VALUES ($1, $2::macaddr, $3::macaddr, $4, $5, $6)
            "#,
        )
        .bind(&candidate)
        .bind(mac_eth.as_deref())
        .bind(mac_wifi.as_deref())
        .bind(service_name)
        .bind(SqlJson(metadata.clone()))
        .bind(expires_at)
        .execute(&mut *tx)
        .await;

        match inserted {
            Ok(_) => {
                generated = Some(candidate);
                break;
            }
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
                continue;
            }
            Err(err) => return Err(err.into()),
        }
    }

    let token = generated.ok_or_else(|| anyhow!("Failed to issue unique adoption token"))?;
    cleanup_unused_tokens(
        &mut tx,
        mac_eth.as_deref(),
        mac_wifi.as_deref(),
        Some(token.as_str()),
    )
    .await?;

    tx.commit().await?;
    Ok(token)
}

async fn cleanup_unused_tokens(
    tx: &mut Transaction<'_, Postgres>,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
    preserve_token: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM adoption_tokens
        WHERE used_at IS NULL
          AND expires_at <= NOW()
        "#,
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM adoption_tokens
        WHERE used_at IS NULL
          AND (
            ($1::macaddr IS NOT NULL AND mac_eth = $1::macaddr)
            OR ($2::macaddr IS NOT NULL AND mac_wifi = $2::macaddr)
          )
          AND ($3::text IS NULL OR token <> $3::text)
        "#,
    )
    .bind(mac_eth)
    .bind(mac_wifi)
    .bind(preserve_token)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn lock_adoption_token_macs(
    tx: &mut Transaction<'_, Postgres>,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
) -> Result<(), sqlx::Error> {
    let mut macs: Vec<&str> = Vec::new();
    if let Some(mac) = mac_eth {
        macs.push(mac);
    }
    if let Some(mac) = mac_wifi {
        macs.push(mac);
    }
    macs.sort_unstable();
    macs.dedup();

    for mac in macs {
        let key = advisory_lock_key("adoption_tokens", mac);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn advisory_lock_key(namespace: &str, value: &str) -> i64 {
    fn fnv1a_64(input: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in input.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    let combined = format!("{namespace}:{value}");
    fnv1a_64(&combined) as i64
}

fn normalize_mac_opt(value: Option<&str>) -> Option<String> {
    value.map(normalize_mac).filter(|value| !value.is_empty())
}

fn normalize_mac(value: &str) -> String {
    value.trim().to_lowercase()
}

fn generate_token() -> String {
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
