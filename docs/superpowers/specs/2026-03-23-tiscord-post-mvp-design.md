# Tiscord Post-MVP Features

Extends the MVP with 9 features: typing indicators, custom status, voice channel status, reactions, message pinning, search, threads, user profiles, and file/image preview. All features follow the existing three-layer architecture (Discord → Store → TUI) and Component trait pattern.

## Implementation order

Features are ordered by dependency and complexity. Each feature is self-contained and shippable independently.

1. Typing indicators (display-only, simplest gateway event)
2. Custom status (display-only, extends existing presence data)
3. Voice channel status (display-only, extends channel tree)
4. Reactions (first user interaction feature, introduces emoji picker)
5. Message pinning (similar interaction pattern to reactions, new overlay)
6. Search (new overlay, REST API integration)
7. Threads (most complex navigation change, introduces message pane stack)
8. User profiles (new popup overlay, REST API calls)
9. File/image preview (terminal capability detection, most isolated)

## Shared infrastructure: message pane navigation stack

Three features (threads, search, pinning) need to temporarily replace the message pane content and return to the previous view. Rather than implementing this three different ways, the message pane maintains a navigation stack.

Add to `UiState`:

```rust
enum PaneView {
    Channel(ChannelId),
    Thread { parent_channel: ChannelId, thread_id: ChannelId },
    SearchContext { channel_id: ChannelId, message_id: MessageId, query: String },
    PinContext { channel_id: ChannelId, message_id: MessageId },
}

// In UiState:
message_pane_stack: Vec<PaneView>,  // base channel is always at index 0
```

- Entering a thread, selecting a search result, or jumping to a pinned message pushes a `PaneView` onto the stack
- Escape pops back to the previous view
- Max depth: 2 (no nested threads-within-search)
- The message pane renders based on `stack.last()`, using the corresponding channel ID for message display
- The channel header shows breadcrumbs: `# general > 🧵 Thread Name` or `# general > Search: "query"`

This infrastructure should be built as part of feature 7 (threads) since that's the first feature that needs it, but designed to accommodate search and pin contexts from the start.

## Feature 1: Typing indicators

### Store changes

New field in Store:

```rust
typing: HashMap<ChannelId, Vec<TypingUser>>,

struct TypingUser {
    user_id: UserId,
    display_name: String,
    started_at: Instant,
}
```

### Event processing

Extends the existing `DiscordEvent::TypingStart` variant (currently `{ channel_id, user_id }`) by adding a `display_name` field: `TypingStart { channel_id, user_id, display_name }`.

The `display_name` is resolved during event translation by looking up the user in the twilight in-memory cache (`cache.user(user_id)`) or the store's member list. If the name cannot be resolved, fall back to `"Unknown User"`.

Translated from twilight's `Event::TypingStart`. On receive:

- Insert or update entry in `typing` map for the channel
- Entries expire after 10 seconds (Discord's typing timeout)
- Expiry checked lazily: on each render tick when reading typing state, filter out entries older than 10 seconds. Additionally, clean up on any new event for the same channel.
- Ignore typing events from the current user (no self-indicator)

### Display: status bar

When the focused channel has active typers, the status bar center section shows:

- 1 user: `alice is typing...`
- 2 users: `alice, bob are typing...`
- 3+ users: `several people are typing...`

Styled with the theme's dimmed/muted color to avoid visual noise.

### Display: channel tree

Channels with active typers show a `⋯` suffix: `# general ⋯`

The channel tree column width increases by 2 characters to accommodate this without truncating channel names. This is a static increase in the default layout config, not dynamic per-channel.

### New keybindings

None. Display-only feature.

## Feature 2: Custom status

### Store changes

Extend member info with custom status:

```rust
struct CustomStatus {
    emoji: Option<String>,   // unicode emoji or custom emoji name
    text: Option<String>,
}

// Add to member struct:
custom_status: Option<CustomStatus>,
```

### Event processing

Populated from `PresenceUpdate` gateway events. Discord sends custom status as an activity with `activity_type: Custom`. Extract the emoji and state text fields.

The current `DiscordEvent::PresenceUpdate` is a unit variant with no fields, and `translate_event` does not handle `Event::PresenceUpdate` (it falls through to the catch-all). This feature requires:

1. Expanding the variant to carry data: `PresenceUpdate { user_id, guild_id, status, custom_status }`
2. Adding a match arm in `translate_event` to parse the twilight `Event::PresenceUpdate` payload, extracting the custom status activity from the activities list
3. Processing the event in the store to update the member's custom status

### Display: member sidebar

Below each username in the member list, show the custom status in the theme's dimmed color:

```
alice
  🎮 Playing Rust
bob
  📚 Reading docs
charlie
```

- If only emoji: show just the emoji
- If only text: show just text
- If neither: no extra line (no vertical space wasted)
- Lines are indented 2 spaces from the username

### Display: user profile popup

Custom status shown prominently near the top of the profile, below display name. See Feature 8 for profile details.

### New keybindings

None. Display-only feature.

## Feature 3: Voice channel status

### Store changes

New field in Store:

```rust
voice_states: HashMap<ChannelId, Vec<VoiceUser>>,

struct VoiceUser {
    user_id: UserId,
    display_name: String,
    self_mute: bool,
    self_deaf: bool,
}
```

### Event processing

New `DiscordEvent` variant: `VoiceStateUpdate { channel_id, user_id, display_name, self_mute, self_deaf, left }`.

Translated from twilight's `Event::VoiceStateUpdate`:

- If `channel_id` is Some: user joined or updated state in that channel. Upsert into `voice_states`.
- If `channel_id` is None: user left voice. Remove from all channels (search by user_id).
- On `GuildCreate`, populate initial voice states from the guild's voice state list.

### Display: channel tree

Voice channels render with a speaker icon and connected user count:

```
🔊 General (3)
```

When a voice channel is selected (highlighted), it expands inline to show connected users:

```
🔊 General (3)
   alice 🔇
   bob
   charlie
```

- `🔇` indicator for self-muted or self-deafened users
- Users indented 3 spaces from the channel name
- Pressing Enter on a voice channel only toggles expand/collapse (no voice support)
- When deselected, the channel collapses back to the single-line view with count

Voice channels remain in their natural position within categories, not separated into a dedicated section. This matches Discord's desktop layout.

### New keybindings

None. Voice channels respond to existing Enter/selection keybindings.

## Feature 4: Reactions

### Store changes

Extend `StoredMessage` (in `src/store/messages.rs`) with reactions:

```rust
struct Reaction {
    emoji: ReactionEmoji,
    count: u32,
    me: bool,  // current user has reacted with this emoji
}

enum ReactionEmoji {
    Unicode(String),
    Custom { id: EmojiId, name: String },
}

// Add to StoredMessage:
reactions: Vec<Reaction>,
```

**Initial population:** Reactions must be populated when messages are first created, not just via gateway reaction events. The `StoredMessage` construction in `store/mod.rs` (for `MessageCreate` events) and in `MessagesLoaded` (REST history fetch) must extract reactions from the twilight `Message` struct, which carries a `reactions: Vec<MessageReaction>` field. Without this, existing reactions on messages loaded from history would be lost.

New field for emoji quick-react memory:

```rust
// In config, persisted to config.toml:
recent_emojis: Vec<String>,  // last 6 used emoji, most recent first
```

### Event processing

New `DiscordEvent` variants:

- `ReactionAdd { channel_id, message_id, emoji, user_id }`
- `ReactionRemove { channel_id, message_id, emoji, user_id }`
- `ReactionRemoveAll { channel_id, message_id }`

On `ReactionAdd`: find the message in the buffer, find or create the reaction entry, increment count, set `me = true` if user_id matches current user.

On `ReactionRemove`: find the reaction entry, decrement count, set `me = false` if user_id matches current user. Remove the reaction entry if count reaches 0.

On `ReactionRemoveAll`: clear all reactions on the message.

### New `Action` variants

- `AddReaction { channel_id, message_id, emoji }`
- `RemoveReaction { channel_id, message_id, emoji }`

Dispatched to the Discord layer, which calls the REST API: `PUT /channels/{id}/messages/{id}/reactions/{emoji}/@me` and `DELETE` respectively.

### Display: message rendering

Reactions render as a row below the message content:

```
alice                    2m ago
Has anyone tried the new async closures?
[👍 3] [❤️ 1] [🔥 2]
```

- Each reaction is `[emoji count]` with a space separator
- Reactions the current user has added are rendered with a highlighted background (theme accent color) or bold
- If reactions overflow the terminal width, they wrap to the next line
- Empty reaction list: no extra line rendered

### Emoji picker: three-layer interaction

**Layer 1 — Quick-react (`+` on selected message):**

A horizontal bar appears below the selected message showing the 6 most recently used emojis, plus `[search]` and `[browse]` actions:

```
👍  ❤️  😂  🔥  👀  🦀  │ search  browse
```

- Left/right arrow keys navigate between emojis and actions
- Enter selects the highlighted emoji (sends reaction immediately) or opens search/browse
- Escape cancels
- Default recent emojis (before any reactions are sent): `👍 ❤️ 😂 🔥 👀 🚀` (common Discord defaults)
- Recent emojis updated on each reaction sent, persisted to config file under `[reactions]` section

**Layer 2 — Fuzzy search (from quick-react → search):**

```
── Emoji Search ──
> thumbs│
  👍 thumbsup
  👎 thumbsdown
```

- Text input with fuzzy matching against emoji names
- Uses the existing `fuzzy-matcher` crate
- Results shown as a vertical scrollable list below the input
- Up/down navigate results, Enter selects, Escape goes back to quick-react
- Searches both Unicode emoji names and custom server emojis

**Layer 3 — Category browser (from quick-react → browse):**

```
── Emoji Browser ──
[Smileys] People  Nature  Food  Activities  Objects  Symbols  Server

😀 😃 😄 😁 😆 😅 🤣 😂 🙂 😊
😇 🥰 😍 🤩 😘 😗 😚 😙 🥲 😋
```

- Tab/Shift+Tab cycles between categories
- Arrow keys navigate the emoji grid
- Enter selects, Escape goes back to quick-react
- "Server" category shows custom emojis from the current guild
- Grid width adapts to terminal width

### Emoji data

Embed a static Unicode emoji dataset at compile time: ~1,800 entries as a const array of `(&str, &str)` tuples (name, emoji character). ~50KB compiled. This avoids runtime file loading and keeps the binary self-contained.

Custom server emojis are already available from guild data in the store (twilight caches these on `GuildCreate`).

### Removing reactions

`-` on a selected message:

- If the user has reactions on this message: shows only the emojis they've reacted with in a horizontal picker, select one to remove
- If the user has no reactions: status bar shows `No reactions to remove` for 3 seconds

### New keybindings

| Key | Context                        | Action                  |
| --- | ------------------------------ | ----------------------- |
| `+` | Message list, message selected | Open quick-react picker |
| `-` | Message list, message selected | Remove own reaction     |

## Feature 5: Message pinning

### Store changes

New field in Store:

```rust
pinned_messages: HashMap<ChannelId, Option<Vec<Message>>>,  // None = not yet fetched
```

Lazily fetched: `None` until the user opens the pins overlay for that channel.

### Event processing

New `DiscordEvent` variant: `ChannelPinsUpdate { channel_id }`.

On receive: set `pinned_messages[channel_id] = None` (invalidate cache). The next time the overlay opens, it refetches.

### New `Action` variants

- `FetchPinnedMessages { channel_id }`
- `PinMessage { channel_id, message_id }`
- `UnpinMessage { channel_id, message_id }`

REST calls:

- `GET /channels/{id}/pins` → returns pinned messages
- `PUT /channels/{id}/pins/{message_id}` → pin
- `DELETE /channels/{id}/pins/{message_id}` → unpin

### Display: channel header

Pin count shown in the channel header after the channel name:

```
# general  📌 7
```

Count is sourced from the pinned messages cache if loaded, otherwise fetched with a lightweight REST call on channel switch. If not yet loaded, show no pin count (avoid blocking the header render).

### Overlay: pins viewer (`Ctrl+p`)

Opens a scrollable overlay (same visual pattern as command palette):

```
── Pinned Messages (7) ──
  alice            3d ago
  Important: meeting at 3pm tomorrow

  bob              1w ago
  Here's the project repo link: github.com/...

  charlie          2w ago
  Remember to update your dependencies
```

- Each entry: author, relative timestamp, first line of message content (truncated to fit)
- Up/down navigate, Enter jumps to the pinned message in context (pushes `PinContext` onto the navigation stack if the message is in the current channel's buffer, otherwise fetches surrounding messages)
- Escape closes the overlay
- Loading state: show `Loading pinned messages...` on first open while fetching

### Interaction: pin/unpin

`P` (Shift+p) on a selected message (`p` is reserved for user profiles in Feature 8):

- Check permissions: requires `MANAGE_MESSAGES` permission for the current user in this channel. If not permitted, status bar shows `No permission to pin messages` for 3 seconds.
- If permitted, show confirmation in status bar: `Pin this message? (y/n)` (or `Unpin this message? (y/n)` if already pinned)
- Discord sends a system message when a message is pinned, hence the confirmation
- On confirm: dispatch `PinMessage` or `UnpinMessage` action

### New keybindings

| Key      | Context                                    | Action                       |
| -------- | ------------------------------------------ | ---------------------------- |
| `Ctrl+p` | Any (non-overlay)                          | Open pinned messages overlay |
| `P`      | FocusTarget::MessageList, msg selected     | Pin/unpin message            |

## Feature 6: Search

### Store changes

New `SearchState` in Store:

```rust
struct SearchState {
    query: String,
    scope: SearchScope,
    results: Vec<SearchResult>,
    selected: usize,
    loading: bool,
    debounce_deadline: Option<Instant>,
}

enum SearchScope {
    CurrentChannel(ChannelId),
    Server(GuildId),
}

struct SearchResult {
    message_id: MessageId,
    channel_id: ChannelId,
    channel_name: String,  // for display in server-wide results
    author_name: String,
    content_preview: String,  // first ~80 chars, with query highlighted
    timestamp: DateTime,
}
```

### New `Action` variants

- `SearchMessages { scope, query }` → REST call to Discord search API
- `NavigateToSearchResult { channel_id, message_id }` → fetches context around the message

REST endpoints:

- `GET /channels/{id}/messages/search?content={query}` (channel scope)
- `GET /guilds/{id}/messages/search?content={query}` (server scope)

### Overlay: search UI (`/`)

Opens a search overlay with an input field at the top:

```
── Search ──  [# general]  (Ctrl+/ to toggle scope)
> async closures│

  alice: Has anyone tried the new async closures?     5m
  bob: The async closures syntax is much cleaner       4m
  alice: I wrote about async closures on my blog...    2d
```

- Default scope: current channel. `Ctrl+/` toggles to server-wide. Scope indicator shown next to the title: `[# general]` or `[🏠 Server Name]`.
- Debounced search: 300ms after the last keystroke before firing the API request. This respects Discord's aggressive rate limits (~1 req/sec) while feeling responsive.
- Results shown as a scrollable list below the input
- Each result: `author: content_preview  timestamp`
- In server-wide mode, channel name prefix: `#random > author: content...`
- Query terms highlighted in results (bold or accent color)
- Up/down navigate results, Enter navigates to the result in context
- Escape closes the overlay

### Search result navigation

When the user selects a search result:

1. Push `SearchContext { channel_id, message_id, query }` onto the message pane stack
2. Fetch ~50 messages around the target message via `GET /channels/{id}/messages?around={message_id}&limit=50`
3. Render the message feed with the matched message highlighted (accent color background)
4. Escape pops back to the search overlay with results preserved
5. `n` / `N` navigate to next/previous result directly from the context view (pushing a new context, replacing the current one on the stack)

### New keybindings

| Key      | Context                                 | Action                        |
| -------- | --------------------------------------- | ----------------------------- |
| `/`      | FocusTarget::MessageList                | Open search overlay           |
| `Ctrl+/` | Search overlay                          | Toggle scope (channel/server) |
| `n`      | FocusTarget::MessageList (after search) | Next search result            |
| `N`      | FocusTarget::MessageList (after search) | Previous search result        |

## Feature 7: Threads

### Store changes

New fields in Store:

```rust
active_threads: HashMap<ChannelId, Vec<ThreadInfo>>,

struct ThreadInfo {
    id: ChannelId,       // thread's own channel ID
    name: String,
    parent_channel: ChannelId,
    message_count: u32,
    last_message_at: Option<DateTime>,
}
```

Thread messages use the existing `MessageBuffer` system — keyed by the thread's channel ID (Discord treats threads as channels internally). No separate message storage needed.

The message pane navigation stack (`message_pane_stack: Vec<PaneView>`) is introduced with this feature. See the "Shared infrastructure" section above.

### Event processing

New `DiscordEvent` variants:

- `ThreadCreate { thread_info }` → add to `active_threads` for the parent channel
- `ThreadUpdate { thread_info }` → update entry
- `ThreadDelete { thread_id, parent_channel }` → remove entry
- `ThreadListSync { guild_id, threads }` → bulk update on connect

Thread messages arrive as normal `MessageCreate` events with the thread's channel ID. The existing message processing handles them automatically — they're just messages in a different channel.

### New `Action` variants

- Reuse existing `FetchMessages { channel_id, before, limit }` with the thread's channel ID (Discord's API treats threads as channels — no separate action needed)
- `CreateThread { channel_id, message_id, name }` → `POST /channels/{id}/messages/{id}/threads`

### Display: message indicators

Messages that started a thread show a thread indicator below the content, above reactions:

```
alice                    5m ago
Has anyone tried the new async closures?
🧵 Async Discussion (12 replies)
[👍 3] [🦀 5]
```

Styled with the theme's accent color (same family as links). The reply count helps the user decide whether to open the thread.

### Thread view

When the user opens a thread (`t` on a message with a thread):

1. Push `PaneView::Thread { parent_channel, thread_id }` onto the message pane stack
2. Fetch thread messages if not already in the message buffer
3. Render the thread view:

```
# general > 🧵 Async Discussion
┌──────────────────────────────────────┐
│ alice                    5m ago      │  ← parent message (pinned at top)
│ Has anyone tried the new async       │
│ closures?                            │
├──────────────────────────────────────┤
│ bob                      4m ago      │  ← thread messages
│ Yes! The syntax is so much cleaner   │
│                                      │
│ charlie                  3m ago      │
│ How do they handle lifetimes?        │
│                                      │
│ alice                    2m ago      │
│ Same as regular closures             │
├──────────────────────────────────────┤
│ Reply to thread...                   │  ← message input sends to thread
└──────────────────────────────────────┘
```

- Channel header shows breadcrumb: `# general > 🧵 Thread Name`
- Parent message pinned at the top in a visually distinct block (bordered, accent-colored left border)
- Thread messages below, using the same message rendering as the main feed
- Message input sends to the thread channel, not the parent channel
- Escape pops back to the parent channel view

### Creating threads

`T` (Shift+t) on any message:

- Status bar input prompt: `Thread name: `
- User types thread name, Enter confirms
- Dispatch `CreateThread` action
- On success, automatically open the new thread (push onto stack)
- On failure, show error in status bar

### New keybindings

| Key | Context                                    | Action            |
| --- | ------------------------------------------ | ----------------- |
| `t` | Message list, message with thread selected | Open thread       |
| `T` | Message list, any message selected         | Create new thread |

## Feature 8: User profiles

### Store changes

New field in Store:

```rust
user_profiles: HashMap<UserId, CachedProfile>,

struct CachedProfile {
    profile: UserProfile,
    fetched_at: Instant,
}

struct UserProfile {
    user_id: UserId,
    username: String,
    display_name: Option<String>,
    avatar_url: Option<String>,
    created_at: DateTime,
    custom_status: Option<CustomStatus>,  // from Feature 2
    bot: bool,
}

// Guild-specific profile data (fetched separately):
struct GuildMemberProfile {
    roles: Vec<(String, Option<Color>)>,  // (role name, role color)
    joined_at: DateTime,
    nickname: Option<String>,
}
```

Profile cache has a 5-minute TTL. On access, if `Instant::now() - fetched_at > Duration::from_secs(300)`, refetch.

### New `Action` variants

- `FetchUserProfile { user_id }` → `GET /users/{id}`
- `FetchGuildMemberProfile { guild_id, user_id }` → `GET /guilds/{id}/members/{id}`

### Display: minimal popup (`p` on username)

A small popup (3-5 lines) anchored near the selected message or member list entry:

```
┌─────────────────────────┐
│ Alice Johnson  @alice    │
│ 🟢 Online  🎮 Playing   │
│ Admin (red)              │
│         [Enter: details] │
└─────────────────────────┘
```

- Display name + username
- Online status indicator + custom status (from Feature 2)
- Top role with role color
- Hint to expand to full view

Position: anchored to the right of the cursor position in the message list, or to the left of the cursor in the member sidebar (to avoid going off-screen).

### Display: full profile overlay (Enter from minimal popup)

Larger centered overlay (similar sizing to command palette):

```
── User Profile ──────────────────────
  Alice Johnson (@alice)
  🟢 Online
  🎮 Playing Rust

  Joined server: 2024-01-15
  Account created: 2022-06-01

  Roles: Admin, Moderator, Developer

  Bot: No
──────────────────────────────────────
```

- All fields from `UserProfile` and `GuildMemberProfile`
- If terminal supports images (detected by Feature 9's capability check), avatar rendered at top-right of the overlay
- Escape closes or shrinks back to minimal popup

### Profile trigger contexts

`p` works from two locations:

- **Message list**: profiles the author of the selected message
- **Member sidebar**: profiles the selected member

Same popup behavior, different anchor position calculation.

### New keybindings

| Key     | Context                         | Action                                  |
| ------- | ------------------------------- | --------------------------------------- |
| `p`     | Message list, message selected  | Open minimal profile for message author |
| `p`     | Member sidebar, member selected | Open minimal profile for member         |
| `Enter` | Minimal profile popup open      | Expand to full profile                  |
| `Esc`   | Profile popup open              | Close profile                           |

## Feature 9: File/image preview

### Terminal capability detection

At startup, before entering the main event loop, detect graphics protocol support:

```rust
enum GraphicsProtocol {
    None,
    Sixel,
    Kitty,
}

struct TerminalCapabilities {
    graphics: GraphicsProtocol,
}
```

Detection sequence:

1. Check `TERM_PROGRAM` env var for known terminals:
   - `WezTerm` → Sixel + Kitty
   - `kitty` → Kitty
   - `iTerm2`, `iTerm.app` → Sixel
   - `mintty` → Sixel
2. If inconclusive, send Kitty graphics query (`\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\`) and check for response
3. If no Kitty support, send Sixel device attributes query (`\x1b[c`) and check for `4` in response
4. Store result in `TerminalCapabilities` — immutable for the session

Prefer Kitty protocol over Sixel when both are available (better color support, faster rendering).

### Store changes

New image cache in Store:

```rust
image_cache: LruCache<String, CachedImage>,  // keyed by attachment URL, max 50 entries

struct CachedImage {
    protocol_data: Vec<u8>,  // pre-encoded for the detected protocol
    width: u16,              // in terminal columns
    height: u16,             // in terminal rows
}
```

LRU cache with max 50 entries. When full, oldest entries evicted.

### Image loading pipeline

When a message with image attachments is rendered and `graphics != None`:

1. Check `image_cache` for the URL
2. If cached: render inline from cache
3. If not cached: show placeholder `[loading image.png...]`
4. Dispatch async task: fetch image bytes via HTTP → decode with `image` crate → resize to fit max 40 columns wide (proportional height) → encode for detected protocol → insert into cache
5. On next render tick, the cached image is available and renders inline

Failures (network error, unsupported format, decode error) silently fall back to the link display: `[file: image.png (2.1 MB)]`. No error shown to the user — the link is always functional.

### Display: inline images

When `graphics != None` and image is cached:

```
alice                    2m ago
Check out this screenshot:
[image: screenshot.png (1.2 MB)]
┌──────────────────────────────┐
│                              │
│     (rendered image)         │
│                              │
└──────────────────────────────┘
```

- Image renders below the attachment link (link always shown for click-through)
- Max width: 40 columns (roughly half the message pane). Height proportional.
- Only image types rendered inline: PNG, JPEG, GIF (first frame), WebP
- Non-image attachments (PDF, zip, etc.) always show as links only
- Multiple images in one message render sequentially

When `graphics == None`: no change from current behavior. Just the link.

### Sixel encoding

For Sixel output: resize image → quantize to 256 colors → encode as Sixel escape sequence. Options:

- Use `sixel-rs` crate if it's maintained and lightweight
- Otherwise, implement a minimal Sixel encoder (the format is a simple RLE color-mapped encoding, ~200 lines of Rust)

Evaluate at implementation time based on crate quality.

### Kitty graphics protocol

For Kitty output: resize image → encode as PNG → base64 → wrap in Kitty escape sequences (`\x1b_Gf=100,a=T,...;{base64_data}\x1b\\`). No additional crate needed beyond `base64` (likely already in dependency tree).

### New dependencies

- `image` crate — decode (PNG, JPEG, GIF, WebP) and resize
- `base64` crate — Kitty protocol encoding (check if already transitive dependency)
- Potentially `sixel-rs` — Sixel encoding (evaluate at implementation time)
- `lru` crate — LRU cache for images

### New keybindings

None. Image rendering is automatic based on terminal capabilities.

## Configuration additions

New config sections in `~/.config/tiscord/config.toml`:

```toml
[reactions]
recent = ["👍", "❤️", "😂", "🔥", "👀", "🚀"]  # persisted quick-react emojis

[images]
enabled = true          # master toggle for inline images
max_width = 40          # max image width in terminal columns

[typing]
show_in_status = true   # typing indicator in status bar
show_in_channels = true # typing indicator in channel tree
```

## New keybindings summary

| Key       | Context                                          | Action                  | Feature   |
| --------- | ------------------------------------------------ | ----------------------- | --------- |
| `+`       | FocusTarget::MessageList, msg selected           | Open quick-react picker | Reactions |
| `-`       | FocusTarget::MessageList, msg selected           | Remove own reaction     | Reactions |
| `p`       | FocusTarget::MessageList / MemberSidebar         | Open user profile       | Profiles  |
| `Ctrl+p`  | Any (non-overlay)                                | Open pinned messages    | Pinning   |
| `P`       | FocusTarget::MessageList, msg selected           | Pin/unpin message       | Pinning   |
| `/`       | FocusTarget::MessageList                         | Open search             | Search    |
| `Ctrl+/`  | Search overlay                                   | Toggle search scope     | Search    |
| `n` / `N` | FocusTarget::MessageList (after search)          | Next/prev search result | Search    |
| `t`       | FocusTarget::MessageList, threaded msg           | Open thread             | Threads   |
| `T`       | FocusTarget::MessageList, any msg                | Create new thread       | Threads   |

## New `DiscordEvent` variants summary

Gateway events (translated from twilight gateway events):

- `TypingStart { channel_id, user_id, display_name }` (extends existing variant)
- `PresenceUpdate { user_id, guild_id, status, custom_status }` (extends existing unit variant)
- `VoiceStateUpdate { channel_id, user_id, display_name, self_mute, self_deaf, left }`
- `ReactionAdd { channel_id, message_id, emoji, user_id }`
- `ReactionRemove { channel_id, message_id, emoji, user_id }`
- `ReactionRemoveAll { channel_id, message_id }`
- `ChannelPinsUpdate { channel_id }`
- `ThreadCreate { thread_info }`
- `ThreadUpdate { thread_info }`
- `ThreadDelete { thread_id, parent_channel }`
- `ThreadListSync { guild_id, threads }`

REST response events (like existing `MessagesLoaded`, `ChannelsLoaded`):

- `SearchResults { results }`
- `PinnedMessagesLoaded { channel_id, messages }`
- `UserProfileLoaded { user_id, profile }`
- `ImageLoaded { url, cached_image }`

## New `Action` variants summary

- `AddReaction { channel_id, message_id, emoji }`
- `RemoveReaction { channel_id, message_id, emoji }`
- `FetchPinnedMessages { channel_id }`
- `PinMessage { channel_id, message_id }`
- `UnpinMessage { channel_id, message_id }`
- `SearchMessages { scope, query }`
- `NavigateToSearchResult { channel_id, message_id }`
- `CreateThread { channel_id, message_id, name }`
- `FetchUserProfile { user_id }`
- `FetchGuildMemberProfile { guild_id, user_id }`
- `FetchImage { url, channel_id, message_id }`

Note: Thread message fetching reuses the existing `FetchMessages` action with the thread's channel ID.

## New files

| File                                          | Purpose                                  |
| --------------------------------------------- | ---------------------------------------- |
| `src/store/typing.rs`                         | Typing state management                  |
| `src/store/voice.rs`                          | Voice state management                   |
| `src/store/search.rs`                         | Search state and results                 |
| `src/store/profiles.rs`                       | User profile caching                     |
| `src/store/images.rs`                         | Image cache management                   |
| `src/tui/components/overlays/emoji_picker.rs` | Three-layer emoji picker                 |
| `src/tui/components/overlays/pins.rs`         | Pinned messages overlay                  |
| `src/tui/components/overlays/profile.rs`      | User profile popup/overlay               |
| `src/tui/emoji_data.rs`                       | Embedded Unicode emoji dataset           |
| `src/tui/terminal_caps.rs`                    | Terminal capability detection            |
| `src/tui/image_renderer.rs`                   | Sixel/Kitty image encoding and rendering |

## Testing strategy

Extends the existing testing approach:

- **Unit tests for store state transitions**: typing expiry, voice state join/leave, reaction add/remove counts, search state management, profile cache TTL, image cache LRU eviction
- **Unit tests for emoji picker**: fuzzy search matching, category filtering, recent emoji ordering
- **Unit tests for navigation stack**: push/pop behavior, max depth enforcement, breadcrumb generation
- **Unit tests for terminal capability detection**: mock env vars and query responses
- **Unit tests for image encoding**: known input image → expected Sixel/Kitty output bytes
- **Integration tests**: mock Discord gateway events for typing, voice state, reactions, threads flowing through to store state
