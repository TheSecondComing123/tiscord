use std::collections::HashMap;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::id::Id;

#[derive(Debug, Clone)]
pub struct GuildInfo {
    pub id: Id<GuildMarker>,
    pub name: String,
    pub icon: Option<String>,
    pub channels: Vec<ChannelInfo>,
}

#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub id: Id<ChannelMarker>,
    pub name: String,
    pub kind: ChannelKind,
    pub category_id: Option<Id<ChannelMarker>>,
    pub position: i32,
    /// Channel topic/description (text channels only).
    pub topic: Option<String>,
    /// Whether the channel is marked as NSFW.
    pub nsfw: bool,
    /// Slowmode delay in seconds (0 means no slowmode).
    pub rate_limit_per_user: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelKind {
    Text,
    Voice,
    Category,
    Announcement,
    Forum,
}

#[derive(Debug, Default)]
pub struct GuildState {
    pub guilds: Vec<GuildInfo>,
    guild_map: HashMap<Id<GuildMarker>, usize>,
}

impl GuildState {
    pub fn add_guild(&mut self, info: GuildInfo) {
        let id = info.id;
        // Replace if already exists, otherwise append.
        if let Some(&idx) = self.guild_map.get(&id) {
            self.guilds[idx] = info;
        } else {
            let idx = self.guilds.len();
            self.guilds.push(info);
            self.guild_map.insert(id, idx);
        }
    }

    pub fn remove_guild(&mut self, id: Id<GuildMarker>) {
        if let Some(idx) = self.guild_map.remove(&id) {
            self.guilds.remove(idx);
            // Shift all indices after the removed element down by one.
            for val in self.guild_map.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
        }
    }

    pub fn get_guild(&self, id: Id<GuildMarker>) -> Option<&GuildInfo> {
        let idx = self.guild_map.get(&id)?;
        self.guilds.get(*idx)
    }

    /// Returns channels for the guild sorted by category position then channel position.
    /// Categories come before their children. Uncategorized channels sort first.
    pub fn add_channel_to_guild(&mut self, guild_id: Id<GuildMarker>, channel: ChannelInfo) {
        if let Some(&idx) = self.guild_map.get(&guild_id) {
            let channels = &mut self.guilds[idx].channels;
            // Replace if same id already exists, otherwise append.
            if let Some(existing) = channels.iter_mut().find(|c| c.id == channel.id) {
                *existing = channel;
            } else {
                channels.push(channel);
            }
        }
    }

    pub fn update_channel_in_guild(&mut self, guild_id: Id<GuildMarker>, channel: ChannelInfo) {
        self.add_channel_to_guild(guild_id, channel);
    }

    pub fn remove_channel_from_guild(
        &mut self,
        guild_id: Id<GuildMarker>,
        channel_id: Id<ChannelMarker>,
    ) {
        if let Some(&idx) = self.guild_map.get(&guild_id) {
            self.guilds[idx].channels.retain(|c| c.id != channel_id);
        }
    }

    pub fn get_channels_for_guild(&self, id: Id<GuildMarker>) -> Vec<&ChannelInfo> {
        let guild = match self.get_guild(id) {
            Some(g) => g,
            None => return vec![],
        };

        let mut channels: Vec<&ChannelInfo> = guild.channels.iter().collect();

        // Sort: categories before children, then by position within each group.
        // Key: (category_position, category_id_u64_or_0, is_not_category, position)
        let category_position: HashMap<Id<ChannelMarker>, i32> = guild
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Category)
            .map(|c| (c.id, c.position))
            .collect();

        channels.sort_by_key(|c| {
            let (cat_pos, cat_id) = match c.category_id {
                Some(cid) => (
                    category_position.get(&cid).copied().unwrap_or(i32::MAX),
                    cid.get(),
                ),
                None => {
                    if c.kind == ChannelKind::Category {
                        (c.position, c.id.get())
                    } else {
                        (i32::MIN, 0u64)
                    }
                }
            };
            let is_child = if c.category_id.is_some() { 1i32 } else { 0 };
            (cat_pos, cat_id, is_child, c.position)
        });

        channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guild_id(n: u64) -> Id<GuildMarker> {
        Id::new(n)
    }

    fn ch_id(n: u64) -> Id<ChannelMarker> {
        Id::new(n)
    }

    fn make_guild(id: u64, name: &str, channels: Vec<ChannelInfo>) -> GuildInfo {
        GuildInfo {
            id: guild_id(id),
            name: name.to_string(),
            icon: None,
            channels,
        }
    }

    fn make_channel(id: u64, name: &str, kind: ChannelKind, category_id: Option<u64>, position: i32) -> ChannelInfo {
        ChannelInfo {
            id: ch_id(id),
            name: name.to_string(),
            kind,
            category_id: category_id.map(ch_id),
            position,
            topic: None,
            nsfw: false,
            rate_limit_per_user: None,
        }
    }

    #[test]
    fn test_add_guild() {
        let mut state = GuildState::default();
        state.add_guild(make_guild(1, "Test Guild", vec![]));

        assert_eq!(state.guilds.len(), 1);
        let g = state.get_guild(guild_id(1)).unwrap();
        assert_eq!(g.name, "Test Guild");
    }

    #[test]
    fn test_add_guild_replaces_existing() {
        let mut state = GuildState::default();
        state.add_guild(make_guild(1, "Original", vec![]));
        state.add_guild(make_guild(1, "Updated", vec![]));

        assert_eq!(state.guilds.len(), 1);
        assert_eq!(state.get_guild(guild_id(1)).unwrap().name, "Updated");
    }

    #[test]
    fn test_remove_guild() {
        let mut state = GuildState::default();
        state.add_guild(make_guild(1, "Alpha", vec![]));
        state.add_guild(make_guild(2, "Beta", vec![]));
        state.add_guild(make_guild(3, "Gamma", vec![]));

        state.remove_guild(guild_id(2));

        assert_eq!(state.guilds.len(), 2);
        assert!(state.get_guild(guild_id(1)).is_some());
        assert!(state.get_guild(guild_id(2)).is_none());
        assert!(state.get_guild(guild_id(3)).is_some());
    }

    #[test]
    fn test_get_guild_missing() {
        let state = GuildState::default();
        assert!(state.get_guild(guild_id(99)).is_none());
    }

    #[test]
    fn test_channel_sorting() {
        // Category 10 at position 0, Category 20 at position 1.
        // Channel 11 under category 10 at position 5.
        // Channel 12 under category 10 at position 2.
        // Channel 21 under category 20 at position 1.
        // Channel 1 uncategorized at position 3.
        let channels = vec![
            make_channel(20, "cat-b", ChannelKind::Category, None, 1),
            make_channel(10, "cat-a", ChannelKind::Category, None, 0),
            make_channel(11, "ch-a2", ChannelKind::Text, Some(10), 5),
            make_channel(12, "ch-a1", ChannelKind::Text, Some(10), 2),
            make_channel(21, "ch-b1", ChannelKind::Text, Some(20), 1),
            make_channel(1, "uncategorized", ChannelKind::Text, None, 3),
        ];

        let mut state = GuildState::default();
        state.add_guild(make_guild(1, "G", channels));

        let sorted = state.get_channels_for_guild(guild_id(1));
        let names: Vec<&str> = sorted.iter().map(|c| c.name.as_str()).collect();

        // Expected: uncategorized first, then cat-a, then its children by position,
        // then cat-b, then its children.
        assert_eq!(names, vec!["uncategorized", "cat-a", "ch-a1", "ch-a2", "cat-b", "ch-b1"]);
    }
}
