use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use twilight_model::id::marker::{MessageMarker, UserMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerInfo {
    pub name: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactionEmoji {
    Unicode(String),
    Custom { id: u64, name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: ReactionEmoji,
    pub count: u32,
    pub me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub fields: Vec<EmbedField>,
    pub footer: Option<String>,
    pub author_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollAnswer {
    pub text: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollInfo {
    pub question: String,
    pub answers: Vec<PollAnswer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub kind: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: Id<MessageMarker>,
    pub author_name: String,
    pub author_id: Id<UserMarker>,
    pub content: String,
    pub timestamp: String,
    pub reply_to: Option<ReplyContext>,
    pub attachments: Vec<Attachment>,
    pub is_edited: bool,
    pub edited_timestamp: Option<String>,
    pub reactions: Vec<Reaction>,
    pub embeds: Vec<Embed>,
    pub stickers: Vec<StickerInfo>,
    pub poll: Option<PollInfo>,
    pub components: Vec<ComponentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyContext {
    pub author_name: String,
    pub content_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            messages: VecDeque::new(),
            capacity,
        }
    }

    pub fn push(&mut self, msg: StoredMessage) {
        if self.messages.len() == self.capacity {
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

    pub fn update(&mut self, id: Id<MessageMarker>, content: String, edited_timestamp: Option<String>) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.content = content;
            msg.is_edited = true;
            if edited_timestamp.is_some() {
                msg.edited_timestamp = edited_timestamp;
            }
        }
    }

    /// Prepend a batch of messages to the front of the buffer (older history).
    /// Messages are expected in chronological order (oldest first); they will
    /// appear at the front of the deque in that order.
    /// If prepending would exceed capacity, the excess is trimmed from the back.
    pub fn prepend(&mut self, msgs: Vec<StoredMessage>) {
        for msg in msgs.into_iter().rev() {
            self.messages.push_front(msg);
        }
        while self.messages.len() > self.capacity {
            self.messages.pop_back();
        }
    }

    pub fn add_reaction(&mut self, message_id: Id<MessageMarker>, emoji: ReactionEmoji, user_is_self: bool) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            if let Some(reaction) = msg.reactions.iter_mut().find(|r| reaction_emoji_eq(&r.emoji, &emoji)) {
                reaction.count += 1;
                if user_is_self {
                    reaction.me = true;
                }
            } else {
                msg.reactions.push(Reaction {
                    emoji,
                    count: 1,
                    me: user_is_self,
                });
            }
        }
    }

    pub fn remove_reaction(&mut self, message_id: Id<MessageMarker>, emoji: &ReactionEmoji, user_is_self: bool) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            if let Some(reaction) = msg.reactions.iter_mut().find(|r| reaction_emoji_eq(&r.emoji, emoji)) {
                if reaction.count > 1 {
                    reaction.count -= 1;
                    if user_is_self {
                        reaction.me = false;
                    }
                } else {
                    // count will drop to 0, remove the entry
                    let emoji_ref = emoji;
                    msg.reactions.retain(|r| !reaction_emoji_eq(&r.emoji, emoji_ref));
                }
            }
        }
    }

    pub fn remove_all_reactions(&mut self, message_id: Id<MessageMarker>) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            msg.reactions.clear();
        }
    }
}

/// Maximum number of messages cached per channel when persisting to disk.
const PERSIST_LIMIT: usize = 100;

/// Save the most recent messages per channel to disk.
pub fn save_cache(
    messages: &std::collections::HashMap<twilight_model::id::Id<twilight_model::id::marker::ChannelMarker>, MessageBuffer>,
) {
    use std::collections::HashMap;
    let path = crate::config::Config::data_dir().join("message_cache.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Store last N messages per channel, keyed by channel ID string.
    let cache: HashMap<String, Vec<&StoredMessage>> = messages
        .iter()
        .map(|(cid, buf)| {
            let msgs: Vec<&StoredMessage> = buf.messages().iter().rev().take(PERSIST_LIMIT).collect();
            (cid.get().to_string(), msgs.into_iter().rev().collect())
        })
        .collect();
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::write(path, json);
    }
}

/// Load the message cache from disk. Returns a map of channel ID → messages.
pub fn load_cache() -> std::collections::HashMap<twilight_model::id::Id<twilight_model::id::marker::ChannelMarker>, MessageBuffer> {
    use std::collections::HashMap;
    let path = crate::config::Config::data_dir().join("message_cache.json");
    if !path.exists() {
        return HashMap::new();
    }
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };
    let raw: HashMap<String, Vec<StoredMessage>> = match serde_json::from_str(&data) {
        Ok(r) => r,
        Err(_) => return HashMap::new(),
    };
    raw.into_iter()
        .filter_map(|(k, msgs)| {
            let id: u64 = k.parse().ok()?;
            let cid = twilight_model::id::Id::new(id);
            let mut buf = MessageBuffer::new(500);
            for msg in msgs {
                buf.push(msg);
            }
            Some((cid, buf))
        })
        .collect()
}

pub fn reaction_emoji_eq(a: &ReactionEmoji, b: &ReactionEmoji) -> bool {
    match (a, b) {
        (ReactionEmoji::Unicode(a), ReactionEmoji::Unicode(b)) => a == b,
        (ReactionEmoji::Custom { id: a, .. }, ReactionEmoji::Custom { id: b, .. }) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_msg(id: u64, content: &str) -> StoredMessage {
        StoredMessage {
            id: Id::new(id),
            author_name: format!("user_{id}"),
            author_id: Id::new(id),
            content: content.to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            reply_to: None,
            attachments: vec![],
            is_edited: false,
            edited_timestamp: None,
            reactions: vec![],
            embeds: vec![],
            stickers: vec![],
            poll: None,
            components: vec![],
        }
    }

    #[test]
    fn test_push_and_retrieve() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "hello"));
        buf.push(make_test_msg(2, "world"));

        assert_eq!(buf.len(), 2);
        let msgs: Vec<_> = buf.messages().iter().collect();
        assert_eq!(msgs[0].id, Id::new(1));
        assert_eq!(msgs[0].content, "hello");
        assert_eq!(msgs[1].id, Id::new(2));
        assert_eq!(msgs[1].content, "world");
    }

    #[test]
    fn test_eviction_at_capacity() {
        let mut buf = MessageBuffer::new(3);
        buf.push(make_test_msg(1, "first"));
        buf.push(make_test_msg(2, "second"));
        buf.push(make_test_msg(3, "third"));
        buf.push(make_test_msg(4, "fourth"));

        assert_eq!(buf.len(), 3);
        let ids: Vec<u64> = buf.messages().iter().map(|m| m.id.get()).collect();
        assert_eq!(ids, vec![2, 3, 4]);
    }

    #[test]
    fn test_remove_by_id() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "a"));
        buf.push(make_test_msg(2, "b"));
        buf.push(make_test_msg(3, "c"));

        buf.remove(Id::new(2));

        assert_eq!(buf.len(), 2);
        let ids: Vec<u64> = buf.messages().iter().map(|m| m.id.get()).collect();
        assert_eq!(ids, vec![1, 3]);
    }

    #[test]
    fn test_update_content() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "original"));

        buf.update(Id::new(1), "edited".to_string(), Some("2026-01-01T01:00:00Z".to_string()));

        let msg = buf.messages().iter().find(|m| m.id == Id::new(1)).unwrap();
        assert_eq!(msg.content, "edited");
        assert!(msg.is_edited);
        assert_eq!(msg.edited_timestamp.as_deref(), Some("2026-01-01T01:00:00Z"));
    }

    #[test]
    fn test_add_reaction_new_emoji() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "hello"));

        buf.add_reaction(Id::new(1), ReactionEmoji::Unicode("👍".to_string()), false);

        let msg = buf.messages().iter().find(|m| m.id == Id::new(1)).unwrap();
        assert_eq!(msg.reactions.len(), 1);
        assert_eq!(msg.reactions[0].count, 1);
        assert!(!msg.reactions[0].me);
    }

    #[test]
    fn test_add_reaction_existing_increments() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "hello"));

        buf.add_reaction(Id::new(1), ReactionEmoji::Unicode("👍".to_string()), false);
        buf.add_reaction(Id::new(1), ReactionEmoji::Unicode("👍".to_string()), true);

        let msg = buf.messages().iter().find(|m| m.id == Id::new(1)).unwrap();
        assert_eq!(msg.reactions.len(), 1);
        assert_eq!(msg.reactions[0].count, 2);
        assert!(msg.reactions[0].me);
    }

    #[test]
    fn test_remove_reaction_decrements() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "hello"));

        buf.add_reaction(Id::new(1), ReactionEmoji::Unicode("👍".to_string()), true);
        buf.remove_reaction(Id::new(1), &ReactionEmoji::Unicode("👍".to_string()), true);

        let msg = buf.messages().iter().find(|m| m.id == Id::new(1)).unwrap();
        assert_eq!(msg.reactions.len(), 0);
    }

    #[test]
    fn test_remove_all_reactions() {
        let mut buf = MessageBuffer::new(10);
        buf.push(make_test_msg(1, "hello"));

        buf.add_reaction(Id::new(1), ReactionEmoji::Unicode("👍".to_string()), false);
        buf.add_reaction(Id::new(1), ReactionEmoji::Unicode("❤️".to_string()), true);
        buf.remove_all_reactions(Id::new(1));

        let msg = buf.messages().iter().find(|m| m.id == Id::new(1)).unwrap();
        assert_eq!(msg.reactions.len(), 0);
    }
}
