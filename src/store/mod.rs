pub mod guilds;
pub mod messages;
pub mod notifications;
pub mod state;

use std::collections::HashMap;
use twilight_model::id::marker::{ChannelMarker, UserMarker};
use twilight_model::id::Id;

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
}
