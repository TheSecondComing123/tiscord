use std::cell::Cell;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::messages::StoredMessage;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::components::message::render_message;
use crate::tui::keybindings::KeyAction;
use crate::tui::theme;

const CHORD_TIMEOUT: Duration = Duration::from_millis(500);

pub struct MessageList {
    // Wrapped in Cell so they can be mutated in render(&self).
    selected_index: Cell<Option<usize>>,
    auto_scroll: Cell<bool>,
    is_fetching_history: Cell<bool>,
    last_message_count: Cell<usize>,
    last_channel_id: Cell<Option<u64>>,
    pending_chord: Option<(KeyCode, Instant)>,
}

impl MessageList {
    pub fn new() -> Self {
        Self {
            selected_index: Cell::new(None),
            auto_scroll: Cell::new(true),
            is_fetching_history: Cell::new(false),
            last_message_count: Cell::new(0),
            last_channel_id: Cell::new(None),
            pending_chord: None,
        }
    }

    pub fn get_selected_message<'a>(&self, store: &'a Store) -> Option<&'a StoredMessage> {
        let channel_id = store.ui.selected_channel?;
        let buffer = store.messages.get(&channel_id)?;
        let index = self.selected_index.get()?;
        buffer.messages().get(index)
    }

    /// Reset selection and fetching state. Called when switching to a different channel.
    pub fn reset(&self) {
        self.selected_index.set(None);
        self.auto_scroll.set(true);
        self.is_fetching_history.set(false);
        self.last_message_count.set(0);
    }
}

impl Component for MessageList {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::MessageList {
            return Ok(None);
        }

        let channel_id = match store.ui.selected_channel {
            Some(id) => id,
            None => return Ok(None),
        };

        let buffer = match store.messages.get(&channel_id) {
            Some(buf) => buf,
            None => return Ok(None),
        };

        let message_count = buffer.len();
        let last_index = if message_count > 0 { message_count - 1 } else { 0 };
        let selected = self.selected_index.get();
        let is_fetching = self.is_fetching_history.get();

        // Handle pending chord
        if let Some((chord_key, chord_time)) = self.pending_chord.take() {
            if chord_time.elapsed() < CHORD_TIMEOUT
                && chord_key == KeyCode::Char('g')
                && key.code == KeyCode::Char('g')
            {
                self.selected_index.set(Some(0));
                self.auto_scroll.set(false);
                if !is_fetching {
                    let oldest_id = buffer.messages().front().map(|m| m.id);
                    self.is_fetching_history.set(true);
                    return Ok(Some(Action::FetchMessages {
                        channel_id,
                        before: oldest_id,
                        limit: 50,
                    }));
                }
                return Ok(None);
            }
            // Expired or unmatched chord - fall through and process key normally
        }

        match (key.code, key.modifiers) {
            // Navigation
            (KeyCode::Char('j') | KeyCode::Down, KeyModifiers::NONE) => {
                if message_count == 0 {
                    return Ok(None);
                }
                self.auto_scroll.set(false);
                let current = selected.unwrap_or(last_index);
                self.selected_index.set(Some(current.saturating_add(1).min(last_index)));
            }
            (KeyCode::Char('k') | KeyCode::Up, KeyModifiers::NONE) => {
                if message_count == 0 {
                    return Ok(None);
                }
                self.auto_scroll.set(false);
                let current = selected.unwrap_or(last_index);
                let new_idx = current.saturating_sub(1);
                self.selected_index.set(Some(new_idx));

                if new_idx == 0 && !is_fetching {
                    let oldest_id = buffer.messages().front().map(|m| m.id);
                    self.is_fetching_history.set(true);
                    return Ok(Some(Action::FetchMessages {
                        channel_id,
                        before: oldest_id,
                        limit: 50,
                    }));
                }
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                if message_count == 0 {
                    return Ok(None);
                }
                self.auto_scroll.set(false);
                let current = selected.unwrap_or(last_index);
                let new_idx = current.saturating_sub(10);
                self.selected_index.set(Some(new_idx));

                if new_idx == 0 && !is_fetching {
                    let oldest_id = buffer.messages().front().map(|m| m.id);
                    self.is_fetching_history.set(true);
                    return Ok(Some(Action::FetchMessages {
                        channel_id,
                        before: oldest_id,
                        limit: 50,
                    }));
                }
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if message_count == 0 {
                    return Ok(None);
                }
                self.auto_scroll.set(false);
                let current = selected.unwrap_or(last_index);
                self.selected_index.set(Some(current.saturating_add(10).min(last_index)));
            }
            // KeyCode::Char('G') arrives with SHIFT on some terminals; handle both.
            (KeyCode::Char('G'), _) => {
                if message_count > 0 {
                    self.selected_index.set(Some(last_index));
                    self.auto_scroll.set(true);
                }
            }
            // Start gg chord
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.pending_chord = Some((KeyCode::Char('g'), Instant::now()));
            }

            // Focus transitions
            (KeyCode::Enter | KeyCode::Char('i'), KeyModifiers::NONE) => {
                // Go to message input
                store.ui.focus = FocusTarget::MessageInput;
            }
            (KeyCode::Esc | KeyCode::Left, KeyModifiers::NONE) => {
                // Back to channel tree
                store.ui.focus = FocusTarget::ChannelTree;
            }

            // Message actions (return ComponentKeyAction for App to handle)
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::Reply)));
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::EditMessage)));
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::DeleteMessage)));
            }
            (KeyCode::Char('+'), KeyModifiers::NONE) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::OpenEmojiPicker)));
            }
            // P (uppercase) - pin or unpin the selected message
            (KeyCode::Char('P'), _) => {
                if let Some(msg) = self.get_selected_message(store) {
                    let message_id = msg.id;
                    if let Some(channel_id) = store.ui.selected_channel {
                        // Check if already pinned
                        let is_pinned = store
                            .pinned_messages
                            .get(&channel_id)
                            .and_then(|opt| opt.as_ref())
                            .map(|pins| pins.iter().any(|p| p.id == message_id))
                            .unwrap_or(false);

                        if is_pinned {
                            return Ok(Some(Action::UnpinMessage { channel_id, message_id }));
                        } else {
                            return Ok(Some(Action::PinMessage { channel_id, message_id }));
                        }
                    }
                }
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

        // Reset state when the channel changes.
        let channel_raw = channel_id.get();
        if self.last_channel_id.get() != Some(channel_raw) {
            self.reset();
            self.last_channel_id.set(Some(channel_raw));
        }

        let buffer = match store.messages.get(&channel_id) {
            Some(buf) => buf,
            None => {
                let placeholder = Paragraph::new("No messages")
                    .style(theme::muted())
                    .alignment(Alignment::Center);
                frame.render_widget(placeholder, area);
                return;
            }
        };

        let messages = buffer.messages();

        if messages.is_empty() {
            let placeholder = Paragraph::new("No messages")
                .style(theme::muted())
                .alignment(Alignment::Center);
            frame.render_widget(placeholder, area);
            return;
        }

        // If the buffer grew since last render, history arrived - clear the fetching flag.
        let current_count = buffer.len();
        let prev_count = self.last_message_count.get();
        if current_count != prev_count {
            if prev_count > 0 && current_count > prev_count {
                self.is_fetching_history.set(false);
            }
            self.last_message_count.set(current_count);
        }

        // Reserve one line at the top for the loading indicator when fetching.
        let (msg_area, indicator_area) = if self.is_fetching_history.get() && area.height > 1 {
            let chunks = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(area);
            (chunks[1], Some(chunks[0]))
        } else {
            (area, None)
        };

        if let Some(ind_area) = indicator_area {
            let loading = Paragraph::new("Loading history...")
                .style(Style::default().fg(theme::IDLE).bg(theme::BG_SECONDARY))
                .alignment(Alignment::Center);
            frame.render_widget(loading, ind_area);
        }

        // Build all lines, tracking which message index each line belongs to.
        // line_owners[i] = message index for the i-th rendered line.
        let mut all_lines: Vec<Line<'static>> = Vec::new();
        let mut line_owners: Vec<usize> = Vec::new();

        for (msg_idx, msg) in messages.iter().enumerate() {
            let rendered = render_message(msg, msg_area.width);
            let line_count = rendered.len();
            all_lines.extend(rendered);
            for _ in 0..line_count {
                line_owners.push(msg_idx);
            }
        }

        // Apply selection highlight to all lines belonging to the selected message.
        if let Some(sel_idx) = self.selected_index.get() {
            for (line_idx, &owner) in line_owners.iter().enumerate() {
                if owner == sel_idx {
                    let line = &mut all_lines[line_idx];
                    *line = line.clone().patch_style(Style::default().bg(theme::BG_SECONDARY));
                }
            }
        }

        let total_lines = all_lines.len() as u16;
        let visible_height = msg_area.height;

        // Compute the scroll offset for this frame.
        let scroll_offset: u16 = if self.auto_scroll.get() {
            // Always show the newest messages at the bottom.
            total_lines.saturating_sub(visible_height)
        } else if let Some(sel_idx) = self.selected_index.get() {
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

        frame.render_widget(paragraph, msg_area);
    }
}
