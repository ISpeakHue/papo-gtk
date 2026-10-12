mod desktop;
pub(crate) mod state;
use super::*;
use crate::models::{Message,NotificationList,NotificationEvent};
use state::{Inbox,ReadJournal};
use std::collections::HashSet;
#[derive(Debug)]
pub enum NoticeMsg {
    Open, Refresh, More, Mark(Uuid), OpenMessage(Uuid),
    Loaded{token:Uuid,cursor:Option<MessageCursor>,result:anyhow::Result<NotificationList>},
    Marked{id:Uuid,result:anyhow::Result<u32>},
    MarkedBatch{ids:Vec<Uuid>,result:anyhow::Result<u32>},
    DesktopReady(Result<gtk::gio::DBusProxy,gtk::glib::Error>),
}
#[derive(Default)]
pub(super) struct Notifications {
    pub inbox:Inbox,pub journal:ReadJournal,view:Option<InboxView>,
    request:Option<Uuid>,refresh_again:bool,cursor:Option<MessageCursor>,more:bool,
    marks:HashSet<Uuid>,failed_marks:HashSet<Uuid>,read_error:Option<String>,seen_events:HashSet<Uuid>,pub(super) jobs:Vec<tokio::task::JoinHandle<()>>,desktop:desktop::Desktop,
    pending_open:Option<Uuid>,
    pub read_pending:bool,pub read_request:Option<Uuid>,
}
struct InboxView {window:gtk::Window,rows:gtk::Box,status:gtk::Label,more:gtk::Button}
impl Drop for Notifications{fn drop(&mut self){for job in self.jobs.drain(..){job.abort();}if let Some(v)=self.view.take(){v.window.close();}}}
impl MainWindowModel {
    fn notification_channels(&self)->Vec<state::NoticeChannel>{
        self.channels.iter().filter(|c|self.access.get(&c.id).is_some_and(|a|a.read)).cloned().map(Into::into)
        .chain(self.direct.items.iter().map(|d|state::NoticeChannel{id:d.id,name:format!("DM · {}",d.user.display_name()),notification_settings:Some(crate::models::NotificationSettings::All)})).collect()
    }

    pub(super) fn notification_event(&mut self,msg:NoticeMsg,root:&gtk::Box,sender:ComponentSender<Self>){
        match msg {
            NoticeMsg::Open=>{
                self.notifications.failed_marks.clear();self.notifications.read_error=None;
                if let Some(v)=self.notifications.view.as_ref().filter(|v|v.window.is_visible()){v.window.present();self.mark_open_notices(&sender);sender.input(MainWindowMsg::Notification(NoticeMsg::Refresh));return;}
                let window=gtk::Window::builder().title("Notificações").default_width(550).default_height(550).build();if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
                let body=gtk::Box::new(gtk::Orientation::Vertical,8);body.set_margin_top(16);body.set_margin_bottom(16);body.set_margin_start(16);body.set_margin_end(16);
                let refresh=gtk::Button::with_label("Atualizar notificações");let input=sender.input_sender().clone();refresh.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::Refresh));});body.append(&refresh);
                let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);let rows=gtk::Box::new(gtk::Orientation::Vertical,8);body.append(&gtk::ScrolledWindow::builder().vexpand(true).child(&rows).build());
                let more=gtk::Button::with_label("Mais notificações");let input=sender.input_sender().clone();more.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::More));});body.append(&more);window.set_child(Some(&body));window.present();
                self.notifications.view=Some(InboxView{window,rows,status,more});self.load_notice_authors(&sender);self.render_inbox(&sender);self.mark_open_notices(&sender);self.refresh_notifications(None,sender);
            }
            NoticeMsg::Refresh=>self.refresh_notifications(None,sender),
            NoticeMsg::More=>{if self.notifications.more&&self.notifications.request.is_none(){self.refresh_notifications(self.notifications.cursor,sender);}}
            NoticeMsg::Loaded{token,cursor,result}=>{
                if self.notifications.request!=Some(token){return;}self.notifications.request=None;
                match result {
                    Ok(page)=>{
                        let next=page.notifications.last().map(|n|MessageCursor{created_at:n.created_at,id:n.id});
                        self.notifications.more=page.has_more&&next.is_some()&&next!=cursor;self.notifications.cursor=next;
                        for n in &page.notifications{if n.read{self.notifications.seen_events.remove(&n.id);}}
                        if cursor.is_none(){self.notifications.inbox.merge_head(page.notifications);}else{self.notifications.inbox.merge(page.notifications);}
                        let rows=&self.notifications.inbox.rows;let seen=&self.notifications.seen_events;self.notifications.failed_marks.retain(|id|rows.get(id).is_some_and(|n|!n.read||seen.contains(id)));
                        if self.notifications.failed_marks.is_empty(){self.notifications.read_error=None;}
                        if let Some(v)=&self.notifications.view{v.status.set_text(self.notifications.read_error.as_deref().unwrap_or(""));}
                    }
                    Err(e)=>{if let Some(v)=&self.notifications.view{v.status.set_text(&e.to_string());}sender.input(MainWindowMsg::ActionError(e));}
                }
                self.load_notice_authors(&sender);self.mark_open_notices(&sender);self.deliver_notices(sender.clone());self.render_inbox(&sender);
                if let Some(id)=self.notifications.pending_open.take(){
                    if self.notifications.inbox.rows.get(&id).is_some_and(|n|n.channel_id.is_some()){sender.input(MainWindowMsg::Notification(NoticeMsg::OpenMessage(id)));}
                    else if let Some(v)=&self.notifications.view{v.status.set_text("Esta mensagem não está mais disponível nas notificações acessíveis.");}
                }
                if self.notifications.refresh_again{self.notifications.refresh_again=false;self.refresh_notifications(None,sender);}
            }
            NoticeMsg::Mark(id)=>{
                if self.notifications.marks.contains(&id){return;}
                let Some(row)=self.notifications.inbox.rows.get(&id)else{return;};if row.read{return;}
                if !row.persisted{self.notifications.inbox.mark(id);self.render_inbox(&sender);return;}
                self.notifications.marks.insert(id);let api=self.api_client.clone();let user=self.current_user.id;
                self.notifications.jobs.push(tokio::spawn(async move{let result=api.read_notifications(user,&[id]).await;sender.input(MainWindowMsg::Notification(NoticeMsg::Marked{id,result}));}));
            }
            NoticeMsg::MarkedBatch{ids,result}=>{
                for id in &ids{self.notifications.marks.remove(id);}
                match result{
                    Ok(updated) if updated==ids.len() as u32=>{for id in ids{self.notifications.inbox.mark(id);self.notifications.failed_marks.remove(&id);self.notifications.seen_events.remove(&id);}if self.notifications.failed_marks.is_empty(){self.notifications.read_error=None;if let Some(v)=&self.notifications.view{v.status.set_text("");}}},
                    Ok(_)=>{self.notifications.failed_marks.extend(ids);self.notifications.read_error=Some("Algumas notificações não estão mais disponíveis. Atualize a lista.".into());if let Some(v)=&self.notifications.view{v.status.set_text(self.notifications.read_error.as_deref().unwrap());}},
                    Err(e)=>{self.notifications.failed_marks.extend(ids);self.notifications.read_error=Some(e.to_string());if let Some(v)=&self.notifications.view{v.status.set_text(&e.to_string());}sender.input(MainWindowMsg::ActionError(e));}
                }
                self.render_inbox(&sender);
            }
            NoticeMsg::Marked{id,result}=>{
                self.notifications.marks.remove(&id);match result{Ok(updated)if updated>0=>{self.notifications.inbox.mark(id);self.notifications.failed_marks.remove(&id);self.notifications.seen_events.remove(&id);if self.notifications.failed_marks.is_empty(){self.notifications.read_error=None;if let Some(v)=&self.notifications.view{v.status.set_text("");}}},Ok(_)=>{if let Some(v)=&self.notifications.view{v.status.set_text("Esta notificação não está mais disponível.");}},Err(e)=>{self.notifications.failed_marks.insert(id);self.notifications.read_error=Some(e.to_string());if let Some(v)=&self.notifications.view{v.status.set_text(&e.to_string());}sender.input(MainWindowMsg::ActionError(e));}}self.render_inbox(&sender);
            }
            NoticeMsg::OpenMessage(id)=>{
                if let Some(window)=root.root().and_downcast::<gtk::Window>(){window.present();}
                let target=self.notifications.inbox.rows.get(&id).and_then(|n|n.channel_id.map(|c|(c,n.message_id)));
                if let Some((channel_id,message_id))=target.filter(|(c,_)|self.can_read_target(*c)){
                    sender.input(MainWindowMsg::Notification(NoticeMsg::Mark(id)));if let Some(v)=&self.notifications.view{v.window.set_visible(false);}sender.input(MainWindowMsg::Navigate{channel_id,message_id});
                }else{
                    if target.is_none(){self.notifications.pending_open=Some(id);}
                    sender.input(MainWindowMsg::Notification(NoticeMsg::Open));if let Some(v)=&self.notifications.view{v.status.set_text("Localizando mensagem… Se o canal estiver indisponível, a notificação permanecerá aqui.");}
                }
            }
            NoticeMsg::DesktopReady(result)=>{self.notifications.desktop.ready(result,sender.input_sender().clone());self.deliver_notices(sender);}
        }
    }
    fn mark_open_notices(&mut self,sender:&ComponentSender<Self>){
        let open=self.notifications.view.as_ref().is_some_and(|v|v.window.is_visible());
        let rows=&self.notifications.inbox.rows;self.notifications.seen_events.retain(|id|rows.contains_key(id));
        if !open&&self.notifications.seen_events.is_empty(){return;}
        let channels=self.notification_channels();
        let ids:Vec<_>=self.notifications.inbox.ordered().into_iter().filter(|n|
            ((open&&!n.read)||(n.persisted&&self.notifications.seen_events.contains(&n.id)))
            &&!self.notifications.marks.contains(&n.id)&&!self.notifications.failed_marks.contains(&n.id)
            &&n.channel_id.is_some_and(|id|channels.iter().any(|c|c.id==id))).map(|n|(n.id,n.persisted)).collect();
        let mut persisted=vec![];
        for (id,persisted_row) in ids{if persisted_row{persisted.push(id);}else{self.notifications.inbox.mark(id);self.notifications.seen_events.insert(id);}}
        for ids in persisted.chunks(1000){
            let ids=ids.to_vec();self.notifications.marks.extend(ids.iter().copied());let api=self.api_client.clone();let user=self.current_user.id;let output=sender.input_sender().clone();
            self.notifications.jobs.push(tokio::spawn(async move{let result=api.read_notifications(user,&ids).await;let _=output.send(MainWindowMsg::Notification(NoticeMsg::MarkedBatch{ids,result}));}));
        }
    }
    pub(super) fn refresh_notifications(&mut self,cursor:Option<MessageCursor>,sender:ComponentSender<Self>){
        self.notifications.jobs.retain(|job|!job.is_finished());
        if self.notifications.request.is_some(){if cursor.is_none(){self.notifications.refresh_again=true;}return;}
        let token=Uuid::new_v4();self.notifications.request=Some(token);let api=self.api_client.clone();let user=self.current_user.id;
        if let Some(v)=&self.notifications.view{v.status.set_text("Carregando…");v.more.set_sensitive(false);}
        self.notifications.jobs.push(tokio::spawn(async move{let result=api.notifications(user,cursor).await;sender.input(MainWindowMsg::Notification(NoticeMsg::Loaded{token,cursor,result}));}));
    }
    pub(super) fn incoming_notice(&mut self,event:NotificationEvent,sender:ComponentSender<Self>){
        self.notifications.inbox.event(event);self.load_notice_authors(&sender);self.mark_open_notices(&sender);self.deliver_notices(sender.clone());self.render_inbox(&sender);self.mark_open_notices(&sender);self.refresh_notifications(None,sender);
    }
    pub(super) fn deliver_notices(&mut self,sender:ComponentSender<Self>){
        if self.account.config.notifications.as_ref().is_some_and(|p|p.enabled==Some(true)){self.notifications.desktop.initialize(sender.input_sender().clone());}
        let channels=self.notification_channels();
        let mut send=Vec::new();
        for n in self.notifications.inbox.rows.values_mut().filter(|n|n.pending_desktop){
            if n.channel_id.is_none()||self.access_request.is_some()&&!channels.iter().any(|c|Some(c.id)==n.channel_id){continue;}
            if let Some((body,sound))=state::delivery(n,&self.account.config,&channels,self.current_user.id,&self.notifications.inbox.messages){
                if !self.notifications.desktop.available(){continue;}
                let users=self.users.iter().map(|u|(u.id,u.clone())).collect();send.push((n.id,crate::ui::chat::mentions::render(&body,&users),sound));
            }n.pending_desktop=false;
        }
        for (id,body,sound)in send{self.notifications.desktop.send(id,body,sound);}
        let allowed=self.notifications.inbox.rows.values().filter(|n|state::delivery(n,&self.account.config,&channels,self.current_user.id,&self.notifications.inbox.messages).is_some()).map(|n|n.id).collect();self.notifications.desktop.revoke(&allowed);
    }
    fn load_notice_authors(&mut self,sender:&ComponentSender<Self>){
        let ids:HashSet<_>=self.notifications.inbox.rows.values().filter(|n|n.channel_id.is_some_and(|id|self.can_read_target(id))).filter_map(|n|n.author_id).collect();
        let missing:Vec<_>=ids.iter().copied().filter(|id|!self.users.iter().any(|u|u.id==*id)&&!self.members.is_pending(*id)).collect();
        for ids in missing.chunks(50){self.fetch_members(ids.to_vec(),sender.clone());}
        self.load_avatars(ids.into_iter().collect(),false,sender.clone());
    }
    pub(super) fn render_inbox(&self,sender:&ComponentSender<Self>){
        let channels=self.notification_channels();
        self.sidebar.emit(SidebarMsg::UnreadNotifications{count:self.notifications.inbox.unread(&channels),more:self.notifications.more});
        self.chat.emit(ChatMsg::NotificationCount{count:self.notifications.inbox.unread(&channels),more:self.notifications.more});
        let Some(v)=&self.notifications.view else{return;};while let Some(c)=v.rows.first_child(){v.rows.remove(&c);}
        for n in self.notifications.inbox.ordered(){
            let channel=n.channel_id.and_then(|id|channels.iter().find(|c|c.id==id));
            let label=if let Some(channel)=channel{let preview=self.account.config.notifications.as_ref().and_then(|p|p.message_preview).unwrap_or(true);let users=self.users.iter().map(|u|(u.id,u.clone())).collect();format!("{}#{} · {}\n{}",if n.read{""}else{"● "},channel.name,n.created_at.format("%d/%m/%Y %H:%M"),if preview{crate::ui::chat::mentions::render(&n.content,&users)}else{"Nova mensagem".into()})}else{"Nova notificação — mensagem não localizada ou canal indisponível".into()};
            let row=gtk::Box::new(gtk::Orientation::Horizontal,8);let open=gtk::Button::new();open.set_widget_name(&format!("notification-{}",n.id));open.set_hexpand(true);
            let content=gtk::Box::new(gtk::Orientation::Horizontal,10);let details=gtk::Box::new(gtk::Orientation::Vertical,4);
            if channel.is_some(){
                let author=n.author_id.and_then(|id|self.users.iter().find(|u|u.id==id)).or_else(||n.author_id.and_then(|id|self.direct.items.iter().map(|d|&d.user).find(|u|u.id==id)));
                let name=author.map(|u|u.display_name()).unwrap_or("Membro indisponível");
                let avatar=crate::media::avatar_image(n.author_id.and_then(|id|self.avatars.textures.get(&id)),36);avatar.set_text(Some(name));avatar.set_valign(gtk::Align::Start);content.append(&avatar);
                let heading=gtk::Label::new(Some(name));heading.set_xalign(0.0);heading.add_css_class("heading");if let Some(roles)=author.and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&heading,roles);}details.append(&heading);
            }
            let l=gtk::Label::new(Some(&label));l.set_wrap(true);l.set_xalign(0.0);details.append(&l);content.append(&details);open.set_child(Some(&content));let input=sender.input_sender().clone();let id=n.id;open.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::OpenMessage(id)));});row.append(&open);
            if (!n.read||self.notifications.seen_events.contains(&id))&&(self.notifications.failed_marks.contains(&id)||channel.is_none()){let mark=gtk::Button::with_label(if n.persisted{"Marcar lida"}else{"Dispensar"});mark.set_sensitive(!self.notifications.marks.contains(&id));let input=sender.input_sender().clone();mark.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::Mark(id)));});row.append(&mark);}v.rows.append(&row);
        }v.more.set_sensitive(self.notifications.more&&self.notifications.request.is_none());
    }
    pub(super) fn history_observed(&mut self,channel:Uuid,messages:Vec<Message>,latest:bool,sender:&ComponentSender<Self>){
        if !self.can_read_target(channel){return;}
        if latest{self.direct_history(channel,&messages,sender);}
        if latest{self.notifications.journal.observed_read(channel,&messages,&mut self.channels);}for m in messages{self.notifications.inbox.observe(m);}self.publish_voice_channels();self.mark_open_notices(sender);self.deliver_notices(sender.clone());self.render_inbox(sender);
    }
}
#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext,backend:&std::sync::Arc<std::sync::Mutex<crate::ui::chat::actions::tests::Backend>>){
    use adw::prelude::*;
    use crate::ui::chat::actions::tests::{find_button,pump,until};
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.view.is_some()&&main.model().notifications.marks.is_empty());let w=main.model().notifications.view.as_ref().unwrap().window.clone();
    let authors=crate::ui::chat::actions::tests::descendants(w.upcast_ref());
    let first=main.model().notifications.inbox.ordered()[0].author_id.unwrap();let name=main.model().users.iter().find(|u|u.id==first).unwrap().display_name().to_owned();
    assert!(authors.iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()==name));assert!(authors.iter().any(|w|w.is::<adw::Avatar>()),"notices show their author's avatar");
    let first_notice=main.model().notifications.inbox.ordered()[0].id;
    assert!(!main.model().notifications.inbox.rows[&first_notice].read,"a failed automatic write keeps the notice unread");assert!(main.model().notifications.view.as_ref().unwrap().status.text().contains("Read failed"));
    find_button(w.upcast_ref(),"Marcar lida").emit_clicked();until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&first_notice].read);
    // Opening a group of unread notices must produce one bulk write, not a
    // burst of per-notice requests. All seeded notices belong to an allowed channel.
    let batch:Vec<_>=(0..5).map(|_|Uuid::new_v4()).collect();let channel=main.model().channels[0].id;
    let baseline=backend.lock().unwrap().requests.len();
    {let mut state=main.state().get_mut();state.model.notifications.inbox.merge(batch.iter().map(|id|serde_json::from_value(serde_json::json!({"id":id,"message_id":Uuid::new_v4(),"channel_id":channel,"author_id":first,"message_content":"bulk preview","read":false,"created_at":"2026-10-01T00:00:00Z"})).unwrap()).collect());}
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.marks.is_empty()&&batch.iter().all(|id|main.model().notifications.inbox.rows[id].read));
    let writes:Vec<_>=backend.lock().unwrap().requests[baseline..].iter().filter(|(method,path,_)|method=="PUT"&&path.ends_with("/read_notification")).cloned().collect();
    assert_eq!(writes.len(),1,"opening five notices sends only one read request");assert_eq!(writes[0].2["notification_ids"].as_array().unwrap().len(),5);
    {let mut state=main.state().get_mut();for id in batch{state.model.notifications.inbox.rows.remove(&id);}}
    find_button(w.upcast_ref(),"Mais notificações").emit_clicked();until(context,||main.model().notifications.request.is_none()&&main.model().notifications.inbox.rows.len()==2&&main.model().notifications.marks.is_empty());
    let id=Uuid::parse_str("12345678-1234-4234-8234-123456789ac0").unwrap();assert!(main.model().notifications.inbox.rows[&id].read,"opening another page also reads its notices automatically");
    assert!(!crate::ui::chat::actions::tests::descendants(w.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Marcar lida")),"successfully read notices need no extra button");
    main.model().chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::CancelNavigation));pump(context);
    let target=main.model().notifications.inbox.rows[&id].message_id;
    let open=crate::ui::chat::actions::tests::descendants(w.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.widget_name()==format!("notification-{id}"))).unwrap();open.emit_clicked();
    until(context,||crate::ui::chat::actions::tests::descendants(main.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("message-{target}")&&w.has_css_class("papo-message-highlight")));
    for _ in 0..80{pump(context);std::thread::sleep(Duration::from_millis(5));}
    let chat=main.model().chat.widget().clone();let widgets=crate::ui::chat::actions::tests::descendants(chat.upcast_ref());let row=widgets.iter().find(|w|w.widget_name()==format!("message-{target}")).unwrap();let list=row.parent().unwrap();let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap();let y=f64::from(row.compute_bounds(&list).unwrap().y());let adj=scroll.vadjustment();assert!(y>=adj.value()&&y<adj.value()+adj.page_size(),"notification activation must show its target message");
    host.set_content(None::<&gtk::Widget>);host.close();
    let inactive=Uuid::parse_str("12345678-1234-4234-8234-123456789ac2").unwrap();let selected=main.model().active_channel_id;
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":inactive,"author_id":Uuid::nil(),"content":"inactive content","created_at":"2026-10-06T00:00:00Z"})).unwrap();
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message)));pump(context);
    assert_eq!(main.model().active_channel_id,selected);assert!(main.model().channels.iter().find(|c|c.id==inactive).unwrap().has_unread());assert!(crate::ui::chat::actions::tests::descendants(main.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("unread-{inactive}")));
    // Ephemeral events resolve from live messages, never via their author ID.
    let event_id=Uuid::new_v4();let msg_id=Uuid::new_v4();let user=main.model().current_user.id;let channel=main.model().channels[0].id;
    let event=NotificationEvent{id:event_id,message_id:msg_id,user_id:Some(user),message_content:"ephemeral preview".into()};main.emit(MainWindowMsg::WsReceived(WsEvent::NewNotification(event.clone())));main.emit(MainWindowMsg::WsReceived(WsEvent::NewNotification(event)));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.inbox.rows.contains_key(&event_id));assert_eq!(main.model().notifications.inbox.rows[&event_id].channel_id,None);
    let message:Message=serde_json::from_value(serde_json::json!({"id":msg_id,"channel_id":channel,"author_id":user,"content":"ephemeral preview","created_at":"2026-10-06T00:00:00Z"})).unwrap();main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message)));pump(context);assert_eq!(main.model().notifications.inbox.rows[&event_id].channel_id,Some(channel));assert!(main.model().channels[0].has_unread());
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.inbox.rows[&event_id].read);assert!(!main.model().notifications.inbox.rows[&event_id].persisted,"viewing a local event must not turn it into a persisted API notice");
    w.set_visible(false);
    main.emit(MainWindowMsg::Notification(NoticeMsg::Refresh));until(context,||main.model().notifications.request.is_none());assert!(main.model().notifications.inbox.rows[&id].read);
    // A seen local event may receive its persisted record after the inbox
    // closes. Its backend acknowledgement must still happen once.
    let baseline=backend.lock().unwrap().requests.len();let token=Uuid::new_v4();main.state().get_mut().model.notifications.request=Some(token);
    main.emit(MainWindowMsg::Notification(NoticeMsg::Loaded{token,cursor:Some(MessageCursor{created_at:chrono::Utc::now(),id:event_id}),result:Ok(serde_json::from_value(serde_json::json!({"notifications":[{"id":event_id,"message_id":msg_id,"channel_id":channel,"author_id":user,"message_content":"ephemeral preview","read":false,"created_at":"2026-10-06T00:00:00Z"}],"has_more":false})).unwrap())}));
    until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&event_id].persisted&&!main.model().notifications.seen_events.contains(&event_id));
    let writes:Vec<_>=backend.lock().unwrap().requests[baseline..].iter().filter(|(method,path,_)|method=="PUT"&&path.ends_with("/read_notification")).cloned().collect();assert_eq!(writes.len(),1);assert_eq!(writes[0].2["notification_ids"],serde_json::json!([event_id]));
    desktop::exercise(main,context,id);

}
#[cfg(test)]
pub(crate) fn exercise_denied(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{descendants,pump};pump(context);let w=main.model().notifications.view.as_ref().unwrap().window.clone();assert!(!descendants(w.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text().contains("notification preview")||l.text().contains("ephemeral preview")));assert_eq!(main.model().notifications.inbox.unread(&main.model().notification_channels()),0);
}
