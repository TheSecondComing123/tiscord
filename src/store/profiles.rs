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
        self.cache.insert(
            user_id,
            CachedProfile {
                profile,
                fetched_at: Instant::now(),
            },
        );
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
}
