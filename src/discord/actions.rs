use tokio::sync::mpsc;
use twilight_http::Client as HttpClient;
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
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
    /// Internal action used by components to request cross-component coordination.
    /// Intercepted by App before reaching the action handler.
    ComponentKeyAction(KeyAction),
}

pub async fn run_action_handler(
    http: HttpClient,
    mut action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<DiscordEvent>,
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
                }
            }
            Action::DeleteMessage {
                channel_id,
                message_id,
            } => {
                if let Err(e) = http.delete_message(channel_id, message_id).await {
                    tracing::error!("failed to delete message: {e}");
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
                        Err(e) => tracing::error!("failed to deserialize messages: {e}"),
                    },
                    Err(e) => tracing::error!("failed to fetch messages: {e}"),
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
                        Err(e) => tracing::error!("failed to deserialize channels: {e}"),
                    },
                    Err(e) => tracing::error!("failed to fetch channels: {e}"),
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
                }
            }
            Action::SearchMessages { scope, query } => {
                // TODO: Discord's message search REST endpoint is not available through
                // twilight-http for user accounts. A real implementation would issue a
                // raw GET request to:
                //   /guilds/{guild_id}/messages/search?content={query}
                //   /channels/{channel_id}/messages/search?content={query}
                // For now we return empty results so the UI infrastructure is exercised.
                tracing::debug!("search requested: {:?} query={:?}", scope, query);
                let _ = event_tx.send(DiscordEvent::SearchResults { results: Vec::new() });
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
                        Err(e) => tracing::error!("failed to load messages for search nav: {e}"),
                    },
                    Err(e) => tracing::error!("failed to fetch channel for search nav: {e}"),
                }
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
                        Err(e) => tracing::error!("failed to deserialize members: {e}"),
                    },
                    Err(e) => tracing::error!("failed to fetch members: {e}"),
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
                                    }
                                })
                                .collect();
                            let _ = event_tx.send(DiscordEvent::PinnedMessagesLoaded {
                                channel_id,
                                messages: stored,
                            });
                        }
                        Err(e) => tracing::error!("failed to deserialize pinned messages: {e}"),
                    },
                    Err(e) => tracing::error!("failed to fetch pinned messages: {e}"),
                }
            }
            Action::PinMessage { channel_id, message_id } => {
                if let Err(e) = http.create_pin(channel_id, message_id).await {
                    tracing::error!("failed to pin message: {e}");
                }
            }
            Action::UnpinMessage { channel_id, message_id } => {
                if let Err(e) = http.delete_pin(channel_id, message_id).await {
                    tracing::error!("failed to unpin message: {e}");
                }
            }
        }
    }
}
