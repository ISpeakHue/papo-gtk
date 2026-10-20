//! EN-US: Bidirectional, contiguous history windows. Recent channel heads live separately.
//! PT-BR: Janelas contíguas de histórico permitem navegação nas duas direções; mensagens recentes ficam em um cache separado.
use super::*;

pub(super) const HISTORY_LIMIT:usize=600;
#[derive(Default)]
pub(super) struct Newer {
    pub has_more:bool,
    pub request:Option<Uuid>,
    pub error:Option<String>,
}
impl ChatModel {
    pub(super) fn load_newer(&mut self,sender:&ComponentSender<Self>){
        if !self.newer.has_more||self.history.loading()||self.older_requested||!self.access.read||self.actions.navigation.is_some(){return;}
        let(Some(api),Some(channel),Some(cursor))=(self.actions.api.clone(),self.active_channel.as_ref().map(|c|c.id()),self.history.messages.last().map(MessageCursor::from))else{return;};
        let request=Uuid::new_v4();let epoch=self.actions.epoch;
        self.newer.request=Some(request);self.newer.error=None;self.history.begin(request);
        let input=sender.input_sender().clone();
        self.transfers.jobs.push(tokio::spawn(async move{
            let result=api.messages_after(channel,cursor).await;
            let _=input.send(ChatMsg::NewerLoaded{epoch,request,result});
        }));
    }
    pub(super) fn retain_history_window(&mut self,widgets:&ChatModelWidgets,position:Option<&Position>)->bool{
        if self.actions.pinned_view||self.transfers.saving>0||!self.history_ready||self.history.loading()||self.older_requested||self.reconcile_request.is_some()||self.viewport.scrolling()||self.initial_read.is_some()||self.actions.navigation.is_some()||self.history.messages.is_empty()||(self.history.messages.len()<=HISTORY_LIMIT&&self.history.cache_bytes()<=16<<20){return false;}
        let captured=self.viewport.capture(&widgets.scrolled_window,&widgets.messages_list);
        let index=match position.unwrap_or(&captured){
            Position::Bottom=>self.history.messages.len()-1,
            Position::Message(id)|Position::Unread(id)|Position::Anchor{id:Some(id),..}=>self.history.messages.iter().position(|m|m.id==*id).unwrap_or(self.history.messages.len()-1),
            _=>self.history.messages.len()-1,
        };
        let range=self.history.retention_range(index,HISTORY_LIMIT,16<<20);
        let (start,end)=(range.start,range.end);
        if start==0&&end==self.history.messages.len(){return false;}
        if end<self.history.messages.len(){
            if !self.history_detached{if let Some(channel)=&self.active_channel{
                self.conversations.store(channel.id(),self.history.recent_copy(cache::MESSAGE_LIMIT),self.has_more||self.history.messages.len()>cache::MESSAGE_LIMIT);
            }}
            self.history_detached=true;self.newer.has_more=true;self.viewing_old=true;
        }
        if start>0{self.has_more=true;self.retry_cursor=None;self.history_error=None;}
        self.history.retain_range(start..end);
        self.transfers.trim_provider_ids(&self.history.messages);
        self.transfers.release_distant(&self.history.messages);
        let ids=self.history.messages.iter().map(|m|m.id).collect();
        self.render_window.retain_ids(&ids);self.render_window.range=0..0;
        true
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,until,pump};
    use adw::prelude::*;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let template:Message=serde_json::from_value(backend.lock().unwrap().message.clone()).unwrap();
    // Repeated timestamps exercise UUID tie breaking on every page boundary.
    let messages:Vec<Message>=(0..1250).map(|i|{let mut m=template.clone();m.id=Uuid::from_u128(10000+i);m.created_at=template.created_at+chrono::Duration::seconds((i/7) as i64);m.content=Some(format!("Message {i}"));m}).collect();
    backend.lock().unwrap().paged_messages=messages.iter().map(|m|serde_json::json!({"id":m.id,"channel_id":m.channel_id,"author_id":m.author_id,"content":m.content,"created_at":m.created_at})).collect();
    let channel:Channel=serde_json::from_value(serde_json::json!({"id":template.channel_id,"name":"Paging","created_at":template.created_at})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel),api:Some(api),messages:messages[1220..].to_vec(),user_id:template.author_id,..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:template.author_id.unwrap(),access:crate::models::Access::direct()});
    let host=adw::Window::builder().default_width(1000).default_height(700).content(chat.widget()).build();host.present();performance::settle(context);
    chat.emit(ChatMsg::Action(ActionMsg::NavigateAt(MessageCursor::from(&messages[100]))));
    until(context,||chat.model().actions.highlight==Some(messages[100].id));performance::settle(context);
    assert!(chat.model().history_detached&&chat.model().newer.has_more);
    assert!(!chat.model().history.messages.iter().any(|m|m.id==messages[1249].id));
    let scroll=actions::tests::descendants(chat.widget().upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::ScrolledWindow>().ok()).find(|s|s.has_css_class("papo-history-surface")).unwrap();
    // Failed forward reads retain their cursor and require an explicit retry.
    {let mut state=chat.state().get_mut();let request=Uuid::new_v4();state.model.history.begin(request);state.model.newer.request=Some(request);let epoch=state.model.actions.epoch;drop(state);chat.emit(ChatMsg::NewerLoaded{epoch,request,result:Err(anyhow::anyhow!("Synthetic error"))});}
    pump(context);assert!(chat.model().newer.error.is_some());
    let before=chat.model().history.messages.len();
    chat.emit(ChatMsg::LoadNewer);until(context,||chat.model().newer.request.is_none());assert!(chat.model().newer.error.is_none());assert!(chat.model().history.messages.len()>before);
    // Reading down crosses enough pages to evict old metadata, without gaps.
    for _ in 0..20{
        if !chat.model().history_detached{break;}
        let old_last=chat.model().history.messages.last().unwrap().id;
        let adj=scroll.vadjustment();adj.set_value((adj.upper()-adj.page_size()).max(0.0));
        chat.emit(ChatMsg::MediaViewport);
        until(context,||chat.model().history.messages.last().unwrap().id!=old_last||!chat.model().history_detached);
        performance::settle(context);
        assert!(chat.model().history.messages.len()<=HISTORY_LIMIT);
        let ids:Vec<_>=chat.model().history.messages.iter().map(|m|m.id.as_u128()).collect();
        assert!(ids.windows(2).all(|pair|pair[1]==pair[0]+1),"retention must keep a contiguous range");
    }
    until(context,||!chat.model().history_detached&&chat.model().reconcile_request.is_none());
    assert_eq!(chat.model().history.messages.last().unwrap().id,messages[1249].id);
    assert!(chat.model().has_more);
    let mut live=messages[1249].clone();live.id=Uuid::from_u128(11250);live.created_at+=chrono::Duration::seconds(1);
    chat.emit(ChatMsg::AddMessage(live.clone()));pump(context);assert_eq!(chat.model().history.messages.last().unwrap().id,live.id);
    // A response from an abandoned window cannot alter the current history.
    chat.emit(ChatMsg::NewerLoaded{epoch:Uuid::new_v4(),request:Uuid::new_v4(),result:Ok(MessageListResponse{channel_id:template.channel_id,messages:vec![messages[0].clone()],has_more:false})});pump(context);
    assert!(!chat.model().history.messages.iter().any(|m|m.id==messages[0].id));
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);server.abort();pump(context);
}
