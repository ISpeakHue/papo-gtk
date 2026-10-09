//! Participant conversations share the message pipeline but never public channel roles.
use super::*;
use crate::models::{DirectConversation,Message,MessageCursor};
use std::collections::{HashMap,HashSet};
#[derive(Debug)]
pub enum DirectMsg {
    Refresh, Open(Uuid), Select(Uuid,Option<Uuid>), Hide(Uuid), Blocks, Block(Uuid,bool),
    Listed{token:Uuid,version:u64,result:anyhow::Result<Vec<DirectConversation>>},
    Selected{token:Uuid,message:Option<Uuid>,result:anyhow::Result<DirectConversation>},
    Hidden{ id:Uuid,result:anyhow::Result<()> },
    Blocked{ id:Uuid,block:bool,result:anyhow::Result<()> },
    BlocksLoaded{token:Uuid,result:anyhow::Result<Vec<UserSummary>>},
}
#[derive(Default)]
pub(super) struct Direct {
    pub items:Vec<DirectConversation>, request:Option<Uuid>,refresh_again:bool,selection:Option<Uuid>,version:u64,
    changed:HashMap<Uuid,u64>,read:HashMap<Uuid,MessageCursor>,hidden:HashSet<Uuid>,hidden_since:HashMap<Uuid,u64>,blocked_peers:HashSet<Uuid>,busy:HashSet<Uuid>,jobs:Vec<tokio::task::JoinHandle<()>>,
    pub presence:HashMap<Uuid,crate::ws::PresenceStatus>,last_refresh:Option<Instant>,
    blocks:Option<BlockView>, blocks_request:Option<Uuid>,
}
struct BlockView {window:gtk::Window,rows:gtk::Box,status:gtk::Label}
impl Drop for Direct{fn drop(&mut self){for j in self.jobs.drain(..){j.abort();}if let Some(v)=self.blocks.take(){v.window.close();}}}
fn cursor_key(c:MessageCursor)->(chrono::DateTime<chrono::Utc>,Uuid){(c.created_at,c.id)}
fn reconcile_read(dm:&mut DirectConversation,read:&HashMap<Uuid,MessageCursor>){
    if let Some(cursor)=read.get(&dm.id){
        if dm.last_message.as_ref().is_some_and(|m|(m.created_at,m.id)<=cursor_key(*cursor)){
            dm.last_read_message=Some(cursor.id);dm.unread_count=0;
        }
    }
}
fn observe_read(dm:&mut DirectConversation,messages:&[Message],read:&mut HashMap<Uuid,MessageCursor>){
    let Some(m)=messages.iter().filter(|m|m.channel_id==dm.id).max_by_key(|m|(m.created_at,m.id))else{return;};
    let cursor=MessageCursor::from(m);
    // last_read_at is the time of the read operation, not the message's timestamp.
    if read.get(&dm.id).is_some_and(|old|cursor_key(*old)>cursor_key(cursor)){return;}
    read.insert(dm.id,cursor);dm.last_read_message=Some(m.id);
    if dm.last_message.as_ref().map_or(true,|last|(last.created_at,last.id)<=cursor_key(cursor)){dm.unread_count=0;}
}
fn apply_live(dm:&mut DirectConversation,m:&Message,own:Uuid,read:&HashMap<Uuid,MessageCursor>)->bool{
    if dm.last_message.as_ref().is_some_and(|last|(last.created_at,last.id)>=(m.created_at,m.id)){return false;}
    dm.last_message=Some(crate::models::ChannelLastMessage{id:m.id,content:m.content.clone(),author_id:m.author_id,author_username:None,created_at:m.created_at});
    if m.author_id.is_some_and(|a|a!=own)&&dm.last_read_at.map_or(true,|t|m.created_at>t)
        &&read.get(&dm.id).map_or(true,|cursor|cursor_key(*cursor)<(m.created_at,m.id)){
        dm.unread_count=dm.unread_count.saturating_add(1);
    }true
}
fn merge_snapshot(mut items:Vec<DirectConversation>,live:&[DirectConversation],changed:&HashMap<Uuid,u64>,version:u64)->Vec<DirectConversation>{
    for live in live{if changed.get(&live.id).is_some_and(|v|*v>version){if let Some(d)=items.iter_mut().find(|d|d.id==live.id){*d=live.clone();}else{items.push(live.clone());}}}items
}
impl MainWindowModel {
    pub(super) fn direct_access(&self)->crate::models::Access{
        let mut a=crate::models::Access::direct();a.manage_server=self.server_access.manage_server;a.manage_channels=self.server_access.manage_channels;a.manage_roles=self.server_access.manage_roles;a.ban=self.server_access.ban;a.everyone=self.server_access.everyone;a
    }
    pub(super) fn direct_cancel_selection(&mut self){self.direct.selection=None;}
    pub(super) fn direct_selection_pending(&self)->bool{self.direct.selection.is_some()}
    pub(super) fn can_read_target(&self,id:Uuid)->bool{
        self.direct.items.iter().any(|d|d.id==id)||self.access.get(&id).is_some_and(|a|a.read)&&self.channels.iter().any(|c|c.id==id)
    }
    pub(super) fn publish_direct(&self){self.sidebar.emit(SidebarMsg::SetDirect(self.direct.items.clone()));}
    pub(super) fn direct_snapshot(&mut self,mut dm:DirectConversation,sender:&ComponentSender<Self>){
        if self.direct.busy.contains(&dm.id)||self.direct.hidden.contains(&dm.id)||self.direct.blocked_peers.contains(&dm.user.id){return;}
        reconcile_read(&mut dm,&self.direct.read);
        self.direct.version+=1;self.direct.changed.insert(dm.id,self.direct.version);
        let id=dm.id;let peer=dm.user.id;
        if let Some(d)=self.direct.items.iter_mut().find(|d|d.id==id){*d=dm.clone();}else{self.direct.items.push(dm.clone());}
        self.direct.items.sort_by_key(|d|std::cmp::Reverse(d.last_message.as_ref().map(|m|m.created_at).unwrap_or(d.created_at)));
        if self.active_channel_id==Some(id){self.chat.emit(ChatMsg::UpdateDirect(dm));}
        self.publish_direct();self.load_avatars(vec![peer],false,sender.clone());self.deliver_notices(sender.clone());self.render_inbox(sender);self.render_search(sender);
    }
    pub(super) fn direct_history(&mut self,id:Uuid,messages:&[Message],sender:&ComponentSender<Self>){
        if let Some(dm)=self.direct.items.iter_mut().find(|d|d.id==id){
            observe_read(dm,messages,&mut self.direct.read);
            self.direct.version+=1;self.direct.changed.insert(id,self.direct.version);self.publish_direct();
        }
        self.render_inbox(sender);
    }
    pub(super) fn direct_message(&mut self,m:&Message){
        if let Some(dm)=self.direct.items.iter_mut().find(|d|d.id==m.channel_id){
            if apply_live(dm,m,self.current_user.id,&self.direct.read){
                self.direct.version+=1;self.direct.changed.insert(dm.id,self.direct.version);
            }self.publish_direct();
        }
    }
    pub(super) fn refresh_direct(&mut self,sender:ComponentSender<Self>){
        if self.direct.request.is_some(){self.direct.refresh_again=true;return;}
        let token=Uuid::new_v4();self.direct.request=Some(token);self.direct.last_refresh=Some(Instant::now());
        let version=self.direct.version;let api=self.api_client.clone();
        self.direct.jobs.retain(|j|!j.is_finished());
        self.direct.jobs.push(tokio::spawn(async move{let result=api.list_direct().await;sender.input(MainWindowMsg::Direct(DirectMsg::Listed{token,version,result}));}));
    }
    pub(super) fn tick_direct(&mut self,sender:ComponentSender<Self>){if self.direct.last_refresh.map_or(true,|t|t.elapsed()>=Duration::from_secs(30)){self.refresh_direct(sender);}}
    fn clear_direct(&mut self,id:Uuid){
        self.direct.items.retain(|d|d.id!=id);self.direct.changed.remove(&id);
        if self.active_channel_id==Some(id){self.active_channel_id=None;self.preview_requests.clear();self.chat.emit(ChatMsg::ClearChannel);}
        self.publish_direct();
    }
    pub(super) fn direct_denied(&mut self,sender:ComponentSender<Self>){
        if let Some(id)=self.active_channel_id.filter(|id|self.direct.items.iter().any(|d|d.id==*id)){self.clear_direct(id);}
        self.direct.selection=None;self.refresh_direct(sender.clone());self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);
    }
    fn refresh_blocks(&mut self,sender:ComponentSender<Self>){let token=Uuid::new_v4();self.direct.blocks_request=Some(token);let api=self.api_client.clone();self.direct.jobs.push(tokio::spawn(async move{let result=api.list_blocks().await;sender.input(MainWindowMsg::Direct(DirectMsg::BlocksLoaded{token,result}));}));}
    pub(super) fn direct_event(&mut self,msg:DirectMsg,sender:ComponentSender<Self>,root:&gtk::Box){
        match msg {
            DirectMsg::Refresh=>self.refresh_direct(sender),
            DirectMsg::Open(user)=>{
                if user==self.current_user.id{self.chat.emit(ChatMsg::OperationError("Não é possível abrir uma DM consigo mesmo.".into()));return;}
                let token=Uuid::new_v4();self.direct.selection=Some(token);let api=self.api_client.clone();let own=self.current_user.id;
                self.direct.jobs.push(tokio::spawn(async move{let result=api.open_direct(own,user).await;sender.input(MainWindowMsg::Direct(DirectMsg::Selected{token,message:None,result}));}));
            }
            DirectMsg::Select(id,message)=>{
                let token=Uuid::new_v4();self.direct.selection=Some(token);let api=self.api_client.clone();
                self.direct.jobs.push(tokio::spawn(async move{let result=api.get_direct(id).await;sender.input(MainWindowMsg::Direct(DirectMsg::Selected{token,message,result}));}));
            }
            DirectMsg::Selected{token,message,result}=>{
                if self.direct.selection!=Some(token){return;}self.direct.selection=None;
                match result {
                    Ok(dm)=>{
                        let id=dm.id;self.close_profile_for_direct(dm.user.id);self.direct.hidden.remove(&id);self.direct.hidden_since.remove(&id);self.direct_snapshot(dm.clone(),&sender);
                        self.chat.emit(ChatMsg::SetAccess{user_id:self.current_user.id,access:self.direct_access()});
                        if self.active_channel_id!=Some(id){self.active_channel_id=Some(id);self.preview_requests.clear();self.notifications.read_pending=false;self.chat.emit(ChatMsg::SetDirect(dm));self.load_history(id,None,sender.clone());}
                        self.sidebar.emit(SidebarMsg::SetSelection(id));
                        if let Some(message_id)=message{self.chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::Navigate(message_id)));}
                    }
                    Err(e)=>{self.profile_action_status(&e.to_string());if crate::api::is_permission_error(&e){self.direct_denied(sender.clone());}sender.input(MainWindowMsg::OperationFailed(e));}
                }
            }
            DirectMsg::Listed{token,version,result}=>{
                if self.direct.request!=Some(token){return;}self.direct.request=None;
                if std::mem::take(&mut self.direct.refresh_again){self.refresh_direct(sender.clone());}
                match result {
                    Ok(mut items)=>{
                        items=merge_snapshot(items,&self.direct.items,&self.direct.changed,version);
                        items.retain(|d|!self.direct.busy.contains(&d.id)&&!self.direct.hidden_since.get(&d.id).is_some_and(|v|*v>version)&&!self.direct.blocked_peers.contains(&d.user.id));
                        for d in &mut items{reconcile_read(d,&self.direct.read);}
                        if let Some(id)=self.active_channel_id.filter(|id|self.direct.items.iter().any(|d|d.id==*id)&&!items.iter().any(|d|d.id==*id)){self.clear_direct(id);}
                        for d in &items{self.direct.hidden.remove(&d.id);self.direct.hidden_since.remove(&d.id);}
                        self.direct.items=items;self.direct.changed.retain(|_,v|*v>version);self.publish_direct();
                        self.load_avatars(self.direct.items.iter().map(|d|d.user.id).collect(),false,sender.clone());self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);
                    }
                    Err(e)=>{self.direct_denied_without_refresh();self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);sender.input(MainWindowMsg::OperationFailed(e));}
                }
            }
            DirectMsg::Hide(id)=>{
                if !self.direct.busy.insert(id){return;}self.direct.selection=None;let api=self.api_client.clone();
                self.direct.jobs.push(tokio::spawn(async move{let result=api.hide_direct(id).await;sender.input(MainWindowMsg::Direct(DirectMsg::Hidden{id,result}));}));
            }
            DirectMsg::Hidden{id,result}=>{self.direct.busy.remove(&id);match result{Ok(())=>{self.direct.version+=1;self.direct.hidden_since.insert(id,self.direct.version);self.direct.hidden.insert(id);self.clear_direct(id);self.refresh_direct(sender.clone());},Err(e)=>sender.input(MainWindowMsg::OperationFailed(e))}self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);}
            DirectMsg::Blocks=>{
                if let Some(v)=self.direct.blocks.take(){v.window.close();}
                let (window,body)=crate::ui::chat::actions::window(root,"Usuários bloqueados");
                let info=gtk::Label::new(Some("Bloquear impede acesso e envio de DMs entre os dois usuários. O histórico do servidor permanece visível."));info.set_wrap(true);body.append(&info);
                let refresh=gtk::Button::with_label("Atualizar bloqueios");let s=sender.clone();refresh.connect_clicked(move |_|s.input(MainWindowMsg::Direct(DirectMsg::Blocks)));body.append(&refresh);
                let status=gtk::Label::new(Some("Carregando…"));status.set_wrap(true);body.append(&status);let rows=gtk::Box::new(gtk::Orientation::Vertical,4);body.append(&rows);window.present();self.direct.blocks=Some(BlockView{window,rows,status});self.refresh_blocks(sender);
            }
            DirectMsg::BlocksLoaded{token,result}=>{
                if self.direct.blocks_request!=Some(token){return;}self.direct.blocks_request=None;
                if let Some(v)=&self.direct.blocks{while let Some(c)=v.rows.first_child(){v.rows.remove(&c);}
                    match result{Ok(users)=>{self.direct.blocked_peers=users.iter().map(|u|u.id).collect();v.status.set_text(if users.is_empty(){"Nenhum usuário bloqueado."}else{""});for user in users{let b=gtk::Button::with_label(&format!("Desbloquear {}",user.display_name()));let id=user.id;let s=sender.clone();b.connect_clicked(move |_|s.input(MainWindowMsg::Direct(DirectMsg::Block(id,false))));v.rows.append(&b);}},Err(e)=>{v.status.set_text(&e.to_string());sender.input(MainWindowMsg::ActionError(e));}}
                }
            }
            DirectMsg::Block(id,block)=>{
                if id==self.current_user.id||!self.direct.busy.insert(id){return;}
                let api=self.api_client.clone();let own=self.current_user.id;self.direct.selection=None;
                self.direct.jobs.push(tokio::spawn(async move{let result=api.block_user(own,id,block).await;sender.input(MainWindowMsg::Direct(DirectMsg::Blocked{id,block,result}));}));
            }
            DirectMsg::Blocked{id,block,result}=>{
                self.direct.busy.remove(&id);
                match result{Ok(())=>{if block{self.direct.blocked_peers.insert(id);let ids:Vec<_>=self.direct.items.iter().filter(|d|d.user.id==id).map(|d|d.id).collect();for id in ids{self.clear_direct(id);}}else{self.direct.blocked_peers.remove(&id);}
                    self.refresh_direct(sender.clone());self.refresh_blocks(sender.clone());if let Some(v)=&self.direct.blocks{v.status.set_text("Bloqueio atualizado.");}
                    self.profile_action_status(if block{"Usuário bloqueado. DMs indisponíveis."}else{"Usuário desbloqueado."});
                    self.chat.emit(ChatMsg::OperationError(if block{"Usuário bloqueado. DMs indisponíveis."}else{"Usuário desbloqueado. Abra o perfil para reabrir a DM."}.into()));
                },Err(e)=>{self.profile_action_status(&e.to_string());if let Some(v)=&self.direct.blocks{v.status.set_text(&e.to_string());}sender.input(MainWindowMsg::OperationFailed(e));}}
                self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);
            }
        }
    }
    fn direct_denied_without_refresh(&mut self){if let Some(id)=self.active_channel_id.filter(|id|self.direct.items.iter().any(|d|d.id==*id)){self.clear_direct(id);}self.direct.items.clear();self.publish_direct();}
}

#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{descendants,find_button,pump,until};
    let peer=Uuid::parse_str("12345678-1234-4234-8234-123456789ac0").unwrap();let dm=Uuid::parse_str("12345678-1234-4234-8234-123456789ac3").unwrap();
    let public=main.model().channels[0].clone();
    let composer=||descendants(main.model().chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();
    main.emit(MainWindowMsg::ChannelSelected(public.clone()));pump(context);composer().set_text("server draft");pump(context);
    main.emit(MainWindowMsg::Account(AccountMsg::Profile(peer)));until(context,||gtk::Window::list_toplevels().iter().any(|w|descendants(w).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Mensagem direta"))));
    let profile=gtk::Window::list_toplevels().into_iter().find(|w|descendants(w).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Mensagem direta"))).unwrap();find_button(&profile,"Mensagem direta").emit_clicked();
    until(context,||main.model().active_channel_id==Some(dm)&&main.model().chat.model().active_channel.as_ref().is_some_and(|t|t.is_direct()));
    until(context,||main.model().avatars.textures.contains_key(&peer));
    assert!(!main.model().channels.iter().any(|c|c.id==dm));assert!(!main.model().access.contains_key(&dm));
    composer().set_text("DM test");pump(context);main.model().chat.emit(ChatMsg::SendClicked);
    until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::TextView>()).any(|v|v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false)=="DM test"));
    until(context,||find_button(main.model().chat.widget().upcast_ref(),"Fixar").is_sensitive());
    find_button(main.model().chat.widget().upcast_ref(),"Fixar").emit_clicked();until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Desafixar")&&b.is_sensitive()));
    find_button(main.model().chat.widget().upcast_ref(),"Desafixar").emit_clicked();until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Fixar")&&b.is_sensitive()));
    composer().set_text("DM draft");pump(context);
    main.emit(MainWindowMsg::ChannelSelected(public.clone()));pump(context);assert_eq!(composer().text(),"server draft");
    main.emit(MainWindowMsg::Direct(DirectMsg::Select(dm,None)));until(context,||main.model().active_channel_id==Some(dm)&&composer().text()=="DM draft");
    main.emit(MainWindowMsg::WsReceived(WsEvent::PresenceUpdate(crate::ws::PresenceEntry{user_id:peer,status:crate::ws::PresenceStatus::Busy,status_message:None,typing:None,nickname:None,user_voice:vec![]})));pump(context);assert_eq!(main.model().direct.presence[&peer],crate::ws::PresenceStatus::Busy);
    // Hold a list result while many refresh triggers arrive: retain its token
    // and schedule only one successor rather than starting discarded requests.
    let held=Uuid::new_v4();let version=main.model().direct.version;let items=main.model().direct.items.clone();
    main.state().get_mut().model.direct.request=Some(held);let jobs=main.model().direct.jobs.len();
    for _ in 0..50{main.emit(MainWindowMsg::Direct(DirectMsg::Refresh));}pump(context);
    assert_eq!(main.model().direct.request,Some(held));assert!(main.model().direct.refresh_again);assert_eq!(main.model().direct.jobs.len(),jobs);
    main.emit(MainWindowMsg::Direct(DirectMsg::Listed{token:held,version,result:Ok(items)}));until(context,||main.model().direct.request.is_none());assert!(!main.model().direct.refresh_again);
    // An older REST request cannot overwrite a newer dm_update or confirmed read.
    let old_token=Uuid::new_v4();let version=main.model().direct.version;
    main.emit(MainWindowMsg::Direct(DirectMsg::Refresh));until(context,||main.model().direct.request.is_none());
    let mut snapshot=main.model().direct.items[0].clone();snapshot.unread_count=4;
    main.emit(MainWindowMsg::WsReceived(WsEvent::DirectUpdate(snapshot.clone())));pump(context);assert_eq!(main.model().direct.items[0].unread_count,4);
    main.emit(MainWindowMsg::Direct(DirectMsg::Listed{token:old_token,version,result:Ok(vec![])}));pump(context);assert_eq!(main.model().direct.items[0].unread_count,4);
    main.emit(MainWindowMsg::Direct(DirectMsg::Hide(dm)));until(context,||main.model().direct.request.is_none()&&main.model().direct.items.is_empty()&&main.model().active_channel_id.is_none());
    main.emit(MainWindowMsg::Direct(DirectMsg::Open(peer)));until(context,||main.model().active_channel_id==Some(dm)&&composer().text()=="DM draft");
    until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::TextView>()).any(|v|v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false)=="DM test"));
    main.emit(MainWindowMsg::Direct(DirectMsg::Block(peer,true)));until(context,||main.model().direct.busy.is_empty()&&main.model().direct.items.is_empty()&&main.model().active_channel_id.is_none());
    main.emit(MainWindowMsg::Direct(DirectMsg::Open(peer)));until(context,||main.model().direct.selection.is_none());assert!(main.model().direct.items.is_empty());assert!(!main.model().channels.is_empty());
    main.emit(MainWindowMsg::Direct(DirectMsg::Blocks));until(context,||main.model().direct.blocks_request.is_none()&&main.model().direct.blocks.is_some());let w=main.model().direct.blocks.as_ref().unwrap().window.clone();find_button(w.upcast_ref(),"Desbloquear Bob").emit_clicked();until(context,||!main.model().direct.blocked_peers.contains(&peer)&&main.model().direct.busy.is_empty()&&main.model().direct.blocks_request.is_none());
    main.emit(MainWindowMsg::Direct(DirectMsg::Open(peer)));until(context,||main.model().active_channel_id==Some(dm));main.emit(MainWindowMsg::WsReceived(WsEvent::Reconnected));until(context,||main.model().direct.request.is_none()&&main.model().access_request.is_none());assert_eq!(main.model().active_channel_id,Some(dm));
    main.emit(MainWindowMsg::Direct(DirectMsg::Hide(dm)));until(context,||main.model().direct.items.is_empty()&&main.model().direct.request.is_none());main.emit(MainWindowMsg::ChannelSelected(public));pump(context);assert_eq!(composer().text(),"server draft");
    if let Ok(w)=profile.downcast::<gtk::Window>(){w.close();}w.close();
}

#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    fn fixture()->(DirectConversation,Vec<Message>){
        let id=Uuid::new_v4();let first=Uuid::from_u128(1);let next=Uuid::from_u128(2);let date="2026-10-03T00:00:00Z";
        let dm=serde_json::from_value(json!({"id":id,"user":{"id":Uuid::new_v4(),"username":"peer","created_at":date},"created_at":date,"unread_count":2,"last_read_at":"2026-10-05T00:00:00Z","last_message":{"id":next,"created_at":date}})).unwrap();
        let messages=[first,next].into_iter().map(|msg|serde_json::from_value(json!({"id":msg,"channel_id":id,"content":"hi","created_at":date})).unwrap()).collect();(dm,messages)
    }
    #[test]
    fn dm_reads_use_message_cursor_not_read_operation_time_and_never_regress(){
        let (mut dm,messages)=fixture();let mut read=HashMap::new();observe_read(&mut dm,&messages,&mut read);assert_eq!(dm.last_read_message,Some(messages[1].id));assert_eq!(dm.unread_count,0);
        observe_read(&mut dm,&messages[..1],&mut read);assert_eq!(dm.last_read_message,Some(messages[1].id));assert_eq!(dm.unread_count,0);
        let mut stale=dm.clone();stale.last_read_message=None;stale.unread_count=2;reconcile_read(&mut stale,&read);assert_eq!(stale.unread_count,0);assert_eq!(stale.last_read_message,Some(messages[1].id));
    }
    #[test]
    fn in_flight_dm_list_cannot_replace_a_newer_event_or_read(){
        let (old,_)=fixture();let mut live=old.clone();live.unread_count=0;let changed=HashMap::from([(old.id,2)]);
        assert_eq!(merge_snapshot(vec![old.clone()],&[live.clone()],&changed,1)[0].unread_count,0);
        assert_eq!(merge_snapshot(vec![old.clone()],&[live],&changed,2)[0].unread_count,2);
    }
    #[test]
    fn own_and_already_read_dm_messages_never_increment_unread(){
        let (mut dm,messages)=fixture();let own=Uuid::new_v4();let mut m=messages[1].clone();m.id=Uuid::from_u128(3);m.author_id=Some(own);dm.unread_count=0;
        assert!(apply_live(&mut dm,&m,own,&HashMap::new()));assert_eq!(dm.unread_count,0);
        m.id=Uuid::from_u128(4);m.author_id=Some(dm.user.id);assert!(apply_live(&mut dm,&m,own,&HashMap::new()));assert_eq!(dm.unread_count,0); // read operation occurred later
        m.id=Uuid::from_u128(5);m.created_at=chrono::DateTime::parse_from_rfc3339("2026-10-06T00:00:00Z").unwrap().with_timezone(&chrono::Utc);assert!(apply_live(&mut dm,&m,own,&HashMap::new()));assert_eq!(dm.unread_count,1);
        assert!(!apply_live(&mut dm,&m,own,&HashMap::new()));assert_eq!(dm.unread_count,1);
    }
}
