//! Real posters, bounded HTTP ranges and recent first-frame residency.
use super::*;
use adw::prelude::*;
use actions::tests::{mock,descendants,pump,until};

fn end_metadata_video()->Vec<u8>{
    let original=include_bytes!("../../../../tests/fixtures/chat-h264.mp4");let mut offset=0;
    while offset+8<=original.len(){
        let length=u32::from_be_bytes(original[offset..offset+4].try_into().unwrap()) as usize;
        if &original[offset+4..offset+8]==b"moov" {
            let padding=3<<20;let mut output=original[..offset].to_vec();
            output.extend_from_slice(&((padding+8) as u32).to_be_bytes());output.extend_from_slice(b"free");output.resize(output.len()+padding,0);
            output.extend_from_slice(&original[offset..]);return output;
        }
        assert!(length>=8);offset+=length;
    }
    panic!("fixture has no end-of-file MP4 metadata")
}
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let other=Uuid::new_v4();let message=Uuid::new_v4();let now=chrono::Utc::now();
    let png=Uuid::new_v4();let gif=Uuid::new_v4();let webm=Uuid::new_v4();let mp4=Uuid::new_v4();let thumbnail=Uuid::new_v4();let embed=Uuid::new_v4();
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(32,16,image::Rgba([30,90,220,255]))).write_to(&mut bytes,image::ImageFormat::Png).unwrap();let still=bytes.into_inner();
    let padded=end_metadata_video();let total=padded.len();
    {
        let mut b=backend.lock().unwrap();b.range_media=true;
        b.attachment_thumbnails.insert(format!("/attachments/{png}/thumbnail"),still.clone());
        b.attachment_thumbnails.insert(format!("/attachments/{gif}"),crate::media::animation::fixture());
        b.attachment_thumbnails.insert(format!("/attachments/{mp4}"),padded);
        b.attachment_thumbnails.insert(format!("/attachments/{thumbnail}/thumbnail"),still);
    }
    let attachment=|key,mime,name,thumb:Option<Uuid>|serde_json::json!({"id":key,"mime_type":mime,"original_file_name":name,"thumbnail_id":thumb,"size_bytes":512,"created_at":now});
    let target=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"Cached media","created_at":now})).unwrap();
    let message:Message=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"created_at":now,"attachments":[
        attachment(png,"image/png","image.png",None),attachment(gif,"image/gif","animation.gif",None),attachment(webm,"video/webm","clip.webm",None),
        attachment(mp4,"video/mp4","clip.mp4",None),attachment(thumbnail,"video/webm","with-thumbnail.webm",Some(Uuid::new_v4()))
    ],"embeds":[{"id":embed,"source_type":"custom","fetch_method":"manual","title":"Embedded video","created_at":now,"video":{"url":"https://example.test/video.webm","mime_type":"video/webm"}}]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target(channel)),user_id:Some(user),messages:vec![message.clone()],..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(800).content(chat.widget()).build();window.present();
    let keys=[png,gif,webm,mp4,thumbnail,embed];
    until(context,||keys.iter().all(|key|chat.model().transfers.textures.contains_key(key))&&chat.model().media_render_pending.is_none());
    performance::settle(context);
    assert!(chat.model().transfers.videos.is_empty()&&chat.model().transfers.video_files.is_empty(),"posters must not create players or retain original videos");
    for key in [webm,mp4,embed]{let model=chat.model();let texture=&model.transfers.textures[&key];assert!(texture.width()<=320&&texture.height()<=180);}
    for key in [webm,mp4,thumbnail]{
        let card=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with(&format!("attachment-{key}-"))).unwrap();
        assert_eq!(descendants(&card).into_iter().filter(|w|w.is::<gtk::Picture>()).count(),1,"a video poster must appear once without a duplicate image viewer");
    }
    {
        let b=backend.lock().unwrap();let ranges:Vec<_>=b.media_ranges.iter().filter(|(p,_)|p==&format!("/attachments/{mp4}")).map(|(_,r)|r.as_str()).collect();
        assert_eq!(ranges,vec!["bytes=0-2097151",&format!("bytes={}-{}",total-(256<<10),total-1)]);
        assert!(!b.requests.iter().any(|(_,p,_)|p==&format!("/attachments/{thumbnail}")),"an existing server thumbnail avoids video range requests");
    }
    let calls=||backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.starts_with("/attachments/")||p.ends_with("/video")).count();let baseline=calls();
    for _ in 0..3 {
        let image=chat.model().transfers.textures[&png].downgrade();let poster=chat.model().transfers.textures[&mp4].downgrade();let motion=chat.model().transfers.animations[&gif].0.downgrade();
        chat.emit(ChatMsg::SetChannel(target(other)));
        until(context,||chat.model().active_channel.as_ref().is_some_and(|c|c.id()==other)&&motion.upgrade().is_none());
        assert!(image.upgrade().is_some()&&poster.upgrade().is_some(),"recent first-frame textures remain resident while inactive");
        assert!(chat.model().transfers.textures.is_empty()&&chat.model().transfers.animations.is_empty());
        assert!(keys.iter().all(|key|chat.model().transfers.encoded.contains(*key)),"compressed data remains reusable alongside resident first frames");
        chat.emit(ChatMsg::SetChannel(target(channel)));
        until(context,||keys.iter().all(|key|chat.model().transfers.textures.contains_key(key))&&chat.model().media_render_pending.is_none());
        assert_eq!(calls(),baseline,"recent media must not be downloaded or probed again when returning to a channel");
    }
    chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});pump(context);
    assert!(keys.iter().all(|key|!chat.model().transfers.encoded.contains(*key)),"permission loss clears reusable bytes");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
    exercise_failure(context);
}

fn exercise_failure(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let key=Uuid::new_v4();let now=chrono::Utc::now();
    backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{key}"),b"invalid video fixture".to_vec());
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Unsupported video","created_at":now})).unwrap();
    let message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":now,"attachments":[{"id":key,"mime_type":"video/webm","original_file_name":"invalid.webm","size_bytes":21,"created_at":now}]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![message],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(800).default_height(650).content(chat.widget()).build();window.present();
    until(context,||descendants(chat.widget().upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Prévia indisponível")));
    assert!(actions::tests::find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").is_sensitive());
    let count=||backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/attachments/{key}")).count();let baseline=count();
    for _ in 0..12{chat.emit(ChatMsg::MediaViewport);}performance::settle(context);assert_eq!(count(),baseline,"an unsupported preview must not repeatedly download the video");
    assert!(!chat.model().transfers.encoded.contains(key));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
