use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;

const HASH_PREFIX: &str = "pbkdf2_sha256";
const DEFAULT_ITERATIONS: u32 = 200_000;
const SALT_BYTES: usize = 16;
const DERIVED_BYTES: usize = 32;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    hash_password_with_iterations(password, DEFAULT_ITERATIONS)
}

pub fn hash_password_with_iterations(password: &str, iterations: u32) -> anyhow::Result<String> {
    let trimmed = password.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Password cannot be blank");
    }

    let mut salt = [0u8; SALT_BYTES];
    OsRng.fill_bytes(&mut salt);
    let derived = derive(trimmed.as_bytes(), &salt, iterations);

    Ok(format!(
        "{}${}${}${}",
        HASH_PREFIX,
        iterations,
        STANDARD_NO_PAD.encode(salt),
        STANDARD_NO_PAD.encode(derived)
    ))
}

pub fn verify_password(password: &str, password_hash: &str) -> bool {
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
