use super::*;
use adw::prelude::*;
use actions::tests::{mock,pump,until,find_button,descendants};

fn paints(entry:&gtk::TextView)->usize {
    let mut iter=entry.buffer().start_iter();let mut count=0;
    loop{count+=usize::from(iter.paintable().is_some());if !iter.forward_char(){break;}}count
}
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let emoji=Uuid::new_v4();
    backend.lock().unwrap().emojis=vec![serde_json::json!({"id":emoji,"name":"dance","format":"PNG","image_blob":actions::tests::tiny_png_base64(),"created_at":"2026-10-03T00:00:00Z"})];
    let message:Message=serde_json::from_value(backend.lock().unwrap().message.clone()).unwrap();let user=message.author_id.unwrap();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":message.channel_id,"name":"Inline emoji","created_at":message.created_at})).unwrap();
    let member:UserSummary=serde_json::from_value(serde_json::json!({"id":user,"username":"alice","nickname":"Álice","created_at":message.created_at})).unwrap();
    let messages:Vec<_>=(0..100).map(|i|{let mut m=message.clone();m.id=if i==99{message.id}else{Uuid::new_v4()};m.content=Some(format!("Line {i}\n{}", "enough text to scroll ".repeat(5)));m.created_at+=chrono::Duration::seconds(i);m}).collect();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target.clone()),messages,user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});chat.emit(ChatMsg::SetUsers(vec![member]));
    let host=adw::Window::builder().title("Papo inline emoji input").default_width(820).default_height(650).content(chat.widget()).build();host.present();
    chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));
    until(context,||chat.model().actions.textures.contains_key(&emoji));performance::settle(context);
    let entry=field(chat.widget().upcast_ref());entry.grab_focus();entry.set_text(":dan");entry.set_position(-1);
    until(context,||chat.model().emoji_visible);chat.emit(ChatMsg::EmojiSelected(":dance:".into()));pump(context);
    assert_eq!(paints(&entry),1);assert_eq!(entry.text(),"\u{fffc} ");assert_eq!(entry.position(),2);assert_eq!(chat.model().draft.text,":dance: ");
    entry.set_position(0);entry.emit_by_name::<()>("insert-at-cursor",&[&"á é "]);pump(context);
    assert_eq!(chat.model().draft.text,"á é :dance: ");assert_eq!(paints(&entry),1);
    entry.set_position(-1);entry.emit_by_name::<()>("insert-at-cursor",&[&"@ali"]);pump(context);chat.emit(ChatMsg::MentionSelected(Some(user)));pump(context);
    let wire=format!("á é :dance: @mention(<@{user}>) ");assert_eq!(chat.model().draft.text,wire);assert!(entry.text().contains("@Álice"));
    // Copy/cut/paste serialize both kinds of inline objects, with no U+FFFC.
    entry.select_region(0,-1);entry.emit_by_name::<()>("copy-clipboard",&[]);let copied=context.block_on(entry.clipboard().read_text_future()).unwrap().unwrap();assert_eq!(copied,wire);
    entry.select_region(4,5);entry.emit_by_name::<()>("cut-clipboard",&[]);pump(context);assert_eq!(paints(&entry),0);assert_eq!(context.block_on(entry.clipboard().read_text_future()).unwrap().unwrap(),":dance:");
    entry.set_position(4);entry.emit_by_name::<()>("paste-clipboard",&[]);until(context,||paints(&entry)==1);assert_eq!(chat.model().draft.text,wire);
    entry.set_position(5);entry.emit_by_name::<()>("backspace",&[]);pump(context);assert_eq!(paints(&entry),0);assert!(!chat.model().draft.text.contains("dance"));
    entry.activate_action("draft.undo",None).unwrap();pump(context);assert_eq!(paints(&entry),1);assert_eq!(chat.model().draft.text,wire);entry.activate_action("draft.redo",None).unwrap();pump(context);assert_eq!(paints(&entry),0);
    if std::env::var_os("PAPO_NATIVE_INPUT_TEST").is_some(){
        entry.grab_focus();multiline::keys_for("Papo inline emoji input",&["Control_L+z"]);until(context,||paints(&entry)==1);assert_eq!(chat.model().draft.text,wire);
        multiline::keys_for("Papo inline emoji input",&["Control_L+Shift_L+z"]);until(context,||paints(&entry)==0);
    }
    // Fully typed codes and pasted text promote to paintables after IM commits.
    entry.set_text("é :dance: :unknown: `:dance:` https://example.test/:dance:");entry.set_position(-1);until(context,||paints(&entry)==1);
    assert_eq!(chat.model().draft.text,"é :dance: :unknown: `:dance:` https://example.test/:dance:");
    let draft=chat.model().draft.text.clone();let mut other=target.clone();other.id=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(other));pump(context);chat.emit(ChatMsg::SetChannel(target));pump(context);
    assert_eq!(paints(&entry),1);assert_eq!(chat.model().draft.text,draft);
    // Editor completion is independent of the chat draft and cannot send/save.
    let mut editing=message.clone();editing.content=Some(format!("Hi @mention(<@{user}>) :dance:"));
    chat.emit(ChatMsg::Action(ActionMsg::OpenEdit(editing)));pump(context);
    let editor=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.title().as_deref()==Some("Editar mensagem")).unwrap();
    let text=descendants(editor.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();assert_eq!(paints(&text),1);
    text.set_position(-1);text.emit_by_name::<()>("insert-at-cursor",&[&" :dan"]);
    let panel=descendants(editor.upcast_ref()).into_iter().find(|w|w.widget_name()=="edit-emoji-suggestions").unwrap();until(context,||panel.is_visible());
    let controllers=text.observe_controllers();let key=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    assert!(key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Tab,&0u32,&gtk::gdk::ModifierType::empty()]));pump(context);
    assert_eq!(paints(&text),2);assert!(!text.buffer().has_selection());assert_eq!(chat.model().draft.text,draft);
    assert!(!backend.lock().unwrap().requests.iter().any(|(m,_,_)|m=="PUT"));
    find_button(editor.upcast_ref(),"Inserir emoji").emit_clicked();pump(context);
    let picker=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.title().as_deref()==Some("Emojis")&&w.is_visible()).unwrap();picker.close();pump(context);
    find_button(editor.upcast_ref(),"Inserir emoji").emit_clicked();pump(context);
    assert!(gtk::Window::list_toplevels().iter().any(|w|w.downcast_ref::<gtk::Window>().is_some_and(|w|w.title().as_deref()==Some("Emojis")&&w.is_visible())));
    let save=find_button(editor.upcast_ref(),"Salvar edição");save.emit_clicked();until(context,||save.is_sensitive());save.emit_clicked();until(context,||!editor.is_visible());
    let expected=format!("Hi @mention(<@{user}>) :dance: :dance: ");
    let edits:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p.ends_with(&message.id.to_string())).cloned().collect();assert_eq!(edits.len(),2);assert!(edits.iter().all(|(_,_,body)|body["content"]==expected));
    // Navigation keys operate on the chat while the composer keeps its caret.
    performance::settle(context);entry.grab_focus();let caret=entry.position();
    let scroll=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok().filter(|s|s.child().is_some_and(|v|descendants(&v).iter().any(|w|w.is::<gtk::ListBox>())))).unwrap();
    let adj=scroll.vadjustment();chat.emit(ChatMsg::Latest);performance::settle(context);let bottom=adj.value();assert!(bottom>200.0);
    let controllers=host.observe_controllers();let keys:Vec<_>=(0..controllers.n_items()).filter_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>()).collect();
    let press=|key|{keys.iter().any(|c|c.emit_by_name::<bool>("key-pressed",&[&key,&0u32,&gtk::gdk::ModifierType::empty()]))};
    assert!(press(gtk::gdk::Key::Page_Up));until(context,||adj.value()<bottom-100.0);performance::settle(context);let up=adj.value();
    assert!(press(gtk::gdk::Key::Page_Down));until(context,||adj.value()>up+50.0);
    assert!(press(gtk::gdk::Key::End));until(context,||adj.upper()-adj.page_size()-adj.value()<1.0);assert_eq!(entry.position(),caret);assert_eq!(chat.model().draft.text,draft);
    let composer=chat.model().composer.clone();composer.set_emojis(&entry,&Actions::default());composer.emoji(&entry,"😄",false);
    assert!(composer.wire_text().contains(":dance:"));assert!(!composer.wire_text().contains('\u{fffc}'),"a removed emoji still has a lossless text fallback");
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);server.abort();pump(context);
}
