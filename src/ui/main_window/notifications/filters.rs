//! Local notification filters and channel/sender groups; IDs keep duplicate names distinct.
use super::*;
use std::{cell::{Cell,RefCell},rc::Rc};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub(crate) enum Group {Channel(Uuid),Sender(Uuid),Conversation(Uuid),Unknown}
impl Group {
    fn name(self)->String{match self{Self::Channel(id)=>format!("notification-channel-{id}"),Self::Sender(id)=>format!("notification-sender-{id}"),Self::Conversation(id)=>format!("notification-conversation-{id}"),Self::Unknown=>"notification-unknown-sender".into()}}
}
pub(super) struct Channels {
    pub(super) widget:gtk::DropDown,
    ids:RefCell<Vec<Option<Uuid>>>,
    updating:Rc<Cell<bool>>,
}
impl Channels {
    pub fn new(body:&gtk::Box,sender:&ComponentSender<MainWindowModel>)->Self{
        let line=gtk::Box::new(gtk::Orientation::Horizontal,8);let label=gtk::Label::new(Some("Canal"));line.append(&label);
        let widget=gtk::DropDown::from_strings(&["Todos os canais"]);widget.set_hexpand(true);widget.set_widget_name("notification-channel-filter");
        widget.update_property(&[gtk::accessible::Property::Label("Filtrar notificações por canal")]);line.append(&widget);body.append(&line);
        let updating=Rc::new(Cell::new(false));let active=updating.clone();let input=sender.input_sender().clone();
        widget.connect_selected_notify(move |_|{if !active.get(){let _=input.send(MainWindowMsg::Notification(NoticeMsg::FilterChanged));}});
        Self{widget,ids:RefCell::new(vec![None]),updating}
    }
    pub fn selected(&self)->Option<Uuid>{self.ids.borrow().get(self.widget.selected() as usize).copied().flatten()}
    pub fn update(&self,channels:&[(Uuid,String)]){
        let selected=self.selected();let mut choices=channels.to_vec();choices.sort_by_cached_key(|(id,name)|(name.to_lowercase(),*id));
        let ids:Vec<_>=std::iter::once(None).chain(choices.iter().map(|(id,_)|Some(*id))).collect();
        let names:Vec<_>=std::iter::once("Todos os canais".to_string()).chain(choices.iter().map(|(_,name)|format!("# {name}"))).collect();
        let same_names=self.widget.model().and_downcast::<gtk::StringList>().is_some_and(|model|model.n_items()==names.len() as u32&&names.iter().enumerate().all(|(i,name)|model.string(i as u32).as_deref()==Some(name.as_str())));
        if *self.ids.borrow()==ids&&same_names{return;}
        self.updating.set(true);let labels:Vec<_>=names.iter().map(String::as_str).collect();self.widget.set_model(Some(&gtk::StringList::new(&labels)));
        self.widget.set_selected(ids.iter().position(|id|*id==selected).unwrap_or(0) as u32);*self.ids.borrow_mut()=ids;self.updating.set(false);
    }
}

pub(super) struct Rows {
    pub expander:gtk::Expander,pub body:gtk::Box,pub rows:std::collections::HashMap<Uuid,(String,gtk::Box)>,
    badge:gtk::Label,title:gtk::Label,avatar:Option<adw::Avatar>,
}
impl Rows {
    pub fn new(view:&InboxView,parent:&gtk::Box,key:Group,name:&str,texture:Option<&gtk::gdk::Texture>,author:Option<&crate::models::UserSummary>,sender:&ComponentSender<MainWindowModel>)->Self{
        let expander=gtk::Expander::new(None);expander.set_widget_name(&key.name());expander.add_css_class("card");
        let header=gtk::Box::new(gtk::Orientation::Horizontal,8);header.set_margin_top(6);header.set_margin_bottom(6);header.set_margin_end(6);
        let server=matches!(key,Group::Channel(_)|Group::Unknown);
        let avatar=if server{let icon=gtk::Label::new(Some("#"));icon.set_size_request(32,32);icon.add_css_class("heading");icon.add_css_class("dim-label");header.append(&icon);None}
            else{let avatar=crate::media::avatar_image(texture,32);avatar.set_text(Some(name));header.append(&avatar);Some(avatar)};
        let title=gtk::Label::new(None);title.set_xalign(0.0);title.set_hexpand(true);title.set_ellipsize(gtk::pango::EllipsizeMode::End);title.add_css_class("heading");header.append(&title);
        let badge=gtk::Label::new(None);badge.set_widget_name(if server{"notification-channel-unread"}else{"notification-sender-unread"});badge.add_css_class("papo-notification-count");badge.set_valign(gtk::Align::Center);header.append(&badge);expander.set_label_widget(Some(&header));
        let body=gtk::Box::new(gtk::Orientation::Vertical,8);body.set_margin_start(12);body.set_margin_top(8);body.set_margin_bottom(8);expander.set_child(Some(&body));
        expander.set_expanded(view.expanded.borrow().get(&key).copied().unwrap_or(false));
        let input=sender.input_sender().clone();expander.connect_expanded_notify(move |widget|{let _=input.send(MainWindowMsg::Notification(NoticeMsg::GroupExpanded(key,widget.is_expanded())));});
        parent.append(&expander);let mut rows=Self{expander,body,badge,title,avatar,rows:Default::default()};rows.update(name,texture,author,0);rows
    }
    pub fn update(&mut self,name:&str,texture:Option<&gtk::gdk::Texture>,author:Option<&crate::models::UserSummary>,unread:usize){
        self.title.set_text(name);self.title.set_attributes(None);self.title.remove_css_class("papo-role-name");self.title.set_tooltip_text(None);
        if let Some(author)=author{self.title.set_tooltip_text(Some(&format!("@{}",author.username)));if let Some(roles)=author.roles.as_deref(){crate::ui::style::role_color(&self.title,roles);}}
        if let Some(avatar)=&self.avatar{avatar.set_text(Some(name));avatar.set_custom_image(texture);}
        self.badge.set_text(&unread.to_string());self.badge.set_visible(unread>0);
        let label=format!("{unread} notificações não lidas carregadas. Outras páginas podem não estar nesta janela.");
        self.badge.update_property(&[gtk::accessible::Property::Label(&label)]);self.badge.set_tooltip_text(Some(&label));
    }
}
pub(super) fn reconcile(parent:&gtk::Box,desired:&[gtk::Widget]){
    let mut child=parent.first_child();while let Some(widget)=child{child=widget.next_sibling();if !desired.contains(&widget){parent.remove(&widget);}}
    let mut previous=None::<gtk::Widget>;
    for widget in desired{if widget.parent().is_none(){parent.append(widget);}if widget.prev_sibling()!=previous{parent.reorder_child_after(widget,previous.as_ref());}previous=Some(widget.clone());}
}
impl MainWindowModel {
    pub(super) fn notice_author(&self,n:&state::Notice)->Option<&crate::models::UserSummary>{
        n.author_id.and_then(|id|self.users.iter().find(|u|u.id==id).or_else(||self.direct.items.iter().map(|d|&d.user).find(|u|u.id==id)))
    }
    pub(super) fn notice_group(&self,n:&state::Notice,available:bool)->Group{
        if !self.personal_notice(n){return n.channel_id.map(Group::Channel).unwrap_or(Group::Unknown);}
        if available{if let Some(id)=n.author_id.or_else(||self.direct.items.iter().find(|d|Some(d.id)==n.channel_id).map(|d|d.user.id)){return Group::Sender(id);}}
        n.channel_id.map(Group::Conversation).unwrap_or(Group::Unknown)
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants};
    use crate::models::NotificationSummary;
    use adw::prelude::*;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let dm_a=Uuid::from_u128(110);let dm_b=Uuid::from_u128(111);let peer_a=Uuid::from_u128(210);let peer_b=Uuid::from_u128(211);
    {let mut b=backend.lock().unwrap();b.fail_read=false;b.dms=[(dm_a,peer_a,"first"),(dm_b,peer_b,"second")].into_iter().map(|(id,peer,username)|serde_json::json!({"id":id,"user":{"id":peer,"username":username,"nickname":"Same name","created_at":"2026-10-03T00:00:00Z"},"created_at":"2026-10-03T00:00:00Z","unread_count":0})).collect();}
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().direct.items.len()==2&&main.model().access_request.is_none()&&main.model().channels.len()>=2);
    main.emit(MainWindowMsg::Notification(NoticeMsg::Open));until(context,||main.model().notifications.request.is_none()&&main.model().notifications.marks.is_empty());
    let (filter,tabs,rows,personal,window,a,b)={let model=main.model();let v=model.notifications.view.as_ref().unwrap();(v.channel_filter.widget.clone(),v.tabs.clone(),v.rows.clone(),v.personal_rows.clone(),v.window.clone(),model.channels[0].id,model.channels[1].id)};
    let select=|id:Option<Uuid>|{let index={let model=main.model();let ids=model.notifications.view.as_ref().unwrap().channel_filter.ids.borrow();ids.iter().position(|current|*current==id).unwrap()};filter.set_selected(index as u32);pump(context);};
    select(Some(a));
    let a1=Uuid::new_v4();let a2=Uuid::new_v4();let b1=Uuid::new_v4();let dm1=Uuid::new_v4();let dm2=Uuid::new_v4();let dm3=Uuid::new_v4();
    let notice=|id,channel,author,second:i64|NotificationSummary{id,message_id:Uuid::new_v4(),channel_id:channel,author_id:Some(author),message_content:"Filter fixture".into(),read:false,created_at:chrono::DateTime::from_timestamp(1791028800+second,0).unwrap()};
    {let mut state=main.state().get_mut();state.model.notifications.inbox=Inbox::default();state.model.notifications.inbox.merge(vec![notice(a1,a,peer_a,0),notice(a2,a,peer_a,1),notice(b1,b,peer_b,2),notice(dm1,dm_a,peer_a,3),notice(dm2,dm_a,peer_a,4),notice(dm3,dm_b,peer_b,5)]);}
    let baseline=backend.lock().unwrap().requests.len();main.emit(MainWindowMsg::Notification(NoticeMsg::FilterChanged));pump(context);
    let contains=|root:&gtk::Box,id|{
        let mut widgets=descendants(root.upcast_ref());let nested:Vec<_>=widgets.iter().filter_map(|w|w.downcast_ref::<gtk::Expander>().and_then(|e|e.child())).collect();
        for child in nested{widgets.extend(descendants(&child));}widgets.iter().any(|w|w.widget_name()==format!("notification-{id}"))
    };
    let channel_group=|id|descendants(rows.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Expander>().ok().filter(|w|w.widget_name()==Group::Channel(id).name())).unwrap();
    let channel_badge=|id|descendants(channel_group(id).upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|w|w.widget_name()=="notification-channel-unread")).unwrap();
    assert!(!channel_group(a).is_expanded());assert_eq!(channel_badge(a).text(),"2");
    assert!(!main.model().notifications.inbox.rows[&a1].read&&!main.model().notifications.inbox.rows[&a2].read,"opening the window or filtering must not read a collapsed channel");
    channel_group(a).set_expanded(true);until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&a1].read&&main.model().notifications.inbox.rows[&a2].read);
    assert!(!channel_badge(a).is_visible());
    assert!(contains(&rows,a1)&&contains(&rows,a2)&&!contains(&rows,b1));assert!(!main.model().notifications.inbox.rows[&b1].read,"a hidden channel must stay unread");
    let dm_page=tabs.page(&tabs.child_by_name("personal").unwrap());
    assert_eq!(tabs.visible_child_name().as_deref(),Some("server"));
    assert_eq!(dm_page.title().as_deref(),Some("Mensagens diretas (3)"),"the inactive DM tab must show the sum of unread messages, not the number of senders");
    assert!(descendants(window.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|label|label.is_visible()&&label.text()=="Mensagens diretas (3)"),"the count must be on the tab button");
    let group=|peer|descendants(personal.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Expander>().ok().filter(|w|w.widget_name()==Group::Sender(peer).name())).unwrap();
    assert_eq!(descendants(personal.upcast_ref()).iter().filter(|w|w.is::<gtk::Expander>()).count(),2,"equal nicknames must remain separate sender groups");
    assert!(group(peer_a).child().unwrap().first_child().is_none(),"collapsed groups must not build message widgets");
    assert!(!group(peer_a).is_expanded()&&!group(peer_b).is_expanded(),"DM senders start collapsed");
    let badge=|peer|descendants(group(peer).upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|w|w.widget_name()=="notification-sender-unread")).unwrap();
    assert_eq!(badge(peer_a).text(),"2");assert_eq!(badge(peer_b).text(),"1");assert!(badge(peer_a).is_visible()&&badge(peer_b).is_visible());
    select(Some(b));assert!(!contains(&rows,b1)&&!contains(&rows,a1),"collapsed channels have only a header");
    assert!(!channel_group(b).is_expanded());assert_eq!(channel_badge(b).text(),"1");assert!(!main.model().notifications.inbox.rows[&b1].read);
    select(None);assert!(channel_group(a).is_expanded()&&!channel_group(b).is_expanded(),"channel expansion survives filtering without expanding other channels");
    crate::ui::main_window::layout::tests::preview(&window,"notification-channel-groups",context);
    select(Some(b));
    assert_eq!(dm_page.title().as_deref(),Some("Mensagens diretas (3)"),"server channel filters must not change the DM total");
    crate::ui::main_window::layout::tests::preview(&window,"notification-channel-filter",context);
    tabs.set_visible_child_name("personal");pump(context);
    assert!([dm1,dm2,dm3].iter().all(|id|!main.model().notifications.inbox.rows[id].read),"switching to DMs must not read collapsed groups");
    until(context,||badge(peer_a).is_mapped()&&badge(peer_b).is_mapped());
    crate::ui::main_window::layout::tests::preview(&window,"notification-sender-groups",context);
    group(peer_a).set_expanded(true);until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&dm1].read&&main.model().notifications.inbox.rows[&dm2].read);
    assert!(!badge(peer_a).is_visible(),"successful reads clear only the opened sender's badge");assert_eq!(badge(peer_b).text(),"1");
    assert_eq!(dm_page.title().as_deref(),Some("Mensagens diretas (1)"));
    assert!(!main.model().notifications.inbox.rows[&dm3].read,"a collapsed sender must stay unread");assert!(!group(peer_b).is_expanded());
    assert!(!filter.is_mapped(),"channel filters belong only to the server tab");
    group(peer_b).set_expanded(true);until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&dm3].read);
    assert_eq!(dm_page.title().as_deref(),Some("Mensagens diretas"),"the tab count disappears after all accessible DMs are read");
    group(peer_b).set_expanded(false);pump(context);
    // Paginated data must respect both filter and group state.
    let extra=Uuid::new_v4();let dm_extra=Uuid::new_v4();let token=Uuid::new_v4();main.state().get_mut().model.notifications.request=Some(token);
    main.emit(MainWindowMsg::Notification(NoticeMsg::Loaded{token,cursor:Some(MessageCursor{created_at:chrono::Utc::now(),id:token}),result:Ok(NotificationList{notifications:vec![notice(extra,a,peer_a,6),notice(dm_extra,dm_b,peer_b,7)],has_more:false})}));
    until(context,||main.model().notifications.request.is_none());pump(context);
    assert_eq!(dm_page.title().as_deref(),Some("Mensagens diretas (1)"),"new unread notifications update the tab count");
    assert!(!group(peer_b).is_expanded());assert!(badge(peer_b).is_visible());assert_eq!(badge(peer_b).text(),"1","new notifications update a collapsed sender's count");assert!(!main.model().notifications.inbox.rows[&dm_extra].read);assert!(!main.model().notifications.inbox.rows[&extra].read);
    tabs.set_visible_child_name("server");pump(context);assert!(!contains(&rows,b1)&&!contains(&rows,extra));assert_eq!(main.model().notifications.view.as_ref().unwrap().channel_filter.selected(),Some(b));
    assert!(!main.model().notifications.inbox.rows[&b1].read);assert_eq!(channel_badge(b).text(),"1");
    channel_group(b).set_expanded(true);until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&b1].read);assert!(!channel_badge(b).is_visible());
    select(None);until(context,||main.model().notifications.marks.is_empty()&&main.model().notifications.inbox.rows[&extra].read);assert!(contains(&rows,a1)&&contains(&rows,b1)&&contains(&rows,extra));
    assert!(channel_group(a).is_expanded()&&channel_group(b).is_expanded());
    // Renames/reordering preserve selection by ID, and access loss removes it.
    select(Some(b));{let mut state=main.state().get_mut();state.model.channels.iter_mut().find(|c|c.id==b).unwrap().name="AAA renamed".into();}
    main.emit(MainWindowMsg::Notification(NoticeMsg::TabChanged));pump(context);assert_eq!(main.model().notifications.view.as_ref().unwrap().channel_filter.selected(),Some(b));
    {let mut state=main.state().get_mut();state.model.access.get_mut(&b).unwrap().read=false;}
    main.emit(MainWindowMsg::Notification(NoticeMsg::TabChanged));pump(context);assert_eq!(main.model().notifications.view.as_ref().unwrap().channel_filter.selected(),None);
    assert!(!main.model().notifications.view.as_ref().unwrap().channel_filter.ids.borrow().contains(&Some(b)));
    let requests=backend.lock().unwrap().requests[baseline..].to_vec();assert!(!requests.iter().any(|(method,path,_)|method=="GET"&&path.contains("/notifications")),"local filters and sender groups must not refetch pages");
    // Unread notifications in a no-longer-accessible DM must not inflate the tab.
    main.state().get_mut().model.direct.items.retain(|dm|dm.id!=dm_b);
    main.emit(MainWindowMsg::Notification(NoticeMsg::TabChanged));pump(context);
    assert_eq!(dm_page.title().as_deref(),Some("Mensagens diretas"));
    // A row nested inside a group still opens the exact DM/message.
    tabs.set_visible_child_name("personal");pump(context);group(peer_a).set_expanded(true);pump(context);
    let open=descendants(personal.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|w|w.widget_name()==format!("notification-{dm1}"))).unwrap();open.emit_clicked();until(context,||main.model().active_channel_id==Some(dm_a));
    window.close();host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
