use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use postgres::{Client, NoTls};
use postgres::types::Json;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
#[cfg(test)]
use subtle::ConstantTimeEq;

use crate::config::postgres_connection_string;
use crate::config::SetupConfig;

const HASH_PREFIX: &str = "pbkdf2_sha256";
const DEFAULT_ITERATIONS: u32 = 200_000;
const SALT_BYTES: usize = 16;
const DERIVED_BYTES: usize = 32;

pub(crate) const BOOTSTRAP_ADMIN_EMAIL: &str = "admin@farmdashboard.local";
const BOOTSTRAP_ADMIN_NAME: &str = "Admin";
const BOOTSTRAP_ADMIN_ROLE: &str = "admin";

const BOOTSTRAP_ADMIN_CAPABILITIES: &[&str] = &[
    "nodes.view",
    "sensors.view",
    "outputs.view",
    "schedules.view",
    "metrics.view",
    "backups.view",
    "setup.credentials.view",
    "config.write",
    "users.manage",
    "schedules.write",
    "outputs.command",
    "alerts.view",
    "alerts.ack",
    "analytics.view",
];

pub(crate) struct BootstrapAdminCredentials {
    pub email: String,
    pub password: String,
}

pub(crate) fn ensure_bootstrap_admin(
    config: &SetupConfig,
) -> Result<Option<BootstrapAdminCredentials>> {
    let mut client = Client::connect(&postgres_connection_string(&config.database_url), NoTls)
        .context("Failed to connect to Postgres for bootstrap admin")?;

    let exists: bool = client
        .query_one("SELECT EXISTS(SELECT 1 FROM users)", &[])
        .context("Failed to check for existing users")?
        .get(0);
    if exists {
        return Ok(None);
    }

    let password = generate_temp_password();
    let password_hash = hash_password(&password)?;
    let capabilities_json =
        serde_json::json!(BOOTSTRAP_ADMIN_CAPABILITIES);

    client
        .execute(
            r#"
            INSERT INTO users (name, email, role, capabilities, password_hash)
            VALUES ($1, $2, $3, $4::jsonb, $5)
            "#,
            &[
                &BOOTSTRAP_ADMIN_NAME,
                &BOOTSTRAP_ADMIN_EMAIL,
                &BOOTSTRAP_ADMIN_ROLE,
                &Json(&capabilities_json),
                &password_hash,
            ],
        )
        .context("Failed to create bootstrap admin user")?;

    Ok(Some(BootstrapAdminCredentials {
        email: BOOTSTRAP_ADMIN_EMAIL.to_string(),
        password,
    }))
}

fn generate_temp_password() -> String {
    let mut bytes = [0u8; 12];
    OsRng.fill_bytes(&mut bytes);
    format!("FD-{}", hex::encode(bytes))
}

fn hash_password(password: &str) -> Result<String> {
    let trimmed = password.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Password cannot be blank");
    }

    let mut salt = [0u8; SALT_BYTES];
    OsRng.fill_bytes(&mut salt);
    let derived = derive(trimmed.as_bytes(), &salt, DEFAULT_ITERATIONS);
    Ok(format!(
        "{}${}${}${}",
        HASH_PREFIX,
        DEFAULT_ITERATIONS,
        STANDARD_NO_PAD.encode(salt),
        STANDARD_NO_PAD.encode(derived)
    ))
}

#[cfg(test)]
fn verify_password(password: &str, password_hash: &str) -> bool {
    let trimmed = password.trim();
    if trimmed.is_empty() {
        return false;
    }

    let mut parts = password_hash.splitn(4, '$');
    let prefix = parts.next().unwrap_or("");
    let iterations_text = parts.next().unwrap_or("");
    let salt_b64 = parts.next().unwrap_or("");
    let hash_b64 = parts.next().unwrap_or("");
    if prefix != HASH_PREFIX {
        return false;
    }
    let iterations: u32 = match iterations_text.parse() {
        Ok(value) => value,
        Err(_) => return false,
    };

    let salt = match STANDARD_NO_PAD.decode(salt_b64) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let expected = match STANDARD_NO_PAD.decode(hash_b64) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let derived = derive(trimmed.as_bytes(), &salt, iterations);
    derived.ct_eq(expected.as_slice()).into()
}

fn derive(password: &[u8], salt: &[u8], iterations: u32) -> [u8; DERIVED_BYTES] {
    let mut out = [0u8; DERIVED_BYTES];
    pbkdf2_hmac::<Sha256>(password, salt, iterations, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_round_trip() {
        let password = "SmokeTest!123";
        let hash = hash_password(password).expect("hash_password");
        assert!(hash.starts_with("pbkdf2_sha256$"));
        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong-password", &hash));
    }
}
