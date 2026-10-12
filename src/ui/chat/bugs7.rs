//! Mapped regressions for recent-row reuse, unread boundaries and Up editing.
use super::*;
use actions::tests::{descendants,pump,until};
use performance::settle;
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    let user=Uuid::new_v4();let other=Uuid::new_v4();let a=Uuid::new_v4();let b=Uuid::new_v4();let created=chrono::Utc::now();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"cached rows","created_at":created})).unwrap();
    let messages:Vec<Message>=(0..1000).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":a,"author_id":if i==999{other}else{user},"content":format!("Cached message {i}"),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a)),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();settle(context);
    let avatar=crate::media::PreparedImage{width:8,height:8,pixels:vec![255;256]}.texture();chat.emit(ChatMsg::SetAvatars(HashMap::from([(user,avatar.clone())])));pump(context);
    let before=chat.model().rendered.clone();
    chat.emit(ChatMsg::SetChannel(channel(b)));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:b,messages:vec![],has_more:false})});settle(context);
    let start=Instant::now();chat.emit(ChatMsg::SetChannel(channel(a)));pump(context);
    assert!(descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).filter(|a|a.widget_name()==format!("avatar-{user}")).all(|a|a.custom_image().as_ref()==Some(avatar.upcast_ref())),"cached avatars must be restored from the current texture cache");
    let kept=chat.model().rendered.iter().filter(|(key,(_,row))|before.get(*key).is_some_and(|(_,old)|old==row)).count();
    assert!(kept>=198,"a warm visit must reuse GTK text rows, retained={kept}");assert_eq!(chat.model().history.messages.len(),200);
    eprintln!("warm channel: reused_rows={kept}, selection_us={}",start.elapsed().as_micros());settle(context);
    // Changes while inactive must replace their rows without rebuilding all
    // the other cached rows. Deleted messages must never be mounted again.
    chat.emit(ChatMsg::SetChannel(channel(b)));pump(context);
    chat.emit(ChatMsg::CacheChange(Change::Edit(messages[995].id,"edited while inactive".into(),None)));chat.emit(ChatMsg::CacheChange(Change::Delete(messages[996].id)));chat.emit(ChatMsg::SetChannel(channel(a)));settle(context);
    assert_eq!(chat.model().history.messages.iter().find(|m|m.id==messages[995].id).unwrap().content.as_deref(),Some("edited while inactive"));assert!(!chat.model().rendered.contains_key(&format!("message-{}",messages[996].id)));
    assert_ne!(chat.model().rendered[&format!("message-{}",messages[995].id)].1,before[&format!("message-{}",messages[995].id)].1);
    let mut unread=channel(a);unread.last_read_message=Some(messages[990].id);unread.last_message=Some(serde_json::from_value(serde_json::json!({"id":messages[999].id,"created_at":messages[999].created_at})).unwrap());
    chat.emit(ChatMsg::SetChannel(unread));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:a,messages:messages[800..].to_vec(),has_more:false})});settle(context);
    let divider=chat.model().rendered[&format!("unread-start-{}",messages[991].id)].1.clone();
    assert_eq!(divider.next_sibling().unwrap().widget_name(),format!("message-{}",messages[991].id));assert!(!divider.is_focusable());
    let scroll=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok()).unwrap();let bounds=divider.compute_bounds(&scroll).unwrap();assert!(bounds.y()+bounds.height()>0.0&&bounds.y()<scroll.height() as f32,"the unread marker must be visible at the opening anchor");
    chat.emit(ChatMsg::AddMessage({let mut m=messages[999].clone();m.id=Uuid::new_v4();m.created_at+=chrono::Duration::seconds(1);m}));pump(context);assert_eq!(chat.model().rendered[&format!("unread-start-{}",messages[991].id)].1,divider);
    // Exercise the actual GTK controller, preserving drafts and rejecting a
    // queued key from the previous selection.
    let entry=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();entry.grab_focus();
    let controllers=entry.observe_controllers();let key=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    entry.set_text("keep this draft");pump(context);let stopped=key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Up,&0u32,&gtk::gdk::ModifierType::empty()]);pump(context);assert!(!stopped);assert_eq!(entry.text(),"keep this draft");assert!(!gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.is_visible()&&w.title().as_deref()==Some("Editar mensagem")));
    entry.set_text("");pump(context);assert!(key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Up,&0u32,&gtk::gdk::ModifierType::empty()]));
    until(context,||gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.is_visible()&&w.title().as_deref()==Some("Editar mensagem")));
    let edit=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.is_visible()&&w.title().as_deref()==Some("Editar mensagem"))).unwrap();
    let editor=descendants(edit.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();let buffer=editor.buffer();assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),true),"Cached message 998");edit.close();pump(context);
    let request=Uuid::new_v4();let epoch=chat.model().viewport.input_epoch();chat.state().get_mut().model.edit_lookup=Some(request);entry.set_text("draft typed while lookup was pending");pump(context);
    chat.emit(ChatMsg::LatestOwn{epoch,request,result:Ok(Some(messages[998].clone()))});pump(context);assert_eq!(entry.text(),"draft typed while lookup was pending");assert!(!gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.is_visible()&&w.title().as_deref()==Some("Editar mensagem")));
    entry.set_text("");pump(context);
    let stale=chat.model().viewport.input_epoch();chat.emit(ChatMsg::SetChannel(channel(b)));chat.emit(ChatMsg::EditLatest(stale));pump(context);assert!(!gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.is_visible()&&w.title().as_deref()==Some("Editar mensagem")));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
    transfers::exercise_gif_providers(context);

}
