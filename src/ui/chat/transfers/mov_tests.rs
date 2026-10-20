//! QuickTime attachments whose MIME type was reduced to generic binary data.
use super::*;
use adw::prelude::*;
use actions::tests::{mock,descendants,find_button,pump,until};

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let message=Uuid::new_v4();let now=chrono::Utc::now();
    let binary=Uuid::new_v4();let quicktime=Uuid::new_v4();let blocked=Uuid::new_v4();
    let bytes=include_bytes!("../../../../tests/fixtures/chat-video.mov");
    {let mut b=backend.lock().unwrap();b.range_media=true;for key in [binary,quicktime]{b.attachment_thumbnails.insert(format!("/attachments/{key}"),bytes.to_vec());}}
    let attachment=|id,mime,name,status|serde_json::json!({"id":id,"mime_type":mime,"original_file_name":name,"size_bytes":bytes.len(),"moderation_status":status,"created_at":now});
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"MOV attachments","created_at":now})).unwrap();
    let msg:Message=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"created_at":now,"attachments":[
        attachment(binary,"application/octet-stream","phone.MOV","clean"),
        attachment(quicktime,"video/quicktime","movie.mov","clean"),
        attachment(blocked,"application/octet-stream","blocked.mov","blocked")
    ]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![msg],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let host=adw::Window::builder().default_width(900).default_height(800).content(chat.widget()).build();host.present();
    until(context,||[binary,quicktime].iter().all(|key|chat.model().transfers.textures.contains_key(key)));
    performance::settle(context);
    let card=|key|descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with(&format!("attachment-{key}-"))).unwrap();
    for key in [binary,quicktime]{
        assert!(find_button(&card(key),"Reproduzir vídeo").is_mapped());
        assert!(!descendants(&card(key)).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|matches!(b.label().as_deref(),Some("phone.MOV"|"movie.mov")))));
        assert_eq!(chat.model().transfers.textures[&key].width(),160,"a real MOV frame must decode");
    }
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p.starts_with(&format!("/attachments/{blocked}"))),"MOV recognition must preserve moderation checks");
    assert!(chat.model().transfers.videos.is_empty(),"posters do not start playback");
    // Both MIME routes must reach the native player; the fixture has no audio.
    for key in [binary,quicktime]{
        find_button(&card(key),"Reproduzir vídeo").emit_clicked();
        until(context,||chat.model().transfers.videos.iter().any(|p|p.key==key));
        let media=chat.model().transfers.videos.iter().find(|p|p.key==key).unwrap().media.clone();
        until(context,||{assert!(media.error().is_none(),"MOV playback error: {:?}",media.error());media.is_prepared()&&media.has_video()&&media.timestamp()>0});
        chat.emit(ChatMsg::Transfer(TransferMsg::CloseVideo{message,key}));pump(context);
        assert!(chat.model().transfers.videos.is_empty());
        let calls=backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/attachments/{key}")).count();
        find_button(&card(key),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());
        assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/attachments/{key}")).count(),calls,"reopening reuses the bounded video cache");
        chat.emit(ChatMsg::Transfer(TransferMsg::CloseVideo{message,key}));pump(context);
    }
    chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});pump(context);
    assert!(chat.model().transfers.video_files.is_empty()&&chat.model().transfers.videos.is_empty());
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);server.abort();pump(context);
}
