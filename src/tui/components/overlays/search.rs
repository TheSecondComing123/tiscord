use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

use crate::discord::actions::Action;
use crate::store::search::SearchScope;
use crate::store::state::FocusTarget;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct SearchOverlay {
    pub visible: bool,
    /// Local copy of the query being typed (synced to store on submit/search).
    query: String,
    cursor_pos: usize,
}

impl SearchOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            cursor_pos: 0,
        }
    }

    /// Open the overlay with the given initial scope and clear previous state.
    pub fn open(&mut self, store: &mut Store, scope: SearchScope) {
        self.visible = true;
        self.query.clear();
        self.cursor_pos = 0;
        store.search.clear();
        store.search.scope = Some(scope);
    }

    /// Close and clear all search state.
    pub fn close(&mut self, store: &mut Store) {
        self.visible = false;
        self.query.clear();
        self.cursor_pos = 0;
        store.search.clear();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // --- text input helpers ---

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

    fn move_up(&self, store: &mut Store) {
        if store.search.selected > 0 {
            store.search.selected -= 1;
        }
    }

    fn move_down(&self, store: &mut Store) {
        let max = store.search.results.len().saturating_sub(1);
        if store.search.selected < max {
            store.search.selected += 1;
        }
    }

    /// Toggle scope between CurrentChannel and Server (if a guild is selected).
    fn toggle_scope(&self, store: &mut Store) {
        let current_scope = store.search.scope.take();
        store.search.scope = match current_scope {
            Some(SearchScope::CurrentChannel(ch_id)) => {
                // Try to escalate to server scope.
                store
                    .ui
                    .selected_guild
                    .map(SearchScope::Server)
                    .or(Some(SearchScope::CurrentChannel(ch_id)))
            }
            Some(SearchScope::Server(_)) => {
                // Drop back to channel scope if one is selected.
                store
                    .ui
                    .selected_channel
                    .map(SearchScope::CurrentChannel)
                    .or_else(|| {
                        store.ui.selected_guild.map(SearchScope::Server)
                    })
            }
            None => store
                .ui
                .selected_channel
                .map(SearchScope::CurrentChannel),
        };
    }

    /// Dispatch a search action based on current query and scope.
    fn submit_search(&mut self, store: &mut Store) -> Option<Action> {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return None;
        }
        store.search.query = query.clone();
        store.search.loading = true;
        store.search.results.clear();
        store.search.selected = 0;

        let scope = store.search.scope.clone()?;
        Some(Action::SearchMessages { scope, query })
    }

    /// Activate the currently selected result: navigate to it and close the overlay.
    fn select_result(&mut self, store: &mut Store) -> Option<Action> {
        let result = store.search.results.get(store.search.selected)?.clone();
        let channel_id = result.channel_id;
        let message_id = result.message_id;

        // Navigate store to the result's channel.
        store.ui.selected_channel = Some(channel_id);
        store.ui.message_scroll_offset = 0;
        // Try to find the guild that owns this channel.
        let guild_id = store.guilds.guilds.iter().find_map(|g| {
            if g.channels.iter().any(|c| c.id == channel_id) {
                Some(g.id)
            } else {
                None
            }
        });
        if let Some(gid) = guild_id {
            store.ui.selected_guild = Some(gid);
        }

        self.close(store);

        Some(Action::NavigateToSearchResult {
            channel_id,
            message_id,
        })
    }
}

impl Component for SearchOverlay {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if !self.visible || store.ui.focus != FocusTarget::SearchOverlay {
            return Ok(None);
        }

        // Ctrl+/ toggles scope.
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('/') {
            self.toggle_scope(store);
            return Ok(None);
        }

        match key.code {
            KeyCode::Esc => {
                self.close(store);
                store.ui.focus = FocusTarget::MessageList;
            }

            KeyCode::Up => self.move_up(store),
            KeyCode::Down => self.move_down(store),

            KeyCode::Enter => {
                // If there are results, navigate to the selected one; otherwise submit.
                let action = if !store.search.results.is_empty() {
                    self.select_result(store)
                } else {
                    let action = self.submit_search(store);
                    action
                };
                if action.is_some() {
                    store.ui.focus = FocusTarget::MessageList;
                }
                return Ok(action);
            }

            KeyCode::Backspace => {
                self.delete_before();
            }

            KeyCode::Char(ch)
                if key.modifiers == KeyModifiers::NONE
                    || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.insert_char(ch);
            }

            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        if !self.visible {
            return;
        }

        // Floating panel: 55% width, 65% height, centered.
        let panel_width = (area.width * 55 / 100).max(50).min(area.width);
        let panel_height = (area.height * 65 / 100).max(12).min(area.height);
        let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
        let panel_y = area.y + (area.height.saturating_sub(panel_height)) / 2;

        let panel_area = Rect {
            x: panel_x,
            y: panel_y,
            width: panel_width,
            height: panel_height,
        };

        frame.render_widget(Clear, panel_area);

        // Outer block with title.
        let scope_label = match &store.search.scope {
            Some(SearchScope::CurrentChannel(id)) => {
                // Find channel name.
                let name = store.guilds.guilds.iter().find_map(|g| {
                    g.channels.iter().find(|c| c.id == *id).map(|c| c.name.as_str())
                });
                format!(" [# {}]", name.unwrap_or("channel"))
            }
            Some(SearchScope::Server(id)) => {
                let name = store
                    .guilds
                    .get_guild(*id)
                    .map(|g| g.name.as_str())
                    .unwrap_or("server");
                format!(" [@ {}]", name)
            }
            None => String::from(" [no scope]"),
        };

        let title = format!(" Message Search{} ", scope_label);
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::ACCENT))
            .style(Style::default().bg(theme::BG_SECONDARY))
            .title(Span::styled(title, Style::default().fg(theme::ACCENT)));
        let inner_area = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        // Split: input (3 lines) + hint (1 line) + results list.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner_area);

        let input_area = chunks[0];
        let hint_area = chunks[1];
        let results_area = chunks[2];

        // --- Search input ---
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));
        let input_inner = input_block.inner(input_area);
        frame.render_widget(input_block, input_area);

        if self.query.is_empty() {
            let placeholder = Paragraph::new(Span::styled(
                "Type to search messages...",
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
                self.query.as_str(),
                Style::default().fg(theme::TEXT_PRIMARY),
            ));
            frame.render_widget(p, input_inner);
            let col = self.cursor_pos as u16;
            if input_inner.width > 0 && input_inner.height > 0 {
                let cx = (input_inner.x + col)
                    .min(input_inner.x + input_inner.width.saturating_sub(1));
                frame.set_cursor_position(Position {
                    x: cx,
                    y: input_inner.y,
                });
            }
        }

        // --- Hint line ---
        let hint = Paragraph::new(Span::styled(
            " Enter: search/select  Up/Down: navigate  Ctrl+/: toggle scope  Esc: close",
            theme::muted(),
        ));
        frame.render_widget(hint, hint_area);

        // --- Results list ---
        if store.search.loading {
            let loading = Paragraph::new(Span::styled("Searching...", theme::muted()));
            frame.render_widget(loading, results_area);
            return;
        }

        if store.search.results.is_empty() && !store.search.query.is_empty() {
            let empty = Paragraph::new(Span::styled("No results found.", theme::muted()));
            frame.render_widget(empty, results_area);
            return;
        }

        let visible_count = results_area.height as usize;
        let selected = store.search.selected;
        let scroll_offset = if selected < visible_count {
            0
        } else {
            selected - visible_count + 1
        };

        // Determine if we're in server scope (show channel name column).
        let server_scope = matches!(&store.search.scope, Some(SearchScope::Server(_)));

        let items: Vec<ListItem> = store
            .search
            .results
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_count)
            .map(|(i, result)| {
                let is_selected = i == selected;
                let bg = if is_selected { theme::BG_TERTIARY } else { theme::BG_SECONDARY };
                let author_style = Style::default().fg(theme::ACCENT).bg(bg);
                let channel_style = Style::default().fg(theme::TEXT_MUTED).bg(bg);
                let content_style = Style::default().fg(theme::TEXT_PRIMARY).bg(bg);
                let ts_style = Style::default().fg(theme::TEXT_MUTED).bg(bg);

                let mut spans = vec![
                    Span::styled(format!("{} ", result.author_name), author_style),
                ];

                if server_scope && !result.channel_name.is_empty() {
                    spans.push(Span::styled(
                        format!("[#{}] ", result.channel_name),
                        channel_style,
                    ));
                }

                spans.push(Span::styled(result.content_preview.as_str(), content_style));
                spans.push(Span::styled(
                    format!("  {}", result.timestamp),
                    ts_style,
                ));

                ListItem::new(Line::from(spans))
            })
            .collect();

        if items.is_empty() {
            // No query submitted yet — show prompt.
            if store.search.query.is_empty() {
                let prompt = Paragraph::new(Span::styled(
                    "Press Enter to search.",
                    theme::muted(),
                ));
                frame.render_widget(prompt, results_area);
            }
        } else {
            let list = List::new(items).style(Style::default().bg(theme::BG_SECONDARY));
            frame.render_widget(list, results_area);
        }
    }
}
