use std::collections::HashSet;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

use crate::discord::actions::Action;
use crate::store::guilds::ChannelKind;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

/// An entry in the flat, navigable channel tree list.
#[derive(Debug, Clone)]
enum TreeEntry {
    /// A category header that can be collapsed/expanded.
    Category { id: Id<ChannelMarker> },
    /// A selectable channel.
    Channel { selectable_idx: usize },
}

pub struct ChannelTree {
    /// Index into the flat `TreeEntry` list (includes categories).
    cursor: usize,
    /// Set of category IDs whose children are currently hidden.
    collapsed_categories: HashSet<Id<ChannelMarker>>,
}

impl ChannelTree {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            collapsed_categories: HashSet::new(),
        }
    }

    /// Build the flat list of tree entries, respecting collapsed categories.
    /// Returns (entries, selectable_channels_in_order).
    fn build_entries<'a>(
        &self,
        channels: &[&'a crate::store::guilds::ChannelInfo],
    ) -> (Vec<TreeEntry>, Vec<usize>) {
        let mut entries: Vec<TreeEntry> = Vec::new();
        let mut selectable_order: Vec<usize> = Vec::new(); // index into channels slice
        let mut selectable_idx = 0usize;
        let mut current_category: Option<Id<ChannelMarker>> = None;
        let mut current_category_collapsed = false;

        for (ch_idx, ch) in channels.iter().enumerate() {
            match ch.kind {
                ChannelKind::Category => {
                    current_category = Some(ch.id);
                    current_category_collapsed = self.collapsed_categories.contains(&ch.id);
                    entries.push(TreeEntry::Category { id: ch.id });
                }
                _ => {
                    // Determine if this channel belongs to a collapsed category.
                    let is_hidden = match ch.category_id {
                        Some(cat_id) => self.collapsed_categories.contains(&cat_id),
                        None => false,
                    };
                    if !is_hidden {
                        entries.push(TreeEntry::Channel { selectable_idx });
                        selectable_order.push(ch_idx);
                        selectable_idx += 1;
                    }
                }
            }
            let _ = (ch_idx, current_category, current_category_collapsed);
        }

        (entries, selectable_order)
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
        let (entries, selectable_order) = self.build_entries(&channels);

        let total_entries = entries.len();

        match key.code {
            KeyCode::Down => {
                if total_entries > 0 && self.cursor + 1 < total_entries {
                    self.cursor += 1;
                }
            }
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Enter | KeyCode::Right => {
                // Check what is under the cursor.
                match entries.get(self.cursor) {
                    Some(TreeEntry::Category { id }) => {
                        // Toggle collapse for this category.
                        if self.collapsed_categories.contains(id) {
                            self.collapsed_categories.remove(id);
                        } else {
                            self.collapsed_categories.insert(*id);
                        }
                        // Clamp cursor in case entries shrank.
                        let (new_entries, _) = self.build_entries(&channels);
                        if self.cursor >= new_entries.len() && !new_entries.is_empty() {
                            self.cursor = new_entries.len() - 1;
                        }
                        return Ok(None);
                    }
                    Some(TreeEntry::Channel { selectable_idx }) => {
                        // Select this channel.
                        if let Some(&ch_idx) = selectable_order.get(*selectable_idx) {
                            if let Some(ch) = channels.get(ch_idx) {
                                let channel_id = ch.id;
                                let is_forum = ch.kind == ChannelKind::Forum;
                                store.ui.selected_channel = Some(channel_id);
                                store.notifications.mark_read(channel_id);
                                store.ui.focus = FocusTarget::MessageInput;
                                if is_forum {
                                    return Ok(Some(Action::FetchActiveThreads {
                                        guild_id,
                                        channel_id,
                                    }));
                                } else {
                                    return Ok(Some(Action::FetchMessages {
                                        channel_id,
                                        before: None,
                                        limit: 50,
                                    }));
                                }
                            }
                        }
                        store.ui.focus = FocusTarget::MessageInput;
                        return Ok(None);
                    }
                    None => {
                        store.ui.focus = FocusTarget::MessageInput;
                        return Ok(None);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Left => {
                store.ui.focus = FocusTarget::ServerList;
                return Ok(None);
            }
            KeyCode::Char('m') => {
                // Toggle mute for the channel under the cursor.
                if let Some(entry) = entries.get(self.cursor) {
                    if let TreeEntry::Channel { selectable_idx } = entry {
                        if let Some(&ch_idx) = selectable_order.get(*selectable_idx) {
                            if let Some(ch) = channels.get(ch_idx) {
                                store.toggle_mute_channel(ch.id);
                            }
                        }
                    }
                }
                return Ok(None);
            }
            _ => {}
        }

        // After Up/Down navigation, auto-select the underlying channel (if any).
        if let Some(entry) = entries.get(self.cursor) {
            if let TreeEntry::Channel { selectable_idx } = entry {
                if let Some(&ch_idx) = selectable_order.get(*selectable_idx) {
                    if let Some(ch) = channels.get(ch_idx) {
                        let channel_id = ch.id;
                        let is_forum = ch.kind == ChannelKind::Forum;
                        store.ui.selected_channel = Some(channel_id);
                        store.notifications.mark_read(channel_id);
                        if is_forum {
                            return Ok(Some(Action::FetchActiveThreads {
                                guild_id,
                                channel_id,
                            }));
                        } else {
                            return Ok(Some(Action::FetchMessages {
                                channel_id,
                                before: None,
                                limit: 50,
                            }));
                        }
                    }
                }
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
        let (entries, selectable_order) = self.build_entries(&channels);

        let mut items: Vec<ListItem> = Vec::new();
        let mut selected_list_row: Option<usize> = None;

        for (entry_idx, entry) in entries.iter().enumerate() {
            let is_cursor = entry_idx == self.cursor;

            match entry {
                TreeEntry::Category { id } => {
                    // Find this category in the channels list.
                    let cat = channels.iter().find(|c| c.id == *id);
                    let cat_name = cat.map(|c| c.name.as_str()).unwrap_or("?");
                    let is_collapsed = self.collapsed_categories.contains(id);
                    let indicator = if is_collapsed { "▸" } else { "▾" };
                    let label = format!("{} {}", indicator, cat_name.to_uppercase());

                    let style = if is_cursor {
                        theme::selected().add_modifier(Modifier::BOLD)
                    } else {
                        theme::muted().add_modifier(Modifier::BOLD)
                    };

                    if is_cursor {
                        selected_list_row = Some(items.len());
                    }
                    items.push(ListItem::new(Line::from(Span::styled(label, style))));
                }

                TreeEntry::Channel { selectable_idx } => {
                    let ch_idx = match selectable_order.get(*selectable_idx) {
                        Some(&i) => i,
                        None => continue,
                    };
                    let ch = match channels.get(ch_idx) {
                        Some(c) => c,
                        None => continue,
                    };

                    if is_cursor {
                        selected_list_row = Some(items.len());
                    }

                    match ch.kind {
                        ChannelKind::Voice => {
                            let user_count = store.voice.user_count(ch.id);
                            let name = if user_count > 0 {
                                format!("\u{1f50a} {} ({})", ch.name, user_count)
                            } else {
                                format!("\u{1f50a} {}", ch.name)
                            };
                            let style = if is_cursor {
                                theme::selected()
                            } else {
                                theme::muted()
                            };
                            items.push(ListItem::new(Line::from(Span::styled(name, style))));

                            // When selected, show connected users indented below
                            if is_cursor && user_count > 0 {
                                for voice_user in store.voice.get_users(ch.id) {
                                    let mute_indicator =
                                        if voice_user.self_mute || voice_user.self_deaf {
                                            "\u{1f507} "
                                        } else {
                                            "   "
                                        };
                                    let label =
                                        format!("  {}{}", mute_indicator, voice_user.display_name);
                                    items.push(ListItem::new(Line::from(Span::styled(
                                        label,
                                        theme::muted(),
                                    ))));
                                }
                            }
                        }
                        // Text, Announcement, Forum
                        _ => {
                            let has_unread = store.notifications.has_unreads(ch.id);
                            let has_mention = store.notifications.has_mentions(ch.id);
                            let has_slowmode =
                                ch.rate_limit_per_user.map(|r| r > 0).unwrap_or(false);
                            let is_muted = store.is_channel_muted(ch.id, store.ui.selected_guild);

                            let prefix = if is_muted {
                                "\u{1f507} "
                            } else if ch.nsfw {
                                "🔞 "
                            } else {
                                "# "
                            };
                            let suffix = if has_slowmode && !is_muted { " \u{1f40c}" } else { "" };
                            let name = format!("{}{}{}", prefix, ch.name, suffix);

                            let has_typers = store.typing.has_typers(ch.id);

                            let base_style = if is_cursor {
                                theme::selected()
                            } else if is_muted {
                                theme::muted().add_modifier(Modifier::DIM)
                            } else {
                                theme::secondary_text()
                            };
                            let name_style = if !is_muted && (has_unread || has_mention) {
                                base_style.add_modifier(Modifier::BOLD)
                            } else {
                                base_style
                            };

                            let mut spans = vec![Span::styled(name, name_style)];

                            if has_typers && !is_muted {
                                spans.push(Span::styled(
                                    " \u{22ef}",
                                    theme::muted().add_modifier(Modifier::DIM),
                                ));
                            }

                            if has_mention && !is_muted {
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
                        }
                    }
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
