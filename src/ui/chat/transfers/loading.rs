//! EN-US: Keep small previews moving while full GIF animations prepare off-thread.
//! PT-BR: Mantém o carregamento de prévias pequenas enquanto as animações completas são preparadas fora da interface.
use super::*;

#[cfg(test)]
#[tokio::test]
async fn a_new_viewport_cancels_obsolete_previews_without_discarding_recent_demand() {
    let mut transfers = Transfers::default();
    let keys:[Uuid;4] = std::array::from_fn(|_|Uuid::new_v4());
    let mut jobs = Vec::new();
    for key in keys {
        let job = tokio::spawn(std::future::pending::<()>());
        jobs.push(job.abort_handle());
        transfers.media_job(key,job);
        transfers.requested.insert(key);
        transfers.image_tokens.insert(key,Uuid::new_v4());
    }
    transfers.visible_keys.insert(keys[0]);
    transfers.recent_keys.insert(keys[1],HashSet::from([Uuid::new_v4()]));
    transfers.video_requests.insert((Uuid::new_v4(),keys[2]),Uuid::new_v4());
    transfers.sizes.insert(keys[3],(640,160));
    transfers.cancel_obsolete_previews();
    tokio::task::yield_now().await;
    assert!(jobs[3].is_finished());
    assert!(jobs[..3].iter().all(|job|!job.is_finished()));
    assert_eq!(transfers.requested,HashSet::from([keys[0],keys[1],keys[2]]));
    assert!(!transfers.image_tokens.contains_key(&keys[3]),"late cancelled responses cannot mount");
    assert_eq!(transfers.sizes[&keys[3]],(640,160),"cancelling work cannot collapse a row");
    assert!(transfers.dirty.is_empty(),"cancelling queued work cannot request a layout change");
}

pub(super) fn queue_image(
    transfers: &mut Transfers,
    sender: &ComponentSender<ChatModel>,
    message: Uuid,
    key: Uuid,
    input: ImageInput,
) {
    let gif_video = transfers.gif_requested.contains(&key);
    let animate = transfers.scope.contains(&message);
    let has_preview = transfers.textures.contains_key(&key);
    if animate { transfers.static_previews.remove(&key); }
    let token = Uuid::new_v4();
    transfers.image_tokens.insert(key, token);
    let epoch = transfers.epoch;
    let decoders = transfers.decode_limiter.clone();
    let animations = transfers.animation_limiter.clone();
    let output = sender.input_sender().clone();
    transfers.media_job(key, tokio::spawn(async move {
        let wait = crate::performance::timer("media_decode_wait_us");
        let Ok(permit) = decoders.clone().acquire_owned().await else { return; };
        drop(wait);
        let initial = tokio::task::spawn_blocking(move || {
            // EN-US: Keep the permit inside the blocking decoder: cancelling its async caller does not stop decoding.
            // PT-BR: Mantém a permissão dentro do decodificador bloqueante: cancelar a tarefa assíncrona não interrompe a decodificação.
            let _permit = permit;
            let _timing = crate::performance::timer("media_decode_us");
            let bytes: std::sync::Arc<[u8]> = match input {
                ImageInput::Shared(bytes) => bytes,
                ImageInput::Bytes(bytes) => bytes.into(),
                ImageInput::Base64(data) => {
                    use base64::Engine;
                    if data.len() > 8 << 20 { return None; }
                    base64::engine::general_purpose::STANDARD.decode(data).ok()?.into()
                }
            };
            let dimensions = image::ImageReader::new(std::io::Cursor::new(bytes.as_ref()))
                .with_guessed_format().ok().and_then(|r| r.into_dimensions().ok())
                .filter(|(w,h)| *w > 0 && *h > 0 && *w <= 4096 && *h <= 4096);
            let gif = bytes.starts_with(b"GIF8");
            let image = if gif && animate && has_preview {
                None
            } else if gif {
                crate::media::animation::PreparedAnimation::first(&bytes)
            } else {
                crate::media::PreparedImage::decode_thumbnail(&bytes, 680, 460)
            };
            Some((bytes, dimensions, gif, image))
        }).await.ok().flatten();
        let Some((bytes, dimensions, gif, mut image)) = initial else {
            let _ = output.send(ChatMsg::Transfer(TransferMsg::ImageReady { epoch, key, token, result: None }));
            return;
        };
        if gif && !animate {
            let _ = output.send(ChatMsg::Transfer(TransferMsg::Decoded {
                epoch, message, key, token, bytes, dimensions, preview_only: true, image, animation: None,
            }));
        } else if gif {
            if let Some(first) = image.take() {
                let _ = output.send(ChatMsg::Transfer(TransferMsg::FirstFrame {
                    epoch, message, key, token, bytes: bytes.clone(), dimensions, image: first,
                }));
            }
            let wait = crate::performance::timer("gif_decode_wait_us");
            let Ok(animation_slot) = animations.acquire_owned().await else { return; };
            let Ok(decode_slot) = decoders.acquire_owned().await else { return; };
            drop(wait);
            let encoded = bytes.clone();
            let (animation, fallback) = tokio::task::spawn_blocking(move || {
                let (_animation, _decoder) = (animation_slot, decode_slot);
                let _timing = crate::performance::timer("gif_decode_us");
                let animation = crate::media::animation::PreparedAnimation::decode(&encoded);
                let fallback = animation.is_none().then(|| crate::media::animation::PreparedAnimation::first(&encoded)).flatten();
                (animation, fallback)
            }).await.unwrap_or_default();
            let _ = output.send(ChatMsg::Transfer(TransferMsg::Decoded {
                epoch, message, key, token, bytes, dimensions, preview_only: false, image: fallback, animation,
            }));
        } else if gif_video && image.is_none() && crate::media::gif_links::is_video(&bytes) {
            let Ok(_animation) = animations.acquire_owned().await else { return; };
            let Ok(_decoder) = decoders.acquire_owned().await else { return; };
            let encoded = bytes.clone();
            let result = async {
                let file = gif_file(encoded).await?;
                crate::media::animation::PreparedAnimation::video(&file.0).await
            }.await.ok();
            let _ = output.send(ChatMsg::Transfer(TransferMsg::AnimationReady {
                epoch, message, key, token, from_video: true, encoded: Some(bytes), result,
            }));
        } else {
            let _ = output.send(ChatMsg::Transfer(TransferMsg::Decoded {
                epoch, message, key, token, bytes, dimensions, preview_only: false, image, animation: None,
            }));
        }
    }));
}

#[cfg(test)]
pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    use actions::tests::{mock,pump,until};
    use adw::prelude::*;
    let (api,backend,server) = tokio::runtime::Handle::current().block_on(mock());
    let channel = Uuid::new_v4();
    let user = Uuid::new_v4();
    let now = chrono::Utc::now();
    let keys: Vec<_> = (0..31).map(|_| Uuid::new_v4()).collect();
    let target: Channel = serde_json::from_value(serde_json::json!({"id":channel,"name":"Loading pipeline","created_at":now})).unwrap();
    let attachment = |key| serde_json::json!({"id":key,"mime_type":"image/png","original_file_name":"image.png","size_bytes":128,"created_at":now});
    let messages:Vec<Message> = keys.chunks(8).enumerate().map(|(index,keys)|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":now+chrono::Duration::seconds(index as i64),"attachments":keys.iter().map(attachment).collect::<Vec<_>>()})).unwrap()).collect();
    use base64::Engine;
    let png = base64::engine::general_purpose::STANDARD.decode(actions::tests::tiny_png_base64()).unwrap();
    backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{}/thumbnail", keys[30]), png.clone());
    let chat = ChatModel::builder().launch(ChatInit { api:Some(api),active_channel:Some(target),messages,user_id:Some(user),..Default::default() }).detach();
    // Thirty existing textures must not consume the admission budget for the
    // thirty-first attachment across the materialized messages.
    { let mut state = chat.state().get_mut(); for key in &keys[..30] { state.model.transfers.cache(*key, &png); } }
    chat.emit(ChatMsg::SetAccess { user_id:user,access:crate::models::Access::direct() });
    let window = adw::Window::builder().default_width(850).default_height(800).content(chat.widget()).build();
    window.present();
    until(context, || chat.model().transfers.textures.contains_key(&keys[30]));
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.starts_with("/attachments/")).count(),1);
    window.set_content(None::<&gtk::Widget>); window.close(); drop(chat); server.abort(); pump(context);
    progressive(context);
    poster_priority(context);
}

#[cfg(test)]
fn poster_priority(context: &gtk::glib::MainContext) {
    use actions::tests::{mock,pump,until};
    use adw::prelude::*;
    use base64::Engine;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let now=chrono::Utc::now();let channel=Uuid::new_v4();let user=Uuid::new_v4();
    let missing:Vec<_>=(0..8).map(|_|Uuid::new_v4()).collect();
    let thumb=Uuid::new_v4();let still=Uuid::new_v4();
    let png=base64::engine::general_purpose::STANDARD.decode(actions::tests::tiny_png_base64()).unwrap();
    {let mut state=backend.lock().unwrap();for key in [thumb,still]{state.attachment_thumbnails.insert(format!("/attachments/{key}/thumbnail"),png.clone());}}
    let mut attachments:Vec<_>=missing.iter().map(|key|serde_json::json!({"id":key,"mime_type":"video/mp4","original_file_name":"clip.mp4","size_bytes":128,"created_at":now})).collect();
    attachments.push(serde_json::json!({"id":thumb,"mime_type":"video/mp4","original_file_name":"thumb.mp4","size_bytes":128,"thumbnail_id":Uuid::new_v4(),"created_at":now}));
    attachments.push(serde_json::json!({"id":still,"mime_type":"image/png","original_file_name":"image.png","size_bytes":128,"created_at":now}));
    let message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":now,"attachments":attachments})).unwrap();
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Poster priority","created_at":now})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![message],user_id:Some(user),..Default::default()}).detach();
    let held=tokio::runtime::Handle::current().block_on(chat.model().transfers.poster_limiter.clone().acquire_owned()).unwrap();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(700).content(chat.widget()).build();window.present();
    until(context,||[thumb,still].iter().all(|key|chat.model().transfers.textures.contains_key(key)));
    assert_eq!(missing.iter().filter(|key|chat.model().transfers.requested.contains(key)).count(),2,"slow poster work cannot fill the pending image queue");
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,path,_)|missing.iter().any(|key|path==&format!("/attachments/{key}"))),"both images load with local poster extraction blocked");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);drop(held);server.abort();pump(context);
}

#[cfg(test)]
fn progressive(context: &gtk::glib::MainContext) {
    use actions::tests::{mock,pump,until};
    use adw::prelude::*;
    use base64::Engine;
    let (api,backend,server) = tokio::runtime::Handle::current().block_on(mock());
    let channel = Uuid::new_v4(); let user = Uuid::new_v4(); let now = chrono::Utc::now();
    let gifs = [Uuid::new_v4(), Uuid::new_v4()]; let still = Uuid::new_v4();
    let payload = base64::engine::general_purpose::STANDARD.encode(crate::media::animation::fixture());
    let png = base64::engine::general_purpose::STANDARD.decode(actions::tests::tiny_png_base64()).unwrap();
    backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{still}/thumbnail"),png);
    let message: Message = serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":now,
        "attachments":[{"id":still,"mime_type":"image/png","original_file_name":"image.png","size_bytes":128,"created_at":now}],
        "embeds":gifs.iter().map(|id|serde_json::json!({"id":id,"fetch_method":"gif","thumbnail":{"mime_type":"image/gif","width":8,"height":8},"image_data":payload,"created_at":now})).collect::<Vec<_>>()})).unwrap();
    let target: Channel = serde_json::from_value(serde_json::json!({"id":channel,"name":"Progressive GIFs","created_at":now})).unwrap();
    let chat = ChatModel::builder().launch(ChatInit { api:Some(api),active_channel:Some(target),messages:vec![message],user_id:Some(user),..Default::default() }).detach();
    let held = chat.model().transfers.animation_limiter.clone().try_acquire_owned().unwrap();
    chat.emit(ChatMsg::SetAccess { user_id:user,access:crate::models::Access::direct() });
    let window = adw::Window::builder().default_width(850).default_height(800).content(chat.widget()).build(); window.present();
    until(context, || gifs.iter().chain([&still]).all(|key|chat.model().transfers.textures.contains_key(key)));
    assert!(chat.model().transfers.animations.is_empty(),"first frames and the still image display while full animation decoding is blocked");
    assert_eq!(chat.model().transfers.decode_limiter.available_permits(),2,"waiting animations cannot reserve the thumbnail decoders");
    let frames = gifs.map(|key|chat.model().transfers.textures[&key].downgrade());
    drop(held);
    until(context, || gifs.iter().all(|key|chat.model().transfers.animations.contains_key(key)) && chat.model().media_render_pending.is_none());
    // EN-US: GTK may retain the old snapshot until the next frame, after the model has already rendered.
    // PT-BR: O GTK pode manter o snapshot anterior até o próximo quadro, após a renderização do modelo.
    until(context, || frames.iter().all(|frame|frame.upgrade().is_none()));
    assert!(frames.iter().all(|frame|frame.upgrade().is_none()),"full animations release the temporary first-frame textures");
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p.starts_with("/attachments/")||p.starts_with("/embeds/")).count(),1);
    window.set_content(None::<&gtk::Widget>); window.close(); drop(chat); server.abort(); pump(context);
}
