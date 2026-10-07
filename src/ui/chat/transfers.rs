//! Streaming transfers and bounded media textures owned by the chat session.
use super::*;
#[cfg(test)] use adw::prelude::AdwWindowExt;
use crate::api::features:: {
    TemporaryFile, UploadFile, MAX_FILE
};
use std:: {
    collections:: {
        HashSet, VecDeque
    }, path::PathBuf
};
#[derive(Debug)]
pub enum TransferMsg {
    ImageReady{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:Option<crate::media::PreparedImage>},
    CloseVideo{message:Uuid,key:Uuid},AttachmentVideo{message:Uuid,attachment:Uuid},
    Preview{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:anyhow::Result<LinkPreview>},
    Choose, Selected {
        epoch: Uuid, channel: Uuid, result: anyhow::Result<Vec<UploadFile>>
    }, OpenImage{message:Uuid,key:Uuid}, Remove(usize), RetryImage(Uuid), Cancel, Progress {
        id: Uuid, bytes: u64
    }, Reveal {
        message: Uuid, attachment: Uuid
    }, Download {
        message: Uuid, attachment: Attachment
    }, Save {
        epoch: Uuid, message: Uuid, attachment: Uuid, path: PathBuf
    }, Video {
        message: Uuid, preview: Uuid
    }, Bytes {
        epoch: Uuid, message: Uuid, key: Uuid, preview: bool, result: anyhow::Result<Vec<u8>>
    }, File {
        epoch: Uuid, message: Uuid, key: Uuid, token:Option<Uuid>, destination: Option<PathBuf>, result: anyhow::Result<TemporaryFile>
    }, Saved {
        epoch: Uuid, message: Uuid, result: anyhow::Result<()>
    },
}
pub(super) struct Transfers {
    image_tokens:HashMap<Uuid,Uuid>,preview_tokens:HashMap<Uuid,Uuid>, epoch: Uuid, limiter: std::sync::Arc<tokio::sync::Semaphore>, textures: HashMap<Uuid, gtk::gdk::Texture>, order: VecDeque<Uuid>, requested: HashSet<Uuid>, revealed: HashSet<Uuid>, jobs: Vec<tokio::task::JoinHandle<()>>, uploads: HashMap<Uuid, tokio::task::JoinHandle<()>>, pub progress: u64, video_requests:HashMap<(Uuid,Uuid),Uuid>,videos: Vec<Playback>,images:Vec<(Uuid,Uuid,gtk::Window)>,
}
struct Playback {message:Uuid,key:Uuid,view:gtk::Box,media:gtk::MediaFile,file:TemporaryFile}
impl Drop for Playback{
    fn drop(&mut self){
        // Detach GTK's controls before closing their source: unrealizing a Video
        // can otherwise send pause requests into a player already being torn down.
        self.media.set_muted(true);self.media.pause();
        let mut child=self.view.first_child();while let Some(widget)=child{if let Some(video)=widget.downcast_ref::<gtk::Video>(){video.set_media_stream(None::<&gtk::MediaStream>);}child=widget.next_sibling();}
        self.media.clear();
    }
}
impl Default for Transfers {
    fn default() -> Self {
        Self {
            image_tokens:Default::default(),preview_tokens:Default::default(), epoch: Uuid::new_v4(), limiter: std::sync::Arc::new(tokio::sync::Semaphore::new(4)), textures: HashMap::new(), order: VecDeque::new(), requested: HashSet::new(), revealed: HashSet::new(), jobs: Vec::new(), uploads: HashMap::new(), progress: 0, video_requests:Default::default(),videos: Vec::new(),images:Vec::new()
        }
    }
}
impl Drop for Transfers {
    fn drop(&mut self) {
        self.reset();
        for (_, job) in self.uploads.drain() {
            job.abort();
        }
    }
}
impl Transfers {
    pub(super) fn visual_state(&self,message:&Message)->Vec<(Uuid,bool,usize)>{
        message.attachments.iter().flatten().map(|a|(a.id,self.visible(a),self.textures.get(&a.id).map_or(0,|t|t.as_ptr() as usize)))
            .chain(message.previews.iter().flatten().map(|p|(p.id,true,self.textures.get(&p.id).map_or(0,|t|t.as_ptr() as usize)))).chain(self.videos.iter().filter(|p|p.message==message.id).map(|p|(p.key,false,p.view.as_ptr() as usize))).collect()
    }
    pub fn invalidate_message(&mut self, message: Uuid) {
        self.images.retain(|(id,_,w)|{if *id==message{w.close();false}else{true}});
        self.video_requests.retain(|(id,_),_|*id!=message);self.videos.retain(|p|p.message!=message);
    }
    pub fn invalidate(&mut self, id: Uuid) {
        self.images.retain(|(_,key,w)|{if *key==id{w.close();false}else{true}});
        self.video_requests.retain(|(_,key),_|*key!=id);self.videos.retain(|p|p.key!=id);
        self.revealed.remove(&id);
        self.textures.remove(&id);
        self.requested.remove(&id);self.preview_tokens.remove(&id);self.image_tokens.remove(&id);
    }
    pub fn reset(&mut self) {
        self.epoch = Uuid::new_v4();
        self.requested.clear();self.preview_tokens.clear();self.image_tokens.clear();
        self.revealed.clear();
        self.textures.clear();
        for (_,_,window) in self.images.drain(..){window.close();}
        self.order.clear();
        for job in self.jobs.drain(..) {
            job.abort();
        } self.video_requests.clear();self.videos.clear();
    }
    pub fn visible(&self, a: &Attachment) -> bool {
        match a.moderation_status.as_deref() {
            Some("blocked"|"pending"|"processing") => false, Some("sensitive") => self.revealed.contains(&a.id), _ => true
        }
    }
    #[cfg(test)]
    fn cache(&mut self,id:Uuid,bytes:&[u8]){if let Some(image)=crate::media::PreparedImage::decode(bytes,680,460){self.cache_image(id,image);}}
    fn cache_image(&mut self,id:Uuid,image:crate::media::PreparedImage){
            let texture=image.texture();
            self.textures.insert(id, texture);
            self.order.retain(|key| *key != id);
            self.order.push_back(id);
            // 32 * 680 * 460 * 4 < 40 MiB decoded thumbnails.
            while self.order.len()>32 {
                if let Some(old) = self.order.pop_front() {
                    self.textures.remove(&old);
                }
            }
    }
}
enum ImageInput{Bytes(Vec<u8>),Base64(String)}
fn queue_image(transfers:&mut Transfers,sender:&ComponentSender<ChatModel>,message:Uuid,key:Uuid,input:ImageInput){
    let token=Uuid::new_v4();transfers.image_tokens.insert(key,token);let epoch=transfers.epoch;let limiter=transfers.limiter.clone();let output=sender.input_sender().clone();
    transfers.jobs.push(tokio::spawn(async move{
        let Ok(_permit)=limiter.acquire_owned().await else{return;};
        let result=tokio::task::spawn_blocking(move ||{let bytes=match input{ImageInput::Bytes(bytes)=>bytes,ImageInput::Base64(data)=>{use base64::Engine;if data.len()>6<<20{return None;}base64::engine::general_purpose::STANDARD.decode(data).ok()?}};crate::media::PreparedImage::decode(&bytes,680,460)}).await.ok().flatten();
        let _=output.send(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,message,key,token,result}));
    }));
}
impl ChatModel {
    pub(super) fn start_send(&mut self, sender: &ComponentSender<Self>, request_id: Uuid, channel_id: Uuid) {
        let Some(api) = self.actions.api.clone() else {
            self.draft.finish(request_id, Err("API indisponível".into()));
            return;
        };
        let request = crate::models::CreateMessageRequest {
            channel_id, content: Some(self.draft.text.trim().into()), reply_to: self.draft.reply.as_ref().map(|m|m.id)
        };
        let files = self.draft.files.clone();
        let s = sender.clone();
        let progress = sender.clone();
        self.transfers.progress = 0;
        self.transfers.uploads.retain(|_, job| !job.is_finished());
        self.transfers.uploads.insert(request_id, tokio::spawn(async move {
            let result = api.send_with_files(&request, &files, move |bytes|progress.input(ChatMsg::Transfer(TransferMsg::Progress {
                id: request_id, bytes
            }))).await;
            let result = result.map_err(|e| {
                let message = format!("Falha no envio: {e}. Confira o histórico antes de reenviar.");
                let _ = s.output(ChatOutput::ActionError(e));
                message
            });
            s.input(ChatMsg::SendFinished {
                request_id, channel_id, result
            });
        }));
    }
    pub(super) fn hydrate_media(&mut self, sender: &ComponentSender<Self>) {
        let Some(api) = self.actions.api.clone() else {
            return;
        };
        self.transfers.jobs.retain(|j|!j.is_finished());
        let epoch = self.transfers.epoch;
        // A normalized URL can share a preview ID across several messages.
        // Reuse full metadata already hydrated in this history for later summaries.
        let mut known:HashMap<Uuid,LinkPreview>=HashMap::new();
        for p in self.history.messages.iter().flat_map(|m|m.previews.iter().flatten()){
            let score=|p:&LinkPreview|usize::from(p.title.is_some())+usize::from(p.description.is_some())+usize::from(p.image_mime_type.is_some())+usize::from(p.video_url.is_some());
            if known.get(&p.id).is_none_or(|old|p.fetched_at>old.fetched_at||(p.fetched_at==old.fetched_at&&score(p)>score(old))){known.insert(p.id,p.clone());}
        }
        for m in &mut self.history.messages {
            for a in m.attachments.iter().flatten() {
                if a.mime_type.starts_with("image/") && self.transfers.visible(a)
                && self.transfers.requested.insert(a.id) {
                    let api = api.clone();
                    let s = sender.clone();
                    let key = a.id;
                    let message = m.id;
                    let limiter = self.transfers.limiter.clone();
                    self.transfers.jobs.push(tokio::spawn(async move {
                        let Ok(_permit) = limiter.acquire_owned().await else {
                            return;
                        };
                        let result = api.media_bytes(&format!("/attachments/{key}/thumbnail"), 4<<20).await;
                        s.input(ChatMsg::Transfer(TransferMsg::Bytes {
                            epoch, message, key, preview: false, result
                        }));
                    }));
                }
            }
            for p in m.previews.iter_mut().flatten() {
                if let Some(full)=known.get(&p.id){if full.fetched_at>=p.fetched_at{*p=full.clone();}}
                if let Some(data) = p.image_data.take() {
                    if self.transfers.requested.insert(p.id) {
                        queue_image(&mut self.transfers,sender,m.id,p.id,ImageInput::Base64(data));
                    }
                } else if self.transfers.requested.insert(p.id) {
                    let api = api.clone();
                    let s = sender.clone();
                    let key = p.id;
                    let message = m.id;
                    let token=Uuid::new_v4();self.transfers.preview_tokens.insert(key,token);
                    let limiter = self.transfers.limiter.clone();
                    self.transfers.jobs.push(tokio::spawn(async move {
                        let Ok(_permit) = limiter.acquire_owned().await else {
                            return;
                        };
                        let result=api.get_link_preview(key).await;
                        let _=s.input_sender().send(ChatMsg::Transfer(TransferMsg::Preview{epoch,message,key,token,result}));
                    }));
                }
            }
        }
    }
    pub(super) fn handle_transfer(&mut self, msg: TransferMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        match msg {
            TransferMsg::ImageReady{epoch,message,key,token,result}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token){return;}
                let valid=self.history.messages.iter().find(|m|m.id==message).is_some_and(|m|m.previews.iter().flatten().any(|p|p.id==key)||m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a)));
                self.transfers.image_tokens.remove(&key);if valid{if let Some(image)=result{self.transfers.cache_image(key,image);}}
            },
            TransferMsg::AttachmentVideo{message,attachment}=>{if self.access.read&&self.history.messages.iter().find(|m|m.id==message).is_some_and(|m|m.attachments.iter().flatten().any(|a|a.id==attachment&&a.mime_type.starts_with("video/")&&self.transfers.visible(a))){self.start_download(message,attachment,None,false,sender);}},
            TransferMsg::CloseVideo{message,key}=>{self.transfers.video_requests.remove(&(message,key));self.videos_retain(message,key);},
            TransferMsg::Preview{epoch,message,key,token,result}=>{
                if epoch!=self.transfers.epoch||self.transfers.preview_tokens.get(&key)!=Some(&token)||!self.access.read||!self.history.messages.iter().find(|m|m.id==message).is_some_and(|m|m.previews.iter().flatten().any(|p|p.id==key)){return;}
                match result{
                    Ok(mut preview) if preview.id==key=>{
                        if let Some(data)=preview.image_data.take(){queue_image(&mut self.transfers,sender,message,key,ImageInput::Base64(data));}
                        let ids:Vec<_>=self.history.messages.iter().filter(|m|m.previews.iter().flatten().any(|p|p.id==key)).map(|m|m.id).collect();
                        for id in ids{self.history.apply(Change::Preview(id,preview.clone()));}
                    },
                    Err(error)=>{if crate::api::is_session_error(&error){let _=sender.output(ChatOutput::ActionError(error));}},_=>{},
                }
            },
            TransferMsg::OpenImage{message,key}=>{
                if !self.access.read||!self.history.messages.iter().any(|m|m.id==message&&(m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a))||m.previews.iter().flatten().any(|p|p.id==key))){return;}
                let Some(texture)=self.transfers.textures.get(&key)else{return;};
                if let Some((_,_,window))=self.transfers.images.iter().find(|(m,k,w)|*m==message&&*k==key&&w.is_visible()){window.present();return;}
                let window=gtk::Window::builder().title("Imagem").default_width(900).default_height(680).build();if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
                let picture=gtk::Picture::for_paintable(texture);picture.set_can_shrink(true);picture.set_content_fit(gtk::ContentFit::Contain);window.set_child(Some(&picture));window.present();self.transfers.images.retain(|(_,_,w)|w.is_visible());self.transfers.images.push((message,key,window));
            },
            TransferMsg::Choose => {
                if self.draft.is_sending() || !self.access.attachments || !self.access.send {
                    return;
                }
                let Some(channel) = self.active_channel.as_ref().map(|c|c.id()) else {
                    return;
                };
                let epoch = self.transfers.epoch;
                let parent = root.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                let s = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    if let Ok(files) = gtk::FileDialog::builder().title("Adicionar arquivos").build().open_multiple_future(parent.as_ref()).await {
                        let paths: Vec<_>=(0..files.n_items()).filter_map(|i|files.item(i).and_then(|o|o.downcast::<gtk::gio::File>().ok()).and_then(|f|f.path())).collect();
                        tokio::spawn(async move {
                            let result = async {
                                let mut selected = Vec::new();
                                for path in paths {
                                    selected.push(UploadFile::inspect(path).await?);
                                }
                                Ok(selected)
                            }.await;
                            s.input(ChatMsg::Transfer(TransferMsg::Selected {
                                epoch, channel, result
                            }));
                        });
                    }
                });
            }
            TransferMsg::Selected {
                epoch, channel, result
            } => {
                if epoch!=self.transfers.epoch||self.active_channel.as_ref().map(|c|c.id())!=Some(channel)||self.draft.is_sending() {
                    return;
                }
                match result {
                    Ok(files) => {
                        let mut all = self.draft.files.clone();
                        all.extend(files);
                        match crate::api::features::validate_upload(Some(&self.draft.text), &all) {
                            Ok(()) => {
                                self.draft.files = all;
                                self.draft.error = None;
                            }, Err(e) => self.draft.error = Some(e.to_string())
                        }
                    }
                    Err(e) => self.draft.error = Some(e.to_string())
                }
            }
            TransferMsg::RetryImage(id) => {
                self.transfers.requested.remove(&id);
            }
            TransferMsg::Remove(index) => {
                if !self.draft.is_sending()&&index<self.draft.files.len() {
                    self.draft.files.remove(index);
                }
            }
            TransferMsg::Cancel => {
                if let Some(id) = self.draft.pending_id() {
                    if let Some(job) = self.transfers.uploads.remove(&id) {
                        job.abort();
                    }
                }
                self.draft.cancel();
            }
            TransferMsg::Progress {
                id, bytes
            } => {
                if self.draft.pending_id()==Some(id) {
                    self.transfers.progress = bytes;
                }
            }
            TransferMsg::Reveal {
                message, attachment
            } => {
                if self.history.messages.iter().any(|m|m.id==message) {
                    self.transfers.revealed.insert(attachment);
                    self.transfers.requested.remove(&attachment);
                }
            }
            TransferMsg::Bytes {
                epoch, message, key, preview, result
            } => {
                if epoch!=self.transfers.epoch {
                    return;
                }
                let valid = self.history.messages.iter().find(|m|m.id==message).is_some_and(|m|if preview {
                    m.previews.iter().flatten().any(|p|p.id==key)
                }
                else {
                    m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a))
                });
                if valid {
                    match result {
                        Ok(bytes) => queue_image(&mut self.transfers,sender,message,key,ImageInput::Bytes(bytes)), Err(e) => {
                            if crate::api::is_session_error(&e) {
                                let _ = sender.output(ChatOutput::ActionError(e));
                            }
                        }
                    }
                }
            }
            TransferMsg::Download {
                message, attachment
            } => {
                if !self.transfers.visible(&attachment) {
                    return;
                }
                let epoch = self.transfers.epoch;
                let parent = root.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                let s = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    if let Ok(file) = gtk::FileDialog::builder().title("Salvar arquivo").initial_name(&attachment.original_file_name).build().save_future(parent.as_ref()).await {
                        if let Some(path) = file.path() {
                            s.input(ChatMsg::Transfer(TransferMsg::Save {
                                epoch, message, attachment: attachment.id, path
                            }));
                        }
                    }
                });
            }
            TransferMsg::Save {
                epoch, message, attachment, path
            } => {
                if epoch!=self.transfers.epoch||!self.history.messages.iter().any(|m|m.id==message&&m.attachments.iter().flatten().any(|a|a.id==attachment&&self.transfers.visible(a))) {
                    return;
                }
                self.start_download(message, attachment, Some(path), false, sender);
            }
            TransferMsg::Video {
                message, preview
            } => {
                if self.access.read&&self.history.messages.iter().any(|m|m.id==message&&m.previews.iter().flatten().any(|p|p.id==preview)) {
                    self.start_download(message, preview, None, true, sender);
                }
            }
            TransferMsg::File {
                epoch, message, key, token,destination, result
            } => {
                if epoch!=self.transfers.epoch || !self.access.read||!self.history.messages.iter().any(|m|m.id==message&&if destination.is_some() {
                    m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a))
                }
                else {
                    m.previews.iter().flatten().any(|p|p.id==key&&p.video_url.is_some())||m.attachments.iter().flatten().any(|a|a.id==key&&a.mime_type.starts_with("video/")&&self.transfers.visible(a))
                }) {
                    return;
                }
                if let Some(token)=token{if self.transfers.video_requests.get(&(message,key))!=Some(&token){return;}self.transfers.video_requests.remove(&(message,key));}
                match result {
                    Ok(file) => {
                        self.draft.error = None;
                        if let Some(destination) = destination {
                            let s = sender.clone();
                            self.transfers.jobs.push(tokio::spawn(async move {
                                let result = crate::api::features::save_download(file, destination).await;
                                s.input(ChatMsg::Transfer(TransferMsg::Saved {
                                    epoch, message, result
                                }));
                            }));
                        }
                        else {
                            if self.transfers.videos.len()>=2{self.transfers.videos.remove(0);}
                            let view=gtk::Box::new(gtk::Orientation::Vertical,6);view.set_widget_name(&format!("video-{message}-{key}"));
                            let close=gtk::Button::with_label("Fechar vídeo");close.set_halign(gtk::Align::End);let input=sender.input_sender().clone();close.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::CloseVideo{message,key}));});view.append(&close);
                            let media=gtk::MediaFile::for_filename(&file.0);let video=gtk::Video::for_media_stream(Some(&media));video.set_size_request(0,260);video.set_hexpand(true);view.append(&video);
                            let error=gtk::Label::new(None);error.set_wrap(true);error.add_css_class("error");view.append(&error);let error_now=error.clone();media.connect_error_notify(move |m|{if let Some(e)=m.error(){error.set_text(&format!("Não foi possível reproduzir: {e}"));}});if let Some(e)=media.error(){error_now.set_text(&format!("Não foi possível reproduzir: {e}"));}
                            media.play();self.transfers.videos.push(Playback{message,key,view,media,file});
                        }
                    }, Err(e) => {
                        self.draft.error = Some(e.to_string());
                        let _ = sender.output(ChatOutput::ActionError(e));
                    }
                }
            }
            TransferMsg::Saved {
                epoch, message, result
            } => {
                if epoch!=self.transfers.epoch||!self.history.messages.iter().any(|m|m.id==message) {
                    return;
                }
                self.draft.error = Some(match result {
                    Ok(()) => "Arquivo salvo.".into(), Err(e) => format!("Falha ao salvar: {e}")
                });
            }
        }
    }
    fn videos_retain(&mut self,message:Uuid,key:Uuid){self.transfers.videos.retain(|p|p.message!=message||p.key!=key);}
    fn start_download(&mut self, message: Uuid, key: Uuid, destination: Option<PathBuf>, video: bool, sender: &ComponentSender<Self>) {
        let Some(api) = self.actions.api.clone() else {
            return;
        };
        let inline=destination.is_none();
        if inline&&(self.transfers.video_requests.contains_key(&(message,key))||self.transfers.videos.iter().any(|p|p.message==message&&p.key==key)){return;}
        let token=if inline{let token=Uuid::new_v4();self.transfers.video_requests.insert((message,key),token);Some(token)}else{None};
        let s = sender.clone();
        let epoch = self.transfers.epoch;
        self.draft.error = Some(if video {
            "Carregando vídeo…"
        }
        else {
            "Baixando arquivo…"
        }.into());
        let endpoint = if video {
            format!("/link-previews/{key}/video")
        }
        else {
            format!("/attachments/{key}")
        };
        self.transfers.jobs.push(tokio::spawn(async move {
            let result = api.download_temporary(&endpoint, if video {
                256<<20
            }
            else {
                MAX_FILE
            }).await;
            s.input(ChatMsg::Transfer(TransferMsg::File {
                epoch, message, key,token,destination, result
            }));
        }));
    }
}
pub(super) fn attachment_widget(a: &Attachment, message: Uuid, media: &Transfers, sender: &ComponentSender<ChatModel>)->gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_halign(gtk::Align::Start);if !a.mime_type.starts_with("image/"){card.add_css_class("card");}
    let label=gtk::Label::new(Some(&format!("{} · {}",a.original_file_name,format_file_size(a.size_bytes))));label.set_xalign(0.0);label.set_ellipsize(pango::EllipsizeMode::End);label.set_max_width_chars(45);label.add_css_class("caption");card.append(&label);
    if media.visible(a) {
        if a.mime_type.starts_with("video/"){append_video(&card,media,message,a.id,false,sender);}
        if let Some(t) = media.textures.get(&a.id) {
            let image = crate::media::preview::picture(t);
            image.set_can_shrink(true);
            let open=gtk::Button::new();open.set_has_frame(false);open.set_halign(gtk::Align::Start);open.add_css_class("papo-inline-image");open.set_tooltip_text(Some("Abrir imagem"));open.set_child(Some(&image));let s=sender.clone();let key=a.id;open.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::OpenImage{message,key})));card.append(&open);
        }
        else if a.mime_type.starts_with("image/") {
            let button = gtk::Button::with_label("Carregar imagem");
            let key = a.id;
            let s = sender.clone();
            button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));
            card.append(&button);
        }
        let button = gtk::Button::with_label("Baixar arquivo");
        button.add_css_class("flat");button.set_halign(gtk::Align::Start);
        let a = a.clone();
        let s = sender.clone();
        button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::Download {
            message, attachment: a.clone()
        })));
        card.append(&button);
    } else if a.moderation_status.as_deref()==Some("sensitive") {
        let button = gtk::Button::with_label("Conteúdo sensível — revelar");
        let attachment = a.id;
        let s = sender.clone();
        button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::Reveal {
            message, attachment
        })));
        card.append(&button);
    }
    else {
        card.append(&gtk::Label::new(Some("Aguardando moderação")));
    }
    card
}
fn append_video(card:&gtk::Box,media:&Transfers,message:Uuid,key:Uuid,preview:bool,sender:&ComponentSender<ChatModel>){
    if let Some(playback)=media.videos.iter().find(|v|v.message==message&&v.key==key){
        let playing=playback.media.is_playing();
        if let Some(parent)=playback.view.parent().and_downcast::<gtk::Box>(){parent.remove(&playback.view);}else if let Some(parent)=playback.view.parent().and_downcast::<adw::Clamp>(){parent.set_child(None::<&gtk::Widget>);}
        let clamp=adw::Clamp::new();clamp.set_maximum_size(720);clamp.set_tightening_threshold(720);clamp.set_child(Some(&playback.view));card.append(&clamp);if playing{playback.media.play();}
    }else{
        let button=gtk::Button::with_label("Reproduzir vídeo");button.set_sensitive(!media.video_requests.contains_key(&(message,key)));let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(if preview{TransferMsg::Video{message,preview:key}}else{TransferMsg::AttachmentVideo{message,attachment:key}}));});card.append(&button);
    }
}

pub(super) fn preview_widget(p: &LinkPreview, message: Uuid, media: &Transfers, sender: &ComponentSender<ChatModel>)->gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_halign(gtk::Align::Start);
    card.add_css_class("card");card.add_css_class("papo-link-preview");
    if let Some(provider)=&p.provider_name{let label=gtk::Label::new(Some(provider));label.set_xalign(0.0);label.add_css_class("caption");label.add_css_class("dim-label");card.append(&label);}
    if let Ok(url) = reqwest::Url::parse(&p.url) {
        if matches!(url.scheme(), "http"|"https") {
            let link=gtk::LinkButton::with_label(url.as_str(),p.title.as_deref().unwrap_or(&p.url));link.set_tooltip_text(Some(url.as_str()));
            if let Some(label)=link.child().and_downcast::<gtk::Label>(){label.set_max_width_chars(45);label.set_ellipsize(pango::EllipsizeMode::End);}
            card.append(&link);
        }
    }
    if let Some(desc) = &p.description {
        let label = gtk::Label::new(Some(desc));
        label.set_wrap(true);
        label.set_wrap_mode(pango::WrapMode::WordChar);label.set_max_width_chars(45);label.set_xalign(0.0);
        card.append(&label);
    }
    if let Some(t) = media.textures.get(&p.id) {
        let image = crate::media::preview::picture(t);
        image.set_can_shrink(true);
        let open=gtk::Button::new();open.set_has_frame(false);open.set_halign(gtk::Align::Start);open.set_tooltip_text(Some("Abrir imagem"));open.set_child(Some(&image));let s=sender.clone();let key=p.id;open.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::OpenImage{message,key})));card.append(&open);
    }
    else if p.image_mime_type.is_some() {
        let button = gtk::Button::with_label("Carregar miniatura");
        let key = p.id;
        let s = sender.clone();
        button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));
        card.append(&button);
    }
    if p.title.as_deref().is_none_or(str::is_empty)&&p.description.as_deref().is_none_or(str::is_empty)&&p.image_mime_type.is_none()&&p.video_url.is_none(){
        let label=gtk::Label::new(Some("Prévia sem conteúdo disponível no servidor."));label.set_wrap(true);label.add_css_class("dim-label");card.append(&label);
        let retry=gtk::Button::with_label("Atualizar prévia");let s=sender.clone();let key=p.id;retry.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));card.append(&retry);
    }
    if let Some(embed)=&p.embed_url{if let Ok(url)=reqwest::Url::parse(embed){if matches!(url.scheme(),"http"|"https"){card.append(&gtk::LinkButton::with_label(url.as_str(),"Abrir conteúdo incorporado"));}}}
    if p.video_url.is_some(){append_video(&card,media,message,p.id,true,sender);}

    card
}
#[cfg(test)]
pub(crate) fn exercise(api: &crate::api::ApiClient, context: &gtk::glib::MainContext) {
    use super::actions::tests:: {
        descendants, find_button, pump, until
    };
    use serde_json::json;
    let user = "12345678-1234-4234-8234-123456789abc".parse().unwrap();
    let channel = "12345678-1234-4234-8234-123456789abd".parse().unwrap();
    let chat = ChatModel::builder().launch(ChatInit {
        api: Some(api.clone()), user_id: Some(user), ..Default::default()
    }).detach();
    chat.emit(ChatMsg::SetAccess {
        user_id: user, access: crate::models::Access::resolve(user, Some(user), &[], &[], &[])
    });
    chat.emit(ChatMsg::SetChannel(serde_json::from_value(json!( {
        "id": channel, "name": "files", "created_at": "2026-10-03T00:00:00Z"
    })).unwrap()));
    pump(context);
    let test_window=adw::Window::builder().default_width(900).default_height(650).content(chat.widget()).build();test_window.present();until(context,||test_window.is_mapped());
    let composer=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();composer.grab_focus();
    let path = std::env::temp_dir().join(format!("papo-ui-upload-{}", Uuid::new_v4()));
    std::fs::write(&path, b"file content").unwrap();
    let _cleanup = TemporaryFile(path.clone());
    let file = tokio::runtime::Handle::current().block_on(UploadFile::inspect(path)).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::Selected {
        epoch: chat.model().transfers.epoch, channel, result: Ok(vec![file.clone()])
    }));
    pump(context);
    let send = descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.tooltip_text().as_deref()==Some("Enviar mensagem"))).unwrap();
    assert!(send.is_sensitive());
    send.emit_clicked();
    until(context, ||chat.model().draft.error.as_ref().is_some_and(|e|e.contains("Upload failed")));
    assert_eq!(chat.model().draft.files.len(), 1);assert!(composer.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN),"a failed send keeps typing focus");
    send.emit_clicked();
    until(context, ||chat.model().draft.files.is_empty());
    assert_eq!(chat.model().history.messages.len(), 1);assert!(composer.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN),"a successful send keeps typing focus");
    chat.emit(ChatMsg::Transfer(TransferMsg::Selected {
        epoch: chat.model().transfers.epoch, channel, result: Ok(vec![file.clone()])
    }));
    pump(context);
    send.emit_clicked();
    chat.emit(ChatMsg::Transfer(TransferMsg::Cancel));
    pump(context);
    assert!(!chat.model().draft.is_sending());
    assert_eq!(chat.model().draft.files.len(), 1);
    let message = Uuid::new_v4();
    let attachment = Uuid::new_v4();
    let preview: Uuid = "12345678-1234-4234-8234-123456789ac0".parse().unwrap();
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!( {
        "id": message, "channel_id": channel, "created_at": "2026-10-03T00:00:00Z", "attachments": [ {
            "id": attachment, "mime_type": "image/png", "original_file_name": "Sensitive", "size_bytes": 10, "created_at": "2026-10-03T00:00:00Z", "moderation_status": "sensitive"
        }], "previews": [ {
            "id": preview, "url": "https://x.com/g1/status/2107799424168563173", "kind": "og", "fetched_at": "2026-10-03T00:00:00Z"
        }]
    })).unwrap()));
    until(context, ||chat.model().transfers.textures.contains_key(&preview));
    assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::LinkButton>()));
    assert!(descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text().contains("Elefante-marinho")),"a summary without image fields must hydrate the full text, thumbnail and video metadata");
    let preview_token=chat.model().transfers.preview_tokens[&preview];let preview_epoch=chat.model().transfers.epoch;
    let stale:LinkPreview=serde_json::from_value(json!({"id":preview,"url":"https://x.com/g1/status/2107799424168563173","kind":"og","title":"stale response","fetched_at":"2026-10-03T00:00:00Z"})).unwrap();
    chat.emit(ChatMsg::ApplyChange(Change::Preview(message,stale.clone())));pump(context);
    until(context,||chat.model().history.messages.iter().find(|m|m.id==message).unwrap().previews.as_ref().unwrap()[0].title.as_deref()==Some("g1 (@g1)"));
    chat.emit(ChatMsg::Transfer(TransferMsg::Preview{epoch:preview_epoch,message,key:preview,token:preview_token,result:Ok(stale)}));pump(context);
    assert_eq!(chat.model().history.messages.iter().find(|m|m.id==message).unwrap().previews.as_ref().unwrap()[0].title.as_deref(),Some("g1 (@g1)"),"a delayed preview response must not overwrite a newer fetch");
    // Decode both tracks through GTK's native backend, then close repeatedly to
    // exercise control detachment and asynchronous decoder teardown.
    find_button(chat.widget().upcast_ref(), "Reproduzir vídeo").emit_clicked();
    until(context, || !chat.model().transfers.videos.is_empty());
    assert!(chat.model().transfers.videos[0].file.0.exists());assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::Video>()),"video belongs to the timeline");
    let stream=chat.model().transfers.videos[0].media.clone();stream.set_muted(true);until(context,||stream.is_prepared()||stream.error().is_some());assert!(stream.error().is_none(),"inline video and audio must decode: {:?}",stream.error());assert!(stream.has_video()&&stream.has_audio());let file_path=chat.model().transfers.videos[0].file.0.clone();find_button(chat.widget().upcast_ref(),"Fechar vídeo").emit_clicked();pump(context);assert!(chat.model().transfers.videos.is_empty());assert!(!stream.is_playing()&&stream.file().is_none());assert!(!file_path.exists());
    for _ in 0..4{
        find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());
        let stream=chat.model().transfers.videos[0].media.clone();stream.set_muted(true);until(context,||stream.is_prepared()||stream.error().is_some());assert!(stream.error().is_none());
        let video=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Video>().ok()).unwrap();let path=chat.model().transfers.videos[0].file.0.clone();find_button(chat.widget().upcast_ref(),"Fechar vídeo").emit_clicked();pump(context);
        assert!(!stream.is_playing()&&stream.file().is_none()&&video.media_stream().is_none()&&!path.exists());
    }
    find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());
    find_button(chat.widget().upcast_ref(), "Conteúdo sensível — revelar").emit_clicked();
    pump(context);
    assert!(chat.model().transfers.revealed.contains(&attachment));
    let epoch = chat.model().transfers.epoch;
    chat.emit(ChatMsg::ApplyChange(Change::Moderation(message, attachment, "pending".into())));
    pump(context);
    assert!(!chat.model().transfers.revealed.contains(&attachment));
    chat.emit(ChatMsg::DeleteMessage(message));
    pump(context);
    assert!(chat.model().transfers.videos.is_empty());
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1, 1).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let image = bytes.into_inner();
    chat.emit(ChatMsg::Transfer(TransferMsg::Bytes {
        epoch, message, key: attachment, preview: false, result: Ok(image.clone())
    }));
    pump(context);
    assert!(!chat.model().transfers.textures.contains_key(&attachment));
    let video_message=Uuid::new_v4();let video_attachment=Uuid::new_v4();chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!({"id":video_message,"channel_id":channel,"author_id":user,"created_at":"2026-10-03T12:01:00Z","attachments":[{"id":video_attachment,"mime_type":"video/webm","original_file_name":"clip.webm","size_bytes":8293,"created_at":"2026-10-03T12:01:00Z"}]})).unwrap()));pump(context);
    chat.emit(ChatMsg::Transfer(TransferMsg::AttachmentVideo{message:video_message,attachment:video_attachment}));chat.emit(ChatMsg::Transfer(TransferMsg::CloseVideo{message:video_message,key:video_attachment}));pump(context);until(context,||chat.model().transfers.jobs.iter().all(|job|job.is_finished()));pump(context);assert!(chat.model().transfers.videos.is_empty(),"closing while downloading must reject the delayed file");
    find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("video-{video_message}-{video_attachment}")));let attachment_stream=chat.model().transfers.videos[0].media.clone();attachment_stream.set_muted(true);chat.emit(ChatMsg::DeleteMessage(video_message));pump(context);assert!(chat.model().transfers.videos.is_empty()&&!attachment_stream.is_playing()&&attachment_stream.file().is_none(),"deleting a message stops attachment playback");
    let mut bounded = Transfers::default();
    for _ in 0..40 {
        bounded.cache(Uuid::new_v4(), &image);
    }
    assert_eq!(bounded.textures.len(), 32);
    chat.emit(ChatMsg::ClearChannel);
    chat.emit(ChatMsg::Transfer(TransferMsg::Bytes {
        epoch, message, key: preview, preview: true, result: Ok(image)
    }));
    pump(context);
    assert!(chat.model().transfers.textures.is_empty());
    test_window.set_content(None::<&gtk::Widget>);test_window.close();
    // Inline pictures use their aspect ratio, grow beyond thumbnail sizes, and revoke an open viewer.
    let picture_id=Uuid::new_v4();let message_id=Uuid::new_v4();
    let image_message=serde_json::from_value(json!({"id":message_id,"channel_id":channel,"author_id":user,"created_at":"2026-10-03T12:00:00Z","attachments":[{"id":picture_id,"mime_type":"image/png","original_file_name":"example.png","size_bytes":2048,"created_at":"2026-10-03T12:00:00Z"}]})).unwrap();
    let photo=ChatModel::builder().launch(ChatInit{active_channel:Some(serde_json::from_value(json!({"id":channel,"name":"images","type":"text","created_at":"2026-10-03T12:00:00Z"})).unwrap()),messages:vec![image_message],user_id:Some(user),..Default::default()}).detach();photo.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access{read:true,..Default::default()}});pump(context);
    let epoch=photo.model().transfers.epoch;let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1024,256,image::Rgba([48,120,180,255]))).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
    photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{epoch,message:message_id,key:picture_id,preview:false,result:Ok(bytes.into_inner())}));until(context,||photo.model().transfers.textures.contains_key(&picture_id));assert_eq!((photo.model().transfers.textures[&picture_id].width(),photo.model().transfers.textures[&picture_id].height()),(680,170));
    let window=adw::Window::builder().default_width(900).default_height(650).content(photo.widget()).build();window.present();for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let open=find_button(photo.widget().upcast_ref(),"Abrir imagem");assert!(open.width()>0&&open.width()<=750&&open.width()>650);crate::ui::main_window::layout::tests::preview(&window,"inline-image",context);open.emit_clicked();pump(context);assert!(photo.model().transfers.images[0].2.is_visible());let viewer=photo.model().transfers.images[0].2.clone();
    let limiter=photo.model().transfers.limiter.clone();let permit=tokio::runtime::Handle::current().block_on(limiter.acquire_many_owned(4)).unwrap();
    let mut encoded=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(32,32).write_to(&mut encoded,image::ImageFormat::Png).unwrap();photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{epoch,message:message_id,key:picture_id,preview:false,result:Ok(encoded.into_inner())}));pump(context);assert!(photo.model().transfers.image_tokens.contains_key(&picture_id));
    let ticks=std::rc::Rc::new(std::cell::Cell::new(0));let count=ticks.clone();let timer=gtk::glib::timeout_add_local(std::time::Duration::from_millis(5),move ||{count.set(count.get()+1);gtk::glib::ControlFlow::Continue});for _ in 0..20{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}timer.remove();assert!(ticks.get()>5,"GTK remains responsive while image work waits off-thread");
    photo.emit(ChatMsg::ApplyChange(Change::Moderation(message_id,picture_id,"blocked".into())));pump(context);assert!(!viewer.is_visible());assert!(photo.model().transfers.images.is_empty());drop(permit);until(context,||photo.model().transfers.jobs.iter().all(|job|job.is_finished()));pump(context);assert!(!photo.model().transfers.textures.contains_key(&picture_id),"delayed decoding cannot restore blocked media");window.close();
}
