use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use twilight_model::id::marker::{ChannelMarker, MessageMarker};
use twilight_model::id::Id;

use crate::discord::actions::Action;
use crate::store::state::FocusTarget;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::emoji_data::{EMOJI_CATEGORIES, EMOJI_DATA};
use crate::tui::theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerMode {
    QuickReact,
    Search,
    Browse,
}

pub struct EmojiPicker {
    pub mode: PickerMode,
    pub visible: bool,
    // QuickReact state: indices 0..recent_emojis.len() are emojis,
    // then index recent_emojis.len() = [search], recent_emojis.len()+1 = [browse]
    quick_react_index: usize,
    // Search state
    search_query: String,
    search_cursor: usize,
    search_results: Vec<(&'static str, &'static str)>,
    search_selected: usize,
    // Browse state
    browse_category: usize,
    browse_index: usize,
    // Data
    recent_emojis: Vec<String>,
    pub pending_channel_id: Option<Id<ChannelMarker>>,
    pub pending_message_id: Option<Id<MessageMarker>>,
}

impl EmojiPicker {
    pub fn new(recent: Vec<String>) -> Self {
        Self {
            mode: PickerMode::QuickReact,
            visible: false,
            quick_react_index: 0,
            search_query: String::new(),
            search_cursor: 0,
            search_results: Vec::new(),
            search_selected: 0,
            browse_category: 0,
            browse_index: 0,
            recent_emojis: recent,
            pending_channel_id: None,
            pending_message_id: None,
        }
    }

    pub fn open(&mut self, channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>) {
        self.pending_channel_id = Some(channel_id);
        self.pending_message_id = Some(message_id);
        self.mode = PickerMode::QuickReact;
        self.visible = true;
        self.quick_react_index = 0;
        self.search_query.clear();
        self.search_cursor = 0;
        self.search_results.clear();
        self.search_selected = 0;
        self.browse_category = 0;
        self.browse_index = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.pending_channel_id = None;
        self.pending_message_id = None;
        self.search_query.clear();
        self.search_cursor = 0;
        self.search_results.clear();
    }

    /// Return the currently highlighted emoji string, if any.
    pub fn selected_emoji(&self) -> Option<String> {
        match self.mode {
            PickerMode::QuickReact => {
                let n = self.recent_emojis.len();
                if self.quick_react_index < n {
                    self.recent_emojis.get(self.quick_react_index).cloned()
                } else {
                    // [search] or [browse] button selected - no emoji yet
                    None
                }
            }
            PickerMode::Search => {
                self.search_results
                    .get(self.search_selected)
                    .map(|(_, e)| (*e).to_string())
            }
            PickerMode::Browse => {
                let cat_names = EMOJI_CATEGORIES.get(self.browse_category)?;
                let name = cat_names.1.get(self.browse_index)?;
                EMOJI_DATA
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, e)| (*e).to_string())
            }
        }
    }

    // --- private helpers ---

    fn quick_react_slots(&self) -> usize {
        // emojis + [search] + [browse]
        self.recent_emojis.len() + 2
    }

    fn search_insert_char(&mut self, ch: char) {
        let byte_pos = self
            .search_query
            .char_indices()
            .nth(self.search_cursor)
            .map(|(b, _)| b)
            .unwrap_or(self.search_query.len());
        self.search_query.insert(byte_pos, ch);
        self.search_cursor += 1;
        self.refilter_search();
    }

    fn search_delete_before(&mut self) {
        if self.search_cursor == 0 {
            return;
        }
        let byte_end = self
            .search_query
            .char_indices()
            .nth(self.search_cursor)
            .map(|(b, _)| b)
            .unwrap_or(self.search_query.len());
        let byte_start = self
            .search_query
            .char_indices()
            .nth(self.search_cursor - 1)
            .map(|(b, _)| b)
            .unwrap_or(0);
        self.search_query.drain(byte_start..byte_end);
        self.search_cursor -= 1;
        self.refilter_search();
    }

    fn refilter_search(&mut self) {
        if self.search_query.is_empty() {
            // Show all emojis when query is empty
            self.search_results = EMOJI_DATA.iter().map(|(n, e)| (*n, *e)).collect();
        } else {
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(i64, &'static str, &'static str)> = EMOJI_DATA
                .iter()
                .filter_map(|(name, emoji)| {
                    matcher
                        .fuzzy_match(name, &self.search_query)
                        .map(|score| (score, *name, *emoji))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.search_results = scored.into_iter().map(|(_, n, e)| (n, e)).collect();
        }
        self.search_selected = 0;
    }

    fn browse_category_emojis(&self) -> &[&'static str] {
        EMOJI_CATEGORIES
            .get(self.browse_category)
            .map(|(_, names)| *names)
            .unwrap_or(&[])
    }

    fn build_add_reaction(&self) -> Option<Action> {
        let emoji = self.selected_emoji()?;
        let channel_id = self.pending_channel_id?;
        let message_id = self.pending_message_id?;
        Some(Action::AddReaction {
            channel_id,
            message_id,
            emoji,
        })
    }
}

impl Component for EmojiPicker {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if !self.visible || store.ui.focus != FocusTarget::EmojiPicker {
            return Ok(None);
        }

        match self.mode {
            PickerMode::QuickReact => {
                let slots = self.quick_react_slots();
                match key.code {
                    KeyCode::Esc => {
                        self.close();
                        store.ui.focus = FocusTarget::MessageList;
                    }
                    KeyCode::Left => {
                        if self.quick_react_index > 0 {
                            self.quick_react_index -= 1;
                        } else {
                            self.quick_react_index = slots.saturating_sub(1);
                        }
                    }
                    KeyCode::Right => {
                        self.quick_react_index = (self.quick_react_index + 1) % slots;
                    }
                    KeyCode::Enter => {
                        let n = self.recent_emojis.len();
                        if self.quick_react_index == n {
                            // Switch to Search mode
                            self.mode = PickerMode::Search;
                            self.refilter_search();
                        } else if self.quick_react_index == n + 1 {
                            // Switch to Browse mode
                            self.mode = PickerMode::Browse;
                        } else {
                            // Select the emoji
                            let action = self.build_add_reaction();
                            self.close();
                            store.ui.focus = FocusTarget::MessageList;
                            return Ok(action);
                        }
                    }
                    _ => {}
                }
            }
            PickerMode::Search => {
                match key.code {
                    KeyCode::Esc => {
                        // Return to QuickReact
                        self.mode = PickerMode::QuickReact;
                        self.search_query.clear();
                        self.search_cursor = 0;
                        self.search_results.clear();
                    }
                    KeyCode::Up => {
                        if !self.search_results.is_empty() {
                            if self.search_selected == 0 {
                                self.search_selected = self.search_results.len() - 1;
                            } else {
                                self.search_selected -= 1;
                            }
                        }
                    }
                    KeyCode::Down => {
                        if !self.search_results.is_empty() {
                            self.search_selected =
                                (self.search_selected + 1) % self.search_results.len();
                        }
                    }
                    KeyCode::Enter => {
                        let action = self.build_add_reaction();
                        self.close();
                        store.ui.focus = FocusTarget::MessageList;
                        return Ok(action);
                    }
                    KeyCode::Backspace => {
                        self.search_delete_before();
                    }
                    KeyCode::Char(ch)
                        if key.modifiers == KeyModifiers::NONE
                            || key.modifiers == KeyModifiers::SHIFT =>
                    {
                        self.search_insert_char(ch);
                    }
                    _ => {}
                }
            }
            PickerMode::Browse => {
                match key.code {
                    KeyCode::Esc => {
                        // Return to QuickReact
                        self.mode = PickerMode::QuickReact;
                    }
                    KeyCode::Tab => {
                        // Next category
                        self.browse_category =
                            (self.browse_category + 1) % EMOJI_CATEGORIES.len();
                        self.browse_index = 0;
                    }
                    KeyCode::BackTab => {
                        // Previous category
                        if self.browse_category == 0 {
                            self.browse_category = EMOJI_CATEGORIES.len() - 1;
                        } else {
                            self.browse_category -= 1;
                        }
                        self.browse_index = 0;
                    }
                    KeyCode::Left => {
                        if self.browse_index > 0 {
                            self.browse_index -= 1;
                        }
                    }
                    KeyCode::Right => {
                        let count = self.browse_category_emojis().len();
                        if count > 0 && self.browse_index < count - 1 {
                            self.browse_index += 1;
                        }
                    }
                    KeyCode::Up => {
                        // Move up one row in the grid (cols = 10)
                        let cols = 10usize;
                        if self.browse_index >= cols {
                            self.browse_index -= cols;
                        }
                    }
                    KeyCode::Down => {
                        let cols = 10usize;
                        let count = self.browse_category_emojis().len();
                        let new_idx = self.browse_index + cols;
                        if new_idx < count {
                            self.browse_index = new_idx;
                        }
                    }
                    KeyCode::Enter => {
                        let action = self.build_add_reaction();
                        self.close();
                        store.ui.focus = FocusTarget::MessageList;
                        return Ok(action);
                    }
                    _ => {}
                }
            }
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _store: &Store) {
        if !self.visible {
            return;
        }

        match self.mode {
            PickerMode::QuickReact => self.render_quick_react(frame, area),
            PickerMode::Search => self.render_search(frame, area),
            PickerMode::Browse => self.render_browse(frame, area),
        }
    }
}

impl EmojiPicker {
    fn render_quick_react(&self, frame: &mut Frame, area: Rect) {
        // Small horizontal bar near the bottom-center
        let n = self.recent_emojis.len();
        // Each emoji slot: 3 chars wide, [search] and [browse] buttons
        let slot_width = 3u16;
        let btn_width = 8u16; // "[search]" and "[browse]"
        let total_width = (n as u16 * slot_width) + btn_width * 2 + 2; // +2 for borders
        let panel_width = total_width.max(30).min(area.width);
        let panel_height = 3u16;

        let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
        let panel_y = area.y + area.height.saturating_sub(panel_height + 2);

        let panel_area = Rect {
            x: panel_x,
            y: panel_y,
            width: panel_width,
            height: panel_height,
        };

        frame.render_widget(Clear, panel_area);

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::ACCENT))
            .style(Style::default().bg(theme::BG_SECONDARY))
            .title(Span::styled(
                " React ",
                Style::default().fg(theme::ACCENT),
            ));
        let inner_area = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        // Build the line of emoji slots
        let mut spans: Vec<Span> = Vec::new();
        for (i, emoji) in self.recent_emojis.iter().enumerate() {
            let style = if i == self.quick_react_index {
                Style::default()
                    .fg(theme::TEXT_PRIMARY)
                    .bg(theme::BG_TERTIARY)
            } else {
                Style::default().fg(theme::TEXT_PRIMARY)
            };
            spans.push(Span::styled(format!("{} ", emoji), style));
        }

        // [search] button
        let search_idx = n;
        let search_style = if self.quick_react_index == search_idx {
            Style::default()
                .fg(theme::ACCENT)
                .bg(theme::BG_TERTIARY)
        } else {
            Style::default().fg(theme::TEXT_SECONDARY)
        };
        spans.push(Span::styled("[search]", search_style));

        // [browse] button
        let browse_idx = n + 1;
        let browse_style = if self.quick_react_index == browse_idx {
            Style::default()
                .fg(theme::ACCENT)
                .bg(theme::BG_TERTIARY)
        } else {
            Style::default().fg(theme::TEXT_SECONDARY)
        };
        spans.push(Span::styled("[browse]", browse_style));

        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), inner_area);
    }

    fn render_search(&self, frame: &mut Frame, area: Rect) {
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

        frame.render_widget(Clear, panel_area);

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::ACCENT))
            .style(Style::default().bg(theme::BG_SECONDARY))
            .title(Span::styled(
                " Search Emoji (Esc to go back) ",
                Style::default().fg(theme::ACCENT),
            ));
        let inner_area = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(inner_area);

        let input_area = chunks[0];
        let results_area = chunks[1];

        // Input box
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));
        let input_inner = input_block.inner(input_area);
        frame.render_widget(input_block, input_area);

        if self.search_query.is_empty() {
            let placeholder = Paragraph::new(Span::styled(
                "Type emoji name...",
                theme::muted(),
            ));
            frame.render_widget(placeholder, input_inner);
            if input_inner.width > 0 && input_inner.height > 0 {
                frame.set_cursor_position(Position {
                    x: input_inner.x,
                    y: input_inner.y,
                });
            }
        } else {
            let p = Paragraph::new(Span::styled(
                self.search_query.as_str(),
                Style::default().fg(theme::TEXT_PRIMARY),
            ));
            frame.render_widget(p, input_inner);
            let col = self.search_cursor as u16;
            if input_inner.width > 0 && input_inner.height > 0 {
                let cx = (input_inner.x + col)
                    .min(input_inner.x + input_inner.width.saturating_sub(1));
                frame.set_cursor_position(Position {
                    x: cx,
                    y: input_inner.y,
                });
            }
        }

        // Results list
        let visible_count = results_area.height as usize;
        let scroll_offset = if self.search_results.is_empty() || visible_count == 0 {
            0
        } else {
            let sel = self.search_selected;
            if sel < visible_count {
                0
            } else {
                sel - visible_count + 1
            }
        };

        let items: Vec<ListItem> = self
            .search_results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_count)
            .map(|(i, (name, emoji))| {
                let style = if i == self.search_selected {
                    Style::default()
                        .fg(theme::TEXT_PRIMARY)
                        .bg(theme::BG_TERTIARY)
                } else {
                    Style::default().fg(theme::TEXT_SECONDARY)
                };
                let line = Line::from(vec![
                    Span::styled(format!("{} ", emoji), style),
                    Span::styled(format!(":{name}:"), style),
                ]);
                ListItem::new(line)
            })
            .collect();

        if items.is_empty() {
            let empty =
                Paragraph::new(Span::styled("No results", theme::muted()));
            frame.render_widget(empty, results_area);
        } else {
            let list = List::new(items).style(Style::default().bg(theme::BG_SECONDARY));
            frame.render_widget(list, results_area);
        }
    }

    fn render_browse(&self, frame: &mut Frame, area: Rect) {
        let panel_width = (area.width * 3 / 4).max(50).min(area.width);
        let panel_height = (area.height * 3 / 4).max(12).min(area.height);
        let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
        let panel_y = area.y + (area.height.saturating_sub(panel_height)) / 2;

        let panel_area = Rect {
            x: panel_x,
            y: panel_y,
            width: panel_width,
            height: panel_height,
        };

        frame.render_widget(Clear, panel_area);

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::ACCENT))
            .style(Style::default().bg(theme::BG_SECONDARY))
            .title(Span::styled(
                " Browse Emoji  Tab/Shift+Tab: category  Arrows: navigate  Enter: select  Esc: back ",
                Style::default().fg(theme::ACCENT),
            ));
        let inner_area = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner_area);

        let tab_area = chunks[0];
        let grid_area = chunks[1];

        // Category tabs
        let tab_spans: Vec<Span> = EMOJI_CATEGORIES
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                if i == self.browse_category {
                    Span::styled(
                        format!(" {} ", name),
                        Style::default()
                            .fg(theme::TEXT_PRIMARY)
                            .bg(theme::ACCENT),
                    )
                } else {
                    Span::styled(
                        format!(" {} ", name),
                        Style::default().fg(theme::TEXT_SECONDARY),
                    )
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(Line::from(tab_spans)), tab_area);

        // Emoji grid - 10 columns
        let cols = 10usize;
        let emoji_names = self.browse_category_emojis();
        let mut lines: Vec<Line> = Vec::new();
        let mut row_start = 0;

        while row_start < emoji_names.len() {
            let row_end = (row_start + cols).min(emoji_names.len());
            let row_names = &emoji_names[row_start..row_end];

            let spans: Vec<Span> = row_names
                .iter()
                .enumerate()
                .map(|(col, name)| {
                    let idx = row_start + col;
                    let emoji_char = EMOJI_DATA
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, e)| *e)
                        .unwrap_or("?");
                    let style = if idx == self.browse_index {
                        Style::default()
                            .fg(theme::TEXT_PRIMARY)
                            .bg(theme::BG_TERTIARY)
                    } else {
                        Style::default().fg(theme::TEXT_PRIMARY)
                    };
                    Span::styled(format!("{} ", emoji_char), style)
                })
                .collect();

            lines.push(Line::from(spans));
            row_start += cols;
        }

        let grid = Paragraph::new(lines).style(Style::default().bg(theme::BG_SECONDARY));
        frame.render_widget(grid, grid_area);
    }
}
