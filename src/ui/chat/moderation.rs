//! EN-US: Events drive moderation updates; legacy servers get one batched fallback per covered group.
//! PT-BR: Eventos conduzem a moderação; servidores antigos recebem apenas uma consulta alternativa em lote para cada grupo coberto.
use super::*;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Poll {
    channel:Option<Uuid>,
    user:Option<Uuid>,
    timer:Option<(Uuid,gtk::glib::SourceId)>,
    request:Option<Uuid>,
    job:Option<tokio::task::JoinHandle<()>>,
    checked:HashSet<Uuid>,
    denied:bool,
}
impl Poll {
    fn stop(&mut self)->Option<Uuid>{
        if let Some((_,timer))=self.timer.take(){timer.remove();}
        if let Some(job)=self.job.take(){job.abort();}
        self.request.take()
    }
}
impl Drop for Poll{fn drop(&mut self){self.stop();}}

const FALLBACK_DELAY:std::time::Duration=std::time::Duration::from_secs(3);
impl ChatModel {
    pub(super) fn apply_moderation_change(&mut self,message:Uuid,key:Uuid,status:&str)->bool{
        if self.history.is_deleted(message){return false;}
        let previous=self.history.messages.iter().find(|m|m.id==message).and_then(|m|m.attachments.as_ref()).and_then(|a|a.iter().find(|a|a.id==key)).cloned();
        if status!="blocked"&&previous.as_ref().is_some_and(|a|a.moderation_status.as_deref()==Some(status)){return false;}
        let mut invalidate=false;
        if let Some(previous)=&previous{
            let was_visible=self.transfers.visible(previous);let mut updated=previous.clone();updated.moderation_status=Some(status.into());
            invalidate=was_visible!=self.transfers.visible(&updated)||status=="blocked";
        }
        if matches!(status,"clean"|"failed"|"sensitive"|"blocked"){self.transfers.sent_attachments.remove(&key);}
        if invalidate{self.transfers.invalidate(key);}
        if status=="blocked"{self.transfers.invalidate_message(message);}
        self.history.apply(Change::Moderation(message,key,status.into()));
        previous.is_some()||status=="blocked"
    }
    fn pending_moderation(&self)->Vec<&Message>{
        self.history.messages.iter().skip(self.render_window.range.start).take(self.render_window.range.len())
            .filter(|m|self.transfers.scope.contains(&m.id)&&m.attachments.iter().flatten().any(|a|!self.transfers.sent_attachments.contains(&a.id)&&matches!(a.moderation_status.as_deref(),Some("pending"|"processing"))))
            .collect()
    }
    pub(super) fn schedule_moderation(&mut self,sender:&ComponentSender<Self>,mapped:bool){
        if self.actions.pinned_view{return;}
        let channel=self.active_channel.as_ref().map(|c|c.id());
        if self.moderation.channel!=channel||self.moderation.user!=self.user_id{
            if let Some(request)=self.moderation.stop(){self.history.fail_snapshot(request);}
            self.moderation=Poll::default();self.moderation.channel=channel;self.moderation.user=self.user_id;
        }
        if !self.access.read{self.moderation.denied=false;}
        let pending:HashSet<_>=self.history.messages.iter().filter(|m|m.attachments.iter().flatten().any(|a|matches!(a.moderation_status.as_deref(),Some("pending"|"processing")))).map(|m|m.id).collect();
        self.moderation.checked.retain(|id|pending.contains(id));
        // An in-flight read keeps its journal while the viewport moves. Do not
        // cancel it merely because its batch has just been marked as checked.
        if self.moderation.request.is_some()&&mapped&&self.access.read{return;}
        let eligible=mapped&&self.access.read&&self.history_ready&&!self.history.loading()&&!self.viewport.scrolling()&&self.actions.api.is_some()&&!self.moderation.denied;
        let needed=eligible&&self.pending_moderation().into_iter().any(|m|!self.moderation.checked.contains(&m.id));
        if !needed{
            if let Some(request)=self.moderation.stop(){self.history.fail_snapshot(request);}
            return;
        };
        if self.moderation.timer.is_some()||self.moderation.request.is_some(){return;}
        let token=Uuid::new_v4();let input=sender.input_sender().clone();
        // Keep the source alive until its queued input is consumed, so a
        // channel switch can safely cancel it even after the timeout fires.
        let timer=gtk::glib::timeout_add_local(FALLBACK_DELAY,move||{let _=input.send(ChatMsg::CheckModeration(token));gtk::glib::ControlFlow::Continue});
        self.moderation.timer=Some((token,timer));
    }
    pub(super) fn check_moderation(&mut self,token:Uuid,sender:&ComponentSender<Self>){
        if !self.moderation.timer.as_ref().is_some_and(|(id,_)|*id==token){return;}
        if let Some((_,timer))=self.moderation.timer.take(){timer.remove();}
        if !self.access.read||self.viewport.scrolling()||self.history.loading(){return;}
        let target=self.pending_moderation().into_iter().filter(|m|!self.moderation.checked.contains(&m.id))
            .max_by_key(|m|(m.created_at,m.id)).cloned();
        let (Some(target),Some(api))=(target,self.actions.api.clone())else{return;};
        // The inclusive page covers this message and its 29 predecessors.
        // Mark the whole covered batch once, including failed/unchanged replies.
        let covered=self.history.messages.iter().rev().filter(|m|(m.created_at,m.id)<=(target.created_at,target.id)).take(30).map(|m|m.id);
        self.moderation.checked.extend(covered);
        let request=Uuid::new_v4();self.moderation.request=Some(request);self.history.begin_snapshot(request);
        let input=sender.input_sender().clone();
        self.moderation.job=Some(tokio::spawn(async move{
            let result=api.moderation_messages(target.channel_id,&target).await;
            let _=input.send(ChatMsg::ModerationChecked{request,result});
        }));
    }
    pub(super) fn moderation_checked(&mut self,request:Uuid,result:anyhow::Result<MessageListResponse>)->bool{
        if self.moderation.request!=Some(request){return false;}
        self.moderation.request=None;self.moderation.job.take();
        match result{
            Ok(page)=>{
                let changes=self.history.finish_moderation(request,page.messages);let mut changed=false;
                for change in changes{
                    if let Change::Moderation(message,key,status)=change{changed|=self.apply_moderation_change(message,key,&status);}
                }
                return changed;
            },
            Err(error)=>{self.history.fail_snapshot(request);if crate::api::is_permission_error(&error){self.moderation.denied=true;}},
        }
        false
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[tokio::test]async fn cancellation_aborts_fallback_without_replenishing_its_budget(){
        let mut fallback=Poll::default();let request=Uuid::new_v4();let message=Uuid::new_v4();fallback.request=Some(request);fallback.checked.insert(message);
        let job=tokio::spawn(std::future::pending::<()>());let cancelled=job.abort_handle();fallback.job=Some(job);
        assert_eq!(fallback.stop(),Some(request));tokio::task::yield_now().await;assert!(cancelled.is_finished());assert!(fallback.checked.contains(&message));assert_eq!(fallback.stop(),None);
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    exercise_events(context);
    use actions::tests::{mock,descendants,pump,until};
    use performance::settle;
    let (api,backend,task)=tokio::runtime::Handle::current().block_on(mock());
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let id=Uuid::new_v4();let key=Uuid::new_v4();let now=chrono::Utc::now();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Moderation","created_at":now})).unwrap();
    let value=serde_json::json!({"id":id,"channel_id":channel,"author_id":user,"content":"Uploaded image","created_at":now,"attachments":[{"id":key,"mime_type":"image/png","original_file_name":"image.png","size_bytes":1,"created_at":now,"moderation_status":"pending"}]});
    let pending:Message=serde_json::from_value(value.clone()).unwrap();
    // Mirror the actual storage JOIN: history returns an empty status although
    // the upload echo says pending. A synthetic clean verdict misses this bug.
    let mut complete=value;complete["attachments"][0]["moderation_status"]=serde_json::json!("");
    let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(30,20).write_to(&mut png,image::ImageFormat::Png).unwrap();
    {let mut b=backend.lock().unwrap();b.channel_messages.insert(channel.to_string(),vec![complete.clone()]);b.attachment_thumbnails.insert(format!("/attachments/{key}/thumbnail"),png.into_inner());}
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![pending.clone()],api:Some(api),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(950).default_height(650).content(chat.widget()).build();window.present();settle(context);
    chat.emit(ChatMsg::AddMessage(pending.clone()));
    // The real POST echo can precede or follow the WS arrival.
    chat.emit(ChatMsg::SendFinished{request_id:Uuid::new_v4(),channel_id:channel,result:Ok(pending.clone())});
    chat.state().get_mut().model.history_ready=true;chat.emit(ChatMsg::MediaViewport);pump(context);
    let waiting=||descendants(chat.widget().upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Aguardando moderação"));
    assert!(waiting());assert!(chat.model().transfers.textures.is_empty(),"pending image is not fetched early");
    until(context,||chat.model().transfers.textures.contains_key(&key)&&chat.model().media_render_pending.is_none());
    assert!(!waiting(),"approval must update the visible row without switching channels");
    assert_eq!(chat.model().active_channel.as_ref().unwrap().id(),channel);
    let reads=||backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p.starts_with(&format!("/channels/{channel}/messages?"))).count();
    assert_eq!(reads(),1);assert!(chat.model().moderation.timer.is_none()&&chat.model().moderation.request.is_none());
    chat.emit(ChatMsg::SendFinished{request_id:Uuid::new_v4(),channel_id:channel,result:Ok(pending.clone())});settle(context);assert!(!waiting());assert_eq!(reads(),1,"late POST cannot restart pending checks");
    // Older compatible servers omit the field entirely instead of using "".
    complete["attachments"][0].as_object_mut().unwrap().remove("moderation_status");backend.lock().unwrap().channel_messages.insert(channel.to_string(),vec![complete.clone()]);
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(id,key,"pending".into())));settle(context);
    until(context,||chat.model().transfers.textures.contains_key(&key)&&chat.model().media_render_pending.is_none());assert!(!waiting());
    // Sensitive completion stays behind the existing explicit reveal action.
    complete["attachments"][0]["moderation_status"]=serde_json::json!("sensitive");backend.lock().unwrap().channel_messages.insert(channel.to_string(),vec![complete.clone()]);
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(id,key,"pending".into())));settle(context);
    until(context,||chat.model().history.messages[0].attachments.as_ref().unwrap()[0].moderation_status.as_deref()==Some("sensitive"));
    assert!(!chat.model().transfers.textures.contains_key(&key));assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Conteúdo sensível — revelar"))));
    // Cancelling a channel cancels its timer and rejects a delayed HTTP result.
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(id,key,"pending".into())));settle(context);
    assert!(chat.model().moderation.timer.is_some());let stale=Uuid::new_v4();chat.state().get_mut().model.moderation.request=Some(stale);chat.state().get_mut().model.history.begin_snapshot(stale);
    chat.emit(ChatMsg::ClearChannel);pump(context);assert!(chat.model().moderation.timer.is_none()&&chat.model().moderation.request.is_none());
    chat.emit(ChatMsg::ModerationChecked{request:stale,result:Ok(MessageListResponse{channel_id:channel,messages:vec![serde_json::from_value(complete).unwrap()],has_more:false})});pump(context);assert!(chat.model().history.messages.is_empty());
    window.close();drop(chat);task.abort();pump(context);
}

#[cfg(test)]fn exercise_events(context:&gtk::glib::MainContext){
    use actions::tests::{mock,descendants,pump,until};use performance::settle;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let peer=Uuid::new_v4();let now=chrono::Utc::now();
    let message=|author,offset:i64|->Message{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":author,"created_at":now+chrono::Duration::seconds(offset),"attachments":[{"id":Uuid::new_v4(),"mime_type":"image/png","original_file_name":"photo.png","size_bytes":1,"created_at":now,"moderation_status":"pending"}]})).unwrap()};
    let pending=message(peer,0);let key=pending.attachments.as_ref().unwrap()[0].id;
    // Short images keep all three rows in view, isolating event handling from
    // deliberate offscreen eviction and scroll anchoring.
    let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(300,20).write_to(&mut png,image::ImageFormat::Png).unwrap();let png=png.into_inner();
    backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{key}/thumbnail"),png.clone());
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Moderation events","created_at":now})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![pending.clone()],api:Some(api),user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(950).default_height(760).content(chat.widget()).build();window.present();settle(context);
    let deliver=|kind:&str,id:Uuid,key:Uuid,status:&str|{
        let event=crate::ws::parse_event(&serde_json::json!({"type":kind,"channel_id":channel,"message_id":id,"attachment_id":key,"status":status}).to_string()).unwrap();
        let crate::ws::WsEvent::AttachmentModeration{message_id,attachment_id,status,..}=event else{panic!("wrong event");};chat.emit(ChatMsg::ApplyChange(Change::Moderation(message_id,attachment_id,status)));
    };
    let reads=||backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="GET"&&path.starts_with(&format!("/channels/{channel}/messages?"))).count();
    let downloads=|key:Uuid|backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p==&format!("/attachments/{key}/thumbnail")).count();
    deliver("attachment_moderation_update",pending.id,key,"clean");pump(context);
    let row=chat.model().rendered[&format!("message-{}",pending.id)].1.clone();
    assert!(!descendants(row.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Aguardando moderação")),"the event updates the row immediately");
    until(context,||chat.model().transfers.textures.contains_key(&key)&&chat.model().media_render_pending.is_none());settle(context);
    let texture=chat.model().transfers.textures[&key].clone();let passes=chat.model().render_passes;
    for _ in 0..25{deliver("attachment_moderation_update",pending.id,key,"clean");}pump(context);
    assert_eq!(chat.model().render_passes,passes,"duplicate moderation cannot rebuild the timeline");assert_eq!(chat.model().transfers.textures[&key],texture);assert_eq!(downloads(key),1);assert_eq!(reads(),0);
    // A completion which precedes its message is replayed without polling.
    let early=message(peer,1);let early_key=early.attachments.as_ref().unwrap()[0].id;backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{early_key}/thumbnail"),png.clone());
    deliver("moderation_update",early.id,early_key,"clean");chat.emit(ChatMsg::AddMessage(early));until(context,||chat.model().transfers.textures.contains_key(&early_key));
    // A successful own upload displays its authorized thumbnail immediately,
    // retaining pending metadata until the real completion event arrives.
    let own=message(user,2);let own_key=own.attachments.as_ref().unwrap()[0].id;backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{own_key}/thumbnail"),png.clone());
    let file=tokio::runtime::Handle::current().block_on(crate::api::features::UploadFile::clipboard_png(&png)).unwrap();
    let request={let mut state=chat.state().get_mut();state.model.draft.files.push(file);state.model.draft.begin().unwrap()};
    chat.emit(ChatMsg::SendFinished{request_id:request,channel_id:channel,result:Ok(own.clone())});until(context,||chat.model().transfers.textures.contains_key(&own_key)&&chat.model().media_render_pending.is_none());
    assert_eq!(chat.model().history.messages.iter().find(|m|m.id==own.id).unwrap().attachments.as_ref().unwrap()[0].moderation_status.as_deref(),Some("pending"));assert!(chat.model().moderation.timer.is_none());assert_eq!(reads(),0);
    settle(context);assert_eq!(downloads(own_key),1);let own_texture=chat.model().transfers.textures[&own_key].clone();deliver("moderation_update",own.id,own_key,"clean");settle(context);assert_eq!(chat.model().transfers.textures[&own_key],own_texture);assert_eq!(downloads(own_key),1);
    deliver("moderation_update",own.id,own_key,"sensitive");pump(context);assert!(!chat.model().transfers.textures.contains_key(&own_key));assert!(!chat.model().transfers.visible(&chat.model().history.messages.iter().find(|m|m.id==own.id).unwrap().attachments.as_ref().unwrap()[0]));
    chat.emit(ChatMsg::Transfer(transfers::TransferMsg::Reveal{message:own.id,attachment:own_key}));until(context,||chat.model().transfers.textures.contains_key(&own_key));settle(context);let count=downloads(own_key);
    deliver("moderation_update",own.id,own_key,"sensitive");pump(context);assert!(chat.model().transfers.visible(&chat.model().history.messages.iter().find(|m|m.id==own.id).unwrap().attachments.as_ref().unwrap()[0]));assert!(chat.model().transfers.textures.contains_key(&own_key));assert_eq!(downloads(own_key),count);
    // Several unchanged pending messages share ONE fallback, never a retry loop.
    let legacy:Vec<_>=(3..6).map(|offset|message(peer,offset)).collect();let ids:Vec<_>=legacy.iter().map(|m|m.id).collect();
    let values=legacy.iter().map(|m|serde_json::json!({"id":m.id,"channel_id":channel,"author_id":peer,"created_at":m.created_at,"attachments":[{"id":m.attachments.as_ref().unwrap()[0].id,"mime_type":"image/png","original_file_name":"photo.png","size_bytes":1,"created_at":now,"moderation_status":"pending"}]})).collect();backend.lock().unwrap().channel_messages.insert(channel.to_string(),values);
    for m in legacy{chat.emit(ChatMsg::AddMessage(m));}chat.emit(ChatMsg::MediaViewport);settle(context);
    let token=chat.model().moderation.timer.as_ref().unwrap().0;chat.emit(ChatMsg::CheckModeration(token));until(context,||chat.model().moderation.request.is_none()&&ids.iter().all(|id|chat.model().moderation.checked.contains(id)));
    assert_eq!(reads(),1);assert!(chat.model().moderation.timer.is_none());for _ in 0..10{chat.emit(ChatMsg::MediaViewport);}settle(context);assert_eq!(reads(),1);assert!(chat.model().moderation.timer.is_none());
    deliver("attachment_moderation_update",own.id,own_key,"blocked");pump(context);assert!(!chat.model().history.messages.iter().any(|m|m.id==own.id));
    window.close();drop(chat);server.abort();pump(context);
}
