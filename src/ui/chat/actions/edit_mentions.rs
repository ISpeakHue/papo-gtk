//! The editor owns its mention state; it never mutates the channel's draft.
use super::*;

pub(super) struct Mentions {
    pub composer: composer::Composer,
    pub panel: gtk::ScrolledWindow,
    choices: gtk::Box,
    manual: bool,
    dismissed: Option<(String,i32)>,
    range: Option<(usize,usize)>,
    signature: String,
    token: Uuid,
    input: relm4::Sender<ChatMsg>,
}
impl Mentions {
    pub fn new(text:&gtk::TextView,outer:&gtk::Box,token:Uuid,sender:&ComponentSender<ChatModel>)->Self {
        let choices=gtk::Box::new(gtk::Orientation::Vertical,2);
        let panel=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(200).propagate_natural_height(true).child(&choices).visible(false).build();
        panel.set_widget_name("edit-mention-suggestions");panel.add_css_class("papo-mention-suggestions");panel.add_css_class("papo-member-suggestions");
        outer.append(&panel);input::shortcuts::completion(&panel,text);
        let composer=composer::Composer::default();let output=sender.input_sender().clone();
        composer.connect_changed(text,move |_|{let _=output.send(ChatMsg::Action(ActionMsg::EditMentions(token)));});
        let output=sender.input_sender().clone();
        text.buffer().connect_mark_set(move |_,_,mark|{if mark.name().as_deref()==Some("insert"){let _=output.send(ChatMsg::Action(ActionMsg::EditMentions(token)));}});
        Self{composer,panel,choices,manual:false,dismissed:None,range:None,signature:String::new(),token,input:sender.input_sender().clone()}
    }
    pub fn dismiss(&mut self,text:&gtk::TextView){
        self.dismissed=Some((self.composer.wire_text(),text.position()));self.manual=false;self.panel.set_visible(false);
    }
    pub fn open(&mut self){self.manual=true;self.dismissed=None;}
    pub fn select(&mut self,text:&gtk::TextView,id:Option<Uuid>,users:&HashMap<Uuid,UserSummary>){
        let caret=self.composer.mention_in(text,id,users,self.range);
        self.composer.focus_after_completion(text,caret);self.dismiss(text);
    }
    pub fn refresh(&mut self,text:&gtk::TextView,users:&HashMap<Uuid,UserSummary>,avatars:&HashMap<Uuid,gtk::gdk::Texture>,config:&crate::models::UserConfig,allowed:bool,everyone:bool){
        if !allowed||self.composer.is_composing(){self.panel.set_visible(false);return;}
        let dismissed=self.dismissed.as_ref()==Some(&(self.composer.wire_text(),text.position()));
        self.composer.refresh_names(text,users);
        if dismissed{self.dismiss(text);return;}
        let displayed=text.text();let span=self.composer.mention_span(text);
        let completion=mentions::completion(&displayed,text.position());
        if !self.manual&&span.is_none()&&completion.is_none(){self.panel.set_visible(false);return;}
        self.range=span.or_else(||completion.as_ref().map(|(s,e,_)|(*s,*e))).or_else(||{
            // The explicit picker inserts at the caret (or replaces a selection).
            let byte=|offset:i32|displayed.char_indices().nth(offset as usize).map_or(displayed.len(),|(i,_)|i);
            Some(text.buffer().selection_bounds().map(|(s,e)|(byte(s.offset()),byte(e.offset()))).unwrap_or_else(||{let at=byte(text.position());(at,at)}))
        });
        let filter=if self.manual||span.is_some(){String::new()}else{completion.map(|(_,_,f)|f.to_lowercase()).unwrap_or_default()};
        let mut members:Vec<_>=users.values().filter(|u|!u.banned&&(u.display_name().to_lowercase().contains(&filter)||u.username.to_lowercase().contains(&filter))).collect();
        members.sort_by_cached_key(|u|(u.display_name().to_lowercase(),u.username.to_lowercase(),u.id));
        let everyone=everyone&&"everyone".contains(&filter);
        let show=config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true);
        let signature=format!("{filter}|{everyone}|{show}|{:?}",members.iter().take(50).map(|u|(u.id,u.display_name(),&u.username,crate::models::role::role_rgb(u.roles.as_deref().unwrap_or_default()),avatars.get(&u.id).map(|t|t.as_ptr() as usize))).collect::<Vec<_>>());
        if self.signature!=signature{
            self.signature=signature;clear(&self.choices);
            for user in members.iter().take(50){let id=user.id;let token=self.token;let input=self.input.clone();
                self.choices.append(&mentions::choice(user,avatars.get(&id),show,move ||{let _=input.send(ChatMsg::Action(ActionMsg::EditMentionSelected(token,Some(id))));}));}
            if everyone{let input=self.input.clone();let token=self.token;let button=gtk::Button::with_label("@everyone");button.add_css_class("flat");button.connect_clicked(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::EditMentionSelected(token,None)));});self.choices.append(&button);}
            if members.is_empty()&&!everyone{let empty=label("Nenhum membro encontrado");empty.add_css_class("dim-label");self.choices.append(&empty);}
        }
        self.panel.set_vscrollbar_policy(if self.choices.measure(gtk::Orientation::Vertical,-1).1>200{gtk::PolicyType::Automatic}else{gtk::PolicyType::External});
        self.panel.set_visible(true);
    }
}
impl ChatModel {
    pub(in crate::ui::chat) fn refresh_edit_mentions(&mut self){
        if let Some(edit)=self.actions.edit.as_mut(){
            let allowed=!edit.pending&&self.user_id.is_some_and(|u|self.access.can_edit(u,edit.message.author_id));
            edit.mentions.refresh(&edit.text,&self.users_map,&self.avatars,&self.config,allowed,self.access.everyone);
        }
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use super::tests::{mock,pump,until,descendants,find_button};
    use adw::prelude::*;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let own="12345678-1234-4234-8234-123456789abc".parse().unwrap();
    let channel_id:Uuid="12345678-1234-4234-8234-123456789abd".parse().unwrap();
    let first=Uuid::new_v4();let second=Uuid::new_v4();let target=Uuid::new_v4();let missing=Uuid::new_v4();let banned=Uuid::new_v4();
    let member=|id,name,nick|serde_json::from_value::<UserSummary>(serde_json::json!({"id":id,"username":name,"nickname":nick,"created_at":chrono::Utc::now(),"roles":[{"id":Uuid::nil(),"name":"Member","color":"#2ec27e"}]})).unwrap();
    let mut users=vec![member(first,"first","Íris Silva"),member(second,"second","Íris Silva"),member(target,"ISpeakHue","ISpeakHue"),member(banned,"banned","Banned")];users[3].banned=true;
    // Replacing a complete mention must preserve adjacent punctuation.
    let probe=gtk::TextView::new();let composer=composer::Composer::default();let indexed=users.iter().cloned().map(|u|(u.id,u)).collect();
    composer.set_wire(&probe,&format!("@mention(<@{first}>)!"),&indexed);probe.set_position(2);
    composer.mention_in(&probe,Some(target),&indexed,composer.mention_span(&probe));
    assert_eq!(composer.wire_text(),format!("@mention(<@{target}>)!"));
    let channel=serde_json::from_value(serde_json::json!({"id":channel_id,"name":"Editing","created_at":chrono::Utc::now()})).unwrap();
    let wire=format!("Olá @mention(<@{first}>) e @mention(<@{second}>) e @mention(<@{missing}>)\n@everyone @literal");
    let message:Message=serde_json::from_value(serde_json::json!({"id":"12345678-1234-4234-8234-123456789abe","channel_id":channel_id,"author_id":own,"content":wire,"created_at":chrono::Utc::now()})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(channel),messages:vec![message.clone()],user_id:Some(own),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::direct()});chat.emit(ChatMsg::SetUsers(users.clone()));
    let host=adw::Window::builder().default_width(800).default_height(600).content(chat.widget()).build();host.present();performance::settle(context);
    let draft=composer::field(chat.widget().upcast_ref());draft.set_text("Keep my chat draft");pump(context);
    chat.emit(ChatMsg::Action(ActionMsg::OpenEdit(message.clone())));pump(context);
    let(model_token,window,text,panel)={let model=chat.model();let edit=model.actions.edit.as_ref().unwrap();(edit.token,edit.window.clone(),edit.text.clone(),edit.mentions.panel.clone())};
    performance::settle(context);
    assert_eq!(text.text(),"Olá @Íris Silva e @Íris Silva e @Usuário removido\n@everyone @literal");
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().mentions.composer.wire_text(),wire);
    let buffer=text.buffer();let tag=buffer.tag_table().lookup("papo-composer-mention").unwrap();assert!(buffer.iter_at_offset(5).has_tag(&tag));
    assert!(!panel.is_visible());assert!(window.height()<240,"hidden suggestions must keep the editor compact");
    let passes=chat.model().render_passes;
    // Moving into an existing highlighted name offers full-span replacement.
    text.set_position(7);until(context,||panel.is_visible());
    assert!(descendants(panel.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).all(|b|b.tooltip_text().as_deref()!=Some("@banned")&&b.label().as_deref()!=Some("@everyone")));
    let choice=find_button(panel.upcast_ref(),"@ISpeakHue");
    let name=descendants(choice.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|l|l.text()=="ISpeakHue")).unwrap();assert!(name.has_css_class("papo-role-name"));
    let image=crate::media::PreparedImage{width:8,height:8,pixels:[30,150,210,255].repeat(64)}.texture();chat.emit(ChatMsg::SetAvatars(HashMap::from([(target,image.clone())])));pump(context);
    let choice=find_button(panel.upcast_ref(),"@ISpeakHue");assert!(descendants(choice.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).any(|a|a.custom_image().as_ref()==Some(image.upcast_ref())));
    choice.grab_focus();choice.emit_clicked();pump(context);assert!(!panel.is_visible());assert!(!text.buffer().has_selection());
    text.emit_by_name::<()>("insert-at-cursor",&[&"novo "]);pump(context);
    let mut expected=format!("Olá @mention(<@{target}>) novo e @mention(<@{second}>) e @mention(<@{missing}>)\n@everyone @literal");
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().mentions.composer.wire_text(),expected,"switching one duplicate must preserve the other ID and unknown-user ID");
    // Add a new mention with keyboard completion; Enter/Tab must not save.
    text.set_position(-1);text.emit_by_name::<()>("insert-at-cursor",&[&"\n@IS"]);until(context,||panel.is_visible());
    let keys=text.observe_controllers();let keys=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    assert!(keys.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Tab,&0u32,&gtk::gdk::ModifierType::empty()]));pump(context);
    expected.push_str(&format!("\n@mention(<@{target}>) "));
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().mentions.composer.wire_text(),expected);assert!(!buffer.has_selection());
    assert!(!backend.lock().unwrap().requests.iter().any(|(m,_,_)|m=="PUT"));
    assert_eq!(chat.model().draft.text,"Keep my chat draft");assert_eq!(draft.text(),"Keep my chat draft");
    assert_eq!(chat.model().render_passes,passes,"editing mention text must not rebuild chat history");
    // Native composition must keep working in the editor, too.
    if std::env::var_os("PAPO_NATIVE_INPUT_TEST").is_some(){
        assert!(std::process::Command::new("setxkbmap").args(["-layout","br","-variant",""]).status().unwrap().success());
        text.grab_focus();multiline::keys_for("Editar mensagem",&["dead_acute"]);performance::settle(context);multiline::keys_for("Editar mensagem",&["a"]);
        until(context,||text.text().ends_with(" á"));expected.push('á');
    }
    // Late nickname data updates labels without changing mention identities.
    users[1].nickname=Some("Nova Íris".into());chat.emit(ChatMsg::SetUsers(users.clone()));pump(context);
    assert!(text.text().contains("@Nova Íris"));assert!(!panel.is_visible());
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().mentions.composer.wire_text(),expected);
    // Permission changes and stale selections cannot add unauthorized mentions.
    chat.emit(ChatMsg::Action(ActionMsg::EditMentionSelected(model_token,None)));chat.emit(ChatMsg::Action(ActionMsg::EditMentionSelected(model_token,Some(banned))));pump(context);
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().mentions.composer.wire_text(),expected);
    let save=find_button(window.upcast_ref(),"Salvar edição");save.emit_clicked();until(context,||chat.model().actions.edit.as_ref().is_some_and(|e|!e.pending&&!e.error.text().is_empty()));
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().mentions.composer.wire_text(),expected,"failed saves must preserve mention identities");
    save.emit_clicked();until(context,||chat.model().actions.edit.is_none());
    let edits:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p.ends_with(&message.id.to_string())).cloned().collect();assert_eq!(edits.len(),2);assert!(edits.iter().all(|(_,_,body)|body["content"]==expected&&body.get("embeds").is_none()));
    // Everyone is offered only with permission; Enter completes without saving.
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::resolve(own,Some(own),&[],&[],&[])});
    chat.emit(ChatMsg::Action(ActionMsg::OpenEdit(message.clone())));pump(context);
    let (entry,panel)={let model=chat.model();let edit=model.actions.edit.as_ref().unwrap();(edit.text.clone(),edit.mentions.panel.clone())};
    entry.set_position(-1);entry.emit_by_name::<()>("insert-at-cursor",&[&" @every"]);until(context,||panel.is_visible());
    let keys=entry.observe_controllers();let keys=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    assert!(keys.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Return,&0u32,&gtk::gdk::ModifierType::empty()]));pump(context);
    assert!(entry.text().ends_with("@everyone "));assert!(chat.model().actions.edit.is_some());
    find_button(chat.model().actions.edit.as_ref().unwrap().window.upcast_ref(),"Mencionar membro (digite @ para filtrar)").emit_clicked();pump(context);assert!(panel.is_visible());
    let token=chat.model().actions.edit.as_ref().unwrap().token;chat.emit(ChatMsg::Action(ActionMsg::DismissEditMentions(token)));pump(context);assert!(!panel.is_visible());
    chat.emit(ChatMsg::Action(ActionMsg::CancelEdit(token)));pump(context);
    // Closing and reopening gets fresh state; late callbacks cannot edit it.
    chat.emit(ChatMsg::Action(ActionMsg::OpenEdit(message.clone())));pump(context);let token=chat.model().actions.edit.as_ref().unwrap().token;
    chat.emit(ChatMsg::Action(ActionMsg::EditMentionSelected(model_token,Some(target))));pump(context);assert_eq!(chat.model().actions.edit.as_ref().unwrap().token,token);
    chat.model().actions.edit.as_ref().unwrap().window.close();until(context,||chat.model().actions.edit.is_none());
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);server.abort();pump(context);
}
