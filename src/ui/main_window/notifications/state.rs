use crate::models::*;
use std::collections::{HashMap,HashSet,VecDeque};
use uuid::Uuid;
#[derive(Debug,Clone)]
pub struct Notice {
    pub id:Uuid,pub message_id:Uuid,pub channel_id:Option<Uuid>,pub author_id:Option<Uuid>,
    pub content:String,pub read:bool,pub persisted:bool,pub created_at:chrono::DateTime<chrono::Utc>,
    pub pending_desktop:bool, pub seen_event:bool,
}
#[derive(Default)]
pub struct Inbox {
    pub rows:HashMap<Uuid,Notice>,pub messages:HashMap<Uuid,Message>,
    message_order:VecDeque<Uuid>,read:HashSet<Uuid>,deleted:HashSet<Uuid>,
}
impl Inbox {
    pub fn event(&mut self,event:NotificationEvent){
        if self.deleted.contains(&event.message_id){return;}
        if let Some(row)=self.rows.get_mut(&event.id){if !row.seen_event{row.seen_event=true;row.pending_desktop=!row.read;}return;}
        let channel_id=self.messages.get(&event.message_id).map(|m|m.channel_id);
        self.rows.insert(event.id,Notice{id:event.id,message_id:event.message_id,channel_id,author_id:event.user_id,content:event.message_content,read:false,persisted:false,created_at:chrono::Utc::now(),pending_desktop:true,seen_event:true});
        // Ephemeral events cannot be paged later; keep session memory bounded.
        let mut ephemeral:Vec<_>=self.rows.values().filter(|n|!n.persisted).map(|n|(n.created_at,n.id)).collect();ephemeral.sort();
        let excess=ephemeral.len().saturating_sub(500);for (_,id) in ephemeral.into_iter().take(excess){self.rows.remove(&id);}
    }
    pub fn merge_head(&mut self,rows:Vec<NotificationSummary>){
        let boundary=rows.last().map(|n|(n.created_at,n.id));let ids:HashSet<_>=rows.iter().map(|n|n.id).collect();
        self.rows.retain(|id,n|!n.persisted||ids.contains(id)||boundary.is_some_and(|oldest|(n.created_at,n.id)<oldest));
        self.merge(rows);
    }
    pub fn merge(&mut self,rows:Vec<NotificationSummary>){
        for n in rows{
            if self.deleted.contains(&n.message_id){continue;}
            let pending=self.rows.get(&n.id).is_some_and(|r|r.pending_desktop);
            let seen_event=self.rows.get(&n.id).is_some_and(|r|r.seen_event);
            let read=n.read||self.read.contains(&n.id)||self.rows.get(&n.id).is_some_and(|r|r.read);
            self.rows.insert(n.id,Notice{id:n.id,message_id:n.message_id,channel_id:Some(n.channel_id),author_id:n.author_id,content:n.message_content,read,persisted:true,created_at:n.created_at,pending_desktop:pending,seen_event});
        }
    }
    pub fn observe(&mut self,m:Message){
        if self.deleted.contains(&m.id){return;}
        for n in self.rows.values_mut().filter(|n|n.message_id==m.id){n.channel_id=Some(m.channel_id);n.author_id=m.author_id;}
        if !self.messages.contains_key(&m.id){self.message_order.push_back(m.id);}self.messages.insert(m.id,m);
        while self.message_order.len()>512 {if let Some(id)=self.message_order.pop_front(){self.messages.remove(&id);}}
    }
    pub fn mark(&mut self,id:Uuid){if let Some(n)=self.rows.get_mut(&id){n.read=true;n.pending_desktop=false;if n.persisted{self.read.insert(id);}}}
    pub fn delete(&mut self,message:Uuid){self.deleted.insert(message);self.messages.remove(&message);self.rows.retain(|_,n|n.message_id!=message);}
    pub fn ordered(&self)->Vec<&Notice>{let mut rows:Vec<_>=self.rows.values().collect();rows.sort_by_key(|n|std::cmp::Reverse((n.created_at,n.id)));rows}
    pub fn unread(&self,channels:&[NoticeChannel])->usize {self.rows.values().filter(|n|!n.read&&n.channel_id.map_or(true,|id|channels.iter().any(|c|c.id==id))).count()}
}
#[derive(Clone)]
pub struct NoticeChannel {pub id:Uuid,pub name:String,pub notification_settings:Option<NotificationSettings>}
impl From<Channel> for NoticeChannel{fn from(c:Channel)->Self{Self{id:c.id,name:c.name,notification_settings:c.notification_settings}}}
/// No preview or destination may be exposed until current channel access is known.
pub fn delivery(n:&Notice,config:&UserConfig,channels:&[NoticeChannel],user:Uuid,messages:&HashMap<Uuid,Message>)->Option<(String,bool)> {
    let channel=channels.iter().find(|c|Some(c.id)==n.channel_id)?;
    let prefs=config.notifications.as_ref()?;
    if !prefs.enabled.unwrap_or(true)||n.read||n.author_id==Some(user)||channel.notification_settings==Some(NotificationSettings::Off){return None;}
    let content=messages.get(&n.message_id).and_then(|m|m.content.as_deref()).unwrap_or(&n.content).to_ascii_lowercase();
    let mentioned=content.contains(&format!("@mention(<@{user}>)"))||content.split(|c:char|c.is_whitespace()||c.is_ascii_punctuation()&&c!='@').any(|s|s=="@everyone");
    let reply=messages.get(&n.message_id).and_then(|m|m.reply_to).and_then(|id|messages.get(&id)).is_some_and(|m|m.author_id==Some(user));
    if !prefs.mentions.unwrap_or(true) && (mentioned||n.persisted&&!reply&&!messages.contains_key(&n.message_id)){return None;}
    if channel.notification_settings!=Some(NotificationSettings::All)&&!n.persisted&&!mentioned&&!reply{return None;}
    let body=if prefs.message_preview.unwrap_or(true){n.content.clone()}else{"Nova mensagem no Papo".into()};
    Some((body,prefs.sound.unwrap_or(true)))
}
/// Reapply only observations newer than a channel snapshot's request.
#[derive(Default)]
pub struct ReadJournal {
    pub version:u64,last:HashMap<Uuid,(u64,ChannelLastMessage)>,read:HashMap<Uuid,(u64,MessageCursor)>,
    preferences:HashMap<Uuid,(u64,NotificationSettings)>,
}
impl ReadJournal {
    pub fn preference(&mut self,channel:Uuid,setting:NotificationSettings){self.version+=1;self.preferences.insert(channel,(self.version,setting));}
    pub fn message(&mut self,m:&Message,channels:&mut [Channel]){
        let Some(c)=channels.iter_mut().find(|c|c.id==m.channel_id) else{return;};
        if c.last_message.as_ref().is_some_and(|old|(old.created_at,old.id)>=(m.created_at,m.id)){return;}
        self.version+=1;let last=ChannelLastMessage{id:m.id,content:m.content.clone(),author_id:m.author_id,author_username:None,created_at:m.created_at};c.last_message=Some(last.clone());self.last.insert(c.id,(self.version,last));
    }
    pub fn observed_read(&mut self,channel:Uuid,messages:&[Message],channels:&mut [Channel]){
        let Some(newest)=messages.iter().filter(|m|m.channel_id==channel).max_by_key(|m|(m.created_at,m.id)) else{return;};
        self.message(newest,channels);let Some(c)=channels.iter_mut().find(|c|c.id==channel)else{return;};
        let cursor=MessageCursor::from(newest);
        if self.read.get(&channel).is_some_and(|(_,old)|(old.created_at,old.id)>=(cursor.created_at,cursor.id)){return;}
        // An older history page must not regress a read marker already returned in the snapshot.
        if c.last_read_message==c.last_message.as_ref().map(|m|m.id)&&c.last_message.as_ref().is_some_and(|m|(m.created_at,m.id)>(cursor.created_at,cursor.id)){return;}
        self.version+=1;c.last_read_message=Some(cursor.id);c.last_read_at=Some(chrono::Utc::now());self.read.insert(channel,(self.version,cursor));
    }
    pub fn reconcile(&self,snapshot_version:u64,channels:&mut [Channel]){
        for c in channels {
            if let Some((_,setting))=self.preferences.get(&c.id).filter(|(v,_)|*v>snapshot_version){c.notification_settings=Some(setting.clone());}
            if let Some((version,last))=self.last.get(&c.id).filter(|(v,_)|*v>snapshot_version){let _=version;if c.last_message.as_ref().map_or(true,|old|(old.created_at,old.id)<(last.created_at,last.id)){c.last_message=Some(last.clone());}}
            if let Some((_,read))=self.read.get(&c.id).filter(|(v,_)|*v>snapshot_version){c.last_read_message=Some(read.id);c.last_read_at=Some(chrono::Utc::now());}
        }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    fn message(channel:Uuid,id:Uuid,second:i64)->Message{serde_json::from_value(serde_json::json!({"id":id,"channel_id":channel,"author_id":Uuid::nil(),"content":"hello","created_at":chrono::DateTime::from_timestamp(second,0).unwrap()})).unwrap()}
    fn channel(id:Uuid)->Channel{serde_json::from_value(serde_json::json!({"id":id,"name":"general","created_at":"2026-10-03T00:00:00Z"})).unwrap()}
    fn event(id:Uuid,message_id:Uuid)->NotificationEvent{NotificationEvent{id,message_id,user_id:Some(Uuid::nil()),message_content:"hello".into()}}
    #[test] fn event_rest_echo_resolves_once_and_stale_unread_cannot_undo_success(){
        let(id,msg,ch)=(Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());let mut inbox=Inbox::default();inbox.event(event(id,msg));inbox.event(event(id,msg));assert_eq!(inbox.rows.len(),1);assert_eq!(inbox.rows[&id].channel_id,None);assert!(!inbox.rows[&id].persisted);
        let row=NotificationSummary{id,message_id:msg,channel_id:ch,author_id:None,message_content:"hello".into(),read:false,created_at:chrono::Utc::now()};inbox.merge(vec![row.clone()]);assert_eq!(inbox.rows.len(),1);assert!(inbox.rows[&id].persisted);assert_eq!(inbox.rows[&id].channel_id,Some(ch));inbox.mark(id);inbox.merge(vec![row]);assert!(inbox.rows[&id].read);
        inbox.delete(msg);inbox.event(event(id,msg));assert!(inbox.rows.is_empty());
        let id2=Uuid::new_v4();let msg2=Uuid::new_v4();let rest=NotificationSummary{id:id2,message_id:msg2,channel_id:ch,author_id:None,message_content:"late WS".into(),read:false,created_at:chrono::Utc::now()};inbox.merge(vec![rest]);assert!(!inbox.rows[&id2].pending_desktop);inbox.event(event(id2,msg2));assert!(inbox.rows[&id2].pending_desktop);inbox.rows.get_mut(&id2).unwrap().pending_desktop=false;inbox.event(event(id2,msg2));assert!(!inbox.rows[&id2].pending_desktop);inbox.merge_head(vec![]);assert!(inbox.rows.is_empty());
    }
    #[test] fn missed_message_has_no_destination_and_ephemeral_dismissal_has_no_persisted_id(){let mut inbox=Inbox::default();let(id,msg,ch)=(Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());inbox.event(event(id,msg));assert_eq!(inbox.rows[&id].channel_id,None);inbox.observe(message(ch,msg,1));assert_eq!(inbox.rows[&id].channel_id,Some(ch));inbox.mark(id);assert!(!inbox.rows[&id].persisted);assert!(!inbox.read.contains(&id));}
    #[test] fn desktop_preferences_and_permissions_hide_content(){
        let(id,msg,ch,user)=(Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());let mut inbox=Inbox::default();inbox.event(event(id,msg));let mut config=UserConfig::default();let mut channels=vec![NoticeChannel::from(channel(ch))];channels[0].notification_settings=Some(NotificationSettings::All);
        assert!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages).is_none());inbox.observe(message(ch,msg,1));assert!(delivery(&inbox.rows[&id],&config,&[],user,&inbox.messages).is_none());assert_eq!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages),Some(("hello".into(),true)));
        let prefs=config.notifications.as_mut().unwrap();prefs.sound=Some(false);prefs.message_preview=Some(false);assert_eq!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages),Some(("Nova mensagem no Papo".into(),false)));
        config.notifications.as_mut().unwrap().enabled=Some(false);assert!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages).is_none());config.notifications.as_mut().unwrap().enabled=Some(true);channels[0].notification_settings=Some(NotificationSettings::Off);assert!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages).is_none());
        channels[0].notification_settings=None;assert!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages).is_none());inbox.rows.get_mut(&id).unwrap().persisted=true;inbox.rows.get_mut(&id).unwrap().content=format!("@mention(<@{user}>)");inbox.messages.clear();assert!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages).is_some());config.notifications.as_mut().unwrap().mentions=Some(false);assert!(delivery(&inbox.rows[&id],&config,&channels,user,&inbox.messages).is_none());
    }
    #[test] fn older_snapshot_cannot_restore_notifications_after_a_channel_is_muted(){let id=Uuid::new_v4();let mut channels=vec![channel(id)];let mut journal=ReadJournal::default();let snapshot=journal.version;journal.preference(id,NotificationSettings::Off);journal.reconcile(snapshot,&mut channels);assert_eq!(channels[0].notification_settings,Some(NotificationSettings::Off));assert!(channels[0].last_read_message.is_none());}
    #[test] fn live_unread_survives_old_snapshot_and_older_history_cannot_regress_read(){
        let ch=Uuid::new_v4();let mut channels=vec![channel(ch)];let mut journal=ReadJournal::default();let first=message(ch,Uuid::from_u128(1),1);let second=message(ch,Uuid::from_u128(2),1);
        let old_version=journal.version;journal.message(&first,&mut channels);assert!(channels[0].has_unread());journal.observed_read(ch,&[first.clone()],&mut channels);assert!(!channels[0].has_unread());journal.message(&second,&mut channels);assert!(channels[0].has_unread());let mut stale=vec![channel(ch)];journal.reconcile(old_version,&mut stale);assert_eq!(stale[0].last_message.as_ref().unwrap().id,second.id);assert_eq!(stale[0].last_read_message,Some(first.id));assert!(stale[0].has_unread());journal.observed_read(ch,&[second.clone()],&mut stale);journal.observed_read(ch,&[first],&mut stale);assert_eq!(stale[0].last_read_message,Some(second.id));assert!(!stale[0].has_unread());
    }
}
