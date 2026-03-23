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
    PresenceUpdate,
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
        guilds: Vec<(Id<GuildMarker>, String)>,
        session_id: String,
        resume_url: String,
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
        _ => None,
    }
}
