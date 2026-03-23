use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::messages::StoredMessage;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct PinsOverlay {
    pub visible: bool,
    selected_index: usize,
}

impl PinsOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_index: 0,
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.selected_index = 0;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn move_up(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = count - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    fn move_down(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.selected_index = (self.selected_index + 1) % count;
    }
}

impl Component for PinsOverlay {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if !self.visible {
            return Ok(None);
        }

        let pin_count = store
            .ui
            .selected_channel
            .and_then(|ch| store.pinned_messages.get(&ch))
            .and_then(|opt| opt.as_ref())
            .map(|v| v.len())
            .unwrap_or(0);

        match key.code {
            KeyCode::Esc => {
                self.close();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up(pin_count);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down(pin_count);
            }
            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        if !self.visible {
            return;
        }

        // Floating panel: 60% width, 70% height, centered.
        let panel_width = (area.width * 3 / 5).max(50).min(area.width);
        let panel_height = (area.height * 7 / 10).max(10).min(area.height);
        let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
        let panel_y = area.y + (area.height.saturating_sub(panel_height)) / 2;

        let panel_area = Rect {
            x: panel_x,
            y: panel_y,
            width: panel_width,
            height: panel_height,
        };

        frame.render_widget(Clear, panel_area);

        let pins_opt: Option<&Vec<StoredMessage>> = store
            .ui
            .selected_channel
            .and_then(|ch| store.pinned_messages.get(&ch))
            .and_then(|opt| opt.as_ref());

        let title = match pins_opt {
            Some(pins) => format!(" Pinned Messages ({}) ", pins.len()),
            None => " Pinned Messages (loading...) ".to_string(),
        };

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::ACCENT))
            .style(Style::default().bg(theme::BG_SECONDARY))
            .title(Span::styled(title, Style::default().fg(theme::ACCENT)));
        let inner_area = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        match pins_opt {
            None => {
                let loading = Paragraph::new("Loading pinned messages...")
                    .style(theme::muted())
                    .alignment(Alignment::Center);
                frame.render_widget(loading, inner_area);
            }
            Some(pins) if pins.is_empty() => {
                let empty = Paragraph::new("No pinned messages in this channel.")
                    .style(theme::muted())
                    .alignment(Alignment::Center);
                frame.render_widget(empty, inner_area);
            }
            Some(pins) => {
                let visible_count = inner_area.height as usize;
                let scroll_offset = if self.selected_index < visible_count {
                    0
                } else {
                    self.selected_index - visible_count + 1
                };

                let items: Vec<ListItem> = pins
                    .iter()
                    .enumerate()
                    .skip(scroll_offset)
                    .take(visible_count)
                    .map(|(i, msg)| {
                        let is_selected = i == self.selected_index;

                        let header_style = if is_selected {
                            Style::default().fg(theme::ACCENT).bg(theme::BG_TERTIARY)
                        } else {
                            Style::default().fg(theme::ACCENT)
                        };
                        let content_style = if is_selected {
                            Style::default().fg(theme::TEXT_PRIMARY).bg(theme::BG_TERTIARY)
                        } else {
                            Style::default().fg(theme::TEXT_SECONDARY)
                        };

                        // Truncate timestamp to date+time without microseconds
                        let ts = msg.timestamp.split('.').next().unwrap_or(&msg.timestamp).replace('T', " ");

                        let preview: String = msg.content.chars().take(panel_width as usize - 4).collect();
                        let preview = if msg.content.len() > panel_width as usize - 4 {
                            format!("{preview}...")
                        } else {
                            preview
                        };

                        let header = Line::from(vec![
                            Span::styled("  ", header_style),
                            Span::styled(msg.author_name.clone(), header_style.add_modifier(Modifier::BOLD)),
                            Span::styled(format!("  {ts}"), Style::default().fg(theme::TEXT_MUTED).bg(if is_selected { theme::BG_TERTIARY } else { theme::BG_SECONDARY })),
                        ]);
                        let content_line = Line::from(Span::styled(format!("  {preview}"), content_style));

                        ListItem::new(vec![header, content_line])
                    })
                    .collect();

                let list = List::new(items).style(Style::default().bg(theme::BG_SECONDARY));
                frame.render_widget(list, inner_area);
            }
        }
    }
}
