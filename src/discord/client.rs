use anyhow::Result;
use tokio::sync::mpsc;
use twilight_gateway::{EventTypeFlags, Intents, Shard, ShardId};
use twilight_gateway::StreamExt as _;
use twilight_http::Client as HttpClient;
use twilight_model::id::Id;

use super::events::{translate_event, DiscordEvent};

pub fn create_http_client(token: &str) -> HttpClient {
    HttpClient::builder()
        .token(token.to_string())
        .build()
}

pub fn required_intents() -> Intents {
    Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MEMBERS
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT
}

/// Try to manually extract user/guild info from a raw READY JSON payload
/// that twilight can't deserialize (user account Ready has extra fields).
fn try_parse_ready_from_raw(json: &str) -> Option<DiscordEvent> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let d = v.get("d")?;

    let user = d.get("user")?;
    let user_id_str = user.get("id")?.as_str()?;
    let user_id: Id<twilight_model::id::marker::UserMarker> =
        Id::new(user_id_str.parse().ok()?);
    let username = user
        .get("global_name")
        .and_then(|v| v.as_str())
        .or_else(|| user.get("username").and_then(|v| v.as_str()))?
        .to_string();

    let guilds: Vec<(Id<twilight_model::id::marker::GuildMarker>, String)> = d
        .get("guilds")
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| {
                    let id = g.get("id")?.as_str()?.parse().ok()?;
                    let name = g
                        .get("properties")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .or_else(|| g.get("name").and_then(|n| n.as_str()))
                        .unwrap_or("Unknown")
                        .to_string();
                    Some((Id::new(id), name))
                })
                .collect()
        })
        .unwrap_or_default();

    let session_id = d.get("session_id")?.as_str()?.to_string();
    let resume_url = d
        .get("resume_gateway_url")
        .and_then(|v| v.as_str())
        .unwrap_or("wss://gateway.discord.gg")
        .to_string();

    Some(DiscordEvent::UserReady {
        user_id,
        username,
        guilds,
        session_id,
        resume_url,
    })
}

pub async fn run_gateway(
    token: String,
    event_tx: mpsc::UnboundedSender<DiscordEvent>,
) -> Result<()> {
    let intents = required_intents();
    tracing::info!("connecting to discord gateway...");
    let mut shard = Shard::new(ShardId::ONE, token, intents);
    tracing::info!("shard created, starting event loop");

    loop {
        match shard.next_event(EventTypeFlags::all()).await {
            Some(Ok(event)) => {
                if let Some(discord_event) = translate_event(event) {
                    tracing::debug!("discord event: {:?}", std::mem::discriminant(&discord_event));
                    let _ = event_tx.send(discord_event);
                }
            }
            Some(Err(e)) => {
                let err_str = e.to_string();
                // Check if this is a failed READY parse - extract data manually
                if err_str.contains("\"t\":\"READY\"") || err_str.contains("t\":\"READY") {
                    tracing::info!("parsing user-account READY event manually");
                    // The error message contains the raw JSON - extract it
                    if let Some(start) = err_str.find("event={") {
                        let raw = &err_str[start + 6..];
                        if let Some(evt) = try_parse_ready_from_raw(raw) {
                            let _ = event_tx.send(evt);
                        } else {
                            tracing::error!("failed to manually parse READY");
                        }
                    }
                } else {
                    tracing::debug!("skipping unparseable gateway event: {e}");
                }
                continue;
            }
            None => {
                tracing::warn!("gateway stream ended");
                let _ = event_tx.send(DiscordEvent::GatewayDisconnect);
                break;
            }
        }
    }

    Ok(())
}
