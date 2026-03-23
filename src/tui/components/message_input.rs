use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct MessageInput {
    /// Content of the input buffer.
    content: String,
    /// Cursor position as a character index (not byte index).
    cursor_pos: usize,
}

impl MessageInput {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
        }
    }

    /// Pre-fill the input (used when entering edit mode).
    pub fn set_content(&mut self, content: String) {
        self.cursor_pos = content.chars().count();
        self.content = content;
    }

    /// Clear content and reset cursor.
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_pos = 0;
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
}

impl Component for MessageInput {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::MessageInput {
            return Ok(None);
        }

        match key.code {
            // Printable characters
            KeyCode::Char(ch) => {
                if key.modifiers.contains(KeyModifiers::SHIFT) && ch == '\n' {
                    // Shift+Enter - but crossterm represents Shift+Enter differently;
                    // handle it in the Enter branch instead.
                    self.insert_char('\n');
                } else {
                    self.insert_char(ch);
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

                let content = self.content.clone();

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

            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
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

        // Resolve channel name for placeholder.
        let channel_name: String = store
            .ui
            .selected_guild
            .and_then(|gid| {
                store.ui.selected_channel.and_then(|cid| {
                    let channels = store.guilds.get_channels_for_guild(gid);
                    channels
                        .into_iter()
                        .find(|ch| ch.id == cid)
                        .map(|ch| ch.name.clone())
                })
            })
            .unwrap_or_else(|| "channel".to_string());

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
            let placeholder = format!("Message #{}...", channel_name);
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
}
