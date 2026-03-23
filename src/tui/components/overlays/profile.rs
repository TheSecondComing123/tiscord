use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use twilight_model::id::marker::UserMarker;
use twilight_model::id::Id;

use crate::discord::actions::Action;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileMode {
    /// Small 4-line popup.
    Minimal,
    /// Larger centered overlay with all fields.
    Full,
}

pub struct ProfileOverlay {
    pub visible: bool,
    pub mode: ProfileMode,
    pub user_id: Option<Id<UserMarker>>,
}

impl ProfileOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            mode: ProfileMode::Minimal,
            user_id: None,
        }
    }

    pub fn open(&mut self, user_id: Id<UserMarker>) {
        self.visible = true;
        self.mode = ProfileMode::Minimal;
        self.user_id = Some(user_id);
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.user_id = None;
        self.mode = ProfileMode::Minimal;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Component for ProfileOverlay {
    fn handle_key_event(&mut self, key: KeyEvent, _store: &mut Store) -> Result<Option<Action>> {
        if !self.visible {
            return Ok(None);
        }

        match key.code {
            KeyCode::Enter => {
                // Expand Minimal -> Full
                if self.mode == ProfileMode::Minimal {
                    self.mode = ProfileMode::Full;
                }
            }
            KeyCode::Esc => {
                match self.mode {
                    ProfileMode::Full => {
                        // Shrink Full -> close
                        self.close();
                    }
                    ProfileMode::Minimal => {
                        self.close();
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        if !self.visible {
            return;
        }

        let user_id = match self.user_id {
            Some(id) => id,
            None => return,
        };

        let profile = store.profiles.get(user_id);

        match self.mode {
            ProfileMode::Minimal => self.render_minimal(frame, area, profile),
            ProfileMode::Full => self.render_full(frame, area, store, profile),
        }
    }
}

impl ProfileOverlay {
    fn render_minimal(
        &self,
        frame: &mut Frame,
        area: Rect,
        profile: Option<&crate::store::profiles::UserProfile>,
    ) {
        // Small popup: 36 wide, 6 tall, anchored near the center-bottom
        let panel_width = 36u16.min(area.width);
        let panel_height = 6u16.min(area.height);
        let panel_x = area.x + (area.width.saturating_sub(panel_width)) / 2;
        let panel_y = area.y + (area.height.saturating_sub(panel_height)) * 3 / 4;

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
            .title(Span::styled(" Profile ", Style::default().fg(theme::ACCENT)));
        let inner = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        let lines = match profile {
            None => vec![
                Line::from(Span::styled("Loading...", theme::muted())),
                Line::from(Span::styled(
                    "Press Enter to expand",
                    theme::muted(),
                )),
            ],
            Some(p) => {
                let mut lines = Vec::new();
                // Username (bold)
                lines.push(Line::from(Span::styled(
                    p.username.clone(),
                    Style::default()
                        .fg(theme::TEXT_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                )));
                // Display name (if different from username)
                if let Some(ref dn) = p.display_name {
                    if dn != &p.username {
                        lines.push(Line::from(Span::styled(
                            dn.clone(),
                            Style::default().fg(theme::TEXT_SECONDARY),
                        )));
                    }
                }
                // Bot indicator
                if p.bot {
                    lines.push(Line::from(Span::styled(
                        "[BOT]",
                        Style::default().fg(theme::ACCENT),
                    )));
                }
                // Hint
                lines.push(Line::from(Span::styled(
                    "Enter: expand  Esc: close",
                    theme::muted(),
                )));
                lines
            }
        };

        let text = Text::from(lines);
        frame.render_widget(Paragraph::new(text), inner);
    }

    fn render_full(
        &self,
        frame: &mut Frame,
        area: Rect,
        store: &Store,
        profile: Option<&crate::store::profiles::UserProfile>,
    ) {
        // Larger centered overlay: 50% width, 50% height
        let panel_width = (area.width / 2).max(40).min(area.width);
        let panel_height = (area.height / 2).max(12).min(area.height);
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
            .title(Span::styled(" User Profile ", Style::default().fg(theme::ACCENT)));
        let inner = outer_block.inner(panel_area);
        frame.render_widget(outer_block, panel_area);

        let mut lines: Vec<Line> = Vec::new();

        match profile {
            None => {
                lines.push(Line::from(Span::styled("Loading profile...", theme::muted())));
            }
            Some(p) => {
                // Username (bold)
                lines.push(Line::from(vec![
                    Span::styled("Username:  ", theme::muted()),
                    Span::styled(
                        p.username.clone(),
                        Style::default()
                            .fg(theme::TEXT_PRIMARY)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                // Display name
                if let Some(ref dn) = p.display_name {
                    lines.push(Line::from(vec![
                        Span::styled("Display:   ", theme::muted()),
                        Span::styled(dn.clone(), Style::default().fg(theme::TEXT_SECONDARY)),
                    ]));
                }

                // Bot indicator
                if p.bot {
                    lines.push(Line::from(vec![
                        Span::styled("Type:      ", theme::muted()),
                        Span::styled("[BOT]", Style::default().fg(theme::ACCENT)),
                    ]));
                }

                // Member info from guild cache (nickname, roles)
                if let Some(guild_id) = store.ui.selected_guild {
                    if let Some(members) = store.members.get(&guild_id) {
                        if let Some(member) = members.iter().find(|m| m.id == p.user_id) {
                            lines.push(Line::from(""));
                            // Nickname (member name may differ from username)
                            if member.name != p.username {
                                lines.push(Line::from(vec![
                                    Span::styled("Nickname:  ", theme::muted()),
                                    Span::styled(
                                        member.name.clone(),
                                        Style::default().fg(theme::TEXT_SECONDARY),
                                    ),
                                ]));
                            }
                            // Custom status
                            if let Some(ref cs) = member.custom_status {
                                let status_text = match (&cs.emoji, &cs.text) {
                                    (Some(e), Some(t)) => format!("{} {}", e, t),
                                    (Some(e), None) => e.clone(),
                                    (None, Some(t)) => t.clone(),
                                    (None, None) => String::new(),
                                };
                                if !status_text.is_empty() {
                                    lines.push(Line::from(vec![
                                        Span::styled("Status:    ", theme::muted()),
                                        Span::styled(
                                            status_text,
                                            Style::default().fg(theme::TEXT_SECONDARY),
                                        ),
                                    ]));
                                }
                            }
                        }
                    }
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Esc: close", theme::muted())));

        let text = Text::from(lines);
        frame.render_widget(Paragraph::new(text), inner);
    }
}
