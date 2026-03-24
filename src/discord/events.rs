use twilight_model::channel::Message;
use twilight_model::gateway::payload::incoming::Ready;
use twilight_model::guild::Guild;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub enum DiscordEvent {
    Ready(Box<Ready>),
    GuildCreate(Box<Guild>),
    GuildDelete(Id<GuildMarker>),
    ChannelCreate(Box<twilight_model::channel::Channel>),
    ChannelUpdate(Box<twilight_model::channel::Channel>),
    ChannelDelete(Id<ChannelMarker>),
    MessageCreate(Box<Message>),
    MessageUpdate {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        content: Option<String>,
    },
    MessageDelete {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    TypingStart {
        channel_id: Id<ChannelMarker>,
        user_id: Id<UserMarker>,
        display_name: String,
    },
    PresenceUpdate {
        user_id: Id<UserMarker>,
        guild_id: Id<GuildMarker>,
        custom_status: Option<crate::store::CustomStatus>,
    },
    MemberChunk {
        guild_id: Id<GuildMarker>,
        members: Vec<twilight_model::guild::Member>,
    },
    GatewayReconnect,
    GatewayDisconnect,
    // User account Ready (manually parsed since twilight can't deserialize it)
    UserReady {
        user_id: Id<UserMarker>,
        username: String,
        guilds: Vec<ReadyGuild>,
        guild_folders: Vec<crate::store::GuildFolder>,
        dm_channels: Vec<(Id<ChannelMarker>, Vec<String>)>,
        session_id: String,
        resume_url: String,
    },
    ReactionAdd {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: crate::store::messages::ReactionEmoji,
        user_id: Id<UserMarker>,
    },
    ReactionRemove {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: crate::store::messages::ReactionEmoji,
        user_id: Id<UserMarker>,
    },
    ReactionRemoveAll {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    // Channel pins updated (gateway event: a message was pinned or unpinned)
    ChannelPinsUpdate {
        channel_id: Id<ChannelMarker>,
    },
    // REST response events (sent by action handler, not gateway)
    ChannelsLoaded {
        guild_id: Id<GuildMarker>,
        channels: Vec<twilight_model::channel::Channel>,
    },
    MessagesLoaded {
        channel_id: Id<ChannelMarker>,
        messages: Vec<Message>,
    },
    MembersLoaded {
        guild_id: Id<GuildMarker>,
        members: Vec<twilight_model::guild::Member>,
    },
    PinnedMessagesLoaded {
        channel_id: Id<ChannelMarker>,
        messages: Vec<crate::store::messages::StoredMessage>,
    },
    VoiceStateUpdate {
        channel_id: Option<Id<ChannelMarker>>,
        user_id: Id<UserMarker>,
        display_name: String,
        self_mute: bool,
        self_deaf: bool,
    },
    /// Search results returned by the action handler (REST response).
    SearchResults {
        results: Vec<crate::store::search::SearchResult>,
    },
    ThreadCreate {
        thread_info: crate::store::ThreadInfo,
    },
    ThreadDelete {
        thread_id: twilight_model::id::Id<ChannelMarker>,
        parent_channel: twilight_model::id::Id<ChannelMarker>,
    },
    ThreadListSync {
        guild_id: twilight_model::id::Id<GuildMarker>,
        threads: Vec<crate::store::ThreadInfo>,
    },
    /// REST response: a user profile was fetched and is ready to cache.
    UserProfileLoaded {
        profile: crate::store::profiles::UserProfile,
    },
    /// DM channels fetched via REST.
    DmChannelsLoaded {
        channels: Vec<twilight_model::channel::Channel>,
    },
    /// Image fetched and encoded; ready to be stored in the image cache.
    ImageLoaded {
        url: String,
        image: crate::store::images::CachedImage,
    },
    /// An API action failed; carry a short user-friendly message to display.
    ActionError {
        message: String,
    },
}

/// Guild data extracted from the user account Ready payload.
#[derive(Debug, Clone)]
pub struct ReadyGuild {
    pub id: Id<GuildMarker>,
    pub name: String,
    pub channels: Vec<ReadyChannel>,
}

/// Channel data extracted from the user account Ready payload.
#[derive(Debug, Clone)]
pub struct ReadyChannel {
    pub id: Id<ChannelMarker>,
    pub name: String,
    pub kind: u8,
    pub parent_id: Option<Id<ChannelMarker>>,
    pub position: i32,
}

use twilight_gateway::Event;
use twilight_model::gateway::payload::incoming::GuildCreate;

pub fn translate_event(event: Event) -> Option<DiscordEvent> {
    match event {
        Event::Ready(ready) => Some(DiscordEvent::Ready(Box::new(ready))),
        Event::GuildCreate(gc) => match *gc {
            GuildCreate::Available(guild) => Some(DiscordEvent::GuildCreate(Box::new(guild))),
            GuildCreate::Unavailable(_) => None,
        },
        Event::GuildDelete(gd) => Some(DiscordEvent::GuildDelete(gd.id)),
        Event::ChannelCreate(cc) => Some(DiscordEvent::ChannelCreate(Box::new(cc.0))),
        Event::ChannelUpdate(cu) => Some(DiscordEvent::ChannelUpdate(Box::new(cu.0))),
        Event::ChannelDelete(cd) => Some(DiscordEvent::ChannelDelete(cd.0.id)),
        Event::MessageCreate(mc) => Some(DiscordEvent::MessageCreate(Box::new(mc.0))),
        Event::MessageUpdate(mu) => Some(DiscordEvent::MessageUpdate {
            channel_id: mu.channel_id,
            message_id: mu.id,
            content: Some(mu.0.content.clone()),
        }),
        Event::MessageDelete(md) => Some(DiscordEvent::MessageDelete {
            channel_id: md.channel_id,
            message_id: md.id,
        }),
        Event::MemberChunk(mc) => Some(DiscordEvent::MemberChunk {
            guild_id: mc.guild_id,
            members: mc.members.clone(),
        }),
        Event::TypingStart(ts) => Some(DiscordEvent::TypingStart {
            channel_id: ts.channel_id,
            user_id: ts.user_id,
            display_name: String::new(), // resolved in store from member cache
        }),
        Event::PresenceUpdate(e) => {
            let custom_status = e.activities.iter()
                .find(|a| a.kind == twilight_model::gateway::presence::ActivityType::Custom)
                .map(|a| crate::store::CustomStatus {
                    emoji: a.emoji.as_ref().map(|em| em.name.clone()),
                    text: a.state.clone(),
                });
            Some(DiscordEvent::PresenceUpdate {
                user_id: e.user.id(),
                guild_id: e.guild_id,
                custom_status,
            })
        },
        Event::VoiceStateUpdate(e) => Some(DiscordEvent::VoiceStateUpdate {
            channel_id: e.0.channel_id,
            user_id: e.0.user_id,
            display_name: String::new(), // resolved in store from member cache
            self_mute: e.0.self_mute,
            self_deaf: e.0.self_deaf,
        }),
        Event::ReactionAdd(ra) => {
            let emoji = match &ra.emoji {
                twilight_model::channel::message::EmojiReactionType::Unicode { name } => {
                    crate::store::messages::ReactionEmoji::Unicode(name.clone())
                }
                twilight_model::channel::message::EmojiReactionType::Custom { id, name, .. } => {
                    crate::store::messages::ReactionEmoji::Custom {
                        id: id.get(),
                        name: name.clone().unwrap_or_default(),
                    }
                }
            };
            Some(DiscordEvent::ReactionAdd {
                channel_id: ra.channel_id,
                message_id: ra.message_id,
                emoji,
                user_id: ra.user_id,
            })
        }
        Event::ReactionRemove(rr) => {
            let emoji = match &rr.emoji {
                twilight_model::channel::message::EmojiReactionType::Unicode { name } => {
                    crate::store::messages::ReactionEmoji::Unicode(name.clone())
                }
                twilight_model::channel::message::EmojiReactionType::Custom { id, name, .. } => {
                    crate::store::messages::ReactionEmoji::Custom {
                        id: id.get(),
                        name: name.clone().unwrap_or_default(),
                    }
                }
            };
            Some(DiscordEvent::ReactionRemove {
                channel_id: rr.channel_id,
                message_id: rr.message_id,
                emoji,
                user_id: rr.user_id,
            })
        }
        Event::ReactionRemoveAll(rra) => Some(DiscordEvent::ReactionRemoveAll {
            channel_id: rra.channel_id,
            message_id: rra.message_id,
        }),
        Event::ChannelPinsUpdate(cpu) => Some(DiscordEvent::ChannelPinsUpdate {
            channel_id: cpu.channel_id,
        }),
        Event::ThreadCreate(tc) => {
            // A thread is a Channel with a parent_id pointing at the parent channel.
            if let Some(parent_channel) = tc.0.parent_id {
                Some(DiscordEvent::ThreadCreate {
                    thread_info: crate::store::ThreadInfo {
                        id: tc.0.id,
                        name: tc.0.name.clone().unwrap_or_default(),
                        parent_channel,
                        message_count: tc.0.message_count.unwrap_or(0),
                    },
                })
            } else {
                None
            }
        }
        Event::ThreadDelete(td) => Some(DiscordEvent::ThreadDelete {
            thread_id: td.id,
            parent_channel: td.parent_id,
        }),
        Event::ThreadListSync(tls) => {
            let threads: Vec<crate::store::ThreadInfo> = tls
                .threads
                .iter()
                .filter_map(|ch| {
                    ch.parent_id.map(|parent_channel| crate::store::ThreadInfo {
                        id: ch.id,
                        name: ch.name.clone().unwrap_or_default(),
                        parent_channel,
                        message_count: ch.message_count.unwrap_or(0),
                    })
                })
                .collect();
            Some(DiscordEvent::ThreadListSync {
                guild_id: tls.guild_id,
                threads,
            })
        }
        _ => None,
    }
}
