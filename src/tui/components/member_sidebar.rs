use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct MemberSidebar;

impl MemberSidebar {
    pub fn new() -> Self {
        Self
    }
}

impl Component for MemberSidebar {
    fn handle_key_event(&mut self, _key: KeyEvent, _store: &mut Store) -> Result<Option<Action>> {
        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _store: &Store) {
        let block = Block::default()
            .title("Members")
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));

        let placeholder = Paragraph::new("No members loaded")
            .style(theme::muted())
            .block(block);

        frame.render_widget(placeholder, area);
    }
}
