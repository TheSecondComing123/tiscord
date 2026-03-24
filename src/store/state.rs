use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

/// The current user's own online status, cycled with Ctrl+S.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnStatus {
    Online,
    Idle,
    Dnd,
    Invisible,
}

impl OwnStatus {
    /// Cycle to the next status in the sequence: Online → Idle → DND → Invisible → Online.
    pub fn cycle(self) -> Self {
        match self {
            OwnStatus::Online => OwnStatus::Idle,
            OwnStatus::Idle => OwnStatus::Dnd,
            OwnStatus::Dnd => OwnStatus::Invisible,
            OwnStatus::Invisible => OwnStatus::Online,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OwnStatus::Online => "online",
            OwnStatus::Idle => "idle",
            OwnStatus::Dnd => "dnd",
            OwnStatus::Invisible => "invisible",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            OwnStatus::Online => "Online",
            OwnStatus::Idle => "Idle",
            OwnStatus::Dnd => "DND",
            OwnStatus::Invisible => "Invisible",
        }
    }
}

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
    /// The current user's own presence status (cycled with Ctrl+S).
    pub own_status: OwnStatus,
    /// Timestamp of the currently selected message (set by MessageList during render).
    pub selected_message_timestamp: Option<String>,
    /// The user's current custom status text (set via Ctrl+Shift+S).
    pub custom_status_text: Option<String>,
    /// When Some, the custom-status input bar is open; value is the text being typed.
    pub custom_status_input: Option<String>,
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
            own_status: OwnStatus::Online,
            selected_message_timestamp: None,
            custom_status_text: None,
            custom_status_input: None,
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
