//! Readable mention labels in the native multiline composer; IDs stay in draft/request text.
use super::*;
use mentions::{DisplayText,Span};
use std::{cell::{Cell,RefCell},rc::Rc};
#[derive(Clone,Default)]
pub(super) struct Composer { value:Rc<RefCell<DisplayText>>, updating:Rc<Cell<bool>>, composing:Rc<Cell<bool>> }
fn byte(text:&str,position:i32)->usize {
    if position<0{text.len()}else{text.char_indices().nth(position as usize).map_or(text.len(),|(i,_)|i)}
}

#[cfg(test)]
pub(crate) fn exercise_suggestions(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,pump,until};use adw::prelude::*;
    use crate::models::UserConfig;
    let user=Uuid::new_v4();let peer=Uuid::new_v4();let role=Uuid::new_v4();let date=chrono::Utc::now();
    let mut members:Vec<UserSummary>=vec![serde_json::from_value(serde_json::json!({"id":peer,"username":"danilo","nickname":"Danilo","roles":[{"id":role,"name":"Member","color":"#2ec27e"}],"created_at":date})).unwrap()];
    members.extend((0..60).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"username":format!("member{i:02}"),"nickname":format!("Member {i:02}"),"created_at":date})).unwrap()));
    let channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"Suggestions","created_at":date})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});chat.emit(ChatMsg::SetUsers(members.clone()));
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();performance::settle(context);
    let widgets=descendants(chat.widget().upcast_ref());let entry=composer::field(chat.widget().upcast_ref());let panel=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>().filter(|w|w.widget_name()=="mention-suggestions")).unwrap().clone();
    entry.grab_focus_without_selecting();entry.set_text("@dan");entry.set_position(-1);until(context,||chat.model().mention_visible);performance::settle(context);
    let button=first_mention_button(panel.upcast_ref()).unwrap();let avatar=descendants(button.upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::Avatar>().ok()).unwrap();
    let nickname=descendants(button.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|w|w.text()=="Danilo")).unwrap();assert!(nickname.has_css_class("papo-role-name"));assert!(nickname.attributes().is_some());assert!(avatar.custom_image().is_none());
    let compact=|button:&gtk::Button|{let panel_bounds=panel.compute_bounds(&panel).unwrap();let row=button.compute_bounds(&panel).unwrap();assert!(panel_bounds.height()<=row.height()+12.0,"a single suggestion must fit its row and padding, panel={} row={}",panel_bounds.height(),row.height());};compact(&button);
    let texture=crate::media::PreparedImage{width:32,height:32,pixels:[30,150,210,255].repeat(32*32)}.texture();
    chat.emit(ChatMsg::SetAvatars(HashMap::from([(peer,texture.clone())])));pump(context);assert_eq!(first_mention_button(panel.upcast_ref()).unwrap(),button,"late avatar data must update the existing suggestion");assert_eq!(avatar.custom_image().as_ref(),Some(texture.upcast_ref()));
    for (color,name) in [(adw::ColorScheme::ForceLight,"mention-suggestions-light"),(adw::ColorScheme::ForceDark,"mention-suggestions-dark")]{let manager=adw::StyleManager::default();let previous=manager.color_scheme();manager.set_color_scheme(color);crate::ui::main_window::layout::tests::preview(&window,name,context);manager.set_color_scheme(previous);}
    members[0].roles.as_mut().unwrap()[0].color=Some("#e01b24".into());chat.emit(ChatMsg::SetUsers(members.clone()));pump(context);
    let recolored=first_mention_button(panel.upcast_ref()).unwrap();assert_ne!(button,recolored,"a role color change must refresh an open suggestion");let nickname=descendants(recolored.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|w|w.text()=="Danilo")).unwrap();let attributes=nickname.attributes().unwrap();let color=attributes.iterator().attrs().into_iter().find_map(|a|a.downcast::<pango::AttrColor>().ok()).unwrap().color();assert_eq!((color.red(),color.green(),color.blue()),(0xe0e0,0x1b1b,0x2424));
    chat.emit(ChatMsg::SetAvatars(HashMap::new()));pump(context);assert!(descendants(recolored.upcast_ref()).into_iter().filter_map(|w|w.downcast::<adw::Avatar>().ok()).all(|a|a.custom_image().is_none()));
    let mut config=UserConfig::default();config.display.as_mut().unwrap().show_avatars=Some(false);chat.emit(ChatMsg::SetConfig(config));pump(context);performance::settle(context);assert!(descendants(panel.upcast_ref()).into_iter().filter_map(|w|w.downcast::<adw::Avatar>().ok()).all(|a|!a.is_visible()));compact(&first_mention_button(panel.upcast_ref()).unwrap());chat.emit(ChatMsg::SetConfig(UserConfig::default()));pump(context);assert!(descendants(panel.upcast_ref()).into_iter().filter_map(|w|w.downcast::<adw::Avatar>().ok()).all(|a|a.is_visible()));
    entry.set_text("@member");entry.set_position(-1);pump(context);performance::settle(context);
    assert_eq!(descendants(panel.upcast_ref()).iter().filter(|w|w.is::<gtk::Button>()).count(),50);assert!(panel.height()<=260,"long suggestion lists must remain bounded");assert!(panel.vadjustment().upper()>panel.vadjustment().page_size(),"long lists must remain scrollable");
    panel.vadjustment().set_value(panel.vadjustment().upper());entry.set_text("@dan");entry.set_position(-1);pump(context);performance::settle(context);compact(&first_mention_button(panel.upcast_ref()).unwrap());assert!(panel.vadjustment().value()<1.0,"filtering back to one item must reset the scroll position");
    entry.set_text("@missing-user");entry.set_position(-1);pump(context);performance::settle(context);assert!(first_mention_button(panel.upcast_ref()).is_none());assert!(panel.height()<=64,"empty results must not reserve the tall list's height");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext) {
    use actions::tests::{descendants,mock,pump,until};use adw::prelude::*;
    text::exercise(context);
    exercise_suggestions(context);
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user="12345678-1234-4234-8234-123456789abc".parse().unwrap();let peer="12345678-1234-4234-8234-123456789ac0".parse().unwrap();let duplicate=Uuid::new_v4();
    let a="12345678-1234-4234-8234-123456789abd".parse().unwrap();let b=Uuid::new_v4();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"mentions","created_at":chrono::Utc::now()})).unwrap();
    let members:Vec<UserSummary>=[(peer,"bob"),(duplicate,"bob2")].into_iter().map(|(id,username)|serde_json::from_value(serde_json::json!({"id":id,"username":username,"nickname":"ISpeakHue","created_at":chrono::Utc::now()})).unwrap()).collect();
    let opened=Rc::new(RefCell::new(Vec::new()));let profiles=opened.clone();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a)),user_id:Some(user),api:Some(api),..Default::default()}).connect_receiver(move |_,output|{if let ChatOutput::OpenProfile(id)=output{profiles.borrow_mut().push(id);}});
    let mut access=crate::models::Access::direct();access.everyone=true;chat.emit(ChatMsg::SetAccess{user_id:user,access});chat.emit(ChatMsg::SetUsers(members));
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();performance::settle(context);
    let entry=composer::field(chat.widget().upcast_ref());entry.grab_focus();entry.set_text("é @I");entry.set_position(-1);pump(context);
    chat.emit(ChatMsg::MentionSelected(Some(peer)));pump(context);
    assert_eq!(entry.text(),"é @ISpeakHue ");assert_eq!(chat.model().draft.text,format!("é @mention(<@{peer}>) "));
    let buffer=entry.buffer();let tag=buffer.tag_table().lookup("papo-composer-mention").unwrap();assert!(buffer.iter_at_offset(2).has_tag(&tag));assert!(!buffer.iter_at_offset(12).has_tag(&tag));
    crate::ui::main_window::layout::tests::preview(&window,"composer-mention",context);
    let renamed:Vec<_>=chat.model().users_map.values().cloned().map(|mut u|{if u.id==peer{u.nickname=Some("Íris".into());}u}).collect();
    chat.emit(ChatMsg::SetUsers(renamed));pump(context);assert_eq!(entry.text(),"é @Íris ");assert_eq!(entry.position(),8);assert_eq!(chat.model().draft.text,format!("é @mention(<@{peer}>) "));
    let restored:Vec<_>=chat.model().users_map.values().cloned().map(|mut u|{u.nickname=Some("ISpeakHue".into());u}).collect();chat.emit(ChatMsg::SetUsers(restored));pump(context);assert_eq!(entry.position(),13);
    let mut position=entry.position();entry.insert_text("teste @I",&mut position);entry.set_position(position);pump(context);chat.emit(ChatMsg::MentionSelected(Some(duplicate)));pump(context);
    assert!(chat.model().draft.text.contains(&peer.to_string())&&chat.model().draft.text.contains(&duplicate.to_string()));
    entry.delete_text(2,12);pump(context);assert!(!chat.model().draft.text.contains(&peer.to_string()));assert!(chat.model().draft.text.contains(&duplicate.to_string()));
    let label_start=entry.text().find('@').unwrap();let mut position=entry.text()[..label_start+1].chars().count() as i32;entry.insert_text("x",&mut position);entry.set_position(position);pump(context);
    assert!(!chat.model().draft.text.contains("@mention("),"editing the name removes its target instead of tagging the wrong user");
    entry.set_text("@ISpeakHue");pump(context);assert_eq!(chat.model().draft.text,"@ISpeakHue","plain typed names do not acquire a hidden target");
    entry.set_text("@I");entry.set_position(-1);pump(context);chat.emit(ChatMsg::MentionSelected(Some(peer)));pump(context);
    chat.emit(ChatMsg::EmojiSelected("😄".into()));pump(context);assert_eq!(entry.text(),"@ISpeakHue 😄 ");assert_eq!(chat.model().draft.text,format!("@mention(<@{peer}>) 😄 "));
    chat.emit(ChatMsg::SetChannel(channel(b)));pump(context);assert!(entry.text().is_empty());chat.emit(ChatMsg::SetChannel(channel(a)));pump(context);
    assert_eq!(entry.text(),"@ISpeakHue 😄 ");assert_eq!(chat.model().draft.text,format!("@mention(<@{peer}>) 😄 "));
    chat.emit(ChatMsg::SendClicked);until(context,||!chat.model().draft.is_sending()&&chat.model().draft.error.is_some());assert_eq!(entry.text(),"@ISpeakHue 😄 ");
    chat.emit(ChatMsg::SendClicked);until(context,||chat.model().draft.text.is_empty());assert!(entry.text().is_empty());
    let posts:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="POST"&&p=="/messages").cloned().collect();assert_eq!(posts.len(),2);
    for(_,_,body) in posts{let body=body.as_str().unwrap();assert!(body.contains(&format!("@mention(<@{peer}>) 😄")));assert!(!body.contains("@ISpeakHue"));}
    chat.emit(ChatMsg::MentionSelected(None));pump(context);assert_eq!(entry.text(),"@everyone ");assert_eq!(chat.model().draft.text,"@everyone ");
    chat.emit(ChatMsg::SetChannel(channel(b)));pump(context);chat.emit(ChatMsg::SetChannel(channel(a)));pump(context);assert_eq!(entry.text(),"@everyone ");assert!(entry.buffer().iter_at_offset(0).has_tag(&entry.buffer().tag_table().lookup("papo-composer-mention").unwrap()));
    {let model=chat.model();let view=text::widget(&format!("Olá @mention(<@{peer}>)"),&model.users_map,&model.actions);let buffer=view.buffer();assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false),"Olá @ISpeakHue");assert!(!buffer.iter_at_offset(4).tags().is_empty());}
    let controllers=entry.observe_controllers();let key=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    for completion_key in [gtk::gdk::Key::Return,gtk::gdk::Key::Tab]{
        entry.set_text("é @bob2");entry.set_position(-1);entry.grab_focus_without_selecting();until(context,||chat.model().mention_visible);
        assert!(key.emit_by_name::<bool>("key-pressed",&[&completion_key,&0u32,&gtk::gdk::ModifierType::empty()]));pump(context);
        assert_eq!(entry.text(),"é @ISpeakHue ");assert!(entry.selection_bounds().is_none(),"autocomplete must leave a caret, not a selected name");assert_eq!(entry.position(),13);
        entry.emit_by_name::<()>("insert-at-cursor",&[&"continua 😄"]);pump(context);
        assert_eq!(entry.text(),"é @ISpeakHue continua 😄");assert_eq!(chat.model().draft.text,format!("é @mention(<@{duplicate}>) continua 😄"));
        entry.emit_by_name::<()>("backspace",&[]);pump(context);assert_eq!(entry.text(),"é @ISpeakHue continua ");assert_eq!(chat.model().draft.text,format!("é @mention(<@{duplicate}>) continua "));
        let choices=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()=="mention-suggestions").unwrap();
        entry.set_text("@bob2");entry.set_position(-1);until(context,||chat.model().mention_visible);let button=first_mention_button(&choices).unwrap();button.grab_focus();button.emit_clicked();pump(context);
        assert!(entry.selection_bounds().is_none(),"selecting a suggestion with focus on the popup must not select the completed entry");assert_eq!(entry.position(),11);
    }
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="POST"&&p=="/messages").count(),2,"autocomplete Enter must not send the draft");
    entry.emit_by_name::<()>("insert-at-cursor",&[&"native keyboard"]);pump(context);
    chat.emit(ChatMsg::SendClicked);until(context,||chat.model().draft.text.is_empty());
    let sent=backend.lock().unwrap().requests.iter().rev().find(|(m,p,_)|m=="POST"&&p=="/messages").unwrap().2.clone();assert!(sent.as_str().unwrap().contains(&format!("@mention(<@{duplicate}>) native keyboard")),"native typing must send the selected member ID");
    let message=Uuid::new_v4();let canonical=format!("é @mention(<@{peer}>) @bob2 @everyone");
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(serde_json::json!({"id":message,"channel_id":a,"author_id":user,"content":canonical,"created_at":chrono::Utc::now()})).unwrap()));performance::settle(context);
    let row=chat.model().rendered[&format!("message-{message}")].1.clone();let view=descendants(row.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();let buffer=view.buffer();assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false),"é @ISpeakHue @bob2 @everyone");
    let tag=buffer.tag_table().lookup("papo-mention").unwrap();assert!(buffer.iter_at_offset(3).has_tag(&tag)&&buffer.iter_at_offset(14).has_tag(&tag));assert_eq!(tag.background_rgba().unwrap().to_string(),"rgb(53,132,228)");
    let gestures=view.observe_controllers();let click=(0..gestures.n_items()).find_map(|i|gestures.item(i).and_downcast::<gtk::GestureClick>().filter(|g|g.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    let point=|offset|{let rect=view.iter_location(&buffer.iter_at_offset(offset));let(x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x()+1,rect.y()+1);(f64::from(x),f64::from(y))};
    for (offset,id) in [(3,peer),(14,duplicate)]{let(x,y)=point(offset);click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);pump(context);assert_eq!(opened.borrow().last(),Some(&id));}
    assert_eq!(opened.borrow().len(),2);
    let(x,y)=point(3);click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&(x+25.0),&y]);pump(context);assert_eq!(opened.borrow().len(),2,"dragging to select must not open a profile");
    buffer.select_range(&buffer.start_iter(),&buffer.end_iter());click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);pump(context);assert_eq!(opened.borrow().len(),2,"copying selected text must not open a profile");buffer.select_range(&buffer.start_iter(),&buffer.start_iter());
    let(x,y)=point(22);click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);pump(context);assert_eq!(opened.borrow().len(),2,"everyone has no individual user profile");
    crate::ui::main_window::layout::tests::preview(&window,"clickable-mentions",context);
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
impl Composer {
    pub fn focus_after_completion(&self,entry:&gtk::TextView,caret:i32){
        entry.grab_focus_without_selecting();entry.select_region(caret,caret);entry.set_position(caret);
    }
    pub fn connect(&self,entry:&gtk::TextView,sender:&ComponentSender<ChatModel>) {
        let buffer=entry.buffer();
        let composer=self.clone();buffer.connect_insert_text(move |_,at,text|{
            if !composer.updating.get(){let mut value=composer.value.borrow_mut();let start=byte(&value.text,at.offset());value.replace(start,start,text);}
        });
        let composer=self.clone();buffer.connect_delete_range(move |_,start,end|{
            if !composer.updating.get(){let mut value=composer.value.borrow_mut();let(start,end)=(byte(&value.text,start.offset()),byte(&value.text,end.offset()));value.replace(start,end,"");}
        });
        let composer=self.clone();let input=sender.input_sender().clone();let weak=entry.downgrade();buffer.connect_changed(move |_|{
            if composer.updating.get(){return;}
            let Some(entry)=weak.upgrade()else{return;};
            if composer.value.borrow().text!=entry.text().as_str(){*composer.value.borrow_mut()=DisplayText{text:entry.text().into(),spans:vec![]};}
            composer.highlight(&entry);let _=input.send(ChatMsg::InputChanged(composer.wire_text()));
        });
        // Native composition is transient GTK state, not a draft edit. Avoid
        // rebuilding the component while the input method owns the preedit.
        let composing=self.composing.clone();entry.connect_preedit_changed(move |_,text|composing.set(!text.is_empty()));
    }
    pub fn is_composing(&self)->bool{self.composing.get()}
    pub fn wire_text(&self)->String {self.value.borrow().wire_text()}
    pub fn refresh_names(&self,entry:&gtk::TextView,users:&HashMap<Uuid,UserSummary>) {
        let old=self.value.borrow();if old.spans.is_empty(){return;}
        let next=DisplayText::parse(&old.wire_text(),users);if old.text==next.text{return;}
        let position=entry.position();let mut caret=position;
        for(before,after) in old.spans.iter().zip(&next.spans){
            let start=old.text[..before.start].chars().count() as i32;let end=old.text[..before.end].chars().count() as i32;
            if position<start{break;}
            let length=next.text[after.start..after.end].chars().count() as i32;
            if position<end{caret=next.text[..after.start].chars().count() as i32+(position-start).min(length);break;}
            caret+=length-(end-start);
        }
        drop(old);*self.value.borrow_mut()=next;self.refresh(entry);entry.set_position(caret);
    }
    pub fn set_wire(&self,entry:&gtk::TextView,text:&str,users:&HashMap<Uuid,UserSummary>) {
        *self.value.borrow_mut()=DisplayText::parse(text,users);self.refresh(entry);
    }
    fn refresh(&self,entry:&gtk::TextView) {
        self.updating.set(true);entry.set_text(&self.value.borrow().text);self.updating.set(false);self.highlight(entry);
    }
    fn highlight(&self,entry:&gtk::TextView) {
        let buffer=entry.buffer();let tag=buffer.tag_table().lookup("papo-composer-mention").unwrap_or_else(||{
            let tag=gtk::TextTag::builder().name("papo-composer-mention").background("#3584e4").foreground("#ffffff").build();buffer.tag_table().add(&tag);tag
        });
        buffer.remove_tag(&tag,&buffer.start_iter(),&buffer.end_iter());
        let value=self.value.borrow();
        for span in &value.spans {let start=value.text[..span.start].chars().count() as i32;let end=value.text[..span.end].chars().count() as i32;buffer.apply_tag(&tag,&buffer.iter_at_offset(start),&buffer.iter_at_offset(end));}
    }
    pub fn mention(&self,entry:&gtk::TextView,id:Option<Uuid>,users:&HashMap<Uuid,UserSummary>)->i32 {
        let mut value=self.value.borrow_mut();
        let(start,end)=mentions::completion(&value.text,entry.position()).map(|(s,e,_)|(s,e)).unwrap_or_else(||{
            let end=value.text.len();let gap=if end==0||value.text.ends_with(char::is_whitespace){""}else{" "};value.replace(end,end,gap);(value.text.len(),value.text.len())
        });
        let label=id.and_then(|id|users.get(&id).map(|u|format!("@{}",u.display_name()))).unwrap_or_else(||"@everyone".into());
        value.replace(start,end,&format!("{label} "));value.spans.push(Span{start,end:start+label.len(),id});value.spans.sort_by_key(|s|s.start);
        let caret=value.text[..start+label.len()+1].chars().count() as i32;drop(value);self.refresh(entry);caret
    }
    pub fn emoji(&self,entry:&gtk::TextView,value:&str,replace:bool)->i32 {
        let mut text=self.value.borrow_mut();let end=byte(&text.text,entry.position());
        let(start,mut end)=if replace{emoji::completion(&text.text,entry.position()).map_or((end,end),|(s,e,_)|(s,e))}else{(end,end)};
        if replace&&text.text[end..].starts_with(':'){end+=1;}
        text.replace(start,end,&format!("{value} "));let caret=text.text[..start+value.len()+1].chars().count() as i32;drop(text);self.refresh(entry);caret
    }
}

#[cfg(test)]pub(crate) fn field(root:&gtk::Widget)->gtk::TextView{input::find(root)}
