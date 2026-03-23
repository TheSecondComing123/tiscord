use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    ServerList,
    ChannelTree,
    MessageList,
    MessageInput,
    MemberSidebar,
    CommandPalette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Reconnecting,
}

#[derive(Debug)]
pub struct UiState {
    pub selected_guild: Option<Id<GuildMarker>>,
    pub selected_channel: Option<Id<ChannelMarker>>,
    pub focus: FocusTarget,
    pub member_sidebar_visible: bool,
    pub message_scroll_offset: usize,
    pub sidebar_scroll_offset: usize,
    pub dm_mode: bool,
    pub connection_status: ConnectionStatus,
    pub reply_to: Option<ReplyTarget>,
    pub editing_message: Option<Id<MessageMarker>>,
}

#[derive(Debug, Clone)]
pub struct ReplyTarget {
    pub message_id: Id<MessageMarker>,
    pub author_name: String,
    pub content_preview: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_guild: None,
            selected_channel: None,
            focus: FocusTarget::ServerList,
            member_sidebar_visible: false,
            message_scroll_offset: 0,
            sidebar_scroll_offset: 0,
            dm_mode: false,
            connection_status: ConnectionStatus::Connecting,
            reply_to: None,
            editing_message: None,
        }
    }
}
