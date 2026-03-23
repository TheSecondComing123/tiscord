use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::discord::actions::Action;
use crate::discord::events::DiscordEvent;
use crate::store::state::{ConnectionStatus, FocusTarget, InputMode, ReplyTarget};
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::components::member_sidebar::MemberSidebar;
use crate::tui::components::message_pane::MessagePane;
use crate::tui::components::overlays::command_palette::CommandPalette;
use crate::tui::components::sidebar::ServerChannelSidebar;
use crate::tui::keybindings::{KeyAction, KeyDispatcher};
use crate::tui::terminal::Tui;
use crate::tui::theme;

pub struct App {
    store: Arc<RwLock<Store>>,
    action_tx: mpsc::UnboundedSender<Action>,
    discord_event_rx: mpsc::UnboundedReceiver<DiscordEvent>,
    config: Config,
    should_quit: bool,
    key_dispatcher: KeyDispatcher,
    sidebar: ServerChannelSidebar,
    message_pane: MessagePane,
    member_sidebar: MemberSidebar,
    command_palette: CommandPalette,
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
            sidebar: ServerChannelSidebar::new(),
            message_pane: MessagePane::new(),
            member_sidebar: MemberSidebar::new(),
            command_palette: CommandPalette::new(),
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
                let sidebar_width = self.config.ui.layout.sidebar_width;
                let member_width = self.config.ui.layout.member_width;
                let member_visible = store.ui.member_sidebar_visible;

                let sidebar_ref = &self.sidebar;
                let message_pane_ref = &self.message_pane;
                let member_sidebar_ref = &self.member_sidebar;
                let command_palette_ref = &self.command_palette;

                terminal.draw(|frame| {
                    let area = frame.area();

                    // Build horizontal layout constraints
                    let constraints: Vec<Constraint> = if member_visible {
                        vec![
                            Constraint::Length(sidebar_width),
                            Constraint::Min(1),
                            Constraint::Length(member_width),
                        ]
                    } else {
                        vec![
                            Constraint::Length(sidebar_width),
                            Constraint::Min(1),
                        ]
                    };

                    let columns = Layout::horizontal(constraints).split(area);

                    // Left: sidebar with right border
                    let sidebar_block = Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(Style::default().fg(theme::BORDER))
                        .style(Style::default().bg(theme::BG));
                    let sidebar_inner = sidebar_block.inner(columns[0]);
                    frame.render_widget(sidebar_block, columns[0]);
                    sidebar_ref.render(frame, sidebar_inner, &store);

                    // Center: message pane
                    message_pane_ref.render(frame, columns[1], &store);

                    // Right: member sidebar (if visible)
                    if member_visible && columns.len() > 2 {
                        member_sidebar_ref.render(frame, columns[2], &store);
                    }

                    // Status bar hint at bottom-right of the frame area
                    let status_text = match store.ui.connection_status {
                        ConnectionStatus::Connecting => "connecting...",
                        ConnectionStatus::Connected => "",
                        ConnectionStatus::Disconnected => "disconnected",
                        ConnectionStatus::Reconnecting => "reconnecting...",
                    };
                    if !status_text.is_empty() {
                        let status_span = Span::styled(status_text, theme::muted());
                        let status_line = Line::from(status_span).alignment(Alignment::Right);
                        let status_area = Rect {
                            x: area.x,
                            y: area.y,
                            width: area.width,
                            height: 1,
                        };
                        frame.render_widget(status_line, status_area);
                    }

                    // Overlay: command palette (rendered on top of everything else).
                    if command_palette_ref.is_visible() {
                        command_palette_ref.render(frame, area, &store);
                    }
                })?;
            }

            // Poll terminal events
            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key)?;
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        let (mode, focus) = {
            let store = self.store.read().unwrap();
            (store.ui.input_mode, store.ui.focus)
        };

        // When the command palette is focused, route all keys to it directly
        // before the normal dispatcher has a chance to intercept them.
        if focus == FocusTarget::CommandPalette {
            let mut store = self.store.write().unwrap();
            let result = self.command_palette.handle_key_event(key, &mut store)?;
            if let Some(action) = result {
                let _ = self.action_tx.send(action);
            }
            return Ok(());
        }

        let action = self.key_dispatcher.dispatch(key, mode, focus);

        match action {
            KeyAction::Quit => {
                self.should_quit = true;
            }

            KeyAction::FocusSidebar => {
                let mut store = self.store.write().unwrap();
                store.ui.focus = FocusTarget::ServerList;
                store.ui.input_mode = InputMode::Normal;
            }

            KeyAction::ToggleMemberSidebar => {
                let mut store = self.store.write().unwrap();
                store.ui.member_sidebar_visible = !store.ui.member_sidebar_visible;
                if store.ui.member_sidebar_visible {
                    store.ui.focus = FocusTarget::MemberSidebar;
                } else if store.ui.focus == FocusTarget::MemberSidebar {
                    store.ui.focus = FocusTarget::MessageList;
                }
            }

            KeyAction::EnterInsertMode => {
                let mut store = self.store.write().unwrap();
                store.ui.focus = FocusTarget::MessageInput;
                store.ui.input_mode = InputMode::Insert;
            }

            KeyAction::ExitInsertMode => {
                let mut store = self.store.write().unwrap();
                store.ui.focus = FocusTarget::MessageList;
                store.ui.input_mode = InputMode::Normal;
                store.ui.reply_to = None;
                store.ui.editing_message = None;
                self.message_pane.message_input.clear();
            }

            KeyAction::CycleFocusForward => {
                let mut store = self.store.write().unwrap();
                store.ui.focus = cycle_focus_forward(store.ui.focus);
            }

            KeyAction::CycleFocusBackward => {
                let mut store = self.store.write().unwrap();
                store.ui.focus = cycle_focus_backward(store.ui.focus);
            }

            KeyAction::MoveUp | KeyAction::MoveDown | KeyAction::Select | KeyAction::Back
            | KeyAction::JumpToTop | KeyAction::JumpToBottom | KeyAction::PageUp
            | KeyAction::PageDown => {
                let mut store = self.store.write().unwrap();
                let result = match store.ui.focus {
                    FocusTarget::ServerList | FocusTarget::ChannelTree => {
                        self.sidebar.handle_key_event(key, &mut store)?
                    }
                    FocusTarget::MessageList => {
                        self.message_pane.message_list.handle_key_event(key, &mut store)?
                    }
                    FocusTarget::MemberSidebar => {
                        self.member_sidebar.handle_key_event(key, &mut store)?
                    }
                    _ => None,
                };
                if let Some(action) = result {
                    let _ = self.action_tx.send(action);
                }
            }

            KeyAction::SendMessage => {
                // SendMessage comes from the KeyDispatcher in Insert mode (Enter key).
                // Route to the message_input to handle the actual send logic.
                let mut store = self.store.write().unwrap();
                let result = self.message_pane.message_input.handle_key_event(key, &mut store)?;
                if let Some(action) = result {
                    let _ = self.action_tx.send(action);
                }
                // After sending, exit insert mode
                store.ui.focus = FocusTarget::MessageList;
                store.ui.input_mode = InputMode::Normal;
            }

            KeyAction::InsertNewline => {
                // Shift+Enter in insert mode - pass through to input
                let mut store = self.store.write().unwrap();
                let result = self.message_pane.message_input.handle_key_event(key, &mut store)?;
                if let Some(action) = result {
                    let _ = self.action_tx.send(action);
                }
            }

            KeyAction::Unhandled(raw_key) => {
                // In insert mode, pass unhandled keys to the focused component
                let mut store = self.store.write().unwrap();
                let result = match store.ui.focus {
                    FocusTarget::MessageInput => {
                        self.message_pane.message_input.handle_key_event(raw_key, &mut store)?
                    }
                    FocusTarget::MessageList => {
                        self.message_pane.message_list.handle_key_event(raw_key, &mut store)?
                    }
                    FocusTarget::ServerList | FocusTarget::ChannelTree => {
                        self.sidebar.handle_key_event(raw_key, &mut store)?
                    }
                    FocusTarget::MemberSidebar => {
                        self.member_sidebar.handle_key_event(raw_key, &mut store)?
                    }
                    _ => None,
                };
                if let Some(action) = result {
                    let _ = self.action_tx.send(action);
                }
            }

            KeyAction::Reply => {
                let store_read = self.store.read().unwrap();
                let msg_data = self
                    .message_pane
                    .message_list
                    .get_selected_message(&store_read)
                    .map(|m| (m.id, m.author_name.clone(), m.content.chars().take(80).collect::<String>()));
                drop(store_read);

                if let Some((message_id, author_name, content_preview)) = msg_data {
                    let mut store = self.store.write().unwrap();
                    store.ui.reply_to = Some(ReplyTarget {
                        message_id,
                        author_name,
                        content_preview,
                    });
                    store.ui.focus = FocusTarget::MessageInput;
                    store.ui.input_mode = InputMode::Insert;
                }
            }

            KeyAction::EditMessage => {
                let store_read = self.store.read().unwrap();
                let current_user_id = store_read.current_user_id;
                let msg_data = self
                    .message_pane
                    .message_list
                    .get_selected_message(&store_read)
                    .filter(|m| Some(m.author_id) == current_user_id)
                    .map(|m| (m.id, m.content.clone()));
                drop(store_read);

                if let Some((message_id, content)) = msg_data {
                    let mut store = self.store.write().unwrap();
                    store.ui.editing_message = Some(message_id);
                    store.ui.reply_to = None;
                    store.ui.focus = FocusTarget::MessageInput;
                    store.ui.input_mode = InputMode::Insert;
                    drop(store);
                    self.message_pane.message_input.set_content(content);
                }
            }

            KeyAction::DeleteMessage => {
                let store_read = self.store.read().unwrap();
                let current_user_id = store_read.current_user_id;
                let channel_id = store_read.ui.selected_channel;
                let msg_data = self
                    .message_pane
                    .message_list
                    .get_selected_message(&store_read)
                    .filter(|m| Some(m.author_id) == current_user_id)
                    .and_then(|m| channel_id.map(|ch| (ch, m.id)));
                drop(store_read);

                if let Some((channel_id, message_id)) = msg_data {
                    let _ = self.action_tx.send(Action::DeleteMessage { channel_id, message_id });
                }
            }

            KeyAction::OpenCommandPalette => {
                let mut store = self.store.write().unwrap();
                self.command_palette.open(&store);
                store.ui.focus = FocusTarget::CommandPalette;
                store.ui.input_mode = InputMode::Insert;
            }

            // Actions not yet implemented - ignore for now
            KeyAction::AddReaction
            | KeyAction::YankMessage
            | KeyAction::OpenSearch
            | KeyAction::NextSearchResult
            | KeyAction::PrevSearchResult => {}
        }

        Ok(())
    }
}

fn cycle_focus_forward(current: FocusTarget) -> FocusTarget {
    match current {
        FocusTarget::ServerList => FocusTarget::ChannelTree,
        FocusTarget::ChannelTree => FocusTarget::MessageList,
        FocusTarget::MessageList => FocusTarget::ServerList,
        other => other,
    }
}

fn cycle_focus_backward(current: FocusTarget) -> FocusTarget {
    match current {
        FocusTarget::ServerList => FocusTarget::MessageList,
        FocusTarget::ChannelTree => FocusTarget::ServerList,
        FocusTarget::MessageList => FocusTarget::ChannelTree,
        other => other,
    }
}
