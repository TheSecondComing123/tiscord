use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::components::channel_tree::ChannelTree;
use crate::tui::components::server_list::ServerList;
use crate::tui::theme;

pub struct ServerChannelSidebar {
    server_list: ServerList,
    channel_tree: ChannelTree,
}

impl ServerChannelSidebar {
    pub fn new() -> Self {
        Self {
            server_list: ServerList::new(),
            channel_tree: ChannelTree::new(),
        }
    }
}

impl Component for ServerChannelSidebar {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        match store.ui.focus {
            FocusTarget::ServerList => self.server_list.handle_key_event(key, store),
            FocusTarget::ChannelTree => self.channel_tree.handle_key_event(key, store),
            _ => Ok(None),
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        // Calculate server list height: min 3, proportional to guild count (1 DM + guilds),
        // capped at 40% of the available area.
        let entry_count = 1 + store.guilds.guilds.len();
        // +2 for the block borders (top + bottom)
        let desired = (entry_count as u16).saturating_add(2);
        let max_height = (area.height * 2 / 5).max(3);
        let server_list_height = desired.max(3).min(max_height);

        let chunks = Layout::vertical([
            Constraint::Length(server_list_height),
            Constraint::Min(0),
        ])
        .split(area);

        self.server_list.render(frame, chunks[0], store);

        if store.ui.dm_mode {
            // DM mode: render an empty bordered block until DMList arrives in Task 20.
            let block = Block::default()
                .title("Direct Messages")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::BORDER))
                .style(Style::default().bg(theme::BG));
            frame.render_widget(block, chunks[1]);
        } else {
            self.channel_tree.render(frame, chunks[1], store);
        }
    }
}
