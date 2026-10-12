//! Original image work belongs to the viewer, never to the chat thumbnail cache.
use super::*;

impl ChatModel {
    pub(super) fn load_viewer(&mut self,message:Uuid,key:Uuid,token:Uuid,sender:&ComponentSender<Self>){
        // Preserve animated GIFs; their existing paintable owns its frame timer.
        if self.transfers.animations.contains_key(&key){return;}
        let Some(api)=self.actions.api.clone()else{return;};
        let Some(m)=self.history.messages.iter().find(|m|m.id==message)else{return;};
        let attachment=m.attachments.iter().flatten().any(|a|a.id==key&&a.mime_type.starts_with("image/")&&self.transfers.visible(a));
        let embed=m.embeds.iter().flatten().any(|e|e.id==key);
        if !attachment&&!embed{return;}
        let epoch=self.transfers.epoch;let limiter=self.transfers.viewer_limiter.clone();let output=sender.input_sender().clone();
        let job=tokio::spawn(async move{
            let result=async{
                let permit=limiter.acquire_owned().await?;
                if attachment {
                    let file=api.download_temporary(&format!("/attachments/{key}"),MAX_FILE).await?;
                    tokio::task::spawn_blocking(move ||{
                        let _permit=permit;let file=file;
                        crate::media::PreparedImage::decode_viewer(&file.0).ok_or_else(||anyhow::anyhow!("Não foi possível exibir a imagem original"))
                    }).await?
                } else {
                    // The backend currently exposes only its cached embed image.
                    // Decode at its available resolution, without the chat cap.
                    let data=api.get_embed(key).await?.image_data.ok_or_else(||anyhow::anyhow!("Imagem do embed indisponível"))?;
                    tokio::task::spawn_blocking(move ||{
                        let _permit=permit;use base64::Engine;
                        anyhow::ensure!(data.len()<=6<<20,"Imagem do embed muito grande");
                        let bytes=base64::engine::general_purpose::STANDARD.decode(data.split_once(',').map_or(data.as_str(),|(_,data)|data))?;
                        crate::media::PreparedImage::decode_thumbnail(&bytes,4096,4096).ok_or_else(||anyhow::anyhow!("Não foi possível exibir a imagem do embed"))
                    }).await?
                }
            }.await;
            let _=output.send(ChatMsg::Transfer(TransferMsg::ViewerReady{epoch,message,key,token,result}));
        });
        self.transfers.viewer_jobs.insert(token,job);
    }

    pub(super) fn viewer_ready(&mut self,epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:anyhow::Result<crate::media::PreparedImage>,sender:&ComponentSender<Self>){
        if epoch!=self.transfers.epoch{return;}
        self.transfers.viewer_jobs.remove(&token);
        if !self.access.read||!self.history.messages.iter().any(|m|m.id==message&&(m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a))||m.embeds.iter().flatten().any(|e|e.id==key))){return;}
        let Some((_,_,window))=self.transfers.images.iter().find(|(m,k,w)|*m==message&&*k==key&&w.is_visible()&&w.widget_name()==format!("viewer-{token}"))else{return;};
        match result {
            Ok(image)=>{if let Some(picture)=window.child().and_downcast::<gtk::Picture>(){picture.set_paintable(Some(&image.texture()));}},
            Err(error)=>{
                window.set_title(Some("Imagem · não foi possível carregar o original"));
                if let Some(picture)=window.child(){picture.set_tooltip_text(Some(&format!("Exibindo prévia. {error}")));}
                if crate::api::is_session_error(&error){let _=sender.output(ChatOutput::ActionError(error));}
            },
        }
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{pump,until};use tokio::io::{AsyncReadExt,AsyncWriteExt};
    use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
    let key=Uuid::new_v4();let message=Uuid::new_v4();let channel=Uuid::new_v4();let user=Uuid::new_v4();let date=chrono::Utc::now();
    let requests=Arc::new(Mutex::new(Vec::new()));let counts=requests.clone();let fail=Arc::new(AtomicBool::new(false));let failing=fail.clone();
    let png=|width,height|{let mut file=std::io::Cursor::new(Vec::new());let pixels=image::ImageBuffer::from_fn(width,height,|x,y|image::Rgba(if (x/8+y/8)%2==0{[20u8,130,240,255]}else{[240,210,40,255]}));image::DynamicImage::ImageRgba8(pixels).write_to(&mut file,image::ImageFormat::Png).unwrap();file.into_inner()};
    let thumb=png(640,320);let original=png(1800,900);
    let(api,server)=tokio::runtime::Handle::current().block_on(async move{
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let api=crate::api::ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
        let server=tokio::spawn(async move{loop{let(mut socket,_)=listener.accept().await.unwrap();let thumb=thumb.clone();let original=original.clone();let counts=counts.clone();let failing=failing.clone();tokio::spawn(async move{
            let mut request=vec![];loop{let mut bytes=[0;2048];let n=socket.read(&mut bytes).await.unwrap_or(0);if n==0{return;}request.extend_from_slice(&bytes[..n]);if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}}
            let path=String::from_utf8_lossy(&request).split_whitespace().nth(1).unwrap().to_owned();counts.lock().unwrap().push(path.clone());
            let(status,data)=if path.ends_with("/thumbnail"){("200 OK",thumb)}else if path==format!("/attachments/{key}"){
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if failing.load(Ordering::SeqCst){("404 Not Found",b"{\"detail\":\"Original unavailable\"}".to_vec())}else{("200 OK",original)}
            }else{("404 Not Found",b"{}".to_vec())};
            let header=format!("HTTP/1.1 {status}\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",data.len());let _=socket.write_all(header.as_bytes()).await;let _=socket.write_all(&data).await;
        });}});(api,server)
    });
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Original images","created_at":date})).unwrap();
    let photo:Message=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"created_at":date,"attachments":[{"id":key,"mime_type":"image/png","original_file_name":"photo.png","size_bytes":1024,"created_at":date}]})).unwrap();
    let mut next_photo=photo.clone();next_photo.id=Uuid::new_v4();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![photo],user_id:Some(user),..Default::default()}).detach();chat.state().get_mut().model.actions.api=Some(api);chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(900).default_height(650).content(chat.widget()).build();window.present();until(context,||chat.model().transfers.textures.contains_key(&key));performance::settle(context);
    let originals=||requests.lock().unwrap().iter().filter(|p|*p==&format!("/attachments/{key}")).count();assert_eq!(originals(),0,"chat previews must never fetch originals");
    let thumbnail=chat.model().transfers.textures[&key].clone();assert_eq!(thumbnail.width(),640);
    let row=chat.model().rendered[&format!("message-{message}")].1.clone();let passes=chat.model().render_passes;
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));until(context,||!chat.model().transfers.images.is_empty());
    let viewer=chat.model().transfers.images[0].2.clone();let picture=viewer.child().and_downcast::<gtk::Picture>().unwrap();
    until(context,||originals()==1);assert_eq!(picture.paintable().unwrap().intrinsic_width(),640,"the preview is immediately available while the original downloads");
    until(context,||picture.paintable().is_some_and(|p|p.intrinsic_width()==1800));
    assert_eq!(picture.paintable().unwrap().intrinsic_height(),900);assert_eq!(chat.model().transfers.textures[&key],thumbnail);assert_eq!(chat.model().rendered[&format!("message-{message}")].1,row);assert_eq!(chat.model().render_passes,passes,"viewer decoding must not rebuild chat rows");
    let quality=picture.paintable().unwrap().downgrade();let old_token=viewer.widget_name().strip_prefix("viewer-").unwrap().parse().unwrap();let epoch=chat.model().transfers.epoch;
    crate::ui::main_window::layout::tests::preview(&viewer,"original-image",context);
    viewer.close();drop(picture);drop(viewer);until(context,||chat.model().transfers.images.is_empty());assert!(quality.upgrade().is_none(),"the original texture is released when the viewer closes");
    let permit=tokio::runtime::Handle::current().block_on(chat.model().transfers.viewer_limiter.clone().acquire_owned()).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));until(context,||!chat.model().transfers.viewer_jobs.is_empty());let reopened=chat.model().transfers.images[0].2.clone();
    chat.emit(ChatMsg::Transfer(TransferMsg::ViewerReady{epoch,message,key,token:old_token,result:Ok(crate::media::PreparedImage{width:1,height:1,pixels:vec![255;4]})}));pump(context);assert_eq!(reopened.child().and_downcast::<gtk::Picture>().unwrap().paintable().unwrap().intrinsic_width(),640,"an earlier viewer result cannot replace a reopened image");
    reopened.close();until(context,||chat.model().transfers.viewer_jobs.is_empty()&&chat.model().transfers.images.is_empty());drop(reopened);drop(permit);pump(context);assert_eq!(originals(),1,"closing cancels queued original work");
    fail.store(true,Ordering::SeqCst);chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));until(context,||!chat.model().transfers.images.is_empty());let viewer=chat.model().transfers.images[0].2.clone();
    until(context,||viewer.title().is_some_and(|t|t.contains("não foi possível")));assert!(viewer.is_visible());assert_eq!(viewer.child().and_downcast::<gtk::Picture>().unwrap().paintable().unwrap().intrinsic_width(),640,"failed original requests retain the usable preview");viewer.close();until(context,||chat.model().transfers.images.is_empty());drop(viewer);
    let permit=tokio::runtime::Handle::current().block_on(chat.model().transfers.viewer_limiter.clone().acquire_owned()).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));until(context,||!chat.model().transfers.viewer_jobs.is_empty());let viewer=chat.model().transfers.images[0].2.clone();let token=viewer.widget_name().strip_prefix("viewer-").unwrap().parse().unwrap();
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(message,key,"blocked".into())));until(context,||chat.model().transfers.images.is_empty()&&chat.model().transfers.viewer_jobs.is_empty());assert!(!viewer.is_visible());
    chat.emit(ChatMsg::Transfer(TransferMsg::ViewerReady{epoch,message,key,token,result:Ok(crate::media::PreparedImage{width:1,height:1,pixels:vec![255;4]})}));pump(context);assert!(chat.model().transfers.images.is_empty());assert!(!chat.model().transfers.textures.contains_key(&key));drop(permit);drop(viewer);
    let message=next_photo.id;chat.emit(ChatMsg::AddMessage(next_photo));until(context,||chat.model().transfers.textures.contains_key(&key));
    let permit=tokio::runtime::Handle::current().block_on(chat.model().transfers.viewer_limiter.clone().acquire_owned()).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));until(context,||!chat.model().transfers.viewer_jobs.is_empty());let viewer=chat.model().transfers.images[0].2.clone();let token=viewer.widget_name().strip_prefix("viewer-").unwrap().parse().unwrap();
    let next:Channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"Other channel","created_at":date})).unwrap();chat.emit(ChatMsg::SetChannel(next));
    until(context,||chat.model().transfers.epoch!=epoch);assert!(chat.model().transfers.images.is_empty()&&chat.model().transfers.viewer_jobs.is_empty());assert!(!viewer.is_visible());
    chat.emit(ChatMsg::Transfer(TransferMsg::ViewerReady{epoch,message,key,token,result:Ok(crate::media::PreparedImage{width:1,height:1,pixels:vec![255;4]})}));pump(context);assert!(chat.model().transfers.textures.is_empty());drop(permit);drop(viewer);
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
