use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::discord::actions::Action;
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::theme;

pub struct ChannelHeader;

impl ChannelHeader {
    pub fn new() -> Self {
        Self
    }
}

impl Component for ChannelHeader {
    fn handle_key_event(&mut self, _key: KeyEvent, _store: &mut Store) -> Result<Option<Action>> {
        Ok(None)
    }

    fn render(&self, frame: &mut Frame, area: Rect, store: &Store) {
        let (channel_name, topic) = resolve_channel_info(store);

        let mut spans = vec![Span::styled(
            format!("# {}", channel_name),
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        )];

        if let Some(topic_text) = topic {
            if !topic_text.is_empty() {
                spans.push(Span::styled("  ", Style::default()));
                spans.push(Span::styled(topic_text, theme::muted()));
            }
        }

        // Show pin count if there are pinned messages loaded for this channel.
        if let Some(channel_id) = store.ui.selected_channel {
            if let Some(Some(pins)) = store.pinned_messages.get(&channel_id) {
                if !pins.is_empty() {
                    spans.push(Span::styled(
                        format!("  \u{1f4cc} {}", pins.len()),
                        Style::default().fg(theme::TEXT_MUTED),
                    ));
                }
            }
        }

        let line = Line::from(spans);

        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::BG));

        let paragraph = Paragraph::new(line).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn resolve_channel_info(store: &Store) -> (String, Option<String>) {
    let guild_id = match store.ui.selected_guild {
        Some(id) => id,
        None => return ("No channel".to_string(), None),
    };

    let channel_id = match store.ui.selected_channel {
        Some(id) => id,
        None => return ("No channel".to_string(), None),
    };

    let guild = match store.guilds.get_guild(guild_id) {
        Some(g) => g,
        None => return ("No channel".to_string(), None),
    };

    match guild.channels.iter().find(|ch| ch.id == channel_id) {
        Some(ch) => (ch.name.clone(), None), // ChannelInfo doesn't store topic
        None => ("No channel".to_string(), None),
    }
}
