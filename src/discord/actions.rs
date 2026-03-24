use tokio::sync::mpsc;
use twilight_http::Client as HttpClient;
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker};
use twilight_model::id::Id;

use super::events::DiscordEvent;
use crate::tui::keybindings::KeyAction;

#[derive(Debug, Clone)]
pub enum Action {
    SendMessage {
        channel_id: Id<ChannelMarker>,
        content: String,
        reply_to: Option<Id<MessageMarker>>,
    },
    EditMessage {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        content: String,
    },
    DeleteMessage {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    FetchMessages {
        channel_id: Id<ChannelMarker>,
        before: Option<Id<MessageMarker>>,
        limit: u16,
    },
    FetchGuildMembers {
        guild_id: Id<GuildMarker>,
    },
    FetchGuildChannels {
        guild_id: Id<GuildMarker>,
    },
    AddReaction {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        /// Unicode emoji string (e.g. "👍") or custom emoji in "name:id" format.
        emoji: String,
    },
    RemoveReaction {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        /// Unicode emoji string (e.g. "👍") or custom emoji in "name:id" format.
        emoji: String,
    },
    FetchPinnedMessages {
        channel_id: Id<ChannelMarker>,
    },
    PinMessage {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    UnpinMessage {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    /// Search messages in a channel or guild.
    /// NOTE: Discord's search API is not exposed by twilight-http for user accounts.
    /// This dispatches a stub that returns empty results with a TODO for real implementation.
    SearchMessages {
        scope: crate::store::search::SearchScope,
        query: String,
    },
    /// Navigate to a specific message in a channel.
    NavigateToSearchResult {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    /// Open a thread view by pushing a Thread pane onto the navigation stack.
    OpenThread {
        parent_channel: Id<ChannelMarker>,
        thread_id: Id<ChannelMarker>,
    },
    /// Fetch a user profile from the REST API and emit UserProfileLoaded.
    FetchUserProfile {
        user_id: Id<UserMarker>,
    },
    /// Fetch an image attachment and emit ImageLoaded once encoded.
    /// TODO: actual HTTP fetch + encoding requires the 'image' and 'base64' crates.
    FetchImage {
        url: String,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    /// Fetch DM channels via REST (GET /users/@me/channels).
    FetchDmChannels,
    /// Send a typing indicator for the given channel (POST /channels/{id}/typing).
    SendTyping {
        channel_id: Id<ChannelMarker>,
    },
    /// Upload a file to a Discord channel with an optional text message.
    UploadFile {
        channel_id: Id<ChannelMarker>,
        file_path: String,
        message: Option<String>,
    },
    /// Update the current user's own presence/status.
    /// NOTE: Twilight does not expose a gateway presence update for user accounts.
    /// The status is tracked locally in UiState; no gateway command is issued.
    SetStatus {
        status: String,
    },
    /// Set the current user's custom status (emoji + text).
    /// NOTE: This requires sending a gateway opcode 3 UPDATE_PRESENCE with a custom activity.
    /// Twilight-gateway does not expose this for user accounts; the custom status is stored
    /// locally in UiState and displayed in the status bar.
    SetCustomStatus {
        emoji: Option<String>,
        text: Option<String>,
    },
    /// Internal action used by components to request cross-component coordination.
    /// Intercepted by App before reaching the action handler.
    ComponentKeyAction(KeyAction),
}

pub async fn run_action_handler(
    http: HttpClient,
    mut action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<DiscordEvent>,
    token: String,
) {
    while let Some(action) = action_rx.recv().await {
        match action {
            Action::SendMessage {
                channel_id,
                content,
                reply_to,
            } => {
                let mut req = http.create_message(channel_id).content(&content);
                if let Some(reply_id) = reply_to {
                    req = req.reply(reply_id);
                }
                if let Err(e) = req.await {
                    tracing::error!("failed to send message: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to send message".to_string(),
                    });
                }
            }
            Action::EditMessage {
                channel_id,
                message_id,
                content,
            } => {
                if let Err(e) = http
                    .update_message(channel_id, message_id)
                    .content(Some(&content))
                    .await
                {
                    tracing::error!("failed to edit message: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to edit message".to_string(),
                    });
                }
            }
            Action::DeleteMessage {
                channel_id,
                message_id,
            } => {
                if let Err(e) = http.delete_message(channel_id, message_id).await {
                    tracing::error!("failed to delete message: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to delete message".to_string(),
                    });
                }
            }
            Action::FetchMessages {
                channel_id,
                before,
                limit,
            } => {
                let result = if let Some(before_id) = before {
                    http.channel_messages(channel_id)
                        .before(before_id)
                        .limit(limit)
                        .await
                } else {
                    http.channel_messages(channel_id).limit(limit).await
                };

                match result {
                    Ok(response) => match response.models().await {
                        Ok(messages) => {
                            let _ = event_tx.send(DiscordEvent::MessagesLoaded {
                                channel_id,
                                messages,
                            });
                        }
                        Err(e) => {
                            tracing::error!("failed to deserialize messages: {e}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Failed to fetch messages".to_string(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("failed to fetch messages: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Failed to fetch messages".to_string(),
                        });
                    }
                }
            }
            Action::FetchGuildChannels { guild_id } => {
                match http.guild_channels(guild_id).await {
                    Ok(response) => match response.models().await {
                        Ok(channels) => {
                            let _ = event_tx.send(DiscordEvent::ChannelsLoaded {
                                guild_id,
                                channels,
                            });
                        }
                        Err(e) => {
                            tracing::error!("failed to deserialize channels: {e}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Failed to load channels".to_string(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("failed to fetch channels: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Failed to load channels".to_string(),
                        });
                    }
                }
            }
            Action::AddReaction {
                channel_id,
                message_id,
                emoji,
            } => {
                let reaction = RequestReactionType::Unicode { name: &emoji };
                if let Err(e) = http.create_reaction(channel_id, message_id, &reaction).await {
                    tracing::error!("failed to add reaction: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to add reaction".to_string(),
                    });
                }
            }
            Action::RemoveReaction {
                channel_id,
                message_id,
                emoji,
            } => {
                let reaction = RequestReactionType::Unicode { name: &emoji };
                if let Err(e) = http
                    .delete_current_user_reaction(channel_id, message_id, &reaction)
                    .await
                {
                    tracing::error!("failed to remove reaction: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to remove reaction".to_string(),
                    });
                }
            }
            Action::SearchMessages { scope, query } => {
                tracing::debug!("search requested: {:?} query={:?}", scope, query);
                let url = match &scope {
                    crate::store::search::SearchScope::CurrentChannel(channel_id) => {
                        format!(
                            "https://discord.com/api/v10/channels/{}/messages/search?content={}",
                            channel_id,
                            urlencoding_encode(&query)
                        )
                    }
                    crate::store::search::SearchScope::Server(guild_id) => {
                        format!(
                            "https://discord.com/api/v10/guilds/{}/messages/search?content={}",
                            guild_id,
                            urlencoding_encode(&query)
                        )
                    }
                };
                let search_client = reqwest::Client::new();
                match search_client
                    .get(&url)
                    .header("Authorization", &token)
                    .send()
                    .await
                {
                    Ok(response) => {
                        let status = response.status();
                        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
                            tracing::warn!("search returned {status}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Search not authorized".to_string(),
                            });
                        } else if !status.is_success() {
                            tracing::warn!("search returned {status}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: format!("Search failed: {status}"),
                            });
                        } else {
                            match response.json::<serde_json::Value>().await {
                                Ok(body) => {
                                    let results = parse_search_results(&body, &scope);
                                    let _ = event_tx.send(DiscordEvent::SearchResults { results });
                                }
                                Err(e) => {
                                    tracing::error!("failed to parse search response: {e}");
                                    let _ = event_tx.send(DiscordEvent::SearchResults { results: Vec::new() });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("search request failed: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Search request failed".to_string(),
                        });
                    }
                }
            }
            Action::NavigateToSearchResult {
                channel_id,
                message_id: _,
            } => {
                // Fetch messages for the target channel so it is loaded; the UI will
                // set selected_channel before dispatching this action.
                let result = http.channel_messages(channel_id).limit(50).await;
                match result {
                    Ok(response) => match response.models().await {
                        Ok(messages) => {
                            let _ = event_tx.send(DiscordEvent::MessagesLoaded {
                                channel_id,
                                messages,
                            });
                        }
                        Err(e) => {
                            tracing::error!("failed to load messages for search nav: {e}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Failed to fetch messages".to_string(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("failed to fetch channel for search nav: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Failed to fetch messages".to_string(),
                        });
                    }
                }
            }
            Action::OpenThread { .. } => {
                // Handled by App before reaching here; ignore if it leaks.
            }
            Action::ComponentKeyAction(_) => {
                // Handled by App before reaching here; ignore if it leaks.
            }
            Action::FetchGuildMembers { guild_id } => {
                match http.guild_members(guild_id).limit(100).await {
                    Ok(response) => match response.models().await {
                        Ok(members) => {
                            let _ = event_tx.send(DiscordEvent::MembersLoaded {
                                guild_id,
                                members,
                            });
                        }
                        Err(e) => {
                            tracing::error!("failed to deserialize members: {e}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Failed to load members".to_string(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("failed to fetch members: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Failed to load members".to_string(),
                        });
                    }
                }
            }
            Action::FetchPinnedMessages { channel_id } => {
                match http.pins(channel_id).await {
                    Ok(response) => match response.model().await {
                        Ok(pins_listing) => {
                            let stored: Vec<crate::store::messages::StoredMessage> = pins_listing
                                .items
                                .into_iter()
                                .map(|pin| {
                                    let msg = pin.message;
                                    crate::store::messages::StoredMessage {
                                        id: msg.id,
                                        author_name: msg.author.name.clone(),
                                        author_id: msg.author.id,
                                        content: msg.content.clone(),
                                        timestamp: msg.timestamp.iso_8601().to_string(),
                                        reply_to: msg.referenced_message.as_ref().map(|r| {
                                            crate::store::messages::ReplyContext {
                                                author_name: r.author.name.clone(),
                                                content_preview: if r.content.len() <= 80 {
                                                    r.content.clone()
                                                } else {
                                                    format!("{}...", &r.content[..80])
                                                },
                                            }
                                        }),
                                        attachments: msg
                                            .attachments
                                            .iter()
                                            .map(|a| crate::store::messages::Attachment {
                                                filename: a.filename.clone(),
                                                size: a.size,
                                                url: a.url.clone(),
                                            })
                                            .collect(),
                                        is_edited: false,
                                        reactions: msg
                                            .reactions
                                            .iter()
                                            .map(|r| {
                                                use twilight_model::channel::message::EmojiReactionType;
                                                crate::store::messages::Reaction {
                                                    emoji: match &r.emoji {
                                                        EmojiReactionType::Unicode { name } => {
                                                            crate::store::messages::ReactionEmoji::Unicode(name.clone())
                                                        }
                                                        EmojiReactionType::Custom { id, name, .. } => {
                                                            crate::store::messages::ReactionEmoji::Custom {
                                                                id: id.get(),
                                                                name: name.clone().unwrap_or_default(),
                                                            }
                                                        }
                                                    },
                                                    count: r.count as u32,
                                                    me: r.me,
                                                }
                                            })
                                            .collect(),
                                        embeds: vec![],
                                        stickers: vec![],
                                    }
                                })
                                .collect();
                            let _ = event_tx.send(DiscordEvent::PinnedMessagesLoaded {
                                channel_id,
                                messages: stored,
                            });
                        }
                        Err(e) => {
                            tracing::error!("failed to deserialize pinned messages: {e}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Failed to fetch pinned messages".to_string(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("failed to fetch pinned messages: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Failed to fetch pinned messages".to_string(),
                        });
                    }
                }
            }
            Action::PinMessage { channel_id, message_id } => {
                if let Err(e) = http.create_pin(channel_id, message_id).await {
                    tracing::error!("failed to pin message: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to pin message".to_string(),
                    });
                }
            }
            Action::UnpinMessage { channel_id, message_id } => {
                if let Err(e) = http.delete_pin(channel_id, message_id).await {
                    tracing::error!("failed to unpin message: {e}");
                    let _ = event_tx.send(DiscordEvent::ActionError {
                        message: "Failed to unpin message".to_string(),
                    });
                }
            }
            Action::FetchUserProfile { user_id } => {
                match http.user(user_id).await {
                    Ok(response) => match response.model().await {
                        Ok(user) => {
                            let profile = crate::store::profiles::UserProfile {
                                user_id: user.id,
                                username: user.name.clone(),
                                display_name: user.global_name.clone(),
                                bot: user.bot,
                            };
                            let _ = event_tx.send(DiscordEvent::UserProfileLoaded { profile });
                        }
                        Err(e) => {
                            tracing::error!("failed to deserialize user profile: {e}");
                            let _ = event_tx.send(DiscordEvent::ActionError {
                                message: "Failed to load user profile".to_string(),
                            });
                        }
                    },
                    Err(e) => {
                        tracing::error!("failed to fetch user profile: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: "Failed to load user profile".to_string(),
                        });
                    }
                }
            }
            Action::SendTyping { channel_id } => {
                if let Err(e) = http.create_typing_trigger(channel_id).await {
                    tracing::debug!("failed to send typing trigger: {e}");
                }
            }
            Action::UploadFile {
                channel_id,
                file_path,
                message,
            } => {
                match std::fs::read(&file_path) {
                    Ok(file_bytes) => {
                        let filename = std::path::Path::new(&file_path)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".to_string());
                        let attachment =
                            twilight_model::http::attachment::Attachment::from_bytes(
                                filename,
                                file_bytes,
                                1,
                            );
                        let attachments = [attachment];
                        let mut req = http.create_message(channel_id).attachments(&attachments);
                        if let Some(ref text) = message {
                            req = req.content(text);
                        }
                        match req.await {
                            Ok(_) => {
                                tracing::info!("file uploaded: {file_path}");
                                let _ = event_tx.send(DiscordEvent::FileUploaded { channel_id });
                            }
                            Err(e) => {
                                tracing::error!("failed to upload file: {e}");
                                let _ = event_tx.send(DiscordEvent::ActionError {
                                    message: format!("Failed to upload file: {e}"),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("failed to read file {file_path}: {e}");
                        let _ = event_tx.send(DiscordEvent::ActionError {
                            message: format!("Cannot read file: {e}"),
                        });
                    }
                }
            }
            Action::SetStatus { status } => {
                // TODO: Twilight does not support sending a gateway presence update for
                // user accounts (opcode 3 UPDATE_PRESENCE). The status is stored locally
                // in UiState::own_status by the App before dispatching this action, so
                // the status bar reflects the change immediately. A raw WebSocket message
                // would be needed to actually change the status on Discord's servers.
                tracing::debug!("SetStatus requested: {status} (local-only; gateway update not yet implemented)");
            }
            Action::SetCustomStatus { emoji, text } => {
                // TODO: Setting a custom status requires sending a gateway opcode 3
                // UPDATE_PRESENCE payload with an activity of type Custom (4) containing
                // the emoji and state fields. Twilight-gateway does not expose this for
                // user accounts. The custom status is stored locally in UiState and
                // surfaced in the status bar. A real implementation would need to send:
                //   {"op":3,"d":{"since":null,"activities":[{"type":4,"name":"Custom Status",
                //     "state":<text>,"emoji":{"name":<emoji>}}],"status":"online","afk":false}}
                tracing::debug!("SetCustomStatus: emoji={:?} text={:?} (local-only; gateway update not implemented)", emoji, text);
            }
            Action::FetchDmChannels => {
                // twilight-http doesn't expose GET /users/@me/channels.
                // DM channels are populated from the Ready payload instead.
                // This is a no-op fallback if Ready didn't include them.
                tracing::debug!("FetchDmChannels: DMs loaded from Ready payload");
            }
            Action::FetchImage { url, channel_id: _, message_id: _ } => {
                let client = reqwest::Client::new();
                match client.get(&url).send().await {
                    Ok(response) => match response.bytes().await {
                        Ok(bytes) => {
                            match crate::tui::image_renderer::encode_image(
                                &bytes,
                                crate::tui::terminal_caps::GraphicsProtocol::Kitty,
                                40, // default max width in columns
                            ) {
                                Ok((data, w, h)) => {
                                    let _ = event_tx.send(DiscordEvent::ImageLoaded {
                                        url,
                                        image: crate::store::images::CachedImage {
                                            protocol_data: data,
                                            width: w,
                                            height: h,
                                        },
                                    });
                                }
                                Err(e) => tracing::debug!("image encode failed: {e}"),
                            }
                        }
                        Err(e) => tracing::error!("failed to download image bytes: {e}"),
                    },
                    Err(e) => tracing::error!("failed to fetch image: {e}"),
                }
            }
        }
    }
}

/// Percent-encode a string for use in a URL query parameter.
fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

/// Parse Discord search response JSON into SearchResult structs.
/// Discord returns: `{ messages: [[msg1, ctx1, ...], [msg2, ctx2, ...]], total_results: N }`
/// The first element of each inner array is the matching message.
fn parse_search_results(
    body: &serde_json::Value,
    scope: &crate::store::search::SearchScope,
) -> Vec<crate::store::search::SearchResult> {
    let messages = match body.get("messages").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    messages
        .iter()
        .filter_map(|group| {
            // Each group is an array; first element is the matching message
            let msg = group.as_array()?.first()?;
            let message_id_str = msg.get("id")?.as_str()?;
            let channel_id_str = msg.get("channel_id")?.as_str()?;
            let content = msg.get("content")?.as_str().unwrap_or("").to_string();
            let author_name = msg
                .get("author")
                .and_then(|a| a.get("username"))
                .and_then(|u| u.as_str())
                .unwrap_or("unknown")
                .to_string();
            let timestamp = msg
                .get("timestamp")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            let message_id = message_id_str.parse::<u64>().ok()?;
            let channel_id = channel_id_str.parse::<u64>().ok()?;

            let channel_name = match scope {
                crate::store::search::SearchScope::CurrentChannel(_) => {
                    format!("#{channel_id_str}")
                }
                crate::store::search::SearchScope::Server(_) => {
                    format!("#{channel_id_str}")
                }
            };

            let preview = if content.len() > 100 {
                format!("{}...", &content[..100])
            } else {
                content
            };

            Some(crate::store::search::SearchResult {
                message_id: twilight_model::id::Id::new(message_id),
                channel_id: twilight_model::id::Id::new(channel_id),
                channel_name,
                author_name,
                content_preview: preview,
                timestamp,
            })
        })
        .collect()
}
