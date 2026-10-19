//! Provider animations must bypass blocked public pages and still thumbnails.
use super::*;
use actions::tests::{mock,descendants,find_button,pump,until};
use adw::prelude::*;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let message=Uuid::new_v4();let now=chrono::Utc::now();
    let klipy=Uuid::new_v4();let named=Uuid::new_v4();let still=Uuid::new_v4();let gif=Uuid::new_v4();
    let generic=Uuid::new_v4();let attribution=Uuid::new_v4();
    let url="https://klipy.com/gifs/happycadogt-bang";
    let embed=|id,url,provider|serde_json::json!({"id":id,"source_type":"custom","fetch_method":if id==named{""}else{"manual"},"url":url,"site_name":provider,"title":"GIF example","description":"GIF information belongs in its viewer","created_at":now,
        "image_data":actions::tests::tiny_png_base64(),"thumbnail":{"mime_type":"image/webp","width":640,"height":360},"video":{"url":"https://example.test/rendition.mp4","mime_type":"video/mp4","width":160,"height":96}});
    {
        let mut b=backend.lock().unwrap();for id in [klipy,named]{b.attachment_thumbnails.insert(format!("/embeds/{id}/video"),include_bytes!("../../../../tests/fixtures/chat-h264.mp4").to_vec());}
        let mut png=std::io::Cursor::new(vec![]);image::DynamicImage::new_rgb8(32,16).write_to(&mut png,image::ImageFormat::Png).unwrap();
        b.attachment_thumbnails.insert(format!("/attachments/{still}/thumbnail"),png.into_inner());b.attachment_thumbnails.insert(format!("/attachments/{gif}"),crate::media::animation::fixture());
    }
    use base64::Engine;let encoded_gif=base64::engine::general_purpose::STANDARD.encode(crate::media::animation::fixture());
    let msg:Message=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"content":url,"created_at":now,
        "embeds":[embed(klipy,url,"klipy.com"),embed(named,"https://example.test/provider-card","giphy.com"),
            {"id":generic,"source_type":"custom","fetch_method":"manual","image":{"mime_type":"image/gif"},"created_at":now,"image_data":encoded_gif},
            {"id":attribution,"source_type":"custom","fetch_method":"manual","site_name":"Bridge sender","author":{"name":"Bridge sender"},"description":url,"footer":{"text":"Discord"},"created_at":now}],
        "attachments":[{"id":still,"mime_type":"image/png","original_file_name":"tiny.png","size_bytes":128,"created_at":now},
            {"id":gif,"mime_type":"image/gif","original_file_name":"tiny.gif","size_bytes":128,"created_at":now}]})).unwrap();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"GIF providers","created_at":now})).unwrap();let original=target.clone();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![msg],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(850).content(chat.widget()).build();window.present();
    until(context,||[klipy,named,gif,generic].iter().all(|id|chat.model().transfers.animations.contains_key(id))&&chat.model().transfers.textures.contains_key(&still)&&chat.model().media_render_pending.is_none());performance::settle(context);
    assert!(chat.model().transfers.videos.is_empty(),"GIF renditions must never create a GTK video player");
    for id in [klipy,named]{
        assert_eq!(chat.model().transfers.sizes[&id],(160,96),"decoded preview dimensions must not override server geometry");
        assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/embeds/{id}/video")).count(),1);
        assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p==&format!("/embeds/{id}")),"a still-thumbnail GET must not race the provider animation");
    }
    assert_eq!(chat.model().transfers.sizes[&still],(32,16));
    let card=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with(&format!("attachment-{still}-"))).unwrap();
    let frame=descendants(&card).into_iter().find_map(|w|w.downcast::<crate::media::frame::Frame>().ok()).unwrap();assert_eq!((frame.width(),frame.height()),(32,16));
    assert!(!descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.tooltip_text().as_deref()==Some("Reproduzir vídeo")));
    let widgets=descendants(chat.widget().upcast_ref());
    assert_eq!(widgets.iter().filter(|w|w.has_css_class("papo-link-preview")).count(),1,"only the sender attribution keeps its compact rich card");
    assert!(!widgets.iter().any(|w|w.is::<gtk::Video>()||w.is::<gtk::LinkButton>()),"GIF chat surfaces have neither players nor source links");
    let labels:Vec<_>=widgets.iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).map(|l|l.text().to_string()).collect();
    assert!(labels.iter().any(|s|s=="Bridge sender"));
    assert!(!labels.iter().any(|s|s.contains(url)||s.contains("GIF example")||s.contains("belongs in its viewer")),"GIF metadata is hidden until opened");
    let texts:Vec<_>=widgets.iter().filter_map(|w|w.downcast_ref::<gtk::TextView>()).map(|v|v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false).to_string()).collect();
    assert!(!texts.iter().any(|s|s.contains(url)),"the raw source URL must not survive in a reused text row or bot attribution");
    let media=widgets.iter().find(|w|w.widget_name().starts_with(&format!("preview-{klipy}-"))).unwrap();
    let requests=backend.lock().unwrap().requests.len();find_button(media,"Abrir GIF").emit_clicked();until(context,||!chat.model().transfers.images.is_empty());
    let viewer=chat.model().transfers.images[0].2.clone();
    assert_eq!(viewer.title().as_deref(),Some("GIF"));
    assert!(viewer::picture(&viewer).is_some(),"viewer helpers must still find the picture after GIF details are appended");
    let details=descendants(viewer.upcast_ref());assert!(details.iter().any(|w|w.widget_name()=="gif-information"));
    assert!(details.iter().filter_map(|w|w.downcast_ref::<gtk::LinkButton>()).any(|b|b.uri()==url));
    assert!(details.iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="GIF example"));
    assert!(details.iter().filter_map(|w|w.downcast_ref::<gtk::Picture>()).any(|p|p.paintable().is_some_and(|p|p==chat.model().transfers.animations[&klipy].0.clone().upcast::<gtk::gdk::Paintable>())));
    assert_eq!(backend.lock().unwrap().requests.len(),requests,"opening an animated GIF reuses its decoded frames");
    viewer.close();until(context,||chat.model().transfers.images.is_empty());drop(details);drop(viewer);drop(widgets);
    let animation=chat.model().transfers.animations[&klipy].0.clone();let first=animation.current_image();until(context,||animation.current_image()!=first);
    let before=backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.ends_with("/video")).count();
    let weak=chat.model().transfers.animations[&klipy].0.downgrade();drop(animation);
    let mut other=original.clone();other.id=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(other));until(context,||weak.upgrade().is_none());
    assert!(chat.state().get_mut().model.transfers.encoded.get_video(klipy).is_some());
    chat.emit(ChatMsg::SetChannel(original));until(context,||[klipy,named].iter().all(|id|chat.model().transfers.animations.contains_key(id)));
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.ends_with("/video")).count(),before,"compressed GIF renditions must survive channel switching without another HTTP request");
    // An unavailable proxy must expose the source and retry only when opened,
    // without repeatedly fetching a static thumbnail or starting a player.
    let broken=Uuid::new_v4();let broken_message=Uuid::new_v4();let broken_url="https://klipy.com/gifs/halloween-scary-22";
    backend.lock().unwrap().attachment_thumbnails.insert(format!("/embeds/{broken}/video"),b"not a media file".to_vec());
    let broken_embed=embed(broken,broken_url,"klipy.com");
    let msg:Message=serde_json::from_value(serde_json::json!({"id":broken_message,"channel_id":channel,"author_id":user,"created_at":now+chrono::Duration::seconds(1),"content":broken_url,"embeds":[broken_embed]})).unwrap();
    chat.emit(ChatMsg::AddMessage(msg));until(context,||chat.model().transfers.failed.contains_key(&broken)&&chat.model().media_render_pending.is_none());performance::settle(context);
    let widgets=descendants(chat.widget().upcast_ref());let tile=widgets.iter().find(|w|w.widget_name().starts_with(&format!("preview-{broken}-"))).unwrap();
    let menu=descendants(tile).into_iter().find_map(|w|w.downcast::<gtk::MenuButton>().ok()).unwrap();assert!(menu.tooltip_text().unwrap().contains("indisponível"));
    menu.popup();until(context,||menu.popover().unwrap().is_visible());let popover=menu.popover().unwrap();
    assert!(descendants(popover.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::LinkButton>()).any(|b|b.uri()==broken_url));
    assert!(find_button(popover.upcast_ref(),"Tentar novamente").is_sensitive());menu.popdown();
    let count=backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/embeds/{broken}/video")).count();
    assert_eq!(count,1);performance::settle(context);assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/embeds/{broken}/video")).count(),count);
    assert!(!chat.model().transfers.video_files.contains_key(&broken),"a failed animation must not retain an invalid rendition");
    menu.popup();find_button(popover.upcast_ref(),"Tentar novamente").emit_clicked();
    until(context,||backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/embeds/{broken}/video")).count()==2&&chat.model().transfers.failed.contains_key(&broken)&&chat.model().media_render_pending.is_none());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}

// The GIF resolver is now entirely server-owned. No public network is used.
pub(crate) fn exercise_backend_gifs(context:&gtk::glib::MainContext){
    use base64::Engine;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let now=chrono::Utc::now();
    let urls=["https://klipy.com/gifs/halloween-scary-22","https://media0.giphy.com/media/v1.Y2lkPTc5MGI3NjExYnlmcjdvMHN1a2p2aDBiajlyYmtpMjczMnptMTFyamdtdjh2MThuMCZlcD12MV9pbnRlcm5hbF9naWZfYnlfaWQmY3Q9Zw/okfvUCpgArv3y/giphy.gif","https://tenor.com/view/cat-123"];
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Backend GIF media","created_at":now})).unwrap();
    let mut messages:Vec<Message>=urls.iter().map(|url|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":url,"created_at":now})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target.clone()),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(850).content(chat.widget()).build();window.present();performance::settle(context);
    assert!(chat.model().transfers.giphy.is_empty(),"a link without backend metadata must not start provider scraping");
    let mut keys=vec![];
    for(index,message) in messages.iter_mut().enumerate(){
        let key=Uuid::new_v4();keys.push(key);
        let mut bytes=crate::media::animation::fixture();
        if index==0{bytes.resize((4<<20)+32,0);}
        let mut value=serde_json::json!({"id":key,"source_type":"link","fetch_method":"gif","provider":(["KLIPY","GIPHY","Tenor"][index]),"url":urls[index],"title":"GIF from backend","thumbnail":{"mime_type":"image/gif","width":8,"height":8},"created_at":now});
        // Resolved Klipy/Giphy embeds can carry both renditions. The GIF
        // bytes must win without a relay request or a video player.
        if index<2{value["video"]=serde_json::json!({"url":"https://example.test/fallback.mp4","mime_type":"video/mp4"});}
        let data=base64::engine::general_purpose::STANDARD.encode(bytes);
        if index==2{value["image_data"]=data.into();}else{let mut response=value.clone();response["image_data"]=data.into();backend.lock().unwrap().embed_values.insert(key.to_string(),response);}
        message.embeds=Some(vec![serde_json::from_value(value).unwrap()]);
        chat.emit(ChatMsg::AddMessage(message.clone()));
    }
    until(context,||keys.iter().all(|key|chat.model().transfers.animations.contains_key(key))&&chat.model().media_render_pending.is_none());performance::settle(context);
    assert!(chat.model().transfers.videos.is_empty());
    assert!(chat.model().transfers.encoded.contains(keys[0]),"the backend's 5 MiB GIF limit must fit the compressed cache");
    for(index,key) in keys.iter().enumerate(){let requests=backend.lock().unwrap().requests.clone();assert_eq!(requests.iter().filter(|(_,p,_)|p==&format!("/embeds/{key}")).count(),usize::from(index!=2));assert!(!requests.iter().any(|(_,p,_)|p==&format!("/embeds/{key}/video")));}
    let widgets=descendants(chat.widget().upcast_ref());assert!(!widgets.iter().any(|w|w.is::<gtk::Video>()||w.is::<gtk::LinkButton>()));drop(widgets);
    let before=backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.starts_with("/embeds/")).count();
    let mut other=target.clone();other.id=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(other));pump(context);chat.emit(ChatMsg::SetChannel(target));
    until(context,||keys.iter().all(|key|chat.model().transfers.animations.contains_key(key))&&chat.model().media_render_pending.is_none());
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.starts_with("/embeds/")).count(),before,"returning to the channel reuses GIF bytes");
    // An invalid primary image can still use an authorized video rendition,
    // without turning the bare GIF surface into a video player.
    let fallback=Uuid::new_v4();let fallback_message=Uuid::new_v4();
    let value=serde_json::json!({"id":fallback,"source_type":"link","fetch_method":"gif","provider":"KLIPY","url":"https://klipy.com/gifs/fallback-test","thumbnail":{"mime_type":"image/gif"},"video":{"url":"https://example.test/fallback.webm","mime_type":"video/webm"},"created_at":now});
    {let mut b=backend.lock().unwrap();let mut response=value.clone();response["image_data"]=base64::engine::general_purpose::STANDARD.encode(b"invalid image").into();b.embed_values.insert(fallback.to_string(),response);b.attachment_thumbnails.insert(format!("/embeds/{fallback}/video"),include_bytes!("../../../../tests/fixtures/chat-video.webm").to_vec());}
    let message=serde_json::from_value(serde_json::json!({"id":fallback_message,"channel_id":channel,"author_id":user,"content":"https://klipy.com/gifs/fallback-test","created_at":now+chrono::Duration::seconds(1),"embeds":[value]})).unwrap();
    chat.emit(ChatMsg::AddMessage(message));until(context,||chat.model().transfers.animations.contains_key(&fallback));
    assert!(chat.model().transfers.videos.is_empty());
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/embeds/{fallback}/video")).count(),1);
    let composer=super::super::composer::field(chat.widget().upcast_ref());composer.set_text("Keep this draft");pump(context);
    chat.emit(ChatMsg::OperationError("First temporary failure".into()));pump(context);let old=chat.model().error_timer.as_ref().unwrap().0;
    // Simulate an already-finished source whose dismissal is still queued.
    {let model=chat.model();let timer=&model.error_timer.as_ref().unwrap().1;gtk::glib::MainContext::default().find_source_by_id(timer).unwrap().destroy();}
    chat.emit(ChatMsg::OperationError("New temporary failure".into()));pump(context);chat.emit(ChatMsg::DismissError(old));pump(context);
    assert_eq!(chat.model().draft.error.as_deref(),Some("New temporary failure"),"an old timeout cannot dismiss a newer error");
    let before_expiry=std::time::Instant::now()+std::time::Duration::from_secs(3);
    while std::time::Instant::now()<before_expiry{pump(context);std::thread::sleep(std::time::Duration::from_millis(10));}
    assert!(chat.model().draft.error.is_some(),"errors remain readable before the five-second timeout");
    until(context,||chat.model().draft.error.is_none());assert_eq!(composer.text(),"Keep this draft");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
