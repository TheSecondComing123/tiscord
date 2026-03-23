use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::components::channel_header::ChannelHeader;
use crate::tui::components::message_input::MessageInput;
use crate::tui::components::message_list::MessageList;

pub struct MessagePane {
    channel_header: ChannelHeader,
    pub message_list: MessageList,
    pub message_input: MessageInput,
}

impl MessagePane {
    pub fn new() -> Self {
        Self {
            channel_header: ChannelHeader::new(),
            message_list: MessageList::new(),
            message_input: MessageInput::new(),
        }
    }
}

impl Component for MessagePane {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        match store.ui.focus {
            FocusTarget::MessageList => self.message_list.handle_key_event(key, store),
            FocusTarget::MessageInput => self.message_input.handle_key_event(key, store),
            _ => Ok(None),
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        // Split vertically: header (2 lines), message list (rest), input (3 lines)
        let chunks = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

        self.channel_header.render(frame, chunks[0], store);
        self.message_list.render(frame, chunks[1], store);
        self.message_input.render(frame, chunks[2], store);
    }
}
