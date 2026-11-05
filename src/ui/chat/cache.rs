//! EN-US: Recent conversations belong to one authenticated chat session, never to disk.
//! PT-BR: Conversas recentes pertencem a uma única sessão autenticada e não são persistidas em disco.
use std::collections::VecDeque;
use uuid::Uuid;
use super::state::{Change, History};

#[derive(Default)]
pub(super) struct AccountConversations {
    pub owner: Option<crate::account::AccountId>,
    pub drafts: std::collections::HashMap<Uuid, super::state::Draft>,
    pub conversations: Conversations,
    pub surfaces: super::surface_cache::Surfaces,
}
impl AccountConversations {
    pub fn bind(&mut self, owner: Option<crate::account::AccountId>) -> bool {
        if self.owner == owner { return false; }
        *self = Self { owner, ..Default::default() };
        true
    }
}

const CHANNEL_LIMIT: usize = 8;
pub(super) const MESSAGE_LIMIT: usize = 200;
const BYTE_LIMIT: usize = 16 << 20;

#[derive(Default)]
pub(super) struct Conversations {
    entries: VecDeque<(Uuid, History, bool, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Message;
    #[test]
    fn ownership_separates_identical_channel_ids_and_preserves_relogin_drafts() {
        let channel=Uuid::new_v4();let user=Uuid::new_v4();
        let api=crate::api::ApiClient::new("https://one.test/api").unwrap();
        let owner=crate::account::AccountId::new(&api,user);
        let mut state=AccountConversations::default();assert!(state.bind(Some(owner.clone())));
        let mut draft=super::super::state::Draft::default();draft.text="unfinished".into();state.drafts.insert(channel,draft);
        state.conversations.store(channel,history(channel,1),false);
        let relogin=crate::api::ApiClient::new("https://one.test/api/").unwrap();
        assert!(!state.bind(Some(crate::account::AccountId::new(&relogin,user))));
        assert_eq!(state.drafts[&channel].text,"unfinished");
        assert!(state.conversations.take(channel).is_some());
        let other=crate::account::AccountId::new(&crate::api::ApiClient::new("https://two.test/api/").unwrap(),user);
        assert!(state.bind(Some(other)));assert!(state.drafts.is_empty());assert!(state.conversations.take(channel).is_none());
        state.drafts.insert(channel,Default::default());assert!(state.bind(Some(owner)));assert!(state.drafts.is_empty());
        assert!(state.bind(None));assert!(state.drafts.is_empty());
    }
    fn history(channel: Uuid, count: usize) -> History {
        let mut history=History::default(); history.select(channel);
        for i in 0..count {
            history.messages.push(serde_json::from_value(serde_json::json!({
                "id":Uuid::new_v4(),"channel_id":channel,"content":format!("message {i}"),
                "created_at":chrono::DateTime::from_timestamp(i as i64,0).unwrap()
            })).unwrap());
        }
        history
    }
    #[test]
    fn retains_recent_pages_and_evicts_least_recent_conversations() {
        let mut cache=Conversations::default(); let first=Uuid::new_v4();
        cache.store(first,history(first,300),false);
        let (retained,more)=cache.take(first).unwrap();
        assert_eq!(retained.messages.len(),200); assert!(more);
        assert_eq!(retained.messages[0].content.as_deref(),Some("message 100"));
        cache.store(first,retained,more);
        for _ in 0..CHANNEL_LIMIT { let id=Uuid::new_v4(); cache.store(id,history(id,1),false); }
        assert!(cache.take(first).is_none()); assert_eq!(cache.entries.len(),CHANNEL_LIMIT);
    }
    #[test]
    fn rejects_oversized_pages_and_applies_permission_and_live_changes() {
        let mut cache=Conversations::default(); let a=Uuid::new_v4(); let b=Uuid::new_v4();
        let original=history(a,1); let message=original.messages[0].id;
        cache.store(a,original,false); cache.store(b,history(b,1),false);
        cache.apply(&Change::Delete(message)); let (retained,_) = cache.take(a).unwrap(); assert!(retained.messages.is_empty());
        cache.retain(&[]); assert!(cache.take(b).is_none());
        let mut huge=history(a,1); huge.messages[0].content=Some("x".repeat(BYTE_LIMIT+1));
        cache.store(a,huge,false); assert!(cache.take(a).is_none());
    }
    #[test]
    fn live_updates_keep_cache_hits_and_tombstones_without_promoting_idle_channels() {
        let mut cache=Conversations::default(); let a=Uuid::new_v4(); let b=Uuid::new_v4();
        let original=history(a,1); let old=original.messages[0].clone();
        cache.store(a,original,false); cache.store(b,history(b,1),false);
        cache.apply(&Change::Edit(old.id,"live edit".into(),None));
        assert_eq!(cache.entries.front().unwrap().0,a,"live traffic must not promote an unvisited channel");
        let mut new=old.clone(); new.id=Uuid::new_v4(); new.created_at+=chrono::Duration::seconds(1);
        cache.apply(&Change::Upsert(new.clone())); cache.apply(&Change::Delete(old.id));
        cache.apply(&Change::Upsert(old));
        let (retained,_)=cache.take(a).unwrap(); assert_eq!(retained.messages,vec![new]);
        // The aggregate metadata budget also evicts across several channels.
        for _ in 0..3 { let id=Uuid::new_v4(); let mut page=history(id,1); page.messages[0].content=Some("x".repeat(6<<20)); cache.store(id,page,false); }
        assert!(cache.entries.iter().map(|e|e.3).sum::<usize>()<=BYTE_LIMIT);
        assert_eq!(cache.entries.len(),2);
    }
    #[test]
    fn cancelled_requests_cannot_replace_restored_history() {
        let mut cache=Conversations::default(); let channel=Uuid::new_v4(); let mut original=history(channel,1);
        let stale=Uuid::new_v4(); original.begin(stale); cache.store(channel,original,false);
        let (mut restored,_)=cache.take(channel).unwrap();
        assert!(!restored.finish(stale,Vec::<Message>::new(),false)); assert_eq!(restored.messages.len(),1);
        let refresh=Uuid::new_v4(); restored.begin_snapshot(refresh);
        let old=restored.messages[0].clone(); restored.apply(super::super::state::Change::Delete(old.id));
        assert!(restored.finish_reconcile(refresh,vec![old],true)); assert!(restored.messages.is_empty());
    }
}

impl Conversations {
    #[cfg(test)]pub(super) fn entries_empty(&self)->bool{self.entries.is_empty()}
    pub fn store(&mut self, channel: Uuid, mut history: History, has_more: bool) {
        self.entries.retain(|(id, ..)| *id != channel);
        let trimmed = history.prepare_cache(MESSAGE_LIMIT);
        let bytes = history.cache_bytes();
        if bytes > BYTE_LIMIT || history.deleted_count() > MESSAGE_LIMIT * 2 { return; }
        self.entries.push_back((channel, history, has_more || trimmed, bytes));
        self.enforce_limits();
    }

    fn enforce_limits(&mut self) {
        while self.entries.len() > CHANNEL_LIMIT || self.entries.iter().map(|e| e.3).sum::<usize>() > BYTE_LIMIT {
            self.entries.pop_front();
        }
    }

    pub fn apply(&mut self, change: &Change) {
        let index=self.entries.iter().position(|(channel,history,..)|match change {
            Change::Upsert(message) => *channel==message.channel_id,
            Change::Edit(id,..) | Change::Reaction(id,..) | Change::Reactions(id,..) | Change::Preview(id,..) | Change::Embeds(id,..) | Change::RemovePreview(id,..) | Change::Moderation(id,..) | Change::Delete(id) => history.messages.iter().any(|m|m.id==*id),
        });
        if let Some(index)=index {
            let entry=&mut self.entries[index];
            entry.1.apply(change.clone());
            entry.2 |= entry.1.prepare_cache(MESSAGE_LIMIT);
            entry.3=entry.1.cache_bytes();
            if entry.3 > BYTE_LIMIT || entry.1.deleted_count() > MESSAGE_LIMIT * 2 { self.entries.remove(index); }
            self.enforce_limits();
        }
    }

    pub fn take(&mut self, channel: Uuid) -> Option<(History, bool)> {
        let index = self.entries.iter().position(|e| e.0 == channel)?;
        let (_, history, has_more, _) = self.entries.remove(index)?;
        Some((history, has_more))
    }

    pub fn clear(&mut self) { self.entries.clear(); }
    pub fn retain(&mut self, allowed: &[Uuid]) { self.entries.retain(|e| allowed.contains(&e.0)); }
    pub fn invalidate_channel(&mut self, channel: Uuid) { self.entries.retain(|e| e.0 != channel); }
}
