use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

use crate::discord::actions::Action;
use crate::store::guilds::ChannelKind;
use crate::store::state::FocusTarget;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteEntryKind {
    Server,
    Channel,
    Dm,
}

#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub label: String,
    pub kind: PaletteEntryKind,
    pub guild_id: Option<Id<GuildMarker>>,
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Context string shown alongside the label (e.g. server name for channels).
    pub context: String,
}

pub struct CommandPalette {
    query: String,
    cursor_pos: usize,
    /// Full list built when the palette opens; filtering draws from this.
    all_entries: Vec<PaletteEntry>,
    /// Currently displayed (filtered/sorted) results.
    results: Vec<PaletteEntry>,
    selected_index: usize,
    visible: bool,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor_pos: 0,
            all_entries: Vec::new(),
            results: Vec::new(),
            selected_index: 0,
            visible: false,
        }
    }

    /// Open the palette: build the full entry list from the store and show it.
    pub fn open(&mut self, store: &Store) {
        self.visible = true;
        self.query.clear();
        self.cursor_pos = 0;
        self.selected_index = 0;
        self.all_entries = build_entries(store);
        self.results = self.all_entries.clone();
    }

    /// Close the palette and clear state.
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.cursor_pos = 0;
        self.selected_index = 0;
        self.results.clear();
        self.all_entries.clear();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // --- private helpers ---

    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.query
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.query.len())
    }

    fn insert_char(&mut self, ch: char) {
        let byte_pos = self.char_to_byte(self.cursor_pos);
        self.query.insert(byte_pos, ch);
        self.cursor_pos += 1;
    }

    fn delete_before(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let byte_end = self.char_to_byte(self.cursor_pos);
        let byte_start = self.char_to_byte(self.cursor_pos - 1);
        self.query.drain(byte_start..byte_end);
        self.cursor_pos -= 1;
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.results = self.all_entries.clone();
            self.selected_index = 0;
            return;
        }

        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, PaletteEntry)> = self
            .all_entries
            .iter()
            .filter_map(|entry| {
                matcher
                    .fuzzy_match(&entry.label, &self.query)
                    .map(|score| (score, entry.clone()))
            })
            .collect();

        // Sort highest score first.
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.results = scored.into_iter().map(|(_, e)| e).collect();
        self.selected_index = 0;
    }

    fn move_up(&mut self) {
        if self.results.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.results.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.results.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.results.len();
    }

    /// Activate the currently selected entry: update store navigation state and
    /// return an Action if messages need to be fetched. Closes the palette.
    fn select_current(&mut self, store: &mut Store) -> Option<Action> {
        let entry = self.results.get(self.selected_index)?.clone();
        self.close();

        match entry.kind {
            PaletteEntryKind::Server => {
                if let Some(guild_id) = entry.guild_id {
                    store.ui.selected_guild = Some(guild_id);
                    store.ui.dm_mode = false;
                    // Pick the first text channel in the guild automatically.
                    let first_text = store
                        .guilds
                        .get_channels_for_guild(guild_id)
                        .into_iter()
                        .find(|ch| ch.kind == ChannelKind::Text)
                        .map(|ch| ch.id);
                    if let Some(channel_id) = first_text {
                        store.ui.selected_channel = Some(channel_id);
                        store.ui.message_scroll_offset = 0;
                        return Some(Action::FetchMessages {
                            channel_id,
                            before: None,
                            limit: 50,
                        });
                    }
                }
                None
            }
            PaletteEntryKind::Channel => {
                if let (Some(guild_id), Some(channel_id)) = (entry.guild_id, entry.channel_id) {
                    store.ui.selected_guild = Some(guild_id);
                    store.ui.selected_channel = Some(channel_id);
                    store.ui.dm_mode = false;
                    store.ui.message_scroll_offset = 0;
                    Some(Action::FetchMessages {
                        channel_id,
                        before: None,
                        limit: 50,
                    })
                } else {
                    None
                }
            }
            PaletteEntryKind::Dm => {
                if let Some(channel_id) = entry.channel_id {
                    store.ui.selected_channel = Some(channel_id);
                    store.ui.dm_mode = true;
                    store.ui.selected_guild = None;
                    store.ui.message_scroll_offset = 0;
                    Some(Action::FetchMessages {
                        channel_id,
                        before: None,
                        limit: 50,
                    })
                } else {
                    None
                }
            }
        }
    }
}

impl Component for CommandPalette {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if !self.visible || store.ui.focus != FocusTarget::CommandPalette {
            return Ok(None);
        }

        // Ctrl+j / Ctrl+k for navigation inside the palette.
        if key.modifiers == KeyModifiers::CONTROL {
            match key.code {
                KeyCode::Char('j') => {
                    self.move_down();
                    return Ok(None);
                }
                KeyCode::Char('k') => {
                    self.move_up();
                    return Ok(None);
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => {
                self.close();
                store.ui.focus = FocusTarget::MessageList;
                store.ui.input_mode = crate::store::state::InputMode::Normal;
            }

            KeyCode::Up => {
                self.move_up();
            }

            KeyCode::Down => {
                self.move_down();
            }

            KeyCode::Enter => {
                let action = self.select_current(store);
                store.ui.focus = FocusTarget::MessageList;
                store.ui.input_mode = crate::store::state::InputMode::Normal;
                return Ok(action);
            }

            KeyCode::Backspace => {
                self.delete_before();
                self.refilter();
            }

            KeyCode::Char(ch) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
                self.insert_char(ch);
                self.refilter();
            }

            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _store: &Store) {
        if !self.visible {
            return;
        }

        // Floating panel: 50% width, 60% height, centered.
        let panel_width = (area.width / 2).max(40).min(area.width);
        let panel_height = (area.height * 3 / 5).max(10).min(area.height);
        let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
        let panel_y = area.y + (area.height.saturating_sub(panel_height)) / 2;

        let panel_area = Rect {
            x: panel_x,
            y: panel_y,
            width: panel_width,
            height: panel_height,
        };

        // Clear the area beneath the panel (acts as the overlay).
        frame.render_widget(Clear, panel_area);

        // Outer block.
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::ACCENT))
            .style(Style::default().bg(theme::BG_SECONDARY))
            .title(Span::styled(" Command Palette ", Style::default().fg(theme::ACCENT)));
        let inner_area = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        // Split inner area: search input (3 lines) + results list.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(inner_area);

        let input_area = chunks[0];
        let results_area = chunks[1];

        // --- Search input ---
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));
        let input_inner = input_block.inner(input_area);
        frame.render_widget(input_block, input_area);

        if self.query.is_empty() {
            let placeholder = Paragraph::new(Span::styled(
                "Type to search servers, channels, DMs...",
                theme::muted(),
            ));
            frame.render_widget(placeholder, input_inner);
            // Cursor at start.
            if input_inner.width > 0 && input_inner.height > 0 {
                frame.set_cursor_position(Position {
                    x: input_inner.x,
                    y: input_inner.y,
                });
            }
        } else {
            let p = Paragraph::new(Span::styled(
                self.query.as_str(),
                Style::default().fg(theme::TEXT_PRIMARY),
            ));
            frame.render_widget(p, input_inner);
            // Show cursor after query text.
            let col = self.cursor_pos as u16;
            if input_inner.width > 0 && input_inner.height > 0 {
                let cx = (input_inner.x + col).min(input_inner.x + input_inner.width.saturating_sub(1));
                frame.set_cursor_position(Position {
                    x: cx,
                    y: input_inner.y,
                });
            }
        }

        // --- Results list ---
        let visible_count = results_area.height as usize;

        // Scroll window so selected is always visible.
        let scroll_offset = if self.results.is_empty() || visible_count == 0 {
            0
        } else {
            let sel = self.selected_index;
            if sel < visible_count {
                0
            } else {
                sel - visible_count + 1
            }
        };

        let items: Vec<ListItem> = self
            .results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_count)
            .map(|(i, entry)| {
                let icon = match entry.kind {
                    PaletteEntryKind::Server => "  ",
                    PaletteEntryKind::Channel => "# ",
                    PaletteEntryKind::Dm => "@ ",
                };
                let label_style = if i == self.selected_index {
                    Style::default().fg(theme::TEXT_PRIMARY).bg(theme::BG_TERTIARY)
                } else {
                    Style::default().fg(theme::TEXT_SECONDARY)
                };
                let context_style = if i == self.selected_index {
                    Style::default().fg(theme::TEXT_MUTED).bg(theme::BG_TERTIARY)
                } else {
                    Style::default().fg(theme::TEXT_MUTED)
                };

                let line = if entry.context.is_empty() {
                    Line::from(vec![
                        Span::styled(icon, label_style),
                        Span::styled(entry.label.as_str(), label_style),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(icon, label_style),
                        Span::styled(entry.label.as_str(), label_style),
                        Span::styled("  ", context_style),
                        Span::styled(entry.context.as_str(), context_style),
                    ])
                };

                ListItem::new(line)
            })
            .collect();

        if items.is_empty() {
            let empty = Paragraph::new(Span::styled("No results", theme::muted()));
            frame.render_widget(empty, results_area);
        } else {
            let list = List::new(items).style(Style::default().bg(theme::BG_SECONDARY));
            frame.render_widget(list, results_area);
        }
    }
}

/// Build the full flat list of navigable entries from the store.
fn build_entries(store: &Store) -> Vec<PaletteEntry> {
    let mut entries = Vec::new();

    // Servers + their text channels.
    for guild in &store.guilds.guilds {
        entries.push(PaletteEntry {
            label: guild.name.clone(),
            kind: PaletteEntryKind::Server,
            guild_id: Some(guild.id),
            channel_id: None,
            context: String::new(),
        });

        for channel in store.guilds.get_channels_for_guild(guild.id) {
            // Only include navigable channel types.
            match channel.kind {
                ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum => {}
                ChannelKind::Voice | ChannelKind::Category => continue,
            }
            entries.push(PaletteEntry {
                label: channel.name.clone(),
                kind: PaletteEntryKind::Channel,
                guild_id: Some(guild.id),
                channel_id: Some(channel.id),
                context: guild.name.clone(),
            });
        }
    }

    // DM channels.
    for dm in &store.dm_channels {
        let label = dm.recipient_names.join(", ");
        entries.push(PaletteEntry {
            label,
            kind: PaletteEntryKind::Dm,
            guild_id: None,
            channel_id: Some(dm.channel_id),
            context: String::new(),
        });
    }

    entries
}
