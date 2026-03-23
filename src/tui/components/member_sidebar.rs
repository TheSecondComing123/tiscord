use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::discord::actions::Action;
use crate::store::{MemberStatus, Store};
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct MemberSidebar {
    scroll_offset: usize,
}

impl MemberSidebar {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }
}

impl Component for MemberSidebar {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::MemberSidebar {
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Esc | KeyCode::Left => {
                // Back to message list
                store.ui.focus = FocusTarget::MessageList;
            }
            KeyCode::Enter | KeyCode::Right => {
                // Go to message input
                store.ui.focus = FocusTarget::MessageInput;
            }
            _ => {}
        }
        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let is_focused = store.ui.focus == FocusTarget::MemberSidebar;
        let border_style = if is_focused {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default().fg(theme::BORDER)
        };

        let block = Block::default()
            .title("Members")
            .borders(Borders::LEFT)
            .border_style(border_style)
            .style(Style::default().bg(theme::BG));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let guild_id = match store.ui.selected_guild {
            Some(id) => id,
            None => {
                let placeholder = Paragraph::new("No guild selected")
                    .style(theme::muted());
                frame.render_widget(placeholder, inner);
                return;
            }
        };

        let members = match store.members.get(&guild_id) {
            Some(m) => m,
            None => {
                let placeholder = Paragraph::new("No members loaded")
                    .style(theme::muted());
                frame.render_widget(placeholder, inner);
                return;
            }
        };

        // Partition into online and offline groups.
        let online: Vec<_> = members.iter().filter(|m| m.status == MemberStatus::Online).collect();
        let offline: Vec<_> = members.iter().filter(|m| m.status != MemberStatus::Online).collect();

        let mut lines: Vec<Line> = Vec::new();

        // Online group
        if !online.is_empty() {
            let header = Line::from(Span::styled(
                format!("ONLINE \u{2014} {}", online.len()),
                Style::default().fg(theme::TEXT_MUTED),
            ));
            lines.push(header);
            for member in &online {
                let dot = Span::styled("\u{25CF} ", Style::default().fg(theme::ONLINE));
                let name = Span::styled(member.name.as_str(), Style::default().fg(theme::TEXT_PRIMARY));
                lines.push(Line::from(vec![dot, name]));
                if let Some(cs) = &member.custom_status {
                    let status_text = match (&cs.emoji, &cs.text) {
                        (Some(e), Some(t)) => format!("  {} {}", e, t),
                        (Some(e), None) => format!("  {}", e),
                        (None, Some(t)) => format!("  {}", t),
                        (None, None) => continue,
                    };
                    lines.push(Line::from(Span::styled(status_text, theme::muted())));
                }
            }
        }

        // Offline group (includes Unknown)
        if !offline.is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            let header = Line::from(Span::styled(
                format!("OFFLINE \u{2014} {}", offline.len()),
                Style::default().fg(theme::TEXT_MUTED),
            ));
            lines.push(header);
            for member in &offline {
                let dot = Span::styled("\u{25CF} ", Style::default().fg(theme::TEXT_MUTED));
                let name = Span::styled(member.name.as_str(), Style::default().fg(theme::TEXT_SECONDARY));
                lines.push(Line::from(vec![dot, name]));
                if let Some(cs) = &member.custom_status {
                    let status_text = match (&cs.emoji, &cs.text) {
                        (Some(e), Some(t)) => format!("  {} {}", e, t),
                        (Some(e), None) => format!("  {}", e),
                        (None, Some(t)) => format!("  {}", t),
                        (None, None) => continue,
                    };
                    lines.push(Line::from(Span::styled(status_text, theme::muted())));
                }
            }
        }

        if lines.is_empty() {
            let placeholder = Paragraph::new("No members")
                .style(theme::muted());
            frame.render_widget(placeholder, inner);
            return;
        }

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .scroll((self.scroll_offset as u16, 0));

        frame.render_widget(paragraph, inner);
    }
}
