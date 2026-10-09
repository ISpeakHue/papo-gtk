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
    AnimationReady{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:Option<crate::media::animation::PreparedAnimation>},
    ImageReady{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:Option<crate::media::PreparedImage>},
    DismissImages,
    CloseVideo{message:Uuid,key:Uuid},AttachmentVideo{message:Uuid,attachment:Uuid},
    Preview{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:anyhow::Result<Embed>},
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
        epoch: Uuid, message: Uuid, key: Uuid, token:Option<Uuid>, preview: bool, result: anyhow::Result<Vec<u8>>
    }, File {
        epoch: Uuid, message: Uuid, key: Uuid, token:Option<Uuid>, destination: Option<PathBuf>, result: anyhow::Result<TemporaryFile>
    }, Saved {
        epoch: Uuid, message: Uuid, result: anyhow::Result<()>
    },
}
pub(super) struct Transfers {
    pub(super) revision: u64,
    normalized_revision:Option<u64>,
    #[cfg(test)]pub(super) metadata_visits:usize,
    #[cfg(test)]pub(super) candidate_visits:usize,
    dismissed_images:HashSet<(Uuid,Uuid)>,
    wanted:HashSet<Uuid>,
    sizes:HashMap<Uuid,(i32,i32)>,
    embeds:HashMap<Uuid,Embed>,
    video_files:HashMap<Uuid,(std::rc::Rc<TemporaryFile>,u64)>,
    video_order:VecDeque<Uuid>,
    pub scope:HashSet<Uuid>,
    pub priority:HashMap<Uuid,u64>,
    pub dirty:HashSet<Uuid>,
    failed:HashMap<Uuid,(u32,std::time::Instant)>,
    resolved:HashSet<Uuid>,
    payloads:HashMap<Uuid,(chrono::DateTime<chrono::Utc>,String)>,
    payload_order:VecDeque<Uuid>,
    giphy:HashMap<String,Uuid>,animations:HashMap<Uuid,(crate::media::animation::Animation,usize)>,
    pub(super) image_tokens:HashMap<Uuid,Uuid>,preview_tokens:HashMap<Uuid,Uuid>, pub(super) epoch: Uuid, limiter: std::sync::Arc<tokio::sync::Semaphore>, pub(super) textures: HashMap<Uuid, gtk::gdk::Texture>, order: VecDeque<Uuid>, requested: HashSet<Uuid>, revealed: HashSet<Uuid>, pub(super) jobs: Vec<tokio::task::JoinHandle<()>>, uploads: HashMap<Uuid, tokio::task::JoinHandle<()>>, pub progress: u64, video_requests:HashMap<(Uuid,Uuid),Uuid>,videos: Vec<Playback>,images:Vec<(Uuid,Uuid,gtk::Window)>,
}
struct Playback {message:Uuid,key:Uuid,view:gtk::Box,media:gtk::MediaFile,file:std::rc::Rc<TemporaryFile>,fullscreen:std::rc::Rc<super::video::Fullscreen>}
impl Drop for Playback{
    fn drop(&mut self){
        // Detach GTK's controls before closing their source: unrealizing a Video
        // can otherwise send pause requests into a player already being torn down.
        self.media.set_muted(true);self.media.pause();
        self.fullscreen.shutdown();
        detach_video(self.view.upcast_ref());
        self.media.clear();
    }
}
fn detach_video(widget:&gtk::Widget){
    if let Some(video)=widget.downcast_ref::<gtk::Video>(){video.set_media_stream(None::<&gtk::MediaStream>);}
    let mut child=widget.first_child();while let Some(w)=child{detach_video(&w);child=w.next_sibling();}
}
// Claim picture clicks without intercepting the player's buttons or seek slider.
fn video_surface(media:&gtk::MediaFile)->gtk::Video{
    let video=gtk::Video::for_media_stream(Some(media));
    let click=gtk::GestureClick::new();click.set_button(1);click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let claimed=std::rc::Rc::new(std::cell::Cell::new(false));let eligible=claimed.clone();
    let weak=video.downgrade();click.connect_pressed(move |gesture,_,x,y|{
        eligible.set(false);let Some(video)=weak.upgrade()else{return;};let mut hit=video.pick(x,y,gtk::PickFlags::DEFAULT);
        while let Some(widget)=hit{if widget.is::<gtk::Button>()||widget.is::<gtk::Range>()||widget.is::<gtk::MediaControls>(){return;}hit=widget.parent();}
        eligible.set(true);gesture.set_state(gtk::EventSequenceState::Claimed);
    });
    let weak=media.downgrade();click.connect_released(move |_,_,_,_|{
        if claimed.replace(false){if let Some(media)=weak.upgrade(){if media.is_playing(){media.pause();}else{media.play();}}}
    });video.add_controller(click);video
}
fn outside_image(x:f64,y:f64,width:f64,height:f64,image_width:f64,image_height:f64)->bool{
    let scale=(width/image_width).min(height/image_height);let w=image_width*scale;let h=image_height*scale;
    x<(width-w)/2.0||x>(width+w)/2.0||y<(height-h)/2.0||y>(height+h)/2.0
}
fn dismiss_image(window:&gtk::Window,picture:&gtk::Picture){
    let click=gtk::GestureClick::new();click.set_button(1);let weak=window.downgrade();
    click.connect_released(move |gesture,_,x,y|{let Some(window)=weak.upgrade()else{return;};let Some(picture)=gesture.widget().and_downcast::<gtk::Picture>()else{return;};let Some(image)=picture.paintable()else{return;};
        if outside_image(x,y,f64::from(picture.width()),f64::from(picture.height()),f64::from(image.intrinsic_width().max(1)),f64::from(image.intrinsic_height().max(1))){window.close();}
    });picture.add_controller(click);
    let keys=gtk::EventControllerKey::new();let weak=window.downgrade();keys.connect_key_pressed(move |_,key,_,_|{if key==gtk::gdk::Key::Escape{if let Some(window)=weak.upgrade(){window.close();}gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});window.add_controller(keys);

}
impl Default for Transfers {
    fn default() -> Self {
        Self {
            revision: 0,normalized_revision:None,
            #[cfg(test)]metadata_visits:0,
            #[cfg(test)]candidate_visits:0,
            dismissed_images:Default::default(),wanted:Default::default(),sizes:Default::default(),embeds:Default::default(),video_files:Default::default(),video_order:Default::default(),scope:Default::default(),priority:Default::default(),dirty:Default::default(),failed:Default::default(),resolved:Default::default(),payloads:Default::default(),payload_order:Default::default(),
            giphy:Default::default(),animations:Default::default(),image_tokens:Default::default(),preview_tokens:Default::default(), epoch: Uuid::new_v4(), limiter: std::sync::Arc::new(tokio::sync::Semaphore::new(4)), textures: HashMap::new(), order: VecDeque::new(), requested: HashSet::new(), revealed: HashSet::new(), jobs: Vec::new(), uploads: HashMap::new(), progress: 0, video_requests:Default::default(),videos: Vec::new(),images:Vec::new()
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
    pub fn release_offscreen_playback(&mut self,ids:&HashSet<Uuid>){
        self.videos.retain(|p|ids.contains(&p.message)||p.fullscreen.is_open());self.video_requests.retain(|(message,_),_|ids.contains(message));
    }
    pub fn invalidate_message(&mut self, message: Uuid) {
        self.images.retain(|(id,_,w)|{if *id==message{w.close();false}else{true}});
        self.video_requests.retain(|(id,_),_|*id!=message);for p in self.videos.iter().filter(|p|p.message==message){self.video_files.remove(&p.key);self.video_order.retain(|key|*key!=p.key);}self.videos.retain(|p|p.message!=message);
    }
    pub fn invalidate(&mut self, id: Uuid) {
        self.images.retain(|(_,key,w)|{if *key==id{w.close();false}else{true}});
        self.video_requests.retain(|(_,key),_|*key!=id);self.videos.retain(|p|p.key!=id);
        self.revealed.remove(&id);self.video_files.remove(&id);self.video_order.retain(|key|*key!=id);self.embeds.remove(&id);self.sizes.remove(&id);
        self.textures.remove(&id);self.animations.remove(&id);
        self.payloads.remove(&id);self.payload_order.retain(|key|*key!=id);
        self.failed.remove(&id);self.resolved.remove(&id);
        self.requested.remove(&id);self.preview_tokens.remove(&id);self.image_tokens.remove(&id);
    }
    pub fn switch_channel(&mut self){
        // Cache only session-owned media; requests, reveal consent and players
        // always belong to the selected channel's epoch.
        self.giphy.retain(|_,key|self.textures.contains_key(key));
        let textures=std::mem::take(&mut self.textures);let animations=std::mem::take(&mut self.animations);let order=std::mem::take(&mut self.order);
        let giphy=std::mem::take(&mut self.giphy);let sizes=std::mem::take(&mut self.sizes);let previews=std::mem::take(&mut self.embeds);let resolved=std::mem::take(&mut self.resolved);
        let files=std::mem::take(&mut self.video_files);let videos=std::mem::take(&mut self.video_order);
        self.reset();
        self.textures=textures;self.animations=animations;self.order=order;self.giphy=giphy;self.sizes=sizes;self.embeds=previews;self.resolved=resolved;self.video_files=files;self.video_order=videos;
    }
    pub fn reset(&mut self) {
        self.normalized_revision=None;
        self.epoch = Uuid::new_v4();self.wanted.clear();self.sizes.clear();self.embeds.clear();self.video_files.clear();self.video_order.clear();self.dismissed_images.clear();
        self.scope.clear();self.priority.clear();self.dirty.clear();self.failed.clear();self.resolved.clear();self.payloads.clear();self.payload_order.clear();
        self.requested.clear();self.preview_tokens.clear();self.image_tokens.clear();
        self.revealed.clear();
        self.textures.clear();self.animations.clear();self.giphy.clear();
        for (_,_,window) in self.images.drain(..){window.close();}
        self.order.clear();
        for job in self.jobs.drain(..) {
            job.abort();
        } self.video_requests.clear();self.videos.clear();
    }
    fn payload(&mut self,key:Uuid,at:chrono::DateTime<chrono::Utc>,data:String){
        if data.len()>8<<20||self.payloads.get(&key).is_some_and(|(old,_)|*old>at){return;}
        self.resolved.remove(&key);self.payloads.insert(key,(at,data));self.payload_order.retain(|id|*id!=key);self.payload_order.push_back(key);
        while self.payloads.values().map(|(_,data)|data.len()).sum::<usize>()>8<<20||self.payload_order.len()>32{
            let Some(key)=self.payload_order.pop_front()else{break;};self.payloads.remove(&key);
        }
    }
    fn request(&mut self,key:Uuid)->bool{
        self.wanted.insert(key);
        if self.textures.contains_key(&key){self.order.retain(|id|*id!=key);self.order.push_back(key);return false;}
        if self.requested.len()>=8||self.textures.contains_key(&key)||self.resolved.contains(&key)||self.failed.get(&key).is_some_and(|(attempt,at)|*attempt>=3||*at>std::time::Instant::now()){return false;}
        self.requested.insert(key)
    }
    fn failed(&mut self,key:Uuid){
        self.requested.remove(&key);let attempt=self.failed.get(&key).map_or(1,|(n,_)|n+1);
        self.failed.insert(key,(attempt,std::time::Instant::now()+std::time::Duration::from_secs(2u64.pow(attempt.min(5)))));
    }
    pub(super) fn authorized(&self,messages:&[Message],key:Uuid)->bool{
        messages.iter().any(|m|self.giphy_key(m,key)||m.embeds.iter().flatten().any(|p|p.id==key)||m.attachments.iter().flatten().any(|a|a.id==key&&self.visible(a)))
    }
    pub fn visible(&self, a: &Attachment) -> bool {
        match a.moderation_status.as_deref() {
            Some("blocked"|"pending"|"processing") => false, Some("sensitive") => self.revealed.contains(&a.id), _ => true
        }
    }
    #[cfg(test)]
    fn cache(&mut self,id:Uuid,bytes:&[u8]){if let Some(image)=crate::media::PreparedImage::decode_thumbnail(bytes,680,460){self.cache_image(id,image);}}
    fn remember_size(&mut self,key:Uuid,width:i32,height:i32){
        if self.sizes.len()>=256&&!self.sizes.contains_key(&key){if let Some(old)=self.sizes.keys().copied().find(|id|!self.textures.contains_key(id)){self.sizes.remove(&old);}}
        self.sizes.insert(key,(width,height));
    }
    fn prune_textures(&mut self){
        // Prefer offscreen entries even when older downloads complete late.
        while self.order.len()>32{let index=self.order.iter().position(|id|!self.wanted.contains(id)).unwrap_or(0);if let Some(old)=self.order.remove(index){self.dirty.insert(old);self.textures.remove(&old);self.animations.remove(&old);}}
    }
    fn remember_preview(&mut self,mut preview:Embed){
        preview.image_data=None;
        if !self.sizes.contains_key(&preview.id){
            if let Some((width,height))=preview.thumbnail.as_ref().and_then(|m|m.width.zip(m.height)).filter(|(w,h)|*w>0&&*h>0){self.remember_size(preview.id,width,height);}
        }
        if self.embeds.len()>=128&&!self.embeds.contains_key(&preview.id){if let Some(old)=self.embeds.keys().copied().find(|id|!self.wanted.contains(id)){self.embeds.remove(&old);self.resolved.remove(&old);}}
        self.embeds.insert(preview.id,preview);
    }
    fn remember_video(&mut self,key:Uuid,file:std::rc::Rc<TemporaryFile>){
        let size=file.0.metadata().map_or(u64::MAX,|m|m.len());if size>128<<20{return;}
        self.video_files.insert(key,(file,size));self.video_order.retain(|id|*id!=key);self.video_order.push_back(key);
        while self.video_files.len()>4||self.video_files.values().map(|(_,size)|size).sum::<u64>()>128<<20{if let Some(old)=self.video_order.pop_front(){self.video_files.remove(&old);}else{break;}}
    }
    fn cache_image(&mut self,id:Uuid,image:crate::media::PreparedImage){
            self.dirty.insert(id);
            self.requested.remove(&id);self.failed.remove(&id);
            self.revision=self.revision.wrapping_add(1);
            let texture=image.texture();self.remember_size(id,texture.width(),texture.height());
            self.animations.remove(&id);self.textures.insert(id, texture);
            self.order.retain(|key| *key != id);
            self.order.push_back(id);
            self.prune_textures();
    }
}

#[cfg(test)]
pub(super) fn exercise_performance(context:&gtk::glib::MainContext){
    use super::performance::settle;
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let start=chrono::Utc::now();
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"media burst","created_at":start})).unwrap();
    let messages:Vec<Message>=(0..24).map(|i|serde_json::from_value(serde_json::json!({
        "id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":start+chrono::Duration::seconds(i),
        "content":format!("Image {i}"),"attachments":[{"id":Uuid::new_v4(),"mime_type":"image/png","original_file_name":"image.png","size_bytes":128,"created_at":start}]
    })).unwrap()).collect();
    let chat=ChatModel::builder().launch(super::ChatInit{active_channel:Some(target),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();settle(context);
    let passes=chat.model().render_passes;
    let epoch=chat.model().transfers.epoch;
    for message in &messages{
        let key=message.attachments.as_ref().unwrap()[0].id;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(key,token);
        chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,message:message.id,key,token,result:Some(crate::media::PreparedImage{width:32,height:32,pixels:vec![255;32*32*4]})}));
    }
    settle(context);
    assert_eq!(chat.model().transfers.textures.len(),24);
    assert_eq!(chat.model().render_passes-passes,1,"a burst of decodes must reconcile history once");
    let passes=chat.model().render_passes;
    for message in &messages{
        chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch:Uuid::new_v4(),message:message.id,key:message.attachments.as_ref().unwrap()[0].id,token:Uuid::new_v4(),result:None}));
    }
    settle(context);
    assert_eq!(chat.model().render_passes,passes,"stale completions must not rebuild or move history");
    window.set_content(None::<&gtk::Widget>);window.close();
}
impl Transfers{
    pub(super) fn giphy_key(&self,message:&Message,key:Uuid)->bool{
        // Ordinary image completions must not parse every cached message for
        // Giphy links while checking permissions or dirty media slots.
        if !self.giphy.values().any(|id|*id==key){return false;}
        crate::media::giphy::ids(message.content.as_deref().unwrap_or("")).iter().any(|id|self.giphy.get(id)==Some(&key))
    }
    fn placeholder(&self,key:Uuid,child:&impl IsA<gtk::Widget>,embedded:bool)->crate::media::frame::Frame{
        let (width,height)=self.sizes.get(&key).copied().unwrap_or((640,360));let ratio=f64::from(width)/f64::from(height.max(1));
        let width=if embedded{420.0f64.min(300.0*ratio)}else{640.0f64.min(480.0*ratio)};
        crate::media::frame::frame(child,width.round() as i32,(width/ratio).round() as i32)
    }
    fn picture(&self,key:Uuid)->Option<gtk::Picture>{
        if let Some((animation,_))=self.animations.get(&key){Some(animation.picture())}else{self.textures.get(&key).map(crate::media::preview::picture)}
    }
    fn cache_animation(&mut self,key:Uuid,prepared:crate::media::animation::PreparedAnimation){
        self.dirty.insert(key);
        self.requested.remove(&key);self.failed.remove(&key);
        self.revision=self.revision.wrapping_add(1);
        let bytes=prepared.bytes();let animation=prepared.paintable();let first=animation.first();self.remember_size(key,first.width(),first.height());
        self.textures.insert(key,first);self.animations.insert(key,(animation,bytes));self.order.retain(|id|*id!=key);self.order.push_back(key);
        self.prune_textures();
        // Keep a static first frame when several GIFs exhaust the animation
        // budget. Dropping the texture too would trigger an endless refetch loop.
        while self.animations.values().map(|(_,n)|*n).sum::<usize>()>48<<20{
            let old=self.order.iter().find(|id|self.animations.contains_key(id)&&!self.wanted.contains(id)).or_else(||self.order.iter().find(|id|self.animations.contains_key(id))).copied();
            let Some(old)=old else{break;};self.animations.remove(&old);self.dirty.insert(old);
        }
    }
}

enum ImageInput{Bytes(Vec<u8>),Base64(String)}
fn queue_image(transfers:&mut Transfers,sender:&ComponentSender<ChatModel>,message:Uuid,key:Uuid,input:ImageInput){
    let token=Uuid::new_v4();transfers.image_tokens.insert(key,token);let epoch=transfers.epoch;let limiter=transfers.limiter.clone();let output=sender.input_sender().clone();
    transfers.jobs.push(tokio::spawn(async move{
        let Ok(_permit)=limiter.acquire_owned().await else{return;};
        let result=tokio::task::spawn_blocking(move ||{let bytes=match input{ImageInput::Bytes(bytes)=>bytes,ImageInput::Base64(data)=>{use base64::Engine;if data.len()>6<<20{return (None,None);}let Ok(bytes)=base64::engine::general_purpose::STANDARD.decode(data)else{return (None,None);};bytes}};
            if bytes.starts_with(b"GIF8"){(None,crate::media::animation::PreparedAnimation::decode(&bytes))}else{(crate::media::PreparedImage::decode_thumbnail(&bytes,680,460),None)}
        }).await.unwrap_or_default();
        let msg=if let Some(animation)=result.1{TransferMsg::AnimationReady{epoch,message,key,token,result:Some(animation)}}else{TransferMsg::ImageReady{epoch,message,key,token,result:result.0}};
        let _=output.send(ChatMsg::Transfer(msg));
    }));
}
impl ChatModel {
    pub(super) fn start_send(&mut self, sender: &ComponentSender<Self>, request_id: Uuid, channel_id: Uuid) {
        let Some(api) = self.actions.api.clone() else {
            self.draft.finish(request_id, Err("API indisponível".into()));
            return;
        };
        let request = crate::models::CreateMessageRequest {
            embeds:self.draft.embeds.clone(),
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
        if self.viewport.scrolling(){self.viewport.media_when_idle(sender);return;}
        if !self.access.read{return;}
        // An opening unread search can replace this intermediate page. Reserve
        // download slots and decode work for the destination that will be shown.
        if self.initial_read.is_some()&&(!self.history_ready||self.history.loading()||self.older_requested){return;}
        let Some(api) = self.actions.api.clone() else {
            return;
        };
        #[cfg(test)]{self.media_hydrations+=1;}
        self.transfers.jobs.retain(|j|!j.is_finished());
        let epoch = self.transfers.epoch;
        // A normalized URL can share a preview ID across several messages.
        // Reuse full metadata already hydrated in this history for later summaries.
        // Move encoded payloads out before sharing metadata: cloning a preview
        // used to copy megabytes of base64 for every message referencing it.
        self.transfers.wanted.clear();
        let mut images=HashMap::new();
        let mut known:HashMap<Uuid,Embed>=HashMap::new();
        let revision=self.history.media_revision();
        if self.transfers.normalized_revision!=Some(revision){
        #[cfg(test)]{self.transfers.metadata_visits+=self.history.messages.len();}
        for p in self.history.messages.iter_mut().flat_map(|m|m.embeds.iter_mut().flatten()){
            if let Some(cached)=self.transfers.embeds.get(&p.id){
                if cached.version()>=p.version(){if p.image_data.is_none(){*p=cached.clone();}}
                else{self.transfers.invalidate(p.id);}
            }
            let data=p.image_data.take();
            let score=|p:&Embed|usize::from(p.title.is_some())+usize::from(p.description.is_some())+usize::from(p.thumbnail.is_some())+usize::from(p.has_video());
            if known.get(&p.id).is_none_or(|old|p.version()>old.version()||(p.version()==old.version()&&(score(p)>score(old)||(score(p)==score(old)&&data.is_some()&&!images.contains_key(&p.id))))){
                images.remove(&p.id);if let Some(data)=data{images.insert(p.id,data);}
                known.insert(p.id,p.clone());
            }
        }
        for preview in known.values(){self.transfers.remember_preview(preview.clone());}
        for (key,data) in images{if let Some(preview)=known.get(&key){self.transfers.payload(key,preview.version(),data);}}
        self.transfers.normalized_revision=Some(revision);
        }
        let scope=self.transfers.scope.clone();let mut candidates=0;
        // Candidate discovery is limited to materialized rows. Scrolling must
        // not rescan every cached page or clone unchanged embed metadata.
        #[cfg(test)]{self.transfers.candidate_visits+=self.render_window.range.len();}
        let mut messages:Vec<_>=self.history.messages.iter_mut().skip(self.render_window.range.start).take(self.render_window.range.len()).filter(|m|scope.contains(&m.id)).collect();
        messages.sort_by_key(|m|self.transfers.priority.get(&m.id).copied().unwrap_or(u64::MAX));
        for m in messages {
            for id in crate::media::giphy::ids(m.content.as_deref().unwrap_or("")){
                if candidates>=24{break;}candidates+=1;
                let key=*self.transfers.giphy.entry(id.clone()).or_insert_with(Uuid::new_v4);
                if self.transfers.request(key){let token=Uuid::new_v4();self.transfers.image_tokens.insert(key,token);let s=sender.clone();let message=m.id;let limiter=self.transfers.limiter.clone();
                    self.transfers.jobs.push(tokio::spawn(async move{let Ok(_permit)=limiter.acquire_owned().await else{return;};let result=crate::media::giphy::fetch(&id).await;s.input(ChatMsg::Transfer(TransferMsg::Bytes{epoch,message,key,token:Some(token),preview:true,result}));}));
                }
            }
            for a in m.attachments.iter().flatten() {
                if candidates>=24{break;}if a.mime_type.starts_with("image/"){candidates+=1;}
                if a.mime_type.starts_with("image/") && self.transfers.visible(a)
                && self.transfers.request(a.id) {
                    let api = api.clone();
                    let s = sender.clone();
                    let key = a.id;let token=Uuid::new_v4();self.transfers.image_tokens.insert(key,token);
                    let message = m.id;
                    let limiter = self.transfers.limiter.clone();
                    self.transfers.jobs.push(tokio::spawn(async move {
                        let Ok(_permit) = limiter.acquire_owned().await else {
                            return;
                        };
                        let result = api.media_bytes(&format!("/attachments/{key}/thumbnail"), 4<<20).await;
                        s.input(ChatMsg::Transfer(TransferMsg::Bytes {
                            epoch, message, key, token:Some(token), preview: false, result
                        }));
                    }));
                }
            }
            for p in m.embeds.iter_mut().flatten() {
                // New history/WS payloads already carry complete rich metadata.
                // Only thumbnails need an authorized GET for their bytes.
                if !p.fetch_method.is_empty()&&p.thumbnail.is_none(){continue;}
                if candidates>=24{break;}candidates+=1;
                if let Some(full)=known.get(&p.id).or_else(||self.transfers.embeds.get(&p.id)){if full.version()>=p.version()&&full!=p{*p=full.clone();}}
                if self.transfers.payloads.contains_key(&p.id){
                    if self.transfers.request(p.id){
                        let (_,data)=self.transfers.payloads.remove(&p.id).unwrap();self.transfers.payload_order.retain(|key|*key!=p.id);
                        queue_image(&mut self.transfers,sender,m.id,p.id,ImageInput::Base64(data));
                    }
                }else if self.transfers.request(p.id){
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
                        let result=api.get_embed(key).await;
                        let _=s.input_sender().send(ChatMsg::Transfer(TransferMsg::Preview{epoch,message,key,token,result}));
                    }));
                }
            }
        }
    }
    pub(super) fn handle_transfer(&mut self, msg: TransferMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        match msg {
            TransferMsg::AnimationReady{epoch,message:_,key,token,result}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token){return;}
                let valid=self.transfers.authorized(&self.history.messages,key);
                self.transfers.image_tokens.remove(&key);self.transfers.requested.remove(&key);if valid{if let Some(animation)=result{self.transfers.cache_animation(key,animation);}else{self.transfers.failed(key);}}
            },
            TransferMsg::ImageReady{epoch,message:_,key,token,result}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token){return;}
                let valid=self.transfers.authorized(&self.history.messages,key);
                self.transfers.image_tokens.remove(&key);self.transfers.requested.remove(&key);if valid{if let Some(image)=result{self.transfers.cache_image(key,image);}else{self.transfers.failed(key);}}
            },
            TransferMsg::AttachmentVideo{message,attachment}=>{if self.access.read&&self.history.messages.iter().find(|m|m.id==message).is_some_and(|m|m.attachments.iter().flatten().any(|a|a.id==attachment&&a.mime_type.starts_with("video/")&&self.transfers.visible(a))){self.start_download(message,attachment,None,false,sender);}},
            TransferMsg::CloseVideo{message,key}=>{self.transfers.video_requests.remove(&(message,key));self.videos_retain(message,key);},
            TransferMsg::Preview{epoch,message,key,token,result}=>{
                if epoch!=self.transfers.epoch||self.transfers.preview_tokens.get(&key)!=Some(&token)||!self.access.read{return;}
                if !self.history.messages.iter().any(|m|m.embeds.iter().flatten().any(|p|p.id==key)){self.transfers.preview_tokens.remove(&key);self.transfers.requested.remove(&key);return;}
                self.transfers.preview_tokens.remove(&key);
                match result{
                    Ok(mut preview) if preview.id==key=>{
                        self.transfers.revision=self.transfers.revision.wrapping_add(1);
                        if let Some(data)=preview.image_data.take(){queue_image(&mut self.transfers,sender,message,key,ImageInput::Base64(data));}else{self.transfers.requested.remove(&key);self.transfers.resolved.insert(key);}
                        self.transfers.remember_preview(preview.clone());
                        let ids:Vec<_>=self.history.messages.iter().filter(|m|m.embeds.iter().flatten().any(|p|p.id==key)).map(|m|m.id).collect();
                        for id in ids{self.history.apply(Change::Preview(id,preview.clone()));}
                    },
                    Err(error)=>{self.transfers.failed(key);if crate::api::is_session_error(&error){let _=sender.output(ChatOutput::ActionError(error));}},_=>{self.transfers.failed(key);},
                }
            },
            TransferMsg::DismissImages=>{
                self.transfers.dismissed_images.clear();
                for (message,key,window) in self.transfers.images.drain(..){if window.is_visible(){self.transfers.dismissed_images.insert((message,key));window.close();}}
            },
            TransferMsg::OpenImage{message,key}=>{
                if self.transfers.dismissed_images.remove(&(message,key)){return;}

                if !self.access.read||!self.history.messages.iter().any(|m|m.id==message&&(self.transfers.giphy_key(m,key)||m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a))||m.embeds.iter().flatten().any(|p|p.id==key))){return;}
                let Some(texture)=self.transfers.textures.get(&key)else{return;};
                if let Some((_,_,window))=self.transfers.images.iter().find(|(m,k,w)|*m==message&&*k==key&&w.is_visible()){window.close();return;}
                let window=gtk::Window::builder().title("Imagem").default_width(900).default_height(680).build();if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
                let picture=self.transfers.picture(key).unwrap_or_else(||gtk::Picture::for_paintable(texture));picture.set_can_shrink(true);picture.set_content_fit(gtk::ContentFit::Contain);window.set_child(Some(&picture));dismiss_image(&window,&picture);window.present();self.transfers.images.retain(|(_,_,w)|w.is_visible());self.transfers.images.push((message,key,window));
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
                if !self.transfers.requested.contains(&id){self.transfers.failed.remove(&id);self.transfers.resolved.remove(&id);}
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
                epoch, message, key, token, preview:_, result
            } => {
                if epoch!=self.transfers.epoch||!self.access.read||token.is_some_and(|token|self.transfers.image_tokens.get(&key)!=Some(&token)) {
                    return;
                }
                let valid=self.transfers.authorized(&self.history.messages,key);
                if !valid{self.transfers.requested.remove(&key);}
                if valid {
                    match result {
                        Ok(bytes) => queue_image(&mut self.transfers,sender,message,key,ImageInput::Bytes(bytes)), Err(e) => {
                            self.transfers.failed(key);
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
                if self.access.read&&self.history.messages.iter().any(|m|m.id==message&&m.embeds.iter().flatten().any(|p|p.id==preview)) {
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
                    m.embeds.iter().flatten().any(|p|p.id==key&&p.has_video())||m.attachments.iter().flatten().any(|a|a.id==key&&a.mime_type.starts_with("video/")&&self.transfers.visible(a))
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
                            let file=std::rc::Rc::new(file);self.transfers.remember_video(key,file.clone());self.play_video(message,key,file,sender);
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
    fn play_video(&mut self,message:Uuid,key:Uuid,file:std::rc::Rc<TemporaryFile>,sender:&ComponentSender<Self>){
                            if self.transfers.videos.len()>=2{self.transfers.videos.remove(0);}
                            let view=gtk::Box::new(gtk::Orientation::Vertical,0);view.set_widget_name(&format!("video-{message}-{key}"));let player=gtk::Box::new(gtk::Orientation::Vertical,6);view.append(&player);
                            let close=gtk::Button::with_label("Fechar vídeo");close.set_halign(gtk::Align::End);let input=sender.input_sender().clone();close.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::CloseVideo{message,key}));});player.append(&close);
                            let media=gtk::MediaFile::for_filename(&file.0);let video=video_surface(&media);video.set_hexpand(true);let frame=crate::media::frame::frame(&video,640,360);player.append(&frame);let controls=super::video::controls(&video,&media);player.append(&controls);
                            let fullscreen=super::video::Fullscreen::new(&view,&player,&frame,&media,&controls);
                            let error=gtk::Label::new(None);error.set_wrap(true);error.add_css_class("error");player.append(&error);let error_now=error.clone();media.connect_error_notify(move |m|{if let Some(e)=m.error(){error.set_text(&format!("Não foi possível reproduzir: {e}"));}});if let Some(e)=media.error(){error_now.set_text(&format!("Não foi possível reproduzir: {e}"));}
                            media.play();self.transfers.videos.push(Playback{message,key,view,media,file,fullscreen});
    }
    fn start_download(&mut self, message: Uuid, key: Uuid, destination: Option<PathBuf>, video: bool, sender: &ComponentSender<Self>) {
        let Some(api) = self.actions.api.clone() else {
            return;
        };
        let inline=destination.is_none();
        if inline&&(self.transfers.video_requests.contains_key(&(message,key))||self.transfers.videos.iter().any(|p|p.message==message&&p.key==key)){return;}
        if inline{if let Some((file,_))=self.transfers.video_files.get(&key){let file=file.clone();self.transfers.video_order.retain(|id|*id!=key);self.transfers.video_order.push_back(key);self.play_video(message,key,file,sender);return;}}
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
            format!("/embeds/{key}/video")
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
    card.set_halign(gtk::Align::Start);let visual=a.mime_type.starts_with("image/")||a.mime_type.starts_with("video/");if !visual{card.add_css_class("card");}
    let label=gtk::Label::new(Some(&format!("{} · {}",a.original_file_name,format_file_size(a.size_bytes))));label.set_xalign(0.0);label.set_ellipsize(pango::EllipsizeMode::End);label.set_max_width_chars(45);label.add_css_class("caption");let header=gtk::Box::new(gtk::Orientation::Horizontal,6);label.set_hexpand(true);header.append(&label);card.append(&header);
    if media.visible(a) {
        if a.mime_type.starts_with("video/"){append_video(&card,media,message,a.id,false,sender);}
        if let Some(t) = media.textures.get(&a.id) {
            let image = media.picture(a.id).unwrap();
            image.set_can_shrink(true);
            let open=gtk::Button::new();open.set_has_frame(false);open.set_halign(gtk::Align::Start);open.add_css_class("papo-inline-image");open.set_tooltip_text(Some("Abrir imagem"));open.set_child(Some(&crate::media::frame::image_frame(&image,t,false)));let s=sender.clone();let key=a.id;open.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::OpenImage{message,key})));card.append(&open);
        }
        else if a.mime_type.starts_with("image/") {
            let button = gtk::Button::with_label("Carregar imagem");
            let key = a.id;
            let s = sender.clone();
            button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));
            card.append(&media.placeholder(a.id,&button,false));
        }
        let button = if visual{gtk::Button::from_icon_name("folder-download-symbolic")}else{gtk::Button::with_label("Baixar arquivo")};
        button.set_tooltip_text(Some("Baixar arquivo"));button.update_property(&[gtk::accessible::Property::Label("Baixar arquivo")]);button.add_css_class("flat");button.set_halign(gtk::Align::End);
        let a = a.clone();
        let s = sender.clone();
        button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::Download {
            message, attachment: a.clone()
        })));
        header.append(&button);
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
        card.append(&playback.view);if playing{playback.media.play();}
    }else{
        let overlay=gtk::Overlay::new();
        if let Some(texture)=media.textures.get(&key){overlay.set_child(Some(&crate::media::preview::picture(texture)));}
        else{overlay.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical,0)));}
        let button=gtk::Button::from_icon_name("media-playback-start-symbolic");button.set_tooltip_text(Some("Reproduzir vídeo"));button.update_property(&[gtk::accessible::Property::Label("Reproduzir vídeo")]);button.add_css_class("papo-media-play");button.set_halign(gtk::Align::Center);button.set_valign(gtk::Align::Center);
        button.set_sensitive(!media.video_requests.contains_key(&(message,key)));let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(if preview{TransferMsg::Video{message,preview:key}}else{TransferMsg::AttachmentVideo{message,attachment:key}}));});overlay.add_overlay(&button);
        card.append(&crate::media::frame::frame(&overlay,640,360));
    }
}

pub(super) fn preview_widget(p: &Embed, message: Uuid, media: &Transfers, sender: &ComponentSender<ChatModel>)->gtk::Box {
    let card=super::rich::body(p);
    if !p.has_video(){if let Some(t) = media.textures.get(&p.id) {
        let image = media.picture(p.id).unwrap();
        image.set_can_shrink(true);
        let open=gtk::Button::new();open.set_has_frame(false);open.set_halign(gtk::Align::Start);open.add_css_class("papo-inline-image");open.set_tooltip_text(Some("Abrir imagem"));open.set_child(Some(&crate::media::frame::image_frame(&image,t,true)));let s=sender.clone();let key=p.id;open.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::OpenImage{message,key})));card.append(&open);
    }
    else if p.thumbnail.is_some() {
        let button = gtk::Button::with_label("Carregar miniatura");
        let key = p.id;
        let s = sender.clone();
        button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));
        card.append(&media.placeholder(key,&button,true));
    }
    }
    if p.title.as_deref().is_none_or(str::is_empty)&&p.description.as_deref().is_none_or(str::is_empty)&&p.thumbnail.is_none()&&!p.has_video()&&p.fields.is_empty()&&p.author.is_none()&&p.footer.is_none()&&p.image.is_none(){
        let label=gtk::Label::new(Some("Prévia sem conteúdo disponível no servidor."));label.set_wrap(true);label.add_css_class("dim-label");card.append(&label);
        if p.fetch_method.is_empty(){let retry=gtk::Button::with_label("Atualizar prévia");let s=sender.clone();let key=p.id;retry.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));card.append(&retry);}
    }
    if let Some(url)=p.embed_url.as_deref().and_then(super::rich::youtube_url){card.append(&gtk::LinkButton::with_label(url.as_str(),"Abrir no YouTube"));}
    if p.has_video(){append_video(&card,media,message,p.id,true,sender);}
    super::rich::footer(&card,p);

    // Keep the existing 640-pixel video surface while bounding text-only cards.
    // A clamp centers its child when tightening. Make its natural width equal
    // to the capped card width and align the whole clamp with the message text.
    let maximum=if p.has_video(){700}else{520};
    let clamp=adw::Clamp::builder().maximum_size(maximum).tightening_threshold(maximum).halign(gtk::Align::Start).child(&card).build();
    let wrapper=gtk::Box::new(gtk::Orientation::Vertical,0);wrapper.append(&clamp);wrapper
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
    let decode_gate=tokio::runtime::Handle::current().block_on(chat.model().transfers.limiter.clone().acquire_many_owned(4)).unwrap();
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!( {
        "id": message, "channel_id": channel, "created_at": "2026-10-03T00:00:00Z", "attachments": [ {
            "id": attachment, "mime_type": "image/png", "original_file_name": "Sensitive", "size_bytes": 10, "created_at": "2026-10-03T00:00:00Z", "moderation_status": "sensitive"
        }], "previews": [ {
            "id": preview, "url": "https://x.com/g1/status/2107799424168563173", "kind": "og", "fetched_at": "2026-10-03T00:00:00Z"
        }]
    })).unwrap()));
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(message)));until(context,||chat.model().transfers.preview_tokens.contains_key(&preview));let preview_token=chat.model().transfers.preview_tokens[&preview];drop(decode_gate);
    until(context, ||chat.model().transfers.textures.contains_key(&preview)&&chat.model().media_render_pending.is_none());
    assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::LinkButton>()));
    assert!(descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text().contains("Elefante-marinho")),"a summary without image fields must hydrate the full text, thumbnail and video metadata");
    let preview_epoch=chat.model().transfers.epoch;
    let stale:Embed=serde_json::from_value(json!({"id":preview,"url":"https://x.com/g1/status/2107799424168563173","kind":"og","title":"stale response","fetched_at":"2026-10-03T00:00:00Z"})).unwrap();
    chat.emit(ChatMsg::ApplyChange(Change::Preview(message,stale.clone())));pump(context);
    until(context,||chat.model().history.messages.iter().find(|m|m.id==message).unwrap().embeds.as_ref().unwrap()[0].title.as_deref()==Some("g1 (@g1)")&&chat.model().media_render_pending.is_none());
    chat.emit(ChatMsg::Transfer(TransferMsg::Preview{epoch:preview_epoch,message,key:preview,token:preview_token,result:Ok(stale)}));pump(context);
    assert_eq!(chat.model().history.messages.iter().find(|m|m.id==message).unwrap().embeds.as_ref().unwrap()[0].title.as_deref(),Some("g1 (@g1)"),"a delayed preview response must not overwrite a newer fetch");
    // Decode both tracks through GTK's native backend, then close repeatedly to
    // exercise control detachment and asynchronous decoder teardown.
    find_button(chat.widget().upcast_ref(), "Reproduzir vídeo").emit_clicked();
    until(context, || !chat.model().transfers.videos.is_empty());
    assert!(chat.model().transfers.videos[0].file.0.exists());assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::Video>()),"video belongs to the timeline");
    let stream=chat.model().transfers.videos[0].media.clone();assert!(!stream.is_muted()&&stream.volume()>0.0,"user-started video must have audible defaults");until(context,||stream.is_prepared()||stream.error().is_some());assert!(!stream.is_muted()&&stream.volume()>0.0,"audio defaults survive asynchronous preparation");super::video::exercise(chat.widget(),&stream,context);assert!(stream.error().is_none(),"inline video and audio must decode: {:?}",stream.error());assert!(stream.has_video()&&stream.has_audio());
    for _ in 0..30{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let video=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Video>().ok()).unwrap();assert!(video.width()>=600&&video.height()>=330,"actual player must be large: {}x{}",video.width(),video.height());crate::ui::main_window::layout::tests::preview(&test_window,"inline-video",context);
    let row=chat.model().rendered[&format!("message-{message}")].1.clone();let all=descendants(row.upcast_ref());let card=all.iter().find(|w|w.has_css_class("papo-link-preview")).unwrap();let media_box=all.iter().find(|w|w.widget_name()=="message-media").unwrap();assert!(card.compute_bounds(media_box).unwrap().x().abs()<=1.0,"video embeds must align with the message's left edge");
    let controls=video.observe_controllers();let click=(0..controls.n_items()).find_map(|i|controls.item(i).and_downcast::<gtk::GestureClick>().filter(|g|g.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    stream.play();click.emit_by_name::<()>("pressed",&[&1i32,&10.0f64,&10.0f64]);click.emit_by_name::<()>("released",&[&1i32,&10.0f64,&10.0f64]);assert!(!stream.is_playing(),"clicking the video picture must pause it");
    click.emit_by_name::<()>("pressed",&[&1i32,&10.0f64,&10.0f64]);click.emit_by_name::<()>("released",&[&1i32,&10.0f64,&10.0f64]);assert!(stream.is_playing(),"clicking again must resume playback");
    stream.pause();find_button(chat.widget().upcast_ref(),"Tela cheia").emit_clicked();super::performance::settle(context);
    let fullscreen=video.root().and_downcast::<gtk::Window>().unwrap();assert_ne!(fullscreen.clone().upcast::<gtk::Widget>(),test_window.clone().upcast::<gtk::Widget>());assert!(fullscreen.is_visible());assert_eq!(video.media_stream().unwrap(),stream.clone().upcast::<gtk::MediaStream>());assert!(stream.is_muted()&&(stream.volume()-0.3).abs()<0.001);
    chat.state().get_mut().model.transfers.release_offscreen_playback(&HashSet::new());assert_eq!(chat.model().transfers.videos.len(),1,"fullscreen playback survives background timeline eviction");
    chat.emit(ChatMsg::ApplyChange(Change::Edit(message,"Updated while fullscreen".into(),Some(chrono::Utc::now()))));super::performance::settle(context);assert_eq!(video.root().and_downcast::<gtk::Window>().unwrap(),fullscreen,"message reconciliation must not pull the player out of fullscreen");
    find_button(fullscreen.upcast_ref(),"Sair da tela cheia").emit_clicked();super::performance::settle(context);assert!(!fullscreen.is_visible());assert_eq!(video.root().and_downcast::<gtk::Window>().unwrap().upcast::<gtk::Widget>(),test_window.clone().upcast::<gtk::Widget>());assert!(!stream.is_playing()&&stream.is_muted()&&(stream.volume()-0.3).abs()<0.001);
    let file_path=chat.model().transfers.videos[0].file.0.clone();find_button(chat.widget().upcast_ref(),"Fechar vídeo").emit_clicked();pump(context);assert!(chat.model().transfers.videos.is_empty());assert!(!stream.is_playing()&&stream.file().is_none());assert!(file_path.exists(),"closing playback retains its bounded session cache");
    for _ in 0..4{
        find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());
        let stream=chat.model().transfers.videos[0].media.clone();stream.set_muted(true);until(context,||stream.is_prepared()||stream.error().is_some());assert!(stream.error().is_none());
        let video=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Video>().ok()).unwrap();let path=chat.model().transfers.videos[0].file.0.clone();find_button(chat.widget().upcast_ref(),"Fechar vídeo").emit_clicked();pump(context);
        assert!(!stream.is_playing()&&stream.file().is_none()&&video.media_stream().is_none()&&path.exists());
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
    assert!(chat.model().transfers.videos.is_empty());assert!(!file_path.exists(),"deletion removes cached video files as well as playback");
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(1, 1).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let image = bytes.into_inner();
    chat.emit(ChatMsg::Transfer(TransferMsg::Bytes { token:None,
        epoch, message, key: attachment, preview: false, result: Ok(image.clone())
    }));
    pump(context);
    assert!(!chat.model().transfers.textures.contains_key(&attachment));
    let video_message=Uuid::new_v4();let video_attachment=Uuid::new_v4();chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!({"id":video_message,"channel_id":channel,"author_id":user,"created_at":"2026-10-03T12:01:00Z","attachments":[{"id":video_attachment,"mime_type":"video/webm","original_file_name":"clip.webm","size_bytes":8293,"created_at":"2026-10-03T12:01:00Z"}]})).unwrap()));pump(context);
    chat.emit(ChatMsg::Transfer(TransferMsg::AttachmentVideo{message:video_message,attachment:video_attachment}));chat.emit(ChatMsg::Transfer(TransferMsg::CloseVideo{message:video_message,key:video_attachment}));pump(context);until(context,||chat.model().transfers.jobs.iter().all(|job|job.is_finished()));pump(context);assert!(chat.model().transfers.videos.is_empty(),"closing while downloading must reject the delayed file");
    find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("video-{video_message}-{video_attachment}")));let attachment_stream=chat.model().transfers.videos[0].media.clone();attachment_stream.set_muted(true);find_button(chat.widget().upcast_ref(),"Tela cheia").emit_clicked();super::performance::settle(context);let fullscreen=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Vídeo em tela cheia")&&w.is_visible())).unwrap();let delete_epoch=chat.model().actions.epoch;let path=chat.model().transfers.videos[0].file.0.clone();chat.emit(ChatMsg::Action(ActionMsg::DeleteFinished{epoch:delete_epoch,id:video_message,result:Ok(())}));pump(context);assert!(!path.exists()&&!fullscreen.is_visible(),"deleting the attachment also closes fullscreen");chat.emit(ChatMsg::DeleteMessage(video_message));pump(context);assert!(chat.model().transfers.videos.is_empty()&&!attachment_stream.is_playing()&&attachment_stream.file().is_none(),"deleting a message stops attachment playback");
    let gif_message=Uuid::new_v4();let gif_key=Uuid::new_v4();let gif=crate::media::animation::fixture();
    use base64::Engine;
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!({"id":gif_message,"channel_id":channel,"content":"Animated fixture","created_at":"2026-10-03T12:02:00Z","previews":[{"id":gif_key,"url":"https://example.test/animated","kind":"og","image_mime_type":"image/gif","image_data":base64::engine::general_purpose::STANDARD.encode(gif),"fetched_at":"2026-10-03T12:02:00Z"}]})).unwrap()));
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(gif_message)));
    until(context,||chat.model().transfers.animations.contains_key(&gif_key));
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message:gif_message,key:gif_key}));pump(context);let viewer=chat.model().transfers.images.last().unwrap().2.clone();until(context,||viewer.is_mapped());
    let picture=viewer.child().and_downcast::<gtk::Picture>().unwrap();assert!(picture.paintable().is_some_and(|p|p.is::<crate::media::animation::Animation>()),"the viewer also keeps the GIF animated");
    let controllers=picture.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>()).unwrap();
    click.emit_by_name::<()>("released",&[&1i32,&f64::from(picture.width()/2),&f64::from(picture.height()/2)]);assert!(viewer.is_visible(),"clicking the image keeps it open");
    click.emit_by_name::<()>("released",&[&1i32,&1.0f64,&1.0f64]);pump(context);assert!(!viewer.is_visible(),"clicking the letterbox outside the image closes it");
    chat.emit(ChatMsg::DeleteMessage(gif_message));pump(context);
    // Shared metadata must keep the newest payload, without copying base64
    // between messages referencing the same preview.
    let shared=Uuid::new_v4();let request=Uuid::new_v4();
    let shared_messages=(1..=2).map(|width|{
        let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(width,1).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        serde_json::from_value(json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":"2026-10-03T12:01:00Z","previews":[{
            "id":shared,"url":"https://example.test/shared","kind":"og","title":"Shared image","image_mime_type":"image/png",
            "fetched_at":if width==1{"2026-10-03T12:00:00Z"}else{"2026-10-03T12:01:00Z"},"image_data":base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
        }]})).unwrap()
    }).collect();
    chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});
    chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(crate::models::MessageListResponse{channel_id:channel,messages:shared_messages,has_more:false})});
    until(context,||chat.model().transfers.textures.contains_key(&shared)&&chat.model().media_render_pending.is_none());
    assert_eq!(chat.model().transfers.textures[&shared].width(),2,"metadata and image must come from the same newest preview");
    assert!(chat.model().history.messages.iter().flat_map(|m|m.embeds.iter().flatten()).all(|p|p.image_data.is_none()),"encoded preview images must not be retained or cloned into summaries");
    let references:Vec<_>=chat.model().history.messages.iter().filter(|m|m.embeds.iter().flatten().any(|p|p.id==shared)).map(|m|m.id).collect();
    assert_eq!(references.len(),2);let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(shared,token);
    chat.emit(ChatMsg::DeleteMessage(references[0]));chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch:chat.model().transfers.epoch,message:references[0],key:shared,token,result:Some(crate::media::PreparedImage{width:3,height:1,pixels:vec![255;12]})}));
    until(context,||chat.model().transfers.textures[&shared].width()==3);
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message:references[1],key:shared}));pump(context);let viewer=chat.model().transfers.images.last().unwrap().2.clone();
    chat.emit(ChatMsg::Action(ActionMsg::DeleteFinished{epoch:chat.model().actions.epoch,id:references[1],result:Ok(())}));pump(context);assert!(!viewer.is_visible());assert!(chat.model().transfers.images.is_empty());
    let mut bounded = Transfers::default();let first=Uuid::new_v4();bounded.cache(first,&image);
    for _ in 0..40 {bounded.cache(Uuid::new_v4(), &image);}
    assert_eq!(bounded.textures.len(), 32);assert!(!bounded.textures.contains_key(&first));assert!(bounded.request(first),"eviction must make visible media eligible again");assert!(!bounded.request(first),"in-flight work must stay deduplicated");bounded.failed(first);assert!(!bounded.request(first),"failures use a retry delay");bounded.failed.remove(&first);assert!(bounded.request(first));
    chat.emit(ChatMsg::ClearChannel);
    chat.emit(ChatMsg::Transfer(TransferMsg::Bytes { token:None,
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
    photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{token:None,epoch,message:message_id,key:picture_id,preview:false,result:Ok(bytes.into_inner())}));until(context,||photo.model().transfers.textures.contains_key(&picture_id));assert_eq!((photo.model().transfers.textures[&picture_id].width(),photo.model().transfers.textures[&picture_id].height()),(680,170));
    let window=adw::Window::builder().default_width(900).default_height(650).content(photo.widget()).build();window.present();for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let open=find_button(photo.widget().upcast_ref(),"Abrir imagem");let picture=descendants(open.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Picture>().ok()).unwrap();
    assert!((620..=640).contains(&picture.width()),"actual picture width: {}",picture.width());assert!((150..=165).contains(&picture.height()));
    assert!(descendants(photo.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).all(|b|b.label().as_deref()!=Some("Baixar arquivo")),"visual attachments use an unobtrusive download icon");crate::ui::main_window::layout::tests::preview(&window,"inline-image",context);open.emit_clicked();pump(context);assert!(photo.model().transfers.images[0].2.is_visible());let viewer=photo.model().transfers.images[0].2.clone();
    let mut tiny=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(32,32,image::Rgba([48,120,180,255]))).write_to(&mut tiny,image::ImageFormat::Png).unwrap();
    photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{token:None,epoch,message:message_id,key:picture_id,preview:false,result:Ok(tiny.into_inner())}));
    until(context,||photo.model().transfers.textures[&picture_id].width()==32);
    for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let small=descendants(find_button(photo.widget().upcast_ref(),"Abrir imagem").upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Picture>().ok()).unwrap();assert!(small.width()>=470&&small.height()>=470,"a small texture must still be presented as media: {}x{}",small.width(),small.height());
    window.set_default_size(400,650);for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    assert!(small.width()<400&&small.width()>150,"media must shrink within a narrow chat: {}",small.width());assert!((small.width()-small.height()).abs()<=1);crate::ui::main_window::layout::tests::preview(&window,"inline-image-narrow",context);assert!((small.width()-small.height()).abs()<=1,"media aspect ratio remains stable after resize settles: {}x{}",small.width(),small.height());
    let limiter=photo.model().transfers.limiter.clone();let permit=tokio::runtime::Handle::current().block_on(limiter.acquire_many_owned(4)).unwrap();
    let mut encoded=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(32,32).write_to(&mut encoded,image::ImageFormat::Png).unwrap();photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{token:None,epoch,message:message_id,key:picture_id,preview:false,result:Ok(encoded.into_inner())}));pump(context);assert!(photo.model().transfers.image_tokens.contains_key(&picture_id));
    let ticks=std::rc::Rc::new(std::cell::Cell::new(0));let count=ticks.clone();let timer=gtk::glib::timeout_add_local(std::time::Duration::from_millis(5),move ||{count.set(count.get()+1);gtk::glib::ControlFlow::Continue});for _ in 0..20{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}timer.remove();assert!(ticks.get()>5,"GTK remains responsive while image work waits off-thread");
    photo.emit(ChatMsg::ApplyChange(Change::Moderation(message_id,picture_id,"blocked".into())));pump(context);assert!(!viewer.is_visible());assert!(photo.model().transfers.images.is_empty());drop(permit);until(context,||photo.model().transfers.jobs.iter().all(|job|job.is_finished()));pump(context);assert!(!photo.model().transfers.textures.contains_key(&picture_id),"delayed decoding cannot restore blocked media");window.close();
}

pub(super) fn giphy_widgets(message:&Message,media:&Transfers,sender:&ComponentSender<ChatModel>)->Vec<gtk::Widget>{
    crate::media::giphy::ids(message.content.as_deref().unwrap_or("")).iter().filter_map(|id|{
        let key=*media.giphy.get(id)?;let button=gtk::Button::new();button.set_halign(gtk::Align::Start);button.set_has_frame(false);button.add_css_class("papo-inline-image");
        if let Some(picture)=media.picture(key){let texture=media.textures.get(&key)?;button.set_child(Some(&crate::media::frame::image_frame(&picture,texture,true)));button.set_tooltip_text(Some("Abrir GIF"));let input=sender.input_sender().clone();let message=message.id;button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));});}
        else{button.set_label("Carregar GIF do Giphy");let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::RetryImage(key)));});}
        Some(if media.textures.contains_key(&key){button.upcast()}else{media.placeholder(key,&button,true).upcast()})
    }).collect()
}

#[cfg(test)]pub(super) fn exercise_cache_scroll(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,pump,until};use tokio::io::{AsyncReadExt,AsyncWriteExt};
    let requests=std::sync::Arc::new(std::sync::Mutex::new(HashMap::<Uuid,usize>::new()));let counts=requests.clone();
    let (api,server)=tokio::runtime::Handle::current().block_on(async move{
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let api=crate::api::ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
        let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(512,32).write_to(&mut png,image::ImageFormat::Png).unwrap();let png=png.into_inner();
        let server=tokio::spawn(async move{loop{let (mut socket,_)=listener.accept().await.unwrap();let png=png.clone();let counts=counts.clone();tokio::spawn(async move{
            let mut request=vec![];loop{let mut data=[0;2048];let n=socket.read(&mut data).await.unwrap();if n==0{return;}request.extend_from_slice(&data[..n]);if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}}
            let request=String::from_utf8(request).unwrap();let path=request.split_whitespace().nth(1).unwrap();let key=path.split('/').nth(2).unwrap().parse::<Uuid>().unwrap();*counts.lock().unwrap().entry(key).or_default()+=1;
            let header=format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",png.len());socket.write_all(header.as_bytes()).await.unwrap();socket.write_all(&png).await.unwrap();
        });}});(api,server)
    });
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let start=chrono::Utc::now();let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"media scrolling","created_at":start})).unwrap();
    let keys:Vec<_>=(0..80).map(|_|Uuid::new_v4()).collect();let messages:Vec<Message>=keys.iter().enumerate().map(|(i,key)|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":start+chrono::Duration::seconds(i as i64),"content":format!("Picture {i}"),"attachments":[{"id":key,"mime_type":"image/png","original_file_name":"wide.png","size_bytes":128,"created_at":start}]})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();chat.state().get_mut().model.actions.api=Some(api);chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access{read:true,..Default::default()}});
    let window=adw::Window::builder().default_width(900).default_height(650).content(chat.widget()).build();window.present();
    for index in [0,10,20,30,40,50,60,70]{chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[index].id)));until(context,||chat.model().transfers.textures.contains_key(&keys[index]));super::performance::settle(context);}
    assert!(requests.lock().unwrap().len()>32,"scrolling must exercise actual cache eviction");assert!(chat.model().transfers.textures.len()<=32);
    let before=requests.lock().unwrap().get(&keys[0]).copied().unwrap();assert!(!chat.model().transfers.textures.contains_key(&keys[0]));
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[0].id)));until(context,||chat.model().transfers.textures.contains_key(&keys[0]));assert!(requests.lock().unwrap()[&keys[0]]>before,"returning to an evicted visible image reloads it automatically");
    // Thin decoded images expose more neighboring placeholders than their
    // initial geometry did. A momentarily empty request set between layout and
    // the next prefetch batch is not quiescence. Require the complete pipeline
    // to go quiet before checking that the stationary viewport stays quiet.
    let last_count=std::cell::Cell::new(0usize);let changed_at=std::cell::Cell::new(std::time::Instant::now());
    until(context,||{
        let count=requests.lock().unwrap().values().sum();
        if count!=last_count.get(){last_count.set(count);changed_at.set(std::time::Instant::now());}
        let model=chat.model();model.transfers.requested.is_empty()&&model.media_render_pending.is_none()&&model.transfers.jobs.iter().all(|job|job.is_finished())&&changed_at.get().elapsed()>=std::time::Duration::from_millis(250)
    });
    let before=requests.lock().unwrap().clone();let count:usize=before.values().sum();super::performance::settle(context);super::performance::settle(context);let after=requests.lock().unwrap().clone();assert_eq!(after.values().sum::<usize>(),count,"a stationary viewport must not evict and refetch in a loop: {:?}",keys.iter().enumerate().filter_map(|(i,key)|{let a=before.get(key).copied().unwrap_or(0);let b=after.get(key).copied().unwrap_or(0);(a!=b).then_some((i,a,b))}).collect::<Vec<_>>());
    let widgets=descendants(chat.widget().upcast_ref());assert!(widgets.iter().any(|w|w.is::<gtk::Picture>()));window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);server.abort();
}

pub(super) fn update_media_box(container:&gtk::Box,message:&Message,media:&Transfers,sender:&ComponentSender<ChatModel>){
    let mut existing=HashMap::new();let mut child=container.first_child();while let Some(w)=child{child=w.next_sibling();existing.insert(w.widget_name().to_string(),w);}
    let state=|key:Uuid|format!("{}-{}-{}-{}",key,media.textures.get(&key).map_or(0,|t|t.as_ptr() as usize),media.animations.get(&key).map_or(0,|(a,_)|a.as_ptr() as usize),media.videos.iter().find(|p|p.message==message.id&&p.key==key).map_or(0,|p|p.view.as_ptr() as usize));
    let mut ordered=vec![];
    for attachment in message.attachments.iter().flatten(){let name=format!("attachment-{}-{}-{}",state(attachment.id),media.visible(attachment),media.video_requests.contains_key(&(message.id,attachment.id)));let widget=existing.remove(&name).unwrap_or_else(||attachment_widget(attachment,message.id,media,sender).upcast());widget.set_widget_name(&name);ordered.push(widget);}
    for id in crate::media::giphy::ids(message.content.as_deref().unwrap_or("")){
        if let Some(key)=media.giphy.get(&id){let name=format!("giphy-{}",state(*key));let widget=existing.remove(&name).unwrap_or_else(||{let mut single=message.clone();single.content=Some(format!("giphy:{id}"));giphy_widgets(&single,media,sender).remove(0)});widget.set_widget_name(&name);ordered.push(widget);}
    }
    for preview in message.embeds.iter().flatten(){let name=format!("preview-{}-{}",state(preview.id),media.video_requests.contains_key(&(message.id,preview.id)));let widget=existing.remove(&name).unwrap_or_else(||preview_widget(preview,message.id,media,sender).upcast());widget.set_widget_name(&name);ordered.push(widget);}
    for widget in existing.into_values(){container.remove(&widget);}
    let mut previous=None;for widget in ordered{if widget.parent().is_none(){container.insert_child_after(&widget,previous.as_ref());}else if widget.prev_sibling()!=previous{container.reorder_child_after(&widget,previous.as_ref());}previous=Some(widget);}
}
