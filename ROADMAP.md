# Roadmap

## Completed

### Message Display Polish

- [x] Date separators between messages from different days
- [x] "New messages" unread separator line
- [x] Message grouping (consecutive messages from same author collapse header)
- [x] Send typing indicator when user is typing

### Markdown Improvements

- [x] Blockquotes (`> text`)
- [x] Masked links (`[text](url)`)
- [x] Emoji shortcodes (`:thumbsup:` → emoji)
- [x] Underline (`__text__`)

### Input Improvements

- [x] `@mention` autocomplete (fuzzy search members while typing)
- [x] `#channel` autocomplete
- [x] `:emoji:` autocomplete
- [x] Paste support (Ctrl+V / terminal paste)
- [x] Send typing indicator while composing
- [x] Draft persistence (save input when switching channels)

### Search

- [x] Implement raw HTTP search request (bypass twilight-http)
- [x] Query highlighting in results
- [x] Search filters (from:, has:, before:, after:)

### Online Status

- [x] Track online/idle/dnd/offline from PresenceUpdate events
- [x] Show status dots in member sidebar
- [x] Sort members by online status

### Notification System

- [x] Desktop notifications via `notify-rust`
- [x] Channel/server mute controls
- [x] Window title badge with unread count

### Channel Management

- [x] Channel topic display in header
- [x] Category collapse/expand toggle
- [x] NSFW channel indicator
- [x] Slowmode indicator
- [x] Forum channel thread listing

### Message Rendering

- [x] Sticker display (name + description fallback)
- [x] Poll rendering
- [x] Message components (buttons rendered as text labels)
- [x] Embed images/thumbnails (with Kitty)
- [x] Embed color bar (use embed color field)

### User Account

- [x] Set own custom status
- [x] Set online/idle/dnd/invisible status
- [x] Friend list / pending requests
- [x] Block/unblock users

### Caching and Cleanup

- [x] Background cleanup of expired typing indicators
- [x] Bounded profile cache (cap at N entries)

## In Progress

### Remaining Features

- [ ] Sound alerts on mention
- [x] Full edit timestamp on hover
- [ ] User notes

### Moderation (for server admins)

- [ ] Kick/ban members
- [ ] Timeout members
- [ ] Delete messages in bulk
- [ ] View audit log

### Advanced Performance

- [ ] Lazy message loading (only fetch visible range)
- [ ] Persistent message cache across restarts

### Sixel Image Protocol

- [ ] Sixel encoding for iTerm2/mintty terminals
- [ ] Image thumbnail sizing based on terminal cell dimensions

## Won't Implement

- Voice/video calls (no audio in terminal)
- Screen sharing
- Video embeds playback
- Drag-and-drop file upload
- Rich presence / game activity
- Stage channels (speaking in stages)
