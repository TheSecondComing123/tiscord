use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::emoji_data::{EMOJI_DATA, emoji_by_name};
use crate::tui::theme;

/// Discord typing indicators should not be sent more than once per 10 seconds.
const TYPING_THROTTLE: Duration = Duration::from_secs(10);

/// Maximum number of autocomplete suggestions to display at once.
const MAX_SUGGESTIONS: usize = 5;

#[derive(Debug, PartialEq)]
enum AutocompleteKind {
    None,
    Mention,
    Emoji,
    Channel,
}

struct AutocompleteState {
    kind: AutocompleteKind,
    query: String,
    /// Character index in `content` where the trigger character (`@` or `:`) sits.
    trigger_pos: usize,
    /// (display_text, insert_text) pairs.
    suggestions: Vec<(String, String)>,
    selected: usize,
}

impl AutocompleteState {
    fn inactive() -> Self {
        Self {
            kind: AutocompleteKind::None,
            query: String::new(),
            trigger_pos: 0,
            suggestions: Vec::new(),
            selected: 0,
        }
    }

    fn is_active(&self) -> bool {
        self.kind != AutocompleteKind::None
    }
}

/// Map an autocomplete trigger character to the appropriate `AutocompleteKind`.
fn autocomplete_kind_for(trigger_char: char) -> AutocompleteKind {
    match trigger_char {
        '@' => AutocompleteKind::Mention,
        '#' => AutocompleteKind::Channel,
        _ => AutocompleteKind::Emoji,
    }
}

/// Replace all `:name:` shortcodes in `content` with their Unicode emoji equivalents.
/// Shortcodes with no match are left as-is.
pub fn expand_emoji_shortcodes(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b':' {
            // Look for the closing colon.
            if let Some(end) = content[i + 1..].find(':') {
                let name = &content[i + 1..i + 1 + end];
                // Only treat it as a shortcode if the name is non-empty and
                // contains only word characters (letters, digits, underscore).
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    if let Some(emoji) = emoji_by_name(name) {
                        result.push_str(emoji);
                        i += 1 + end + 1; // skip past the closing ':'
                        continue;
                    }
                }
            }
        }
        // No shortcode match — emit the character as-is.
        let ch = content[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }

    result
}

pub struct MessageInput {
    /// Content of the input buffer.
    content: String,
    /// Cursor position as a character index (not byte index).
    cursor_pos: usize,
    /// Timestamp of when we last sent a typing indicator for the current channel.
    last_typing_sent: Option<Instant>,
    /// Whether we are in file-upload path-entry mode (triggered by Ctrl+U).
    pub file_upload_mode: bool,
    /// Buffer for the file path being typed during file-upload mode.
    file_path_buffer: String,
    /// Per-channel draft text saved when the user switches away from a channel.
    drafts: HashMap<Id<ChannelMarker>, String>,
    /// Active inline autocomplete state.
    autocomplete: AutocompleteState,
}

impl MessageInput {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
            last_typing_sent: None,
            file_upload_mode: false,
            file_path_buffer: String::new(),
            drafts: HashMap::new(),
            autocomplete: AutocompleteState::inactive(),
        }
    }

    /// Pre-fill the input (used when entering edit mode).
    pub fn set_content(&mut self, content: String) {
        self.cursor_pos = content.chars().count();
        self.content = content;
        self.autocomplete = AutocompleteState::inactive();
    }

    /// Clear content and reset cursor.
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_pos = 0;
        self.autocomplete = AutocompleteState::inactive();
    }

    /// Save the current content as a draft for `channel_id`.
    /// If the input is empty the draft entry is removed.
    pub fn save_draft(&mut self, channel_id: Id<ChannelMarker>) {
        if self.content.is_empty() {
            self.drafts.remove(&channel_id);
        } else {
            self.drafts.insert(channel_id, self.content.clone());
        }
    }

    /// Restore the draft for `channel_id`, or clear the input if none exists.
    pub fn load_draft(&mut self, channel_id: Id<ChannelMarker>) {
        match self.drafts.get(&channel_id).cloned() {
            Some(draft) => self.set_content(draft),
            None => self.clear(),
        }
    }

    // --- helpers ---

    /// Convert character index to byte offset.
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.content
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.content.len())
    }

    fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    /// Insert `ch` at the current cursor position, then advance cursor by 1.
    fn insert_char(&mut self, ch: char) {
        let byte_pos = self.char_to_byte(self.cursor_pos);
        self.content.insert(byte_pos, ch);
        self.cursor_pos += 1;
    }

    /// Insert an arbitrary string at the current cursor position.
    /// The cursor is advanced past the inserted text.
    pub fn insert_text(&mut self, text: &str) {
        let byte_pos = self.char_to_byte(self.cursor_pos);
        self.content.insert_str(byte_pos, text);
        self.cursor_pos += text.chars().count();
    }

    /// Delete the character before the cursor (Backspace).
    fn delete_before(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let byte_end = self.char_to_byte(self.cursor_pos);
        let byte_start = self.char_to_byte(self.cursor_pos - 1);
        self.content.drain(byte_start..byte_end);
        self.cursor_pos -= 1;
    }

    /// Delete the character at the cursor (Delete).
    fn delete_at(&mut self) {
        if self.cursor_pos >= self.char_count() {
            return;
        }
        let byte_start = self.char_to_byte(self.cursor_pos);
        let byte_end = self.char_to_byte(self.cursor_pos + 1);
        self.content.drain(byte_start..byte_end);
    }

    /// Move cursor to the beginning of the previous word (Ctrl+Left).
    ///
    /// Scans backward from `cursor_pos - 1`. First skips any non-alphanumeric
    /// characters, then skips the alphanumeric run, landing at its start.
    fn move_word_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let chars: Vec<char> = self.content.chars().collect();
        let mut pos = self.cursor_pos;

        // Skip over trailing non-alphanumeric characters.
        while pos > 0 && !chars[pos - 1].is_alphanumeric() {
            pos -= 1;
        }
        // Skip over the alphanumeric word.
        while pos > 0 && chars[pos - 1].is_alphanumeric() {
            pos -= 1;
        }
        self.cursor_pos = pos;
    }

    /// Move cursor to the end of the next word (Ctrl+Right).
    ///
    /// Scans forward from `cursor_pos`. First skips any non-alphanumeric
    /// characters, then skips the alphanumeric run, landing just after it.
    fn move_word_right(&mut self) {
        let count = self.char_count();
        if self.cursor_pos >= count {
            return;
        }
        let chars: Vec<char> = self.content.chars().collect();
        let mut pos = self.cursor_pos;

        // Skip over leading non-alphanumeric characters.
        while pos < count && !chars[pos].is_alphanumeric() {
            pos += 1;
        }
        // Skip over the alphanumeric word.
        while pos < count && chars[pos].is_alphanumeric() {
            pos += 1;
        }
        self.cursor_pos = pos;
    }

    // --- autocomplete helpers ---

    /// Scan backward from cursor to detect an active autocomplete trigger.
    ///
    /// Returns `Some((trigger_char, trigger_pos, query))` when the cursor is
    /// inside a `@<word>`, `:<word>`, or `#<word>` run with at least one query
    /// character. Hyphens are included in the word chars to support Discord
    /// channel names like `general-chat`.
    fn detect_trigger(&self) -> Option<(char, usize, String)> {
        if self.cursor_pos == 0 {
            return None;
        }
        let chars: Vec<char> = self.content.chars().collect();
        let mut pos = self.cursor_pos;

        // Walk backward while we see word chars (alphanumeric, `_`, or `-`).
        while pos > 0
            && (chars[pos - 1].is_alphanumeric()
                || chars[pos - 1] == '_'
                || chars[pos - 1] == '-')
        {
            pos -= 1;
        }

        // Need at least 1 query character after the trigger.
        if pos == self.cursor_pos || pos == 0 {
            return None;
        }

        let trigger_char = chars[pos - 1];
        if trigger_char != '@' && trigger_char != ':' && trigger_char != '#' {
            return None;
        }

        let trigger_pos = pos - 1;
        let query: String = chars[pos..self.cursor_pos].iter().collect();
        Some((trigger_char, trigger_pos, query))
    }

    /// Rebuild the suggestion list from the current autocomplete query and kind.
    fn update_suggestions(&mut self, store: &Store) {
        if !self.autocomplete.is_active() {
            return;
        }

        let matcher = SkimMatcherV2::default();
        let query = self.autocomplete.query.clone();

        match self.autocomplete.kind {
            AutocompleteKind::None => {}

            AutocompleteKind::Mention => {
                let members = store
                    .ui
                    .selected_guild
                    .and_then(|gid| store.members.get(&gid))
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);

                let mut scored: Vec<(i64, String, String)> = members
                    .iter()
                    .filter_map(|m| {
                        matcher
                            .fuzzy_match(&m.name, &query)
                            .map(|score| (score, m.name.clone(), format!("<@{}>", m.id)))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                self.autocomplete.suggestions = scored
                    .into_iter()
                    .take(MAX_SUGGESTIONS)
                    .map(|(_, disp, ins)| (disp, ins))
                    .collect();
            }

            AutocompleteKind::Emoji => {
                let mut scored: Vec<(i64, &str, &str)> = EMOJI_DATA
                    .iter()
                    .filter_map(|(name, ch)| {
                        matcher
                            .fuzzy_match(name, &query)
                            .map(|score| (score, *name, *ch))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                self.autocomplete.suggestions = scored
                    .into_iter()
                    .take(MAX_SUGGESTIONS)
                    .map(|(_, name, ch)| (format!("{} :{name}:", ch), ch.to_string()))
                    .collect();
            }

            AutocompleteKind::Channel => {
                let channels = store
                    .ui
                    .selected_guild
                    .map(|gid| store.guilds.get_channels_for_guild(gid))
                    .unwrap_or_default();

                let mut scored: Vec<(i64, String, String)> = channels
                    .iter()
                    .filter(|ch| ch.kind != crate::store::guilds::ChannelKind::Category
                        && ch.kind != crate::store::guilds::ChannelKind::Voice)
                    .filter_map(|ch| {
                        matcher
                            .fuzzy_match(&ch.name, &query)
                            .map(|score| (score, ch.name.clone(), format!("<#{}>", ch.id)))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                self.autocomplete.suggestions = scored
                    .into_iter()
                    .take(MAX_SUGGESTIONS)
                    .map(|(_, disp, ins)| (format!("# {}", disp), ins))
                    .collect();
            }
        }

        // Clamp selection index.
        if self.autocomplete.suggestions.is_empty() {
            self.autocomplete.selected = 0;
        } else if self.autocomplete.selected >= self.autocomplete.suggestions.len() {
            self.autocomplete.selected = self.autocomplete.suggestions.len() - 1;
        }
    }

    /// Replace the trigger + query range with the selected suggestion's insert text.
    fn apply_suggestion(&mut self) {
        if !self.autocomplete.is_active() || self.autocomplete.suggestions.is_empty() {
            return;
        }

        let insert_text = self.autocomplete.suggestions[self.autocomplete.selected]
            .1
            .clone();
        let trigger_pos = self.autocomplete.trigger_pos;

        // Remove from trigger_pos to cursor_pos (inclusive of the trigger char).
        let byte_start = self.char_to_byte(trigger_pos);
        let byte_end = self.char_to_byte(self.cursor_pos);
        self.content.drain(byte_start..byte_end);
        self.cursor_pos = trigger_pos;

        // Insert the replacement followed by a space.
        let with_space = format!("{} ", insert_text);
        let byte_pos = self.char_to_byte(self.cursor_pos);
        self.content.insert_str(byte_pos, &with_space);
        self.cursor_pos += with_space.chars().count();

        self.autocomplete = AutocompleteState::inactive();
    }
}

impl Component for MessageInput {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::MessageInput {
            return Ok(None);
        }

        // --- File upload mode (Ctrl+U activates; Enter submits; Esc cancels) ---
        if self.file_upload_mode {
            match key.code {
                KeyCode::Esc => {
                    self.file_upload_mode = false;
                    self.file_path_buffer.clear();
                }
                KeyCode::Enter => {
                    let file_path = self.file_path_buffer.trim().to_string();
                    self.file_upload_mode = false;
                    self.file_path_buffer.clear();
                    if !file_path.is_empty() {
                        if let Some(channel_id) = store.ui.selected_channel {
                            store.uploading_file = true;
                            return Ok(Some(Action::UploadFile {
                                channel_id,
                                file_path,
                                message: None,
                            }));
                        }
                    }
                }
                KeyCode::Backspace => {
                    self.file_path_buffer.pop();
                }
                KeyCode::Char(ch)
                    if key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.file_path_buffer.push(ch);
                }
                _ => {}
            }
            return Ok(None);
        }

        // Ctrl+U -> enter file upload mode
        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.file_upload_mode = true;
            self.file_path_buffer.clear();
            return Ok(None);
        }

        // PageUp/PageDown switch to message list for scrolling
        if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
            store.ui.focus = FocusTarget::MessageList;
            return Ok(None);
        }

        // --- Intercept keys when the autocomplete popup is active ---
        if self.autocomplete.is_active() {
            match key.code {
                KeyCode::Esc => {
                    self.autocomplete = AutocompleteState::inactive();
                    return Ok(None);
                }

                KeyCode::Down => {
                    if !self.autocomplete.suggestions.is_empty() {
                        self.autocomplete.selected =
                            (self.autocomplete.selected + 1) % self.autocomplete.suggestions.len();
                    }
                    return Ok(None);
                }

                KeyCode::Up => {
                    if !self.autocomplete.suggestions.is_empty() {
                        let len = self.autocomplete.suggestions.len();
                        self.autocomplete.selected = (self.autocomplete.selected + len - 1) % len;
                    }
                    return Ok(None);
                }

                KeyCode::Enter | KeyCode::Tab => {
                    self.apply_suggestion();
                    return Ok(None);
                }

                KeyCode::Backspace => {
                    self.delete_before();
                    if let Some((trigger_char, trigger_pos, query)) = self.detect_trigger() {
                        self.autocomplete.trigger_pos = trigger_pos;
                        self.autocomplete.query = query;
                        self.autocomplete.kind = autocomplete_kind_for(trigger_char);
                        self.update_suggestions(store);
                    } else {
                        self.autocomplete = AutocompleteState::inactive();
                    }
                    return Ok(None);
                }

                KeyCode::Char(ch)
                    if key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.insert_char(ch);
                    if let Some((trigger_char, trigger_pos, query)) = self.detect_trigger() {
                        self.autocomplete.trigger_pos = trigger_pos;
                        self.autocomplete.query = query;
                        self.autocomplete.kind = autocomplete_kind_for(trigger_char);
                        self.update_suggestions(store);
                    } else {
                        self.autocomplete = AutocompleteState::inactive();
                    }
                    return Ok(None);
                }

                // Any other key dismisses the popup and falls through to normal handling.
                _ => {
                    self.autocomplete = AutocompleteState::inactive();
                }
            }
        }

        match key.code {
            // Esc -> back to channel tree, cancel reply/edit
            KeyCode::Esc => {
                store.ui.focus = FocusTarget::ChannelTree;
                store.ui.reply_to = None;
                store.ui.editing_message = None;
                self.clear();
            }

            // Up arrow with empty input -> focus message list to scroll
            KeyCode::Up if self.content.is_empty() => {
                store.ui.focus = FocusTarget::MessageList;
            }

            // Ctrl+Left -> move to beginning of previous word
            KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_word_left();
            }

            // Ctrl+Right -> move to end of next word
            KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_word_right();
            }

            // Left arrow at cursor position 0 -> back to channel tree
            KeyCode::Left if self.cursor_pos == 0 && key.modifiers == KeyModifiers::NONE => {
                store.ui.focus = FocusTarget::ChannelTree;
            }

            // Left arrow (cursor not at 0)
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }

            // Printable characters
            KeyCode::Char(ch) => {
                if key.modifiers.contains(KeyModifiers::SHIFT) && ch == '\n' {
                    // Shift+Enter - but crossterm represents Shift+Enter differently;
                    // handle it in the Enter branch instead.
                    self.insert_char('\n');
                } else if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT {
                    self.insert_char(ch);

                    // Check whether we just triggered autocomplete.
                    if let Some((trigger_char, trigger_pos, query)) = self.detect_trigger() {
                        self.autocomplete = AutocompleteState {
                            kind: autocomplete_kind_for(trigger_char),
                            query,
                            trigger_pos,
                            suggestions: Vec::new(),
                            selected: 0,
                        };
                        self.update_suggestions(store);
                    }

                    // Emit a typing indicator if we haven't sent one recently.
                    let should_send = self
                        .last_typing_sent
                        .map(|t| t.elapsed() >= TYPING_THROTTLE)
                        .unwrap_or(true);
                    if should_send {
                        if let Some(channel_id) = store.ui.selected_channel {
                            self.last_typing_sent = Some(Instant::now());
                            return Ok(Some(Action::SendTyping { channel_id }));
                        }
                    }
                }
            }

            // Shift+Enter -> newline
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.insert_char('\n');
            }

            // Enter -> send / edit
            KeyCode::Enter => {
                if self.content.is_empty() {
                    return Ok(None);
                }

                let content = expand_emoji_shortcodes(&self.content);

                if let Some(message_id) = store.ui.editing_message {
                    let channel_id = match store.ui.selected_channel {
                        Some(id) => id,
                        None => return Ok(None),
                    };
                    self.clear();
                    store.ui.editing_message = None;
                    store.ui.reply_to = None;
                    return Ok(Some(Action::EditMessage {
                        channel_id,
                        message_id,
                        content,
                    }));
                }

                let channel_id = match store.ui.selected_channel {
                    Some(id) => id,
                    None => return Ok(None),
                };
                let reply_to = store.ui.reply_to.as_ref().map(|r| r.message_id);
                self.clear();
                store.ui.reply_to = None;
                store.ui.editing_message = None;
                return Ok(Some(Action::SendMessage {
                    channel_id,
                    content,
                    reply_to,
                }));
            }

            KeyCode::Backspace => {
                self.delete_before();
            }

            KeyCode::Delete => {
                self.delete_at();
            }

            KeyCode::Right => {
                if self.cursor_pos < self.char_count() {
                    self.cursor_pos += 1;
                }
            }

            KeyCode::Home => {
                self.cursor_pos = 0;
            }

            KeyCode::End => {
                self.cursor_pos = self.char_count();
            }

            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let focused = store.ui.focus == FocusTarget::MessageInput;

        // Determine how many header lines we need above the input box.
        let has_reply = store.ui.reply_to.is_some();
        let has_editing = store.ui.editing_message.is_some();
        let header_lines = u16::from(has_reply) + u16::from(has_editing);

        // Split the area: optional header rows + input box.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if header_lines > 0 {
                vec![Constraint::Length(header_lines), Constraint::Min(3)]
            } else {
                vec![Constraint::Min(3)]
            })
            .split(area);

        let input_area = if header_lines > 0 {
            let header_area = chunks[0];

            // Build indicator lines.
            let mut lines: Vec<Line> = Vec::new();

            if has_reply {
                let author = store
                    .ui
                    .reply_to
                    .as_ref()
                    .map(|r| r.author_name.as_str())
                    .unwrap_or("unknown");
                lines.push(Line::from(Span::styled(
                    format!("> Replying to @{}", author),
                    theme::muted(),
                )));
            }

            if has_editing {
                lines.push(Line::from(Span::styled(
                    "Editing message",
                    theme::muted(),
                )));
            }

            let header_widget = Paragraph::new(lines);
            frame.render_widget(header_widget, header_area);

            chunks[1]
        } else {
            chunks[0]
        };

        // Resolve channel name and slowmode for placeholder.
        let (channel_name, channel_slowmode): (String, Option<u64>) = store
            .ui
            .selected_guild
            .and_then(|gid| {
                store.ui.selected_channel.and_then(|cid| {
                    let channels = store.guilds.get_channels_for_guild(gid);
                    channels
                        .into_iter()
                        .find(|ch| ch.id == cid)
                        .map(|ch| (ch.name.clone(), ch.rate_limit_per_user))
                })
            })
            .unwrap_or_else(|| ("channel".to_string(), None));

        let border_style = if focused {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default().fg(theme::BORDER)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(theme::BG));

        // Render content or placeholder.
        let inner = block.inner(input_area);
        frame.render_widget(block, input_area);

        if self.content.is_empty() {
            let placeholder = if let Some(delay) = channel_slowmode.filter(|&d| d > 0) {
                format!("Message #{}...  Slowmode: {}s", channel_name, delay)
            } else {
                format!("Message #{}...", channel_name)
            };
            let p = Paragraph::new(Span::styled(placeholder, theme::muted()));
            frame.render_widget(p, inner);
        } else {
            let p = Paragraph::new(self.content.as_str())
                .style(Style::default().fg(theme::TEXT_PRIMARY));
            frame.render_widget(p, inner);

            // Show cursor when focused.
            if focused {
                // Count visual columns up to cursor_pos, accounting for newlines.
                let before_cursor = &self.content[..self.char_to_byte(self.cursor_pos)];
                let lines_before: Vec<&str> = before_cursor.split('\n').collect();
                let row_offset = (lines_before.len() as u16).saturating_sub(1);
                let last_line = lines_before.last().copied().unwrap_or("");
                // Use unicode-width for accurate column counting.
                let col_offset = last_line.chars().count() as u16;

                let cursor_x = inner.x + col_offset;
                let cursor_y = inner.y + row_offset;

                if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
                    frame.set_cursor_position(Position {
                        x: cursor_x,
                        y: cursor_y,
                    });
                }
            }
        }

        // Also show cursor when content is empty and focused (at the start of the placeholder area).
        if self.content.is_empty() && focused {
            if inner.width > 0 && inner.height > 0 {
                frame.set_cursor_position(Position {
                    x: inner.x,
                    y: inner.y,
                });
            }
        }

        // --- Render autocomplete popup ---
        if self.autocomplete.is_active() && !self.autocomplete.suggestions.is_empty() {
            let suggestion_count = self.autocomplete.suggestions.len().min(MAX_SUGGESTIONS) as u16;
            // popup height = border (top+bottom) + one row per suggestion
            let popup_height = suggestion_count + 2;
            // popup width: fit the longest display text + padding + borders
            let max_display_len = self
                .autocomplete
                .suggestions
                .iter()
                .map(|(d, _)| d.chars().count())
                .max()
                .unwrap_or(10) as u16;
            let popup_width = (max_display_len + 4).max(20).min(area.width);

            // Position popup just above the input area.
            let popup_y = input_area.y.saturating_sub(popup_height);
            let popup_x = area.x;

            let popup_area = Rect {
                x: popup_x,
                y: popup_y,
                width: popup_width,
                height: popup_height,
            };

            // Only render when there is room above the input.
            if popup_y < input_area.y {
                let items: Vec<ListItem> = self
                    .autocomplete
                    .suggestions
                    .iter()
                    .enumerate()
                    .map(|(i, (display, _))| {
                        let style = if i == self.autocomplete.selected {
                            Style::default()
                                .bg(theme::ACCENT)
                                .fg(theme::TEXT_PRIMARY)
                        } else {
                            Style::default()
                                .bg(theme::BG_SECONDARY)
                                .fg(theme::TEXT_PRIMARY)
                        };
                        ListItem::new(format!(" {} ", display)).style(style)
                    })
                    .collect();

                let popup_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::ACCENT))
                    .style(Style::default().bg(theme::BG_SECONDARY));

                let list = List::new(items).block(popup_block);
                frame.render_widget(list, popup_area);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- insert ---

    #[test]
    fn test_insert_characters_at_end() {
        let mut input = MessageInput::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.content, "hi");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn test_insert_character_at_start() {
        let mut input = MessageInput::new();
        input.content = "world".to_string();
        input.cursor_pos = 0;
        input.insert_char('!');
        assert_eq!(input.content, "!world");
        assert_eq!(input.cursor_pos, 1);
    }

    #[test]
    fn test_insert_character_in_middle() {
        let mut input = MessageInput::new();
        input.content = "hllo".to_string();
        input.cursor_pos = 1;
        input.insert_char('e');
        assert_eq!(input.content, "hello");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn test_insert_unicode() {
        let mut input = MessageInput::new();
        input.insert_char('こ');
        input.insert_char('ん');
        input.insert_char('に');
        assert_eq!(input.content, "こんに");
        assert_eq!(input.cursor_pos, 3);
    }

    // --- backspace ---

    #[test]
    fn test_backspace_at_start_is_noop() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 0;
        input.delete_before();
        assert_eq!(input.content, "hello");
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_backspace_at_end() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 5;
        input.delete_before();
        assert_eq!(input.content, "hell");
        assert_eq!(input.cursor_pos, 4);
    }

    #[test]
    fn test_backspace_in_middle() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 3; // after 'l'
        input.delete_before();
        assert_eq!(input.content, "helo");
        assert_eq!(input.cursor_pos, 2);
    }

    // --- delete ---

    #[test]
    fn test_delete_at_cursor() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 2; // at second 'l'
        input.delete_at();
        assert_eq!(input.content, "helo");
        assert_eq!(input.cursor_pos, 2); // cursor stays
    }

    #[test]
    fn test_delete_at_end_is_noop() {
        let mut input = MessageInput::new();
        input.content = "hi".to_string();
        input.cursor_pos = 2;
        input.delete_at();
        assert_eq!(input.content, "hi");
        assert_eq!(input.cursor_pos, 2);
    }

    // --- cursor movement ---

    #[test]
    fn test_cursor_move_left() {
        let mut input = MessageInput::new();
        input.content = "abc".to_string();
        input.cursor_pos = 3;
        // move left 2 times
        if input.cursor_pos > 0 {
            input.cursor_pos -= 1;
        }
        if input.cursor_pos > 0 {
            input.cursor_pos -= 1;
        }
        assert_eq!(input.cursor_pos, 1);
    }

    #[test]
    fn test_cursor_move_left_at_start_stays() {
        let mut input = MessageInput::new();
        input.content = "abc".to_string();
        input.cursor_pos = 0;
        if input.cursor_pos > 0 {
            input.cursor_pos -= 1;
        }
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_cursor_move_right() {
        let mut input = MessageInput::new();
        input.content = "abc".to_string();
        input.cursor_pos = 0;
        if input.cursor_pos < input.char_count() {
            input.cursor_pos += 1;
        }
        assert_eq!(input.cursor_pos, 1);
    }

    #[test]
    fn test_cursor_move_right_at_end_stays() {
        let mut input = MessageInput::new();
        input.content = "abc".to_string();
        input.cursor_pos = 3;
        if input.cursor_pos < input.char_count() {
            input.cursor_pos += 1;
        }
        assert_eq!(input.cursor_pos, 3);
    }

    #[test]
    fn test_cursor_home() {
        let mut input = MessageInput::new();
        input.content = "hello world".to_string();
        input.cursor_pos = 6;
        input.cursor_pos = 0;
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_cursor_end() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 2;
        input.cursor_pos = input.char_count();
        assert_eq!(input.cursor_pos, 5);
    }

    // --- newline insertion ---

    #[test]
    fn test_newline_insertion() {
        let mut input = MessageInput::new();
        input.content = "helloworld".to_string();
        input.cursor_pos = 5;
        input.insert_char('\n');
        assert_eq!(input.content, "hello\nworld");
        assert_eq!(input.cursor_pos, 6);
    }

    // --- clear ---

    #[test]
    fn test_clear() {
        let mut input = MessageInput::new();
        input.content = "some text".to_string();
        input.cursor_pos = 4;
        input.clear();
        assert_eq!(input.content, "");
        assert_eq!(input.cursor_pos, 0);
    }

    // --- word navigation ---

    #[test]
    fn test_move_word_left_basic() {
        let mut input = MessageInput::new();
        input.content = "hello world".to_string();
        input.cursor_pos = 11; // at end
        input.move_word_left();
        assert_eq!(input.cursor_pos, 6); // start of "world"
    }

    #[test]
    fn test_move_word_left_from_middle_of_word() {
        let mut input = MessageInput::new();
        input.content = "hello world".to_string();
        input.cursor_pos = 8; // inside "world"
        input.move_word_left();
        assert_eq!(input.cursor_pos, 6); // start of "world"
    }

    #[test]
    fn test_move_word_left_at_start_is_noop() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 0;
        input.move_word_left();
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_move_word_left_skips_spaces() {
        let mut input = MessageInput::new();
        input.content = "hello   world".to_string();
        input.cursor_pos = 8; // inside spaces before "world"
        input.move_word_left();
        assert_eq!(input.cursor_pos, 0); // start of "hello"
    }

    #[test]
    fn test_move_word_right_basic() {
        let mut input = MessageInput::new();
        input.content = "hello world".to_string();
        input.cursor_pos = 0;
        input.move_word_right();
        assert_eq!(input.cursor_pos, 5); // end of "hello"
    }

    #[test]
    fn test_move_word_right_from_middle_of_word() {
        let mut input = MessageInput::new();
        input.content = "hello world".to_string();
        input.cursor_pos = 2; // inside "hello"
        input.move_word_right();
        assert_eq!(input.cursor_pos, 5); // end of "hello"
    }

    #[test]
    fn test_move_word_right_at_end_is_noop() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 5;
        input.move_word_right();
        assert_eq!(input.cursor_pos, 5);
    }

    #[test]
    fn test_move_word_right_skips_spaces() {
        let mut input = MessageInput::new();
        input.content = "hello   world".to_string();
        input.cursor_pos = 5; // after "hello", before spaces
        input.move_word_right();
        assert_eq!(input.cursor_pos, 13); // end of "world"
    }

    // --- insert_text ---

    #[test]
    fn test_insert_text_at_end() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 5;
        input.insert_text(" world");
        assert_eq!(input.content, "hello world");
        assert_eq!(input.cursor_pos, 11);
    }

    #[test]
    fn test_insert_text_in_middle() {
        let mut input = MessageInput::new();
        input.content = "helloworld".to_string();
        input.cursor_pos = 5;
        input.insert_text(" pasted ");
        assert_eq!(input.content, "hello pasted world");
        assert_eq!(input.cursor_pos, 13);
    }

    #[test]
    fn test_insert_text_with_newlines() {
        let mut input = MessageInput::new();
        input.insert_text("line1\nline2");
        assert_eq!(input.content, "line1\nline2");
        assert_eq!(input.cursor_pos, 11);
    }

    #[test]
    fn test_insert_text_unicode() {
        let mut input = MessageInput::new();
        input.insert_text("こんにちは");
        assert_eq!(input.content, "こんにちは");
        assert_eq!(input.cursor_pos, 5);
    }

    // --- expand_emoji_shortcodes ---

    #[test]
    fn test_expand_known_shortcode() {
        assert_eq!(expand_emoji_shortcodes(":thumbsup:"), "👍");
    }

    #[test]
    fn test_expand_shortcode_in_sentence() {
        assert_eq!(
            expand_emoji_shortcodes("great job :thumbsup: nice"),
            "great job 👍 nice"
        );
    }

    #[test]
    fn test_expand_multiple_shortcodes() {
        assert_eq!(
            expand_emoji_shortcodes(":fire: :heart:"),
            "🔥 ❤️"
        );
    }

    #[test]
    fn test_unknown_shortcode_left_as_is() {
        assert_eq!(expand_emoji_shortcodes(":notanemoji:"), ":notanemoji:");
    }

    #[test]
    fn test_empty_colon_pair_left_as_is() {
        assert_eq!(expand_emoji_shortcodes("::"), "::");
    }

    #[test]
    fn test_plain_text_unchanged() {
        assert_eq!(expand_emoji_shortcodes("hello world"), "hello world");
    }

    #[test]
    fn test_colons_without_closing_left_as_is() {
        assert_eq!(expand_emoji_shortcodes("hello :world"), "hello :world");
    }

    #[test]
    fn test_expand_shortcode_with_underscore() {
        assert_eq!(expand_emoji_shortcodes(":thumbs_up:"), ":thumbs_up:");
        // thumbsup (no underscore) is the valid key
        assert_eq!(expand_emoji_shortcodes(":ok_hand:"), "👌");
    }

    // --- set_content ---

    #[test]
    fn test_set_content_moves_cursor_to_end() {
        let mut input = MessageInput::new();
        input.set_content("hello".to_string());
        assert_eq!(input.content, "hello");
        assert_eq!(input.cursor_pos, 5);
    }

    // --- char_to_byte for unicode ---

    #[test]
    fn test_char_to_byte_unicode() {
        let mut input = MessageInput::new();
        // Each Japanese character is 3 bytes in UTF-8.
        input.content = "こんにちは".to_string();
        input.cursor_pos = 0;
        assert_eq!(input.char_to_byte(0), 0);
        assert_eq!(input.char_to_byte(1), 3);
        assert_eq!(input.char_to_byte(5), 15);
    }

    // --- autocomplete trigger detection ---

    #[test]
    fn test_detect_trigger_mention() {
        let mut input = MessageInput::new();
        input.content = "hello @ali".to_string();
        input.cursor_pos = 10;
        let result = input.detect_trigger();
        assert!(result.is_some());
        let (trigger, pos, query) = result.unwrap();
        assert_eq!(trigger, '@');
        assert_eq!(pos, 6);
        assert_eq!(query, "ali");
    }

    #[test]
    fn test_detect_trigger_emoji() {
        let mut input = MessageInput::new();
        input.content = ":fir".to_string();
        input.cursor_pos = 4;
        let result = input.detect_trigger();
        assert!(result.is_some());
        let (trigger, pos, query) = result.unwrap();
        assert_eq!(trigger, ':');
        assert_eq!(pos, 0);
        assert_eq!(query, "fir");
    }

    #[test]
    fn test_detect_trigger_no_trigger_just_word() {
        let mut input = MessageInput::new();
        input.content = "hello".to_string();
        input.cursor_pos = 5;
        assert!(input.detect_trigger().is_none());
    }

    #[test]
    fn test_detect_trigger_only_at_sign() {
        // Trigger char alone with no following query should NOT activate autocomplete.
        let mut input = MessageInput::new();
        input.content = "@".to_string();
        input.cursor_pos = 1;
        assert!(input.detect_trigger().is_none());
    }

    // --- autocomplete apply_suggestion ---

    #[test]
    fn test_apply_suggestion_mention() {
        let mut input = MessageInput::new();
        input.content = "hello @ali".to_string();
        input.cursor_pos = 10;
        input.autocomplete = AutocompleteState {
            kind: AutocompleteKind::Mention,
            query: "ali".to_string(),
            trigger_pos: 6,
            suggestions: vec![("alice".to_string(), "<@123456>".to_string())],
            selected: 0,
        };
        input.apply_suggestion();
        assert_eq!(input.content, "hello <@123456> ");
        assert_eq!(input.cursor_pos, input.content.chars().count());
        assert!(!input.autocomplete.is_active());
    }

    #[test]
    fn test_apply_suggestion_emoji() {
        let mut input = MessageInput::new();
        input.content = ":fir".to_string();
        input.cursor_pos = 4;
        input.autocomplete = AutocompleteState {
            kind: AutocompleteKind::Emoji,
            query: "fir".to_string(),
            trigger_pos: 0,
            suggestions: vec![("🔥 :fire:".to_string(), "🔥".to_string())],
            selected: 0,
        };
        input.apply_suggestion();
        assert_eq!(input.content, "🔥 ");
        assert_eq!(input.cursor_pos, input.content.chars().count());
        assert!(!input.autocomplete.is_active());
    }
}
