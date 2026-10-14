//! Exercise real attachment requests, animation and the media download entry points.
use super::*;
use adw::prelude::*;
use actions::tests::{mock,descendants,find_button,pump,until};

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let message=Uuid::new_v4();let date=chrono::Utc::now();
    let gif=Uuid::new_v4();let png=Uuid::new_v4();let video=Uuid::new_v4();let file=Uuid::new_v4();
    let motion=crate::media::animation::fixture();
    let mut still=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(32,16).write_to(&mut still,image::ImageFormat::Png).unwrap();let still=still.into_inner();
    {
        let mut b=backend.lock().unwrap();
        b.attachment_thumbnails.insert(format!("/attachments/{gif}"),motion.clone());
        b.attachment_thumbnails.insert(format!("/attachments/{gif}/thumbnail"),still.clone());
        b.attachment_thumbnails.insert(format!("/attachments/{png}/thumbnail"),still.clone());
    }
    let attachment=|id,mime,name,size|serde_json::json!({"id":id,"mime_type":mime,"original_file_name":name,"size_bytes":size,"created_at":date});
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Media attachments","created_at":date})).unwrap();
    let msg:Message=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"created_at":date,"attachments":[
        attachment(png,"image/png","image.png",128),attachment(video,"video/webm","clip.webm",128),
        attachment(file,"application/pdf","notes.pdf",128),attachment(gif,"image/gif","cat.gif",motion.len())
    ]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),user_id:Some(user),messages:vec![msg],..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(800).content(chat.widget()).build();window.present();
    until(context,||chat.model().transfers.animations.contains_key(&gif)&&chat.model().transfers.textures.contains_key(&png)&&chat.model().media_render_pending.is_none());
    performance::settle(context);
    let animation=chat.model().transfers.animations[&gif].0.clone();let first=animation.current_image();
    until(context,||animation.current_image()!=first);
    let card=|id|descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with(&format!("attachment-{id}-"))).unwrap();
    for key in [png,video,gif]{
        assert!(!descendants(&card(key)).iter().any(|w|w.is::<gtk::Label>()),"visual attachments must have no filename/size header");
        assert!(!descendants(&card(key)).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.icon_name().as_deref()==Some("folder-download-symbolic"))),"no inline download button");
    }
    assert!(find_button(&card(file),"notes.pdf").is_mapped(),"files without a media preview retain a clickable name");
    {
        let b=backend.lock().unwrap();
        assert_eq!(b.requests.iter().filter(|(_,p,_)|p==&format!("/attachments/{gif}")).count(),1);
        assert!(!b.requests.iter().any(|(_,p,_)|p==&format!("/attachments/{gif}/thumbnail")),"a static thumbnail must not replace the small animated original");
        assert!(b.requests.iter().any(|(_,p,_)|p==&format!("/attachments/{png}/thumbnail")),"ordinary images retain thumbnail requests");
        assert!(!b.requests.iter().any(|(_,p,_)|p==&format!("/attachments/{png}")),"opening a channel must not download full still images");
    }
    let row=chat.model().rendered[&format!("message-{message}")].1.clone();
    let bounds=card(gif).compute_bounds(&row).unwrap();
    let controllers=row.observe_controllers();let right=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>().filter(|g|g.button()==3)).unwrap();
    right.emit_by_name::<()>("pressed",&[&1i32,&f64::from(bounds.x()+bounds.width()/2.0),&f64::from(bounds.y()+bounds.height()/2.0)]);
    let popup=||descendants(row.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Popover>().ok().filter(|p|p.widget_name()=="attachment-context-menu"&&p.is_visible()));
    until(context,||popup().is_some());let popup=popup().unwrap();assert!(find_button(popup.upcast_ref(),"Baixar arquivo").is_mapped());
    popup.popdown();
    find_button(&card(gif),"Abrir imagem").emit_clicked();
    until(context,||!chat.model().transfers.images.is_empty());
    let viewer=chat.model().transfers.images.last().unwrap().2.clone();until(context,||viewer.is_mapped());
    assert!(find_button(viewer.upcast_ref(),"Baixar arquivo").is_mapped());
    let picture=viewer::picture(&viewer).unwrap();assert!(picture.paintable().unwrap().is::<crate::media::animation::Animation>());
    crate::ui::main_window::layout::tests::preview(&window,"attachment-media",context);
    let path=std::env::temp_dir().join(format!("papo-saved-{gif}.gif"));
    chat.emit(ChatMsg::Transfer(TransferMsg::Save{epoch:chat.model().transfers.epoch,message,attachment:gif,path:path.clone()}));
    until(context,||path.exists()&&chat.model().transfers.saving==0);
    assert_eq!(std::fs::read(&path).unwrap(),motion,"saving retains all original GIF frames");std::fs::remove_file(&path).unwrap();
    viewer.close();pump(context);
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(message,gif,"sensitive".into())));performance::settle(context);
    assert!(!chat.model().transfers.revealed.contains(&gif));
    chat.emit(ChatMsg::Transfer(TransferMsg::Save{epoch:chat.model().transfers.epoch,message,attachment:gif,path:path.clone()}));pump(context);assert!(!path.exists());
    find_button(&card(gif),"Conteúdo sensível — revelar").emit_clicked();
    until(context,||chat.model().transfers.revealed.contains(&gif));
    let row=chat.model().rendered[&format!("message-{message}")].1.clone();let bounds=card(gif).compute_bounds(&row).unwrap();
    chat.emit(ChatMsg::ContextAt{id:message,x:f64::from(bounds.x()+bounds.width()/2.0),y:f64::from(bounds.y()+bounds.height()/2.0)});
    until(context,||descendants(row.upcast_ref()).into_iter().any(|w|w.widget_name()=="attachment-context-menu"&&w.is_visible()));
    chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});pump(context);
    chat.emit(ChatMsg::Transfer(TransferMsg::Save{epoch:chat.model().transfers.epoch,message,attachment:gif,path:path.clone()}));pump(context);assert!(!path.exists());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
