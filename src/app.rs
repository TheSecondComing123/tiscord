use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::discord::actions::Action;
use crate::discord::events::DiscordEvent;
use crate::store::state::ConnectionStatus;
use crate::store::Store;
use crate::tui::keybindings::{KeyAction, KeyDispatcher};
use crate::tui::terminal::Tui;
use crate::tui::theme;

pub struct App {
    store: Arc<RwLock<Store>>,
    #[allow(dead_code)]
    action_tx: mpsc::UnboundedSender<Action>,
    discord_event_rx: mpsc::UnboundedReceiver<DiscordEvent>,
    #[allow(dead_code)]
    config: Config,
    should_quit: bool,
    key_dispatcher: KeyDispatcher,
}

impl App {
    pub fn new(
        store: Arc<RwLock<Store>>,
        action_tx: mpsc::UnboundedSender<Action>,
        discord_event_rx: mpsc::UnboundedReceiver<DiscordEvent>,
        config: Config,
    ) -> Self {
        Self {
            store,
            action_tx,
            discord_event_rx,
            config,
            should_quit: false,
            key_dispatcher: KeyDispatcher::new(),
        }
    }

    pub async fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        let tick_rate = Duration::from_millis(1000 / u64::from(self.config.ui.fps));

        loop {
            // Drain pending discord events
            while let Ok(event) = self.discord_event_rx.try_recv() {
                let mut store = self.store.write().unwrap();
                store.process_discord_event(event);
            }

            // Render
            {
                let store = self.store.read().unwrap();
                let status_text = match store.ui.connection_status {
                    ConnectionStatus::Connecting => "tiscord - connecting...",
                    ConnectionStatus::Connected => "tiscord - connected",
                    ConnectionStatus::Disconnected => "tiscord - disconnected",
                    ConnectionStatus::Reconnecting => "tiscord - reconnecting...",
                };
                terminal.draw(|frame| {
                    let area = frame.area();
                    let paragraph = Paragraph::new(Text::raw(status_text))
                        .style(theme::base());
                    frame.render_widget(paragraph, area);
                })?;
            }

            // Poll terminal events
            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    let (mode, focus) = {
                        let store = self.store.read().unwrap();
                        (store.ui.input_mode, store.ui.focus)
                    };

                    let action = self.key_dispatcher.dispatch(key, mode, focus);
                    match action {
                        KeyAction::Quit => {
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }
}
