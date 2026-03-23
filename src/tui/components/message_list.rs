use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::components::message::render_message;
use crate::tui::theme;

pub struct MessageList {
    pub selected_index: Option<usize>,
    auto_scroll: bool,
}

impl MessageList {
    pub fn new() -> Self {
        Self {
            selected_index: None,
            auto_scroll: true,
        }
    }
}

impl Component for MessageList {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::MessageList {
            return Ok(None);
        }

        let message_count = store
            .ui
            .selected_channel
            .and_then(|ch| store.messages.get(&ch))
            .map(|buf| buf.len())
            .unwrap_or(0);

        if message_count == 0 {
            return Ok(None);
        }

        let last_index = message_count - 1;

        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.auto_scroll = false;
                let current = self.selected_index.unwrap_or(last_index);
                self.selected_index = Some(current.saturating_add(1).min(last_index));
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.auto_scroll = false;
                let current = self.selected_index.unwrap_or(last_index);
                self.selected_index = Some(current.saturating_sub(1));
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.auto_scroll = false;
                let current = self.selected_index.unwrap_or(last_index);
                self.selected_index = Some(current.saturating_sub(10));
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.auto_scroll = false;
                let current = self.selected_index.unwrap_or(last_index);
                self.selected_index = Some(current.saturating_add(10).min(last_index));
            }
            // KeyCode::Char('G') arrives with SHIFT on some terminals; handle both.
            (KeyCode::Char('G'), _) => {
                self.selected_index = Some(last_index);
                self.auto_scroll = true;
            }
            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let channel_id = match store.ui.selected_channel {
            Some(id) => id,
            None => {
                let placeholder = Paragraph::new("No channel selected")
                    .style(theme::muted())
                    .alignment(Alignment::Center);
                frame.render_widget(placeholder, area);
                return;
            }
        };

        let messages = match store.messages.get(&channel_id) {
            Some(buf) => buf.messages(),
            None => {
                let placeholder = Paragraph::new("No messages")
                    .style(theme::muted())
                    .alignment(Alignment::Center);
                frame.render_widget(placeholder, area);
                return;
            }
        };

        if messages.is_empty() {
            let placeholder = Paragraph::new("No messages")
                .style(theme::muted())
                .alignment(Alignment::Center);
            frame.render_widget(placeholder, area);
            return;
        }

        // Build all lines, tracking which message index each line belongs to.
        // line_owners[i] = message index for the i-th rendered line.
        let mut all_lines: Vec<Line<'static>> = Vec::new();
        let mut line_owners: Vec<usize> = Vec::new();

        for (msg_idx, msg) in messages.iter().enumerate() {
            let rendered = render_message(msg, area.width);
            let line_count = rendered.len();
            all_lines.extend(rendered);
            for _ in 0..line_count {
                line_owners.push(msg_idx);
            }
        }

        // Apply selection highlight to all lines belonging to the selected message.
        if let Some(sel_idx) = self.selected_index {
            for (line_idx, &owner) in line_owners.iter().enumerate() {
                if owner == sel_idx {
                    let line = &mut all_lines[line_idx];
                    *line = line.clone().patch_style(Style::default().bg(theme::BG_SECONDARY));
                }
            }
        }

        let total_lines = all_lines.len() as u16;
        let visible_height = area.height;

        // Compute the scroll offset for this frame.
        let scroll_offset: u16 = if self.auto_scroll {
            // Always show the newest messages at the bottom.
            total_lines.saturating_sub(visible_height)
        } else if let Some(sel_idx) = self.selected_index {
            // Keep the selected message in the visible window.
            let last_line = line_owners
                .iter()
                .rposition(|&o| o == sel_idx)
                .unwrap_or(0) as u16;

            // Scroll to keep the selected message in the visible window.
            // We don't have persistent scroll state, so we anchor to the selection.
            if visible_height == 0 || last_line < visible_height {
                // All content fits or selection is near the top - no offset needed.
                0
            } else {
                // Prefer showing the selected message near the bottom of the view.
                last_line.saturating_sub(visible_height) + 1
            }
        } else {
            total_lines.saturating_sub(visible_height)
        };

        let paragraph = Paragraph::new(all_lines)
            .style(Style::default().fg(theme::TEXT_PRIMARY).bg(theme::BG))
            .scroll((scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }
}
