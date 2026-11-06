//! Reproduce draft clearing, immediate sends, native drops, read readiness and delayed history.
use super::*;
use actions::tests::{descendants, pump, until};
use adw::prelude::*;
use std::{cell::RefCell, rc::Rc, time::Duration};

pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    let user = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let messages: Vec<Message> = (0..160).map(|i| serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "channel_id": channel, "author_id": user,
        "created_at": chrono::DateTime::from_timestamp(1700000000+i,0),
        "content": format!("Variable history row {i}\n{}", "wrapped content ".repeat(i as usize%7+1))
    })).unwrap()).collect();
    let target: Channel = serde_json::from_value(serde_json::json!({"id":channel,"name":"behavior","created_at":messages[0].created_at})).unwrap();
    let outputs = Rc::new(RefCell::new(Vec::new())); let captured = outputs.clone();
    let chat = ChatModel::builder().launch(ChatInit { user_id:Some(user), active_channel:Some(target), messages:messages[80..].to_vec(), ..Default::default() })
        .connect_receiver(move |_, out| captured.borrow_mut().push(out));
    let access = crate::models::Access::direct();
    chat.emit(ChatMsg::SetAccess {user_id:user,access:access.clone()});
    let window = adw::Window::builder().default_width(820).default_height(680).content(chat.widget()).build();
    window.present(); chat.emit(ChatMsg::Latest); performance::settle(context);
    let field = composer::field(chat.widget().upcast_ref());
    field.set_text("unfinished draft"); until(context,||chat.model().draft_channels.contains(&channel));
    field.set_text(""); until(context,||!chat.model().draft_channels.contains(&channel));

    let all = descendants(chat.widget().upcast_ref());
    let list = all.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|l|l.has_css_class("papo-chat-history"))).unwrap();
    let scroll = all.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap();
    let adj = scroll.vadjustment();
    adj.set_value(200.0); performance::settle(context); outputs.borrow_mut().clear();
    let now = Instant::now(); adj.set_value(adj.upper()-adj.page_size());
    loop {
        pump(context);
        if outputs.borrow().iter().any(|out|matches!(out,ChatOutput::ReadReady)){break;}
        let model=chat.model();
        assert!(now.elapsed()<Duration::from_secs(1),"no immediate read: following={} ready={} old={} detached={} opening={} history={} loading={} value={}/{}/{}",model.viewport.following(),model.viewport.read_ready(),model.viewing_old,model.history_detached,model.viewport.opening_flag().get(),model.history_ready,model.history.loading(),adj.value(),adj.upper(),adj.page_size());
        std::thread::sleep(Duration::from_millis(5));
    }

    field.set_text("Immediate optimistic text"); pump(context); chat.emit(ChatMsg::SendClicked);
    until(context,||chat.model().outbox.entries.len()==1);
    let pending = descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with("pending-")&&w.widget_name()!="pending-feedback").unwrap();
    assert!(descendants(&pending).iter().any(|w|w.downcast_ref::<gtk::TextView>().is_some_and(|t|t.text()=="Immediate optimistic text")));
    assert!(!descendants(&pending).iter().find(|w|w.widget_name()=="pending-feedback").unwrap().is_visible(),"fast sends show text immediately without a waiting panel");
    let id = chat.model().outbox.entries[0].id; chat.emit(ChatMsg::CancelPending(id)); pump(context);

    let path = std::env::temp_dir().join(format!("papo-drop-{} á.txt",Uuid::new_v4())); std::fs::write(&path,b"file drop fixture").unwrap();
    let drop = chat.model().transfers.file_drop.clone().unwrap();
    let files = gtk::gdk::FileList::from_array(&[gtk::gio::File::for_path(&path)]);
    let value = gtk::glib::BoxedValue(files.to_value());
    assert!(drop.emit_by_name::<bool>("drop",&[&value,&0.0f64,&0.0f64]));
    until(context,||chat.model().draft.files.len()==1&&chat.model().transfers.clipboard_request.is_none());
    assert_eq!(chat.model().draft.files[0].path,path); assert!(chat.model().outbox.entries.is_empty(),"dropping only stages a file");
    chat.emit(ChatMsg::Transfer(TransferMsg::Remove(0))); pump(context);
    chat.emit(ChatMsg::Transfer(TransferMsg::DropFiles(vec![gtk::gio::File::for_uri("https://example.invalid/private.txt")]))); pump(context);
    assert!(chat.model().draft.files.is_empty()&&chat.model().draft.error.is_some());
    let mut denied = access.clone(); denied.attachments=false;
    chat.emit(ChatMsg::SetAccess{user_id:user,access:denied}); pump(context);
    assert!(!drop.emit_by_name::<bool>("drop",&[&value,&0.0f64,&0.0f64]));
    chat.emit(ChatMsg::SetAccess{user_id:user,access}); pump(context);
    // A completion owned by the previous conversation must never attach here.
    let epoch = chat.model().transfers.epoch;
    chat.emit(ChatMsg::Transfer(TransferMsg::Selected{epoch:Uuid::new_v4(),channel,result:Ok(Vec::new())})); pump(context);
    assert_eq!(chat.model().transfers.epoch,epoch); assert!(chat.model().draft.files.is_empty());
    std::fs::remove_file(path).unwrap();

    // Reach the old top, wait for the page, and keep the same visible text and offset.
    let request = Uuid::new_v4();
    chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});
    chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:channel,has_more:true,messages:Vec::new()})});
    performance::settle(context); adj.set_value(0.0); performance::settle(context);
    let anchor = chat.model().rendered[&format!("message-{}",messages[80].id)].1.clone();
    let before = f64::from(anchor.compute_bounds(list).unwrap().y())-adj.value();
    let request = Uuid::new_v4();
    chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:Some(MessageCursor::from(&messages[80]))});
    performance::settle(context);
    let controllers=scroll.observe_controllers();
    let wheel=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerScroll>()).unwrap();
    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&-1.0f64]); pump(context);
    chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:channel,has_more:false,messages:messages[..80].to_vec()})});
    // Queue another layout update before the next frame, as media completions do.
    let member:UserSummary=serde_json::from_value(serde_json::json!({"id":user,"username":"Regression member","created_at":messages[0].created_at})).unwrap();
    chat.emit(ChatMsg::SetUsers(vec![member]));
    performance::settle(context);
    let anchor = chat.model().rendered[&format!("message-{}",messages[80].id)].1.clone();
    let after = f64::from(anchor.compute_bounds(list).unwrap().y())-adj.value();
    assert!((after-before).abs()<2.0,"prepending history moved the visible message: {before} -> {after}");
    assert!(adj.value()>100.0,"loading an older page must not jump to its new top");
    window.set_content(None::<&gtk::Widget>); window.close();
}
