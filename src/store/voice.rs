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
    /// Add a user to a voice channel, removing them from any previous channel first.
    pub fn user_joined(&mut self, channel_id: Id<ChannelMarker>, user: VoiceUser) {
        // Remove from any channel they may already be in
        self.user_left(user.user_id);
        // Add to the new channel
        self.channels.entry(channel_id).or_default().push(user);
    }

    /// Remove a user from all voice channels.
    pub fn user_left(&mut self, user_id: Id<UserMarker>) {
        for users in self.channels.values_mut() {
            users.retain(|u| u.user_id != user_id);
        }
    }

    /// Returns the slice of users in a channel (empty slice if none).
    pub fn get_users(&self, channel_id: Id<ChannelMarker>) -> &[VoiceUser] {
        self.channels
            .get(&channel_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Count of users currently in a channel.
    pub fn user_count(&self, channel_id: Id<ChannelMarker>) -> usize {
        self.get_users(channel_id).len()
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

    fn make_user(user_id: u64, name: &str) -> VoiceUser {
        VoiceUser {
            user_id: usr(user_id),
            display_name: name.to_string(),
            self_mute: false,
            self_deaf: false,
        }
    }

    #[test]
    fn test_user_join_and_count() {
        let mut state = VoiceState::default();
        state.user_joined(ch(1), make_user(10, "alice"));
        state.user_joined(ch(1), make_user(11, "bob"));

        assert_eq!(state.user_count(ch(1)), 2);
        let users = state.get_users(ch(1));
        assert_eq!(users.len(), 2);
        assert!(users.iter().any(|u| u.display_name == "alice"));
        assert!(users.iter().any(|u| u.display_name == "bob"));
    }

    #[test]
    fn test_user_leave() {
        let mut state = VoiceState::default();
        state.user_joined(ch(1), make_user(10, "alice"));
        state.user_joined(ch(1), make_user(11, "bob"));

        state.user_left(usr(10));

        assert_eq!(state.user_count(ch(1)), 1);
        let users = state.get_users(ch(1));
        assert_eq!(users[0].display_name, "bob");
    }

    #[test]
    fn test_user_switch_channel() {
        let mut state = VoiceState::default();
        state.user_joined(ch(1), make_user(10, "alice"));

        // alice switches to ch(2) — should be removed from ch(1)
        state.user_joined(ch(2), make_user(10, "alice"));

        assert_eq!(state.user_count(ch(1)), 0);
        assert_eq!(state.user_count(ch(2)), 1);
    }

    #[test]
    fn test_empty_channel() {
        let state = VoiceState::default();

        assert_eq!(state.user_count(ch(99)), 0);
        assert_eq!(state.get_users(ch(99)).len(), 0);
    }
}
