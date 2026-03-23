use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::store::state::FocusTarget;
use crate::tui::component::Component;
use crate::tui::theme;

/// An entry in the flat server list. Folder headers are non-selectable.
#[derive(Debug, Clone)]
enum ListEntry {
    Dms,
    FolderHeader { name: String, color: Option<u32> },
    Guild { guild_index: usize },
}

pub struct ServerList {
    selected_index: usize,
}

impl ServerList {
    pub fn new() -> Self {
        Self { selected_index: 0 }
    }

    /// Build the flat list of entries from the store.
    fn build_entries(store: &Store) -> Vec<ListEntry> {
        let mut entries = vec![ListEntry::Dms];

        if store.guild_folders.is_empty() {
            // No folder data — just list all guilds flat
            for i in 0..store.guilds.guilds.len() {
                entries.push(ListEntry::Guild { guild_index: i });
            }
        } else {
            // Track which guilds have been placed in a folder
            let mut placed = vec![false; store.guilds.guilds.len()];

            for folder in &store.guild_folders {
                // Only show folder header if it has a name and multiple guilds
                let show_header = folder.name.is_some() && folder.guild_ids.len() > 1;
                if show_header {
                    entries.push(ListEntry::FolderHeader {
                        name: folder.name.clone().unwrap_or_default(),
                        color: folder.color,
                    });
                }
                for folder_guild_id in &folder.guild_ids {
                    if let Some(idx) = store.guilds.guilds.iter().position(|g| g.id == *folder_guild_id) {
                        entries.push(ListEntry::Guild { guild_index: idx });
                        placed[idx] = true;
                    }
                }
            }

            // Any guilds not in a folder go at the end
            for (i, was_placed) in placed.iter().enumerate() {
                if !was_placed {
                    entries.push(ListEntry::Guild { guild_index: i });
                }
            }
        }

        entries
    }

    /// Find the selectable entry index at the current selected_index.
    fn is_selectable(entry: &ListEntry) -> bool {
        !matches!(entry, ListEntry::FolderHeader { .. })
    }

    /// Move selection in a direction, skipping folder headers.
    fn move_selection(&mut self, entries: &[ListEntry], delta: isize) {
        let mut idx = self.selected_index as isize + delta;
        while idx >= 0 && (idx as usize) < entries.len() {
            if Self::is_selectable(&entries[idx as usize]) {
                self.selected_index = idx as usize;
                return;
            }
            idx += delta;
        }
    }

    /// Apply the current selection to the store.
    fn apply_selection(&self, entries: &[ListEntry], store: &mut Store) -> Option<Action> {
        match &entries[self.selected_index] {
            ListEntry::Dms => {
                store.ui.dm_mode = true;
                store.ui.selected_guild = None;
                store.ui.selected_channel = None;
                if store.dm_channels.is_empty() {
                    Some(Action::FetchDmChannels)
                } else {
                    None
                }
            }
            ListEntry::Guild { guild_index } => {
                if let Some(guild) = store.guilds.guilds.get(*guild_index) {
                    let guild_id = guild.id;
                    let needs_fetch = guild.channels.is_empty();
                    store.ui.selected_guild = Some(guild_id);
                    store.ui.selected_channel = None;
                    store.ui.dm_mode = false;
                    if needs_fetch {
                        Some(Action::FetchGuildChannels { guild_id })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ListEntry::FolderHeader { .. } => None,
        }
    }
}

impl Component for ServerList {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>> {
        if store.ui.focus != FocusTarget::ServerList {
            return Ok(None);
        }

        let entries = Self::build_entries(store);

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_selection(&entries, 1);
                let action = self.apply_selection(&entries, store);
                return Ok(action);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(&entries, -1);
                let action = self.apply_selection(&entries, store);
                return Ok(action);
            }
            KeyCode::Enter | KeyCode::Right => {
                store.ui.focus = FocusTarget::ChannelTree;
            }
            KeyCode::Esc | KeyCode::Left => {}
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

        let entries = Self::build_entries(store);
        let mut items: Vec<ListItem> = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            match entry {
                ListEntry::Dms => {
                    let is_selected = self.selected_index == i;
                    let style = if is_selected { theme::selected() } else { theme::base() };
                    items.push(ListItem::new(Line::from(Span::styled("Direct Messages", style))));
                }
                ListEntry::FolderHeader { name, .. } => {
                    // Non-selectable dimmed folder header
                    let style = Style::default().fg(theme::TEXT_MUTED).add_modifier(Modifier::ITALIC);
                    items.push(ListItem::new(Line::from(Span::styled(
                        format!("▾ {}", name),
                        style,
                    ))));
                }
                ListEntry::Guild { guild_index } => {
                    if let Some(guild) = store.guilds.guilds.get(*guild_index) {
                        let is_selected = self.selected_index == i;
                        let has_mention = guild.channels.iter()
                            .any(|ch| store.notifications.has_mentions(ch.id));
                        let has_unread = guild.channels.iter()
                            .any(|ch| store.notifications.has_unreads(ch.id));

                        let name_style = if is_selected {
                            theme::selected()
                        } else if has_unread || has_mention {
                            theme::base().add_modifier(Modifier::BOLD)
                        } else {
                            theme::base()
                        };

                        // Indent guilds that are inside a folder (a FolderHeader appears before them)
                        let in_folder = entries[..i].iter().rev()
                            .find(|e| matches!(e, ListEntry::FolderHeader { .. } | ListEntry::Dms))
                            .is_some_and(|e| matches!(e, ListEntry::FolderHeader { .. }));
                        let prefix = if in_folder { "  " } else { "" };

                        let mut spans = vec![Span::styled(format!("{}{}", prefix, guild.name), name_style)];

                        if has_mention {
                            let mention_count: u32 = guild.channels.iter()
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
                }
            }
        }

        let list = List::new(items)
            .block(block)
            .highlight_style(theme::selected());
        let mut state = ListState::default().with_selected(Some(self.selected_index));
        frame.render_stateful_widget(list, area, &mut state);
    }
}
