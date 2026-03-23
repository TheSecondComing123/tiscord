# Tiscord - Discord TUI client

A full-featured Discord TUI client built with Rust, ratatui, and the twilight ecosystem. Designed as a personal daily driver.

## Tech stack

- **Language**: Rust (2024 edition)
- **TUI framework**: ratatui + crossterm
- **Discord API**: twilight-gateway, twilight-http, twilight-model, twilight-cache-inmemory
- **Async runtime**: tokio (multi-thread)
- **Serialization**: serde + serde_json
- **Config**: toml (for user config files)
- **Keyring**: keyring crate (secure token storage)

## Authentication

Supports both user tokens and bot tokens through a generic token abstraction. The primary use case is user tokens (selfbot) since this is a personal daily driver that needs DM access and sending messages as yourself.

Token is stored in the OS keyring via the `keyring` crate. First-run prompts for token input. No plaintext token storage on disk.

Note: User tokens are against Discord's ToS. This is an accepted risk for the use case.

## Architecture

Three async layers communicating over tokio mpsc channels. Data flows unidirectionally: Discord -> Store -> TUI -> Actions -> Discord.

### Discord layer

Owns the gateway WebSocket connection and HTTP client. Responsibilities:

- Connects to Discord gateway, handles heartbeat/resume/reconnect
- Receives gateway events, translates to internal `DiscordEvent` enum
- Pushes events to the store via mpsc channel
- Receives `Action`s from the TUI layer, executes them as REST calls or gateway commands
- Rate limit handling (twilight-http handles this internally)
- Uses twilight-cache-inmemory for guild/channel/user caching

### App state (store)

Central state container. Processes `DiscordEvent`s to update state. The TUI reads from this.

State includes:
- Guild list with unread counts and notification state
- Channel trees per guild (categories, text, voice, threads)
- Message history per channel (bounded ring buffer, lazily loaded)
- DM conversations list
- Current user info and presence
- UI state: selected guild, selected channel, scroll position, focus target, overlay state

The store is **not** a full ECS or reactive system. It's a plain struct behind an `Arc<RwLock<>>`. The TUI acquires a read lock on each render tick to read current state. The discord event processor acquires a write lock to apply updates. Since renders are fast (just reading fields to build widgets) and writes are infrequent (individual gateway events), contention is minimal. If profiling shows lock contention, the store can be split into independent `RwLock`s per domain (guilds, messages, UI state) without changing the component API.

Message history per channel uses a bounded ring buffer, default 500 messages. Older messages are evicted as new ones arrive. Scrolling past the buffer triggers a REST fetch for history, which temporarily expands the buffer for that channel. The expanded portion is discarded when the user scrolls back to live messages.

### TUI layer

Ratatui component tree with the official component pattern:

- `Component` trait with `handle_key_event()`, `handle_action()`, `render()`
- Components acquire a read lock on the store during `render()` to read current state
- Key events produce `Action`s sent over mpsc to the store/discord layer
- Render loop runs at configurable FPS (default 30)
- Crossterm backend for terminal I/O

### Channel architecture

```
Discord Gateway ──> DiscordEvent ──> Store (acquires write lock, updates state)
                                         │
Terminal Events ──> KeyEvent ──> TUI ─────┘ (acquires read lock on render tick)
                                  │
                                  └──> Action ──> Discord Layer (REST/gateway)
```

The TUI does NOT receive state updates via channel. It polls the store on every render tick (30 FPS) by acquiring a read lock. This is simpler than an event-driven model and avoids duplicating state.

Channels:
- `discord_event_tx/rx`: DiscordEvent from discord layer to store
- `action_tx/rx`: Action from TUI to discord layer
- `terminal_event_tx/rx`: crossterm events to TUI event loop

## TUI layout

Classic three-column layout:

```
┌──────────────┬─────────────────────────────┬──────────────┐
│ Servers      │ # channel-name              │ ONLINE - 12  │
│              │                             │              │
│ > Gaming     │ alice           12:01       │ alice        │
│   Rust       │ Hey everyone!               │ bob          │
│   Work       │                             │ charlie      │
│              │ bob             12:02       │              │
│ CHANNELS     │ What's up? Anyone working   │ OFFLINE - 5  │
│ # general    │ on the ratatui project?     │ dave         │
│ # random     │                             │ eve          │
│ # voice      │ charlie         12:03       │              │
│              │ Yeah, just pushed a commit  │              │
│              │                             │              │
│              ├─────────────────────────────┤              │
│              │ Message #general...         │              │
└──────────────┴─────────────────────────────┴──────────────┘
```

- Left panel: server list + channel tree. DM list replaces the channel tree when a "Direct Messages" entry is selected in the server list (always present at top of server list)
- Center panel: channel header, message feed, message input
- Right panel: member list (toggleable with Ctrl+m)
- Overlays: search, threads, command palette float over the layout

Panel widths are configurable. Default split: 20% / 60% / 20%. When member panel is hidden: 20% / 80%.

## Component tree

```
App
├── ServerChannelSidebar
│   ├── ServerList
│   ├── ChannelTree
│   └── DMList
├── MessagePane
│   ├── ChannelHeader
│   ├── MessageList
│   │   └── Message (renders: author, time, content, reactions, attachments)
│   └── MessageInput
├── MemberSidebar
│   └── MemberList
└── Overlays
    ├── SearchOverlay
    ├── ThreadOverlay
    └── CommandPalette
```

## Focus model and input modes

One component owns focus at a time. The focused component receives key events. All other components ignore key input but continue rendering.

The app has two input modes, like vim:

**Normal mode** (default): Single-key and chord keybindings are active. Used for navigation, selection, and commands. Active when focus is on any component except the message input.

**Insert mode**: All keypresses go to the text input as literal characters. Only Ctrl-prefixed bindings and Esc work. Active when focus is on the message input or any text field (search box, command palette input).

Transitions:
- `i` in normal mode -> insert mode (focuses message input)
- `Esc` in insert mode -> normal mode (returns focus to message list)
- `Enter` in insert mode -> sends message, stays in insert mode
- `Shift+Enter` in insert mode -> newline in message input

Focus transitions (work in both modes via Ctrl prefix):
- `Ctrl+s`: focus sidebar (enters normal mode)
- `Ctrl+m`: toggle + focus member list (enters normal mode)
- `Ctrl+k`: open command palette (enters insert mode for search input)
- `Esc`: return focus to message list / close overlay (enters normal mode)

## Keybindings

All keybindings are configurable via `~/.config/tiscord/keys.toml`. Defaults:

### Normal mode - navigation
| Key | Action |
|-----|--------|
| `Ctrl+s` | Focus server/channel sidebar |
| `Ctrl+m` | Toggle member sidebar |
| `Ctrl+k` | Command palette |
| `j` / `k` | Navigate up/down in focused list |
| `Enter` | Select item |
| `Esc` | Back / close overlay |
| `Tab` | Cycle focus forward |
| `Shift+Tab` | Cycle focus backward |

### Normal mode - messages (when message list is focused)
| Key | Action |
|-----|--------|
| `i` | Enter insert mode (focus message input) |
| `r` | Reply to selected message (enters insert mode) |
| `e` | Edit own message (enters insert mode with message content) |
| `d` | Delete own message (with confirmation) |
| `+` | Add reaction |
| `g g` | Jump to oldest loaded message |
| `G` | Jump to newest message |
| `Ctrl+u` | Page up in messages |
| `Ctrl+d` | Page down in messages |
| `y` | Yank/copy message content |
| `/` | Open search (enters insert mode for search input) |
| `n` / `N` | Next/previous search result |

### Insert mode (message input / search / command palette)

| Key | Action |
|-----|--------|
| `Enter` | Send message / execute search / select palette item |
| `Shift+Enter` | Newline in message |
| `Esc` | Exit insert mode, return to normal mode |
| `Ctrl+s/m/k` | Focus transitions still work (Ctrl prefix bypasses insert) |
| Arrow keys | Cursor movement within text input |
| `Home` / `End` | Jump to start/end of line |

Multi-key chord sequences (e.g. `g g`) use stateful prefix matching: pressing `g` enters a pending state, and the next keypress within 500ms completes or cancels the chord.

## Message rendering

Messages render with:
- Author name (colored by role color)
- Timestamp (relative for recent, absolute for old)
- Content with Discord markdown rendered:
  - **bold**, *italic*, ~~strikethrough~~, `inline code`
  - Code blocks with syntax name displayed
  - Spoilers shown as `[spoiler]` until Enter is pressed
  - Links shown inline (clickable in terminals that support OSC 8)
  - @mentions highlighted
  - Emoji shortcodes rendered as `:name:` (unicode emoji rendered natively)
- Reactions shown as `[:emoji: N]` below the message
- Attachments shown as `[file: name.ext (size)]` with URL
- Embeds shown as indented blocks with title/description
- Reply context shown as `> replying to @user: truncated message...`

## MVP scope

The MVP delivers a usable daily driver with core features. Post-MVP features are designed into the architecture but not implemented.

### MVP features
1. **Authentication** - token input, keyring storage, gateway connection
2. **Server/channel browsing** - guild list, channel tree with categories, unread indicators
3. **Message feed** - real-time messages, history loading on scroll, markdown rendering
4. **Send messages** - compose and send, multiline input, reply-to
5. **DMs** - direct message list, send/receive DMs
6. **Notifications** - unread counts per channel, mention highlighting, badge on servers with mentions
7. **Basic message actions** - edit, delete own messages
8. **Command palette** - fuzzy-find servers, channels, DMs
9. **Member sidebar** - online/offline member list, toggleable right panel

### Post-MVP features
- Threads (view/participate)
- Reactions (add/remove/view)
- Voice channel status (who's connected, not actual voice)
- Search (message search within channel/server)
- File/image preview (sixel/kitty protocol for supporting terminals, fallback to link)
- Message pinning
- User profiles/info popup
- Custom status
- Typing indicators

## Configuration

Config file at `~/.config/tiscord/config.toml`:

```toml
[ui]
fps = 30
timestamps = "relative"    # "relative", "absolute", "off"
member_sidebar = true       # show by default

[ui.layout]
sidebar_width = 20          # percentage
member_width = 20           # percentage

[notifications]
desktop = false             # desktop notifications via notify-rust
mentions_only = false       # only notify on @mentions

[keybindings]
# override any default keybinding
# focus_sidebar = "ctrl-s"
# toggle_members = "ctrl-m"
```

## Error handling

- Gateway disconnects: automatic reconnect with exponential backoff, resume when possible
- REST failures: display error inline in the message pane, don't crash
- Rate limits: twilight-http handles queuing automatically
- Invalid token: clear error message on startup, prompt to re-enter
- Network loss: show "disconnected" indicator in status bar, auto-reconnect

## Project structure

```
tiscord/
├── Cargo.toml
├── src/
│   ├── main.rs                 # entry point, tokio runtime setup
│   ├── app.rs                  # App struct, main event loop
│   ├── config.rs               # config loading and defaults
│   ├── auth.rs                 # token management, keyring
│   ├── discord/
│   │   ├── mod.rs
│   │   ├── client.rs           # gateway + http client setup
│   │   ├── events.rs           # DiscordEvent enum, gateway event translation
│   │   └── actions.rs          # Action enum, REST call dispatch
│   ├── store/
│   │   ├── mod.rs
│   │   ├── state.rs            # Store struct, all app state
│   │   ├── guilds.rs           # guild/channel state management
│   │   ├── messages.rs         # message history, ring buffers
│   │   └── notifications.rs    # unread tracking, mention counts
│   ├── tui/
│   │   ├── mod.rs
│   │   ├── terminal.rs         # terminal setup, render loop
│   │   ├── component.rs        # Component trait definition
│   │   ├── theme.rs            # colors, styles
│   │   ├── components/
│   │   │   ├── mod.rs
│   │   │   ├── sidebar.rs      # ServerChannelSidebar
│   │   │   ├── server_list.rs  # ServerList
│   │   │   ├── channel_tree.rs # ChannelTree
│   │   │   ├── dm_list.rs      # DMList
│   │   │   ├── message_pane.rs # MessagePane
│   │   │   ├── message_list.rs # MessageList
│   │   │   ├── message.rs      # single Message rendering
│   │   │   ├── message_input.rs# MessageInput
│   │   │   ├── member_sidebar.rs# MemberSidebar
│   │   │   ├── channel_header.rs# ChannelHeader
│   │   │   └── overlays/
│   │   │       ├── mod.rs
│   │   │       ├── search.rs
│   │   │       ├── thread.rs
│   │   │       └── command_palette.rs
│   │   └── markdown.rs         # Discord markdown -> ratatui spans
│   └── utils/
│       ├── mod.rs
│       └── time.rs             # relative timestamp formatting
└── tests/
    └── ...
```

## Dependencies (Cargo.toml)

Core:
- `ratatui` - TUI framework
- `crossterm` - terminal backend
- `tokio` (features: full) - async runtime
- `twilight-gateway` - Discord WebSocket
- `twilight-http` - Discord REST
- `twilight-model` - Discord types
- `twilight-cache-inmemory` - event caching
- `serde`, `serde_json` - serialization
- `toml` - config parsing
- `keyring` - OS keyring for token
- `tracing`, `tracing-subscriber` - logging to `$XDG_DATA_HOME/tiscord/tiscord.log` (never stdout)
- `dirs` - XDG config/data directories
- `unicode-width` - correct terminal column widths
- `fuzzy-matcher` - command palette fuzzy search

## Testing strategy

- Unit tests for store state transitions (given event X, state becomes Y)
- Unit tests for markdown parser (Discord markdown -> ratatui spans)
- Unit tests for message rendering (known input -> expected styled output)
- Integration tests against a mock Discord gateway (tokio test server)
- No TUI snapshot tests for MVP (ratatui's test backend can be added later)
