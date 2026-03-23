# Tiscord MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working Discord TUI client that connects to Discord, shows servers/channels/messages, and lets you send messages - usable as a daily driver.

**Architecture:** Three async layers (Discord, Store, TUI) communicating over tokio mpsc channels. Unidirectional data flow: Discord events update the store, TUI reads the store on render ticks, user actions flow back to the Discord layer as REST calls. See `docs/superpowers/specs/2026-03-22-tiscord-design.md` for full spec.

**Tech Stack:** Rust 2024 edition, ratatui + crossterm, twilight (gateway/http/model/cache), tokio

---

## File map

Every file this plan creates, with its responsibility:

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Dependencies and project metadata |
| `src/main.rs` | Entry point: parse args, init tracing, load config, run app |
| `src/app.rs` | App struct: owns store + channels, runs main event loop (poll + try_recv) |
| `src/config.rs` | Config struct, TOML parsing, defaults, XDG paths |
| `src/auth.rs` | Token storage/retrieval via OS keyring, first-run prompt |
| `src/discord/mod.rs` | Re-exports |
| `src/discord/client.rs` | Start gateway shard, create HTTP client, spawn event loop |
| `src/discord/events.rs` | `DiscordEvent` enum, translate twilight gateway events |
| `src/discord/actions.rs` | `Action` enum, dispatch actions to REST/gateway |
| `src/store/mod.rs` | Re-exports, `Store` struct definition |
| `src/store/state.rs` | `AppState` inner struct: guilds, channels, messages, UI state |
| `src/store/guilds.rs` | Guild/channel state: tree building, selection, sorting |
| `src/store/messages.rs` | `MessageBuffer` ring buffer, history management |
| `src/store/notifications.rs` | Unread counts, mention tracking per channel/guild |
| `src/tui/mod.rs` | Re-exports |
| `src/tui/terminal.rs` | Terminal init/restore, crossterm setup |
| `src/tui/component.rs` | `Component` trait, `InputMode` enum, `FocusTarget` enum |
| `src/tui/theme.rs` | Color palette, style constants |
| `src/tui/keybindings.rs` | Keybinding dispatch, chord state machine, mode-aware routing |
| `src/tui/markdown.rs` | Discord markdown parser -> `Vec<ratatui::text::Span>` |
| `src/tui/components/mod.rs` | Re-exports all components |
| `src/tui/components/sidebar.rs` | `ServerChannelSidebar`: composes server list + channel tree |
| `src/tui/components/server_list.rs` | `ServerList`: guild icons, selection, unread badges |
| `src/tui/components/channel_tree.rs` | `ChannelTree`: categories + channels, collapsible |
| `src/tui/components/dm_list.rs` | `DMList`: DM conversations, replaces channel tree |
| `src/tui/components/message_pane.rs` | `MessagePane`: composes header + list + input |
| `src/tui/components/message_list.rs` | `MessageList`: scrollable message feed |
| `src/tui/components/message.rs` | `MessageWidget`: single message rendering |
| `src/tui/components/message_input.rs` | `MessageInput`: text editor with multiline, cursor |
| `src/tui/components/channel_header.rs` | `ChannelHeader`: channel name, topic |
| `src/tui/components/member_sidebar.rs` | `MemberSidebar`: online/offline grouped member list |
| `src/tui/components/overlays/mod.rs` | Re-exports, overlay rendering logic |
| `src/tui/components/overlays/command_palette.rs` | `CommandPalette`: fuzzy find servers/channels/DMs |
| `src/utils/mod.rs` | Re-exports |
| `src/utils/time.rs` | Relative timestamp formatting |

---

## Task 1: Project scaffolding and dependencies

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Initialize Cargo project**

```bash
cargo init --name tiscord
```

- [ ] **Step 2: Write Cargo.toml with all dependencies**

```toml
[package]
name = "tiscord"
version = "0.1.0"
edition = "2024"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
twilight-gateway = "0.16"
twilight-http = "0.16"
twilight-model = "0.16"
twilight-cache-inmemory = "0.16"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
dirs = "6"
unicode-width = "0.2"
fuzzy-matcher = "0.3"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
```

Note: Check latest crate versions at crates.io before writing. The twilight version may differ - use whatever is current. The `keyring` features may vary by platform.

- [ ] **Step 3: Write minimal main.rs that compiles**

```rust
fn main() {
    println!("tiscord");
}
```

- [ ] **Step 4: Add .gitignore**

```
/target
.superpowers/
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no errors (warnings OK for unused deps)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs .gitignore
git commit -m "feat: initialize project with dependencies"
```

---

## Task 2: Config system

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write tests for config defaults and parsing**

Create `src/config.rs` with a test module that verifies:
- Default config values (fps=30, timestamps="relative", member_sidebar=true, sidebar_width=20, member_width=20)
- Parsing a TOML string into Config
- Missing fields fall back to defaults
- Config file path resolves to `~/.config/tiscord/config.toml` (via `dirs::config_dir()`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.ui.fps, 30);
        assert_eq!(config.ui.timestamps, TimestampMode::Relative);
        assert!(config.ui.member_sidebar);
        assert_eq!(config.ui.layout.sidebar_width, 20);
        assert_eq!(config.ui.layout.member_width, 20);
        assert!(!config.notifications.desktop);
        assert!(!config.notifications.mentions_only);
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml = r#"
            [ui]
            fps = 60
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.ui.fps, 60);
        // Other fields should be defaults
        assert_eq!(config.ui.timestamps, TimestampMode::Relative);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config`
Expected: FAIL - `Config` type doesn't exist yet

- [ ] **Step 3: Implement Config struct**

In `src/config.rs`, define:

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimestampMode {
    Relative,
    Absolute,
    Off,
}

impl Default for TimestampMode {
    fn default() -> Self {
        Self::Relative
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiLayout {
    pub sidebar_width: u16,
    pub member_width: u16,
}

impl Default for UiLayout {
    fn default() -> Self {
        Self {
            sidebar_width: 20,
            member_width: 20,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub fps: u16,
    pub timestamps: TimestampMode,
    pub member_sidebar: bool,
    pub layout: UiLayout,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            timestamps: TimestampMode::default(),
            member_sidebar: true,
            layout: UiLayout::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub desktop: bool,
    pub mentions_only: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            desktop: false,
            mentions_only: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ui: UiConfig,
    pub notifications: NotificationConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig::default(),
            notifications: NotificationConfig::default(),
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tiscord")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tiscord")
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }
}
```

- [ ] **Step 4: Wire config into main.rs**

```rust
mod config;

fn main() -> anyhow::Result<()> {
    let config = config::Config::load()?;
    println!("tiscord - fps: {}", config.ui.fps);
    Ok(())
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib config`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add config system with TOML parsing and defaults"
```

---

## Task 3: Auth / token management

**Files:**
- Create: `src/auth.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement auth module**

`src/auth.rs` handles token storage via OS keyring. Provides `get_token()` which returns the stored token or prompts the user for one via stdin. Uses `keyring` crate with service name `"tiscord"` and username `"discord_token"`.

```rust
use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "tiscord";
const USERNAME: &str = "discord_token";

pub fn get_token() -> Result<String> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    match entry.get_password() {
        Ok(token) => Ok(token),
        Err(keyring::Error::NoEntry) => {
            let token = prompt_token()?;
            entry.set_password(&token)?;
            Ok(token)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn clear_token() -> Result<()> {
    let entry = Entry::new(SERVICE, USERNAME)?;
    entry.delete_credential().context("failed to delete token")
}

fn prompt_token() -> Result<String> {
    eprintln!("No Discord token found. Enter your token:");
    let mut token = String::new();
    std::io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();
    if token.is_empty() {
        anyhow::bail!("token cannot be empty");
    }
    Ok(token)
}
```

Note: No unit tests for auth - it interacts with the OS keyring which requires system access. Manual testing only.

- [ ] **Step 2: Wire into main.rs**

Add `mod auth;` and call `auth::get_token()?` in main to verify it works.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles (may warn about unused imports)

- [ ] **Step 4: Commit**

```bash
git add src/auth.rs src/main.rs
git commit -m "feat: add token management via OS keyring"
```

---

## Task 4: Core types - DiscordEvent and Action enums

**Files:**
- Create: `src/discord/mod.rs`
- Create: `src/discord/events.rs`
- Create: `src/discord/actions.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create discord module structure**

`src/discord/mod.rs`:
```rust
pub mod actions;
pub mod client;
pub mod events;
```

Note: `client.rs` will be created empty for now (just enough to not error).

- [ ] **Step 2: Define DiscordEvent enum**

`src/discord/events.rs` - These are the internal events the Discord layer sends to the store. They abstract over twilight's gateway events into what the app cares about.

```rust
use twilight_model::channel::Message;
use twilight_model::gateway::payload::incoming::Ready;
use twilight_model::guild::Guild;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub enum DiscordEvent {
    Ready(Box<Ready>),
    GuildCreate(Box<Guild>),
    GuildDelete(Id<GuildMarker>),
    ChannelCreate(Box<twilight_model::channel::Channel>),
    ChannelUpdate(Box<twilight_model::channel::Channel>),
    ChannelDelete(Id<ChannelMarker>),
    MessageCreate(Box<Message>),
    MessageUpdate {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        content: Option<String>,
    },
    MessageDelete {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    TypingStart {
        channel_id: Id<ChannelMarker>,
        user_id: Id<twilight_model::id::marker::UserMarker>,
    },
    PresenceUpdate,
    MemberChunk {
        guild_id: Id<GuildMarker>,
        members: Vec<twilight_model::guild::Member>,
    },
    GatewayReconnect,
    GatewayDisconnect,
    // REST response events (sent by action handler, not gateway)
    MessagesLoaded {
        channel_id: Id<ChannelMarker>,
        messages: Vec<twilight_model::channel::Message>,
    },
    MembersLoaded {
        guild_id: Id<GuildMarker>,
        members: Vec<twilight_model::guild::Member>,
    },
}
```

- [ ] **Step 3: Define Action enum**

`src/discord/actions.rs` - These are actions the TUI sends to the Discord layer to execute.

```rust
use twilight_model::id::marker::{ChannelMarker, MessageMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub enum Action {
    SendMessage {
        channel_id: Id<ChannelMarker>,
        content: String,
        reply_to: Option<Id<MessageMarker>>,
    },
    EditMessage {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        content: String,
    },
    DeleteMessage {
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    },
    FetchMessages {
        channel_id: Id<ChannelMarker>,
        before: Option<Id<MessageMarker>>,
        limit: u16,
    },
    FetchGuildMembers {
        guild_id: Id<twilight_model::id::marker::GuildMarker>,
    },
}
```

- [ ] **Step 4: Create empty client.rs placeholder**

`src/discord/client.rs`:
```rust
// Discord gateway + HTTP client - implemented in Task 8
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build`
Expected: Compiles (warnings about unused code OK)

- [ ] **Step 6: Commit**

```bash
git add src/discord/
git commit -m "feat: define DiscordEvent and Action core types"
```

---

## Task 5: Store - state management

**Files:**
- Create: `src/store/mod.rs`
- Create: `src/store/state.rs`
- Create: `src/store/guilds.rs`
- Create: `src/store/messages.rs`
- Create: `src/store/notifications.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write MessageBuffer tests**

`src/store/messages.rs` - Test the ring buffer that holds message history per channel.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_retrieve() {
        let mut buf = MessageBuffer::new(5);
        buf.push(make_test_msg(1, "hello"));
        buf.push(make_test_msg(2, "world"));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.messages()[0].content, "hello");
        assert_eq!(buf.messages()[1].content, "world");
    }

    #[test]
    fn test_eviction_at_capacity() {
        let mut buf = MessageBuffer::new(3);
        buf.push(make_test_msg(1, "a"));
        buf.push(make_test_msg(2, "b"));
        buf.push(make_test_msg(3, "c"));
        buf.push(make_test_msg(4, "d"));
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.messages()[0].content, "b");
    }

    #[test]
    fn test_remove_by_id() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "a"));
        buf.push(make_test_msg(2, "b"));
        buf.push(make_test_msg(3, "c"));
        buf.remove(Id::new(2));
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.messages()[1].content, "c");
    }

    #[test]
    fn test_update_content() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "original"));
        buf.update(Id::new(1), "edited".to_string());
        assert_eq!(buf.messages()[0].content, "edited");
    }
}
```

Include a `make_test_msg` helper that creates a minimal `StoredMessage` struct.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib store::messages`
Expected: FAIL - types don't exist

- [ ] **Step 3: Implement MessageBuffer**

Define `StoredMessage` (a simplified, owned version of a Discord message with just the fields we need for display) and `MessageBuffer` (a `VecDeque` with a max capacity).

```rust
use std::collections::VecDeque;
use twilight_model::id::marker::MessageMarker;
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: Id<MessageMarker>,
    pub author_name: String,
    pub author_id: Id<twilight_model::id::marker::UserMarker>,
    pub content: String,
    pub timestamp: String,
    pub reply_to: Option<ReplyContext>,
    pub attachments: Vec<Attachment>,
    pub is_edited: bool,
}

#[derive(Debug, Clone)]
pub struct ReplyContext {
    pub author_name: String,
    pub content_preview: String,
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub size: u64,
    pub url: String,
}

pub struct MessageBuffer {
    messages: VecDeque<StoredMessage>,
    capacity: usize,
}

impl MessageBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, msg: StoredMessage) {
        if self.messages.len() >= self.capacity {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    pub fn messages(&self) -> &VecDeque<StoredMessage> {
        &self.messages
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn remove(&mut self, id: Id<MessageMarker>) {
        self.messages.retain(|m| m.id != id);
    }

    pub fn update(&mut self, id: Id<MessageMarker>, content: String) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.content = content;
            msg.is_edited = true;
        }
    }
}
```

- [ ] **Step 4: Run message buffer tests**

Run: `cargo test --lib store::messages`
Expected: All pass

- [ ] **Step 5: Implement notification tracking**

`src/store/notifications.rs` - Tracks unread counts and mention counts per channel.

```rust
use std::collections::HashMap;
use twilight_model::id::marker::ChannelMarker;
use twilight_model::id::Id;

#[derive(Debug, Default, Clone)]
pub struct ChannelNotification {
    pub unread_count: u32,
    pub mention_count: u32,
}

#[derive(Debug, Default)]
pub struct NotificationState {
    channels: HashMap<Id<ChannelMarker>, ChannelNotification>,
}
```

Implement methods: `increment_unread`, `increment_mentions`, `mark_read`, `get`, `has_unreads`, `has_mentions`. Write tests for each.

- [ ] **Step 6: Run notification tests**

Run: `cargo test --lib store::notifications`
Expected: All pass

- [ ] **Step 7: Implement guild/channel state**

`src/store/guilds.rs` - Stores guild info and channel trees.

```rust
use std::collections::HashMap;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub struct GuildInfo {
    pub id: Id<GuildMarker>,
    pub name: String,
    pub icon: Option<String>,
    pub channels: Vec<ChannelInfo>,
}

#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub id: Id<ChannelMarker>,
    pub name: String,
    pub kind: ChannelKind,
    pub category_id: Option<Id<ChannelMarker>>,
    pub position: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelKind {
    Text,
    Voice,
    Category,
    Announcement,
    Forum,
}

#[derive(Debug, Default)]
pub struct GuildState {
    pub guilds: Vec<GuildInfo>,
    guild_map: HashMap<Id<GuildMarker>, usize>,
}
```

Implement methods: `add_guild`, `remove_guild`, `get_guild`, `get_channels_for_guild` (returns channels sorted by category then position). Write tests.

- [ ] **Step 8: Run guild state tests**

Run: `cargo test --lib store::guilds`
Expected: All pass

- [ ] **Step 9: Implement Store and AppState**

`src/store/state.rs` - The UI-specific state (selected guild, selected channel, focus, input mode, scroll position).

`src/store/mod.rs` - The `Store` struct that composes everything.

```rust
// store/mod.rs
pub mod guilds;
pub mod messages;
pub mod notifications;
pub mod state;

use std::collections::HashMap;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

pub struct Store {
    pub guilds: guilds::GuildState,
    pub messages: HashMap<Id<ChannelMarker>, messages::MessageBuffer>,
    pub notifications: notifications::NotificationState,
    pub ui: state::UiState,
    pub current_user_id: Option<Id<twilight_model::id::marker::UserMarker>>,
    pub current_user_name: Option<String>,
}
```

`src/store/state.rs`:
```rust
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    ServerList,
    ChannelTree,
    MessageList,
    MessageInput,
    MemberSidebar,
    CommandPalette,
}

#[derive(Debug)]
pub struct UiState {
    pub selected_guild: Option<Id<GuildMarker>>,
    pub selected_channel: Option<Id<ChannelMarker>>,
    pub input_mode: InputMode,
    pub focus: FocusTarget,
    pub member_sidebar_visible: bool,
    pub message_scroll_offset: usize,
    pub sidebar_scroll_offset: usize,
    pub dm_mode: bool,
    pub connection_status: ConnectionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Reconnecting,
}
```

Implement `Store::new()`, `Store::get_or_create_message_buffer(channel_id)`.

- [ ] **Step 10: Verify everything compiles and tests pass**

Run: `cargo test --lib store`
Expected: All tests pass

- [ ] **Step 11: Commit**

```bash
git add src/store/ src/main.rs
git commit -m "feat: add store layer with message buffer, guilds, notifications, and UI state"
```

---

## Task 6: TUI foundation - terminal, Component trait, theme, keybindings

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/terminal.rs`
- Create: `src/tui/component.rs`
- Create: `src/tui/theme.rs`
- Create: `src/tui/keybindings.rs`
- Create: `src/tui/components/mod.rs`

- [ ] **Step 1: Create TUI module structure**

`src/tui/mod.rs`:
```rust
pub mod component;
pub mod components;
pub mod keybindings;
pub mod markdown;
pub mod terminal;
pub mod theme;
```

`src/tui/components/mod.rs`:
```rust
pub mod sidebar;
pub mod server_list;
pub mod channel_tree;
pub mod dm_list;
pub mod message_pane;
pub mod message_list;
pub mod message;
pub mod message_input;
pub mod channel_header;
pub mod member_sidebar;
pub mod overlays;
```

Create empty placeholder files for each component (just `// TODO` comments) so the module tree compiles.

- [ ] **Step 2: Implement terminal setup/restore**

`src/tui/terminal.rs`:

```rust
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, Stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> Result<Tui> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}
```

- [ ] **Step 3: Define Component trait**

`src/tui/component.rs`:

```rust
use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::discord::actions::Action;
use crate::store::Store;

pub trait Component {
    fn handle_key_event(&mut self, key: KeyEvent, store: &mut Store) -> Result<Option<Action>>;
    fn render(&self, frame: &mut Frame, area: Rect, store: &Store);
}
```

Note: Components take `&mut Store` on key events (to update UI state like selection, scroll) but `&Store` on render (read-only). This avoids needing separate channels for UI state updates.

- [ ] **Step 4: Define theme**

`src/tui/theme.rs` - Discord-inspired color palette.

```rust
use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(30, 31, 34);        // Discord dark bg
pub const BG_SECONDARY: Color = Color::Rgb(43, 45, 49);
pub const BG_TERTIARY: Color = Color::Rgb(35, 36, 40);
pub const TEXT_PRIMARY: Color = Color::Rgb(219, 222, 225);
pub const TEXT_SECONDARY: Color = Color::Rgb(148, 155, 164);
pub const TEXT_MUTED: Color = Color::Rgb(94, 103, 114);
pub const ACCENT: Color = Color::Rgb(88, 101, 242);   // Discord blurple
pub const ONLINE: Color = Color::Rgb(35, 165, 89);
pub const IDLE: Color = Color::Rgb(240, 178, 50);
pub const DND: Color = Color::Rgb(237, 66, 69);
pub const MENTION: Color = Color::Rgb(250, 168, 26);
pub const LINK: Color = Color::Rgb(0, 168, 252);
pub const BORDER: Color = Color::Rgb(63, 66, 72);

pub fn base() -> Style {
    Style::default().fg(TEXT_PRIMARY).bg(BG)
}

pub fn secondary_text() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}

pub fn muted() -> Style {
    Style::default().fg(TEXT_MUTED)
}

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn selected() -> Style {
    Style::default().bg(BG_SECONDARY).fg(TEXT_PRIMARY)
}

pub fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}
```

- [ ] **Step 5: Implement keybinding dispatch with chord support**

`src/tui/keybindings.rs` - Mode-aware key dispatch with chord state machine.

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

use crate::store::state::{FocusTarget, InputMode};

const CHORD_TIMEOUT_MS: u128 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    // Navigation
    FocusSidebar,
    ToggleMemberSidebar,
    OpenCommandPalette,
    MoveUp,
    MoveDown,
    Select,
    Back,
    CycleFocusForward,
    CycleFocusBackward,
    // Messages
    EnterInsertMode,
    Reply,
    EditMessage,
    DeleteMessage,
    AddReaction,
    JumpToTop,
    JumpToBottom,
    PageUp,
    PageDown,
    YankMessage,
    OpenSearch,
    NextSearchResult,
    PrevSearchResult,
    // Insert mode
    SendMessage,
    InsertNewline,
    ExitInsertMode,
    // No match
    Unhandled(KeyEvent),
}

pub struct KeyDispatcher {
    pending_chord: Option<(KeyCode, Instant)>,
}

impl KeyDispatcher {
    pub fn new() -> Self {
        Self {
            pending_chord: None,
        }
    }

    pub fn dispatch(&mut self, key: KeyEvent, mode: InputMode, focus: FocusTarget) -> KeyAction {
        // Ctrl-prefixed bindings work in all modes
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.dispatch_ctrl(key);
        }

        match mode {
            InputMode::Insert => self.dispatch_insert(key),
            InputMode::Normal => self.dispatch_normal(key, focus),
        }
    }
}
```

Implement `dispatch_ctrl`, `dispatch_insert`, `dispatch_normal` methods. The normal mode dispatcher handles chord sequences (check `pending_chord`, if `g` is pending and next key is `g` within timeout, return `JumpToTop`).

Write tests for: ctrl bindings in both modes, normal mode single keys, chord `g g`, chord timeout, insert mode passthrough.

- [ ] **Step 6: Run all TUI tests**

Run: `cargo test --lib tui`
Expected: All pass

- [ ] **Step 7: Verify full project compiles**

Run: `cargo build`
Expected: Compiles (warnings OK for unused placeholder modules)

- [ ] **Step 8: Commit**

```bash
git add src/tui/
git commit -m "feat: add TUI foundation - terminal, Component trait, theme, keybindings"
```

---

## Task 7: Markdown parser

**Files:**
- Create: `src/tui/markdown.rs`

- [ ] **Step 1: Write markdown parser tests**

Test cases for Discord markdown -> `Vec<Span>`:
- Plain text: `"hello"` -> one unstyled span
- Bold: `"**bold**"` -> one bold span
- Italic: `"*italic*"` -> one italic span
- Inline code: `` "`code`" `` -> one span with code style
- Strikethrough: `"~~strike~~"` -> one strikethrough span
- Mixed: `"hello **bold** world"` -> three spans
- Code block: ` ```rust\nfn main() {}\n``` ` -> code block spans
- User mention: `"<@123456>"` -> highlighted mention span
- Channel mention: `"<#123456>"` -> highlighted span
- URL: `"https://example.com"` -> link-colored span
- Spoiler: `"||spoiler||"` -> hidden span text `[spoiler]`
- Nested: `"**bold *and italic***"` -> correctly nested styles
- Emoji: `":smile:"` -> rendered as-is (shortcode)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::markdown`
Expected: FAIL

- [ ] **Step 3: Implement Discord markdown parser**

A simple single-pass parser that walks the input string and produces `Vec<Span>`. No need for a full AST - just pattern match on markers (`**`, `*`, `` ` ``, `~~`, `||`, `<@`, `<#`, `http`).

This doesn't need to be perfect - handle the common cases. Edge cases can be refined later.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tui::markdown`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/tui/markdown.rs
git commit -m "feat: add Discord markdown parser for message rendering"
```

---

## Task 8: Utils - time formatting

**Files:**
- Create: `src/utils/mod.rs`
- Create: `src/utils/time.rs`

- [ ] **Step 1: Write time formatting tests**

Test `format_timestamp` function that returns relative strings for recent times ("just now", "2m ago", "1h ago") and absolute for old ("Mar 22, 12:01").

- [ ] **Step 2: Implement and verify**

Simple function that takes a timestamp string (ISO 8601 from Discord) and returns a display string. Use `chrono` if needed or parse manually - Discord timestamps are standard ISO format.

Note: Add `chrono` to Cargo.toml if not already present.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib utils`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/utils/ Cargo.toml
git commit -m "feat: add relative timestamp formatting"
```

---

## Task 9: Discord client - gateway connection and event loop

**Files:**
- Modify: `src/discord/client.rs`
- Modify: `src/discord/events.rs`

- [ ] **Step 1: Implement gateway client**

`src/discord/client.rs` - Creates the twilight gateway shard and HTTP client, spawns a tokio task that receives gateway events, translates them to `DiscordEvent`s, and sends them on the channel.

```rust
use anyhow::Result;
use tokio::sync::mpsc;
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt};
use twilight_http::Client as HttpClient;

use super::actions::Action;
use super::events::DiscordEvent;

pub fn create_http_client(token: &str) -> HttpClient {
    HttpClient::new(token.to_string())
}

pub fn required_intents() -> Intents {
    Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MEMBERS
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT
}

pub async fn run_gateway(
    token: String,
    event_tx: mpsc::UnboundedSender<DiscordEvent>,
) -> Result<()> {
    let intents = required_intents();
    let mut shard = Shard::new(ShardId::ONE, token, intents);

    while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
        let event = match item {
            Ok(event) => event,
            Err(e) => {
                tracing::error!("gateway error: {e}");
                let _ = event_tx.send(DiscordEvent::GatewayDisconnect);
                continue;
            }
        };

        if let Some(discord_event) = translate_event(event) {
            let _ = event_tx.send(discord_event);
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Implement event translation**

Add `translate_event(Event) -> Option<DiscordEvent>` function in `events.rs` that maps twilight `Event` variants to `DiscordEvent` variants. Handle: Ready, GuildCreate, GuildDelete, ChannelCreate, ChannelUpdate, ChannelDelete, MessageCreate, MessageUpdate, MessageDelete, MemberChunk. Return `None` for events we don't care about yet.

- [ ] **Step 3: Implement action dispatch**

Add `run_action_handler` in `actions.rs`. It takes `(http: HttpClient, action_rx: UnboundedReceiver<Action>, event_tx: UnboundedSender<DiscordEvent>)`. It receives `Action`s from the mpsc channel and calls the appropriate twilight-http methods:
- `SendMessage` -> `http.create_message(channel_id).content(&content)` (response arrives via gateway `MessageCreate`, no need to send back)
- `EditMessage` -> `http.update_message(channel_id, message_id).content(&content)` (same, gateway echoes it)
- `DeleteMessage` -> `http.delete_message(channel_id, message_id)` (same)
- `FetchMessages` -> `http.channel_messages(channel_id).limit(limit)` -> send response back via `event_tx.send(DiscordEvent::MessagesLoaded { channel_id, messages })`
- `FetchGuildMembers` -> `http.guild_members(guild_id)` -> send response back via `event_tx.send(DiscordEvent::MembersLoaded { guild_id, members })`

For `SendMessage` with `reply_to`, use `.reply(message_id)` on the message builder.

The `event_tx` allows REST responses to flow through the same `DiscordEvent` channel as gateway events, so the store processes them identically.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles. No runtime test yet - that comes when we wire up the app.

- [ ] **Step 5: Commit**

```bash
git add src/discord/
git commit -m "feat: add Discord gateway client, event translation, and action dispatch"
```

---

## Task 10: App shell - main event loop

**Files:**
- Modify: `src/app.rs` (create)
- Modify: `src/main.rs`

- [ ] **Step 1: Create App struct**

`src/app.rs` - The core struct that owns the store, channels, and runs the main loop.

```rust
use std::sync::{Arc, RwLock};
use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent};
use tokio::sync::mpsc;
use std::time::Duration;

use crate::config::Config;
use crate::discord::actions::Action;
use crate::discord::events::DiscordEvent;
use crate::store::Store;
use crate::tui::terminal::{self, Tui};

pub struct App {
    store: Arc<RwLock<Store>>,
    action_tx: mpsc::UnboundedSender<Action>,
    discord_event_rx: mpsc::UnboundedReceiver<DiscordEvent>,
    config: Config,
    should_quit: bool,
}
```

- [ ] **Step 2: Implement the main loop**

The main loop uses `crossterm::event::poll` with `try_recv` to:
1. Poll crossterm events (key/mouse/resize) with a tick interval based on FPS
2. Receive `DiscordEvent`s from the gateway and apply them to the store
3. Render the TUI on each tick

```rust
impl App {
    pub async fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        let tick_rate = Duration::from_millis(1000 / self.config.ui.fps as u64);

        loop {
            // Drain any pending Discord events
            while let Ok(event) = self.discord_event_rx.try_recv() {
                let mut store = self.store.write().unwrap();
                store.process_discord_event(event);
            }

            // Render
            {
                let store = self.store.read().unwrap();
                terminal.draw(|frame| self.render(frame, &store))?;
            }

            // Poll for terminal events
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
}
```

Implement `process_discord_event` on `Store` that matches on `DiscordEvent` variants and updates the appropriate state fields.

Implement `handle_key` that uses `KeyDispatcher` to translate the key event into a `KeyAction`, then dispatches it (update UI state, or send an `Action` to the discord layer).

Implement `render` that draws the three-column layout using ratatui's `Layout::horizontal()`.

- [ ] **Step 3: Wire everything together in main.rs**

```rust
mod app;
mod auth;
mod config;
mod discord;
mod store;
mod tui;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::load()?;

    // Init tracing to file
    let data_dir = config::Config::data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let file_appender = tracing_appender::rolling::daily(&data_dir, "tiscord.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_env_filter("tiscord=debug")
        .init();

    let token = auth::get_token()?;

    // Create channels
    let (discord_event_tx, discord_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();

    // Create HTTP client and spawn action handler
    let http = discord::client::create_http_client(&token);
    tokio::spawn(discord::actions::run_action_handler(http.clone(), action_rx, discord_event_tx.clone()));

    // Spawn gateway
    tokio::spawn(discord::client::run_gateway(token, discord_event_tx));

    // Create store and app
    let store = std::sync::Arc::new(std::sync::RwLock::new(store::Store::new()));
    let mut app = app::App::new(store, action_tx, discord_event_rx, config);

    // Init terminal and run
    let mut terminal = tui::terminal::init()?;
    let result = app.run(&mut terminal).await;
    tui::terminal::restore()?;
    result
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles. At this point, running it would connect to Discord (if token is set) and show an empty TUI.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat: add app shell with main event loop connecting all layers"
```

---

## Task 11: Server list component

**Files:**
- Modify: `src/tui/components/server_list.rs`

- [ ] **Step 1: Implement ServerList component**

Renders the list of guilds from the store. Shows:
- "Direct Messages" entry at top (always present)
- Guild names, with the selected one highlighted
- Unread indicator (dot or bold) for guilds with unread messages
- Mention count badge for guilds with mentions

Handles `j`/`k` for navigation, `Enter` for selection. Selecting a guild updates `store.ui.selected_guild` and sets `dm_mode` accordingly.

Implement the `Component` trait.

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/server_list.rs
git commit -m "feat: add ServerList component with guild navigation"
```

---

## Task 12: Channel tree component

**Files:**
- Modify: `src/tui/components/channel_tree.rs`

- [ ] **Step 1: Implement ChannelTree component**

Renders channels for the selected guild, grouped by category. Shows:
- Category names as collapsible headers (bold, uppercase)
- Text channels prefixed with `#`
- Voice channels prefixed with speaker icon
- Selected channel highlighted
- Unread indicator per channel

Handles `j`/`k` navigation, `Enter` to select channel (updates `store.ui.selected_channel` and triggers `FetchMessages` action if needed).

- [ ] **Step 2: Verify it compiles and integrate into sidebar**

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/channel_tree.rs
git commit -m "feat: add ChannelTree component with category grouping"
```

---

## Task 13: Sidebar composition

**Files:**
- Modify: `src/tui/components/sidebar.rs`

- [ ] **Step 1: Implement ServerChannelSidebar**

Composes `ServerList` and `ChannelTree` (or `DMList` when in DM mode) vertically. ServerList gets a fixed height (proportional to number of guilds, capped), ChannelTree gets the rest.

Routes key events to whichever sub-component has focus.

- [ ] **Step 2: Commit**

```bash
git add src/tui/components/sidebar.rs
git commit -m "feat: add ServerChannelSidebar composing server list and channel tree"
```

---

## Task 14: Message rendering widget

**Files:**
- Modify: `src/tui/components/message.rs`

- [ ] **Step 1: Write message rendering tests**

Test that a `StoredMessage` renders correctly:
- Author name in role color, timestamp after it
- Content parsed through markdown renderer
- Reply context shown as indented quote
- Attachments shown as `[file: name (size)]`
- Edited indicator `(edited)` appended
- Correct number of lines calculated for scroll math

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::components::message`

- [ ] **Step 3: Implement MessageWidget**

A `StatelessWidget` (or function) that takes a `StoredMessage` and renders it into a given area. Uses the markdown parser for content. Calculates wrapped line count for the message (needed by MessageList for scroll).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tui::components::message`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/tui/components/message.rs
git commit -m "feat: add message rendering widget with markdown and reply context"
```

---

## Task 15: Message list component

**Files:**
- Modify: `src/tui/components/message_list.rs`

- [ ] **Step 1: Implement MessageList component**

Scrollable list of messages for the current channel. Reads from `store.messages[selected_channel]`.

Features:
- Renders messages bottom-up (newest at bottom, like Discord)
- Scroll offset tracking (keyboard `j`/`k` moves selection, `Ctrl+u`/`Ctrl+d` pages)
- Selected message highlighted with subtle background
- Auto-scrolls to bottom when new messages arrive (unless user has scrolled up)
- When scrolled to the top of the buffer, no auto-fetch yet (that's wired in Task 20)

Handles `g g` (jump to top), `G` (jump to bottom).

- [ ] **Step 2: Verify it compiles**

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/message_list.rs
git commit -m "feat: add scrollable MessageList component"
```

---

## Task 16: Message input component

**Files:**
- Modify: `src/tui/components/message_input.rs`

- [ ] **Step 1: Implement MessageInput component**

Text editor for composing messages. Features:
- Single-line by default, grows to multiline with Shift+Enter
- Cursor position tracking and movement (arrow keys, Home/End)
- Character insertion and deletion (Backspace, Delete)
- `Enter` sends the message (produces `Action::SendMessage`)
- Shows reply context above input when replying (`> replying to @user`)
- Clears input after send

The component only handles key events in Insert mode.

- [ ] **Step 2: Write tests for text editing operations**

Test insert, delete, cursor movement, newline insertion, content clearing.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib tui::components::message_input`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/tui/components/message_input.rs
git commit -m "feat: add MessageInput component with multiline text editing"
```

---

## Task 17: Channel header and message pane composition

**Files:**
- Modify: `src/tui/components/channel_header.rs`
- Modify: `src/tui/components/message_pane.rs`

- [ ] **Step 1: Implement ChannelHeader**

Simple component that renders the current channel name and topic. Shows `# channel-name` with the topic text in muted color to the right.

- [ ] **Step 2: Implement MessagePane**

Composes `ChannelHeader`, `MessageList`, and `MessageInput` vertically. Header gets 1 line, input gets 3 lines (expandable), message list gets the rest.

Routes key events to the appropriate sub-component based on focus.

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/channel_header.rs src/tui/components/message_pane.rs
git commit -m "feat: add ChannelHeader and MessagePane composition"
```

---

## Task 18: Three-column layout integration

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Wire all components into App render**

Update `App::render()` to:
1. Split the frame into three columns using `Layout::horizontal()` with percentage constraints from config
2. Render `ServerChannelSidebar` in the left column
3. Render `MessagePane` in the center column
4. Render `MemberSidebar` in the right column (if visible)
5. Draw borders between panels

- [ ] **Step 2: Wire focus management**

Update `App::handle_key()` to:
1. Use `KeyDispatcher` to get `KeyAction`
2. For focus-switching actions (`FocusSidebar`, `ToggleMemberSidebar`, etc.), update `store.ui.focus` and `store.ui.input_mode`
3. For component-specific actions, delegate to the focused component
4. `Ctrl+c` or `q` (in normal mode, not in a list) quits the app

- [ ] **Step 3: Manual test**

Run: `cargo run`
Expected: If a token is configured, the app connects to Discord and shows the three-column layout with guild list populating. Keyboard navigation switches focus between panels.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: integrate three-column layout with focus management"
```

---

## Task 19: Store event processing

**Files:**
- Modify: `src/store/mod.rs`

- [ ] **Step 1: Write event processing tests**

Test that `Store::process_discord_event()` correctly handles:
- `Ready` -> sets current_user_id, current_user_name
- `GuildCreate` -> adds guild to guild state
- `GuildDelete` -> removes guild
- `MessageCreate` -> pushes message to correct channel buffer, increments unread if not selected channel
- `MessageUpdate` -> updates message content in buffer
- `MessageDelete` -> removes message from buffer
- `MemberChunk` -> stores members (for member sidebar)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib store -- process`

- [ ] **Step 3: Implement process_discord_event**

Big match statement on `DiscordEvent` variants. For each, update the appropriate state fields. Convert twilight types to our internal types (e.g., `twilight_model::channel::Message` -> `StoredMessage`).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib store`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/store/
git commit -m "feat: implement store event processing for all Discord events"
```

---

## Task 20: DM list component

**Files:**
- Modify: `src/tui/components/dm_list.rs`
- Modify: `src/store/state.rs` (add DM state)

- [ ] **Step 1: Add DM state to store**

Add a `dm_channels: Vec<DmChannel>` field to the store. `DmChannel` has: channel_id, recipient names, last message preview, timestamp.

Process `ChannelCreate` events for DM channels (type 1 = DM, type 3 = group DM).

- [ ] **Step 2: Implement DMList component**

Renders DM conversations sorted by most recent activity. Shows recipient name(s) and last message preview. Handles `j`/`k` navigation, `Enter` to select (sets `selected_channel`).

- [ ] **Step 3: Wire DM mode into sidebar**

When "Direct Messages" is selected in server list, the sidebar shows `DMList` instead of `ChannelTree`.

- [ ] **Step 4: Commit**

```bash
git add src/tui/components/dm_list.rs src/store/
git commit -m "feat: add DM list component and DM mode in sidebar"
```

---

## Task 21: Notification tracking

**Files:**
- Modify: `src/store/notifications.rs`
- Modify: `src/store/mod.rs` (event processing)
- Modify: `src/tui/components/server_list.rs`
- Modify: `src/tui/components/channel_tree.rs`

- [ ] **Step 1: Wire notification updates into event processing**

When `MessageCreate` arrives:
- If the channel is not the currently selected channel, increment unread count
- If the message mentions the current user, increment mention count
- When the user selects a channel, mark it as read

- [ ] **Step 2: Show unread indicators in server list**

Bold guild name if any channel has unreads. Show mention count badge `(3)` if any channel has mentions.

- [ ] **Step 3: Show unread indicators in channel tree**

Bold channel name if unread. Show mention count next to channel name.

- [ ] **Step 4: Commit**

```bash
git add src/store/ src/tui/components/server_list.rs src/tui/components/channel_tree.rs
git commit -m "feat: add unread and mention notification tracking with UI indicators"
```

---

## Task 22: Member sidebar

**Files:**
- Modify: `src/tui/components/member_sidebar.rs`
- Modify: `src/store/mod.rs` (add member state)

- [ ] **Step 1: Add member state to store**

Store members per guild. Group by online status (online, idle, dnd, offline). Sort alphabetically within groups.

Request member chunks on guild select via `Action::FetchGuildMembers`.

- [ ] **Step 2: Implement MemberSidebar component**

Renders member list grouped by status:
- "ONLINE - N" header, followed by online members
- "OFFLINE - N" header, followed by offline members
- Member names colored by highest role color (if available)
- Scrollable with `j`/`k` when focused

Toggle visibility with `Ctrl+m`.

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/member_sidebar.rs src/store/
git commit -m "feat: add toggleable member sidebar with status grouping"
```

---

## Task 23: Message actions - edit, delete, reply

**Files:**
- Modify: `src/tui/components/message_list.rs`
- Modify: `src/tui/components/message_input.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Implement reply**

When `r` is pressed on a selected message in normal mode:
- Store the reply target (message id + author + content preview) in UI state
- Switch to insert mode, focus message input
- Message input shows reply context bar above the input
- On send, include `reply_to` in the `Action::SendMessage`
- `Esc` clears the reply target

- [ ] **Step 2: Implement edit**

When `e` is pressed on a selected message that belongs to the current user:
- Populate message input with the existing content
- Switch to insert mode
- On send, produce `Action::EditMessage` instead of `SendMessage`
- `Esc` cancels the edit

- [ ] **Step 3: Implement delete**

When `d` is pressed on a selected message that belongs to the current user:
- Show a confirmation prompt (render a small overlay: "Delete message? y/n")
- `y` sends `Action::DeleteMessage`
- `n` or `Esc` cancels

- [ ] **Step 4: Manual test**

Test reply, edit, and delete with a live Discord connection.

- [ ] **Step 5: Commit**

```bash
git add src/tui/components/ src/app.rs src/store/
git commit -m "feat: add message reply, edit, and delete actions"
```

---

## Task 24: Command palette

**Files:**
- Modify: `src/tui/components/overlays/command_palette.rs`
- Modify: `src/tui/components/overlays/mod.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Implement CommandPalette overlay**

Floating panel centered on screen. Features:
- Text input at top for fuzzy search query
- Results list below showing matched servers, channels, DMs
- Each result shows type icon (server/channel/DM) + name + context (server name for channels)
- Uses `fuzzy-matcher` crate for scoring
- `Enter` selects result (navigates to that server/channel)
- `Esc` closes
- `j`/`k` or arrow keys to navigate results (while in insert mode for the search input)

Data source: iterate all guilds and their channels, plus all DM conversations from the store.

- [ ] **Step 2: Wire into App**

`Ctrl+k` opens the palette. Palette renders as an overlay on top of the existing layout. When a result is selected, update `selected_guild` and `selected_channel` in the store and close the palette.

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/overlays/ src/app.rs
git commit -m "feat: add command palette with fuzzy search for servers/channels/DMs"
```

---

## Task 25: History loading and scroll

**Files:**
- Modify: `src/tui/components/message_list.rs`
- Modify: `src/store/messages.rs`

- [ ] **Step 1: Implement history fetch on scroll**

When the user scrolls to the top of the message buffer:
- Send `Action::FetchMessages { channel_id, before: oldest_message_id, limit: 50 }`
- Show a loading indicator at the top while fetching
- When messages arrive (via `DiscordEvent`), prepend them to the buffer (temporarily expanding beyond normal capacity)
- Track `is_fetching_history` flag to avoid duplicate requests

- [ ] **Step 2: Implement expanded buffer cleanup**

When the user scrolls back to the bottom (live view), discard the expanded history portion, returning the buffer to normal capacity.

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/message_list.rs src/store/messages.rs
git commit -m "feat: add history loading on scroll with buffer expansion"
```

---

## Task 26: Error display and connection status

**Files:**
- Modify: `src/app.rs`
- Modify: `src/store/state.rs`

- [ ] **Step 1: Add status bar to layout**

Add a 1-line status bar at the bottom of the screen showing:
- Connection status ("Connected", "Reconnecting...", "Disconnected")
- Current input mode ("NORMAL" / "INSERT")
- Current guild and channel name

- [ ] **Step 2: Display errors inline**

When a REST action fails (send message, edit, delete), display the error in the message pane as a styled error line (red text) that auto-dismisses after 5 seconds.

- [ ] **Step 3: Handle gateway disconnect/reconnect**

Update `store.ui.connection_status` on gateway events. The gateway shard handles reconnection automatically (twilight does this), but we need to update the status indicator.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs src/store/
git commit -m "feat: add status bar, inline error display, and connection status"
```

---

## Task 27: End-to-end integration and polish

**Files:**
- Various minor fixes across all files

- [ ] **Step 1: Full manual test cycle**

Run `cargo run` with a valid token. Verify:
1. Connects to Discord, guilds populate
2. Can navigate servers with `j`/`k`, select with `Enter`
3. Channels load for selected guild
4. Can select a channel, messages load
5. Can press `i`, type a message, send with `Enter`
6. New messages from others appear in real-time
7. Can navigate messages, reply with `r`, edit with `e`, delete with `d`
8. DM mode works (select "Direct Messages")
9. Member sidebar toggles with `Ctrl+m`
10. Command palette opens with `Ctrl+k`, fuzzy search works
11. Unread indicators show on guilds/channels
12. Status bar shows connection state and input mode
13. `Ctrl+c` quits cleanly

- [ ] **Step 2: Fix any issues found during testing**

Address bugs, rendering glitches, or keybinding issues discovered.

- [ ] **Step 3: Final build verification**

Run: `cargo build --release`
Expected: Compiles with no errors

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: complete MVP integration and polish"
```
