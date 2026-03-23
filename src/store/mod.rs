pub mod guilds;
pub mod messages;
pub mod notifications;
pub mod state;

use std::collections::HashMap;
use twilight_model::channel::ChannelType;
use twilight_model::id::marker::{ChannelMarker, UserMarker};
use twilight_model::id::Id;

use crate::discord::events::DiscordEvent;

pub struct Store {
    pub guilds: guilds::GuildState,
    pub messages: HashMap<Id<ChannelMarker>, messages::MessageBuffer>,
    pub notifications: notifications::NotificationState,
    pub ui: state::UiState,
    pub current_user_id: Option<Id<UserMarker>>,
    pub current_user_name: Option<String>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            guilds: guilds::GuildState::default(),
            messages: HashMap::new(),
            notifications: notifications::NotificationState::default(),
            ui: state::UiState::default(),
            current_user_id: None,
            current_user_name: None,
        }
    }

    pub fn get_or_create_message_buffer(
        &mut self,
        channel_id: Id<ChannelMarker>,
    ) -> &mut messages::MessageBuffer {
        self.messages
            .entry(channel_id)
            .or_insert_with(|| messages::MessageBuffer::new(500))
    }

    pub fn process_discord_event(&mut self, event: DiscordEvent) {
        match event {
            DiscordEvent::Ready(ready) => {
                self.current_user_id = Some(ready.user.id);
                self.current_user_name = Some(ready.user.name.clone());
                self.ui.connection_status = state::ConnectionStatus::Connected;
                tracing::info!("ready as {}", ready.user.name);
            }
            DiscordEvent::GuildCreate(guild) => {
                let channels = guild
                    .channels
                    .iter()
                    .map(|ch| guilds::ChannelInfo {
                        id: ch.id,
                        name: ch.name.clone().unwrap_or_default(),
                        kind: match ch.kind {
                            ChannelType::GuildText => guilds::ChannelKind::Text,
                            ChannelType::GuildVoice => guilds::ChannelKind::Voice,
                            ChannelType::GuildCategory => guilds::ChannelKind::Category,
                            ChannelType::GuildAnnouncement => guilds::ChannelKind::Announcement,
                            ChannelType::GuildForum => guilds::ChannelKind::Forum,
                            _ => guilds::ChannelKind::Text,
                        },
                        category_id: ch.parent_id,
                        position: ch.position.unwrap_or(0),
                    })
                    .collect();

                let info = guilds::GuildInfo {
                    id: guild.id,
                    name: guild.name.clone(),
                    icon: guild.icon.map(|h| h.to_string()),
                    channels,
                };
                self.guilds.add_guild(info);
                tracing::debug!("guild create: {}", guild.name);
            }
            DiscordEvent::GuildDelete(guild_id) => {
                self.guilds.remove_guild(guild_id);
                tracing::debug!("guild delete: {guild_id}");
            }
            DiscordEvent::MessageCreate(msg) => {
                let channel_id = msg.channel_id;
                let stored = messages::StoredMessage {
                    id: msg.id,
                    author_name: msg.author.name.clone(),
                    author_id: msg.author.id,
                    content: msg.content.clone(),
                    timestamp: msg.timestamp.iso_8601().to_string(),
                    reply_to: msg.referenced_message.as_ref().map(|r| {
                        messages::ReplyContext {
                            author_name: r.author.name.clone(),
                            content_preview: truncate_preview(&r.content, 80),
                        }
                    }),
                    attachments: msg
                        .attachments
                        .iter()
                        .map(|a| messages::Attachment {
                            filename: a.filename.clone(),
                            size: a.size,
                            url: a.url.clone(),
                        })
                        .collect(),
                    is_edited: false,
                };
                self.get_or_create_message_buffer(channel_id).push(stored);
            }
            DiscordEvent::MessageUpdate {
                channel_id,
                message_id,
                content,
            } => {
                if let Some(new_content) = content {
                    if let Some(buf) = self.messages.get_mut(&channel_id) {
                        buf.update(message_id, new_content);
                    }
                }
            }
            DiscordEvent::MessageDelete {
                channel_id,
                message_id,
            } => {
                if let Some(buf) = self.messages.get_mut(&channel_id) {
                    buf.remove(message_id);
                }
            }
            DiscordEvent::GatewayDisconnect => {
                self.ui.connection_status = state::ConnectionStatus::Disconnected;
                tracing::warn!("gateway disconnected");
            }
            DiscordEvent::GatewayReconnect => {
                self.ui.connection_status = state::ConnectionStatus::Reconnecting;
                tracing::info!("gateway reconnecting");
            }
            _ => {
                tracing::debug!("unhandled discord event: {:?}", std::mem::discriminant(&event));
            }
        }
    }
}

fn truncate_preview(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
