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

// Opt-in integration check: a pasted link has no embed/video metadata from Papo.
pub(crate) fn exercise_klipy(context:&gtk::glib::MainContext){
    assert_eq!(std::env::var("PAPO_KLIPY_LIVE_TEST").as_deref(),Ok("1"),"public provider access must be explicitly enabled");
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let now=chrono::Utc::now();let url="https://klipy.com/gifs/halloween-scary-22";
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":url,"created_at":now})).unwrap();
    let original:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Public Klipy link","created_at":now})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(original.clone()),messages:vec![message],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(700).content(chat.widget()).build();window.present();
    let ready=||{let m=chat.model();m.transfers.giphy.get(url).is_some_and(|key|m.transfers.animations.contains_key(key))&&m.media_render_pending.is_none()};
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(45);
    while !ready()&&std::time::Instant::now()<deadline{pump(context);std::thread::sleep(std::time::Duration::from_millis(10));}assert!(ready(),"a pasted Klipy URL must animate without a Papo embed");performance::settle(context);
    let key=chat.model().transfers.giphy[url];
    assert!(chat.model().history.messages[0].embeds.as_ref().is_none_or(Vec::is_empty));assert!(chat.model().transfers.videos.is_empty());
    let widgets=descendants(chat.widget().upcast_ref());assert!(!widgets.iter().any(|w|w.is::<gtk::Video>()||w.is::<gtk::LinkButton>()));
    assert!(!widgets.iter().filter_map(|w|w.downcast_ref::<gtk::TextView>()).any(|v|v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false).contains(url)));
    let animation=chat.model().transfers.animations[&key].0.clone();let first=animation.current_image();until(context,||animation.current_image()!=first);let weak=animation.downgrade();drop(animation);drop(first);drop(widgets);
    let bytes=chat.state().get_mut().model.transfers.encoded.get(key).unwrap();
    let mut other=original.clone();other.id=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(other));until(context,||weak.upgrade().is_none());
    chat.emit(ChatMsg::SetChannel(original));until(context,ready);
    let restored=chat.state().get_mut().model.transfers.encoded.get(key).unwrap();assert!(std::sync::Arc::ptr_eq(&bytes,&restored),"returning to a pasted Klipy GIF reuses the compressed bytes, without resolving or downloading again");
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,path,_)|path.starts_with("/embeds/")),"the public link requires no backend rendition");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
