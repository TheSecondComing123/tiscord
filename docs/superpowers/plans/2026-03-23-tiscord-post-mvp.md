# Tiscord Post-MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 9 post-MVP features to the Tiscord Discord TUI client: typing indicators, custom status, voice channel status, reactions, message pinning, search, threads, user profiles, and file/image preview.

**Architecture:** Each feature follows the existing three-layer pattern: add DiscordEvent variants → process in Store → render in TUI components → handle user Actions. Features are ordered by dependency/complexity and each is independently shippable. A shared message pane navigation stack (introduced in Feature 7) supports threads, search, and pin-jump. See `docs/superpowers/specs/2026-03-23-tiscord-post-mvp-design.md` for full spec.

**Tech Stack:** Rust 2024 edition, ratatui 0.30 + crossterm 0.29, twilight 0.17, tokio, fuzzy-matcher, image crate (Feature 9), lru crate (Feature 9)

---

## File map

New files this plan creates:

| File | Responsibility |
|------|---------------|
| `src/store/typing.rs` | Typing state: HashMap of active typers per channel, expiry logic |
| `src/store/voice.rs` | Voice state: connected users per voice channel |
| `src/store/search.rs` | Search state: query, scope, results, debounce |
| `src/store/profiles.rs` | User profile cache with 5-minute TTL |
| `src/store/images.rs` | Image LRU cache management |
| `src/tui/components/overlays/emoji_picker.rs` | Three-layer emoji picker (quick-react, search, browse) |
| `src/tui/components/overlays/pins.rs` | Pinned messages overlay |
| `src/tui/components/overlays/search.rs` | Search overlay (currently exists as empty/stub) |
| `src/tui/components/overlays/profile.rs` | User profile popup and full overlay |
| `src/tui/emoji_data.rs` | Compile-time embedded Unicode emoji dataset |
| `src/tui/terminal_caps.rs` | Terminal graphics protocol detection (Sixel/Kitty) |
| `src/tui/image_renderer.rs` | Sixel/Kitty image encoding for inline display |

Existing files modified (grouped by feature):

| File | Modifications |
|------|--------------|
| `Cargo.toml` | Add `image`, `base64`, `lru` crates (Feature 9) |
| `src/discord/events.rs` | Extend DiscordEvent with ~15 new variants |
| `src/discord/actions.rs` | Extend Action with ~11 new variants, handle in run_action_handler |
| `src/store/mod.rs` | Add new fields to Store, new match arms in process_discord_event |
| `src/store/state.rs` | Add FocusTarget variants, message_pane_stack to UiState |
| `src/store/messages.rs` | Add reactions field to StoredMessage |
| `src/store/guilds.rs` | Add thread_id to ChannelInfo for thread indicator |
| `src/config.rs` | Add [reactions], [images], [typing] config sections |
| `src/app.rs` | Route new keybindings, integrate new overlays |
| `src/tui/components/message.rs` | Render reactions, thread indicators, inline images |
| `src/tui/components/message_list.rs` | Handle +/-/p/P/t/T keybindings |
| `src/tui/components/channel_tree.rs` | Typing indicator suffix, voice channel expand/collapse |
| `src/tui/components/channel_header.rs` | Pin count, thread breadcrumbs |
| `src/tui/components/member_sidebar.rs` | Custom status display, profile popup trigger |
| `src/tui/components/message_pane.rs` | Navigation stack rendering |
| `src/main.rs` | Terminal capability detection at startup |

---

## Feature 1: Typing Indicators

### Task 1: Typing state management

**Files:**
- Create: `src/store/typing.rs`
- Modify: `src/store/mod.rs:14-42` (Store struct)
- Test: `src/store/typing.rs` (inline tests)

- [ ] **Step 1: Write failing tests for typing state**

In `src/store/typing.rs`, create the module with types and tests:

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use twilight_model::id::marker::{ChannelMarker, UserMarker};
use twilight_model::id::Id;

const TYPING_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TypingUser {
    pub user_id: Id<UserMarker>,
    pub display_name: String,
    pub started_at: Instant,
}

#[derive(Debug, Default)]
pub struct TypingState {
    channels: HashMap<Id<ChannelMarker>, Vec<TypingUser>>,
}

impl TypingState {
    pub fn add_typing(&mut self, channel_id: Id<ChannelMarker>, user_id: Id<UserMarker>, display_name: String) {
        // TODO: implement
        todo!()
    }

    pub fn get_typers(&self, channel_id: Id<ChannelMarker>) -> Vec<&TypingUser> {
        // TODO: implement
        todo!()
    }

    pub fn has_typers(&self, channel_id: Id<ChannelMarker>) -> bool {
        // TODO: implement
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel_id(n: u64) -> Id<ChannelMarker> { Id::new(n) }
    fn make_user_id(n: u64) -> Id<UserMarker> { Id::new(n) }

    #[test]
    fn test_add_typing_user() {
        let mut state = TypingState::default();
        state.add_typing(make_channel_id(1), make_user_id(100), "alice".into());
        assert_eq!(state.get_typers(make_channel_id(1)).len(), 1);
        assert_eq!(state.get_typers(make_channel_id(1))[0].display_name, "alice");
    }

    #[test]
    fn test_duplicate_user_updates_timestamp() {
        let mut state = TypingState::default();
        state.add_typing(make_channel_id(1), make_user_id(100), "alice".into());
        state.add_typing(make_channel_id(1), make_user_id(100), "alice".into());
        assert_eq!(state.get_typers(make_channel_id(1)).len(), 1);
    }

    #[test]
    fn test_expired_users_filtered() {
        let mut state = TypingState::default();
        let channel = make_channel_id(1);
        // Manually insert an expired entry
        state.channels.entry(channel).or_default().push(TypingUser {
            user_id: make_user_id(100),
            display_name: "alice".into(),
            started_at: Instant::now() - Duration::from_secs(15),
        });
        assert_eq!(state.get_typers(channel).len(), 0);
    }

    #[test]
    fn test_no_typers_returns_empty() {
        let state = TypingState::default();
        assert!(state.get_typers(make_channel_id(999)).is_empty());
        assert!(!state.has_typers(make_channel_id(999)));
    }

    #[test]
    fn test_has_typers() {
        let mut state = TypingState::default();
        state.add_typing(make_channel_id(1), make_user_id(100), "alice".into());
        assert!(state.has_typers(make_channel_id(1)));
        assert!(!state.has_typers(make_channel_id(2)));
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib store::typing -- --nocapture`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement TypingState methods**

Replace the `todo!()` bodies:

```rust
impl TypingState {
    pub fn add_typing(&mut self, channel_id: Id<ChannelMarker>, user_id: Id<UserMarker>, display_name: String) {
        let typers = self.channels.entry(channel_id).or_default();
        if let Some(existing) = typers.iter_mut().find(|t| t.user_id == user_id) {
            existing.started_at = Instant::now();
        } else {
            typers.push(TypingUser {
                user_id,
                display_name,
                started_at: Instant::now(),
            });
        }
    }

    pub fn get_typers(&self, channel_id: Id<ChannelMarker>) -> Vec<&TypingUser> {
        let now = Instant::now();
        self.channels
            .get(&channel_id)
            .map(|typers| {
                typers.iter().filter(|t| now.duration_since(t.started_at) < TYPING_TIMEOUT).collect()
            })
            .unwrap_or_default()
    }

    pub fn has_typers(&self, channel_id: Id<ChannelMarker>) -> bool {
        !self.get_typers(channel_id).is_empty()
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test --lib store::typing -- --nocapture`
Expected: All 5 tests PASS

- [ ] **Step 5: Wire typing state into Store**

In `src/store/mod.rs`:
- Add `pub mod typing;` to module declarations
- Add `use typing::TypingState;` import
- Add field `pub typing: TypingState` to Store struct (after `dm_channels` at line 42)
- Initialize with `typing: TypingState::default()` in Store::new/default

- [ ] **Step 6: Extend DiscordEvent and translate_event**

In `src/discord/events.rs`:
- Modify existing `TypingStart` variant (line 25-28) to add `display_name: String` field:
  ```rust
  TypingStart {
      channel_id: Id<ChannelMarker>,
      user_id: Id<UserMarker>,
      display_name: String,
  },
  ```
- In `translate_event` function, add a match arm for `Event::TypingStart(e)`:
  ```rust
  Event::TypingStart(e) => Some(DiscordEvent::TypingStart {
      channel_id: e.channel_id,
      user_id: e.user_id,
      display_name: String::new(), // resolved in store from cache
  }),
  ```

- [ ] **Step 7: Process TypingStart in store**

In `src/store/mod.rs`, replace the `TypingStart { .. } => {}` no-op arm (line 308) with:
```rust
DiscordEvent::TypingStart { channel_id, user_id, display_name } => {
    if Some(user_id) != self.current_user_id {
        let name = if display_name.is_empty() {
            // Try to resolve from members
            self.members.values()
                .flat_map(|members| members.iter())
                .find(|m| m.id == user_id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| format!("User {}", user_id))
        } else {
            display_name
        };
        self.typing.add_typing(channel_id, user_id, name);
    }
}
```

- [ ] **Step 8: Render typing indicator in status bar**

In `src/app.rs`, in the `render_status_bar` function (line 310), add typing display logic. After the existing center section rendering, check for active typers:

```rust
// In the center section of the status bar:
if let Some(channel_id) = store.ui.selected_channel {
    let typers = store.typing.get_typers(channel_id);
    if !typers.is_empty() {
        let typing_text = match typers.len() {
            1 => format!("{} is typing...", typers[0].display_name),
            2 => format!("{}, {} are typing...", typers[0].display_name, typers[1].display_name),
            _ => "several people are typing...".to_string(),
        };
        // Render typing_text in muted style below the channel info
    }
}
```

- [ ] **Step 9: Render typing indicator in channel tree**

In `src/tui/components/channel_tree.rs`, in the `render` method (line 90), when rendering each channel line, check `store.typing.has_typers(channel.id)` and append ` ⋯` to the channel name span if true.

- [ ] **Step 10: Commit**

```bash
git add src/store/typing.rs src/store/mod.rs src/discord/events.rs src/app.rs src/tui/components/channel_tree.rs
git commit -m "feat: add typing indicators in status bar and channel tree"
```

---

## Feature 2: Custom Status

### Task 2: Custom status display

**Files:**
- Modify: `src/store/mod.rs:14-18` (MemberInfo struct)
- Modify: `src/discord/events.rs` (PresenceUpdate variant)
- Modify: `src/tui/components/member_sidebar.rs:47-133` (render method)
- Test: `src/store/mod.rs` (inline tests)

- [ ] **Step 1: Add CustomStatus type and extend MemberInfo**

In `src/store/mod.rs`, add above the MemberInfo struct:
```rust
#[derive(Debug, Clone)]
pub struct CustomStatus {
    pub emoji: Option<String>,
    pub text: Option<String>,
}
```

Add to `MemberInfo` struct (after `status` field at line 17):
```rust
pub custom_status: Option<CustomStatus>,
```

Update all places that construct `MemberInfo` to include `custom_status: None`.

- [ ] **Step 2: Extend PresenceUpdate event**

In `src/discord/events.rs`, change the unit variant (line 29):
```rust
PresenceUpdate {
    user_id: Id<UserMarker>,
    guild_id: Id<GuildMarker>,
    custom_status: Option<crate::store::CustomStatus>,
},
```

In `translate_event`, add a match arm for `Event::PresenceUpdate`:
```rust
Event::PresenceUpdate(e) => {
    let custom_status = e.presence.activities.iter()
        .find(|a| a.kind == twilight_model::gateway::presence::ActivityType::Custom)
        .map(|a| crate::store::CustomStatus {
            emoji: a.emoji.as_ref().map(|e| e.name.clone().unwrap_or_default()),
            text: a.state.clone(),
        });
    Some(DiscordEvent::PresenceUpdate {
        user_id: e.presence.user.id(),
        guild_id: e.guild_id,
        custom_status,
    })
},
```

- [ ] **Step 3: Process PresenceUpdate in store**

In `src/store/mod.rs`, replace the `PresenceUpdate => {}` no-op arm (line 309) with:
```rust
DiscordEvent::PresenceUpdate { user_id, guild_id, custom_status } => {
    if let Some(members) = self.members.get_mut(&guild_id) {
        if let Some(member) = members.iter_mut().find(|m| m.id == user_id) {
            member.custom_status = custom_status;
        }
    }
}
```

- [ ] **Step 4: Render custom status in member sidebar**

In `src/tui/components/member_sidebar.rs`, in the `render` method, after rendering each member's name, check for `custom_status` and render it on the next line in dimmed style:

```rust
// After the member name line:
if let Some(ref status) = member.custom_status {
    let status_text = match (&status.emoji, &status.text) {
        (Some(e), Some(t)) => format!("  {} {}", e, t),
        (Some(e), None) => format!("  {}", e),
        (None, Some(t)) => format!("  {}", t),
        (None, None) => continue, // skip
    };
    // Render status_text in theme dimmed color
}
```

- [ ] **Step 5: Commit**

```bash
git add src/store/mod.rs src/discord/events.rs src/tui/components/member_sidebar.rs
git commit -m "feat: display custom status in member sidebar"
```

---

## Feature 3: Voice Channel Status

### Task 3: Voice state management and display

**Files:**
- Create: `src/store/voice.rs`
- Modify: `src/store/mod.rs` (Store struct, process_discord_event)
- Modify: `src/discord/events.rs` (new VoiceStateUpdate variant)
- Modify: `src/tui/components/channel_tree.rs` (voice channel rendering)
- Test: `src/store/voice.rs` (inline tests)

- [ ] **Step 1: Write failing tests for voice state**

Create `src/store/voice.rs` with types and tests:

```rust
use std::collections::HashMap;
use twilight_model::id::marker::{ChannelMarker, UserMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub struct VoiceUser {
    pub user_id: Id<UserMarker>,
    pub display_name: String,
    pub self_mute: bool,
    pub self_deaf: bool,
}

#[derive(Debug, Default)]
pub struct VoiceState {
    channels: HashMap<Id<ChannelMarker>, Vec<VoiceUser>>,
}

impl VoiceState {
    pub fn user_joined(&mut self, channel_id: Id<ChannelMarker>, user: VoiceUser) {
        todo!()
    }

    pub fn user_left(&mut self, user_id: Id<UserMarker>) {
        todo!()
    }

    pub fn get_users(&self, channel_id: Id<ChannelMarker>) -> &[VoiceUser] {
        todo!()
    }

    pub fn user_count(&self, channel_id: Id<ChannelMarker>) -> usize {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uid(n: u64) -> Id<UserMarker> { Id::new(n) }
    fn cid(n: u64) -> Id<ChannelMarker> { Id::new(n) }

    #[test]
    fn test_user_join_and_count() {
        let mut vs = VoiceState::default();
        vs.user_joined(cid(1), VoiceUser { user_id: uid(100), display_name: "alice".into(), self_mute: false, self_deaf: false });
        assert_eq!(vs.user_count(cid(1)), 1);
        assert_eq!(vs.get_users(cid(1))[0].display_name, "alice");
    }

    #[test]
    fn test_user_leave() {
        let mut vs = VoiceState::default();
        vs.user_joined(cid(1), VoiceUser { user_id: uid(100), display_name: "alice".into(), self_mute: false, self_deaf: false });
        vs.user_left(uid(100));
        assert_eq!(vs.user_count(cid(1)), 0);
    }

    #[test]
    fn test_user_switch_channel() {
        let mut vs = VoiceState::default();
        vs.user_joined(cid(1), VoiceUser { user_id: uid(100), display_name: "alice".into(), self_mute: false, self_deaf: false });
        vs.user_joined(cid(2), VoiceUser { user_id: uid(100), display_name: "alice".into(), self_mute: false, self_deaf: false });
        assert_eq!(vs.user_count(cid(1)), 0);
        assert_eq!(vs.user_count(cid(2)), 1);
    }

    #[test]
    fn test_empty_channel() {
        let vs = VoiceState::default();
        assert_eq!(vs.user_count(cid(999)), 0);
        assert!(vs.get_users(cid(999)).is_empty());
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib store::voice -- --nocapture`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement VoiceState methods**

```rust
impl VoiceState {
    pub fn user_joined(&mut self, channel_id: Id<ChannelMarker>, user: VoiceUser) {
        // Remove from any existing channel first (channel switch)
        self.user_left(user.user_id);
        self.channels.entry(channel_id).or_default().push(user);
    }

    pub fn user_left(&mut self, user_id: Id<UserMarker>) {
        for users in self.channels.values_mut() {
            users.retain(|u| u.user_id != user_id);
        }
    }

    pub fn get_users(&self, channel_id: Id<ChannelMarker>) -> &[VoiceUser] {
        self.channels.get(&channel_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn user_count(&self, channel_id: Id<ChannelMarker>) -> usize {
        self.get_users(channel_id).len()
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test --lib store::voice -- --nocapture`
Expected: All 4 tests PASS

- [ ] **Step 5: Wire into Store, events, and channel tree**

1. In `src/store/mod.rs`: add `pub mod voice;`, add `pub voice: voice::VoiceState` to Store, init with `voice: voice::VoiceState::default()`
2. In `src/discord/events.rs`: add `VoiceStateUpdate` variant with `channel_id: Option<Id<ChannelMarker>>, user_id, display_name, self_mute, self_deaf`. Add match arm in `translate_event` for `Event::VoiceStateUpdate`.
3. In `src/store/mod.rs` `process_discord_event`: handle `VoiceStateUpdate` — if channel_id is Some, call `self.voice.user_joined(...)`, if None call `self.voice.user_left(user_id)`.
4. In `src/tui/components/channel_tree.rs` render: for `ChannelKind::Voice` channels, render as `🔊 {name} ({count})`. When selected, expand to show connected users with mute indicators below.

- [ ] **Step 6: Commit**

```bash
git add src/store/voice.rs src/store/mod.rs src/discord/events.rs src/tui/components/channel_tree.rs
git commit -m "feat: show voice channel status with connected users"
```

---

## Feature 4: Reactions

### Task 4a: Reaction types and StoredMessage extension

**Files:**
- Modify: `src/store/messages.rs:5-15` (StoredMessage)
- Modify: `src/store/mod.rs` (MessageCreate handling, lines 166-206)
- Test: `src/store/messages.rs` (inline tests)

- [ ] **Step 1: Add Reaction types to messages.rs**

At the top of `src/store/messages.rs`, add:
```rust
#[derive(Debug, Clone)]
pub enum ReactionEmoji {
    Unicode(String),
    Custom { id: u64, name: String },
}

#[derive(Debug, Clone)]
pub struct Reaction {
    pub emoji: ReactionEmoji,
    pub count: u32,
    pub me: bool,
}
```

Add `pub reactions: Vec<Reaction>` to `StoredMessage` (after `is_edited` at line 14).

- [ ] **Step 2: Update StoredMessage construction in store**

In `src/store/mod.rs`, wherever `StoredMessage` is constructed (in `MessageCreate` handler around line 182 and `MessagesLoaded` handler around line 260), add `reactions` field. Parse from twilight's `Message::reactions` field:

```rust
reactions: msg.reactions.iter().map(|r| {
    crate::store::messages::Reaction {
        emoji: match &r.emoji {
            twilight_model::channel::message::ReactionType::Unicode { name } =>
                crate::store::messages::ReactionEmoji::Unicode(name.clone()),
            twilight_model::channel::message::ReactionType::Custom { id, name, .. } =>
                crate::store::messages::ReactionEmoji::Custom { id: id.get(), name: name.clone().unwrap_or_default() },
        },
        count: r.count,
        me: r.me,
    }
}).collect(),
```

- [ ] **Step 3: Commit types**

```bash
git add src/store/messages.rs src/store/mod.rs
git commit -m "feat: add reaction types to StoredMessage"
```

### Task 4b: Reaction event processing

**Files:**
- Modify: `src/discord/events.rs` (new variants)
- Modify: `src/discord/actions.rs` (new action variants)
- Modify: `src/store/mod.rs` (event handling)
- Modify: `src/store/messages.rs` (reaction update methods)
- Test: `src/store/messages.rs` (inline tests)

- [ ] **Step 1: Write tests for reaction add/remove on MessageBuffer**

In `src/store/messages.rs`, add helper methods and tests:
```rust
impl MessageBuffer {
    pub fn add_reaction(&mut self, message_id: Id<MessageMarker>, emoji: ReactionEmoji, user_is_self: bool) {
        todo!()
    }

    pub fn remove_reaction(&mut self, message_id: Id<MessageMarker>, emoji: &ReactionEmoji, user_is_self: bool) {
        todo!()
    }

    pub fn remove_all_reactions(&mut self, message_id: Id<MessageMarker>) {
        todo!()
    }
}
```

Tests:
```rust
#[test]
fn test_add_reaction_new_emoji() {
    let mut buf = MessageBuffer::new(100);
    // push a message, then add_reaction
    // assert reactions vec has 1 entry with count 1
}

#[test]
fn test_add_reaction_existing_emoji_increments() {
    // add same emoji twice, assert count == 2
}

#[test]
fn test_remove_reaction_decrements() {
    // add then remove, assert count == 0 and entry removed
}

#[test]
fn test_remove_all_reactions() {
    // add multiple reactions, remove all, assert empty
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib store::messages -- --nocapture`

- [ ] **Step 3: Implement reaction methods**

```rust
impl MessageBuffer {
    pub fn add_reaction(&mut self, message_id: Id<MessageMarker>, emoji: ReactionEmoji, user_is_self: bool) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            if let Some(existing) = msg.reactions.iter_mut().find(|r| reaction_emoji_eq(&r.emoji, &emoji)) {
                existing.count += 1;
                if user_is_self { existing.me = true; }
            } else {
                msg.reactions.push(Reaction { emoji, count: 1, me: user_is_self });
            }
        }
    }

    pub fn remove_reaction(&mut self, message_id: Id<MessageMarker>, emoji: &ReactionEmoji, user_is_self: bool) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            if let Some(existing) = msg.reactions.iter_mut().find(|r| reaction_emoji_eq(&r.emoji, emoji)) {
                existing.count = existing.count.saturating_sub(1);
                if user_is_self { existing.me = false; }
            }
            msg.reactions.retain(|r| r.count > 0);
        }
    }

    pub fn remove_all_reactions(&mut self, message_id: Id<MessageMarker>) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            msg.reactions.clear();
        }
    }
}

fn reaction_emoji_eq(a: &ReactionEmoji, b: &ReactionEmoji) -> bool {
    match (a, b) {
        (ReactionEmoji::Unicode(a), ReactionEmoji::Unicode(b)) => a == b,
        (ReactionEmoji::Custom { id: a, .. }, ReactionEmoji::Custom { id: b, .. }) => a == b,
        _ => false,
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test --lib store::messages -- --nocapture`

- [ ] **Step 5: Add event variants and action variants**

In `src/discord/events.rs`, add:
```rust
ReactionAdd { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>, emoji: crate::store::messages::ReactionEmoji, user_id: Id<UserMarker> },
ReactionRemove { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>, emoji: crate::store::messages::ReactionEmoji, user_id: Id<UserMarker> },
ReactionRemoveAll { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker> },
```

Add match arms in `translate_event` for `Event::ReactionAdd`, `Event::ReactionRemove`, `Event::ReactionRemoveAll`.

In `src/discord/actions.rs`, add:
```rust
AddReaction { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>, emoji: String },
RemoveReaction { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>, emoji: String },
```

Handle in `run_action_handler` with twilight-http calls: `client.create_reaction(channel_id, message_id, &emoji_request_data)` and `client.delete_current_user_reaction(...)`.

- [ ] **Step 6: Process reaction events in store**

In `src/store/mod.rs` `process_discord_event`, add match arms:
```rust
DiscordEvent::ReactionAdd { channel_id, message_id, emoji, user_id } => {
    let is_self = Some(user_id) == self.current_user_id;
    if let Some(buf) = self.messages.get_mut(&channel_id) {
        buf.add_reaction(message_id, emoji, is_self);
    }
}
DiscordEvent::ReactionRemove { channel_id, message_id, emoji, user_id } => {
    let is_self = Some(user_id) == self.current_user_id;
    if let Some(buf) = self.messages.get_mut(&channel_id) {
        buf.remove_reaction(message_id, &emoji, is_self);
    }
}
DiscordEvent::ReactionRemoveAll { channel_id, message_id } => {
    if let Some(buf) = self.messages.get_mut(&channel_id) {
        buf.remove_all_reactions(message_id);
    }
}
```

- [ ] **Step 7: Commit event processing**

```bash
git add src/discord/events.rs src/discord/actions.rs src/store/mod.rs src/store/messages.rs
git commit -m "feat: process reaction add/remove events"
```

### Task 4c: Render reactions on messages

**Files:**
- Modify: `src/tui/components/message.rs` (render_message function)

- [ ] **Step 1: Add reaction rendering to message.rs**

In `src/tui/components/message.rs`, in `render_message` (line 11), after attachments rendering (line 60), add reaction row rendering:

```rust
// Render reactions
if !msg.reactions.is_empty() {
    let reaction_spans: Vec<Span> = msg.reactions.iter().flat_map(|r| {
        let emoji_str = match &r.emoji {
            ReactionEmoji::Unicode(e) => e.clone(),
            ReactionEmoji::Custom { name, .. } => format!(":{}:", name),
        };
        let style = if r.me {
            Style::default().fg(theme::ACCENT).bold()
        } else {
            Style::default().fg(theme::TEXT_MUTED)
        };
        vec![
            Span::styled(format!("[{} {}]", emoji_str, r.count), style),
            Span::raw(" "),
        ]
    }).collect();
    lines.push(Line::from(reaction_spans));
}
```

- [ ] **Step 2: Verify compilation and visual test**

Run: `cargo build`
Expected: Compiles. Reactions now display below messages.

- [ ] **Step 3: Commit**

```bash
git add src/tui/components/message.rs
git commit -m "feat: render reactions below messages"
```

### Task 4d: Emoji picker

**Files:**
- Create: `src/tui/emoji_data.rs`
- Create: `src/tui/components/overlays/emoji_picker.rs`
- Modify: `src/tui/components/message_list.rs` (keybinding for +/-)
- Modify: `src/app.rs` (integrate emoji picker)
- Modify: `src/config.rs` (recent_emojis config)

- [ ] **Step 1: Create emoji dataset**

Create `src/tui/emoji_data.rs` with a compile-time embedded array. Include the most common ~500 emojis (trimmed from full Unicode list for reasonable binary size):

```rust
pub const EMOJI_DATA: &[(&str, &str)] = &[
    ("grinning", "😀"),
    ("smiley", "😃"),
    ("smile", "😄"),
    // ... ~500 entries covering all major categories
    ("thumbsup", "👍"),
    ("thumbsdown", "👎"),
    ("heart", "❤️"),
    ("fire", "🔥"),
    ("rocket", "🚀"),
    ("eyes", "👀"),
    // etc.
];

pub const EMOJI_CATEGORIES: &[(&str, &[&str])] = &[
    ("Smileys", &["grinning", "smiley", "smile", /* ... */]),
    ("People", &[/* ... */]),
    ("Nature", &[/* ... */]),
    ("Food", &[/* ... */]),
    ("Activities", &[/* ... */]),
    ("Travel", &[/* ... */]),
    ("Objects", &[/* ... */]),
    ("Symbols", &[/* ... */]),
    ("Flags", &[/* ... */]),
];
```

Note: The actual emoji list should be generated from Unicode CLDR data at implementation time. A script or build.rs can generate this from the Unicode emoji data files.

- [ ] **Step 2: Add [reactions] config section**

In `src/config.rs`, add:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionsConfig {
    #[serde(default = "default_recent_emojis")]
    pub recent: Vec<String>,
}

fn default_recent_emojis() -> Vec<String> {
    vec!["👍".into(), "❤️".into(), "😂".into(), "🔥".into(), "👀".into(), "🚀".into()]
}

impl Default for ReactionsConfig {
    fn default() -> Self {
        Self { recent: default_recent_emojis() }
    }
}
```

Add `pub reactions: ReactionsConfig` to the `Config` struct.

- [ ] **Step 3: Create emoji picker overlay**

Create `src/tui/components/overlays/emoji_picker.rs`. Follow the same pattern as `command_palette.rs`:

```rust
pub enum PickerMode {
    QuickReact,
    Search,
    Browse,
}

pub struct EmojiPicker {
    mode: PickerMode,
    visible: bool,
    quick_react_index: usize,
    search_query: String,
    search_results: Vec<(String, String)>, // (name, emoji)
    search_selected: usize,
    browse_category: usize,
    browse_index: usize,
    recent_emojis: Vec<String>,
    pending_channel_id: Option<Id<ChannelMarker>>,
    pending_message_id: Option<Id<MessageMarker>>,
}
```

Implement `Component` trait:
- `handle_key_event`: Route based on `mode`. QuickReact handles Left/Right/Enter/Esc. Search handles text input, Up/Down, Enter, Esc (back to QuickReact). Browse handles arrow keys, Tab, Enter, Esc.
- `render`: Draw the appropriate view based on `mode`. QuickReact as horizontal bar, Search as input+list, Browse as category tabs+grid.

On emoji selection, dispatch `Action::AddReaction { channel_id, message_id, emoji }`.

- [ ] **Step 4: Wire emoji picker into app**

In `src/app.rs`:
- Add `emoji_picker: EmojiPicker` field to App
- In `handle_key`, when `FocusTarget::MessageList` is active and `+` is pressed: open emoji picker with the selected message's ID
- When `-` is pressed: show removal picker (filtered to user's own reactions)
- Route key events to emoji picker when it's visible

- [ ] **Step 5: Commit**

```bash
git add src/tui/emoji_data.rs src/tui/components/overlays/emoji_picker.rs src/config.rs src/app.rs src/tui/components/message_list.rs
git commit -m "feat: add three-layer emoji picker for reactions"
```

---

## Feature 5: Message Pinning

### Task 5: Pin overlay and pin/unpin action

**Files:**
- Create: `src/tui/components/overlays/pins.rs`
- Modify: `src/discord/events.rs` (ChannelPinsUpdate, PinnedMessagesLoaded)
- Modify: `src/discord/actions.rs` (FetchPinnedMessages, PinMessage, UnpinMessage)
- Modify: `src/store/mod.rs` (pinned_messages field, event processing)
- Modify: `src/tui/components/channel_header.rs` (pin count)
- Modify: `src/tui/components/message_list.rs` (P keybinding)
- Modify: `src/app.rs` (Ctrl+P routing, pins overlay)

- [ ] **Step 1: Add pinned_messages field to Store**

In `src/store/mod.rs`, add to Store struct:
```rust
pub pinned_messages: HashMap<Id<ChannelMarker>, Option<Vec<StoredMessage>>>,
```

- [ ] **Step 2: Add event and action variants**

Events: `ChannelPinsUpdate { channel_id }`, `PinnedMessagesLoaded { channel_id, messages: Vec<StoredMessage> }`

Actions: `FetchPinnedMessages { channel_id }`, `PinMessage { channel_id, message_id }`, `UnpinMessage { channel_id, message_id }`

Handle in `run_action_handler`:
- `FetchPinnedMessages`: call `client.pins(channel_id)`, translate response to `PinnedMessagesLoaded` event
- `PinMessage`: call `client.create_pin(channel_id, message_id)`
- `UnpinMessage`: call `client.delete_pin(channel_id, message_id)`

- [ ] **Step 3: Process events in store**

```rust
DiscordEvent::ChannelPinsUpdate { channel_id } => {
    self.pinned_messages.insert(channel_id, None); // invalidate cache
}
DiscordEvent::PinnedMessagesLoaded { channel_id, messages } => {
    self.pinned_messages.insert(channel_id, Some(messages));
}
```

- [ ] **Step 4: Create pins overlay**

Create `src/tui/components/overlays/pins.rs` following command_palette.rs pattern:

```rust
pub struct PinsOverlay {
    visible: bool,
    selected_index: usize,
    loading: bool,
}
```

Implement `Component`:
- `render`: Show "Pinned Messages (N)" header, list of pinned messages with author/timestamp/preview. Loading state when first opened.
- `handle_key_event`: Up/Down navigate, Enter jumps to message (later: pushes PinContext onto nav stack), Esc closes.

- [ ] **Step 5: Add pin count to channel header**

In `src/tui/components/channel_header.rs`, in `render`, check `store.pinned_messages.get(&channel_id)`:
- If `Some(Some(msgs))`: show `📌 {msgs.len()}`
- If `None` or `Some(None)`: show nothing

- [ ] **Step 6: Add P keybinding for pin/unpin**

In `src/tui/components/message_list.rs`, in `handle_key_event`, add:
- `KeyCode::Char('P')`: trigger pin/unpin with confirmation in status bar

In `src/app.rs`, add `Ctrl+P` global shortcut to open pins overlay.

- [ ] **Step 7: Commit**

```bash
git add src/tui/components/overlays/pins.rs src/discord/events.rs src/discord/actions.rs src/store/mod.rs src/tui/components/channel_header.rs src/tui/components/message_list.rs src/app.rs
git commit -m "feat: add pinned messages overlay and pin/unpin action"
```

---

## Feature 6: Search

### Task 6: Search overlay and Discord search API

**Files:**
- Create: `src/store/search.rs`
- Modify/Rewrite: `src/tui/components/overlays/search.rs`
- Modify: `src/discord/events.rs` (SearchResults)
- Modify: `src/discord/actions.rs` (SearchMessages)
- Modify: `src/store/mod.rs` (search state)
- Modify: `src/app.rs` (/ keybinding routing)

- [ ] **Step 1: Create search state module**

Create `src/store/search.rs`:

```rust
use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use twilight_model::id::Id;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum SearchScope {
    CurrentChannel(Id<ChannelMarker>),
    Server(Id<GuildMarker>),
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub message_id: Id<MessageMarker>,
    pub channel_id: Id<ChannelMarker>,
    pub channel_name: String,
    pub author_name: String,
    pub content_preview: String,
    pub timestamp: String,
}

#[derive(Debug)]
pub struct SearchState {
    pub query: String,
    pub scope: Option<SearchScope>,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub loading: bool,
    pub debounce_deadline: Option<Instant>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            scope: None,
            results: Vec::new(),
            selected: 0,
            loading: false,
            debounce_deadline: None,
        }
    }
}
```

- [ ] **Step 2: Add to Store and wire events/actions**

- Add `pub search: search::SearchState` to Store
- Events: `SearchResults { results: Vec<search::SearchResult> }`
- Actions: `SearchMessages { scope: search::SearchScope, query: String }`, `NavigateToSearchResult { channel_id, message_id }`
- Handle `SearchMessages` in `run_action_handler`: call Discord search API endpoint, translate to `SearchResults` event
- Process `SearchResults` in store: `self.search.results = results; self.search.loading = false;`

- [ ] **Step 3: Build search overlay**

Rewrite `src/tui/components/overlays/search.rs`:

```rust
pub struct SearchOverlay {
    visible: bool,
    cursor_pos: usize,
}
```

Implement `Component`:
- `render`: Search input at top with scope indicator `[# channel]` or `[🏠 Server]`. Results list below. Highlight query terms in results.
- `handle_key_event`: Text input for query, 300ms debounce before dispatching `SearchMessages`. Up/Down navigate results. Enter dispatches `NavigateToSearchResult`. `Ctrl+/` toggles scope. Esc closes.

- [ ] **Step 4: Wire into app**

In `src/app.rs`:
- When `FocusTarget::MessageList` and `/` pressed: open search overlay, switch focus
- Add `SearchOverlay` focus target to `FocusTarget` enum in `src/store/state.rs`

- [ ] **Step 5: Commit**

```bash
git add src/store/search.rs src/tui/components/overlays/search.rs src/discord/events.rs src/discord/actions.rs src/store/mod.rs src/store/state.rs src/app.rs
git commit -m "feat: add message search with channel and server scope"
```

---

## Feature 7: Threads

### Task 7a: Navigation stack infrastructure

**Files:**
- Modify: `src/store/state.rs` (PaneView enum, message_pane_stack)
- Modify: `src/tui/components/message_pane.rs` (render based on stack top)
- Modify: `src/tui/components/channel_header.rs` (breadcrumbs)

- [ ] **Step 1: Add PaneView and navigation stack to UiState**

In `src/store/state.rs`, add:
```rust
#[derive(Debug, Clone)]
pub enum PaneView {
    Channel(Id<ChannelMarker>),
    Thread { parent_channel: Id<ChannelMarker>, thread_id: Id<ChannelMarker> },
    SearchContext { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>, query: String },
    PinContext { channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker> },
}
```

Add to `UiState`:
```rust
pub message_pane_stack: Vec<PaneView>,
```

Add helper methods:
```rust
impl UiState {
    pub fn active_channel(&self) -> Option<Id<ChannelMarker>> {
        self.message_pane_stack.last().map(|view| match view {
            PaneView::Channel(id) => *id,
            PaneView::Thread { thread_id, .. } => *thread_id,
            PaneView::SearchContext { channel_id, .. } => *channel_id,
            PaneView::PinContext { channel_id, .. } => *channel_id,
        }).or(self.selected_channel)
    }

    pub fn push_pane(&mut self, view: PaneView) {
        if self.message_pane_stack.len() < 3 { // max depth 2 + base
            self.message_pane_stack.push(view);
        }
    }

    pub fn pop_pane(&mut self) -> bool {
        if self.message_pane_stack.len() > 1 {
            self.message_pane_stack.pop();
            true
        } else {
            false
        }
    }
}
```

- [ ] **Step 2: Update message pane to use stack**

In `src/tui/components/message_pane.rs`, update rendering to use `store.ui.active_channel()` instead of `store.ui.selected_channel` for determining which messages to display.

- [ ] **Step 3: Update channel header for breadcrumbs**

In `src/tui/components/channel_header.rs`, check the top of `message_pane_stack`. If it's a `Thread`, render `# parent > 🧵 Thread Name`. If `SearchContext`, render `# channel > Search: "query"`.

- [ ] **Step 4: Update Esc handling**

In message list's `handle_key_event`, when Esc is pressed: first try `store.ui.pop_pane()`. If that returned false (already at base), do the normal Esc behavior (change focus).

- [ ] **Step 5: Commit**

```bash
git add src/store/state.rs src/tui/components/message_pane.rs src/tui/components/channel_header.rs src/tui/components/message_list.rs
git commit -m "feat: add message pane navigation stack for threads/search/pins"
```

### Task 7b: Thread state and interaction

**Files:**
- Modify: `src/store/mod.rs` (active_threads, event processing)
- Modify: `src/discord/events.rs` (Thread events)
- Modify: `src/discord/actions.rs` (CreateThread)
- Modify: `src/tui/components/message.rs` (thread indicator)
- Modify: `src/tui/components/message_list.rs` (t/T keybindings)

- [ ] **Step 1: Add thread state to Store**

In `src/store/mod.rs`, add:
```rust
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub id: Id<ChannelMarker>,
    pub name: String,
    pub parent_channel: Id<ChannelMarker>,
    pub message_count: u32,
}

// In Store struct:
pub active_threads: HashMap<Id<ChannelMarker>, Vec<ThreadInfo>>,
```

- [ ] **Step 2: Add thread events and actions**

Events: `ThreadCreate { thread_info: ThreadInfo }`, `ThreadUpdate { thread_info: ThreadInfo }`, `ThreadDelete { thread_id, parent_channel }`, `ThreadListSync { guild_id, threads: Vec<ThreadInfo> }`

Actions: `CreateThread { channel_id, message_id, name: String }`

Handle in `translate_event` and `run_action_handler`.

- [ ] **Step 3: Process thread events in store**

```rust
DiscordEvent::ThreadCreate { thread_info } => {
    self.active_threads.entry(thread_info.parent_channel).or_default().push(thread_info);
}
// Similar for Update, Delete, ListSync
```

- [ ] **Step 4: Render thread indicator on messages**

In `src/tui/components/message.rs` `render_message`, after content and before reactions, check if the message has a thread:
```rust
// Check active_threads for this message
// Render: 🧵 Thread Name (N replies)
```

This requires passing the store (or thread info) to `render_message`. Extend the function signature to accept a reference to thread info if available.

- [ ] **Step 5: Add t/T keybindings**

In `src/tui/components/message_list.rs`:
- `t` on a message with a thread: push `PaneView::Thread` onto nav stack, fetch thread messages via existing `FetchMessages` action with thread's channel ID
- `T` on any message: prompt for thread name (reuse status bar input pattern from reply), dispatch `CreateThread`

- [ ] **Step 6: Render thread view**

When `message_pane_stack.last()` is `PaneView::Thread { parent_channel, thread_id }`:
- Render the parent message pinned at the top (fetch from parent channel's message buffer, render in bordered block with accent left border)
- Message list uses thread_id as the channel for messages
- Message input sends to thread_id

- [ ] **Step 7: Commit**

```bash
git add src/store/mod.rs src/discord/events.rs src/discord/actions.rs src/tui/components/message.rs src/tui/components/message_list.rs src/tui/components/message_pane.rs
git commit -m "feat: add thread view, creation, and navigation"
```

---

## Feature 8: User Profiles

### Task 8: Profile popup and overlay

**Files:**
- Create: `src/store/profiles.rs`
- Create: `src/tui/components/overlays/profile.rs`
- Modify: `src/discord/events.rs` (UserProfileLoaded)
- Modify: `src/discord/actions.rs` (FetchUserProfile, FetchGuildMemberProfile)
- Modify: `src/store/mod.rs` (profile cache)
- Modify: `src/tui/components/message_list.rs` (p keybinding)
- Modify: `src/tui/components/member_sidebar.rs` (p keybinding)
- Modify: `src/app.rs` (profile overlay routing)

- [ ] **Step 1: Create profile cache module**

Create `src/store/profiles.rs`:

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use twilight_model::id::marker::UserMarker;
use twilight_model::id::Id;

const PROFILE_TTL: Duration = Duration::from_secs(300); // 5 minutes

#[derive(Debug, Clone)]
pub struct UserProfile {
    pub user_id: Id<UserMarker>,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bot: bool,
}

#[derive(Debug, Clone)]
pub struct GuildMemberProfile {
    pub roles: Vec<(String, Option<u32>)>, // (name, color)
    pub joined_at: Option<String>,
    pub nickname: Option<String>,
}

struct CachedProfile {
    profile: UserProfile,
    fetched_at: Instant,
}

#[derive(Debug, Default)]
pub struct ProfileCache {
    cache: HashMap<Id<UserMarker>, CachedProfile>,
}

impl ProfileCache {
    pub fn get(&self, user_id: Id<UserMarker>) -> Option<&UserProfile> {
        self.cache.get(&user_id)
            .filter(|c| c.fetched_at.elapsed() < PROFILE_TTL)
            .map(|c| &c.profile)
    }

    pub fn insert(&mut self, profile: UserProfile) {
        let user_id = profile.user_id;
        self.cache.insert(user_id, CachedProfile {
            profile,
            fetched_at: Instant::now(),
        });
    }

    pub fn needs_fetch(&self, user_id: Id<UserMarker>) -> bool {
        self.get(user_id).is_none()
    }
}
```

- [ ] **Step 2: Wire into Store, events, actions**

- Add `pub profiles: profiles::ProfileCache` to Store
- Events: `UserProfileLoaded { profile: profiles::UserProfile }`
- Actions: `FetchUserProfile { user_id }`, `FetchGuildMemberProfile { guild_id, user_id }`
- Handle in action handler with REST: `client.user(user_id)` and `client.guild_member(guild_id, user_id)`

- [ ] **Step 3: Create profile overlay component**

Create `src/tui/components/overlays/profile.rs`:

```rust
pub enum ProfileMode {
    Minimal, // Small popup
    Full,    // Large centered overlay
}

pub struct ProfileOverlay {
    visible: bool,
    mode: ProfileMode,
    user_id: Option<Id<UserMarker>>,
    anchor_x: u16,
    anchor_y: u16,
}
```

Implement `Component`:
- `render` Minimal: small 4-line popup near anchor position showing name, status, top role
- `render` Full: larger centered overlay with all profile fields
- `handle_key_event`: Enter expands minimal to full. Esc closes.

- [ ] **Step 4: Add p keybinding to message list and member sidebar**

In `src/tui/components/message_list.rs`:
- `p` on selected message: get author_id, open profile overlay anchored to message

In `src/tui/components/member_sidebar.rs`:
- `p` on selected member: open profile overlay anchored to member entry

In `src/app.rs`: route key events to profile overlay when visible.

- [ ] **Step 5: Commit**

```bash
git add src/store/profiles.rs src/tui/components/overlays/profile.rs src/discord/events.rs src/discord/actions.rs src/store/mod.rs src/tui/components/message_list.rs src/tui/components/member_sidebar.rs src/app.rs
git commit -m "feat: add user profile popup and full overlay"
```

---

## Feature 9: File/Image Preview

### Task 9a: Terminal capability detection

**Files:**
- Create: `src/tui/terminal_caps.rs`
- Modify: `src/main.rs` (detect at startup)
- Test: `src/tui/terminal_caps.rs` (inline tests)

- [ ] **Step 1: Write capability detection with tests**

Create `src/tui/terminal_caps.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    None,
    Sixel,
    Kitty,
}

#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    pub graphics: GraphicsProtocol,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let graphics = detect_graphics_protocol();
        Self { graphics }
    }
}

fn detect_graphics_protocol() -> GraphicsProtocol {
    // 1. Check TERM_PROGRAM env var
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        match term.as_str() {
            "kitty" => return GraphicsProtocol::Kitty,
            "WezTerm" => return GraphicsProtocol::Kitty, // supports both, prefer Kitty
            "iTerm2" | "iTerm.app" => return GraphicsProtocol::Sixel,
            "mintty" => return GraphicsProtocol::Sixel,
            _ => {}
        }
    }
    // 2. Could add terminal query sequences here in the future
    GraphicsProtocol::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_defaults_to_none() {
        // With no env var set, should return None
        // (Can't fully test without mocking env, but verify the function runs)
        let caps = TerminalCapabilities::detect();
        // Just verify it doesn't panic
        let _ = caps.graphics;
    }
}
```

- [ ] **Step 2: Detect at startup and pass to App**

In `src/main.rs`, before creating the App:
```rust
let terminal_caps = tui::terminal_caps::TerminalCapabilities::detect();
```

Pass `terminal_caps` to the App struct (add field to App in `src/app.rs`), and make it available to the Store or pass to message rendering.

- [ ] **Step 3: Commit**

```bash
git add src/tui/terminal_caps.rs src/main.rs src/app.rs
git commit -m "feat: detect terminal graphics protocol at startup"
```

### Task 9b: Image cache and rendering

**Files:**
- Create: `src/store/images.rs`
- Create: `src/tui/image_renderer.rs`
- Modify: `Cargo.toml` (add image, base64, lru crates)
- Modify: `src/tui/components/message.rs` (inline image rendering)
- Modify: `src/discord/actions.rs` (FetchImage action)
- Modify: `src/config.rs` ([images] config section)

- [ ] **Step 1: Add dependencies**

In `Cargo.toml`, add:
```toml
image = "0.25"
base64 = "0.22"
lru = "0.12"
```

- [ ] **Step 2: Create image cache**

Create `src/store/images.rs`:
```rust
use lru::LruCache;
use std::num::NonZeroUsize;

#[derive(Debug, Clone)]
pub struct CachedImage {
    pub protocol_data: Vec<u8>,
    pub width: u16,
    pub height: u16,
}

pub struct ImageCache {
    cache: LruCache<String, CachedImage>,
}

impl ImageCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap()),
        }
    }

    pub fn get(&mut self, url: &str) -> Option<&CachedImage> {
        self.cache.get(url)
    }

    pub fn insert(&mut self, url: String, image: CachedImage) {
        self.cache.put(url, image);
    }

    pub fn contains(&self, url: &str) -> bool {
        self.cache.contains(url)
    }
}
```

- [ ] **Step 3: Create image renderer**

Create `src/tui/image_renderer.rs`:
```rust
use crate::tui::terminal_caps::GraphicsProtocol;

pub fn encode_image(
    image_bytes: &[u8],
    protocol: GraphicsProtocol,
    max_width_cols: u16,
) -> Result<(Vec<u8>, u16, u16), Box<dyn std::error::Error>> {
    let img = image::load_from_memory(image_bytes)?;

    // Resize to fit max_width_cols (approximate: 1 col ≈ 8px)
    let max_px = max_width_cols as u32 * 8;
    let img = img.resize(max_px, max_px * 2, image::imageops::FilterType::Lanczos3);

    match protocol {
        GraphicsProtocol::Kitty => encode_kitty(&img),
        GraphicsProtocol::Sixel => encode_sixel(&img),
        GraphicsProtocol::None => Err("No graphics protocol".into()),
    }
}

fn encode_kitty(img: &image::DynamicImage) -> Result<(Vec<u8>, u16, u16), Box<dyn std::error::Error>> {
    let png_bytes = {
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)?;
        buf
    };
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_bytes);

    // Kitty graphics protocol: chunked transmission
    let mut output = Vec::new();
    let chunks: Vec<&str> = b64.as_bytes().chunks(4096).map(|c| std::str::from_utf8(c).unwrap()).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let m = if i == chunks.len() - 1 { 0 } else { 1 };
        output.extend_from_slice(format!("\x1b_Gf=100,a=T,m={};{}\x1b\\", m, chunk).as_bytes());
    }

    let width_cols = (img.width() / 8) as u16;
    let height_rows = (img.height() / 16) as u16;
    Ok((output, width_cols.max(1), height_rows.max(1)))
}

fn encode_sixel(img: &image::DynamicImage) -> Result<(Vec<u8>, u16, u16), Box<dyn std::error::Error>> {
    // Sixel encoding: quantize to 256 colors, RLE encode
    // Implementation deferred to evaluation of sixel-rs crate vs custom encoder
    Err("Sixel encoding not yet implemented".into())
}
```

- [ ] **Step 4: Add [images] config**

In `src/config.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_width")]
    pub max_width: u16,
}

fn default_true() -> bool { true }
fn default_max_width() -> u16 { 40 }
```

Add to `Config` struct.

- [ ] **Step 5: Add FetchImage action and wire into message rendering**

Actions: `FetchImage { url: String, channel_id, message_id }`

Handle in `run_action_handler`: fetch image bytes via HTTP (use twilight's reqwest or a standalone reqwest client), encode with `image_renderer::encode_image`, store in `ImageCache`, emit `ImageLoaded` event.

In `src/tui/components/message.rs`, for image attachments (PNG, JPEG, GIF, WebP):
- If graphics != None and image is cached: write the protocol_data directly to the terminal (ratatui doesn't handle this natively — use crossterm raw write within the render area)
- If not cached: show `[loading {filename}...]`
- If graphics == None: show the normal `[file: name.ext (size)]` link

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/store/images.rs src/tui/image_renderer.rs src/config.rs src/discord/actions.rs src/discord/events.rs src/store/mod.rs src/tui/components/message.rs
git commit -m "feat: add inline image preview with Kitty/Sixel support"
```

---

## Final Integration

### Task 10: Integration testing and polish

**Files:**
- Modify: `src/app.rs` (final keybinding audit)
- Modify: `src/tui/components/overlays/mod.rs` (register new overlays)

- [ ] **Step 1: Register all new overlays in overlay mod**

In `src/tui/components/overlays/mod.rs`, add `pub mod emoji_picker;`, `pub mod pins;`, `pub mod profile;` alongside existing exports.

- [ ] **Step 2: Audit all new keybindings don't conflict**

Verify in `src/app.rs` and component handle_key_event methods that:
- `+` / `-` only active on FocusTarget::MessageList with a selected message
- `p` only active on MessageList (profiles author) and MemberSidebar (profiles member)
- `P` only active on MessageList (pin/unpin)
- `Ctrl+P` global (opens pins overlay)
- `/` only active on MessageList
- `t` / `T` only active on MessageList
- None of these conflict with existing MVP keybindings (r, e, d, y, gg, G, Ctrl+U, Ctrl+D)

- [ ] **Step 3: Run full test suite**

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 4: Build release**

Run: `cargo build --release`
Expected: Clean compilation

- [ ] **Step 5: Commit**

```bash
git add src/tui/components/overlays/mod.rs src/app.rs
git commit -m "feat: integrate all post-MVP features and audit keybindings"
```
