pub mod guilds;
pub mod images;
pub mod messages;
pub mod notifications;
pub mod profiles;
pub mod search;
pub mod state;
pub mod typing;
pub mod voice;

use std::collections::HashMap;
use twilight_model::channel::ChannelType;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub id: Id<ChannelMarker>,
    pub name: String,
    pub parent_channel: Id<ChannelMarker>,
    pub message_count: u32,
}

use crate::discord::events::DiscordEvent;

#[derive(Debug, Clone)]
pub struct CustomStatus {
    pub emoji: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub id: Id<UserMarker>,
    pub name: String,
    pub status: MemberStatus,
    pub custom_status: Option<CustomStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberStatus {
    Online,
    Idle,
    Dnd,
    Offline,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct GuildFolder {
    pub name: Option<String>,
    pub color: Option<u32>,
    pub guild_ids: Vec<Id<GuildMarker>>,
}

#[derive(Debug, Clone)]
pub struct DmChannel {
    pub channel_id: Id<ChannelMarker>,
    pub recipient_names: Vec<String>,
    pub last_message_preview: Option<String>,
}

pub struct Store {
    pub guilds: guilds::GuildState,
    pub guild_folders: Vec<GuildFolder>,
    pub messages: HashMap<Id<ChannelMarker>, messages::MessageBuffer>,
    pub members: HashMap<Id<GuildMarker>, Vec<MemberInfo>>,
    pub notifications: notifications::NotificationState,
    pub ui: state::UiState,
    pub current_user_id: Option<Id<UserMarker>>,
    pub current_user_name: Option<String>,
    pub dm_channels: Vec<DmChannel>,
    pub typing: typing::TypingState,
    pub voice: voice::VoiceState,
    pub search: search::SearchState,
    /// Cache of pinned messages per channel. None means cache is invalid/not yet loaded.
    pub pinned_messages: HashMap<Id<ChannelMarker>, Option<Vec<messages::StoredMessage>>>,
    /// Active threads indexed by parent channel ID.
    pub active_threads: HashMap<Id<ChannelMarker>, Vec<ThreadInfo>>,
    /// User profile cache.
    pub profiles: profiles::ProfileCache,
    /// Image cache for inline attachment previews.
    pub image_cache: images::ImageCache,
    /// Whether the current terminal supports inline image rendering.
    pub supports_images: bool,
    /// Last API error message to be surfaced as a status-bar toast. Cleared by App after reading.
    pub last_error: Option<String>,
    /// Last informational toast (non-error). Cleared by App after reading.
    pub last_toast: Option<String>,
    /// True while a file upload is in progress (used to show "Uploading..." in status bar).
    pub uploading_file: bool,
}

impl Store {
    pub fn new() -> Self {
        Self {
            guilds: guilds::GuildState::default(),
            guild_folders: Vec::new(),
            messages: HashMap::new(),
            members: HashMap::new(),
            notifications: notifications::NotificationState::default(),
            ui: state::UiState::default(),
            current_user_id: None,
            current_user_name: None,
            dm_channels: Vec::new(),
            typing: typing::TypingState::default(),
            voice: voice::VoiceState::default(),
            search: search::SearchState::default(),
            pinned_messages: HashMap::new(),
            active_threads: HashMap::new(),
            profiles: profiles::ProfileCache::default(),
            image_cache: images::ImageCache::default(),
            supports_images: false,
            last_error: None,
            last_toast: None,
            uploading_file: false,
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
                guild_folders: folders,
                dm_channels: ready_dms,
                ..
            } => {
                self.current_user_id = Some(user_id);
                self.current_user_name = Some(username.clone());
                self.ui.connection_status = state::ConnectionStatus::Connected;
                tracing::info!("ready as {} ({} guilds, {} DMs)", username, ready_guilds.len(), ready_dms.len());
                // Create guilds from Ready data, including channels if present
                for rg in ready_guilds {
                    // Discord channel types: 0=Text, 2=Voice, 4=Category, 5=Announcement, 15=Forum
                    // Thread types (10, 11, 12) are filtered out
                    let channels: Vec<guilds::ChannelInfo> = rg.channels
                        .iter()
                        .filter(|ch| !matches!(ch.kind, 10 | 11 | 12))
                        .map(|ch| guilds::ChannelInfo {
                            id: ch.id,
                            name: ch.name.clone(),
                            kind: match ch.kind {
                                0 => guilds::ChannelKind::Text,
                                2 => guilds::ChannelKind::Voice,
                                4 => guilds::ChannelKind::Category,
                                5 => guilds::ChannelKind::Announcement,
                                15 => guilds::ChannelKind::Forum,
                                _ => guilds::ChannelKind::Text,
                            },
                            category_id: ch.parent_id,
                            position: ch.position,
                            topic: None, // ReadyChannel doesn't carry topic
                            nsfw: false, // ReadyChannel doesn't carry nsfw
                        })
                        .collect();
                    tracing::debug!("guild {} has {} channels, {} members from Ready", rg.name, channels.len(), rg.members.len());
                    let guild_id = rg.id;
                    let info = guilds::GuildInfo {
                        id: guild_id,
                        name: rg.name,
                        icon: None,
                        channels,
                    };
                    self.guilds.add_guild(info);
                    // Populate members from Ready data
                    if !rg.members.is_empty() {
                        let infos: Vec<MemberInfo> = rg.members.into_iter().map(|m| MemberInfo {
                            id: m.user_id,
                            name: m.nickname.unwrap_or(m.username),
                            status: MemberStatus::Unknown,
                            custom_status: None,
                        }).collect();
                        self.members.insert(guild_id, infos);
                    }
                }
                // Store guild folders
                self.guild_folders = folders;
                // Populate DM channels from Ready data
                for (channel_id, recipients) in ready_dms {
                    self.dm_channels.push(DmChannel {
                        channel_id,
                        recipient_names: recipients,
                        last_message_preview: None,
                    });
                }
            }
            DiscordEvent::GuildCreate(guild) => {
                let channels = guild
                    .channels
                    .iter()
                    .filter(|ch| !is_thread_channel(ch.kind))
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
                        topic: ch.topic.clone(),
                        nsfw: ch.nsfw.unwrap_or(false),
                    })
                    .collect();

                let info = guilds::GuildInfo {
                    id: guild.id,
                    name: guild.name.clone(),
                    icon: guild.icon.map(|h| h.to_string()),
                    channels,
                };
                self.guilds.add_guild(info);
                // Extract members from GuildCreate
                if !guild.members.is_empty() {
                    let infos = members_to_infos(guild.members.clone());
                    tracing::debug!("guild create: {} ({} members)", guild.name, infos.len());
                    self.members.insert(guild.id, infos);
                } else {
                    tracing::debug!("guild create: {}", guild.name);
                }
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
                        topic: ch.topic.clone(),
                        nsfw: ch.nsfw.unwrap_or(false),
                    };
                    self.guilds.add_channel_to_guild(guild_id, info);
                    tracing::debug!("channel create: {}", ch.id);
                } else if ch.kind == ChannelType::Private || ch.kind == ChannelType::Group {
                    // DM or group DM channel
                    let already_exists = self.dm_channels.iter().any(|dm| dm.channel_id == ch.id);
                    if !already_exists {
                        let recipients: Vec<String> = ch.recipients.as_deref().unwrap_or(&[]).iter()
                            .map(|u| u.name.clone())
                            .collect();
                        self.dm_channels.push(DmChannel {
                            channel_id: ch.id,
                            recipient_names: recipients,
                            last_message_preview: None,
                        });
                        tracing::debug!("DM channel create: {}", ch.id);
                    }
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
                        topic: ch.topic.clone(),
                        nsfw: ch.nsfw.unwrap_or(false),
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
                    reactions: msg
                        .reactions
                        .iter()
                        .map(|r| messages::Reaction {
                            emoji: match &r.emoji {
                                twilight_model::channel::message::EmojiReactionType::Unicode {
                                    name,
                                } => messages::ReactionEmoji::Unicode(name.clone()),
                                twilight_model::channel::message::EmojiReactionType::Custom {
                                    id,
                                    name,
                                    ..
                                } => messages::ReactionEmoji::Custom {
                                    id: id.get(),
                                    name: name.clone().unwrap_or_default(),
                                },
                            },
                            count: r.count as u32,
                            me: r.me,
                        })
                        .collect(),
                    embeds: msg.embeds.iter().map(|e| messages::Embed {
                        title: e.title.clone(),
                        description: e.description.clone(),
                        url: e.url.clone(),
                        color: e.color,
                        fields: e.fields.iter().map(|f| messages::EmbedField {
                            name: f.name.clone(),
                            value: f.value.clone(),
                            inline: f.inline,
                        }).collect(),
                        footer: e.footer.as_ref().map(|f| f.text.clone()),
                        author_name: e.author.as_ref().map(|a| a.name.clone()),
                    }).collect(),
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
                    .filter(|ch| !is_thread_channel(ch.kind))
                    .map(|ch| guilds::ChannelInfo {
                        id: ch.id,
                        name: ch.name.clone().unwrap_or_default(),
                        kind: channel_kind(ch.kind),
                        category_id: ch.parent_id,
                        position: ch.position.unwrap_or(0),
                        topic: ch.topic.clone(),
                        nsfw: ch.nsfw.unwrap_or(false),
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
                        reactions: msg
                            .reactions
                            .iter()
                            .map(|r| messages::Reaction {
                                emoji: match &r.emoji {
                                    twilight_model::channel::message::EmojiReactionType::Unicode {
                                        name,
                                    } => messages::ReactionEmoji::Unicode(name.clone()),
                                    twilight_model::channel::message::EmojiReactionType::Custom {
                                        id,
                                        name,
                                        ..
                                    } => messages::ReactionEmoji::Custom {
                                        id: id.get(),
                                        name: name.clone().unwrap_or_default(),
                                    },
                                },
                                count: r.count as u32,
                                me: r.me,
                            })
                            .collect(),
                        embeds: msg.embeds.iter().map(|e| messages::Embed {
                            title: e.title.clone(),
                            description: e.description.clone(),
                            url: e.url.clone(),
                            color: e.color,
                            fields: e.fields.iter().map(|f| messages::EmbedField {
                                name: f.name.clone(),
                                value: f.value.clone(),
                                inline: f.inline,
                            }).collect(),
                            footer: e.footer.as_ref().map(|f| f.text.clone()),
                            author_name: e.author.as_ref().map(|a| a.name.clone()),
                        }).collect(),
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
            DiscordEvent::TypingStart {
                channel_id,
                user_id,
                display_name,
            } => {
                // Skip our own typing events
                if Some(user_id) == self.current_user_id {
                    return;
                }
                // Resolve display_name from member cache if not already set
                let name = if display_name.is_empty() {
                    // Try to find the member's display name across all guilds
                    self.members
                        .values()
                        .flat_map(|members| members.iter())
                        .find(|m| m.id == user_id)
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| format!("user:{}", user_id))
                } else {
                    display_name
                };
                self.typing.add_typing(channel_id, user_id, name);
            }
            DiscordEvent::PresenceUpdate { user_id, guild_id, status, custom_status } => {
                if let Some(members) = self.members.get_mut(&guild_id) {
                    if let Some(member) = members.iter_mut().find(|m| m.id == user_id) {
                        member.status = status;
                        member.custom_status = custom_status;
                    }
                }
            }
            DiscordEvent::ReactionAdd { channel_id, message_id, emoji, user_id } => {
                let is_self = Some(user_id) == self.current_user_id;
                if let Some(buf) = self.messages.get_mut(&channel_id) {
                    buf.add_reaction(message_id, emoji, is_self);
                }
            }
            DiscordEvent::ReactionRemove { channel_id, message_id, emoji, user_id } => {
                let is_self = Some(user_id) == self.current_user_id;
                if let Some(buf) = self.messages.get_mut(&channel_id) {
                    buf.remove_reaction(message_id, &emoji, is_self);
                }
            }
            DiscordEvent::ReactionRemoveAll { channel_id, message_id } => {
                if let Some(buf) = self.messages.get_mut(&channel_id) {
                    buf.remove_all_reactions(message_id);
                }
            }
            DiscordEvent::ChannelPinsUpdate { channel_id } => {
                // Invalidate the pin cache for this channel so the next open re-fetches.
                self.pinned_messages.insert(channel_id, None);
                tracing::debug!("channel pins updated for {channel_id}");
            }
            DiscordEvent::PinnedMessagesLoaded { channel_id, messages } => {
                self.pinned_messages.insert(channel_id, Some(messages));
                tracing::debug!("pinned messages loaded for channel {channel_id}");
            }
            DiscordEvent::SearchResults { results } => {
                self.search.results = results;
                self.search.loading = false;
            }
            DiscordEvent::ThreadCreate { thread_info } => {
                self.active_threads
                    .entry(thread_info.parent_channel)
                    .or_default()
                    .push(thread_info);
            }
            DiscordEvent::ThreadDelete { thread_id, parent_channel } => {
                if let Some(threads) = self.active_threads.get_mut(&parent_channel) {
                    threads.retain(|t| t.id != thread_id);
                }
            }
            DiscordEvent::ThreadListSync { guild_id: _, threads } => {
                for thread in threads {
                    self.active_threads
                        .entry(thread.parent_channel)
                        .or_default()
                        .push(thread);
                }
            }
            DiscordEvent::VoiceStateUpdate { channel_id, user_id, display_name, self_mute, self_deaf } => {
                let name = if display_name.is_empty() {
                    self.members
                        .values()
                        .flat_map(|m| m.iter())
                        .find(|m| m.id == user_id)
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| format!("User {}", user_id))
                } else {
                    display_name
                };
                match channel_id {
                    Some(cid) => self.voice.user_joined(cid, voice::VoiceUser {
                        user_id,
                        display_name: name,
                        self_mute,
                        self_deaf,
                    }),
                    None => self.voice.user_left(user_id),
                }
            }
            DiscordEvent::UserProfileLoaded { profile } => {
                self.profiles.insert(profile);
            }
            DiscordEvent::DmChannelsLoaded { channels } => {
                self.dm_channels.clear();
                for ch in channels {
                    let recipients: Vec<String> = ch.recipients.as_deref().unwrap_or(&[]).iter()
                        .map(|u| u.name.clone())
                        .collect();
                    self.dm_channels.push(DmChannel {
                        channel_id: ch.id,
                        recipient_names: recipients,
                        last_message_preview: ch.last_message_id.map(|_| String::new()),
                    });
                }
                tracing::info!("loaded {} DM channels", self.dm_channels.len());
            }
            DiscordEvent::ImageLoaded { url, image } => {
                self.image_cache.insert(url, image);
            }
            DiscordEvent::ActionError { message } => {
                self.last_error = Some(message);
            }
            DiscordEvent::FileUploaded { channel_id: _ } => {
                self.uploading_file = false;
                self.last_toast = Some("File sent".to_string());
                tracing::info!("file upload completed");
            }
        }
    }
}

fn is_thread_channel(ct: ChannelType) -> bool {
    matches!(ct, ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::AnnouncementThread)
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
            custom_status: None,
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
            topic: None,
            nsfw: false,
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
                reactions: vec![],
                embeds: vec![],
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
                reactions: vec![],
                embeds: vec![],
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
