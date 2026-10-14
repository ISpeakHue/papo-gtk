//! Mapped regressions for native Markdown, pin authors and incremental emoji tabs.
use super::*;
use adw::prelude::*;
use actions::tests::{descendants,pump,until};
use performance::settle;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let peer=Uuid::new_v4();let id=Uuid::new_v4();let now=chrono::Utc::now();
    let member:UserSummary=serde_json::from_value(serde_json::json!({"id":peer,"username":"alice","nickname":"Álícia","created_at":now})).unwrap();
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Native Markdown","created_at":now})).unwrap();
    let message:Message=serde_json::from_value(serde_json::json!({"id":id,"channel_id":channel,"author_id":peer,"content":"**Pinned** message","created_at":now})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![message.clone()],user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});chat.emit(ChatMsg::SetUsers(vec![member.clone()]));pump(context);
    let emoji=Uuid::new_v4();let animation=crate::media::animation::PreparedAnimation::decode(&crate::media::animation::fixture()).unwrap().paintable();
    let avatar=crate::media::PreparedImage{width:16,height:16,pixels:vec![200;16*16*4]}.texture();chat.emit(ChatMsg::SetAvatars(HashMap::from([(peer,avatar.clone())])));pump(context);
    {let mut state=chat.state().get_mut();let actions=&mut state.model.actions;actions.emoji_names.insert("dance".into(),emoji);actions.animations.insert(emoji,animation.clone());actions.pinned=vec![message];actions.pins_ready=true;}
    let opened=std::rc::Rc::new(std::cell::Cell::new(None));let captured=opened.clone();
    let source=format!("# Título\n\n**forte _é_** __sublinhado__ ~~riscado~~\n\n:dance: @mention(<@{peer}>) [seguro](https://example.test)\n\n`:dance: @mention(<@{peer}>)`\n\n||oculto **também** `código` :dance:||\n\n[unsafe](javascript:alert(1)) <b>literal</b>");
    let text={let m=chat.model();text::with_profiles(&source,&m.users_map,&m.actions,move|id|captured.set(Some(id)))};
    let window=adw::Window::builder().default_width(850).default_height(680).content(&text).build();window.present();settle(context);
    assert!(text.tooltip_text().is_none(),"hovering cannot disclose concealed spoiler text");
    let buffer=text.buffer();let rendered=buffer.slice(&buffer.start_iter(),&buffer.end_iter(),true).to_string();
    assert!(rendered.contains("forte é sublinhado riscado")&&rendered.contains("@Álícia"));assert!(rendered.contains(&format!(":dance: @mention(<@{peer}>)")),"code must preserve literal tokens");assert!(rendered.contains("<b>literal</b>"));
    let offset=|value:&str|rendered[..rendered.find(value).unwrap()].chars().count() as i32;
    assert!(buffer.iter_at_offset(offset("Título")).tags().iter().any(|t|t.scale()>1.5));
    assert!(buffer.iter_at_offset(offset("é")).tags().iter().any(|t|t.weight()==700));assert!(buffer.iter_at_offset(offset("é")).tags().iter().any(|t|t.style()==pango::Style::Italic));
    assert!(buffer.iter_at_offset(offset("sublinhado")).tags().iter().any(|t|t.underline()==pango::Underline::Single));assert!(buffer.iter_at_offset(offset("riscado")).tags().iter().any(|t|t.is_strikethrough()));
    let paints=||{let mut images=Vec::new();let mut iter=buffer.start_iter();loop{if let Some(image)=iter.paintable(){images.push(image);}if !iter.forward_char(){break;}}images};
    let images=paints();assert_eq!(images.len(),2);assert_eq!(images[0],animation.clone().upcast::<gtk::gdk::Paintable>());assert_ne!(images[1],images[0],"concealed emoji must use a mask rather than expose its animation");
    let controllers=text.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>()).unwrap();
    let tap=|at:i32|{let rect=text.iter_location(&buffer.iter_at_offset(at));let(x,y)=text.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x()+1,rect.y()+1);let(x,y)=(f64::from(x),f64::from(y));click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);};
    tap(offset("@Álícia"));assert_eq!(opened.get(),Some(peer));
    tap(offset("oculto"));pump(context);assert_eq!(paints()[1],animation.clone().upcast::<gtk::gdk::Paintable>());
    for word in ["oculto","também","código"]{assert!(!buffer.iter_at_offset(offset(word)).tags().iter().any(|t|t.is_foreground_set()&&t.is_background_set()),"revealing one spoiler reveals every formatted run in it");}
    window.set_content(None::<&gtk::Widget>);window.close();pump(context);
    chat.emit(ChatMsg::Action(ActionMsg::OpenPins));pump(context);let pins=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Mensagens fixadas"))).unwrap();
    let all=descendants(pins.upcast_ref());assert!(all.iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Álícia")));assert!(all.iter().any(|w|w.downcast_ref::<adw::Avatar>().is_some_and(|a|a.custom_image()==Some(avatar.clone().upcast()))));
    assert!(!all.iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Atualizar mensagens fixadas"))));
    let mut renamed=member;renamed.nickname=Some("Novo apelido".into());chat.emit(ChatMsg::SetUsers(vec![renamed]));pump(context);assert!(descendants(pins.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Novo apelido")));pins.close();
    let root=gtk::Box::new(gtk::Orientation::Vertical,8);let builtin:Vec<_>=(0..400).map(|i|emoji::Choice{name:format!("builtin_{i:03}"),text:"😀".into(),id:None}).collect();
    let custom:Vec<crate::models::Emoji>=(0..200).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":format!("server_{i:03}"),"format":"PNG","created_at":now})).unwrap()).collect();
    let picker=emoji::Picker::new(&root,&builtin,|_|{});picker.update("Test server",&custom,&HashMap::new(),&HashMap::new());
    let window=adw::Window::builder().default_width(420).default_height(420).content(&root).build();window.present();settle(context);
    let all=descendants(root.upcast_ref());let tabs=all.iter().find_map(|w|w.downcast_ref::<gtk::Stack>()).unwrap().clone();let search=all.iter().find_map(|w|w.downcast_ref::<gtk::SearchEntry>()).unwrap().clone();
    assert_eq!(tabs.visible_child_name().as_deref(),Some("unicode"));let active=tabs.visible_child().unwrap();let all=descendants(&active);let grid=all.iter().find_map(|w|w.downcast_ref::<gtk::FlowBox>()).unwrap().clone();let scroll=all.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();
    assert_eq!(grid.observe_children().n_items(),96);let first=grid.first_child().unwrap();scroll.vadjustment().set_value(scroll.vadjustment().upper()-scroll.vadjustment().page_size());until(context,||grid.observe_children().n_items()>=192);assert_eq!(grid.first_child().unwrap(),first,"loading another page preserves existing emoji cells");
    let value=scroll.vadjustment().value();let count=grid.observe_children().n_items();picker.show_error("Emoji request failed");assert!(descendants(root.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.is_visible()&&l.text()=="Emoji request failed")));picker.update("Test server",&custom,&HashMap::new(),&HashMap::new());pump(context);assert_eq!(grid.first_child().unwrap(),first);assert_eq!(grid.observe_children().n_items(),count);assert_eq!(scroll.vadjustment().value(),value);assert!(!descendants(root.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.is_visible()&&l.text()=="Emoji request failed")));
    tabs.set_visible_child_name("server");search.set_text("server_001");until(context,||descendants(&tabs.visible_child().unwrap()).iter().filter(|w|w.has_css_class("papo-emoji-cell")).count()==1);assert_eq!(tabs.visible_child_name().as_deref(),Some("server"));
    assert!(!descendants(root.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Mais emojis"))));
    window.set_content(None::<&gtk::Widget>);window.close();drop(picker);drop(chat);pump(context);
}
