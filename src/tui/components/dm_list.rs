use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct DMList {
    selected_index: usize,
}

impl DMList {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }
}

impl Component for DMList {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        // DMList reuses the ChannelTree focus since it replaces it in DM mode
        if store.ui.focus != FocusTarget::ChannelTree {
            return Ok(None);
        }

        let total = store.dm_channels.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if total > 0 && self.selected_index + 1 < total {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Enter | KeyCode::Right => {
                if let Some(dm) = store.dm_channels.get(self.selected_index) {
                    let channel_id = dm.channel_id;
                    store.ui.selected_channel = Some(channel_id);
                    store.notifications.mark_read(channel_id);
                    store.ui.focus = FocusTarget::MessageInput;
                    return Ok(Some(Action::FetchMessages {
                        channel_id,
                        before: None,
                        limit: 50,
                    }));
                }
            }
            KeyCode::Esc | KeyCode::Left => {
                // Back to server list
                store.ui.focus = FocusTarget::ServerList;
            }
            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let is_focused = store.ui.focus == FocusTarget::ChannelTree;
        let border_style = if is_focused {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default().fg(theme::BORDER)
        };

        let block = Block::default()
            .title("Direct Messages")
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(theme::BG));

        if store.dm_channels.is_empty() {
            frame.render_widget(block, area);
            return;
        }

        let items: Vec<ListItem> = store
            .dm_channels
            .iter()
            .enumerate()
            .map(|(i, dm)| {
                let is_selected = i == self.selected_index;
                let has_unread = store.notifications.has_unreads(dm.channel_id);
                let has_mention = store.notifications.has_mentions(dm.channel_id);

                // Build recipient label
                let recipient_label = if dm.recipient_names.is_empty() {
                    "Unknown".to_string()
                } else {
                    dm.recipient_names.join(", ")
                };

                let name_style = if is_selected {
                    theme::selected()
                } else if has_unread || has_mention {
                    theme::base().add_modifier(Modifier::BOLD)
                } else {
                    theme::secondary_text()
                };

                let mut spans = vec![Span::styled(recipient_label, name_style)];

                // Mention badge
                if has_mention {
                    let count = store
                        .notifications
                        .get(dm.channel_id)
                        .map(|n| n.mention_count)
                        .unwrap_or(0);
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("({})", count),
                        Style::default()
                            .fg(theme::MENTION)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                // Last message preview on a second line, truncated
                let mut lines = vec![Line::from(spans)];
                if let Some(preview) = &dm.last_message_preview {
                    let truncated = truncate_preview(preview, 40);
                    lines.push(Line::from(Span::styled(
                        truncated,
                        theme::muted(),
                    )));
                }

                ListItem::new(Text::from(lines))
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}

fn truncate_preview(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
