//! Real keyboard routing, compact editing and transient download feedback.
use super::*;
use actions::tests::{mock,descendants,find_button,pump,until};
use adw::prelude::*;

fn key(widget:&gtk::Widget)->gtk::EventControllerKey{
    let controllers=widget.observe_controllers();(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap()
}
fn press(widget:&gtk::Widget,value:gtk::gdk::Key)->bool{key(widget).emit_by_name("key-pressed",&[&value,&0u32,&gtk::gdk::ModifierType::empty()])}
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let own="12345678-1234-4234-8234-123456789abc".parse().unwrap();let channel_id="12345678-1234-4234-8234-123456789abd".parse().unwrap();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"Shortcuts","created_at":chrono::Utc::now()})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(channel(channel_id)),user_id:Some(own),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::direct()});
    let users:Vec<UserSummary>=(0..3).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"username":format!("person{i}"),"created_at":chrono::Utc::now()})).unwrap()).collect();
    chat.emit(ChatMsg::SetUsers(users.clone()));
    let container=gtk::Box::new(gtk::Orientation::Vertical,0);let navigation=gtk::Button::with_label("Channel navigation");let search=gtk::Entry::new();container.append(&navigation);container.append(&search);container.append(chat.widget());chat.widget().set_vexpand(true);
    let window=adw::Window::builder().title("Papo report9 input").default_width(850).default_height(680).content(&container).build();window.present();performance::settle(context);
    let entry=composer::field(chat.widget().upcast_ref());
    for (prefix,panel_name) in [("@person","mention-suggestions"),(":smi","emoji-suggestions")]{
        entry.grab_focus();entry.set_text(prefix);entry.set_position(-1);pump(context);
        let panel=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==panel_name).unwrap();until(context,||panel.is_mapped());
        let buttons:Vec<_>=descendants(&panel).into_iter().filter_map(|w|w.downcast::<gtk::Button>().ok()).collect();assert!(buttons.len()>=2);
        assert!(press(entry.upcast_ref(),gtk::gdk::Key::Down));pump(context);assert!(buttons[0].has_focus());
        assert!(press(&panel,gtk::gdk::Key::Down));pump(context);assert!(buttons[1].has_focus());
        let selected=if prefix.starts_with('@'){format!("@{} ",users[1].display_name())}else{String::new()};
        assert!(press(&panel,gtk::gdk::Key::Tab));pump(context);
        assert!(entry.has_focus()&&entry.selection_bounds().is_none());
        if !selected.is_empty(){assert_eq!(entry.text(),selected);}else{assert_ne!(entry.text(),prefix);assert!(!entry.text().starts_with(':'));}
        let before=entry.text().to_string();entry.emit_by_name::<()>("insert-at-cursor",&[&"after"]);pump(context);assert_eq!(entry.text(),format!("{before}after"));
    }
    assert!(!backend.lock().unwrap().requests.iter().any(|(m,p,_)|m=="POST"&&p=="/messages"));
    if std::env::var_os("PAPO_NATIVE_INPUT_TEST").is_some(){
        assert!(std::process::Command::new("setxkbmap").args(["-layout","br","-variant",""]).status().unwrap().success());
        entry.set_text("");navigation.grab_focus();pump(context);
        multiline::keys_for("Papo report9 input",&["a"]);until(context,||entry.text()=="a");assert!(entry.has_focus());
        chat.emit(ChatMsg::SetChannel(channel(Uuid::new_v4())));pump(context);navigation.grab_focus();pump(context);
        multiline::keys_for("Papo report9 input",&["dead_acute"]);performance::settle(context);multiline::keys_for("Papo report9 input",&["e"]);until(context,||entry.text()=="é");
        // Other editable fields and shortcuts keep their own keyboard events.
        search.grab_focus();multiline::keys_for("Papo report9 input",&["b"]);until(context,||search.text()=="b");assert_eq!(entry.text(),"é");
        entry.set_text("");entry.grab_focus();multiline::keys_for("Papo report9 input",&["Shift_L+Return"]);until(context,||entry.text()=="\n");
        chat.emit(ChatMsg::SetChannel(channel(channel_id)));pump(context);
    }
    // The toolbar and the single-line input have a common vertical centre.
    entry.set_text("one line");performance::settle(context);let mention=find_button(chat.widget().upcast_ref(),"Mencionar membro (digite @ para filtrar)");
    let a=entry.compute_bounds(chat.widget()).unwrap();let b=mention.compute_bounds(chat.widget()).unwrap();assert!((a.y()+a.height()/2.0-b.y()-b.height()/2.0).abs()<=2.0,"composer and toolbar centres differ: {a:?} / {b:?}");
    let stored=serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","title":"Keep this card","thumbnail":{"mime_type":"image/png"},"created_at":"2026-10-03T12:00:00Z"});
    backend.lock().unwrap().message["embeds"]=serde_json::json!([stored.clone()]);
    let message:Message=serde_json::from_value(serde_json::json!({"id":"12345678-1234-4234-8234-123456789abe","channel_id":channel_id,"author_id":own,"content":"before","created_at":"2026-10-03T12:00:00Z","embeds":[stored.clone()]})).unwrap();
    chat.emit(ChatMsg::AddMessage(message.clone()));pump(context);chat.emit(ChatMsg::Action(ActionMsg::OpenEdit(message.clone())));pump(context);
    let editor=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Editar mensagem"))).unwrap();
    assert!(!descendants(editor.upcast_ref()).iter().any(|w|w.is::<gtk::Expander>()||w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Adicionar embed"))),"editing must offer text controls only");
    performance::settle(context);assert!(editor.height()<240,"a short edit must fit its contents: {}",editor.height());
    let body=editor.child().unwrap();let save=find_button(editor.upcast_ref(),"Salvar edição");let bounds=save.compute_bounds(&body).unwrap();
    assert!(body.height() as f32-bounds.y()-bounds.height()<=20.0,"the edit dialog must not leave unused space below its buttons");
    let text=descendants(editor.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();text.buffer().set_text("after");
    assert!(!key(text.upcast_ref()).emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Return,&0u32,&gtk::gdk::ModifierType::SHIFT_MASK]));
    assert!(press(text.upcast_ref(),gtk::gdk::Key::Return));until(context,||backend.lock().unwrap().requests.iter().any(|(m,p,_)|m=="PUT"&&p.ends_with("/12345678-1234-4234-8234-123456789abe"))&&find_button(editor.upcast_ref(),"Salvar edição").is_sensitive()); // first mock PUT fails, retain edit
    assert!(editor.is_visible());assert!(press(text.upcast_ref(),gtk::gdk::Key::Return));until(context,||!editor.is_visible());
    let edits:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p.ends_with("/12345678-1234-4234-8234-123456789abe")).cloned().collect();assert_eq!(edits.len(),2);assert!(edits.iter().all(|(_,_,body)|body["content"]=="after"&&body.get("embeds").is_none()));
    assert_eq!(chat.model().history.messages.iter().find(|m|m.id==message.id).unwrap().embeds.as_ref().unwrap()[0].title.as_deref(),Some("Keep this card"));
    // Expiry clears only the corresponding notice, never a newer notice/error.
    let epoch=chat.model().transfers.epoch;
    chat.emit(ChatMsg::Transfer(TransferMsg::Saved{epoch,message:message.id,result:Ok(())}));pump(context);let old=chat.model().transfers.notice.as_ref().unwrap().0;
    chat.emit(ChatMsg::Transfer(TransferMsg::Saved{epoch,message:message.id,result:Ok(())}));pump(context);let latest=chat.model().transfers.notice.as_ref().unwrap().0;
    chat.emit(ChatMsg::Transfer(TransferMsg::DismissNotice{epoch,token:old}));pump(context);assert_eq!(chat.model().transfers.notice.as_ref().unwrap().0,latest);
    chat.emit(ChatMsg::OperationError("Preserve this error".into()));until(context,||chat.model().transfers.notice.is_none());assert_eq!(chat.model().draft.error.as_deref(),Some("Preserve this error"));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
