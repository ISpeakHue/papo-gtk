//! Pinned media uses the same renderer with isolated history and playback.
use super::*;
use actions::tests::{mock,descendants,find_button,pump,until};
use adw::prelude::*;
use serde_json::json;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let date=chrono::Utc::now();
    let png=Uuid::new_v4();let gif=Uuid::new_v4();let emoji=Uuid::new_v4();let embed=Uuid::new_v4();let video=Uuid::new_v4();let file=Uuid::new_v4();
    let motion=crate::media::animation::fixture();
    let mut output=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(32,16).write_to(&mut output,image::ImageFormat::Png).unwrap();let image=output.into_inner();
    let attachment=|id,mime,name,size|json!({"id":id,"mime_type":mime,"original_file_name":name,"size_bytes":size,"created_at":date});
    let current_json=json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":date,"content":"Current history","attachments":[attachment(png,"image/png","photo.png",128)]});
    let current:Message=serde_json::from_value(current_json.clone()).unwrap();
    let old_json=json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":date-chrono::Duration::days(10),"content":":test: **Pinned media**",
        "attachments":[attachment(gif,"image/gif","animation.gif",motion.len()),attachment(video,"video/webm","video.webm",128),attachment(file,"application/pdf","notes.pdf",128)],
        "embeds":[{"id":embed,"source_type":"custom","fetch_method":"manual","description":"**Rich pinned embed**","created_at":date}]});
    let old:Message=serde_json::from_value(old_json.clone()).unwrap();
    {
        let mut b=backend.lock().unwrap();b.pinned=true;b.dm_messages=vec![old_json.clone(),current_json.clone()];
        b.attachment_thumbnails.insert(format!("/attachments/{gif}"),motion);
        b.attachment_thumbnails.insert(format!("/attachments/{png}/thumbnail"),image.clone());
        b.attachment_thumbnails.insert(format!("/attachments/{png}"),image);
        b.embed_values.insert(embed.to_string(),serde_json::to_value(old.embeds.as_ref().unwrap().first().unwrap()).unwrap());
    }
    let target:Channel=serde_json::from_value(json!({"id":channel,"name":"Pinned media","created_at":date})).unwrap();
    let observed=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));let outputs=observed.clone();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![current.clone()],user_id:Some(user),..Default::default()}).connect_receiver(move |_,event|outputs.borrow_mut().push(event));
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(700).content(chat.widget()).build();window.present();
    until(context,||chat.model().transfers.textures.contains_key(&png));
    performance::settle(context);
    let cached=chat.model().transfers.textures[&png].clone();
    // Mimic an emoji already loaded in the main chat's catalog.
    {let mut state=chat.state().get_mut();let m=&mut state.model;m.actions.emojis=vec![serde_json::from_value(json!({"id":emoji,"name":"test","format":"GIF","created_at":date})).unwrap()];m.actions.emoji_names.insert("test".into(),emoji);m.actions.textures.insert(emoji,cached.clone());}
    let history=chat.model().history.messages.iter().map(|m|m.id).collect::<Vec<_>>();
    let before=observed.borrow().len();
    chat.emit(ChatMsg::Action(ActionMsg::OpenPins));
    until(context,||chat.model().actions.pins_window.as_ref().is_some_and(|v|v.chat.model().history.messages.len()==2));
    // Borrowed model guards must not span a GTK event-loop iteration.
    let pin_input=chat.model().actions.pins_window.as_ref().unwrap().chat.sender().clone();
    let pin_window=chat.model().actions.pins_window.as_ref().unwrap().window.clone();
    until(context,||chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.animations.contains_key(&gif));
    performance::settle(context);
    {
        let m=chat.model();let view=m.actions.pins_window.as_ref().unwrap();let pins=view.chat.model();
        assert_eq!(pins.transfers.textures[&png],cached,"reuse decoded image textures");
        assert!(pins.actions.textures.contains_key(&emoji));
        assert!(descendants(view.chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::TextView>()));
        assert!(find_button(view.chat.widget().upcast_ref(),"notes.pdf").is_visible());
        assert!(pins.transfers.authorized(&pins.history.messages,gif),"pins outside loaded history can fetch their media");
        assert_eq!(m.history.messages.iter().map(|m|m.id).collect::<Vec<_>>(),history,"pin browsing must not change the active history");
        assert!(!observed.borrow()[before..].iter().any(|e|matches!(e,ChatOutput::LoadMoreMessages{..}|ChatOutput::HistoryObserved{..}|ChatOutput::ConversationLoaded)));
    }
    pin_input.send(ChatMsg::Action(ActionMsg::Navigate(old.id))).unwrap();performance::settle(context);
    {
        let m=chat.model();let view=m.actions.pins_window.as_ref().unwrap();
        assert!(!descendants(view.chat.widget().upcast_ref()).iter().any(|w|w.is_mapped()&&w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Você está vendo mensagens antigas")),"pins have no ordinary-history navigation banner");
        let buffers:Vec<_>=descendants(view.chat.widget().upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::TextView>().ok().map(|v|v.buffer())).collect();
        assert!(buffers.iter().any(|b| (0..b.char_count()).any(|offset|b.iter_at_offset(offset).paintable().is_some())),"custom emojis render as inline paintables");
        assert!(buffers.iter().any(|b|b.text(&b.start_iter(),&b.end_iter(),true).contains("Rich pinned embed")),"rich embeds use the chat renderer");
    }
    let animation=chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.animations[&gif].0.clone();let first=animation.current_image();
    until(context,||animation.current_image()!=first);
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/attachments/{png}/thumbnail")).count(),1);
    pin_input.send(ChatMsg::Transfer(TransferMsg::AttachmentVideo{message:old.id,attachment:video})).unwrap();
    until(context,||chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.videos.iter().any(|p|p.key==video));
    let playback=chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.videos.iter().find(|p|p.key==video).unwrap().media.clone();
    crate::ui::main_window::layout::tests::preview(&pin_window,"pinned-media",context);
    pin_input.send(ChatMsg::Transfer(TransferMsg::OpenImage{message:current.id,key:png})).unwrap();
    until(context,||!chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.images.is_empty());
    let image_window=chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.images[0].2.clone();
    until(context,||image_window.is_mapped());assert!(find_button(image_window.upcast_ref(),"Baixar arquivo").is_mapped());
    image_window.close();pump(context);
    chat.emit(ChatMsg::ApplyChange(Change::Edit(old.id,"Updated :test:".into(),Some(date))));
    until(context,||chat.model().actions.pins_window.as_ref().unwrap().chat.model().history.messages.iter().any(|m|m.id==old.id&&m.content.as_deref()==Some("Updated :test:")));
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(old.id,gif,"sensitive".into())));
    until(context,||!chat.model().actions.pins_window.as_ref().unwrap().chat.model().transfers.authorized(&chat.model().actions.pins_window.as_ref().unwrap().chat.model().history.messages,gif));
    // An unpin/re-pin refresh must not create a deletion tombstone.
    {let mut b=backend.lock().unwrap();b.dm_messages.retain(|m|m["id"]!=json!(old.id));}
    chat.emit(ChatMsg::Action(ActionMsg::PinEvent{id:old.id,pinned:false}));
    until(context,||chat.model().actions.pins_window.as_ref().unwrap().chat.model().history.messages.len()==1);
    assert!(playback.file().is_none(),"unpinning stops and clears the pinned video player");
    backend.lock().unwrap().dm_messages.push(old_json.clone());
    chat.emit(ChatMsg::Action(ActionMsg::PinEvent{id:old.id,pinned:true}));
    until(context,||chat.model().actions.pins_window.as_ref().unwrap().chat.model().history.messages.len()==2);
    chat.emit(ChatMsg::DeleteMessage(old.id));
    until(context,||chat.model().actions.pins_window.as_ref().unwrap().chat.model().history.messages.len()==1);
    pin_window.close();until(context,||chat.model().actions.pins_window.is_none());
    chat.emit(ChatMsg::Action(ActionMsg::OpenPins));until(context,||chat.model().actions.pins_window.is_some());
    chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});until(context,||chat.model().actions.pins_window.is_none());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
