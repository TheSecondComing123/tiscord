pub mod guilds;
pub mod messages;
pub mod notifications;
pub mod state;

use std::collections::HashMap;
use twilight_model::channel::ChannelType;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use twilight_model::id::Id;

use crate::discord::events::DiscordEvent;

#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub id: Id<UserMarker>,
    pub name: String,
    pub status: MemberStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberStatus {
    Online,
    Offline,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DmChannel {
    pub channel_id: Id<ChannelMarker>,
    pub recipient_names: Vec<String>,
    pub last_message_preview: Option<String>,
}

pub struct Store {
    pub guilds: guilds::GuildState,
    pub messages: HashMap<Id<ChannelMarker>, messages::MessageBuffer>,
    pub members: HashMap<Id<GuildMarker>, Vec<MemberInfo>>,
    pub notifications: notifications::NotificationState,
    pub ui: state::UiState,
    pub current_user_id: Option<Id<UserMarker>>,
    pub current_user_name: Option<String>,
    pub dm_channels: Vec<DmChannel>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            guilds: guilds::GuildState::default(),
            messages: HashMap::new(),
            members: HashMap::new(),
            notifications: notifications::NotificationState::default(),
            ui: state::UiState::default(),
            current_user_id: None,
            current_user_name: None,
            dm_channels: Vec::new(),
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
            DiscordEvent::UserReady {
                user_id,
                username,
                guilds: ready_guilds,
                ..
            } => {
                self.current_user_id = Some(user_id);
                self.current_user_name = Some(username.clone());
                self.ui.connection_status = state::ConnectionStatus::Connected;
                tracing::info!("ready as {} ({} guilds)", username, ready_guilds.len());
                // Create placeholder guilds from Ready data
                // Channels will be empty until we fetch them via REST
                for (guild_id, guild_name) in ready_guilds {
                    let info = guilds::GuildInfo {
                        id: guild_id,
                        name: guild_name,
                        icon: None,
                        channels: Vec::new(),
                    };
                    self.guilds.add_guild(info);
                }
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
            DiscordEvent::ChannelCreate(ch) => {
                if let Some(guild_id) = ch.guild_id {
                    let info = guilds::ChannelInfo {
                        id: ch.id,
                        name: ch.name.clone().unwrap_or_default(),
                        kind: channel_kind(ch.kind),
                        category_id: ch.parent_id,
                        position: ch.position.unwrap_or(0),
                    };
                    self.guilds.add_channel_to_guild(guild_id, info);
                    tracing::debug!("channel create: {}", ch.id);
                }
            }
            DiscordEvent::ChannelUpdate(ch) => {
                if let Some(guild_id) = ch.guild_id {
                    let info = guilds::ChannelInfo {
                        id: ch.id,
                        name: ch.name.clone().unwrap_or_default(),
                        kind: channel_kind(ch.kind),
                        category_id: ch.parent_id,
                        position: ch.position.unwrap_or(0),
                    };
                    self.guilds.update_channel_in_guild(guild_id, info);
                    tracing::debug!("channel update: {}", ch.id);
                }
            }
            DiscordEvent::ChannelDelete(channel_id) => {
                // The ChannelDelete event does not carry guild_id, so search all guilds.
                let guild_ids: Vec<Id<GuildMarker>> =
                    self.guilds.guilds.iter().map(|g| g.id).collect();
                for gid in guild_ids {
                    self.guilds.remove_channel_from_guild(gid, channel_id);
                }
                tracing::debug!("channel delete: {channel_id}");
            }
            DiscordEvent::MessageCreate(msg) => {
                let channel_id = msg.channel_id;

                // Notification tracking: only for channels other than the currently selected one
                let is_selected = self.ui.selected_channel == Some(channel_id);
                if !is_selected {
                    self.notifications.increment_unread(channel_id);
                    // Check for a mention of the current user (<@user_id>)
                    if let Some(uid) = self.current_user_id {
                        let mention_token = format!("<@{}>", uid);
                        if msg.content.contains(&mention_token) {
                            self.notifications.increment_mentions(channel_id);
                        }
                    }
                }

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
            DiscordEvent::ChannelsLoaded {
                guild_id,
                channels,
            } => {
                let channel_infos: Vec<guilds::ChannelInfo> = channels
                    .iter()
                    .map(|ch| guilds::ChannelInfo {
                        id: ch.id,
                        name: ch.name.clone().unwrap_or_default(),
                        kind: channel_kind(ch.kind),
                        category_id: ch.parent_id,
                        position: ch.position.unwrap_or(0),
                    })
                    .collect();
                // Update the guild's channels
                if let Some(guild) = self.guilds.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.channels = channel_infos;
                    tracing::info!("loaded {} channels for {}", guild.channels.len(), guild.name);
                }
            }
            DiscordEvent::MessagesLoaded {
                channel_id,
                messages,
            } => {
                let stored: Vec<messages::StoredMessage> = messages
                    .iter()
                    .map(|msg| messages::StoredMessage {
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
                    })
                    .collect();
                self.get_or_create_message_buffer(channel_id).prepend(stored);
                tracing::debug!("messages loaded for channel {channel_id}");
            }
            DiscordEvent::MembersLoaded { guild_id, members } => {
                let infos = members_to_infos(members);
                self.members.insert(guild_id, infos);
                tracing::debug!("members loaded for guild {guild_id}");
            }
            DiscordEvent::MemberChunk { guild_id, members } => {
                let infos = members_to_infos(members);
                self.members
                    .entry(guild_id)
                    .and_modify(|v| {
                        for info in &infos {
                            if let Some(existing) = v.iter_mut().find(|m| m.id == info.id) {
                                *existing = info.clone();
                            } else {
                                v.push(info.clone());
                            }
                        }
                    })
                    .or_insert(infos);
                tracing::debug!("member chunk for guild {guild_id}");
            }
            DiscordEvent::GatewayDisconnect => {
                self.ui.connection_status = state::ConnectionStatus::Disconnected;
                tracing::warn!("gateway disconnected");
            }
            DiscordEvent::GatewayReconnect => {
                self.ui.connection_status = state::ConnectionStatus::Reconnecting;
                tracing::info!("gateway reconnecting");
            }
            DiscordEvent::TypingStart { .. } => {}
            DiscordEvent::PresenceUpdate => {}
        }
    }
}

fn channel_kind(ct: ChannelType) -> guilds::ChannelKind {
    match ct {
        ChannelType::GuildText => guilds::ChannelKind::Text,
        ChannelType::GuildVoice => guilds::ChannelKind::Voice,
        ChannelType::GuildCategory => guilds::ChannelKind::Category,
        ChannelType::GuildAnnouncement => guilds::ChannelKind::Announcement,
        ChannelType::GuildForum => guilds::ChannelKind::Forum,
        _ => guilds::ChannelKind::Text,
    }
}

fn members_to_infos(members: Vec<twilight_model::guild::Member>) -> Vec<MemberInfo> {
    members
        .into_iter()
        .map(|m| MemberInfo {
            id: m.user.id,
            name: m.nick.clone().unwrap_or_else(|| m.user.name.clone()),
            status: MemberStatus::Unknown,
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_guild_info(id: u64, name: &str) -> guilds::GuildInfo {
        guilds::GuildInfo {
            id: Id::new(id),
            name: name.to_string(),
            icon: None,
            channels: vec![],
        }
    }

    #[test]
    fn test_process_channel_create() {
        let mut store = Store::new();
        store.guilds.add_guild(make_guild_info(1, "TestGuild"));

        let ch = guilds::ChannelInfo {
            id: Id::new(42),
            name: "general".to_string(),
            kind: guilds::ChannelKind::Text,
            category_id: None,
            position: 1,
        };
        store.guilds.add_channel_to_guild(Id::new(1), ch);

        let channels = store.guilds.get_channels_for_guild(Id::new(1));
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].id, Id::new(42));
        assert_eq!(channels[0].name, "general");
    }

    #[test]
    fn test_process_messages_loaded() {
        let mut store = Store::new();
        let channel_id: Id<ChannelMarker> = Id::new(99);

        // Push two existing (newer) messages into the buffer.
        for (id, content) in [(10u64, "newer1"), (11, "newer2")] {
            let msg = messages::StoredMessage {
                id: Id::new(id),
                author_name: "alice".to_string(),
                author_id: Id::new(1),
                content: content.to_string(),
                timestamp: "2026-01-01T00:00:01Z".to_string(),
                reply_to: None,
                attachments: vec![],
                is_edited: false,
            };
            store.get_or_create_message_buffer(channel_id).push(msg);
        }

        // Prepend two older messages (in chronological order, oldest first).
        let older_msgs: Vec<messages::StoredMessage> = [(1u64, "oldest"), (2, "older")]
            .iter()
            .map(|&(id, content)| messages::StoredMessage {
                id: Id::new(id),
                author_name: "bob".to_string(),
                author_id: Id::new(2),
                content: content.to_string(),
                timestamp: "2025-12-31T23:59:58Z".to_string(),
                reply_to: None,
                attachments: vec![],
                is_edited: false,
            })
            .collect();
        store
            .get_or_create_message_buffer(channel_id)
            .prepend(older_msgs);

        let buf = store.messages.get(&channel_id).unwrap();
        assert_eq!(buf.len(), 4);
        let ids: Vec<u64> = buf.messages().iter().map(|m| m.id.get()).collect();
        // Oldest messages at front, newest at back.
        assert_eq!(ids, vec![1, 2, 10, 11]);
    }
}
