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

    let guilds: Vec<super::events::ReadyGuild> = d
        .get("guilds")
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|g| {
                    // Skip guild folders — they have a guild_ids array instead of real guild data
                    if g.get("guild_ids").is_some() {
                        return None;
                    }
                    let id: u64 = g.get("id")?.as_str()?.parse().ok()?;
                    let name = g
                        .get("properties")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .or_else(|| g.get("name").and_then(|n| n.as_str()))
                        .unwrap_or("Unknown")
                        .to_string();

                    // Parse channels from the guild object
                    let channels: Vec<super::events::ReadyChannel> = g
                        .get("channels")
                        .and_then(|c| c.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|ch| {
                                    let ch_id: u64 = ch.get("id")?.as_str()?.parse().ok()?;
                                    let ch_name = ch.get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let kind = ch.get("type")
                                        .and_then(|t| t.as_u64())
                                        .unwrap_or(0) as u8;
                                    let parent_id: Option<Id<twilight_model::id::marker::ChannelMarker>> = ch
                                        .get("parent_id")
                                        .and_then(|p| p.as_str())
                                        .and_then(|s| s.parse().ok())
                                        .map(Id::new);
                                    let position = ch.get("position")
                                        .and_then(|p| p.as_i64())
                                        .unwrap_or(0) as i32;
                                    Some(super::events::ReadyChannel {
                                        id: Id::new(ch_id),
                                        name: ch_name,
                                        kind,
                                        parent_id,
                                        position,
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    // Parse members from the guild object
                    let members: Vec<super::events::ReadyMember> = g
                        .get("members")
                        .and_then(|m| m.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| {
                                    let user = m.get("user")?;
                                    let uid: u64 = user.get("id")?.as_str()?.parse().ok()?;
                                    let username = user.get("global_name")
                                        .and_then(|n| n.as_str())
                                        .or_else(|| user.get("username").and_then(|n| n.as_str()))
                                        .unwrap_or("Unknown")
                                        .to_string();
                                    let nickname = m.get("nick")
                                        .and_then(|n| n.as_str())
                                        .map(|s| s.to_string());
                                    Some(super::events::ReadyMember {
                                        user_id: Id::new(uid),
                                        username,
                                        nickname,
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    Some(super::events::ReadyGuild {
                        id: Id::new(id),
                        name,
                        channels,
                        members,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Parse guild folders
    let guild_folders: Vec<crate::store::GuildFolder> = d
        .get("guild_folders")
        .and_then(|gf| gf.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let guild_ids: Vec<Id<twilight_model::id::marker::GuildMarker>> = f
                        .get("guild_ids")
                        .and_then(|ids| ids.as_array())
                        .map(|ids| {
                            ids.iter()
                                .filter_map(|id| {
                                    id.as_str()?.parse().ok().map(Id::new)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if guild_ids.is_empty() {
                        return None;
                    }
                    let name = f.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
                    let color = f.get("color").and_then(|c| c.as_u64()).map(|c| c as u32);
                    Some(crate::store::GuildFolder {
                        name,
                        color,
                        guild_ids,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Parse DM channels from private_channels array
    let dm_channels: Vec<(Id<twilight_model::id::marker::ChannelMarker>, Vec<String>)> = d
        .get("private_channels")
        .and_then(|pc| pc.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|ch| {
                    let id: u64 = ch.get("id")?.as_str()?.parse().ok()?;
                    // User account Ready may have full recipient objects or just recipient_ids
                    let recipients: Vec<String> = ch
                        .get("recipients")
                        .and_then(|r| r.as_array())
                        .map(|users| {
                            users.iter()
                                .filter_map(|u| {
                                    u.get("global_name")
                                        .and_then(|n| n.as_str())
                                        .or_else(|| u.get("username").and_then(|n| n.as_str()))
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    // If no full recipients, try recipient_ids as fallback display
                    let recipients = if recipients.is_empty() {
                        ch.get("recipient_ids")
                            .and_then(|r| r.as_array())
                            .map(|ids| {
                                ids.iter()
                                    .filter_map(|id| id.as_str().map(|s| format!("User {}", s)))
                                    .collect()
                            })
                            .unwrap_or_default()
                    } else {
                        recipients
                    };
                    Some((Id::new(id), recipients))
                })
                .collect()
        })
        .unwrap_or_default();
    tracing::info!("parsed {} DM channels from Ready", dm_channels.len());

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
        guild_folders,
        dm_channels,
        session_id,
        resume_url,
    })
}

pub async fn run_gateway(
    token: String,
    event_tx: mpsc::UnboundedSender<DiscordEvent>,
) -> Result<()> {
    let intents = required_intents();
    let mut attempt = 0u32;

    loop {
        if attempt > 0 {
            // Signal reconnecting status to the UI.
            let _ = event_tx.send(DiscordEvent::GatewayReconnect);
            tracing::info!("reconnecting to discord gateway (attempt {})...", attempt);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        } else {
            tracing::info!("connecting to discord gateway...");
        }

        let mut shard = Shard::new(ShardId::ONE, token.clone(), intents);
        tracing::info!("shard created, starting event loop");

        'inner: loop {
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
                    continue 'inner;
                }
                None => {
                    tracing::warn!("gateway stream ended, will reconnect in 5 seconds");
                    let _ = event_tx.send(DiscordEvent::GatewayDisconnect);
                    break 'inner;
                }
            }
        }

        attempt += 1;
        // If the sender is closed (app exited), stop reconnecting.
        if event_tx.is_closed() {
            break;
        }
    }

    Ok(())
}
