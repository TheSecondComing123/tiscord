use std::collections::HashMap;
use std::time::{Duration, Instant};

use twilight_model::id::marker::{ChannelMarker, UserMarker};
use twilight_model::id::Id;

const TYPING_EXPIRE_SECS: u64 = 10;

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
    pub fn add_typing(
        &mut self,
        channel_id: Id<ChannelMarker>,
        user_id: Id<UserMarker>,
        display_name: String,
    ) {
        let entry = self.channels.entry(channel_id).or_default();
        // Update existing entry or push new one
        if let Some(existing) = entry.iter_mut().find(|u| u.user_id == user_id) {
            existing.started_at = Instant::now();
            existing.display_name = display_name;
        } else {
            entry.push(TypingUser {
                user_id,
                display_name,
                started_at: Instant::now(),
            });
        }
    }

    /// Returns active typers for a channel, filtering out entries older than 10 seconds.
    pub fn get_typers(&self, channel_id: Id<ChannelMarker>) -> Vec<&TypingUser> {
        let cutoff = Duration::from_secs(TYPING_EXPIRE_SECS);
        self.channels
            .get(&channel_id)
            .map(|users| {
                users
                    .iter()
                    .filter(|u| u.started_at.elapsed() < cutoff)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn has_typers(&self, channel_id: Id<ChannelMarker>) -> bool {
        !self.get_typers(channel_id).is_empty()
    }

    /// Remove all expired typing entries from the map, freeing memory.
    /// Should be called periodically (e.g. every few seconds) from the event loop.
    pub fn cleanup(&mut self) {
        let cutoff = Duration::from_secs(TYPING_EXPIRE_SECS);
        self.channels.retain(|_, users| {
            users.retain(|u| u.started_at.elapsed() < cutoff);
            !users.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(id: u64) -> Id<ChannelMarker> {
        Id::new(id)
    }

    fn usr(id: u64) -> Id<UserMarker> {
        Id::new(id)
    }

    #[test]
    fn test_add_and_get_typer() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());

        let typers = state.get_typers(ch(1));
        assert_eq!(typers.len(), 1);
        assert_eq!(typers[0].display_name, "alice");
        assert_eq!(typers[0].user_id, usr(10));
    }

    #[test]
    fn test_multiple_typers_same_channel() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());
        state.add_typing(ch(1), usr(11), "bob".to_string());

        let typers = state.get_typers(ch(1));
        assert_eq!(typers.len(), 2);
    }

    #[test]
    fn test_typers_different_channels() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());
        state.add_typing(ch(2), usr(11), "bob".to_string());

        assert_eq!(state.get_typers(ch(1)).len(), 1);
        assert_eq!(state.get_typers(ch(2)).len(), 1);
        assert_eq!(state.get_typers(ch(3)).len(), 0);
    }

    #[test]
    fn test_update_existing_typer() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());
        // Call again for the same user — should update, not duplicate
        state.add_typing(ch(1), usr(10), "alice_renamed".to_string());

        let typers = state.get_typers(ch(1));
        assert_eq!(typers.len(), 1);
        assert_eq!(typers[0].display_name, "alice_renamed");
    }

    #[test]
    fn test_has_typers_true_and_false() {
        let mut state = TypingState::default();
        assert!(!state.has_typers(ch(1)));

        state.add_typing(ch(1), usr(10), "alice".to_string());
        assert!(state.has_typers(ch(1)));
    }

    #[test]
    fn test_empty_channel_returns_no_typers() {
        let state = TypingState::default();
        assert_eq!(state.get_typers(ch(99)).len(), 0);
        assert!(!state.has_typers(ch(99)));
    }

    #[test]
    fn test_expired_typer_filtered_out() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());

        // Manually expire the entry by backdating started_at
        let users = state.channels.get_mut(&ch(1)).unwrap();
        users[0].started_at =
            Instant::now() - Duration::from_secs(TYPING_EXPIRE_SECS + 1);

        let typers = state.get_typers(ch(1));
        assert_eq!(typers.len(), 0);
        assert!(!state.has_typers(ch(1)));
    }

    #[test]
    fn test_refreshing_typer_extends_expiry() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());

        // Backdate to near-expiry
        let users = state.channels.get_mut(&ch(1)).unwrap();
        users[0].started_at =
            Instant::now() - Duration::from_secs(TYPING_EXPIRE_SECS - 1);

        // Call add_typing again to refresh
        state.add_typing(ch(1), usr(10), "alice".to_string());

        // Should still be present with a fresh timestamp
        assert!(state.has_typers(ch(1)));
    }

    #[test]
    fn test_cleanup_removes_expired_entries() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());
        state.add_typing(ch(2), usr(11), "bob".to_string());

        // Expire alice's entry
        let users = state.channels.get_mut(&ch(1)).unwrap();
        users[0].started_at =
            Instant::now() - Duration::from_secs(TYPING_EXPIRE_SECS + 1);

        state.cleanup();

        // Channel 1 should be removed entirely (no active typers)
        assert!(!state.channels.contains_key(&ch(1)));
        // Channel 2 still has an active typer
        assert!(state.channels.contains_key(&ch(2)));
    }

    #[test]
    fn test_cleanup_removes_expired_user_but_keeps_channel() {
        let mut state = TypingState::default();
        state.add_typing(ch(1), usr(10), "alice".to_string());
        state.add_typing(ch(1), usr(11), "bob".to_string());

        // Expire only alice
        let users = state.channels.get_mut(&ch(1)).unwrap();
        users[0].started_at =
            Instant::now() - Duration::from_secs(TYPING_EXPIRE_SECS + 1);

        state.cleanup();

        // Channel still exists because bob is still typing
        assert!(state.channels.contains_key(&ch(1)));
        let remaining = &state.channels[&ch(1)];
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].user_id, usr(11));
    }

    #[test]
    fn test_cleanup_empty_state_does_nothing() {
        let mut state = TypingState::default();
        state.cleanup(); // Should not panic on empty state
        assert!(state.channels.is_empty());
    }
}
