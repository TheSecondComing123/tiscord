use std::collections::VecDeque;
use twilight_model::id::marker::{MessageMarker, UserMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: Id<MessageMarker>,
    pub author_name: String,
    pub author_id: Id<UserMarker>,
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

    pub fn update(&mut self, id: Id<MessageMarker>, content: String) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.content = content;
            msg.is_edited = true;
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

        buf.update(Id::new(1), "edited".to_string());

        let msg = buf.messages().iter().find(|m| m.id == Id::new(1)).unwrap();
        assert_eq!(msg.content, "edited");
        assert!(msg.is_edited);
    }
}
