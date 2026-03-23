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

impl NotificationState {
    pub fn increment_unread(&mut self, channel_id: Id<ChannelMarker>) {
        self.channels.entry(channel_id).or_default().unread_count += 1;
    }

    pub fn increment_mentions(&mut self, channel_id: Id<ChannelMarker>) {
        self.channels.entry(channel_id).or_default().mention_count += 1;
    }

    pub fn mark_read(&mut self, channel_id: Id<ChannelMarker>) {
        if let Some(notif) = self.channels.get_mut(&channel_id) {
            notif.unread_count = 0;
            notif.mention_count = 0;
        }
    }

    pub fn get(&self, channel_id: Id<ChannelMarker>) -> Option<&ChannelNotification> {
        self.channels.get(&channel_id)
    }

    pub fn has_unreads(&self, channel_id: Id<ChannelMarker>) -> bool {
        self.channels
            .get(&channel_id)
            .map(|n| n.unread_count > 0)
            .unwrap_or(false)
    }

    pub fn has_mentions(&self, channel_id: Id<ChannelMarker>) -> bool {
        self.channels
            .get(&channel_id)
            .map(|n| n.mention_count > 0)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(id: u64) -> Id<ChannelMarker> {
        Id::new(id)
    }

    #[test]
    fn test_increment_unread() {
        let mut state = NotificationState::default();
        state.increment_unread(ch(1));
        state.increment_unread(ch(1));

        let notif = state.get(ch(1)).unwrap();
        assert_eq!(notif.unread_count, 2);
    }

    #[test]
    fn test_increment_mentions() {
        let mut state = NotificationState::default();
        state.increment_mentions(ch(2));

        let notif = state.get(ch(2)).unwrap();
        assert_eq!(notif.mention_count, 1);
    }

    #[test]
    fn test_mark_read() {
        let mut state = NotificationState::default();
        state.increment_unread(ch(3));
        state.increment_unread(ch(3));
        state.increment_mentions(ch(3));

        state.mark_read(ch(3));

        let notif = state.get(ch(3)).unwrap();
        assert_eq!(notif.unread_count, 0);
        assert_eq!(notif.mention_count, 0);
    }

    #[test]
    fn test_get_missing_channel() {
        let state = NotificationState::default();
        assert!(state.get(ch(99)).is_none());
    }

    #[test]
    fn test_has_unreads() {
        let mut state = NotificationState::default();
        assert!(!state.has_unreads(ch(4)));

        state.increment_unread(ch(4));
        assert!(state.has_unreads(ch(4)));

        state.mark_read(ch(4));
        assert!(!state.has_unreads(ch(4)));
    }

    #[test]
    fn test_has_mentions() {
        let mut state = NotificationState::default();
        assert!(!state.has_mentions(ch(5)));

        state.increment_mentions(ch(5));
        assert!(state.has_mentions(ch(5)));

        state.mark_read(ch(5));
        assert!(!state.has_mentions(ch(5)));
    }
}
