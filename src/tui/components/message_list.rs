use std::cell::Cell;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::messages::StoredMessage;
use crate::store::state::{FocusTarget, PaneView};
use crate::tui::component::Component;
use crate::tui::components::message::render_message_full;
use crate::tui::keybindings::KeyAction;
use crate::tui::theme;

/// Parse an ISO 8601 timestamp string and return the `NaiveDate` (UTC).
/// Returns `None` if parsing fails.
fn message_date(timestamp: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).date_naive())
}

/// Build a centered date separator line, e.g. `── March 22, 2026 ──`.
fn date_separator_line(date: NaiveDate, width: u16) -> Line<'static> {
    let label = date.format("%B %-d, %Y").to_string();
    let inner = format!("── {} ──", label);
    // Center the inner text within `width` using spaces.
    let pad = (width as usize).saturating_sub(inner.len()) / 2;
    let centered = format!("{}{}", " ".repeat(pad), inner);
    Line::from(Span::styled(centered, theme::muted()))
}

/// Build a centered "── NEW ──" separator line in the mention/alert color.
fn new_separator_line(width: u16) -> Line<'static> {
    let inner = "── NEW ──";
    let pad = (width as usize).saturating_sub(inner.len()) / 2;
    let centered = format!("{}{}", " ".repeat(pad), inner);
    Line::from(Span::styled(
        centered,
        Style::default().fg(theme::MENTION),
    ))
}

pub struct MessageList {
    // Wrapped in Cell so they can be mutated in render(&self).
    selected_index: Cell<Option<usize>>,
    auto_scroll: Cell<bool>,
    is_fetching_history: Cell<bool>,
    last_message_count: Cell<usize>,
    last_channel_id: Cell<Option<u64>>,
}

impl MessageList {
    pub fn new() -> Self {
        Self {
            selected_index: Cell::new(None),
            auto_scroll: Cell::new(true),
            is_fetching_history: Cell::new(false),
            last_message_count: Cell::new(0),
            last_channel_id: Cell::new(None),
        }
    }

    pub fn get_selected_message<'a>(&self, store: &'a Store) -> Option<&'a StoredMessage> {
        let channel_id = store.ui.active_channel()?;
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

    /// Scroll messages by a signed delta. Positive = newer, negative = older.
    /// Returns true if we hit the top and should fetch older history.
    pub fn scroll_by(&self, delta: isize, message_count: usize) -> bool {
        if message_count == 0 {
            return false;
        }
        let last = message_count - 1;
        let current = self.selected_index.get().unwrap_or(last) as isize;
        let new_idx = (current + delta).clamp(0, last as isize) as usize;
        self.selected_index.set(Some(new_idx));
        self.auto_scroll.set(new_idx == last);
        // Return true if we hit the top and should fetch older messages
        new_idx == 0 && !self.is_fetching_history.get()
    }

    /// Mark that we're fetching history (prevents duplicate fetches).
    pub fn set_fetching_history(&self) {
        self.is_fetching_history.set(true);
    }
}

impl Component for MessageList {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::MessageList {
            return Ok(None);
        }

        let channel_id = match store.ui.active_channel() {
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

        match (key.code, key.modifiers) {
            // Navigation
            (KeyCode::Down, KeyModifiers::NONE) => {
                if message_count == 0 {
                    return Ok(None);
                }
                self.auto_scroll.set(false);
                let current = selected.unwrap_or(last_index);
                self.selected_index.set(Some(current.saturating_add(1).min(last_index)));
            }
            (KeyCode::Up, KeyModifiers::NONE) => {
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
            (KeyCode::PageUp, _) => {
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
            (KeyCode::PageDown, _) => {
                if message_count == 0 {
                    return Ok(None);
                }
                self.auto_scroll.set(false);
                let current = selected.unwrap_or(last_index);
                self.selected_index.set(Some(current.saturating_add(10).min(last_index)));
            }
            (KeyCode::Home, _) => {
                if message_count > 0 {
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
                }
            }
            (KeyCode::End, _) => {
                if message_count > 0 {
                    self.selected_index.set(Some(last_index));
                    self.auto_scroll.set(true);
                }
            }

            // Focus transitions
            (KeyCode::Enter, KeyModifiers::NONE) => {
                // If the selected message has a reply context, try to scroll to the
                // original (referenced) message in the buffer. Otherwise, focus input.
                let reply_msg_id = self
                    .get_selected_message(store)
                    .and_then(|m| m.reply_to.as_ref().map(|_| m.id));

                if let Some(sel_id) = reply_msg_id {
                    // Find the message that this one is replying to. We look for any
                    // message that appeared *before* the current one with a matching
                    // content preview or simply the nearest earlier message — since
                    // we only store a preview, use the message immediately before it.
                    let channel_id2 = store.ui.active_channel();
                    if let Some(cid) = channel_id2 {
                        if let Some(buf) = store.messages.get(&cid) {
                            let msgs = buf.messages();
                            // Find the index of the selected message.
                            if let Some(cur_pos) = msgs.iter().position(|m| m.id == sel_id) {
                                // The referenced message is somewhere before cur_pos.
                                // Since we don't store the referenced message id in
                                // ReplyContext (only author+preview), jump to the
                                // nearest older message from the same author as shown in
                                // the reply context.
                                let reply_author = msgs[cur_pos]
                                    .reply_to
                                    .as_ref()
                                    .map(|r| r.author_name.clone());
                                let target_idx = if let Some(author) = reply_author {
                                    // Search backwards for a message by that author.
                                    msgs.iter()
                                        .enumerate()
                                        .take(cur_pos)
                                        .rev()
                                        .find(|(_, m)| m.author_name == author)
                                        .map(|(i, _)| i)
                                } else {
                                    None
                                };
                                if let Some(idx) = target_idx {
                                    self.selected_index.set(Some(idx));
                                    self.auto_scroll.set(false);
                                    return Ok(None);
                                }
                            }
                        }
                    }
                }
                store.ui.focus = FocusTarget::MessageInput;
            }
            (KeyCode::Esc | KeyCode::Left, KeyModifiers::NONE) => {
                if !store.ui.pop_pane() {
                    store.ui.focus = FocusTarget::ChannelTree;
                }
            }

            // Message actions — use Ctrl+key for discoverability
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::Reply)));
            }
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::EditMessage)));
            }
            (KeyCode::Delete | KeyCode::Backspace, _) => {
                return Ok(Some(Action::ComponentKeyAction(KeyAction::DeleteMessage)));
            }
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
                if let Some(msg) = self.get_selected_message(store) {
                    let msg_id = msg.id;
                    // Look for a thread whose first message is this one (Discord threads are
                    // channels whose id matches the starter message id), or find by channel.
                    if let Some(channel_id) = store.ui.active_channel() {
                        // Find a thread associated with this channel where the thread id
                        // matches the message id (Discord's thread-from-message convention)
                        // or any thread in this channel.
                        let thread_match = store
                            .active_threads
                            .get(&channel_id)
                            .and_then(|threads| {
                                // Prefer thread whose id == message id (starter message)
                                threads.iter().find(|t| t.id.get() == msg_id.get())
                                    .or_else(|| threads.first())
                            })
                            .map(|t| (t.parent_channel, t.id));

                        if let Some((parent_channel, thread_id)) = thread_match {
                            store.ui.push_pane(PaneView::Thread { parent_channel, thread_id });
                            return Ok(Some(Action::FetchMessages {
                                channel_id: thread_id,
                                before: None,
                                limit: 50,
                            }));
                        }
                    }
                }
            }

            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let channel_id = match store.ui.active_channel() {
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

        // Determine the index of the first "new" (unread) message, if any.
        // When there are N unreads and the user has scrolled up, show a separator
        // before the (total - N)th message.
        let new_separator_before: Option<usize> = if !self.auto_scroll.get() {
            let unread_count = store
                .notifications
                .get(channel_id)
                .map(|n| n.unread_count as usize)
                .unwrap_or(0);
            if unread_count > 0 && unread_count < messages.len() {
                Some(messages.len() - unread_count)
            } else {
                None
            }
        } else {
            None
        };

        let mut prev_date: Option<NaiveDate> = None;

        for (msg_idx, msg) in messages.iter().enumerate() {
            // Insert the "── NEW ──" separator before the first unread message.
            if new_separator_before == Some(msg_idx) {
                all_lines.push(new_separator_line(msg_area.width));
                line_owners.push(msg_idx);
            }

            // Insert a date separator when the calendar day changes.
            let cur_date = message_date(&msg.timestamp);
            if let Some(date) = cur_date {
                if prev_date != Some(date) {
                    let sep = date_separator_line(date, msg_area.width);
                    all_lines.push(sep);
                    // Owned by the current message so it highlights together with it.
                    line_owners.push(msg_idx);
                }
                prev_date = Some(date);
            }

            // Determine whether this message is compact (same author, within 5 minutes,
            // no date boundary, no reply context).
            let compact = if msg_idx == 0 || msg.reply_to.is_some() {
                false
            } else {
                let prev = &messages[msg_idx - 1];
                let same_author = prev.author_id == msg.author_id;
                // Same date (no separator was just inserted) is implied when same_author
                // is true and the timestamps are within 5 minutes.
                let within_5_min = same_author && {
                    use chrono::DateTime;
                    let t_prev = DateTime::parse_from_rfc3339(&prev.timestamp).ok();
                    let t_cur = DateTime::parse_from_rfc3339(&msg.timestamp).ok();
                    match (t_prev, t_cur) {
                        (Some(p), Some(c)) => {
                            let diff = c.signed_duration_since(p).num_seconds().abs();
                            diff <= 5 * 60
                        }
                        _ => false,
                    }
                };
                // Also suppress compactness when a date separator was just emitted.
                let date_boundary = cur_date
                    .and_then(|d| message_date(&messages[msg_idx - 1].timestamp).map(|pd| pd != d))
                    .unwrap_or(false);
                same_author && within_5_min && !date_boundary
            };

            // Look up a thread whose id matches the message id (Discord starter message convention)
            let thread_info = store
                .ui
                .active_channel()
                .and_then(|cid| store.active_threads.get(&cid))
                .and_then(|threads| threads.iter().find(|t| t.id.get() == msg.id.get()));

            let rendered = render_message_full(msg, msg_area.width, thread_info, store.supports_images, compact);
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
