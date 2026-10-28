//! EN-US: Desktop editing policy and optional real key delivery use only a disposable local fixture.
//! PT-BR: A política de edição e as teclas reais opcionais usam só uma fixture local descartável.
use super::*;
use actions::tests::{mock,pump,until,descendants};
use adw::prelude::*;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use gtk::gdk::{Key,ModifierType as M};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let peer=Uuid::new_v4();let channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"desktop-test","created_at":chrono::Utc::now()})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel),user_id:Some(user),api:Some(api),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    chat.emit(ChatMsg::SetUsers(vec![serde_json::from_value(serde_json::json!({"id":peer,"username":"desktoppeer","nickname":"Desktop Peer","created_at":chrono::Utc::now()})).unwrap()]));
    let content=gtk::Box::new(gtk::Orientation::Vertical,8);let other=gtk::Entry::new();other.set_placeholder_text(Some("Other editable"));content.append(&other);content.append(chat.widget());chat.widget().set_vexpand(true);
    let window=adw::Window::builder().title("Papo desktop input test").default_width(780).default_height(620).content(&content).build();window.present();
    let field=find(chat.widget().upcast_ref());until(context,||field.is_mapped()&&field.width()>0);
    #[cfg(windows)]assert!(field.im_module().is_none(),"Windows IME must remain selected by GTK");
    let controllers=field.observe_controllers();let keys=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    let press=|key,mods|keys.emit_by_name::<bool>("key-pressed",&[&key,&0u32,&mods]);
    let window_keys=window.observe_controllers();let window_keys:Vec<_>=(0..window_keys.n_items()).filter_map(|i|window_keys.item(i).and_downcast::<gtk::EventControllerKey>()).collect();
    field.set_text("first\nsecond á é ç");field.set_position(2);field.grab_focus();pump(context);
    for key in [Key::End,Key::Page_Up,Key::Page_Down]{assert!(window_keys.iter().all(|c|!c.emit_by_name::<bool>("key-pressed",&[&key,&0u32,&M::empty()])),"Native editor navigation must not jump chat history");}
    let native=std::env::var("PAPO_NATIVE_INPUT_TEST").as_deref()==Ok("1");
    if native{super::super::multiline::keys_for("Papo desktop input test",&["End"]);}else{field.emit_by_name::<()>("move-cursor",&[&gtk::MovementStep::DisplayLineEnds,&1i32,&false]);}
    until(context,||field.position()==5);assert_eq!(field.text(),"first\nsecond á é ç");
    field.set_position(-1);
    if native{super::super::multiline::keys_for("Papo desktop input test",&["Shift_L+Return"]);}else{field.emit_by_name::<()>("insert-at-cursor",&[&"\n"]);}
    until(context,||field.text().ends_with('\n'));
    field.emit_by_name::<()>("preedit-changed",&[&"候補"]);
    for key in [Key::Return,Key::Tab,Key::Escape]{assert!(!press(key,M::empty()),"IME candidate keys must reach GTK");}
    field.emit_by_name::<()>("preedit-changed",&[&""]);
    field.set_text("@desktop");field.set_position(-1);until(context,||chat.model().mention_visible);
    for mods in [M::CONTROL_MASK,M::ALT_MASK,M::META_MASK,M::SHIFT_MASK]{
        for key in [Key::Return,Key::Tab]{assert!(!press(key,mods));}
    }
    pump(context);assert_eq!(field.text(),"@desktop");
    if native{super::super::multiline::keys_for("Papo desktop input test",&["Tab"]);}else{assert!(press(Key::Tab,M::empty()));}
    until(context,||field.text()=="@Desktop Peer ");assert!(chat.model().draft.text.contains(&peer.to_string()));
    // EN-US: Native TextView focus navigation must let keyboard users leave the multiline editor.
    // PT-BR: A navegação nativa de foco deve permitir sair do editor multilinha usando o teclado.
    if native{super::super::multiline::keys_for("Papo desktop input test",&["Control_L+Tab"]);}else{field.emit_by_name::<()>("move-focus",&[&gtk::DirectionType::TabForward]);}
    until(context,||!field.has_focus());
    other.grab_focus();pump(context);
    assert!(window_keys.iter().all(|c|!c.emit_by_name::<bool>("key-pressed",&[&Key::a,&0u32,&M::empty()])));
    assert!(other.has_focus()||gtk::prelude::RootExt::focus(&window).is_some_and(|w|w.is_ancestor(&other)),"Typing in another editable must not move focus to the composer");
    assert!(!backend.lock().unwrap().requests.iter().any(|(m,p,_)|m=="POST"&&p=="/messages"));
    assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.widget_name()=="message-composer"));
    println!("Desktop input policy passed; backend={}, native_keys={}, scale_factor={}",field.display().type_().name(),native,window.scale_factor());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
