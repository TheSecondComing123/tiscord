use std::collections::HashMap;
use std::time::{Duration, Instant};
use twilight_model::id::marker::UserMarker;
use twilight_model::id::Id;

const PROFILE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const MAX_CACHE_SIZE: usize = 200;

#[derive(Debug, Clone)]
pub struct UserProfile {
    pub user_id: Id<UserMarker>,
    pub username: String,
    pub display_name: Option<String>,
    pub bot: bool,
}

#[derive(Debug, Clone)]
pub struct GuildMemberProfile {
    pub roles: Vec<String>,
    pub joined_at: Option<String>,
    pub nickname: Option<String>,
}

struct CachedProfile {
    profile: UserProfile,
    fetched_at: Instant,
}

#[derive(Default)]
pub struct ProfileCache {
    cache: HashMap<Id<UserMarker>, CachedProfile>,
}

impl ProfileCache {
    pub fn get(&self, user_id: Id<UserMarker>) -> Option<&UserProfile> {
        self.cache
            .get(&user_id)
            .filter(|c| c.fetched_at.elapsed() < PROFILE_TTL)
            .map(|c| &c.profile)
    }

    pub fn insert(&mut self, profile: UserProfile) {
        let user_id = profile.user_id;
        // If already at capacity and this is a new key, evict the oldest entry
        if !self.cache.contains_key(&user_id) && self.cache.len() >= MAX_CACHE_SIZE {
            self.evict_oldest();
        }
        self.cache.insert(
            user_id,
            CachedProfile {
                profile,
                fetched_at: Instant::now(),
            },
        );
    }

    /// Remove the entry with the oldest `fetched_at` timestamp.
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self
            .cache
            .iter()
            .min_by_key(|(_, v)| v.fetched_at)
            .map(|(k, _)| *k)
        {
            self.cache.remove(&oldest_key);
        }
    }

    /// Remove all entries whose TTL has expired.
    pub fn cleanup(&mut self) {
        self.cache.retain(|_, v| v.fetched_at.elapsed() < PROFILE_TTL);
    }

    pub fn needs_fetch(&self, user_id: Id<UserMarker>) -> bool {
        self.get(user_id).is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(id: u64) -> UserProfile {
        UserProfile {
            user_id: Id::new(id),
            username: format!("user{id}"),
            display_name: None,
            bot: false,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = ProfileCache::default();
        let profile = make_profile(1);
        cache.insert(profile.clone());
        let got = cache.get(Id::new(1));
        assert!(got.is_some());
        assert_eq!(got.unwrap().username, "user1");
    }

    #[test]
    fn test_needs_fetch_when_missing() {
        let cache = ProfileCache::default();
        assert!(cache.needs_fetch(Id::new(42)));
    }

    #[test]
    fn test_expired_profile_needs_fetch() {
        let mut cache = ProfileCache::default();
        let profile = make_profile(2);
        let user_id = profile.user_id;
        cache.insert(profile);

        // Manually set the fetched_at to be expired
        let entry = cache.cache.get_mut(&user_id).unwrap();
        entry.fetched_at = Instant::now() - PROFILE_TTL - Duration::from_secs(1);

        assert!(cache.needs_fetch(user_id));
    }

    #[test]
    fn test_bounded_cache_evicts_oldest_when_full() {
        let mut cache = ProfileCache::default();
        // Fill cache to MAX_CACHE_SIZE
        for i in 1..=(MAX_CACHE_SIZE as u64) {
            cache.insert(make_profile(i));
        }
        assert_eq!(cache.cache.len(), MAX_CACHE_SIZE);

        // Manually backdate the first entry so it is the "oldest"
        let entry = cache.cache.get_mut(&Id::new(1)).unwrap();
        entry.fetched_at = Instant::now() - Duration::from_secs(600);

        // Insert one more entry to trigger eviction
        cache.insert(make_profile(MAX_CACHE_SIZE as u64 + 1));

        // Cache should still be at capacity
        assert_eq!(cache.cache.len(), MAX_CACHE_SIZE);
        // The oldest entry (id=1) should have been evicted
        assert!(cache.cache.get(&Id::new(1)).is_none());
        // The newly inserted entry should be present
        assert!(cache.cache.get(&Id::new(MAX_CACHE_SIZE as u64 + 1)).is_some());
    }

    #[test]
    fn test_cleanup_removes_expired_entries() {
        let mut cache = ProfileCache::default();
        cache.insert(make_profile(1));
        cache.insert(make_profile(2));

        // Expire entry 1
        let entry = cache.cache.get_mut(&Id::new(1)).unwrap();
        entry.fetched_at = Instant::now() - PROFILE_TTL - Duration::from_secs(1);

        cache.cleanup();

        assert!(cache.cache.get(&Id::new(1)).is_none());
        assert!(cache.cache.get(&Id::new(2)).is_some());
    }

    #[test]
    fn test_updating_existing_entry_does_not_evict() {
        let mut cache = ProfileCache::default();
        // Fill cache to MAX_CACHE_SIZE
        for i in 1..=(MAX_CACHE_SIZE as u64) {
            cache.insert(make_profile(i));
        }
        // Re-insert an existing entry (update, not new key)
        cache.insert(make_profile(1));
        // Size should remain the same
        assert_eq!(cache.cache.len(), MAX_CACHE_SIZE);
    }
}
