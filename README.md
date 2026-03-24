# tiscord

A Discord client for your terminal. Browse servers, channels, and DMs, read and send messages, react, search, and more — all from the command line.

## Install

Requires Rust 1.85+

```bash
git clone https://github.com/TheSecondComing123/tiscord.git
cd tiscord
cargo build --release
```

The binary is at `target/release/tiscord`.

## Setup

On first launch, tiscord prompts for your Discord token. It's stored securely in your OS keychain (macOS Keychain / Windows Credential Manager) with an encrypted file fallback.

To get your token:

1. Open Discord in a browser
2. Open DevTools (F12) > Network tab
3. Send a message in any channel
4. Find the request and copy the `Authorization` header value

```bash
./target/release/tiscord
```

To clear your stored token:

```bash
./target/release/tiscord --clear-token
```

## Controls

### Navigation

| Key             | Action                |
| --------------- | --------------------- |
| Arrow keys      | Move between items    |
| Enter           | Select / start typing |
| Escape          | Go back               |
| Tab / Shift+Tab | Switch panels         |
| Mouse scroll    | Scroll messages       |

### Message actions (with a message selected)

| Key    | Action              |
| ------ | ------------------- |
| Ctrl+R | Reply               |
| Ctrl+E | Edit your message   |
| Delete | Delete your message |
| Ctrl+T | Open thread         |

### Global

| Key    | Action                |
| ------ | --------------------- |
| Ctrl+K | Command palette       |
| Ctrl+F | Search messages       |
| Ctrl+P | Pinned messages       |
| Ctrl+M | Toggle member sidebar |
| Ctrl+C | Quit                  |

## Features

- Server list with guild folder grouping
- Channel tree with categories, text, voice, and announcement channels
- Direct messages
- Real-time messages via Discord gateway
- Markdown rendering (bold, italic, code blocks, spoilers)
- Message reactions with emoji picker
- Typing indicators
- Member sidebar with custom status
- Voice channel status (who's connected)
- Thread navigation
- Pinned messages
- Message search
- User profiles
- Inline image preview (Kitty terminal protocol)
- Unread and mention tracking
- Embed/link preview rendering

## Configuration

Optional config at `~/.config/tiscord/config.toml`:

```toml
[ui]
fps = 30
timestamps = "relative"  # "relative", "absolute", or "off"
member_sidebar = true

[ui.layout]
sidebar_width = 28
member_width = 24
```

## Disclaimer

tiscord uses a user account token, not a bot token. This is against Discord's Terms of Service. Use at your own risk.
