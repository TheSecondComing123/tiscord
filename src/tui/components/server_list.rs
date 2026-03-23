use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct ServerList {
    selected_index: usize,
}

impl ServerList {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }
}

impl Component for ServerList {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::ServerList {
            return Ok(None);
        }

        // Total entries: 1 (DMs) + number of guilds
        let total = 1 + store.guilds.guilds.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_index + 1 < total {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Enter => {
                if self.selected_index == 0 {
                    store.ui.dm_mode = true;
                    store.ui.selected_guild = None;
                    store.ui.selected_channel = None;
                } else {
                    let guild_idx = self.selected_index - 1;
                    if let Some(guild) = store.guilds.guilds.get(guild_idx) {
                        let guild_id = guild.id;
                        store.ui.selected_guild = Some(guild_id);
                        store.ui.selected_channel = None;
                        store.ui.dm_mode = false;
                        return Ok(Some(Action::FetchGuildMembers { guild_id }));
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let block = Block::default()
            .title("Servers")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));

        // Build items: DMs entry first, then guilds
        let mut items: Vec<ListItem> = Vec::new();

        // DMs entry
        {
            let is_selected = self.selected_index == 0;
            let label = "Direct Messages";
            let style = if is_selected {
                theme::selected()
            } else {
                theme::base()
            };
            items.push(ListItem::new(Line::from(Span::styled(label, style))));
        }

        // Guild entries
        for (i, guild) in store.guilds.guilds.iter().enumerate() {
            let list_index = i + 1;
            let is_selected = self.selected_index == list_index;

            // Check if guild has any unread or mention
            let has_mention = guild
                .channels
                .iter()
                .any(|ch| store.notifications.has_mentions(ch.id));
            let has_unread = guild
                .channels
                .iter()
                .any(|ch| store.notifications.has_unreads(ch.id));

            let name_style = if is_selected {
                theme::selected()
            } else if has_unread || has_mention {
                theme::base().add_modifier(Modifier::BOLD)
            } else {
                theme::base()
            };

            let mut spans = vec![Span::styled(&guild.name, name_style)];

            // Mention badge
            if has_mention {
                let mention_count: u32 = guild
                    .channels
                    .iter()
                    .filter_map(|ch| store.notifications.get(ch.id))
                    .map(|n| n.mention_count)
                    .sum();
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("({})", mention_count),
                    Style::default().fg(theme::MENTION).add_modifier(Modifier::BOLD),
                ));
            }

            items.push(ListItem::new(Line::from(spans)));
        }

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}
