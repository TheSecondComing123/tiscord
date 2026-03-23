use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::discord::actions::Action;
use crate::discord::events::DiscordEvent;
use crate::store::state::{ConnectionStatus, FocusTarget, ReplyTarget};
use crate::store::Store;
use crate::tui::component::Component;
use crate::tui::components::member_sidebar::MemberSidebar;
use crate::tui::components::message_pane::MessagePane;
use crate::tui::components::overlays::command_palette::CommandPalette;
use crate::tui::components::sidebar::ServerChannelSidebar;
use crate::tui::keybindings::KeyAction;
use crate::tui::terminal::Tui;
use crate::tui::theme;

pub struct App {
    store: Arc<RwLock<Store>>,
    action_tx: mpsc::UnboundedSender<Action>,
    discord_event_rx: mpsc::UnboundedReceiver<DiscordEvent>,
    config: Config,
    should_quit: bool,
    sidebar: ServerChannelSidebar,
    message_pane: MessagePane,
    member_sidebar: MemberSidebar,
    command_palette: CommandPalette,
    error_message: Option<(String, Instant)>,
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
            sidebar: ServerChannelSidebar::new(),
            message_pane: MessagePane::new(),
            member_sidebar: MemberSidebar::new(),
            command_palette: CommandPalette::new(),
            error_message: None,
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

            // Auto-clear error messages older than 5 seconds.
            if let Some((_, ts)) = &self.error_message {
                if ts.elapsed() >= Duration::from_secs(5) {
                    self.error_message = None;
                }
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
                let error_ref = &self.error_message;

                terminal.draw(|frame| {
                    let area = frame.area();

                    // Split vertically: main content above, 1-line status bar below.
                    let rows = Layout::vertical([
                        Constraint::Min(1),
                        Constraint::Length(1),
                    ])
                    .split(area);

                    let main_area = rows[0];
                    let status_area = rows[1];

                    // Build horizontal layout constraints for the main area.
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

                    let columns = Layout::horizontal(constraints).split(main_area);

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

                    // Status bar
                    render_status_bar(frame, status_area, &store, error_ref);

                    // Overlay: command palette (rendered on top of everything else).
                    if command_palette_ref.is_visible() {
                        command_palette_ref.render(frame, area, &store);
                    }
                })?;
            }

            // Poll terminal events
            if event::poll(tick_rate)? {
                match event::read()? {
                    Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                        self.handle_key(key)?;
                    }
                    _ => {}
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        let focus = {
            let store = self.store.read().unwrap();
            store.ui.focus
        };

        // When the command palette is focused, route all keys to it directly
        // before global shortcuts can intercept them.
        if focus == FocusTarget::CommandPalette {
            let mut store = self.store.write().unwrap();
            let result = self.command_palette.handle_key_event(key, &mut store)?;
            if let Some(action) = result {
                let _ = self.action_tx.send(action);
            }
            return Ok(());
        }

        // Global shortcuts (Ctrl+key)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    self.should_quit = true;
                    return Ok(());
                }
                KeyCode::Char('k') => {
                    let mut store = self.store.write().unwrap();
                    self.command_palette.open(&store);
                    store.ui.focus = FocusTarget::CommandPalette;
                    return Ok(());
                }
                KeyCode::Char('m') => {
                    let mut store = self.store.write().unwrap();
                    store.ui.member_sidebar_visible = !store.ui.member_sidebar_visible;
                    if store.ui.member_sidebar_visible {
                        store.ui.focus = FocusTarget::MemberSidebar;
                    } else if store.ui.focus == FocusTarget::MemberSidebar {
                        store.ui.focus = FocusTarget::MessageList;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        // Tab / Shift+Tab - cycle focus (except when typing in MessageInput)
        if focus != FocusTarget::MessageInput {
            match key.code {
                KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
                    let mut store = self.store.write().unwrap();
                    store.ui.focus = cycle_focus_forward(store.ui.focus);
                    return Ok(());
                }
                KeyCode::BackTab => {
                    let mut store = self.store.write().unwrap();
                    store.ui.focus = cycle_focus_backward(store.ui.focus);
                    return Ok(());
                }
                _ => {}
            }
        }

        // Route to the focused component
        let mut store = self.store.write().unwrap();
        let result = match store.ui.focus {
            FocusTarget::ServerList | FocusTarget::ChannelTree => {
                self.sidebar.handle_key_event(key, &mut store)?
            }
            FocusTarget::MessageList => {
                self.message_pane.message_list.handle_key_event(key, &mut store)?
            }
            FocusTarget::MessageInput => {
                self.message_pane.message_input.handle_key_event(key, &mut store)?
            }
            FocusTarget::MemberSidebar => {
                self.member_sidebar.handle_key_event(key, &mut store)?
            }
            FocusTarget::CommandPalette => {
                // Already handled above, but satisfy the match
                None
            }
        };

        // Handle cross-component actions returned by components
        if let Some(action) = result {
            match action {
                Action::ComponentKeyAction(key_action) => {
                    match key_action {
                        KeyAction::Reply => {
                            let msg_data = self
                                .message_pane
                                .message_list
                                .get_selected_message(&store)
                                .map(|m| (m.id, m.author_name.clone(), m.content.chars().take(80).collect::<String>()));

                            if let Some((message_id, author_name, content_preview)) = msg_data {
                                store.ui.reply_to = Some(ReplyTarget {
                                    message_id,
                                    author_name,
                                    content_preview,
                                });
                                store.ui.focus = FocusTarget::MessageInput;
                            }
                        }
                        KeyAction::EditMessage => {
                            let current_user_id = store.current_user_id;
                            let msg_data = self
                                .message_pane
                                .message_list
                                .get_selected_message(&store)
                                .filter(|m| Some(m.author_id) == current_user_id)
                                .map(|m| (m.id, m.content.clone()));

                            if let Some((message_id, content)) = msg_data {
                                store.ui.editing_message = Some(message_id);
                                store.ui.reply_to = None;
                                store.ui.focus = FocusTarget::MessageInput;
                                drop(store);
                                self.message_pane.message_input.set_content(content);
                                return Ok(());
                            }
                        }
                        KeyAction::DeleteMessage => {
                            let current_user_id = store.current_user_id;
                            let channel_id = store.ui.selected_channel;
                            let msg_data = self
                                .message_pane
                                .message_list
                                .get_selected_message(&store)
                                .filter(|m| Some(m.author_id) == current_user_id)
                                .and_then(|m| channel_id.map(|ch| (ch, m.id)));

                            if let Some((channel_id, message_id)) = msg_data {
                                let _ = self.action_tx.send(Action::DeleteMessage { channel_id, message_id });
                            }
                        }
                    }
                }
                // Regular discord actions (SendMessage, FetchMessages, etc.)
                other => {
                    let _ = self.action_tx.send(other);
                }
            }
        }

        Ok(())
    }
}

fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    store: &Store,
    error_message: &Option<(String, std::time::Instant)>,
) {
    use ratatui::widgets::Paragraph;

    let status_bg = Style::default().bg(theme::BG_SECONDARY);

    // Left section: connection status
    let (conn_text, conn_color) = match store.ui.connection_status {
        ConnectionStatus::Connected => ("Connected", theme::ONLINE),
        ConnectionStatus::Reconnecting => ("Reconnecting...", theme::IDLE),
        ConnectionStatus::Connecting => ("Connecting...", theme::IDLE),
        ConnectionStatus::Disconnected => ("Disconnected", theme::DND),
    };
    let left_span = Span::styled(
        format!(" {conn_text} "),
        status_bg.fg(conn_color),
    );

    // Center section: error message (if active) or guild > channel
    let center_span = if let Some((msg, _)) = error_message {
        Span::styled(
            msg.clone(),
            status_bg.fg(theme::DND),
        )
    } else {
        let center_text = {
            let guild_name = store
                .ui
                .selected_guild
                .and_then(|gid| store.guilds.get_guild(gid))
                .map(|g| g.name.as_str())
                .unwrap_or("");
            let channel_name = store
                .ui
                .selected_channel
                .and_then(|cid| {
                    store.ui.selected_guild.and_then(|gid| {
                        store
                            .guilds
                            .get_guild(gid)
                            .and_then(|g| g.channels.iter().find(|c| c.id == cid))
                            .map(|c| c.name.as_str())
                    })
                })
                .unwrap_or("");
            if guild_name.is_empty() && channel_name.is_empty() {
                String::new()
            } else if guild_name.is_empty() {
                format!("#{channel_name}")
            } else if channel_name.is_empty() {
                guild_name.to_string()
            } else {
                format!("{guild_name} > #{channel_name}")
            }
        };
        Span::styled(center_text, status_bg.fg(theme::TEXT_SECONDARY))
    };

    // Right section: focused panel
    let (focus_text, focus_color) = match store.ui.focus {
        FocusTarget::ServerList => ("SERVERS", theme::TEXT_SECONDARY),
        FocusTarget::ChannelTree => ("CHANNELS", theme::TEXT_SECONDARY),
        FocusTarget::MessageList => ("MESSAGES", theme::ONLINE),
        FocusTarget::MessageInput => ("INPUT", theme::ACCENT),
        FocusTarget::MemberSidebar => ("MEMBERS", theme::TEXT_SECONDARY),
        FocusTarget::CommandPalette => ("PALETTE", theme::ACCENT),
    };
    let right_span = Span::styled(
        format!(" {focus_text} "),
        status_bg.fg(focus_color),
    );

    // Split the status bar into three equal columns.
    let cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

    // Fill background across the whole bar first.
    frame.render_widget(Paragraph::new("").style(status_bg), area);

    frame.render_widget(
        Paragraph::new(Line::from(left_span)).style(status_bg),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(center_span))
            .style(status_bg)
            .alignment(Alignment::Center),
        cols[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(right_span))
            .style(status_bg)
            .alignment(Alignment::Right),
        cols[2],
    );
}

fn cycle_focus_forward(current: FocusTarget) -> FocusTarget {
    match current {
        FocusTarget::ServerList => FocusTarget::ChannelTree,
        FocusTarget::ChannelTree => FocusTarget::MessageInput,
        FocusTarget::MessageList => FocusTarget::MessageInput,
        FocusTarget::MessageInput => FocusTarget::ServerList,
        other => other,
    }
}

fn cycle_focus_backward(current: FocusTarget) -> FocusTarget {
    match current {
        FocusTarget::ServerList => FocusTarget::MessageInput,
        FocusTarget::ChannelTree => FocusTarget::ServerList,
        FocusTarget::MessageList => FocusTarget::ChannelTree,
        FocusTarget::MessageInput => FocusTarget::ChannelTree,
        other => other,
    }
}
