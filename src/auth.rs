use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "tiscord";
const USERNAME: &str = "discord_token";

pub fn get_token() -> Result<String> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    match entry.get_password() {
        Ok(token) => Ok(token),
        Err(keyring::Error::NoEntry) => {
            let token = prompt_token()?;
            entry.set_password(&token)?;
            Ok(token)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn clear_token() -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    entry.delete_credential().context("failed to delete token")
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
