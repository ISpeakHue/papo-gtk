//! Regressions for the sixth report, using actual mapped controls and layout.
use super::*;
use adw::prelude::*;
use actions::tests::{descendants,find_button,pump,until};
use performance::settle;

pub(crate) fn exercise_send_feedback(context:&gtk::glib::MainContext){
    let channel=Uuid::new_v4();let user=Uuid::new_v4();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"send feedback","created_at":chrono::Utc::now()})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();settle(context);
    let entry=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();
    let progress=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()=="send-progress").unwrap();let cancel=find_button(chat.widget().upcast_ref(),"Cancelar envio");
    entry.set_text("No flashing feedback");settle(context);let before=entry.compute_bounds(chat.widget()).unwrap();
    let request=chat.state().get_mut().model.draft.begin().unwrap();chat.emit(ChatMsg::Transfer(TransferMsg::Progress{id:request,bytes:0}));settle(context);
    assert!(chat.model().draft.is_sending());assert!(!progress.is_visible()&&!cancel.is_visible(),"a pending text send must not flash upload widgets");
    assert_eq!(entry.compute_bounds(chat.widget()).unwrap(),before,"pending text sends must not shift the composer");
    chat.emit(ChatMsg::SendFinished{request_id:request,channel_id:channel,result:Err("fixture failure".into())});pump(context);assert_eq!(entry.text(),"No flashing feedback");assert!(chat.model().draft.error.is_some());
    let request={let mut state=chat.state().get_mut();state.model.draft.text.clear();state.model.draft.embeds=vec![serde_json::from_value(serde_json::json!({"title":"custom card"})).unwrap()];state.model.draft.begin().unwrap()};
    chat.emit(ChatMsg::Transfer(TransferMsg::Progress{id:request,bytes:0}));settle(context);assert!(!progress.is_visible()&&!cancel.is_visible(),"embed-only sends must not flash upload widgets");
    chat.state().get_mut().model.draft.finish(request,Ok(()));
    let request={let mut state=chat.state().get_mut();state.model.draft.files.push(crate::api::features::UploadFile{path:"/tmp/feedback-fixture.webm".into(),name:"fixture.webm".into(),size:100});state.model.draft.begin().unwrap()};
    chat.emit(ChatMsg::Transfer(TransferMsg::Progress{id:request,bytes:50}));settle(context);assert!(progress.is_mapped()&&cancel.is_mapped(),"uploads retain progress and cancellation");
    cancel.emit_clicked();settle(context);assert!(!chat.model().draft.is_sending());assert!(!progress.is_visible()&&!cancel.is_visible());assert_eq!(chat.model().draft.files.len(),1,"cancel keeps the draft available");
    window.set_content(None::<&gtk::Widget>);window.close();
}

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let created=chrono::Utc::now();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"report six","created_at":created})).unwrap();
    let mut messages:Vec<Message>=(0..80).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":created+chrono::Duration::seconds(i),"content":format!("Message {i}")})).unwrap()).collect();
    let image=Uuid::new_v4();messages.last_mut().unwrap().attachments=Some(vec![serde_json::from_value(serde_json::json!({"id":image,"mime_type":"image/png","original_file_name":"photo.png","size_bytes":100,"created_at":created})).unwrap()]);
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target.clone()),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();settle(context);
    assert!(find_button(chat.widget().upcast_ref(),"Buscar mensagens (Ctrl+K)").is_mapped());
    let custom:crate::models::Emoji=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"SERVER_CAT","format":"PNG","created_at":created})).unwrap();
    chat.state().get_mut().model.actions.emojis=vec![custom.clone()];
    // Composer and reaction use the exact same search behavior and square cells.
    find_button(chat.widget().upcast_ref(),"Emojis (digite : para filtrar)").emit_clicked();pump(context);
    let picker=chat.model().actions.composer_picker.as_ref().unwrap().0.clone();until(context,||picker.is_mapped());
    let search=descendants(picker.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::SearchEntry>().ok()).unwrap();search.set_text("server_cat");
    until(context,||descendants(picker.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).filter(|b|b.has_css_class("papo-emoji-cell")).count()==1);
    search.grab_focus();search.set_position(3);let token=Uuid::new_v4();chat.state().get_mut().model.actions.emojis_request=Some(token);
    chat.emit(ChatMsg::Action(ActionMsg::EmojiPage{token,emojis:vec![custom],images:vec![]}));pump(context);
    let search=descendants(picker.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::SearchEntry>().ok()).unwrap();assert_eq!(search.text(),"server_cat");assert_eq!(search.position(),3);assert!(search.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN),"catalog updates preserve active search focus");
    let cell=find_button(picker.upcast_ref(),":SERVER_CAT:");assert_eq!(cell.width_request(),cell.height_request());cell.emit_clicked();pump(context);
    let entry=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();assert_eq!(entry.text(),":SERVER_CAT: ");assert!(entry.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));entry.set_text("");pump(context);
    chat.emit(ChatMsg::Action(ActionMsg::OpenPicker(messages[0].id)));pump(context);
    // Locate by title so the test doesn't depend on the Actions storage layout.
    let reaction=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Escolher reação")&&w.is_visible())).unwrap();
    let search=descendants(reaction.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::SearchEntry>().ok()).unwrap();search.set_text("rocket");
    until(context,||descendants(reaction.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).filter(|b|b.has_css_class("papo-emoji-cell")).all(|b|b.tooltip_text().is_some_and(|t|t.contains("rocket"))));assert!(find_button(reaction.upcast_ref(),"🚀").is_visible());search.set_text("SERVER_CAT");until(context,||descendants(reaction.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.tooltip_text().as_deref()==Some(":SERVER_CAT:")));reaction.close();
    // Media completion must retain the message/text widget, replacing only its media slot.
    let key=format!("message-{}",messages.last().unwrap().id);let row=chat.model().rendered[&key].1.clone();let text=descendants(row.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();
    let epoch=chat.model().transfers.epoch;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(image,token);
    chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,message:messages.last().unwrap().id,key:image,token,result:Some(crate::media::PreparedImage{width:80,height:40,pixels:vec![100;80*40*4]})}));settle(context);
    assert_eq!(chat.model().rendered[&key].1,row);assert!(text.parent().is_some(),"decoding must not unmap the message text");
    let texture=chat.model().transfers.textures[&image].clone();
    let open=find_button(row.upcast_ref(),"Abrir imagem");open.emit_clicked();pump(context);
    let viewer=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Imagem")&&w.is_visible())).unwrap();
    // Parent capture happens on press before the thumbnail's clicked signal.
    let controllers=window.observe_controllers();for i in 0..controllers.n_items(){if let Some(click)=controllers.item(i).and_downcast::<gtk::GestureClick>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Capture){click.emit_by_name::<()>("pressed",&[&1i32,&10.0f64,&10.0f64]);}}
    pump(context);open.emit_clicked();pump(context);assert!(!viewer.is_visible());assert!(!gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.title().as_deref()==Some("Imagem")&&w.is_visible()),"same-thumbnail click closes rather than reopens the viewer");
    // Session cache survives A -> B -> A, but revocation clears it.
    let mut other=target.clone();other.id=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(other));pump(context);chat.emit(ChatMsg::SetChannel(target));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});settle(context);assert_eq!(chat.model().transfers.textures[&image],texture);
    let widgets=descendants(chat.widget().upcast_ref());let list=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|l|l.has_css_class("papo-chat-history"))).unwrap();let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap();
    // Independent sends must paint without another message, pointer or scroll event.
    for i in 0..3{
        let mut message=messages[0].clone();message.id=Uuid::new_v4();message.created_at=created+chrono::Duration::seconds(100+i);message.content=Some(format!("Visible immediately {i}: a long line that wraps to exercise text allocation. ").repeat(3));
        let request={let mut state=chat.state().get_mut();state.model.draft.text=message.content.clone().unwrap();state.model.draft.begin().unwrap()};
        if i==1{chat.emit(ChatMsg::AddMessage(message.clone()));pump(context);}
        chat.emit(ChatMsg::SendFinished{request_id:request,channel_id:channel,result:Ok(message.clone())});settle(context);
        let row=chat.model().rendered[&format!("message-{}",message.id)].1.clone();let bounds=row.compute_bounds(list).unwrap();let adj=scroll.vadjustment();assert!(f64::from(bounds.y()+bounds.height())<=adj.value()+adj.page_size()+2.0);
        let text=descendants(row.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();assert!(text.is_mapped()&&text.height()>0,"send={i} text mapped={} visible={} size={}x{} row mapped={} size={}x{} bounds={bounds:?} list={}x{} adjustment={}/{}/{} root mapped={}",text.is_mapped(),text.is_visible(),text.width(),text.height(),row.is_mapped(),row.width(),row.height(),list.width(),list.height(),adj.value(),adj.upper(),adj.page_size(),window.is_mapped());
        let end=text.iter_location(&text.buffer().end_iter());assert!(end.y()+end.height()<=text.height()+2,"all wrapped send lines must fit: text height={}, end={end:?}",text.height());
        let paintable=gtk::WidgetPaintable::new(Some(&text));let snapshot=gtk::Snapshot::new();paintable.snapshot(&snapshot,text.width() as f64,text.height() as f64);let node=snapshot.to_node().expect("sent text produces a render node without more input");
        let texture=window.renderer().unwrap().render_texture(&node,None);let mut pixels=vec![0;texture.width() as usize*texture.height() as usize*4];texture.download(&mut pixels,texture.width() as usize*4);
        assert!(pixels.chunks_exact(4).collect::<std::collections::HashSet<_>>().len()>4,"sent text must actually paint glyphs");
    }
    window.set_default_size(480,680);until(context,||window.width()<=480);settle(context);
    let last=chat.model().history.messages.last().unwrap().id;let row=chat.model().rendered[&format!("message-{last}")].1.clone();let text=descendants(row.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();
    let end=text.iter_location(&text.buffer().end_iter());assert!(end.y()+end.height()<=text.height()+2,"narrow resizing must show every wrapped line");
    window.set_default_size(850,680);until(context,||window.width()>=800);settle(context);let end=text.iter_location(&text.buffer().end_iter());assert!(end.y()+end.height()<=text.height()+2,"wide resizing preserves complete text");
    let mut denied=chat.model().access.clone();denied.read=false;chat.emit(ChatMsg::SetAccess{user_id:user,access:denied});pump(context);assert!(chat.model().transfers.textures.is_empty());
    window.set_content(None::<&gtk::Widget>);window.close();
}
