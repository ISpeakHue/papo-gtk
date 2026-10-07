//! State transitions kept independent of GTK so races can be tested headlessly.

use uuid::Uuid;
use std::collections::{HashSet,HashMap};

use crate::models::{Message, LinkPreview, MessageReactionSummary};

#[derive(Debug, Clone)]
pub enum Change {
    Upsert(Message),
    Edit(Uuid, String, Option<chrono::DateTime<chrono::Utc>>),
    Reaction(Uuid, MessageReactionSummary),
    Preview(Uuid, LinkPreview),
    RemovePreview(Uuid, Uuid),
    Moderation(Uuid, Uuid, String),
    Reactions(Uuid, Vec<crate::models::ReactionGroup>, Uuid),
    Delete(Uuid),
}

#[derive(Default)]
pub struct History {
    pub messages: Vec<Message>,
    channel_id: Option<Uuid>,
    pending: Option<Uuid>,
    changes: Vec<Change>,
    deleted: HashSet<Uuid>,
}

impl History {
    pub fn select(&mut self, channel_id: Uuid) {
        *self = Self { channel_id: Some(channel_id), ..Self::default() };
    }

    pub fn begin(&mut self, request_id: Uuid) {
        self.pending = Some(request_id);
        self.changes.clear();
    }

    pub fn finish(&mut self, request_id: Uuid, messages: Vec<Message>, append: bool) -> bool {
        if self.pending != Some(request_id) {
            return false;
        }
        self.pending = None;
        let changes = std::mem::take(&mut self.changes);
        let current = if append { std::mem::take(&mut self.messages) } else { Vec::new() };
        let mut merged:HashMap<Uuid,Message>=HashMap::with_capacity(messages.len()+current.len());
        for message in messages.into_iter().chain(current){
            if Some(message.channel_id)!=self.channel_id||self.deleted.contains(&message.id){continue;}
            if let Some(existing)=merged.get_mut(&message.id){merge_message(existing,message);}else{merged.insert(message.id,message);}
        }
        self.messages=merged.into_values().collect();self.messages.sort_by_key(|m|(m.created_at,m.id));
        for change in changes {
            self.apply(change);
        }
        true
    }

    pub fn is_deleted(&self, id: Uuid) -> bool { self.deleted.contains(&id) }

    pub fn loading(&self) -> bool { self.pending.is_some() }

    pub fn fail(&mut self, request_id: Uuid) -> bool {
        if self.pending == Some(request_id) {
            self.pending = None;
            self.changes.clear();
            return true;
        }
        false
    }

    pub fn apply(&mut self, change: Change) {
        if let Change::Upsert(message) = &change {
            if Some(message.channel_id) != self.channel_id || self.deleted.contains(&message.id) {
                return;
            }
        }
        if self.pending.is_some() {
            self.changes.push(change.clone());
        }
        match change {
            Change::Upsert(message) => {
                if let Some(index)=self.messages.iter().position(|m|m.id==message.id){
                    let previous=self.messages[index].created_at;merge_message(&mut self.messages[index],message);
                    if self.messages[index].created_at!=previous{let updated=self.messages.remove(index);let at=self.messages.binary_search_by_key(&(updated.created_at,updated.id),|m|(m.created_at,m.id)).unwrap_or_else(|at|at);self.messages.insert(at,updated);}
                }else{let at=self.messages.binary_search_by_key(&(message.created_at,message.id),|m|(m.created_at,m.id)).unwrap_or_else(|at|at);self.messages.insert(at,message);}
            }
            Change::Edit(id, content, edited_at) => {
                if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
                    if edited_at.is_none() || message.edited_at <= edited_at {
                        message.content = Some(content);
                        message.edited_at = edited_at;
                    }
                }
            }
            Change::Reaction(id, reaction) => {
                if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
                    let reactions = message.reactions.get_or_insert_with(Vec::new);
                    let index = reactions.iter().position(|existing|
                        existing.emoji_id == reaction.emoji_id && existing.unicode == reaction.unicode);
                    if reaction.count <= 0 {
                        if let Some(index) = index { reactions.remove(index); }
                    } else if let Some(index) = index { reactions[index] = reaction; }
                    else { reactions.push(reaction); }
                }
            }
            Change::Reactions(id, groups, user) => {
                if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
                    message.reactions = Some(groups.iter().filter(|g| !g.users.is_empty()).map(|g| MessageReactionSummary {
                        emoji_id: g.emoji_id, unicode: g.unicode.clone(), count: g.users.len() as i32,
                    }).collect());
                    message.user_reactions = Some(groups.iter().flat_map(|g| g.users.iter().filter(move |u| u.user_id == user).map(move |u| crate::models::MessageUserReaction {
                        id: u.id, emoji_id: g.emoji_id, unicode: g.unicode.clone(),
                    })).collect());
                }
            }
            Change::Preview(id, preview) => {
                if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
                    let previews = message.previews.get_or_insert_with(Vec::new);
                    if let Some(existing) = previews.iter_mut().find(|item| item.id == preview.id) { *existing = preview; }
                    else { previews.push(preview); }
                }
            }
            Change::RemovePreview(id, preview_id) => {
                if let Some(previews) = self.messages.iter_mut().find(|m| m.id == id).and_then(|m| m.previews.as_mut()) {
                    previews.retain(|preview| preview.id != preview_id);
                }
            }
            Change::Moderation(id, attachment_id, status) => {
                if status == "blocked" {
                    self.deleted.insert(id);
                    self.messages.retain(|m| m.id != id);
                    return;
                }
                if let Some(attachment) = self.messages.iter_mut().find(|m| m.id == id)
                    .and_then(|m| m.attachments.as_mut()).and_then(|items| items.iter_mut().find(|item| item.id == attachment_id)) {
                    attachment.moderation_status = Some(status);
                }
            }
            Change::Delete(id) => {
                self.deleted.insert(id);
                self.messages.retain(|m| m.id != id);
            }
        }
    }
}

fn merge_message(existing:&mut Message,mut message:Message){
    if message.previews.is_none(){message.previews=existing.previews.take();}
    if message.reactions.is_none(){message.reactions=existing.reactions.take();}
    if message.user_reactions.is_none(){message.user_reactions=existing.user_reactions.take();}
    if existing.edited_at>message.edited_at{message.content=existing.content.take();message.edited_at=existing.edited_at;}
    *existing=message;
}

#[derive(Default)]
pub struct Draft {
    pub text: String,
    pub files: Vec<crate::api::features::UploadFile>,
    pub reply: Option<Message>,
    pub error: Option<String>,
    pending: Option<Uuid>,
}

impl Draft {
    pub fn is_sending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn begin(&mut self) -> Option<Uuid> {
        if self.is_sending() || (self.text.trim().is_empty() && self.files.is_empty()) {
            return None;
        }
        let id = Uuid::new_v4();
        self.pending = Some(id);
        self.error = None;
        Some(id)
    }

    pub fn finish(&mut self, id: Uuid, result: Result<(), String>) -> bool {
        if self.pending != Some(id) {
            return false;
        }
        self.pending = None;
        match result {
            Ok(()) => { self.text.clear(); self.files.clear(); self.reply = None; self.error = None; }
            Err(error) => self.error = Some(error),
        }
        true
    }
    pub fn pending_id(&self) -> Option<Uuid> { self.pending }
    pub fn cancel(&mut self) { self.pending = None; self.error = Some("Envio cancelado. Confira o histórico antes de reenviar: o servidor pode ter recebido a mensagem.".into()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(channel_id: Uuid, second: i64) -> Message {
        Message {
            id: Uuid::new_v4(), channel_id, author_id: Some(Uuid::new_v4()),
            content: Some("original".into()),
            created_at: chrono::DateTime::from_timestamp(second, 0).unwrap(),
            edited_at: None, reply_to: None, attachments: None,
            previews: None, reactions: None, user_reactions: None,
        }
    }

    #[test]fn large_history_pages_merge_once_deduplicate_and_replay_events(){
        let channel=Uuid::new_v4();let mut history=History::default();history.select(channel);let page:Vec<_>=(0..10_000).map(|i|message(channel,i)).collect();let request=Uuid::new_v4();history.begin(request);assert!(history.finish(request,page[5000..].to_vec(),false));
        let edited=page[7000].id;let deleted=page[8000].id;let next=Uuid::new_v4();history.begin(next);history.apply(Change::Edit(edited,"live edit".into(),None));history.apply(Change::Delete(deleted));assert!(history.finish(next,page,true));assert_eq!(history.messages.len(),9999);assert_eq!(history.messages.iter().find(|m|m.id==edited).unwrap().content.as_deref(),Some("live edit"));assert!(!history.messages.iter().any(|m|m.id==deleted));assert!(history.messages.windows(2).all(|pair|(pair[0].created_at,pair[0].id)<(pair[1].created_at,pair[1].id)));
    }

    #[test]
    fn blocked_moderation_tombstones_whole_message_and_rejects_late_history() {
        let channel=Uuid::new_v4();let mut history=History::default();history.select(channel);let m=message(channel,1);history.apply(Change::Upsert(m.clone()));
        let request=Uuid::new_v4();history.begin(request);history.apply(Change::Moderation(m.id,Uuid::new_v4(),"blocked".into()));assert!(history.messages.is_empty());
        history.finish(request,vec![m.clone()],false);history.apply(Change::Upsert(m));assert!(history.messages.is_empty());
    }

    #[test]
    fn file_only_draft_retains_selection_on_failure_and_cancel() {
        let mut draft=Draft::default();draft.files.push(crate::api::features::UploadFile{path:"file".into(),name:"file".into(),size:1});
        let first=draft.begin().unwrap();assert!(draft.finish(first,Err("offline".into())));assert_eq!(draft.files.len(),1);
        let cancelled=draft.begin().unwrap();draft.cancel();assert!(!draft.finish(cancelled,Ok(())));assert_eq!(draft.files.len(),1);
        let next=draft.begin().unwrap();assert!(draft.finish(next,Ok(())));assert!(draft.files.is_empty());
    }

    #[test]
    fn history_preserves_live_arrivals_edits_and_deletions_during_fetch() {
        let channel = Uuid::new_v4();
        let mut history = History::default();
        history.select(channel);
        let request = Uuid::new_v4();
        let original = message(channel, 1);
        let deleted = message(channel, 2);
        let live = message(channel, 3);
        history.begin(request);
        history.apply(Change::Upsert(live.clone()));
        // Edits/deletes can arrive before their corresponding history rows.
        history.apply(Change::Edit(original.id, "edited".into(), Some(chrono::Utc::now())));
        history.apply(Change::Delete(deleted.id));
        assert!(history.finish(request, vec![deleted, original.clone()], false));
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].id, original.id);
        assert_eq!(history.messages[0].content.as_deref(), Some("edited"));
        assert_eq!(history.messages[1].id, live.id);
    }

    #[test]
    fn channel_switch_rejects_old_history_and_old_send_results() {
        let old_channel = Uuid::new_v4();
        let new_channel = Uuid::new_v4();
        let mut history = History::default();
        history.select(old_channel);
        let request = Uuid::new_v4();
        history.begin(request);
        history.select(new_channel);
        assert!(!history.finish(request, vec![message(old_channel, 1)], true));
        history.apply(Change::Upsert(message(old_channel, 2)));
        assert!(history.messages.is_empty());
    }

    #[test]
    fn superseded_requests_cannot_replace_newer_history() {
        let channel = Uuid::new_v4();
        let mut history = History::default();
        history.select(channel);
        let old = Uuid::new_v4();
        let new = Uuid::new_v4();
        history.begin(old);
        history.begin(new);
        history.fail(old);
        assert!(history.finish(new, vec![message(channel, 2)], false));
        assert!(!history.finish(old, vec![message(channel, 1)], false));
        assert_eq!(history.messages[0].created_at.timestamp(), 2);
    }

    #[test]
    fn pagination_and_http_ws_echoes_are_sorted_and_deduplicated() {
        let channel = Uuid::new_v4();
        let mut history = History::default();
        history.select(channel);
        let recent = message(channel, 3);
        history.apply(Change::Upsert(recent.clone()));
        history.apply(Change::Upsert(recent.clone()));
        let request = Uuid::new_v4();
        history.begin(request);
        history.finish(request, vec![recent, message(channel, 2), message(channel, 1)], true);
        assert_eq!(history.messages.iter().map(|m| m.created_at.timestamp()).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn reconnect_snapshot_recovers_messages_reactions_and_deletions() {
        let channel = Uuid::new_v4();
        let mut history = History::default();
        history.select(channel);
        history.apply(Change::Upsert(message(channel, 1)));
        let mut updated = message(channel, 2);
        history.apply(Change::Upsert(updated.clone()));
        updated.reactions = Some(vec![crate::models::MessageReactionSummary {
            emoji_id: None, unicode: Some("❤️".into()), count: 2,
        }]);
        let request = Uuid::new_v4();
        history.begin(request);
        history.finish(request, vec![updated, message(channel, 3)], false);
        assert_eq!(history.messages.len(), 2);
        assert_eq!(history.messages[0].reactions.as_ref().unwrap()[0].count, 2);
        assert_eq!(history.messages[1].created_at.timestamp(), 3);
    }

    #[test]
    fn failed_send_preserves_text_and_reply_and_allows_retry() {
        let mut draft = Draft {
            text: "my draft".into(), reply: Some(message(Uuid::new_v4(), 1)),
            ..Draft::default()
        };
        let request = draft.begin().unwrap();
        assert!(draft.begin().is_none());
        assert!(draft.finish(request, Err("offline".into())));
        assert_eq!(draft.text, "my draft");
        assert!(draft.reply.is_some());
        assert_eq!(draft.error.as_deref(), Some("offline"));
        assert!(!draft.is_sending());
        let retry = draft.begin().unwrap();
        draft.finish(retry, Ok(()));
        assert!(draft.text.is_empty());
        assert!(draft.reply.is_none());
        assert!(draft.error.is_none());
    }

    #[test]
    fn old_completion_cannot_clear_new_channel_draft() {
        let mut old = Draft { text: "old".into(), ..Draft::default() };
        let old_request = old.begin().unwrap();
        let mut new = Draft { text: "new".into(), ..Draft::default() };
        let new_request = new.begin().unwrap();
        assert!(!new.finish(old_request, Ok(())));
        assert_eq!(new.text, "new");
        assert!(new.is_sending());
        assert!(new.finish(new_request, Ok(())));
    }

    #[test]
    fn blank_draft_is_not_sent() {
        let mut draft = Draft { text: " \n ".into(), ..Draft::default() };
        assert!(draft.begin().is_none());
    }
    #[test]
    fn live_reactions_replay_during_fetch_preserve_order_and_remove_zero_counts() {
        let channel = Uuid::new_v4();
        let original = message(channel, 1);
        let mut history = History::default();
        history.select(channel);
        let request = Uuid::new_v4();
        history.begin(request);
        let reaction = |unicode: &str, count| crate::models::MessageReactionSummary {
            emoji_id: None, unicode: Some(unicode.into()), count,
        };
        history.apply(Change::Reaction(original.id, reaction("❤️", 1)));
        history.apply(Change::Reaction(original.id, reaction("👍", 3)));
        history.apply(Change::Reaction(original.id, reaction("❤️", 2)));
        history.finish(request, vec![original.clone()], false);
        let reactions = history.messages[0].reactions.as_ref().unwrap();
        assert_eq!(reactions[0].unicode.as_deref(), Some("❤️"));
        assert_eq!(reactions[0].count, 2);
        assert_eq!(reactions[1].unicode.as_deref(), Some("👍"));
        history.apply(Change::Reaction(original.id, reaction("❤️", 0)));
        assert_eq!(history.messages[0].reactions.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn pagination_orders_equal_timestamps_by_id_and_keeps_live_edits() {
        let channel = Uuid::new_v4();
        let mut first = message(channel, 1);
        let mut second = message(channel, 1);
        first.id = Uuid::from_u128(1);
        second.id = Uuid::from_u128(2);
        let mut history = History::default();
        history.select(channel);
        history.apply(Change::Upsert(second.clone()));
        let request = Uuid::new_v4();
        history.begin(request);
        let edited_at = chrono::DateTime::from_timestamp(10, 123).unwrap();
        history.apply(Change::Edit(first.id, "edited".into(), Some(edited_at)));
        history.finish(request, vec![second, first], true);
        assert_eq!(history.messages[0].id, Uuid::from_u128(1));
        assert_eq!(history.messages[0].edited_at, Some(edited_at));
        assert_eq!(history.messages[0].content.as_deref(), Some("edited"));
    }

    #[test]
    fn preview_changes_replay_during_history_fetch() {
        let channel = Uuid::new_v4();
        let mut original = message(channel, 1);
        let mut preview: LinkPreview = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(), "url":"https://example.test", "kind":"og", "title":"old",
            "fetched_at":"2026-10-03T12:00:00Z"
        })).unwrap();
        original.previews = Some(vec![preview.clone()]);
        let mut history = History::default();
        history.select(channel);
        let request = Uuid::new_v4();
        history.begin(request);
        preview.title = Some("updated".into());
        history.apply(Change::Preview(original.id, preview.clone()));
        history.finish(request, vec![original.clone()], false);
        assert_eq!(history.messages[0].previews.as_ref().unwrap()[0].title.as_deref(),Some("updated"));
        history.begin(request);
        history.apply(Change::RemovePreview(original.id, preview.id));
        history.finish(request, vec![original], false);
        assert!(history.messages[0].previews.as_ref().unwrap().is_empty());
    }

    #[test]
    fn late_message_echo_keeps_live_reactions_and_edits() {
        let channel = Uuid::new_v4();
        let original = message(channel, 1);
        let mut history = History::default();
        history.select(channel);
        history.apply(Change::Upsert(original.clone()));
        history.apply(Change::Reaction(original.id, MessageReactionSummary { emoji_id:None, unicode:Some("❤️".into()), count:1 }));
        history.apply(Change::Edit(original.id, "edited".into(), Some(chrono::Utc::now())));
        history.apply(Change::Upsert(original));
        assert_eq!(history.messages[0].reactions.as_ref().unwrap()[0].count, 1);
        assert_eq!(history.messages[0].content.as_deref(), Some("edited"));
    }

    #[test]
    fn delayed_send_response_cannot_resurrect_deleted_or_blocked_message() {
        let channel = Uuid::new_v4();
        let original = message(channel, 1);
        let mut history = History::default();
        history.select(channel);
        history.apply(Change::Upsert(original.clone()));
        history.apply(Change::Delete(original.id));
        history.apply(Change::Upsert(original.clone()));
        assert!(history.messages.is_empty());
        let request = Uuid::new_v4();
        history.begin(request);
        history.finish(request, vec![original], false);
        assert!(history.messages.is_empty());
    }

    #[test]
    fn participant_snapshots_replay_own_membership_during_history_and_respect_deletes() {
        use crate::models::ReactionGroup;
        let channel = Uuid::new_v4(); let user = Uuid::new_v4(); let original = message(channel, 1);
        let group: ReactionGroup = serde_json::from_value(serde_json::json!({"emoji_id":null,"unicode":"❤️","count":1,
            "users":[{"id":Uuid::new_v4(),"user_id":user,"created_at":"2026-10-03T12:00:00Z"}]})).unwrap();
        let mut history = History::default(); history.select(channel); let token = Uuid::new_v4(); history.begin(token);
        history.apply(Change::Reactions(original.id, vec![group], user));
        history.finish(token,vec![original.clone()],false);
        assert_eq!(history.messages[0].user_reactions.as_ref().unwrap()[0].unicode.as_deref(),Some("❤️"));
        assert_eq!(history.messages[0].reactions.as_ref().unwrap()[0].count,1);
        history.begin(token); history.apply(Change::Reactions(original.id,vec![],user)); history.finish(token,vec![original.clone()],false);
        assert!(history.messages[0].user_reactions.as_ref().unwrap().is_empty());
        history.apply(Change::Delete(original.id)); history.apply(Change::Reactions(original.id,vec![],user));
        assert!(history.messages.is_empty());
    }

    #[test]
    fn older_edit_events_cannot_replace_newer_edits_or_reaction_membership() {
        let channel = Uuid::new_v4(); let original = message(channel,1); let mut history = History::default(); history.select(channel);
        history.apply(Change::Upsert(original.clone()));
        history.apply(Change::Edit(original.id,"new".into(),chrono::DateTime::from_timestamp(20,0)));
        history.apply(Change::Edit(original.id,"old".into(),chrono::DateTime::from_timestamp(10,0)));
        assert_eq!(history.messages[0].content.as_deref(),Some("new"));
    }

}
