mod desktop;
mod filters;
#[cfg(test)]pub(crate) use filters::exercise as exercise_filters;
mod read;
pub(crate) mod state;
use super::*;
use crate::models::{Message,NotificationList,NotificationEvent};
use state::{Inbox,ReadJournal};
const NOTICE_WINDOW:usize=1000;
use std::collections::HashSet;
#[derive(Debug)]
pub enum NoticeMsg {
    Open, Hidden, TabChanged, FilterChanged, GroupExpanded(filters::Group,bool), Refresh, More, Latest, Mark(Uuid), OpenMessage(Uuid),
    Loaded{token:Uuid,cursor:Option<MessageCursor>,result:anyhow::Result<NotificationList>},
    Marked{id:Uuid,result:anyhow::Result<u32>},
    MarkedBatch{ids:Vec<Uuid>,result:anyhow::Result<u32>},
    DesktopAction{notice:Uuid,channel:Uuid,message:Uuid,observed:Option<crate::models::MessageObservation>,token:Option<String>},
    DesktopReady(Result<gtk::gio::DBusProxy,gtk::glib::Error>),
}
#[derive(Default)]
pub(super) struct Notifications {
    pub inbox:Inbox,pub journal:ReadJournal,view:Option<InboxView>,
    request:Option<Uuid>,refresh_again:bool,cursor:Option<MessageCursor>,more:bool,older_pages:bool,
    marks:HashSet<Uuid>,failed_marks:HashSet<Uuid>,read_error:Option<String>,seen_events:HashSet<Uuid>,pub(super) jobs:Vec<tokio::task::JoinHandle<()>>,desktop:desktop::Desktop,
    pending_open:Option<Uuid>,
    reads:read::Reads,
    pub(super) manual_reads:std::collections::HashMap<Uuid,Uuid>,
}
struct InboxView {window:gtk::Window,rows:gtk::Box,personal_rows:gtk::Box,tabs:gtk::Stack,status:gtk::Label,scope:gtk::Label,more:gtk::Button,latest:gtk::Button,
    channel_filter:filters::Channels,expanded:std::cell::RefCell<std::collections::HashMap<filters::Group,bool>>,
    groups:std::cell::RefCell<std::collections::HashMap<filters::Group,filters::Rows>>,
}
impl InboxView{fn personal(&self)->bool{self.tabs.visible_child_name().as_deref()==Some("personal")}}
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
                let window=gtk::Window::builder().title("Notificações").default_width(550).default_height(550).build();let input=sender.input_sender().clone();window.connect_hide(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::Hidden));});if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
                let body=gtk::Box::new(gtk::Orientation::Vertical,8);body.set_margin_top(16);body.set_margin_bottom(16);body.set_margin_start(16);body.set_margin_end(16);
                let tabs=gtk::Stack::new();tabs.set_transition_type(gtk::StackTransitionType::None);tabs.set_vexpand(true);
                let rows=gtk::Box::new(gtk::Orientation::Vertical,8);let personal_rows=gtk::Box::new(gtk::Orientation::Vertical,8);
                let server=gtk::Box::new(gtk::Orientation::Vertical,8);let channel_filter=filters::Channels::new(&server,&sender);
                server.append(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&rows).build());
                tabs.add_titled(&server,Some("server"),"Servidor");
                tabs.add_titled(&gtk::ScrolledWindow::builder().child(&personal_rows).build(),Some("personal"),"Mensagens diretas");
                tabs.set_visible_child_name("server");
                let switcher=gtk::StackSwitcher::builder().stack(&tabs).halign(gtk::Align::Center).build();body.append(&switcher);
                let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);let scope=gtk::Label::new(None);scope.set_wrap(true);scope.add_css_class("caption");scope.add_css_class("dim-label");body.append(&scope);body.append(&tabs);
                let input=sender.input_sender().clone();tabs.connect_visible_child_name_notify(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::TabChanged));});
                let more=gtk::Button::with_label("Mais notificações");let input=sender.input_sender().clone();more.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::More));});body.append(&more);
                let latest=gtk::Button::with_label("Voltar às notificações mais recentes");let input=sender.input_sender().clone();latest.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::Latest));});body.append(&latest);window.set_child(Some(&body));window.present();
                self.notifications.view=Some(InboxView{window,rows,personal_rows,tabs,status,scope,more,latest,channel_filter,expanded:Default::default(),groups:Default::default()});self.load_notice_authors(&sender);self.render_inbox(&sender);self.mark_open_notices(&sender);self.refresh_notifications(None,sender);
            }
            NoticeMsg::Hidden=>{if let Some(v)=self.notifications.view.as_ref().filter(|v|!v.window.is_visible()){for group in v.groups.borrow_mut().values_mut(){group.rows.clear();filters::reconcile(&group.body,&[]);}}},
            NoticeMsg::TabChanged|NoticeMsg::FilterChanged=>{self.mark_open_notices(&sender);self.render_inbox(&sender);},
            NoticeMsg::GroupExpanded(group,expanded)=>{
                if let Some(v)=&self.notifications.view{v.expanded.borrow_mut().insert(group,expanded);}
                if expanded{self.mark_open_notices(&sender);}self.render_inbox(&sender);
            },
            NoticeMsg::Refresh=>self.refresh_notifications(None,sender),
            NoticeMsg::Latest=>{if self.notifications.request.is_none(){self.notifications.inbox.reset_pages();self.notifications.cursor=None;self.notifications.older_pages=false;self.refresh_notifications(None,sender);}},
            NoticeMsg::More=>{if self.notifications.more&&self.notifications.request.is_none(){
                if self.notifications.inbox.rows.len()>=NOTICE_WINDOW{self.notifications.inbox.reset_pages();}
                self.notifications.older_pages=true;self.refresh_notifications(self.notifications.cursor,sender);
            }}
            NoticeMsg::Loaded{token,cursor,result}=>{
                if self.notifications.request!=Some(token){return;}self.notifications.request=None;
                match result {
                    Ok(page)=>{
                        let next=page.notifications.last().map(|n|MessageCursor{created_at:n.created_at,id:n.id});
                        let reset_head=cursor.is_none()&&self.notifications.inbox.rows.len()>=NOTICE_WINDOW;
                        if reset_head{self.notifications.older_pages=false;}
                        // A head refresh must not replace an older pagination cursor.
                        if cursor.is_some()||!self.notifications.older_pages{self.notifications.more=page.has_more&&next.is_some()&&next!=cursor;self.notifications.cursor=next;}
                        for n in &page.notifications{if n.read{self.notifications.seen_events.remove(&n.id);}}
                        if cursor.is_none(){if self.notifications.inbox.rows.len()>=NOTICE_WINDOW{self.notifications.inbox.reset_pages();}self.notifications.inbox.merge_head(page.notifications);}else{self.notifications.inbox.merge(page.notifications);}
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
            NoticeMsg::DesktopAction{notice,channel,message,observed,token}=>{
                if !self.can_read_target(channel){return;}
                if let Some(window)=root.root().and_downcast::<gtk::Window>(){if let Some(token)=token{window.set_startup_id(&token);}window.present();}
                if let Some(observed)=observed{if !self.notifications.inbox.messages.contains_key(&message){self.notifications.inbox.observe(observed);}}
                if self.notifications.inbox.rows.contains_key(&notice){sender.input(MainWindowMsg::Notification(NoticeMsg::Mark(notice)));}
                if let Some(v)=&self.notifications.view{v.window.set_visible(false);}
                sender.input(MainWindowMsg::Navigate{channel_id:channel,message_id:message});
            }
            NoticeMsg::DesktopReady(result)=>{self.notifications.desktop.ready(result,sender.input_sender().clone());self.deliver_notices(sender);}
        }
    }
    fn personal_notice(&self,n:&state::Notice)->bool{n.channel_id.is_some_and(|id|self.direct.known.contains(&id)||self.direct.items.iter().any(|d|d.id==id))}
    fn mark_open_notices(&mut self,sender:&ComponentSender<Self>){
        let open=self.notifications.view.as_ref().is_some_and(|v|v.window.is_visible());
        let personal=self.notifications.view.as_ref().is_some_and(InboxView::personal);
        let rows=&self.notifications.inbox.rows;self.notifications.seen_events.retain(|id|rows.contains_key(id));
        if !open&&self.notifications.seen_events.is_empty(){return;}
        let channels=self.notification_channels();
        let ids:Vec<_>=self.notifications.inbox.ordered().into_iter().filter(|n|
            ((open&&!n.read&&self.personal_notice(n)==personal&&self.notifications.view.as_ref().is_some_and(|v|
                v.expanded.borrow().get(&self.notice_group(n,channels.iter().any(|c|Some(c.id)==n.channel_id))).copied().unwrap_or(false)
                &&(personal||v.channel_filter.selected().is_none_or(|id|n.channel_id==Some(id)))
            ))||(n.persisted&&self.notifications.seen_events.contains(&n.id)))
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
        let read=self.notifications.inbox.messages.get(&event.message_id).is_some_and(|m|self.notifications.reads.covers_cursor(m.channel_id,MessageCursor{id:m.id,created_at:m.created_at}));let id=event.id;
        self.notifications.inbox.event(event);if read{self.notifications.inbox.mark(id);}self.load_notice_authors(&sender);self.mark_open_notices(&sender);self.deliver_notices(sender.clone());self.render_inbox(&sender);self.mark_open_notices(&sender);self.refresh_notifications(None,sender);
    }
    pub(super) fn deliver_notices(&mut self,sender:ComponentSender<Self>){
        if self.account.config.notifications.as_ref().is_some_and(|p|p.enabled==Some(true)){self.notifications.desktop.initialize(sender.input_sender().clone());}
        let channels=self.notification_channels();
        let mut send=Vec::new();
        let users=self.users.iter().map(|u|(u.id,u.clone())).collect();
        for n in self.notifications.inbox.rows.values_mut().filter(|n|n.pending_desktop){
            if n.channel_id.is_none()||self.access_request.is_some()&&!channels.iter().any(|c|Some(c.id)==n.channel_id){continue;}
            if let Some((body,sound))=state::delivery(n,&self.account.config,&channels,self.current_user.id,&self.notifications.inbox.messages){
                if !self.notifications.desktop.available(){continue;}
                send.push((n.id,crate::ui::chat::mentions::render(&body,&users),sound));
            }n.pending_desktop=false;
        }
        let allowed=self.notifications.inbox.rows.values().filter(|n|state::delivery(n,&self.account.config,&channels,self.current_user.id,&self.notifications.inbox.messages).is_some()).map(|n|n.id).collect();let readable=channels.iter().map(|c|c.id).collect();self.notifications.desktop.revoke(&allowed,&readable);
        for (id,body,sound)in send{if let Some(n)=self.notifications.inbox.rows.get(&id){let observed=self.notifications.inbox.messages.get(&n.message_id).cloned();self.notifications.desktop.send(n,observed,body,sound);}}
    }
    fn load_notice_authors(&mut self,sender:&ComponentSender<Self>){
        if !self.notifications.view.as_ref().is_some_and(|v|v.window.is_visible()){return;}
        let ids:HashSet<_>=self.notifications.inbox.rows.values().filter(|n|n.channel_id.is_some_and(|id|self.can_read_target(id))).filter_map(|n|n.author_id).collect();
        let missing:Vec<_>=ids.iter().copied().filter(|id|!self.users.iter().any(|u|u.id==*id)&&!self.members.is_pending(*id)).collect();
        for ids in missing.chunks(50){self.fetch_members(ids.to_vec(),sender.clone());}

    }
    pub(super) fn notice_external<'a>(&'a self,n:&'a state::Notice)->Option<&'a crate::models::ExternalAuthor>{
        if n.author_id.is_some(){return None;}
        n.external().or_else(||self.notifications.inbox.messages.get(&n.message_id).and_then(|m|m.external()))
    }
    pub(super) fn render_inbox(&self,sender:&ComponentSender<Self>){
        let _timing=crate::performance::timer("inbox_update_us");
        let channels=self.notification_channels();
        let unread=self.notifications.inbox.unread(&channels);
        self.sidebar.emit(SidebarMsg::UnreadNotifications{count:unread,more:self.notifications.more});
        let mut counts=HashMap::new();for n in self.notifications.inbox.rows.values().filter(|n|!n.read){if let Some(id)=n.channel_id.filter(|id|self.channels.iter().any(|c|c.id==*id)&&self.can_read_target(*id)){*counts.entry(id).or_insert(0)+=1;}}
        self.sidebar.emit(SidebarMsg::ChannelCounts(counts));
        self.chat.emit(ChatMsg::NotificationCount{count:unread,more:self.notifications.more});
        let Some(v)=self.notifications.view.as_ref().filter(|v|v.window.is_visible())else{return;};
        let channel_index:HashMap<_,_>=channels.iter().map(|c|(c.id,c)).collect();
        let unread_direct=self.notifications.inbox.rows.values().filter(|n|!n.read&&self.personal_notice(n)&&n.channel_id.is_some_and(|id|channel_index.contains_key(&id))).count();
        if let Some(personal)=v.tabs.child_by_name("personal"){
            let title=if unread_direct>0{format!("Mensagens diretas ({unread_direct})")}else{"Mensagens diretas".into()};
            let page=v.tabs.page(&personal);page.set_title(&title);page.set_needs_attention(unread_direct>0&&!v.personal());
        }
        v.channel_filter.widget.set_tooltip_text(Some("Filtra as páginas carregadas. Use Mais notificações para buscar páginas anteriores."));
        v.channel_filter.update(&self.channels.iter().filter(|c|self.access.get(&c.id).is_some_and(|a|a.read)).map(|c|(c.id,c.name.clone())).collect::<Vec<_>>());
        let selected=v.channel_filter.selected();
        let mut buckets:Vec<(filters::Group,Vec<&state::Notice>)>=Vec::new();let mut indices=HashMap::new();
        for n in self.notifications.inbox.ordered(){let key=self.notice_group(n,n.channel_id.is_some_and(|id|channel_index.contains_key(&id)));let index=*indices.entry(key).or_insert_with(||{buckets.push((key,vec![]));buckets.len()-1});buckets[index].1.push(n);}
        let users=self.users.iter().map(|u|(u.id,u.clone())).collect();
        let preview=self.account.config.notifications.as_ref().and_then(|p|p.message_preview).unwrap_or(true);
        let mut groups=v.groups.borrow_mut();groups.retain(|key,_|indices.contains_key(key));v.expanded.borrow_mut().retain(|key,_|indices.contains_key(key));
        let mut server_order=vec![];let mut personal_order=vec![];
        for (key,notices) in buckets{
            let first=notices[0];let personal=self.personal_notice(first);let channel=first.channel_id.and_then(|id|channel_index.get(&id).copied());
            let author=if personal{channel.and_then(|_|self.notice_author(first).or_else(||self.direct.items.iter().find(|d|Some(d.id)==first.channel_id).map(|d|&d.user)))}else{None};
            let name=if personal{author.map(|u|u.display_name()).unwrap_or(if channel.is_some(){"Remetente indisponível"}else{"Conversa indisponível"})}else{channel.map(|c|c.name.as_str()).unwrap_or("Canal indisponível")};
            let texture=author.and_then(|a|self.avatars.textures.get(&a.id));
            let parent=if personal{&v.personal_rows}else{&v.rows};
            let group=groups.entry(key).or_insert_with(||filters::Rows::new(v,parent,key,name,texture,author,sender));
            group.update(name,texture,author,notices.iter().filter(|n|!n.read).count());
            if !personal&&selected.is_some_and(|id|first.channel_id!=Some(id)){group.rows.clear();filters::reconcile(&group.body,&[]);continue;}
            if personal{personal_order.push(group.expander.clone().upcast());}else{server_order.push(group.expander.clone().upcast());}
            if !group.expander.is_expanded()||v.personal()!=personal{group.rows.clear();filters::reconcile(&group.body,&[]);continue;}
            let mut order=vec![];let mut present=HashSet::new();
            for n in notices{
                present.insert(n.id);let channel=n.channel_id.and_then(|id|channel_index.get(&id).copied());
                let author=channel.and_then(|_|self.notice_author(n));
                let translated=if preview{crate::ui::chat::mentions::render(&n.content,&users)}else{String::new()};
                let external=channel.and_then(|_|self.notice_external(n));
                let external_name=external.map(|a|a.name.as_str());let external_texture=external.and_then(|a|self.chat.model().external_avatar(a));
                let signature=format!("{:?}|{n:?}|{preview}|{translated}|{external_name:?}|{:?}|{:?}|{:?}|{}|{}|{}",external_texture.as_ref().map(|t|t.as_ptr()),channel.map(|c|&c.name),author.map(|a|(a.display_name(),&a.roles)),n.author_id.and_then(|id|self.avatars.textures.get(&id)).map(|t|t.as_ptr()),self.notifications.failed_marks.contains(&n.id),self.notifications.marks.contains(&n.id),self.notifications.seen_events.contains(&n.id));
                if let Some((_,row))=group.rows.get(&n.id).filter(|(old,_)|*old==signature){order.push(row.clone().upcast());continue;}
                crate::performance::record("notification_rows_built",1);
            let label=if let Some(channel)=channel{format!("{}{}{} · {}\n{}",if n.read{""}else{"● "},if personal{""}else{"#"},channel.name,n.created_at.format("%d/%m/%Y %H:%M"),if preview{translated}else{"Nova mensagem".into()})}else{"Nova notificação — mensagem não localizada ou canal indisponível".into()};
            let row=gtk::Box::new(gtk::Orientation::Horizontal,8);let open=gtk::Button::new();open.set_widget_name(&format!("notification-{}",n.id));open.set_hexpand(true);
            let content=gtk::Box::new(gtk::Orientation::Horizontal,10);let details=gtk::Box::new(gtk::Orientation::Vertical,4);
            if channel.is_some(){
                let author=self.notice_author(n);
                let name=author.map(|u|u.display_name()).or_else(||self.notice_external(n).map(|a|a.name.as_str())).unwrap_or("Membro indisponível");
                let avatar=crate::media::avatar_image(n.author_id.and_then(|id|self.avatars.textures.get(&id)).or(external_texture.as_ref()),36);avatar.set_text(Some(name));avatar.set_valign(gtk::Align::Start);content.append(&avatar);
                let heading=gtk::Label::new(Some(name));heading.set_xalign(0.0);heading.add_css_class("heading");if let Some(roles)=author.and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&heading,roles);}let identity=gtk::Box::new(gtk::Orientation::Horizontal,6);identity.append(&heading);if external.is_some(){let badge=gtk::Label::new(Some("APP"));badge.add_css_class("accent");badge.add_css_class("caption");identity.append(&badge);}details.append(&identity);
            }
            let l=gtk::Label::new(Some(&label));l.set_wrap(true);l.set_xalign(0.0);details.append(&l);content.append(&details);open.set_child(Some(&content));let input=sender.input_sender().clone();let id=n.id;open.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::OpenMessage(id)));});row.append(&open);
            if (!n.read||self.notifications.seen_events.contains(&id))&&(self.notifications.failed_marks.contains(&id)||channel.is_none()){let mark=gtk::Button::with_label(if n.persisted{"Marcar lida"}else{"Dispensar"});mark.set_sensitive(!self.notifications.marks.contains(&id));let input=sender.input_sender().clone();mark.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::Mark(id)));});row.append(&mark);}
                order.push(row.clone().upcast());group.rows.insert(n.id,(signature,row));
            }
            group.rows.retain(|id,_|present.contains(id));filters::reconcile(&group.body,&order);
        }
        for (parent,mut order,text) in [(&v.rows,server_order,if selected.is_some(){"Nenhuma notificação carregada neste canal. Pode haver outras nas páginas anteriores."}else{"Nenhuma notificação do servidor nas páginas carregadas."}),(&v.personal_rows,personal_order,"Nenhuma mensagem direta nas páginas carregadas.")]{
            if order.is_empty(){let empty=parent.first_child().and_downcast::<gtk::Label>().unwrap_or_else(||gtk::Label::new(None));empty.set_text(text);empty.set_wrap(true);empty.set_margin_top(24);empty.set_margin_bottom(24);empty.add_css_class("dim-label");order.push(empty.upcast());}filters::reconcile(parent,&order);
        }
        let displayed:HashSet<_>=groups.values().flat_map(|g|g.rows.keys().copied()).collect();
        let avatars=self.notifications.inbox.rows.values().filter(|n|displayed.contains(&n.id)).filter_map(|n|n.author_id).collect();
        sender.input(MainWindowMsg::VisibleAvatars(avatars));
        let external=self.notifications.inbox.rows.values().filter(|n|displayed.contains(&n.id)&&n.channel_id.is_some_and(|id|self.can_read_target(id))).filter_map(|n|self.notice_external(n).cloned()).take(128).collect::<Vec<_>>();
        if !external.is_empty(){self.chat.emit(ChatMsg::ExternalAvatars(external));}
        v.scope.set_text(if self.notifications.more{"Os contadores e filtros incluem só as notificações carregadas. Há mais páginas anteriores."}else{"Contadores e filtros das notificações carregadas."});
        v.more.set_sensitive(self.notifications.more&&self.notifications.request.is_none());
        v.more.set_label(if self.notifications.inbox.rows.len()>=NOTICE_WINDOW{"Carregar anteriores e liberar páginas atuais"}else{"Mais notificações"});
        v.latest.set_visible(self.notifications.older_pages||self.notifications.inbox.rows.len()>=NOTICE_WINDOW);v.latest.set_sensitive(self.notifications.request.is_none());
        if self.notifications.inbox.rows.len()>=NOTICE_WINDOW{v.status.set_text("Para continuar, as páginas atuais serão liberadas da memória. Você pode voltar às mais recentes.");}
        v.more.set_tooltip_text(Some(if self.notifications.more{"Os contadores incluem somente notificações carregadas; há mais páginas."}else{"Não há mais páginas anteriores nesta consulta."}));
    }
    pub(super) fn history_observed(&mut self,channel:Uuid,messages:Vec<crate::models::MessageObservation>,latest:bool,sender:&ComponentSender<Self>){
        if !self.can_read_target(channel){return;}
        for m in messages{
            let m=m.into_message();
            self.notifications.journal.message(&m,&mut self.channels);
            if latest{self.direct_message(&m);}
            self.notifications.inbox.observe(m);
        }
        self.publish_voice_channels();self.mark_open_notices(sender);self.deliver_notices(sender.clone());self.render_inbox(sender);
    }
}
#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext,backend:&std::sync::Arc<std::sync::Mutex<crate::ui::chat::actions::tests::Backend>>){
    use adw::prelude::*;
    use crate::ui::chat::actions::tests::{find_button,pump,until};
    // Earlier workflows share this controller and may already have read these
    // fixture notices. Seed the failed-write case independently of that state.
    {let mut b=backend.lock().unwrap();b.fail_read=true;b.read_notices.clear();}
    main.state().get_mut().model.notifications=Notifications::default();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.view.is_some()&&main.model().notifications.marks.is_empty());let mut w=main.model().notifications.view.as_ref().unwrap().window.clone();
    let expand=|window:&gtk::Window,channel:Uuid,expanded|{
        crate::ui::chat::actions::tests::descendants(window.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Expander>().ok().filter(|w|w.widget_name()==format!("notification-channel-{channel}"))).unwrap().set_expanded(expanded);
    };
    let first_notice=main.model().notifications.inbox.ordered()[0].id;
    let first_channel=main.model().notifications.inbox.rows[&first_notice].channel_id.unwrap();
    assert!(!main.model().notifications.inbox.rows[&first_notice].read);expand(&w,first_channel,true);
    until(context,||main.model().notifications.failed_marks.contains(&first_notice));
    let authors=crate::ui::chat::actions::tests::descendants(w.upcast_ref());
    let first=main.model().notifications.inbox.ordered()[0].author_id.unwrap();let name=main.model().users.iter().find(|u|u.id==first).unwrap().display_name().to_owned();
    assert!(authors.iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()==name));assert!(authors.iter().any(|w|w.is::<adw::Avatar>()),"notices show their author's avatar");
    assert!(!main.model().notifications.inbox.rows[&first_notice].read,"a failed automatic write keeps the notice unread");assert!(main.model().notifications.view.as_ref().unwrap().status.text().contains("Read failed"));
    find_button(w.upcast_ref(),"Marcar lida").emit_clicked();until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&first_notice].read);
    // Opening a group of unread notices must produce one bulk write, not a
    // burst of per-notice requests. All seeded notices belong to an allowed channel.
    let batch:Vec<_>=(0..5).map(|_|Uuid::new_v4()).collect();let channel=main.model().channels[0].id;
    expand(&w,first_channel,false);pump(context);
    let baseline=backend.lock().unwrap().requests.len();
    {let mut state=main.state().get_mut();state.model.notifications.inbox.merge(batch.iter().map(|id|serde_json::from_value(serde_json::json!({"id":id,"message_id":Uuid::new_v4(),"channel_id":channel,"author_id":first,"message_content":"bulk preview","read":false,"created_at":"2026-10-01T00:00:00Z"})).unwrap()).collect());}
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none());
    assert!(batch.iter().all(|id|!main.model().notifications.inbox.rows[id].read));expand(&w,channel,true);
    until(context,||main.model().notifications.marks.is_empty()&&batch.iter().all(|id|main.model().notifications.inbox.rows[id].read));
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
    let created_at=main.model().channels.iter().find(|c|c.id==channel).and_then(|c|c.last_message.as_ref()).map_or(chrono::Utc::now(),|m|m.created_at+chrono::Duration::seconds(1));
    let message:Message=serde_json::from_value(serde_json::json!({"id":msg_id,"channel_id":channel,"author_id":user,"content":"ephemeral preview","created_at":created_at})).unwrap();main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message)));pump(context);assert_eq!(main.model().notifications.inbox.rows[&event_id].channel_id,Some(channel));assert!(main.model().channels[0].has_unread());
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.view.as_ref().is_some_and(|v|v.window.is_visible()));
    w=main.model().notifications.view.as_ref().unwrap().window.clone();expand(&w,channel,true);until(context,||main.model().notifications.inbox.rows[&event_id].read);assert!(!main.model().notifications.inbox.rows[&event_id].persisted,"viewing a local event must not turn it into a persisted API notice");
    assert!(!crate::ui::chat::actions::tests::descendants(w.upcast_ref()).iter().any(|w|w.widget_name()=="notification-channel-unread"&&w.is_visible()),"reading a local event must immediately clear the channel badge");
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

#[cfg(test)]pub(crate) fn exercise_tabs(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let dm:Uuid="12345678-1234-4234-8234-123456789ac3".parse().unwrap();let peer:Uuid="12345678-1234-4234-8234-123456789ac0".parse().unwrap();
    backend.lock().unwrap().dms=vec![serde_json::json!({"id":dm,"user":{"id":peer,"username":"bob","created_at":"2026-10-03T00:00:00Z"},"created_at":"2026-10-03T00:00:00Z","unread_count":2})];
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().direct.items.len()==1&&main.model().access_request.is_none()&&!main.model().channels.is_empty());
    let badge=descendants(main.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|w|w.widget_name()=="direct-unread-indicator")).unwrap();
    until(context,||badge.is_visible()&&badge.text()=="2");crate::ui::main_window::layout::tests::preview(&host,"dm-unread-badge",context);
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":dm,"author_id":peer,"content":"DM incoming","created_at":"2026-10-10T00:00:00Z"})).unwrap();
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message.clone())));until(context,||badge.text()=="3");
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message.clone())));pump(context);assert_eq!(badge.text(),"3","duplicate messages cannot inflate the badge");
    main.emit(MainWindowMsg::HistoryObserved{channel_id:dm,messages:vec![crate::models::MessageObservation::from(&message)],latest:true});pump(context);assert_eq!(badge.text(),"3","fetching messages is not a read acknowledgement");
    let request=main.state().get_mut().model.notifications.reads.begin(&message).unwrap();
    main.emit(MainWindowMsg::ChannelRead{request,message,result:Ok(())});until(context,||!badge.is_visible());
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.marks.is_empty()&&main.model().notifications.view.is_some());
    let server_notice=Uuid::new_v4();let direct_notice=Uuid::new_v4();let channel=main.model().channels[0].id;
    let rows=[(server_notice,channel),(direct_notice,dm)].into_iter().map(|(id,channel)|serde_json::from_value(serde_json::json!({"id":id,"message_id":Uuid::new_v4(),"channel_id":channel,"author_id":peer,"message_content":"tab preview","read":false,"created_at":"2026-10-01T00:00:00Z"})).unwrap()).collect();
    // This case checks tab/group acknowledgement; retry failures have their own fixture.
    backend.lock().unwrap().fail_read=false;
    {let mut state=main.state().get_mut();state.model.notifications.inbox=Inbox::default();state.model.notifications.inbox.merge(rows);}
    let baseline=backend.lock().unwrap().requests.len();main.emit(MainWindowMsg::Notification(NoticeMsg::TabChanged));
    pump(context);assert!(!main.model().notifications.inbox.rows[&server_notice].read,"server channels start collapsed and unread");
    let server_group={let model=main.model();crate::ui::chat::actions::tests::descendants(model.notifications.view.as_ref().unwrap().rows.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Expander>().ok().filter(|w|w.widget_name()==format!("notification-channel-{channel}"))).unwrap()};
    assert!(!server_group.is_expanded());server_group.set_expanded(true);
    until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&server_notice].read);
    assert!(!main.model().notifications.inbox.rows[&direct_notice].read,"opening the server tab must not read personal notices");
    let(tabs,server_rows,personal_rows,window)={let model=main.model();let v=model.notifications.view.as_ref().unwrap();(v.tabs.clone(),v.rows.clone(),v.personal_rows.clone(),v.window.clone())};
    let contains=|rows:&gtk::Box,id:Uuid|{
        let mut widgets=descendants(rows.upcast_ref());
        // GTK keeps a collapsed expander's child outside its visible hierarchy.
        let nested:Vec<_>=widgets.iter().filter_map(|w|w.downcast_ref::<gtk::Expander>().and_then(|e|e.child())).collect();
        for child in nested{widgets.extend(descendants(&child));}
        widgets.iter().any(|w|w.widget_name()==format!("notification-{id}"))
    };
    assert!(contains(&server_rows,server_notice)&&!contains(&server_rows,direct_notice));assert!(!contains(&personal_rows,direct_notice)&&!contains(&personal_rows,server_notice),"inactive collapsed groups stay lazy");
    crate::ui::main_window::layout::tests::preview(&window,"notification-server-tab",context);
    tabs.set_visible_child_name("personal");pump(context);
    assert!(!main.model().notifications.inbox.rows[&direct_notice].read,"opening the DM tab must keep collapsed sender messages unread");
    let group=descendants(personal_rows.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Expander>().ok()).unwrap();assert!(!group.is_expanded());
    group.set_expanded(true);until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&direct_notice].read);
    crate::ui::main_window::layout::tests::preview(&window,"notification-dm-tab",context);
    let requests=backend.lock().unwrap().requests[baseline..].to_vec();assert!(!requests.iter().any(|(m,p,_)|m=="GET"&&p.contains("/notifications")),"changing tabs must not refetch notifications");
    let writes:Vec<_>=requests.iter().filter(|(m,p,_)|m=="PUT"&&p.ends_with("/read_notification")).collect();assert_eq!(writes.len(),2);assert_eq!(writes[0].2["notification_ids"],serde_json::json!([server_notice]));assert_eq!(writes[1].2["notification_ids"],serde_json::json!([direct_notice]));
    main.emit(MainWindowMsg::Direct(super::direct::DirectMsg::Hide(dm)));until(context,||main.model().direct.items.is_empty());main.emit(MainWindowMsg::Notification(NoticeMsg::TabChanged));pump(context);assert!(!contains(&server_rows,direct_notice),"hidden DMs must not migrate into the server tab");
    window.close();host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}

#[cfg(test)]pub(crate) fn exercise_backend_update(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use crate::ui::chat::actions::tests::{mock,pump,until};
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    until(context,||main.model().background_started&&main.model().access_request.is_none()&&!main.model().channels.is_empty()&&main.model().notifications.request.is_none());
    let channel=main.model().channels[0].id;
    assert!(main.model().channels[0].has_unread(),"message fetching leaves the channel unread");
    let read_count=||backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="PUT"&&path==&format!("/channels/{channel}/read")).count();
    assert_eq!(read_count(),0,"an unmapped chat is not acknowledged");
    let host=adw::Window::builder().title("Papo backend update test").default_width(1100).default_height(740).content(main.widget()).build();host.present();
    crate::ui::chat::performance::settle(context);
    if gtk::prelude::WidgetExt::display(&host).type_().name()=="GdkX11Display"{
        // Bare virtual displays have no window manager to grant keyboard focus.
        assert!(std::process::Command::new("python3").arg(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/helpers/input_keys.py")).arg("Papo backend update test").status().unwrap().success());
        until(context,||host.is_active());
    }
    assert!(host.is_active(),"the private test host must have desktop focus");
    until(context,||main.model().chat.model().read_candidate().is_some());
    main.emit(MainWindowMsg::TickCleanup);until(context,||!main.model().channels[0].has_unread());assert_eq!(read_count(),1);
    let baseline=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p.contains("/messages?")).count();
    for _ in 0..4{main.emit(MainWindowMsg::TickCleanup);}pump(context);assert_eq!(read_count(),1,"idle ticks must not repeat the read write");
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p.contains("/messages?")).count(),baseline,"read acknowledgements do not refetch messages");
    // Keep a newer live message unread when its write fails, and back off.
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":main.model().current_user.id,"content":"Live unread","created_at":"2026-10-12T12:00:00Z"})).unwrap();
    backend.lock().unwrap().fail_channel_read=true;
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message.clone())));
    until(context,||main.model().chat.model().read_candidate().is_some_and(|m|m.id==message.id));
    main.emit(MainWindowMsg::TickCleanup);until(context,||read_count()==2&&main.model().notifications.reads.idle());
    assert!(main.model().channels[0].has_unread());for _ in 0..4{main.emit(MainWindowMsg::TickCleanup);}pump(context);assert_eq!(read_count(),2,"failed read writes respect the cooldown");
    main.state().get_mut().model.notifications.reads.retry_now(channel);
    main.emit(MainWindowMsg::TickCleanup);until(context,||!main.model().channels[0].has_unread());assert_eq!(read_count(),3);
    let notice=Uuid::new_v4();main.emit(MainWindowMsg::WsReceived(WsEvent::NewNotification(NotificationEvent{id:notice,message_id:message.id,user_id:message.author_id,message_content:"Already read".into()})));pump(context);
    assert!(main.model().notifications.inbox.rows[&notice].read,"late events cannot restore an acknowledged message's unread status");
    let recovery_api=main.model().api_client.clone();crate::ui::login::recovery::exercise(&recovery_api,context);
    super::moderation::exercise(&main,context,&backend);
    // manage_server still permits moderation/audit, but cannot reset another account.
    {let mut state=main.state().get_mut();state.model.server.as_mut().unwrap().owner_id=Some(Uuid::new_v4());state.model.sync_moderation_access();}
    super::moderation::exercise_recovery_denied(&main,context,&backend);
    let dm:Uuid="12345678-1234-4234-8234-123456789ac3".parse().unwrap();let peer:Uuid="12345678-1234-4234-8234-123456789ac0".parse().unwrap();
    let dm_message=Uuid::new_v4();
    {let mut b=backend.lock().unwrap();
        b.dm_messages=vec![serde_json::json!({"id":dm_message,"channel_id":dm,"author_id":peer,"content":"Unread DM","created_at":"2026-10-12T12:00:00Z"})];
        b.dms=vec![serde_json::json!({"id":dm,"user":{"id":peer,"username":"bob","created_at":"2026-10-03T00:00:00Z"},"created_at":"2026-10-03T00:00:00Z","unread_count":2,"last_message":{"id":dm_message,"created_at":"2026-10-12T12:00:00Z"}})];
    }
    main.emit(MainWindowMsg::Direct(super::direct::DirectMsg::Open(peer)));
    until(context,||main.model().active_channel_id==Some(dm)&&main.model().chat.model().read_candidate().is_some());
    if gtk::prelude::WidgetExt::display(&host).type_().name()=="GdkX11Display"{
        assert!(std::process::Command::new("python3").arg(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/helpers/input_keys.py")).arg("Papo backend update test").status().unwrap().success());until(context,||host.is_active());
    }
    main.emit(MainWindowMsg::TickCleanup);until(context,||main.model().direct.items.iter().any(|d|d.id==dm&&d.unread_count==0));
    let writes:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p==&format!("/channels/{dm}/read")).cloned().collect();
    assert_eq!(writes.len(),1);assert_eq!(writes[0].2["message_id"],dm_message.to_string(),"DMs use the same explicit message cursor acknowledgement");
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}

#[cfg(test)]pub(crate) fn exercise_page_window(context:&gtk::glib::MainContext){
    use adw::prelude::AdwWindowExt;
    use crate::ui::chat::actions::tests::{mock,pump,until,find_button};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();let host=adw::Window::builder().default_width(1000).default_height(720).content(main.widget()).build();host.present();
    until(context,||main.model().background_started&&main.model().notifications.request.is_none());main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.view.as_ref().is_some_and(|v|v.window.is_visible()));
    let channel=main.model().channels[0].id;
    {let mut state=main.state().get_mut();let model=&mut state.model;model.notifications.inbox=Default::default();model.notifications.more=true;model.notifications.inbox.merge((0..NOTICE_WINDOW).map(|_|crate::models::NotificationSummary{webhook_id:None,external_author:None,id:Uuid::new_v4(),message_id:Uuid::new_v4(),channel_id:channel,author_id:None,message_content:"fixture".into(),read:false,created_at:chrono::Utc::now()}).collect());}
    main.emit(MainWindowMsg::Notification(NoticeMsg::TabChanged));pump(context);
    let w=main.model().notifications.view.as_ref().unwrap().window.clone();assert!(find_button(w.upcast_ref(),"Carregar anteriores e liberar páginas atuais").is_sensitive());assert!(find_button(w.upcast_ref(),"Voltar às notificações mais recentes").is_visible());
    assert!(main.model().notifications.view.as_ref().unwrap().groups.borrow().values().all(|g|g.rows.is_empty()),"one thousand collapsed notices do not create message widgets");
    let cursor=crate::models::MessageCursor{id:Uuid::new_v4(),created_at:chrono::Utc::now()};main.state().get_mut().model.notifications.cursor=Some(cursor);
    find_button(w.upcast_ref(),"Carregar anteriores e liberar páginas atuais").emit_clicked();until(context,||main.model().notifications.request.is_none()&&main.model().notifications.inbox.rows.len()<NOTICE_WINDOW);
    assert!(main.model().notifications.older_pages);assert!(find_button(w.upcast_ref(),"Voltar às notificações mais recentes").is_visible());
    assert!(backend.lock().unwrap().requests.iter().any(|(_,p,_)|p.contains(&format!("last_id={}",cursor.id))),"paging continues past the window instead of truncating older history");
    find_button(w.upcast_ref(),"Voltar às notificações mais recentes").emit_clicked();until(context,||main.model().notifications.request.is_none()&&!main.model().notifications.older_pages);
    assert!(!find_button(w.upcast_ref(),"Voltar às notificações mais recentes").is_visible());w.close();host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
