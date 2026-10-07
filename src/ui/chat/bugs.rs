//! Mapped-widget regressions for the reported scroll, reply and context-menu bugs.
use super::*;
use actions::tests::{descendants,find_button,pump};
use std::{cell::RefCell,rc::Rc};
use adw::prelude::*;
fn settle(context:&gtk::glib::MainContext){for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}}
fn row(chat:&Controller<ChatModel>,id:Uuid)->gtk::Widget{descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("message-{id}")).unwrap()}

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let start=chrono::DateTime::parse_from_rfc3339("2026-10-07T12:00:00Z").unwrap();
    let mut messages:Vec<Message>=(0..70).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Mensagem {i}: teste de rolagem e respostas."),"created_at":start+chrono::Duration::minutes(i)})).unwrap()).collect();
    let mentioned=Uuid::new_v4();messages[67].content=Some(format!("Olá @mention(<@{mentioned}>)"));messages[68].reply_to=Some(messages[67].id);let mut edited=messages[67].clone();
    let ids:Vec<_>=messages.iter().map(|m|m.id).collect();let selected:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"regressões","type":"text","created_at":start})).unwrap();
    let outputs=Rc::new(RefCell::new(Vec::new()));let captured=outputs.clone();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(selected),messages,user_id:Some(user),..Default::default()}).connect_receiver(move |_,out|captured.borrow_mut().push(out));
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let member:UserSummary=serde_json::from_value(serde_json::json!({"id":user,"username":"Alice","created_at":start,"roles":[{"id":Uuid::new_v4(),"name":"Member","color":"#aabbcc"}]})).unwrap();let mut other:UserSummary=serde_json::from_value(serde_json::json!({"id":mentioned,"username":"Bob","created_at":start})).unwrap();chat.emit(ChatMsg::SetUsers(vec![member.clone(),other.clone()]));pump(context);
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();chat.emit(ChatMsg::Latest);settle(context);
    let scroll=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok()).unwrap();let list=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ListBox>().ok().filter(|l|l.has_css_class("papo-chat-history"))).unwrap();let adj=scroll.vadjustment();
    assert!(adj.upper()>adj.page_size());assert!(chat.model().viewport.following());
    let history_request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:history_request,cursor:None});
    chat.emit(ChatMsg::HistoryLoaded{request_id:history_request,append:true,result:Ok(MessageListResponse{channel_id:channel,has_more:true,messages:vec![]})});settle(context);
    outputs.borrow_mut().clear();adj.set_value(0.0);settle(context);
    assert_eq!(outputs.borrow().iter().filter(|o|matches!(o,ChatOutput::LoadMoreMessages{..})).count(),1,"scrolling to the top must request one older page");
    chat.emit(ChatMsg::AutoOlder);chat.emit(ChatMsg::AutoOlder);settle(context);assert_eq!(outputs.borrow().iter().filter(|o|matches!(o,ChatOutput::LoadMoreMessages{..})).count(),1,"queued scroll events must not duplicate history requests");
    let failed_request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:failed_request,cursor:chat.model().history.messages.first().map(MessageCursor::from)});chat.emit(ChatMsg::HistoryLoaded{request_id:failed_request,append:true,result:Err("offline".into())});settle(context);
    chat.emit(ChatMsg::AutoOlder);settle(context);assert_eq!(outputs.borrow().iter().filter(|o|matches!(o,ChatOutput::LoadMoreMessages{..})).count(),1,"failed pages require an explicit retry");
    find_button(chat.widget().upcast_ref(),"Tentar novamente").emit_clicked();settle(context);assert_eq!(outputs.borrow().iter().filter(|o|matches!(o,ChatOutput::LoadMoreMessages{..})).count(),2);
    let end=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:end,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:end,append:true,result:Ok(MessageListResponse{channel_id:channel,has_more:false,messages:vec![]})});settle(context);chat.emit(ChatMsg::AutoOlder);settle(context);assert_eq!(outputs.borrow().iter().filter(|o|matches!(o,ChatOutput::LoadMoreMessages{..})).count(),2);
    // Test actual wheel-controller delivery; reduced-motion settings may finish immediately.
    adj.set_value(800.0);settle(context);let before_wheel=adj.value();let controllers=scroll.observe_controllers();let wheel=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerScroll>()).unwrap();
    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&1.0f64]);pump(context);assert!(adj.value()<before_wheel+adj.page_size().powf(2.0/3.0)+1.0);settle(context);assert!(adj.value()>before_wheel,"the wheel must advance the timeline");
    let top=row(&chat,ids[20]);adj.set_value(f64::from(top.compute_bounds(&list).unwrap().y())+4.0);settle(context);let before=adj.value();assert!(chat.model().viewing_old);
    for message in [ChatMsg::ClearStaleTyping,ChatMsg::UserTyping{user_id:user,is_typing:true},ChatMsg::NotificationCount{count:2,more:false},ChatMsg::InputChanged("Rascunho".into())]{chat.emit(message);}settle(context);
    assert_eq!(row(&chat,ids[20]),top,"ordinary UI updates must preserve message widgets");assert!((adj.value()-before).abs()<1.0,"typing must not scroll the timeline");
    assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.has_css_class("papo-role-name")));
    let target_row=row(&chat,ids[5]);chat.emit(ChatMsg::Action(ActionMsg::Navigate(ids[5])));settle(context);let target=row(&chat,ids[5]).compute_bounds(&list).unwrap();assert!(f64::from(target.y())>=adj.value()&&f64::from(target.y())<adj.value()+adj.page_size());
    assert_eq!(row(&chat,ids[5]),target_row,"highlight changes must preserve the message widget");assert!(row(&chat,ids[5]).has_css_class("papo-message-highlight"));let old_generation=chat.model().highlight_generation;
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(ids[6])));settle(context);chat.emit(ChatMsg::ExpireHighlight{epoch:chat.model().actions.epoch,id:ids[5],generation:old_generation});pump(context);assert!(row(&chat,ids[6]).has_css_class("papo-message-highlight"),"an older timeout must not clear a newer highlight");
    let highlighted=row(&chat,ids[6]);actions::tests::until(context,||!row(&chat,ids[6]).has_css_class("papo-message-highlight"));assert_eq!(row(&chat,ids[6]),highlighted,"fading out must not remove and recreate the row");
    adj.set_value(f64::from(row(&chat,ids[35]).compute_bounds(&list).unwrap().y())+3.0);settle(context);let value=adj.value();chat.emit(ChatMsg::Action(ActionMsg::PinEvent{id:ids[6],pinned:false}));settle(context);assert!((adj.value()-value).abs()<1.0,"a highlighted message must not repeatedly pull the viewport back");
    let visible=row(&chat,ids[35]);let offset=f64::from(visible.compute_bounds(&list).unwrap().y())-adj.value();
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":"Chegou uma nova mensagem","created_at":start+chrono::Duration::hours(3)})).unwrap()));settle(context);
    assert_eq!(row(&chat,ids[35]),visible,"live arrivals must preserve existing widgets and in-progress clicks");
    assert!((f64::from(row(&chat,ids[35]).compute_bounds(&list).unwrap().y())-adj.value()-offset).abs()<1.0,"live arrivals must preserve the reading position");assert!(chat.model().viewing_old);
    crate::ui::main_window::layout::tests::preview(&window,"history-banner",context);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:channel,has_more:false,messages:vec![serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":"Mensagem anterior","created_at":start-chrono::Duration::days(1)})).unwrap()]})});settle(context);
    assert_eq!(row(&chat,ids[35]),visible,"pagination must preserve unchanged message widgets");
    assert!((f64::from(row(&chat,ids[35]).compute_bounds(&list).unwrap().y())-adj.value()-offset).abs()<1.0,"pagination must retain the same visible message and offset: before offset={}, now={}, adjustment={}",offset,f64::from(row(&chat,ids[35]).compute_bounds(&list).unwrap().y())-adj.value(),adj.value());
    let keyboard_row=row(&chat,ids[60]);keyboard_row.grab_focus();settle(context);assert!((f64::from(visible.compute_bounds(&list).unwrap().y())-adj.value()-offset).abs()<1.0,"programmatic focus must not scroll the timeline");
    let keys=scroll.observe_controllers();let keys=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    let _:bool=keys.emit_by_name("key-pressed",&[&gtk::gdk::Key::Down,&0u32,&gtk::gdk::ModifierType::empty()]);settle(context);
    let bounds=keyboard_row.compute_bounds(&list).unwrap();assert!(f64::from(bounds.y()+bounds.height())<=adj.value()+adj.page_size()+1.0,"keyboard navigation must reveal the focused message");
    other.nickname=Some("Robert".into());chat.emit(ChatMsg::SetUsers(vec![member,other]));settle(context);
    assert_eq!(row(&chat,ids[35]),visible,"renaming a mentioned member must preserve unrelated rows");
    assert!(descendants(&row(&chat,ids[67])).iter().filter_map(|w|w.downcast_ref::<gtk::TextView>()).any(|v|v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false)=="Olá @Robert"));
    edited.content=Some("Texto editado da mensagem respondida".into());chat.emit(ChatMsg::ApplyChange(Change::Upsert(edited)));settle(context);
    assert!(descendants(find_button(&row(&chat,ids[68]),"Ir para a mensagem respondida").upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="Texto editado da mensagem respondida"));
    chat.emit(ChatMsg::ContextAt{id:ids[36],x:90.0,y:20.0});settle(context);let message=row(&chat,ids[36]);
    let menu=descendants(&message).into_iter().find_map(|w|w.downcast::<gtk::MenuButton>().ok()).unwrap();let (pointing,rectangle)=menu.popover().unwrap().pointing_to();assert!(pointing);let point=message.compute_point(&menu,&gtk::graphene::Point::new(90.0,20.0)).unwrap();assert!((rectangle.x() as f32-point.x()).abs()<2.0&&(rectangle.y() as f32-point.y()).abs()<2.0,"the native menu must anchor at the clicked point");
    let copy=find_button(&message,"Copiar texto");assert!(copy.is_visible());copy.emit_clicked();pump(context);let clipboard=gtk::gdk::Display::default().unwrap().clipboard();assert_eq!(context.block_on(clipboard.read_text_future()).unwrap().unwrap().as_str(),"Mensagem 36: teste de rolagem e respostas.");
    // Use the actual button signal, then timer updates, to ensure the click survives.
    let reply=find_button(&message,"Responder");reply.emit_clicked();chat.emit(ChatMsg::ClearStaleTyping);settle(context);assert_eq!(chat.model().draft.reply.as_ref().map(|m|m.id),Some(ids[36]));assert_eq!(row(&chat,ids[36]),message);
    find_button(chat.widget().upcast_ref(),"Cancelar resposta").emit_clicked();pump(context);assert!(chat.model().draft.reply.is_none());
    find_button(chat.widget().upcast_ref(),"Avançar para mensagens recentes").emit_clicked();settle(context);assert!(chat.model().viewport.following());assert!(!chat.model().viewing_old);
    chat.emit(ChatMsg::SetAccess{user_id:Uuid::new_v4(),access:crate::models::Access{read:true,..Default::default()}});settle(context);chat.emit(ChatMsg::ContextMenu(ids[36]));settle(context);let message=row(&chat,ids[36]);assert!(!find_button(&message,"Responder").is_sensitive());assert!(!descendants(&message).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|matches!(b.label().as_deref(),Some("Excluir"|"Editar"|"Fixar"))));
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});settle(context);
    let entry=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();entry.grab_focus();entry.set_text("Olá @Rob");entry.set_position(-1);settle(context);
    let menu=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::MenuButton>().ok().filter(|b|b.label().as_deref()==Some("@"))).unwrap();let mention=menu.popover().unwrap();assert!(mention.is_visible(),"typing @ must open suggestions");assert!(entry.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN),"suggestions must not steal typing focus");
    find_button(mention.upcast_ref(),"Robert").emit_clicked();settle(context);assert!(entry.text().contains(&format!("@mention(<@{mentioned}>)")));assert!(!mention.is_visible());
    entry.set_text("Mensagem sem filtro");entry.set_position(-1);menu.popup();settle(context);assert!(mention.is_visible());assert!(descendants(mention.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Alice")),"the @ button must list members without an active completion");
    find_button(mention.upcast_ref(),"Alice").emit_clicked();settle(context);assert!(entry.text().contains(&format!("@mention(<@{user}>)")));assert!(entry.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));
    entry.set_text("@zzzz");entry.set_position(-1);settle(context);assert!(descendants(mention.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="Nenhum membro encontrado"));chat.emit(ChatMsg::CloseMentions);settle(context);assert!(!mention.is_visible());
    // Custom emojis are paintables inside selectable, read-only native text.
    let emoji_id=Uuid::new_v4();let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(32,32).write_to(&mut bytes,image::ImageFormat::Png).unwrap();let texture=crate::media::bounded_texture_size(&bytes.into_inner(),32,32).unwrap();
    let mut actions=Actions::default();actions.emojis.push(serde_json::from_value(serde_json::json!({"id":emoji_id,"name":"gambiarra","format":"PNG","created_at":start})).unwrap());actions.textures.insert(emoji_id,texture);
    let text=text::widget("Oi :gambiarra: :unknown:",&HashMap::new(),&actions);assert!(!text.is_editable()&&!text.is_cursor_visible());assert!(text.buffer().iter_at_offset(3).paintable().is_some());assert!(text.buffer().text(&text.buffer().start_iter(),&text.buffer().end_iter(),false).contains(":unknown:"));
    window.close();
}
