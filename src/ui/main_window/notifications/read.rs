//! Explicit, coalesced read acknowledgements for server channels and DMs.
use super::*;
use std::collections::HashMap;
use crate::models::MessageCursor;

#[derive(Default)]
pub(super) struct Reads {
    pending:Option<Uuid>,
    acknowledged:HashMap<Uuid,MessageCursor>,
    retry:HashMap<Uuid,Instant>,
}
fn key(cursor:MessageCursor)->(chrono::DateTime<chrono::Utc>,Uuid){(cursor.created_at,cursor.id)}
impl Reads {
    #[cfg(test)]pub(super) fn idle(&self)->bool{self.pending.is_none()}
    #[cfg(test)]pub(super) fn retry_now(&mut self,channel:Uuid){self.retry.insert(channel,Instant::now());}
    pub(super) fn covers(&self,message:&Message)->bool{
        self.acknowledged.get(&message.channel_id).is_some_and(|old|key(*old)>=key(MessageCursor::from(message)))
    }
    pub(super) fn begin(&mut self,message:&Message)->Option<Uuid>{
        if self.pending.is_some()
            || self.retry.get(&message.channel_id).is_some_and(|until|*until>Instant::now())
            || self.covers(message){return None;}
        let request=Uuid::new_v4();self.pending=Some(request);Some(request)
    }
    fn acknowledge(&mut self,message:&Message){
        let cursor=MessageCursor::from(message);self.acknowledged.entry(message.channel_id).and_modify(|old|{if key(cursor)>key(*old){*old=cursor;}}).or_insert(cursor);self.retry.remove(&message.channel_id);
    }
    fn finish(&mut self,request:Uuid,message:&Message,success:bool)->bool{
        if self.pending!=Some(request){return false;}
        self.pending=None;
        if success{
            self.acknowledge(message);
        }else{
            // Do not hammer a denied/unavailable endpoint every UI tick.
            self.retry.insert(message.channel_id,Instant::now()+Duration::from_secs(30));
        }
        true
    }
}

impl MainWindowModel {
    pub(in crate::ui::main_window) fn tick_channel_read(&mut self,root:&gtk::Box,sender:&ComponentSender<Self>){
        if !root.root().and_downcast::<gtk::Window>().is_some_and(|w|w.is_active()){return;}
        let Some(message)=self.chat.model().read_candidate()else{return;};
        if self.active_channel_id!=Some(message.channel_id)||!self.can_read_target(message.channel_id)||self.notifications.manual_reads.contains_key(&message.channel_id){return;}
        let Some(request)=self.notifications.reads.begin(&message)else{return;};
        let api=self.api_client.clone();let sender=sender.clone();
        self.notifications.jobs.push(tokio::spawn(async move{
            let result=api.mark_channel_read(message.channel_id,message.id).await;
            sender.input(MainWindowMsg::ChannelRead{request,message,result});
        }));
    }
    pub(in crate::ui::main_window) fn channel_read_finished(&mut self,request:Uuid,message:Message,result:anyhow::Result<()>,sender:&ComponentSender<Self>){
        if !self.notifications.reads.finish(request,&message,result.is_ok()){return;}
        match result{
            Ok(())=>self.apply_channel_read(&message,sender),
            Err(error)=>sender.input(MainWindowMsg::OperationFailed(error)),
        }
    }
    fn apply_channel_read(&mut self,message:&Message,sender:&ComponentSender<Self>){
        if !self.can_read_target(message.channel_id){return;}
        self.notifications.journal.observed_read(message.channel_id,std::slice::from_ref(message),&mut self.channels);
        self.direct_history(message.channel_id,std::slice::from_ref(message),sender);
        self.notifications.inbox.mark_through(message);self.publish_voice_channels();self.render_inbox(sender);
        if self.notifications.inbox.rows.values().any(|n|n.persisted&&!n.read&&n.channel_id==Some(message.channel_id)&&!self.notifications.inbox.messages.contains_key(&n.message_id)){
            self.refresh_notifications(None,sender.clone());
        }
    }
    pub(in crate::ui::main_window) fn mark_channel_read_action(&mut self,channel:Uuid,sender:&ComponentSender<Self>){
        if !self.can_read_target(channel)||self.notifications.manual_reads.contains_key(&channel){return;}
        let request=Uuid::new_v4();self.notifications.manual_reads.insert(channel,request);let api=self.api_client.clone();let sender=sender.clone();
        self.notifications.jobs.push(tokio::spawn(async move{let result=api.mark_latest_read(channel).await;sender.input(MainWindowMsg::ManualRead{request,channel,result});}));
    }
    pub(in crate::ui::main_window) fn manual_read_finished(&mut self,request:Uuid,channel:Uuid,result:anyhow::Result<Option<Message>>,sender:&ComponentSender<Self>){
        if self.notifications.manual_reads.get(&channel)!=Some(&request){return;}self.notifications.manual_reads.remove(&channel);
        match result{
            Ok(Some(message))=>{self.notifications.reads.acknowledge(&message);self.apply_channel_read(&message,sender);},
            Ok(None)=>{},Err(error)=>sender.input(MainWindowMsg::OperationFailed(error)),
        }
    }

}

#[cfg(test)]mod tests{
    use super::*;
    fn message(channel:Uuid,second:i64)->Message{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":chrono::DateTime::from_timestamp(second,0).unwrap()})).unwrap()}
    #[test]fn reads_are_coalesced_and_only_success_advances_the_cursor(){
        let mut reads=Reads::default();let m=message(Uuid::new_v4(),1);let request=reads.begin(&m).unwrap();
        assert!(reads.begin(&m).is_none());assert!(!reads.finish(Uuid::new_v4(),&m,true));assert!(reads.acknowledged.is_empty());
        assert!(reads.finish(request,&m,true));assert!(reads.begin(&m).is_none());assert!(reads.begin(&message(m.channel_id,0)).is_none());
        let newer=message(m.channel_id,2);let request=reads.begin(&newer).unwrap();reads.finish(request,&newer,true);
        assert_eq!(reads.acknowledged[&m.channel_id].id,newer.id);
    }
    #[test]fn failures_keep_unread_retry_without_blocking_other_channels(){
        let mut reads=Reads::default();let m=message(Uuid::new_v4(),1);let request=reads.begin(&m).unwrap();reads.finish(request,&m,false);
        assert!(reads.acknowledged.is_empty());assert!(reads.begin(&m).is_none());
        let other=message(Uuid::new_v4(),2);let request=reads.begin(&other).unwrap();reads.finish(request,&other,true);
        reads.retry.insert(m.channel_id,Instant::now());assert!(reads.begin(&m).is_some());
    }
}
