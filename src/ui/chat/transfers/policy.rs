//! EN-US: Item-local media status, explicit admission and bounded retention controls.
//! PT-BR: Estado por mídia, carregamento explícito e controles de retenção limitada.
use super::*;
use crate::media::policy::{Preferences,Loading,Usage};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub(super) enum FileStatus {Downloading,Saved,Failed}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub(super) enum Phase {Downloading,Processing,Failed}
impl Transfers {
    pub(super) fn allows(&self,key:Uuid)->bool{self.explicit.contains(&key)||(self.preferences.loading==Loading::Automatic&&!self.cache_cleared&&!self.exhausted.contains(&key))}
    pub(super) fn processing(&mut self,key:Uuid){self.phases.insert(key,Phase::Processing);self.dirty.insert(key);self.revision=self.revision.wrapping_add(1);}
    pub(in crate::ui::chat) fn apply_preferences(&mut self,settings:Preferences){
        if self.preferences.loading!=settings.loading{self.cache_cleared=false;}
        self.preferences=settings;if !settings.low_memory{self.exhausted.clear();}self.encoded.set_low_memory(settings.low_memory);self.resident.set_low_memory(settings.low_memory);
        if settings.loading==Loading::OnClick{let keys:Vec<_>=self.media_jobs.keys().filter(|key|!self.explicit.contains(key)).copied().collect();for key in keys{if let Some(jobs)=self.media_jobs.remove(&key){for job in jobs{job.abort();}}self.requested.remove(&key);self.image_tokens.remove(&key);self.preview_tokens.remove(&key);self.gif_requested.remove(&key);self.video_requests.retain(|(_,id),_|*id!=key);self.phases.remove(&key);}}
        self.prune_textures();self.prune_animations();
        let limit=if settings.low_memory{16<<20}else{128<<20};while self.video_files.values().map(|(_,size)|*size).sum::<u64>()>limit{if let Some(key)=self.video_order.pop_front(){self.video_files.remove(&key);}else{break;}}
        self.dirty.extend(self.sizes.keys().copied());self.revision=self.revision.wrapping_add(1);
    }
    pub(in crate::ui::chat) fn usage(&self)->Usage{
        // EN-US: Count shared preview textures once; this excludes renderer allocations and open originals.
        // PT-BR: Conta texturas compartilhadas uma vez; exclui alocações do renderizador e originais abertos.
        let mut textures=HashMap::new();self.resident.collect_usage(&mut textures);for texture in self.textures.values(){textures.insert(texture.as_ptr() as usize,texture.width() as u64*texture.height() as u64*4);}
        let animations=self.animations.values().map(|(a,bytes)|{let first=a.first();(*bytes as u64).saturating_sub(if textures.contains_key(&(first.as_ptr() as usize)){first.width() as u64*first.height() as u64*4}else{0})}).sum::<u64>();
        Usage{memory_bytes:textures.values().sum::<u64>()+animations+self.encoded.memory_bytes() as u64+self.payloads.values().map(|(_,data)|data.len() as u64).sum::<u64>(),temporary_bytes:self.video_files.values().map(|(_,size)|*size).sum(),disk_bytes:0}
    }
    pub(in crate::ui::chat) fn clear_cache(&mut self){
        // EN-US: Invalidate preview completions, preserving user saves, uploads and already-open media owners.
        // PT-BR: Invalida resultados de prévias, preservando salvamentos, envios e mídias já abertas.
        for (_,jobs) in self.media_jobs.drain(){for job in jobs{job.abort();}}
        self.encoded.clear();self.resident=Default::default();self.resident.set_low_memory(self.preferences.low_memory);
        self.textures.clear();self.animations.clear();self.payloads.clear();self.payload_order.clear();self.video_files.clear();self.video_order.clear();self.order.clear();
        self.phases.clear();self.video_failures.clear();self.failed.clear();self.requested.clear();self.image_tokens.clear();self.preview_tokens.clear();self.gif_requested.clear();self.gif_resolved.clear();self.video_requests.clear();self.resolved.clear();self.static_previews.clear();self.poster_attempted.clear();
        self.explicit.clear();self.exhausted.clear();self.cache_cleared=true;self.dirty.extend(self.sizes.keys().copied());self.revision=self.revision.wrapping_add(1);
    }
    pub(super) fn prune_animations(&mut self){
        let limit=if self.preferences.low_memory{8<<20}else{48<<20};
        while self.animations.values().map(|(_,n)|*n).sum::<usize>()>limit{let old=self.order.iter().find(|id|self.animations.contains_key(id)&&!self.wanted.contains(id)).or_else(||self.order.iter().find(|id|self.animations.contains_key(id))).copied();let Some(old)=old else{break;};self.animations.remove(&old);self.dirty.insert(old);}
    }
    pub(super) fn status(&self,key:Uuid,sender:&ComponentSender<ChatModel>)->gtk::Box{
        let body=gtk::Box::new(gtk::Orientation::Vertical,6);body.set_halign(gtk::Align::Center);body.set_valign(gtk::Align::Center);body.set_widget_name(&format!("media-status-{key}"));
        let phase=self.phases.get(&key);let failed=self.failed.contains_key(&key)||phase==Some(&Phase::Failed);
        let text=if failed{crate::i18n::tr("Não foi possível carregar a mídia.")}else if !self.allows(key){crate::i18n::tr("Carregar mídia")}else if phase==Some(&Phase::Processing){crate::i18n::tr("Processando mídia…")}else{crate::i18n::tr("Baixando mídia…")};
        if failed{body.append(&gtk::Image::from_icon_name("papo-image-missing-symbolic"));}
        if !failed&&self.allows(key){body.append(&gtk::Spinner::builder().spinning(true).build());}
        let label=gtk::Label::new(Some(text));label.set_wrap(true);label.set_justify(gtk::Justification::Center);label.update_property(&[gtk::accessible::Property::Label(text)]);body.append(&label);
        if failed||!self.allows(key){let button=gtk::Button::with_label(if failed{crate::i18n::tr("Tentar novamente")}else{crate::i18n::tr("Carregar mídia")});button.set_widget_name(&format!("load-media-{key}"));let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::RetryImage(key)));});body.append(&button);}
        body
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[tokio::test]async fn click_mode_stops_unrequested_work_and_clear_does_not_cancel_uploads(){
        let mut media=Transfers::default();let key=Uuid::new_v4();let job=tokio::spawn(std::future::pending::<()>());let abort=job.abort_handle();media.media_job(key,job);media.requested.insert(key);media.image_tokens.insert(key,Uuid::new_v4());
        media.apply_preferences(Preferences{loading:Loading::OnClick,low_memory:false});tokio::task::yield_now().await;assert!(abort.is_finished());assert!(!media.allows(key)&&!media.image_tokens.contains_key(&key));assert!(!media.request(key));
        media.explicit.insert(key);assert!(media.request(key));media.encoded.insert(key,Uuid::new_v4(),Uuid::new_v4(),vec![1u8;1024].into());assert_eq!(media.usage().memory_bytes,1024);
        let upload=tokio::spawn(std::future::pending::<()>());let active=upload.abort_handle();media.uploads.insert(Uuid::new_v4(),upload);
        media.clear_cache();tokio::task::yield_now().await;assert!(!active.is_finished());assert_eq!(media.usage(),Usage::default());assert!(!media.allows(key));media.explicit.insert(key);assert!(media.allows(key));
    }
    #[test]fn low_memory_compressed_budget_is_hard_even_for_recent_items(){
        let mut cache=encoded::Cache::default();let channel=Uuid::new_v4();let ids:Vec<_>=(0..8).map(|_|Uuid::new_v4()).collect();cache.set_recent(channel,ids.iter().copied().collect());
        for id in &ids{cache.insert(*id,channel,Uuid::new_v4(),vec![0;4<<20].into());}assert_eq!(cache.memory_bytes(),32<<20);cache.set_low_memory(true);assert!(cache.memory_bytes()<=8<<20);
        let bytes=cache.memory_bytes();cache.insert(Uuid::new_v4(),channel,Uuid::new_v4(),vec![0;12<<20].into());assert_eq!(cache.memory_bytes(),bytes,"oversized entry must not displace useful previews");cache.clear();cache.insert(Uuid::new_v4(),channel,Uuid::new_v4(),vec![0;12<<20].into());assert_eq!(cache.memory_bytes(),0,"clear preserves the selected budget");
    }
    #[test]fn old_config_defaults_and_media_choices_do_not_change_motion(){
        let config:crate::config::AppConfig=toml::from_str("motion = 'reduced'").unwrap();assert_eq!(config.media,Preferences::default());assert_eq!(config.motion,crate::config::Motion::Reduced);
        let config:crate::config::AppConfig=toml::from_str("motion = 'system'\n[media]\nloading = 'on_click'\nlow_memory = true").unwrap();assert_eq!(config.media.loading,Loading::OnClick);assert!(config.media.low_memory);assert_eq!(config.motion,crate::config::Motion::System);
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,pump,until,descendants};use adw::prelude::*;use base64::Engine;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let channel=Uuid::new_v4();let user=Uuid::new_v4();let keys:[Uuid;4]=std::array::from_fn(|_|Uuid::new_v4());let now=chrono::Utc::now();
    let png=base64::engine::general_purpose::STANDARD.decode(actions::tests::tiny_png_base64()).unwrap();backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{}/thumbnail",keys[0]),png.clone());
    let mut attachments:Vec<_>=keys.iter().enumerate().map(|(i,key)|serde_json::json!({"id":key,"mime_type":"image/png","original_file_name":"photo.png","size_bytes":64,"thumbnail_width":320,"thumbnail_height":160,"moderation_status":match i{2=>"blocked",3=>"pending",_=>"clean"},"created_at":now})).collect();
    let document=Uuid::new_v4();attachments.push(serde_json::json!({"id":document,"mime_type":"application/pdf","original_file_name":"notes.pdf","size_bytes":64,"created_at":now}));let message_id=Uuid::new_v4();
    let video=Uuid::new_v4();attachments.push(serde_json::json!({"id":video,"mime_type":"video/webm","original_file_name":"clip.webm","size_bytes":64,"thumbnail_width":320,"thumbnail_height":160,"created_at":now}));
    let message=serde_json::from_value(serde_json::json!({"id":message_id,"channel_id":channel,"created_at":now,"attachments":attachments})).unwrap();let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Media policies","created_at":now})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![message],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::MediaPreferences(Preferences{loading:Loading::OnClick,low_memory:false}));chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(900).default_height(800).content(chat.widget()).build();window.present();performance::settle(context);
    let requests=|key:Uuid|backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path==&format!("/attachments/{key}/thumbnail")).count();assert!(keys.iter().all(|k|requests(*k)==0));
    let has_label=|text:&str|descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()==text);
    assert!(has_label("Mídia bloqueada pela moderação")&&has_label("Aguardando moderação"));
    let button=|key:Uuid|descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.widget_name()==format!("load-media-{key}"))).unwrap();
    let row=||descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with(&format!("attachment-{}-",keys[0]))).unwrap();let before=row().height();
    let held=tokio::runtime::Handle::current().block_on(chat.model().transfers.decode_limiter.clone().acquire_many_owned(2)).unwrap();button(keys[0]).emit_clicked();until(context,||has_label("Processando mídia…")&&row().height()>0);assert_eq!(row().height(),before,"processing preserves known layout dimensions");assert_eq!(requests(keys[1]),0);drop(held);
    until(context,||chat.model().transfers.textures.contains_key(&keys[0]));button(keys[1]).emit_clicked();until(context,||chat.model().transfers.failed.contains_key(&keys[1])&&has_label("Não foi possível carregar a mídia."));assert_eq!(requests(keys[1]),1);
    backend.lock().unwrap().attachment_thumbnails.insert(format!("/attachments/{}/thumbnail",keys[1]),png);button(keys[1]).emit_clicked();until(context,||chat.model().transfers.textures.contains_key(&keys[1]));assert_eq!(requests(keys[0]),1);assert_eq!(requests(keys[1]),2);assert_eq!(requests(keys[2]),0);assert_eq!(requests(keys[3]),0);
    let destination=std::env::temp_dir().join(format!("papo-policy-file-{}.pdf",Uuid::new_v4()));let _cleanup=crate::api::features::TemporaryFile(destination.clone());
    {let mut state=backend.lock().unwrap();state.failed_media.insert(format!("/attachments/{document}"));state.media_delay=std::time::Duration::from_millis(200);}
    let save=||ChatMsg::Transfer(TransferMsg::Save{epoch:chat.model().transfers.epoch,message:message_id,attachment:document,path:destination.clone()});chat.emit(save());until(context,||has_label("Baixando arquivo…"));until(context,||chat.model().transfers.file_status.get(&document)==Some(&FileStatus::Failed));assert!(has_label("Falha no download. Clique no arquivo para tentar novamente."));assert!(chat.model().draft.error.is_none(),"download errors belong to their file tile");
    {let mut state=backend.lock().unwrap();state.failed_media.remove(&format!("/attachments/{document}"));state.media_delay=std::time::Duration::ZERO;state.attachment_thumbnails.insert(format!("/attachments/{document}"),b"%PDF-1.4 fixture".to_vec());}chat.emit(save());until(context,||chat.model().transfers.file_status.get(&document)==Some(&FileStatus::Saved));assert_eq!(std::fs::read(&destination).unwrap(),b"%PDF-1.4 fixture");
    {let mut state=chat.state().get_mut();state.model.transfers.cache_image(video,crate::media::PreparedImage{width:320,height:160,pixels:vec![255;320*160*4]});}backend.lock().unwrap().failed_media.insert(format!("/attachments/{video}"));
    chat.emit(ChatMsg::Transfer(TransferMsg::AttachmentVideo{message:message_id,attachment:video}));until(context,||chat.model().transfers.video_failures.contains_key(&video));assert!(chat.model().transfers.textures.contains_key(&video));button(video).emit_clicked();until(context,||backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path==&format!("/attachments/{video}")).count()==2&&chat.model().transfers.video_failures.contains_key(&video));assert_eq!(requests(keys[0]),1,"video retry must leave unrelated images alone");
    assert!(chat.model().transfers.usage().memory_bytes>0);chat.emit(ChatMsg::ClearMediaCache(Uuid::new_v4()));pump(context);assert_eq!(chat.model().transfers.usage(),Usage::default());let total=keys.iter().map(|k|requests(*k)).sum::<usize>();performance::settle(context);assert_eq!(keys.iter().map(|k|requests(*k)).sum::<usize>(),total,"clear must not immediately refill the cache");button(keys[0]).emit_clicked();until(context,||chat.model().transfers.textures.contains_key(&keys[0]));
    {let mut model=chat.state().get_mut();for _ in 0..32{model.model.transfers.cache_image(Uuid::new_v4(),crate::media::PreparedImage{width:512,height:512,pixels:vec![255;512*512*4]});}}
    chat.emit(ChatMsg::MediaPreferences(Preferences{loading:Loading::OnClick,low_memory:true}));pump(context);assert!(chat.model().transfers.textures.len()<=16);assert!(chat.model().transfers.usage().memory_bytes<=24<<20);
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
