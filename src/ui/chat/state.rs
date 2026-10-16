//! State transitions kept independent of GTK so races can be tested headlessly.

use uuid::Uuid;
use std::collections::{HashSet,HashMap,VecDeque};

use crate::models::{Message, Embed, MessageReactionSummary};

#[derive(Debug, Clone)]
pub enum Change {
    Upsert(Message),
    Edit(Uuid, String, Option<chrono::DateTime<chrono::Utc>>),
    Reaction(Uuid, MessageReactionSummary),
    Preview(Uuid, Embed),
    Embeds(Uuid, Vec<Embed>),
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
    snapshots: HashMap<Uuid, Vec<Change>>,
    dirty:HashSet<Uuid>,
    media_revision:u64,
    early_moderation:VecDeque<(Uuid,Uuid,String)>,
}

impl History {
    // Unpinning hides a message without tombstoning it: it can be pinned again.
    pub(super) fn retain_pins(&mut self, ids: &HashSet<Uuid>) {
        self.messages.retain(|m|ids.contains(&m.id));self.dirty.retain(|id|ids.contains(id));
        self.media_revision=self.media_revision.wrapping_add(1);
    }
    pub(super) fn trim_recent(&mut self,limit:usize)->bool {
        if self.messages.len()<=limit{return false;}
        self.messages.drain(..self.messages.len()-limit);self.messages.shrink_to_fit();
        // A late older-page response must not repopulate the released history.
        self.pending=None;self.changes=Default::default();
        let ids:HashSet<_>=self.messages.iter().map(|m|m.id).collect();self.dirty.retain(|id|ids.contains(id));
        self.media_revision=self.media_revision.wrapping_add(1);true
    }
    pub(super) fn prepare_cache(&mut self, limit: usize) -> bool {
        self.pending = None; self.changes=Default::default(); self.snapshots=Default::default(); self.dirty=Default::default();
        let trimmed = self.messages.len() > limit;
        if trimmed { self.messages.drain(..self.messages.len() - limit); }
        // Encoded images live in the separately bounded media cache. Metadata
        // still lets an evicted thumbnail be fetched on a later visit.
        for embed in self.messages.iter_mut().flat_map(|m| m.embeds.iter_mut().flatten()) { embed.image_data = None; embed.author_image_data = None; }
        self.messages.shrink_to_fit();
        trimmed
    }
    pub(super) fn cache_bytes(&self) -> usize {
        // Account for allocated capacities without serializing messages on the
        // GTK thread when the user switches channels.
        let string=|s:&Option<String>|s.as_ref().map_or(0,String::capacity);
        let media=|m:&Option<crate::models::EmbedMedia>|m.as_ref().map_or(0,|m|string(&m.url)+string(&m.mime_type));
        self.early_moderation.capacity()*std::mem::size_of::<(Uuid,Uuid,String)>()+self.early_moderation.iter().map(|(_,_,status)|status.capacity()).sum::<usize>()+
        self.deleted.capacity()*std::mem::size_of::<Uuid>() + self.messages.capacity()*std::mem::size_of::<Message>() + self.messages.iter().map(|m| {
            m.external_author.as_ref().map_or(0,|a|a.source.capacity()+a.id.capacity()+a.name.capacity()+a.avatar_url.capacity()+a.profile_url.capacity()) + string(&m.content)
                + m.attachments.as_ref().map_or(0,|items|items.capacity()*std::mem::size_of::<crate::models::Attachment>() + items.iter().map(|a|a.mime_type.capacity()+a.original_file_name.capacity()+string(&a.moderation_status)).sum::<usize>())
                + m.embeds.as_ref().map_or(0,|items|items.capacity()*std::mem::size_of::<Embed>() + items.iter().map(|e| {
                    e.source_type.capacity()+e.fetch_method.capacity()+string(&e.provider)+string(&e.site_name)+string(&e.url)+string(&e.title)+string(&e.description)+string(&e.color)+string(&e.embed_url)+string(&e.image_data)+string(&e.author_image_data)
                        + media(&e.thumbnail)+media(&e.image)+media(&e.video)
                        + e.author.as_ref().map_or(0,|a|string(&a.name)+string(&a.url)+media(&a.media))
                        + e.footer.as_ref().map_or(0,|f|string(&f.text)+media(&f.icon))
                        + e.fields.capacity()*std::mem::size_of::<crate::models::EmbedField>()+e.fields.iter().map(|f|f.name.capacity()+f.value.capacity()).sum::<usize>()
                }).sum::<usize>())
                + m.reactions.as_ref().map_or(0,|items|items.capacity()*std::mem::size_of::<MessageReactionSummary>()+items.iter().map(|r|string(&r.unicode)).sum::<usize>())
                + m.user_reactions.as_ref().map_or(0,|items|items.capacity()*std::mem::size_of::<crate::models::MessageUserReaction>()+items.iter().map(|r|string(&r.unicode)).sum::<usize>())
        }).sum::<usize>()
    }
    pub fn media_revision(&self)->u64{self.media_revision}
    pub fn mark_dirty(&mut self,id:Uuid){self.dirty.insert(id);}
    pub fn take_dirty(&mut self)->HashSet<Uuid>{
        let mut dirty=std::mem::take(&mut self.dirty);if dirty.is_empty(){return dirty;}
        let changed=dirty.clone();
        for (index,m) in self.messages.iter().enumerate(){if changed.contains(&m.id){if let Some(next)=self.messages.get(index+1){dirty.insert(next.id);}}}
        // Use the original changes for reply dependencies; don't recursively
        // dirty the entire grouping chain after one changed row.
        for m in &self.messages{if m.reply_to.is_some_and(|id|changed.contains(&id)){dirty.insert(m.id);}}
        dirty
    }
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
        self.media_revision=self.media_revision.wrapping_add(1);
        let changes = std::mem::take(&mut self.changes);
        let current = if append { std::mem::take(&mut self.messages) } else { Vec::new() };
        let previous:HashMap<_,_>=self.messages.iter().map(|m|(m.id,m)).collect();
        let mut merged:HashMap<Uuid,Message>=HashMap::with_capacity(messages.len()+current.len());
        for mut message in current.into_iter().chain(messages.into_iter().map(snapshot_message)){
            if Some(message.channel_id)!=self.channel_id||self.deleted.contains(&message.id){continue;}
            if let Some(previous)=previous.get(&message.id){preserve_moderation(previous,&mut message);}
            if let Some(existing)=merged.get_mut(&message.id){merge_message(existing,message);}else{merged.insert(message.id,message);}
        }
        self.dirty.extend(merged.keys().copied());
        self.messages=merged.into_values().collect();self.messages.sort_by_key(|m|(m.created_at,m.id));
        self.replay_early_moderation();
        for change in changes {
            self.apply(change);
        }
        true
    }

    /// Independent journals let a read receipt coexist with history pagination.
    pub fn begin_snapshot(&mut self, request: Uuid) { self.snapshots.insert(request, Vec::new()); }
    pub fn fail_snapshot(&mut self, request: Uuid) { self.snapshots.remove(&request); }
    /// Polling supplies only attachment states, never stale message content.
    /// Events received during the request take precedence over its response.
    pub(super) fn finish_moderation(&mut self,request:Uuid,messages:Vec<Message>)->Vec<Change>{
        let Some(events)=self.snapshots.remove(&request) else{return Vec::new();};
        let mut changes=Vec::new();
        for fetched in messages.into_iter().filter(|m|Some(m.channel_id)==self.channel_id){
            let Some(current)=self.messages.iter().find(|m|m.id==fetched.id)else{continue;};
            for attachment in current.attachments.iter().flatten().filter(|a|matches!(a.moderation_status.as_deref(),Some("pending"|"processing"))){
                if events.iter().any(|e|matches!(e,Change::Moderation(id,key,_) if *id==current.id&&*key==attachment.id)||matches!(e,Change::Delete(id) if *id==current.id)){continue;}
                if let Some(fresh)=fetched.attachments.iter().flatten().find(|a|a.id==attachment.id){
                    // Some backend versions omit this column from history's
                    // attachment join. Treat the result exactly as reopening
                    // that history would, without claiming it is a clean verdict.
                    let status=fresh.moderation_status.as_deref().unwrap_or("");
                    if matches!(status,""|"clean"|"sensitive"|"blocked"|"failed"){
                        changes.push(Change::Moderation(current.id,attachment.id,status.into()));
                    }
                }
            }
        }
        changes
    }
    #[cfg(test)]
    pub fn finish_snapshot(&mut self, request: Uuid, messages: Vec<Message>) -> bool {
        let Some(changes) = self.snapshots.remove(&request) else { return false; };
        for message in messages { self.upsert(snapshot_message(message)); }
        for change in changes { self.apply(change); }
        true
    }
    pub fn finish_reconcile(&mut self,request:Uuid,messages:Vec<Message>,exhausted:bool)->bool{
        let Some(changes)=self.snapshots.remove(&request) else{return false;};
        let live:HashSet<_>=changes.iter().filter_map(|c|if let Change::Upsert(m)=c{Some(m.id)}else{None}).collect();
        let ids:HashSet<_>=messages.iter().map(|m|m.id).collect();
        let lower=if exhausted{None}else{messages.iter().map(|m|(m.created_at,m.id)).min()};
        let deleted:Vec<_>=self.messages.iter().filter(|m|!ids.contains(&m.id)&&!live.contains(&m.id)&&lower.is_none_or(|key|(m.created_at,m.id)>=key)).map(|m|m.id).collect();
        for id in deleted{self.apply(Change::Delete(id));}
        for m in messages{self.upsert(snapshot_message(m));}
        for change in changes{self.apply(change);}
        true
    }
    fn record(&mut self, change: &Change) {
        if matches!(change,Change::Upsert(m) if m.embeds.is_some())||matches!(change,Change::Preview(..)|Change::Embeds(..)|Change::RemovePreview(..)|Change::Moderation(..)|Change::Delete(_)){
            self.media_revision=self.media_revision.wrapping_add(1);
        }
        let id=match change{Change::Upsert(m)=>m.id,Change::Edit(id,..)|Change::Reaction(id,..)|Change::Reactions(id,..)|Change::Preview(id,..)|Change::Embeds(id,..)|Change::RemovePreview(id,..)|Change::Moderation(id,..)|Change::Delete(id)=>*id};
        self.dirty.insert(id);
        if matches!(change,Change::Delete(_)|Change::Moderation(_,_,_)){
            if let Some(index)=self.messages.iter().position(|m|m.id==id){if let Some(next)=self.messages.get(index+1){self.dirty.insert(next.id);}}
        }
        if self.pending.is_some() { self.changes.push(change.clone()); }
        for changes in self.snapshots.values_mut() { changes.push(change.clone()); }
    }

    pub fn is_deleted(&self, id: Uuid) -> bool { self.deleted.contains(&id) }

    pub(super) fn deleted_count(&self) -> usize { self.deleted.len() }

    pub fn is_pending(&self, request: Uuid) -> bool { self.pending == Some(request) }

    pub fn loading(&self) -> bool { self.pending.is_some() }

    pub fn fail(&mut self, request_id: Uuid) -> bool {
        if self.pending == Some(request_id) {
            self.pending = None;
            self.changes.clear();
            return true;
        }
        false
    }

    /// Report whether an HTTP/WS arrival actually changes the selected history.
    pub fn upsert(&mut self,message:Message)->bool {
        if Some(message.channel_id)!=self.channel_id||self.deleted.contains(&message.id){return false;}
        self.record(&Change::Upsert(message.clone()));
        self.merge_upsert(message)
    }

    fn merge_upsert(&mut self,message:Message)->bool {
        let changed=if let Some(index)=self.messages.iter().position(|m|m.id==message.id){
            let previous=self.messages[index].created_at;
            let changed=merge_message(&mut self.messages[index],message);
            if self.messages[index].created_at!=previous{let updated=self.messages.remove(index);let at=self.messages.binary_search_by_key(&(updated.created_at,updated.id),|m|(m.created_at,m.id)).unwrap_or_else(|at|at);self.messages.insert(at,updated);}
            changed
        }else{let at=self.messages.binary_search_by_key(&(message.created_at,message.id),|m|(m.created_at,m.id)).unwrap_or_else(|at|at);self.messages.insert(at,message);true};
        self.replay_early_moderation()||changed
    }

    fn replay_early_moderation(&mut self)->bool{
        if self.early_moderation.is_empty(){return false;}
        let mut changed=false;
        for (message,key,status) in std::mem::take(&mut self.early_moderation){
            if self.messages.iter().any(|m|m.id==message&&m.attachments.iter().flatten().any(|a|a.id==key)){
                self.apply(Change::Moderation(message,key,status));changed=true;
            }else if !self.deleted.contains(&message){self.early_moderation.push_back((message,key,status));}
        }
        changed
    }

    pub fn apply(&mut self, change: Change) {
        if let Change::Moderation(id,key,status)=&change{
            if self.deleted.contains(id)||(status!="blocked"&&self.messages.iter().find(|m|m.id==*id).and_then(|m|m.attachments.as_ref()).and_then(|a|a.iter().find(|a|a.id==*key)).is_some_and(|a|a.moderation_status.as_deref()==Some(status.as_str()))){return;}
        }
        if let Change::Upsert(message) = &change {
            if Some(message.channel_id) != self.channel_id || self.deleted.contains(&message.id) {
                return;
            }
        }
        self.record(&change);
        match change {
            Change::Upsert(message) => {self.merge_upsert(message);}
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
            Change::Embeds(id, embeds)=>{
                if let Some(message)=self.messages.iter_mut().find(|m|m.id==id){message.embeds=Some(embeds);}
            },
            Change::Preview(id, preview) => {
                if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
                    let previews = message.embeds.get_or_insert_with(Vec::new);
                    if let Some(existing) = previews.iter_mut().find(|item| item.id == preview.id) { *existing = preview; }
                    else { previews.push(preview); }
                }
            }
            Change::RemovePreview(id, preview_id) => {
                if let Some(previews) = self.messages.iter_mut().find(|m| m.id == id).and_then(|m| m.embeds.as_mut()) {
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
                }else{
                    // Completion can beat both the WS message and POST echo.
                    self.early_moderation.retain(|(m,a,_)|(*m,*a)!=(id,attachment_id));
                    self.early_moderation.push_back((id,attachment_id,status));
                    while self.early_moderation.len()>256{self.early_moderation.pop_front();}
                }
            }
            Change::Delete(id) => {
                self.deleted.insert(id);
                self.messages.retain(|m| m.id != id);
            }
        }
    }
}

/// History endpoints are complete snapshots. Empty optional collections clear
/// missed removals; sparse WS/send echoes continue to preserve omitted fields.
fn snapshot_message(mut message:Message)->Message{
    message.embeds.get_or_insert_with(Vec::new);
    message.reactions.get_or_insert_with(Vec::new);
    message.user_reactions.get_or_insert_with(Vec::new);
    message
}

fn preserve_moderation(existing:&Message,message:&mut Message){
    // The POST/WS echo can have been serialized before moderation completed.
    // Legacy history's empty status must not erase an explicit live verdict.
    for incoming in message.attachments.iter_mut().flatten(){
        if matches!(incoming.moderation_status.as_deref(),None|Some(""|"pending"|"processing")){
            if let Some(previous)=existing.attachments.iter().flatten().find(|a|a.id==incoming.id&&matches!(a.moderation_status.as_deref(),Some(""|"clean"|"sensitive"|"failed"))){incoming.moderation_status=previous.moderation_status.clone();}
        }
    }
}
fn merge_message(existing:&mut Message,mut message:Message)->bool {
    if message.attachments.is_none(){message.attachments=existing.attachments.clone();}
    preserve_moderation(existing,&mut message);
    if message.embeds.is_none(){message.embeds=existing.embeds.clone();}
    if message.reactions.is_none(){message.reactions=existing.reactions.clone();}
    if message.user_reactions.is_none(){message.user_reactions=existing.user_reactions.clone();}
    if existing.edited_at>message.edited_at{message.content=existing.content.clone();message.edited_at=existing.edited_at;}
    if *existing==message{return false;}
    *existing=message;true
}

#[derive(Default)]
pub struct Draft {
    pub text: String,
    pub embeds:Vec<crate::models::EmbedInput>,
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
        if self.is_sending() || (self.text.trim().is_empty() && self.files.is_empty()&&self.embeds.is_empty()) {
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
            Ok(()) => { self.text.clear(); self.files.clear();self.embeds.clear(); self.reply = None; self.error = None; }
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
    fn with_attachment(channel:Uuid,status:&str)->Message{
        let mut m=message(channel,1);m.attachments=Some(vec![serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"mime_type":"image/png","original_file_name":"test.png","size_bytes":1,"created_at":m.created_at,"moderation_status":status})).unwrap()]);m
    }
    #[test]fn completed_moderation_survives_delayed_send_echo(){
        for status in ["","clean","sensitive","failed"]{
            let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let m=with_attachment(channel,"pending");h.upsert(m.clone());
            h.apply(Change::Moderation(m.id,m.attachments.as_ref().unwrap()[0].id,status.into()));
            assert!(!h.upsert(m));assert_eq!(h.messages[0].attachments.as_ref().unwrap()[0].moderation_status.as_deref(),Some(status));
        }
    }
    #[test]fn early_moderation_events_survive_message_and_post_arrival(){
        for snapshot in [false,true]{for status in ["clean","sensitive","failed"]{
            let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let m=with_attachment(channel,"pending");let key=m.attachments.as_ref().unwrap()[0].id;
            h.apply(Change::Moderation(m.id,key,status.into()));
            if snapshot{let request=Uuid::new_v4();h.begin(request);h.finish(request,vec![m.clone()],false);}else{h.upsert(m.clone());}
            assert_eq!(h.messages[0].attachments.as_ref().unwrap()[0].moderation_status.as_deref(),Some(status));assert!(!h.upsert(m));assert!(h.early_moderation.is_empty());
        }}
    }
    #[test]fn legacy_history_cannot_erase_a_known_sensitive_event(){
        for status in [None,Some("")]{for full in [false,true]{
            let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let sensitive=with_attachment(channel,"sensitive");h.upsert(sensitive.clone());let mut legacy=sensitive;legacy.attachments.as_mut().unwrap()[0].moderation_status=status.map(str::to_string);
            if full{let request=Uuid::new_v4();h.begin(request);assert!(h.finish(request,vec![legacy],false));}else{assert!(!h.upsert(legacy));}
            assert_eq!(h.messages[0].attachments.as_ref().unwrap()[0].moderation_status.as_deref(),Some("sensitive"));
        }}
    }
    #[test]fn duplicate_moderation_is_a_noop_and_early_queue_is_bounded(){
        let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let m=with_attachment(channel,"clean");let key=m.attachments.as_ref().unwrap()[0].id;h.upsert(m.clone());h.take_dirty();let rev=h.media_revision();
        for _ in 0..50{h.apply(Change::Moderation(m.id,key,"clean".into()));}
        assert!(h.take_dirty().is_empty());assert_eq!(h.media_revision(),rev);
        let blocked=with_attachment(channel,"blocked");let blocked_id=blocked.id;let blocked_key=blocked.attachments.as_ref().unwrap()[0].id;h.upsert(blocked);h.apply(Change::Moderation(blocked_id,blocked_key,"blocked".into()));assert!(h.is_deleted(blocked_id));assert!(!h.messages.iter().any(|m|m.id==blocked_id));
        for _ in 0..1000{h.apply(Change::Moderation(Uuid::new_v4(),Uuid::new_v4(),"clean".into()));}assert_eq!(h.early_moderation.len(),256);
    }
    #[test]fn moderation_refresh_accepts_legacy_history_without_overriding_explicit_verdicts(){
        for status in [None,Some(""),Some("pending"),Some("processing"),Some("unknown")]{
            let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let pending=with_attachment(channel,"pending");h.upsert(pending.clone());
            let mut fetched=pending.clone();fetched.attachments.as_mut().unwrap()[0].moderation_status=status.map(str::to_string);
            let request=Uuid::new_v4();h.begin_snapshot(request);let changes=h.finish_moderation(request,vec![fetched.clone()]);
            if matches!(status,None|Some("")){
                assert_eq!(changes.len(),1);for change in changes{h.apply(change);}
                assert_eq!(h.messages[0].attachments.as_ref().unwrap()[0].moderation_status.as_deref(),Some(""));
                assert!(!h.upsert(pending.clone()),"late upload echoes must preserve the history's available state");
            }else{assert!(changes.is_empty(),"unresolved or unknown verdicts stay pending");}
            h.apply(Change::Moderation(pending.id,pending.attachments.as_ref().unwrap()[0].id,"sensitive".into()));
            let request=Uuid::new_v4();h.begin_snapshot(request);assert!(h.finish_moderation(request,vec![fetched.clone()]).is_empty());
            h.apply(Change::Delete(pending.id));let request=Uuid::new_v4();h.begin_snapshot(request);assert!(h.finish_moderation(request,vec![fetched]).is_empty());assert!(h.messages.is_empty());
        }
    }
    #[test]fn moderation_poll_updates_only_pending_matching_attachments_and_yields_to_events(){
        let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let m=with_attachment(channel,"pending");h.upsert(m.clone());
        let mut fetched=m.clone();fetched.content=Some("stale content".into());fetched.attachments.as_mut().unwrap()[0].moderation_status=Some("clean".into());
        let req=Uuid::new_v4();h.begin_snapshot(req);h.apply(Change::Edit(m.id,"edited live".into(),None));
        for change in h.finish_moderation(req,vec![fetched.clone()]){h.apply(change);}
        assert_eq!(h.messages[0].content.as_deref(),Some("edited live"));assert_eq!(h.messages[0].attachments.as_ref().unwrap()[0].moderation_status.as_deref(),Some("clean"));
        h.apply(Change::Moderation(m.id,m.attachments.as_ref().unwrap()[0].id,"pending".into()));let req=Uuid::new_v4();h.begin_snapshot(req);h.apply(Change::Moderation(m.id,m.attachments.as_ref().unwrap()[0].id,"sensitive".into()));
        assert!(h.finish_moderation(req,vec![fetched.clone()]).is_empty());
        let req=Uuid::new_v4();h.begin_snapshot(req);h.apply(Change::Delete(m.id));assert!(h.finish_moderation(req,vec![fetched.clone()]).is_empty());assert!(h.messages.is_empty());
        h.upsert(with_attachment(channel,"pending"));let req=Uuid::new_v4();h.begin_snapshot(req);fetched.channel_id=Uuid::new_v4();assert!(h.finish_moderation(req,vec![fetched]).is_empty());
    }
    #[test]
    fn returning_to_recent_history_releases_old_pages_and_rejects_late_pagination(){
        let channel=Uuid::new_v4();let mut history=History::default();history.select(channel);
        for i in 0..350{history.upsert(message(channel,i));}
        let request=Uuid::new_v4();history.begin(request);
        let snapshot=Uuid::new_v4();history.begin_snapshot(snapshot);
        let latest=history.messages.last().unwrap().id;history.apply(Change::Edit(latest,"live edit".into(),None));
        assert!(history.trim_recent(200));assert_eq!(history.messages.len(),200);assert_eq!(history.messages.capacity(),200);
        assert!(!history.finish(request,vec![message(channel,0)],true));
        assert!(history.finish_snapshot(snapshot,vec![]));assert_eq!(history.messages.last().unwrap().content.as_deref(),Some("live edit"));
        assert!(!history.trim_recent(200));
    }

    fn message(channel_id: Uuid, second: i64) -> Message {
        Message {
            id: Uuid::new_v4(), channel_id, author_id: Some(Uuid::new_v4()),webhook_id:None,external_author:None,
            content: Some("original".into()),
            created_at: chrono::DateTime::from_timestamp(second, 0).unwrap(),
            edited_at: None, reply_to: None, attachments: None,
            embeds: None, reactions: None, user_reactions: None,
        }
    }

    #[test]
    fn duplicate_send_echo_skips_updates_but_replays_during_fetch(){
        let channel=Uuid::new_v4();let mut history=History::default();history.select(channel);let original=message(channel,1);
        assert!(history.upsert(original.clone()));assert!(!history.upsert(original.clone()));
        history.apply(Change::Reaction(original.id,MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:1}));assert!(!history.upsert(original.clone()),"a bare HTTP echo preserves realtime reactions");
        let request=Uuid::new_v4();history.begin(request);assert!(!history.upsert(original.clone()));history.finish(request,vec![],false);assert_eq!(history.messages.len(),1,"unchanged arrivals still replay over a history snapshot");
        let mut edit=original.clone();edit.content=Some("edited".into());assert!(history.upsert(edit));history.apply(Change::Delete(original.id));assert!(!history.upsert(original));
    }

    #[test]fn complete_embed_lists_replay_over_both_pending_snapshot_types(){
        let channel=Uuid::new_v4();let mut original=message(channel,1);
        let embed:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","created_at":original.created_at,"title":"Old"})).unwrap();original.embeds=Some(vec![embed.clone()]);
        let mut history=History::default();history.select(channel);history.upsert(original.clone());
        let page=Uuid::new_v4();let read=Uuid::new_v4();history.begin(page);history.begin_snapshot(read);history.apply(Change::Embeds(original.id,vec![]));
        assert!(history.finish_snapshot(read,vec![original.clone()]));assert!(history.messages[0].embeds.as_ref().unwrap().is_empty());
        assert!(history.finish(page,vec![original.clone()],false));assert!(history.messages[0].embeds.as_ref().unwrap().is_empty());
        let mut newer=embed;newer.id=Uuid::new_v4();history.apply(Change::Embeds(original.id,vec![newer.clone()]));assert_eq!(history.messages[0].embeds,Some(vec![newer]));
        history.apply(Change::Delete(original.id));history.apply(Change::Embeds(original.id,vec![]));assert!(history.messages.is_empty());
    }
    #[test]fn embed_only_drafts_preserve_inputs_on_failure_and_clear_on_success(){
        let mut draft=Draft{embeds:vec![crate::models::EmbedInput{title:"Rich".into(),..Default::default()}],..Default::default()};
        let id=draft.begin().unwrap();assert!(draft.finish(id,Err("Try again".into())));assert_eq!(draft.embeds.len(),1);let id=draft.begin().unwrap();assert!(draft.finish(id,Ok(())));assert!(draft.embeds.is_empty());
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
        let mut draft=Draft::default();draft.files.push(crate::api::features::UploadFile{path:"file".into(),name:"file".into(),size:1,_temporary:None});
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
        let mut preview: Embed = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(), "url":"https://example.test", "kind":"og", "title":"old",
            "fetched_at":"2026-10-03T12:00:00Z"
        })).unwrap();
        original.embeds = Some(vec![preview.clone()]);
        let mut history = History::default();
        history.select(channel);
        let request = Uuid::new_v4();
        history.begin(request);
        preview.title = Some("updated".into());
        history.apply(Change::Preview(original.id, preview.clone()));
        history.finish(request, vec![original.clone()], false);
        assert_eq!(history.messages[0].embeds.as_ref().unwrap()[0].title.as_deref(),Some("updated"));
        history.begin(request);
        history.apply(Change::RemovePreview(original.id, preview.id));
        history.finish(request, vec![original], false);
        assert!(history.messages[0].embeds.as_ref().unwrap().is_empty());
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

#[cfg(test)]mod correction_tests{
    use super::*;
    fn message(channel:Uuid,time:i64)->Message{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"content":"original","created_at":chrono::DateTime::from_timestamp(time,0)})).unwrap()}
    fn reaction(count:i32)->MessageReactionSummary{MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count}}
    #[test]fn independent_reads_replay_reactions_and_removed_previews_during_pagination(){
        let channel=Uuid::new_v4();let mut history=History::default();history.select(channel);
        let mut old=message(channel,2);let preview=Uuid::new_v4();old.reactions=Some(vec![reaction(1)]);old.embeds=Some(vec![serde_json::from_value(serde_json::json!({"id":preview,"url":"https://example.test","kind":"og","fetched_at":"2026-10-08T12:00:00Z"})).unwrap()]);history.upsert(old.clone());
        let page=Uuid::new_v4();history.begin(page);let read=Uuid::new_v4();history.begin_snapshot(read);
        history.apply(Change::Reaction(old.id,reaction(2)));history.apply(Change::RemovePreview(old.id,preview));
        assert!(history.finish_snapshot(read,vec![old.clone()]));assert_eq!(history.messages[0].reactions.as_ref().unwrap()[0].count,2);assert!(history.messages[0].embeds.as_ref().unwrap().is_empty());
        assert!(history.finish(page,vec![old.clone(),message(channel,1)],true));assert_eq!(history.messages[1].reactions.as_ref().unwrap()[0].count,2);assert!(history.messages[1].embeds.as_ref().unwrap().is_empty());
        let fresh=Uuid::new_v4();history.begin_snapshot(fresh);old.reactions=Some(vec![reaction(3)]);old.embeds=Some(vec![]);history.finish_snapshot(fresh,vec![old]);assert_eq!(history.messages[1].reactions.as_ref().unwrap()[0].count,3);
    }
    #[test]fn reconnect_retains_old_pages_reconciles_deletion_edges_and_replays_live_messages(){
        let channel=Uuid::new_v4();let mut history=History::default();history.select(channel);let a=message(channel,1);let b=message(channel,2);let c=message(channel,3);
        for m in [&a,&b,&c]{history.upsert(m.clone());}let request=Uuid::new_v4();history.begin_snapshot(request);let live=message(channel,4);history.upsert(live.clone());
        let mut updated=b.clone();updated.content=Some("recovered edit".into());history.finish_reconcile(request,vec![a.clone(),updated],true);
        assert_eq!(history.messages.iter().map(|m|m.id).collect::<Vec<_>>(),vec![a.id,b.id,live.id]);assert_eq!(history.messages[1].content.as_deref(),Some("recovered edit"));assert!(history.is_deleted(c.id));
        history.select(Uuid::new_v4());assert!(!history.finish_reconcile(request,vec![c],true));assert!(history.messages.is_empty());
    }
    #[test]fn dirty_rows_include_grouping_neighbors_and_reply_dependents_without_dirtying_entire_history(){
        let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let messages:Vec<_>=(0..1000).map(|i|message(channel,i)).collect();for m in &messages{h.upsert(m.clone());}let mut reply=message(channel,1001);reply.reply_to=Some(messages[400].id);h.upsert(reply.clone());h.take_dirty();
        h.apply(Change::Reaction(messages[400].id,reaction(2)));let dirty=h.take_dirty();assert!(dirty.contains(&messages[400].id));assert!(dirty.contains(&messages[401].id));assert!(dirty.contains(&reply.id));assert_eq!(dirty.len(),3);
    }
    #[test]fn complete_snapshots_clear_missed_removals_while_sparse_echoes_preserve_live_fields(){
        let channel=Uuid::new_v4();let mut h=History::default();h.select(channel);let mut original=message(channel,1);original.reactions=Some(vec![reaction(2)]);h.upsert(original.clone());let mut bare=original;bare.reactions=None;assert!(!h.upsert(bare.clone()));let request=Uuid::new_v4();h.begin_snapshot(request);h.finish_snapshot(request,vec![bare]);assert!(h.messages[0].reactions.as_ref().unwrap().is_empty());
    }

}
