use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::discord::actions::Action;
use crate::store::guilds::ChannelKind;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct ChannelTree {
    selected_index: usize,
}

impl ChannelTree {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }
}

impl Component for ChannelTree {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::ChannelTree {
            return Ok(None);
        }

        let guild_id = match store.ui.selected_guild {
            Some(id) => id,
            None => return Ok(None),
        };

        let channels = store.guilds.get_channels_for_guild(guild_id);
        // Build the flat list of selectable channels (non-category)
        let selectable: Vec<_> = channels
            .iter()
            .filter(|ch| ch.kind != ChannelKind::Category)
            .collect();

        let total = selectable.len();

        let changed = match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if total > 0 && self.selected_index + 1 < total {
                    self.selected_index += 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Enter | KeyCode::Right => {
                // Enter/Right moves focus to message input, ready to type
                store.ui.focus = FocusTarget::MessageInput;
                return Ok(None);
            }
            KeyCode::Esc | KeyCode::Left => {
                // Back to server list
                store.ui.focus = FocusTarget::ServerList;
                return Ok(None);
            }
            _ => false,
        };

        // Auto-select channel on navigate and fetch messages
        if changed {
            if let Some(ch) = selectable.get(self.selected_index) {
                let channel_id = ch.id;
                store.ui.selected_channel = Some(channel_id);
                store.notifications.mark_read(channel_id);
                return Ok(Some(Action::FetchMessages {
                    channel_id,
                    before: None,
                    limit: 50,
                }));
            }
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let block = Block::default()
            .title("Channels")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));

        let guild_id = match store.ui.selected_guild {
            Some(id) => id,
            None => {
                frame.render_widget(block, area);
                return;
            }
        };

        let channels = store.guilds.get_channels_for_guild(guild_id);

        // Track which selectable index we are at while iterating
        let mut selectable_idx: usize = 0;
        let mut selected_list_row: Option<usize> = None;
        let mut items: Vec<ListItem> = Vec::new();

        for ch in &channels {
            match ch.kind {
                ChannelKind::Category => {
                    let label = ch.name.to_uppercase();
                    items.push(ListItem::new(Line::from(Span::styled(
                        label,
                        theme::muted().add_modifier(Modifier::BOLD),
                    ))));
                }
                ChannelKind::Voice => {
                    let is_selected = selectable_idx == self.selected_index;
                    if is_selected {
                        selected_list_row = Some(items.len());
                    }
                    let prefix = "v ";
                    let name = format!("{}{}", prefix, ch.name);
                    let style = if is_selected {
                        theme::selected()
                    } else {
                        theme::muted()
                    };
                    items.push(ListItem::new(Line::from(Span::styled(name, style))));
                    selectable_idx += 1;
                }
                // Text, Announcement, Forum treated as text channels
                _ => {
                    let is_selected = selectable_idx == self.selected_index;
                    if is_selected {
                        selected_list_row = Some(items.len());
                    }
                    let has_unread = store.notifications.has_unreads(ch.id);
                    let has_mention = store.notifications.has_mentions(ch.id);

                    let name = format!("# {}", ch.name);
                    let has_typers = store.typing.has_typers(ch.id);

                    let base_style = if is_selected {
                        theme::selected()
                    } else {
                        theme::secondary_text()
                    };

                    let name_style = if has_unread || has_mention {
                        base_style.add_modifier(Modifier::BOLD)
                    } else {
                        base_style
                    };

                    let mut spans = vec![Span::styled(name, name_style)];

                    if has_typers {
                        spans.push(Span::styled(
                            " \u{22ef}",
                            theme::muted().add_modifier(Modifier::DIM),
                        ));
                    }

                    if has_mention {
                        let count = store
                            .notifications
                            .get(ch.id)
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

                    items.push(ListItem::new(Line::from(spans)));
                    selectable_idx += 1;
                }
            }
        }

        let list = List::new(items)
            .block(block)
            .highlight_style(theme::selected());
        let mut state = ListState::default().with_selected(selected_list_row);
        frame.render_stateful_widget(list, area, &mut state);
    }
}
