//! EN-US: Reaction filters and stable member rows share the main window's metadata cache.
//! PT-BR: Filtros de reações e linhas estáveis de membros compartilham o cache de metadados da janela principal.
use super::*;
use adw::prelude::*;

const PAGE:usize=50;
#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub struct Key(pub Option<Uuid>,pub Option<String>);
impl Key{fn of(group:&ReactionGroup)->Self{Self(group.emoji_id,group.unicode.clone())}}
struct Member{
    row:gtk::ListBoxRow,button:gtk::Button,avatar:adw::Avatar,name:gtk::Label,username:gtk::Label,
}
pub(super) struct View{
    pub window:gtk::Window,pub id:Uuid,pub token:Uuid,pub list:gtk::ListBox,
    filters:gtk::Box,navigation:gtk::Box,stack:gtk::Stack,spinner:gtk::Spinner,status:gtk::Label,error:gtk::Label,
    scroll:gtk::ScrolledWindow,more_button:gtk::Button,selected:Option<Key>,limit:usize,
    filter_signature:Vec<(Key,usize,String,usize)>,buttons:Vec<(Key,gtk::ToggleButton)>,
    members:HashMap<Uuid,Member>,requested:HashSet<Uuid>,sized:bool,
}
impl Drop for View{fn drop(&mut self){self.window.close();}}
impl View{
    pub fn new(root:&gtk::Box,id:Uuid,sender:&ComponentSender<ChatModel>)->Self{
        let token=Uuid::new_v4();
        let window:gtk::Window=adw::Window::builder().title("Reações").default_width(500).default_height(540).modal(true).build().upcast();window.set_widget_name("reaction-participants");window.add_css_class("papo-reaction-browser");window.set_size_request(320,260);
        if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
        let toolbar=adw::ToolbarView::new();let header=adw::HeaderBar::new();header.set_title_widget(Some(&adw::WindowTitle::new("Reações","")));
        toolbar.add_top_bar(&header);
        let body=gtk::Box::new(gtk::Orientation::Horizontal,0);
        let navigation=gtk::Box::new(gtk::Orientation::Horizontal,0);navigation.add_css_class("papo-reaction-filters");
        let filters=gtk::Box::new(gtk::Orientation::Vertical,6);filters.set_margin_start(8);filters.set_margin_end(8);filters.set_margin_top(12);filters.set_margin_bottom(12);
        let choices=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).min_content_width(104).child(&filters).build();choices.set_vexpand(true);navigation.append(&choices);navigation.append(&gtk::Separator::new(gtk::Orientation::Vertical));body.append(&navigation);
        let stack=gtk::Stack::new();stack.set_hexpand(true);stack.set_vexpand(true);stack.set_hhomogeneous(false);stack.set_vhomogeneous(false);body.append(&stack);
        let loading=gtk::Box::new(gtk::Orientation::Vertical,12);loading.set_halign(gtk::Align::Center);loading.set_valign(gtk::Align::Center);loading.set_margin_start(16);loading.set_margin_end(16);
        let spinner=gtk::Spinner::new();spinner.set_size_request(28,28);spinner.start();loading.append(&spinner);
        let status=gtk::Label::new(Some("Carregando reações…"));status.set_wrap(true);status.set_justify(gtk::Justification::Center);status.add_css_class("dim-label");loading.append(&status);stack.add_named(&loading,Some("status"));
        let content=gtk::Box::new(gtk::Orientation::Vertical,4);content.set_margin_start(8);content.set_margin_end(8);content.set_margin_top(8);content.set_margin_bottom(8);
        let list=gtk::ListBox::new();list.set_selection_mode(gtk::SelectionMode::None);list.add_css_class("papo-reaction-members");content.append(&list);
        let more_button=gtk::Button::with_label("Mostrar mais membros");more_button.add_css_class("flat");let input=sender.input_sender().clone();more_button.connect_clicked(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::MoreParticipants(token)));});content.append(&more_button);
        let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&content).build();scroll.set_vexpand(true);crate::ui::smooth_scroll::connect(&scroll,&Default::default());stack.add_named(&scroll,Some("members"));
        let outer=gtk::Box::new(gtk::Orientation::Vertical,0);outer.append(&body);body.set_vexpand(true);
        let error=gtk::Label::new(None);error.set_wrap(true);error.set_lines(3);error.set_ellipsize(pango::EllipsizeMode::End);error.set_xalign(0.0);error.add_css_class("error");error.set_margin_start(12);error.set_margin_end(12);error.set_margin_bottom(8);error.set_visible(false);outer.append(&error);
        toolbar.set_content(Some(&outer));window.downcast_ref::<adw::Window>().unwrap().set_content(Some(&toolbar));
        let input=sender.input_sender().clone();window.connect_close_request(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::ParticipantsClosed(token)));gtk::glib::Propagation::Proceed});
        let keys=gtk::EventControllerKey::new();let weak=window.downgrade();keys.connect_key_pressed(move |_,key,_,_|{if key==gtk::gdk::Key::Escape{if let Some(window)=weak.upgrade(){window.close();}gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});window.add_controller(keys);
        Self{window,id,token,list,filters,navigation,stack,spinner,status,error,scroll,more_button,selected:None,limit:PAGE,filter_signature:vec![],buttons:vec![],members:HashMap::new(),requested:HashSet::new(),sized:false}
    }
    pub fn select(&mut self,key:Key){if self.buttons.iter().any(|(k,_)|*k==key)&&self.selected.as_ref()!=Some(&key){self.selected=Some(key);self.limit=PAGE;self.scroll.vadjustment().set_value(0.0);}}
    pub fn more(&mut self){self.limit=self.limit.saturating_add(PAGE);}
    pub fn retry(&mut self){self.requested.clear();self.error.set_visible(false);}
    pub fn loaded(&self){self.error.set_visible(false);}
    pub fn error(&self,text:&str){self.spinner.stop();self.spinner.set_visible(false);self.error.set_text(text);self.error.set_visible(true);if self.filter_signature.is_empty(){self.status.set_text("Reações indisponíveis. Feche esta janela e tente novamente.");}}
    pub fn render(&mut self,groups:Option<&[ReactionGroup]>,users:&HashMap<Uuid,UserSummary>,avatars:&HashMap<Uuid,gtk::gdk::Texture>,actions:&Actions,own:Option<Uuid>,sender:&ComponentSender<ChatModel>)->Vec<Uuid>{
        let Some(groups)=groups else{self.navigation.set_visible(false);return vec![];};
        self.spinner.stop();self.spinner.set_visible(false);self.navigation.set_visible(!groups.is_empty());
        if groups.is_empty(){self.status.set_text("Nenhuma reação nesta mensagem.");self.stack.set_visible_child_name("status");self.members.clear();while let Some(row)=self.list.first_child(){self.list.remove(&row);}self.selected=None;}
        if !groups.iter().any(|g|Some(Key::of(g))==self.selected){self.selected=groups.first().map(Key::of);self.limit=PAGE;}
        let signature:Vec<_>=groups.iter().map(|group|{
            let name=group.unicode.clone().or_else(||group.emoji_id.map(|id|actions.emoji_label(id))).unwrap_or_else(||"Emoji removido".into());
            (Key::of(group),group.users.len(),name,group.emoji_id.and_then(|id|actions.emoji_paintable(id)).map_or(0,|p|p.as_ptr() as usize))
        }).collect();
        if signature!=self.filter_signature{
            self.buttons.clear();while let Some(child)=self.filters.first_child(){self.filters.remove(&child);}
            for (key,count,name,_) in &signature{
                let button=gtk::ToggleButton::new();if let Some((_,first))=self.buttons.first(){button.set_group(Some(first));}button.add_css_class("flat");button.add_css_class("papo-reaction-filter");button.set_tooltip_text(Some(name));let accessible=format!("{name} · {count} reações");button.update_property(&[gtk::accessible::Property::Label(&accessible)]);
                let content=gtk::Box::new(gtk::Orientation::Horizontal,8);content.set_halign(gtk::Align::Center);
                if let Some(id)=key.0{if let Some(image)=actions.emoji_image(id,24){content.append(&image);}else{let image=gtk::Image::from_icon_name("face-smile-symbolic");image.set_pixel_size(24);content.append(&image);}}
                else if let Some(unicode)=key.1.as_deref(){let emoji=gtk::Label::new(Some(unicode));emoji.add_css_class("title-3");content.append(&emoji);}
                else{let image=gtk::Image::from_icon_name("face-smile-symbolic");image.set_pixel_size(24);content.append(&image);}
                let count=gtk::Label::new(Some(&count.to_string()));count.add_css_class("heading");content.append(&count);button.set_child(Some(&content));
                let input=sender.input_sender().clone();let token=self.token;let key_copy=key.clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::SelectParticipants{token,key:key_copy.clone()}));});self.filters.append(&button);self.buttons.push((key.clone(),button));
            }self.filter_signature=signature;
        }
        for (key,button) in &self.buttons{button.set_active(self.selected.as_ref()==Some(key));}
        let Some(group)=groups.iter().find(|g|Some(Key::of(g))==self.selected)else{return vec![];};
        let mut seen=HashSet::new();let all:Vec<_>=group.users.iter().map(|u|u.user_id).filter(|id|seen.insert(*id)).collect();let ids:Vec<_>=all.iter().copied().take(self.limit).collect();
        if !self.sized{self.window.set_default_size(500,(88+all.len().min(10) as i32*52).clamp(260,620));self.sized=true;}
        self.more_button.set_visible(ids.len()<all.len());
        let removed:Vec<_>=self.members.keys().copied().filter(|id|!ids.contains(id)).collect();for id in removed{if let Some(member)=self.members.remove(&id){self.list.remove(&member.row);}}
        for (index,id) in ids.iter().enumerate(){
            let member=self.members.entry(*id).or_insert_with(||Member::new(*id,own,&self.window,sender));member.update(users.get(id),avatars.get(id));
            if self.list.row_at_index(index as i32).as_ref()!=Some(&member.row){if member.row.parent().is_some(){self.list.remove(&member.row);}self.list.insert(&member.row,index as i32);}
        }
        if ids.is_empty(){self.status.set_text("Nenhum membro nesta reação.");self.stack.set_visible_child_name("status");}else{self.stack.set_visible_child_name("members");}
        ids.into_iter().filter(|id|self.requested.insert(*id)).collect()
    }
}
impl Member{
    fn new(id:Uuid,own:Option<Uuid>,window:&gtk::Window,sender:&ComponentSender<ChatModel>)->Self{
        let row=gtk::ListBoxRow::new();row.set_selectable(false);row.set_activatable(false);
        let button=gtk::Button::new();button.add_css_class("flat");button.add_css_class("papo-reaction-member");button.set_hexpand(true);let input=sender.input_sender().clone();let weak=window.downgrade();button.connect_clicked(move |_|{let Some(window)=weak.upgrade().filter(|w|w.is_visible())else{return;};window.close();let _=input.send(ChatMsg::OpenProfile(id));});
        let body=gtk::Box::new(gtk::Orientation::Horizontal,12);let avatar=crate::media::member_avatar(id,None,36);avatar.set_show_initials(true);body.append(&avatar);
        let labels=gtk::Box::new(gtk::Orientation::Vertical,2);labels.set_hexpand(true);labels.set_valign(gtk::Align::Center);let heading=gtk::Box::new(gtk::Orientation::Horizontal,6);
        let name=gtk::Label::new(None);name.set_xalign(0.0);name.set_ellipsize(pango::EllipsizeMode::End);name.set_max_width_chars(30);name.add_css_class("heading");heading.append(&name);
        if own==Some(id){let own=gtk::Label::new(Some("(você)"));own.add_css_class("caption");own.add_css_class("dim-label");heading.append(&own);}labels.append(&heading);
        let username=gtk::Label::new(None);username.set_xalign(0.0);username.set_ellipsize(pango::EllipsizeMode::End);username.add_css_class("caption");username.add_css_class("dim-label");labels.append(&username);body.append(&labels);button.set_child(Some(&body));row.set_child(Some(&button));Self{row,button,avatar,name,username}
    }
    fn update(&self,user:Option<&UserSummary>,texture:Option<&gtk::gdk::Texture>){
        let name=user.map(UserSummary::display_name).unwrap_or("Membro indisponível");self.name.set_text(name);self.name.set_attributes(None);self.name.remove_css_class("papo-role-name");if let Some(roles)=user.and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&self.name,roles);}
        self.username.set_text(&user.map(|u|format!("@{}",u.username)).unwrap_or_else(||"Perfil não carregado".into()));self.avatar.set_text(Some(name));crate::media::set_avatar(&self.avatar,texture);self.button.set_tooltip_text(Some(&format!("Abrir perfil de {name}")));
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use super::tests::{mock,pump,until,descendants};use std::{cell::RefCell,rc::Rc};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let own:Uuid=super::tests::USER.parse().unwrap();let channel:Uuid=super::tests::CHANNEL.parse().unwrap();let message:Uuid=super::tests::MESSAGE.parse().unwrap();let emoji=Uuid::new_v4();
    let ids:Vec<_>=std::iter::once(own).chain((0..124).map(|_|Uuid::new_v4())).collect();
    let users:Vec<UserSummary>=ids.iter().enumerate().map(|(i,id)|serde_json::from_value(serde_json::json!({"id":id,"username":format!("member{i}"),"nickname":if i==0{"Álícia".into()}else{format!("Membro {i}")},"created_at":super::tests::DATE,"roles":[{"id":channel,"name":"Membro","color":"#33aa77"}]})).unwrap()).collect();
    let group=|emoji_id:Option<Uuid>,unicode:Option<&str>,users:&[Uuid]|serde_json::json!({"emoji_id":emoji_id,"unicode":unicode,"count":users.len(),"users":users.iter().map(|id|serde_json::json!({"id":Uuid::new_v4(),"user_id":id,"created_at":super::tests::DATE})).collect::<Vec<_>>()} );
    backend.lock().unwrap().groups=vec![group(Some(emoji),None,&ids),group(None,Some("❤️"),&ids[1..3])];
    let output=Rc::new(RefCell::new(Vec::new()));let capture=output.clone();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),user_id:Some(own),..Default::default()}).connect_receiver(move |_,event|capture.borrow_mut().push(event));
    let host=adw::Window::builder().default_width(850).default_height(650).content(chat.widget()).build();host.present();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"reactions","created_at":super::tests::DATE})).unwrap();chat.emit(ChatMsg::SetChannel(target));chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::direct()});chat.emit(ChatMsg::RequestHistory{channel_id:channel,cursor:None});
    until(context,||chat.model().actions.pins_ready&&chat.model().actions.emojis_loaded.is_some());
    {let mut state=chat.state().get_mut();state.model.actions.emojis.push(serde_json::from_value(serde_json::json!({"id":emoji,"name":"wave","format":"PNG","created_at":super::tests::DATE})).unwrap());let animation=crate::media::animation::PreparedAnimation::decode(&crate::media::animation::fixture()).unwrap().paintable();state.model.actions.animations.insert(emoji,animation);}
    chat.emit(ChatMsg::SetUsers(users.iter().filter(|u|u.id!=ids[1]).cloned().collect()));pump(context);output.borrow_mut().clear();
    chat.emit(ChatMsg::Action(ActionMsg::OpenParticipants(message)));
    until(context,||chat.model().actions.participants.as_ref().is_some_and(|v|v.members.len()==50));
    let view=chat.model().actions.participants.as_ref().unwrap().window.clone();let token=chat.model().actions.participants.as_ref().unwrap().token;
    assert!(view.is_visible());assert_eq!(chat.model().actions.participants.as_ref().unwrap().buttons.len(),2);
    assert!(descendants(view.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="125"));
    assert!(descendants(view.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Image>()).any(|i|i.paintable().is_some_and(|p|p.is::<crate::media::animation::Animation>())));
    let emitted:Vec<_>=output.borrow().iter().filter_map(|o|if let ChatOutput::AuthorMetadata{ids,..}=o{Some(ids.len())}else{None}).collect();assert_eq!(emitted.iter().filter(|n|**n==50).count(),1);assert!(emitted.iter().all(|n|*n<=50),"metadata must use bounded batches for the selected group");
    let row=chat.model().actions.participants.as_ref().unwrap().members[&ids[1]].row.clone();assert_eq!(chat.model().actions.participants.as_ref().unwrap().members[&ids[1]].name.text(),"Membro indisponível");
    let texture=crate::media::PreparedImage{width:32,height:32,pixels:vec![180;32*32*4]}.texture();chat.emit(ChatMsg::SetUsers(users.clone()));chat.emit(ChatMsg::SetAvatars(HashMap::from([(own,texture.clone())])));pump(context);
    let model=chat.model();let v=model.actions.participants.as_ref().unwrap();assert_eq!(v.members[&ids[1]].row,row);assert_eq!(v.members[&ids[1]].username.text(),"@member1");assert!(v.members[&ids[1]].name.attributes().is_some());assert_eq!(v.members[&own].avatar.custom_image(),Some(texture.upcast()));drop(model);
    let calls=backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.contains("/reactions")).count();
    let more=chat.model().actions.participants.as_ref().unwrap().more_button.clone();more.emit_clicked();pump(context);assert_eq!(chat.model().actions.participants.as_ref().unwrap().members.len(),100);assert_eq!(chat.model().actions.participants.as_ref().unwrap().members[&ids[1]].row,row);
    let heart=chat.model().actions.participants.as_ref().unwrap().buttons[1].1.clone();heart.emit_clicked();pump(context);assert_eq!(chat.model().actions.participants.as_ref().unwrap().members.len(),2);assert!(!chat.model().actions.participants.as_ref().unwrap().more_button.is_visible());assert!(heart.is_active());
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.contains("/reactions")).count(),calls,"selecting reactions must not reload the server");
    chat.emit(ChatMsg::Action(ActionMsg::SelectParticipants{token:Uuid::new_v4(),key:Key(Some(emoji),None)}));pump(context);assert_eq!(chat.model().actions.participants.as_ref().unwrap().selected,Some(Key(None,Some("❤️".into()))));
    chat.model().actions.participants.as_ref().unwrap().buttons[0].1.emit_clicked();pump(context);assert_eq!(chat.model().actions.participants.as_ref().unwrap().members.len(),50);
    // Refresh retains the selected emoji and existing row widgets.
    chat.emit(ChatMsg::Action(ActionMsg::Reconcile(message)));until(context,||!chat.model().actions.reaction_requests.contains_key(&message));assert_eq!(chat.model().actions.participants.as_ref().unwrap().selected,Some(Key(Some(emoji),None)));
    crate::ui::chat::performance::settle(context);crate::ui::main_window::layout::tests::preview(&view,"reaction-participants",context);
    view.set_default_size(340,400);crate::ui::chat::performance::settle(context);assert!(view.width()<=360,"the reaction window must fit a narrow layout: {}",view.width());
    let member=chat.model().actions.participants.as_ref().unwrap().members[&ids[1]].button.clone();member.emit_clicked();pump(context);assert!(output.borrow().iter().any(|o|matches!(o,ChatOutput::OpenProfile(id) if *id==ids[1])));assert!(chat.model().actions.participants.is_none());
    chat.emit(ChatMsg::Action(ActionMsg::OpenParticipants(message)));until(context,||chat.model().actions.participants.as_ref().is_some_and(|v|v.members.len()==50));let view=chat.model().actions.participants.as_ref().unwrap().window.clone();
    // Removing the selected emoji chooses the remaining reaction rather than a blank list.
    backend.lock().unwrap().groups.remove(0);chat.emit(ChatMsg::Action(ActionMsg::Reconcile(message)));until(context,||chat.model().actions.participants.as_ref().unwrap().selected==Some(Key(None,Some("❤️".into()))));assert_eq!(chat.model().actions.participants.as_ref().unwrap().members.len(),2);
    backend.lock().unwrap().groups.clear();chat.emit(ChatMsg::Action(ActionMsg::Reconcile(message)));until(context,||chat.model().actions.participants.as_ref().unwrap().filter_signature.is_empty());assert_eq!(chat.model().actions.participants.as_ref().unwrap().stack.visible_child_name().as_deref(),Some("status"));
    chat.model().actions.participants.as_ref().unwrap().error("Falha sintética");assert!(chat.model().actions.participants.as_ref().unwrap().error.is_visible());
    let keys=view.observe_controllers();let key=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();let _:bool=key.emit_by_name("key-pressed",&[&gtk::gdk::Key::Escape,&0u32,&gtk::gdk::ModifierType::empty()]);pump(context);assert!(chat.model().actions.participants.is_none());assert!(!view.is_visible());
    chat.emit(ChatMsg::Action(ActionMsg::OpenParticipants(message)));pump(context);assert!(chat.model().actions.participants.is_some());chat.emit(ChatMsg::Action(ActionMsg::ParticipantsClosed(token)));pump(context);assert!(chat.model().actions.participants.is_some(),"an old close event must not close a new window");chat.emit(ChatMsg::ClearChannel);pump(context);assert!(chat.model().actions.participants.is_none());
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);server.abort();pump(context);
}
