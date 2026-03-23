use tokio::sync::mpsc;
use twilight_http::Client as HttpClient;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

use super::events::DiscordEvent;

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
        }
    }
}
