//! Encrypted token file storage using AES-256-GCM + PBKDF2.
//!
//! Used as a fallback when the OS keyring is unavailable (e.g. Windows
//! Credential Manager issues). The token is encrypted with a key derived
//! from machine-specific material (hostname + OS username) via PBKDF2
//! with 600,000 iterations. A random salt and nonce are stored alongside
//! the ciphertext.
//!
//! File format: salt (32 bytes) || nonce (12 bytes) || ciphertext (variable)

use std::fs;
use std::path::PathBuf;

use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};

const PBKDF2_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12; // AES-256-GCM standard nonce
const KEY_LEN: usize = 32;   // 256 bits

static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;

fn token_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tiscord").join(".token.enc"))
}

/// Derive a machine-specific password for key derivation.
/// Uses hostname + username from environment as entropy so the file
/// can only be decrypted on the same machine by the same user.
fn machine_password() -> Vec<u8> {
    // COMPUTERNAME on Windows, HOSTNAME on Linux, fallback for macOS
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "tiscord-host".to_string());
    // USERNAME on Windows, USER on Unix
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "tiscord-user".to_string());
    format!("tiscord:{}:{}", hostname, username).into_bytes()
}

/// Derive an AES-256 key from the machine password and a random salt.
fn derive_key(password: &[u8], salt: &[u8]) -> Vec<u8> {
    let mut key = vec![0u8; KEY_LEN];
    pbkdf2::derive(
        PBKDF2_ALG,
        std::num::NonZeroU32::new(PBKDF2_ITERATIONS).unwrap(),
        salt,
        password,
        &mut key,
    );
    key
}

/// Encrypt a token and write it to the token file.
pub fn save_token(token: &str) -> Result<(), String> {
    let path = token_file_path().ok_or("could not determine config directory")?;

    // Ensure parent dir exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
    }

    let rng = SystemRandom::new();
    let password = machine_password();

    // Generate random salt and nonce
    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt).map_err(|_| "failed to generate salt")?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes).map_err(|_| "failed to generate nonce")?;

    // Derive key
    let key_bytes = derive_key(&password, &salt);
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|_| "failed to create encryption key")?;
    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    // Encrypt (in-place)
    let mut ciphertext = token.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut ciphertext)
        .map_err(|_| "encryption failed")?;

    // Write: salt || nonce || ciphertext
    let mut file_data = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    file_data.extend_from_slice(&salt);
    file_data.extend_from_slice(&nonce_bytes);
    file_data.extend_from_slice(&ciphertext);

    fs::write(&path, &file_data).map_err(|e| format!("write token file: {e}"))?;

    // Restrict file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(&path, perms);
    }

    Ok(())
}

/// Read and decrypt the token from the token file.
pub fn load_token() -> Result<String, String> {
    let path = token_file_path().ok_or("could not determine config directory")?;
    let data = fs::read(&path).map_err(|e| format!("read token file: {e}"))?;

    if data.len() < SALT_LEN + NONCE_LEN + AES_256_GCM.tag_len() {
        return Err("token file too short / corrupted".to_string());
    }

    let salt = &data[..SALT_LEN];
    let nonce_bytes: [u8; NONCE_LEN] = data[SALT_LEN..SALT_LEN + NONCE_LEN]
        .try_into()
        .map_err(|_| "invalid nonce")?;
    let mut ciphertext = data[SALT_LEN + NONCE_LEN..].to_vec();

    let password = machine_password();
    let key_bytes = derive_key(&password, salt);
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|_| "failed to create decryption key")?;
    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let plaintext = key
        .open_in_place(nonce, Aad::empty(), &mut ciphertext)
        .map_err(|_| "decryption failed — token file may be corrupted or from a different machine")?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|_| "decrypted token is not valid UTF-8".to_string())
}

/// Delete the encrypted token file.
pub fn delete_token() -> Result<(), String> {
    let path = token_file_path().ok_or("could not determine config directory")?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("delete token file: {e}"))?;
    }
    Ok(())
}
