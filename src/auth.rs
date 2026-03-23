use anyhow::{Context, Result};
use keyring::Entry;

use crate::token_file;

const SERVICE: &str = "tiscord";
const USERNAME: &str = "discord_token";

pub fn get_token() -> Result<String> {
    // Try OS keyring first
    match try_keyring_get() {
        Ok(token) => return Ok(token),
        Err(e) => tracing::debug!("keyring unavailable, trying encrypted file: {e}"),
    }

    // Try encrypted file fallback
    match token_file::load_token() {
        Ok(token) => {
            tracing::info!("loaded token from encrypted file");
            return Ok(token);
        }
        Err(e) => tracing::debug!("encrypted file not found or failed: {e}"),
    }

    // No stored token — prompt and save to both backends
    let token = prompt_token()?;
    save_to_backends(&token);
    Ok(token)
}

pub fn clear_token() -> Result<()> {
    // Clear from both backends
    if let Ok(entry) = Entry::new(SERVICE, USERNAME) {
        let _ = entry.delete_credential();
    }
    let _ = token_file::delete_token();
    eprintln!("Token cleared.");
    Ok(())
}

/// Try to get token from OS keyring.
fn try_keyring_get() -> Result<String> {
    let entry = Entry::new(SERVICE, USERNAME).context("keyring init failed")?;
    match entry.get_password() {
        Ok(token) => Ok(token),
        Err(keyring::Error::NoEntry) => anyhow::bail!("no keyring entry"),
        Err(e) => anyhow::bail!("keyring error: {e}"),
    }
}

/// Save token to all available backends (best-effort).
fn save_to_backends(token: &str) {
    // Try keyring
    if let Ok(entry) = Entry::new(SERVICE, USERNAME) {
        match entry.set_password(token) {
            Ok(()) => tracing::info!("token saved to OS keyring"),
            Err(e) => tracing::warn!("failed to save to keyring: {e}"),
        }
    }
    // Always save encrypted file as fallback
    match token_file::save_token(token) {
        Ok(()) => tracing::info!("token saved to encrypted file"),
        Err(e) => tracing::warn!("failed to save encrypted file: {e}"),
    }
}

fn prompt_token() -> Result<String> {
    eprintln!("No Discord token found. Enter your token:");
    let mut token = String::new();
    std::io::stdin().read_line(&mut token)?;
    let token = token.trim().replace('\r', "").replace('\n', "");
    if token.is_empty() {
        anyhow::bail!("token cannot be empty");
    }
    eprintln!("Token stored ({} chars)", token.len());
    Ok(token)
}
