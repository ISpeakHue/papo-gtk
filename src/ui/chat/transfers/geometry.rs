//! Reproduce first-load letterboxing with a placeholder that has been painted.
use super::*;
use actions::tests::{descendants,pump,until};
use adw::prelude::*;

fn frame(root:&gtk::Widget)->crate::media::frame::Frame{
    descendants(root).into_iter().find_map(|w|w.downcast().ok()).expect("image frame")
}
fn picture(root:&gtk::Widget)->gtk::Picture{
    descendants(root).into_iter().find_map(|w|w.downcast().ok()).expect("decoded image")
}
fn assert_fitted(root:&gtk::Widget){
    let picture=picture(root);let ratio=picture.paintable().unwrap().intrinsic_aspect_ratio();
    assert!(picture.width()>0&&picture.height()>0);
    assert!((f64::from(picture.height())-f64::from(picture.width())/ratio).abs()<=1.5,
        "first-load image must fill its frame without letterboxing: {}×{} at ratio {ratio}",picture.width(),picture.height());
}
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    for (embedded,animated,width,height) in [(false,false,1280,320),(false,false,160,640),(true,false,1280,320),(true,false,160,160),(true,true,256,64)]{
        let channel=Uuid::new_v4();let user=Uuid::new_v4();let key=Uuid::new_v4();let now=chrono::Utc::now();
        let target=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"first images","created_at":now})).unwrap();
        let mime=if animated{"image/gif"}else{"image/png"};
        let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":now,
            "attachments":(!embedded).then(||vec![serde_json::json!({"id":key,"mime_type":mime,"original_file_name":"first.png","size_bytes":128,"created_at":now})]),
            "embeds":embedded.then(||vec![serde_json::json!({"id":key,"source_type":"custom","fetch_method":"manual","created_at":now,"thumbnail":{"mime_type":mime}})])})).unwrap();
        let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target(channel)),messages:vec![message.clone()],user_id:Some(user),..Default::default()}).detach();
        chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
        let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();
        until(context,||!chat.model().viewport.opening_flag().get()&&chat.model().transfers.frames.borrow().contains_key(&(key,embedded)));
        performance::settle(context);let guessed=frame(chat.widget().upcast_ref());assert!(guessed.width()>0&&guessed.height()>0);
        assert!(descendants(chat.widget().upcast_ref()).iter().all(|w|!w.is::<gtk::Picture>()),"the placeholder must be visible before the first decode");
        let mut bytes=std::io::Cursor::new(Vec::new());let rgba=image::RgbaImage::from_pixel(width,height,image::Rgba([48,120,180,255]));
        if animated{image::codecs::gif::GifEncoder::new(&mut bytes).encode_frames((0..2).map(|_|image::Frame::from_parts(rgba.clone(),0,0,image::Delay::from_numer_denom_ms(100,1)))).unwrap();}
        else{image::DynamicImage::ImageRgba8(rgba).write_to(&mut bytes,image::ImageFormat::Png).unwrap();}
        let bytes=bytes.into_inner();let epoch=chat.model().transfers.epoch;
        chat.emit(ChatMsg::Transfer(TransferMsg::Bytes{epoch,message:message.id,key,token:None,result:Ok(bytes.clone())}));
        until(context,||chat.model().transfers.textures.contains_key(&key)&&chat.model().media_render_pending.is_none()&&descendants(chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::Picture>()));
        performance::settle(context);assert_fitted(chat.widget().upcast_ref());
        if animated{assert!(chat.model().transfers.animations.contains_key(&key));}
        let actual=frame(chat.widget().upcast_ref());assert_ne!((actual.width(),actual.height()),(guessed.width(),guessed.height()),"decoded geometry must supersede the guessed first frame");
        // Resizing and returning to a recent channel reuse its fitted preview.
        window.set_default_size(400,680);performance::settle(context);assert_fitted(chat.widget().upcast_ref());let narrow=frame(chat.widget().upcast_ref());let geometry=(narrow.width(),narrow.height());assert!(geometry.0<400);
        if !embedded&&width>height{crate::ui::main_window::layout::tests::preview(&window,"first-image-fitted",context);}
        let cached_preview=chat.model().transfers.textures[&key].as_ptr();
        chat.emit(ChatMsg::SetChannel(target(Uuid::new_v4())));pump(context);chat.emit(ChatMsg::SetChannel(target(channel)));
        until(context,||!chat.model().viewport.opening_flag().get()&&!chat.model().history.messages.is_empty());performance::settle(context);
        assert_eq!(chat.model().transfers.textures.get(&key).map(|t|t.as_ptr()),Some(cached_preview),"recent geometry reuses the cached preview");assert_fitted(chat.widget().upcast_ref());
        // Explicitly evict the decoded preview as memory pressure would do;
        // losing the pixels must not restore the guessed frame dimensions.
        {let mut state=chat.state().get_mut();let media=&mut state.model.transfers;
            media.textures.remove(&key);media.animations.remove(&key);media.resident.invalidate(key);media.dirty.insert(key);
            let token=Uuid::new_v4();state.model.media_render_pending=Some(token);chat.emit(ChatMsg::RenderMedia(token));
        }
        until(context,||chat.model().media_render_pending.is_none());performance::settle(context);
        assert!(chat.model().transfers.textures.is_empty());let restored=frame(chat.widget().upcast_ref());assert_eq!((restored.width(),restored.height()),geometry,"eviction must retain the corrected frame geometry");
        chat.emit(ChatMsg::Transfer(TransferMsg::Bytes{epoch:chat.model().transfers.epoch,message:message.id,key,token:None,result:Ok(bytes)}));
        until(context,||chat.model().transfers.textures.contains_key(&key)&&chat.model().media_render_pending.is_none());performance::settle(context);assert_fitted(chat.widget().upcast_ref());
        window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
    }
}

pub(crate) fn exercise_metadata(context:&gtk::glib::MainContext){
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let photo=Uuid::new_v4();let video=Uuid::new_v4();let message=Uuid::new_v4();let now=chrono::Utc::now();
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Server geometry","created_at":now})).unwrap();
    let msg=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"created_at":now,"attachments":[
        {"id":photo,"mime_type":"image/png","original_file_name":"photo.png","thumbnail_id":Uuid::new_v4(),"thumbnail_width":600,"thumbnail_height":120,"size_bytes":100,"created_at":now},
        {"id":video,"mime_type":"video/mp4","original_file_name":"clip.mp4","thumbnail_id":Uuid::new_v4(),"thumbnail_width":160,"thumbnail_height":240,"size_bytes":100,"created_at":now}
    ]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![msg],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let host=adw::Window::builder().default_width(1000).default_height(800).content(chat.widget()).build();host.present();performance::settle(context);
    let geometry=|key|{let card=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with(&format!("attachment-{key}-"))).unwrap();let frame=descendants(&card).into_iter().find(|w|w.type_().name()=="PapoMediaFrame").unwrap();(frame.width(),frame.height())};
    assert_eq!(geometry(photo),(600,120),"the server ratio reserves space before any image bytes arrive");assert_eq!(geometry(video),(160,240),"video posters also use server thumbnail geometry");
    let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(600,120).write_to(&mut png,image::ImageFormat::Png).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::Bytes{epoch:chat.model().transfers.epoch,message,key:photo,token:None,result:Ok(png.into_inner())}));until(context,||chat.model().transfers.textures.contains_key(&photo));performance::settle(context);
    assert_eq!(geometry(photo),(600,120),"decoding cannot replace the reserved ratio with a guessed frame");
    host.set_default_size(400,800);performance::settle(context);let(w,h)=geometry(photo);assert!(w<600&&(w-5*h).abs()<=3);
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);pump(context);
}
