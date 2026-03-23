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
    },
    PresenceUpdate,
    MemberChunk {
        guild_id: Id<GuildMarker>,
        members: Vec<twilight_model::guild::Member>,
    },
    GatewayReconnect,
    GatewayDisconnect,
    // REST response events (sent by action handler, not gateway)
    MessagesLoaded {
        channel_id: Id<ChannelMarker>,
        messages: Vec<Message>,
    },
    MembersLoaded {
        guild_id: Id<GuildMarker>,
        members: Vec<twilight_model::guild::Member>,
    },
}
