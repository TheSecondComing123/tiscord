use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub enum PaneView {
    Channel(Id<ChannelMarker>),
    Thread { parent_channel: Id<ChannelMarker>, thread_id: Id<ChannelMarker> },
    SearchContext { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>, query: String },
    PinContext { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    ServerList,
    ChannelTree,
    MessageList,
    MessageInput,
    MemberSidebar,
    CommandPalette,
    EmojiPicker,
    SearchOverlay,
    ProfileOverlay,
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
    pub message_pane_stack: Vec<PaneView>,
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
            message_pane_stack: Vec::new(),
        }
    }
}

impl UiState {
    pub fn active_channel(&self) -> Option<Id<ChannelMarker>> {
        self.message_pane_stack.last().map(|view| match view {
            PaneView::Channel(id) => *id,
            PaneView::Thread { thread_id, .. } => *thread_id,
            PaneView::SearchContext { channel_id, .. } => *channel_id,
            PaneView::PinContext { channel_id, .. } => *channel_id,
        }).or(self.selected_channel)
    }

    pub fn push_pane(&mut self, view: PaneView) {
        if self.message_pane_stack.len() < 3 {
            self.message_pane_stack.push(view);
        }
    }

    pub fn pop_pane(&mut self) -> bool {
        if self.message_pane_stack.len() > 1 {
            self.message_pane_stack.pop();
            true
        } else {
            false
        }
    }
}
