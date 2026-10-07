//! Session-local decoded avatars and request generations for asynchronous updates.

use std::collections::{HashMap, HashSet};
use gtk::gdk::Texture;
use uuid::Uuid;

use crate::models::UserProfile;
use super::texture_from_base64;

#[derive(Default)]
pub struct AvatarCache {
    pub textures: HashMap<Uuid, Texture>,
    loaded: HashSet<Uuid>,
    pending: HashMap<Uuid, Uuid>,
}

impl AvatarCache {
    /// The signed-in profile already includes its picture from whoami.
    pub fn seed(&mut self, id: Uuid, blob: Option<&str>) {
        self.pending.remove(&id);
        self.loaded.insert(id);
        let texture = blob.filter(|blob| !blob.is_empty()).and_then(texture_from_base64);
        if let Some(texture) = texture { self.textures.insert(id, texture); }
        else { self.textures.remove(&id); }
    }

    pub fn begin(&mut self, ids: &[Uuid], force: bool) -> Option<(Uuid, Vec<Uuid>)> {
        let request_id = Uuid::new_v4();
        let mut seen = HashSet::new();
        let ids: Vec<_> = ids.iter().copied().filter(|id| {
            seen.insert(*id) && (force || (!self.loaded.contains(id) && !self.pending.contains_key(id)))
        }).collect();
        if ids.is_empty() { return None; }
        for id in &ids { self.pending.insert(*id, request_id); }
        Some((request_id, ids))
    }

    /// A newer avatar_update supersedes any older initial/reconnect response.
    /// Omitted profiles and null/invalid blobs clear the picture to its fallback.
    pub fn finish(&mut self, request_id: Uuid, ids: &[Uuid], profiles: Vec<UserProfile>) -> bool {
        let profiles: HashMap<_, _> = profiles.into_iter().map(|profile| (profile.id, profile)).collect();
        let mut changed = false;
        for id in ids {
            if self.pending.get(id) != Some(&request_id) { continue; }
            self.pending.remove(id);
            self.seed(*id, profiles.get(id).and_then(|profile| profile.avatar_blob.as_deref()));
            changed = true;
        }
        changed
    }

    pub fn fail(&mut self, request_id: Uuid, ids: &[Uuid]) {
        for id in ids {
            if self.pending.get(id) == Some(&request_id) { self.pending.remove(id); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_pending_and_loaded_members_do_not_refetch_profiles() {
        let id = Uuid::new_v4();
        let mut cache = AvatarCache::default();
        let (request, ids) = cache.begin(&[id, id], false).unwrap();
        assert_eq!(ids, vec![id]);
        assert!(cache.begin(&[id], false).is_none());
        assert!(cache.finish(request, &ids, Vec::new()));
        assert!(cache.begin(&[id], false).is_none());
        assert!(cache.textures.is_empty());
        assert!(cache.begin(&[id], true).is_some());
    }

    #[test]
    fn avatar_update_supersedes_old_profile_results_and_errors() {
        let id = Uuid::new_v4();
        let mut cache = AvatarCache::default();
        let (old, ids) = cache.begin(&[id], false).unwrap();
        let (new, _) = cache.begin(&[id], true).unwrap();
        cache.fail(old, &ids);
        assert!(!cache.finish(old, &ids, Vec::new()));
        assert_eq!(cache.pending.get(&id), Some(&new));
        assert!(cache.finish(new, &ids, Vec::new()));
        assert!(!cache.finish(old, &ids, Vec::new()));
    }

    #[test]
    fn failed_fetch_can_retry_and_caches_are_isolated_per_session() {
        let id = Uuid::new_v4();
        let mut cache = AvatarCache::default();
        let (request, ids) = cache.begin(&[id], false).unwrap();
        cache.fail(request, &ids);
        let (retry, ids) = cache.begin(&[id], false).unwrap();
        assert!(cache.finish(retry, &ids, Vec::new()));
        assert!(AvatarCache::default().begin(&[id], false).is_some());
    }
}
