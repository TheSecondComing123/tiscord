# Roadmap

## In Progress

### Message Display Polish
- [ ] Date separators between messages from different days
- [ ] "New messages" unread separator line
- [ ] Message grouping (consecutive messages from same author collapse header)
- [ ] Send typing indicator when user is typing

### Markdown Improvements
- [ ] Blockquotes (`> text`)
- [ ] Masked links (`[text](url)`)
- [ ] Emoji shortcodes (`:thumbsup:` → emoji)
- [ ] Underline (`__text__`)

### Input Improvements
- [ ] `@mention` autocomplete (fuzzy search members while typing)
- [ ] `#channel` autocomplete
- [ ] `:emoji:` autocomplete
- [ ] Paste support (Ctrl+V / terminal paste)
- [ ] Send typing indicator while composing
- [ ] Draft persistence (save input when switching channels)

### Search (currently stubbed)
- [ ] Implement raw HTTP search request (bypass twilight-http)
- [ ] Query highlighting in results
- [ ] Search filters (from:, has:, before:, after:)

### Online Status
- [ ] Track online/idle/dnd/offline from PresenceUpdate events
- [ ] Show status dots in member sidebar
- [ ] Sort members by online status

## Planned

### Notifications
- [ ] Desktop notifications via `notify-rust`
- [ ] Sound alerts on mention
- [ ] Channel/server mute controls
- [ ] Window title badge with unread count

### Channel Management
- [ ] Channel topic display in header
- [ ] Category collapse/expand toggle
- [ ] NSFW channel indicator
- [ ] Slowmode indicator
- [ ] Forum channel thread listing

### Message Features
- [ ] Sticker display (name + description fallback)
- [ ] Poll rendering
- [ ] Message components (buttons rendered as text labels)
- [ ] Embed images/thumbnails (with Kitty/Sixel)
- [ ] Embed color bar (use embed color field)
- [ ] Full edit timestamp on hover

### User Features
- [ ] Set own custom status
- [ ] Set online/idle/dnd/invisible status
- [ ] Friend list / pending requests
- [ ] Block/unblock users
- [ ] User notes

### Moderation (for server admins)
- [ ] Kick/ban members
- [ ] Timeout members
- [ ] Delete messages in bulk
- [ ] View audit log

### Performance
- [ ] Lazy message loading (only fetch visible range)
- [ ] Bounded profile cache (cap at N entries)
- [ ] Background cleanup of expired typing indicators
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
