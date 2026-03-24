use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct ChannelInfoOverlay {
    pub visible: bool,
    channel_name: String,
    topic: Option<String>,
}

impl ChannelInfoOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            channel_name: String::new(),
            topic: None,
        }
    }

    pub fn open(&mut self, channel_name: String, topic: Option<String>) {
        self.channel_name = channel_name;
        self.topic = topic;
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Component for ChannelInfoOverlay {
    fn handle_key_event(&mut self, key: KeyEvent, _store: &mut Store) -> Result<Option<Action>> {
        if !self.visible {
            return Ok(None);
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') | KeyCode::Enter => {
                self.close();
            }
            _ => {}
        }
        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _store: &Store) {
        if !self.visible {
            return;
        }

        // Build centered popup: 60% width, up to 12 lines tall
        let popup_width = (area.width * 6 / 10).max(40).min(area.width.saturating_sub(4));
        let topic_text = self.topic.as_deref().unwrap_or("No topic set.");
        // Count lines needed (rough estimate)
        let text_lines = topic_text
            .chars()
            .filter(|&c| c == '\n')
            .count() as u16
            + 1
            + 4; // borders + title + padding
        let popup_height = text_lines.clamp(5, 14);

        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: area.x + popup_x,
            y: area.y + popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        let title = format!(" #{} ", self.channel_name);
        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG_SECONDARY));

        let paragraph = Paragraph::new(topic_text.to_owned())
            .block(block)
            .style(Style::default().fg(theme::TEXT_PRIMARY).bg(theme::BG_SECONDARY))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, popup_area);
    }
}
