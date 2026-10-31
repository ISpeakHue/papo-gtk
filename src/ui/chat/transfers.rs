//! EN-US: Streaming transfers and bounded media textures owned by the chat session.
//! PT-BR: Transferências em fluxo e texturas de mídia limitadas, pertencentes à sessão do chat.
use super::*;
mod viewer;
#[cfg(test)]pub(crate) mod pins_tests;
pub(super) mod gifs;
mod author;
mod external;
#[cfg(test)]pub(crate) use external::exercise as exercise_external;
mod encoded;
pub(super) mod resident;
#[cfg(test)]mod recent_tests;
#[cfg(test)]pub(crate) use recent_tests::exercise as exercise_recent;
mod loading;
use loading::queue_image;
#[cfg(test)]pub(crate) use loading::exercise as exercise_loading;
#[cfg(test)]mod attachments;
#[cfg(test)]mod mov_tests;
#[cfg(test)]pub(crate) use mov_tests::exercise as exercise_mov;
#[cfg(test)]mod cache_checks;
#[cfg(test)]pub(crate) use cache_checks::exercise as exercise_cache;
#[cfg(test)]pub(crate) use attachments::exercise as exercise_attachments;
#[cfg(test)]mod fixes9;
#[cfg(test)]pub(crate) use fixes9::{exercise as exercise9,exercise_backend_gifs};
#[cfg(test)]mod geometry;
#[cfg(test)]pub(crate) use geometry::{exercise as exercise_geometry,exercise_metadata};
#[cfg(test)]pub(crate) use author::exercise as author_exercise;
#[cfg(test)] pub(crate) use viewer::exercise as exercise_viewer;
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
    InvalidateWebhookAvatar(Uuid),
    RetryExternalAvatar(Uuid), AvatarBudgetReady(Uuid),
    ExternalAvatar{key:String,token:Uuid,image:Option<crate::media::PreparedImage>,failure:Option<crate::api::AvatarFailure>},
    DismissNotice{epoch:Uuid,token:Uuid},
    FirstFrame{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,bytes:std::sync::Arc<[u8]>,dimensions:Option<(u32,u32)>,image:crate::media::PreparedImage},
    Decoded{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,bytes:std::sync::Arc<[u8]>,dimensions:Option<(u32,u32)>,preview_only:bool,image:Option<crate::media::PreparedImage>,animation:Option<crate::media::animation::PreparedAnimation>},
    AuthorReady{epoch:Uuid,key:Uuid,token:Uuid,result:Option<crate::media::PreparedImage>},
    ViewerReady{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,result:anyhow::Result<crate::media::PreparedImage>},
    ViewerClosed{epoch:Uuid,token:Uuid},
    PasteImage(u64),
    ClipboardTexture{epoch:Uuid,channel:Uuid,request:Uuid,result:anyhow::Result<gtk::gdk::Texture>},
    PasteReady{epoch:Uuid,channel:Uuid,request:Uuid,result:anyhow::Result<Vec<UploadFile>>},
    AnimationReady{epoch:Uuid,message:Uuid,key:Uuid,token:Uuid,from_video:bool,encoded:Option<std::sync::Arc<[u8]>>,result:Option<crate::media::animation::PreparedAnimation>},
    ImageReady{epoch:Uuid,key:Uuid,token:Uuid,result:Option<crate::media::PreparedImage>},
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
        epoch: Uuid, message: Uuid, key: Uuid, token:Option<Uuid>, result: anyhow::Result<Vec<u8>>
    }, File {
        epoch: Uuid, message: Uuid, key: Uuid, token:Option<Uuid>, destination: Option<PathBuf>, result: anyhow::Result<TemporaryFile>
    }, Saved {
        epoch: Uuid, message: Uuid, result: anyhow::Result<()>
    },
}
pub(super) struct Transfers {
    pub notice:Option<(Uuid,String)>,
    pub(super) resident:resident::Cache,
    recent_channel:Option<Uuid>,
    recent_keys:HashMap<Uuid,HashSet<Uuid>>,preview_owners:HashMap<Uuid,HashSet<Uuid>>,
    visible_keys:HashSet<Uuid>,
    static_previews:HashSet<Uuid>,
    pub(super) encoded:encoded::Cache,
    poster_limiter:std::sync::Arc<tokio::sync::Semaphore>,
    decode_limiter:std::sync::Arc<tokio::sync::Semaphore>,
    animation_limiter:std::sync::Arc<tokio::sync::Semaphore>,
    poster_attempted:HashSet<Uuid>,
    pub(super) sent_attachments:HashSet<Uuid>,
    authors:author::Authors,
    pub(super) external:external::Avatars,
    viewer_jobs:HashMap<Uuid,tokio::task::JoinHandle<()>>,
    viewer_limiter:std::sync::Arc<tokio::sync::Semaphore>,
    pub(super) clipboard_request: Option<Uuid>,
    media_jobs:HashMap<Uuid,Vec<tokio::task::AbortHandle>>,
    pub(super) saving:usize,
    pub(super) revision: u64,
    normalized_revision:Option<u64>,
    #[cfg(test)]pub(super) metadata_visits:usize,
    #[cfg(test)]pub(super) candidate_visits:usize,
    dismissed_images:HashSet<(Uuid,Uuid)>,
    wanted:HashSet<Uuid>,
    sizes:HashMap<Uuid,(i32,i32)>,
    frames:std::rc::Rc<std::cell::RefCell<HashMap<(Uuid,bool),(i32,i32)>>>,
    pub opening:std::rc::Rc<std::cell::Cell<bool>>,
    embeds:HashMap<Uuid,Embed>,
    video_files:HashMap<Uuid,(std::sync::Arc<TemporaryFile>,u64)>,
    video_order:VecDeque<Uuid>,
    pub scope:HashSet<Uuid>,
    pub priority:HashMap<Uuid,u64>,
    pub dirty:HashSet<Uuid>,
    failed:HashMap<Uuid,(u32,std::time::Instant)>,
    resolved:HashSet<Uuid>,
    payloads:HashMap<Uuid,(chrono::DateTime<chrono::Utc>,String)>,
    payload_order:VecDeque<Uuid>,
    gif_requested:HashSet<Uuid>,gif_resolved:HashSet<Uuid>,
    giphy:HashMap<String,Uuid>,animations:HashMap<Uuid,(crate::media::animation::Animation,usize)>,
    pub(super) image_tokens:HashMap<Uuid,Uuid>,preview_tokens:HashMap<Uuid,Uuid>, pub(super) epoch: Uuid, limiter: std::sync::Arc<tokio::sync::Semaphore>, pub(super) textures: HashMap<Uuid, gtk::gdk::Texture>, order: VecDeque<Uuid>, requested: HashSet<Uuid>, revealed: HashSet<Uuid>, pub(super) jobs: Vec<tokio::task::JoinHandle<()>>, uploads: HashMap<Uuid, tokio::task::JoinHandle<()>>, pub progress: u64, video_requests:HashMap<(Uuid,Uuid),Uuid>,videos: Vec<Playback>,images:Vec<(Uuid,Uuid,gtk::Window)>,
}
// Only immutable cached media is shared; players and pending work stay scoped.
#[derive(Clone, Default)]
pub(in crate::ui::chat) struct PinnedMedia {
    textures: HashMap<Uuid, gtk::gdk::Texture>,
    animations: HashMap<Uuid, (crate::media::animation::Animation, usize)>,
    sizes: HashMap<Uuid, (i32, i32)>,
    static_previews: HashSet<Uuid>,
    embeds: Vec<Embed>,
    external: external::Cached,
}
// Keep the temporary file alive until the player and fullscreen view are detached.
struct Playback {message:Uuid,key:Uuid,view:gtk::Box,media:gtk::MediaFile,_file:std::sync::Arc<TemporaryFile>,fullscreen:std::rc::Rc<super::video::Fullscreen>}
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

}
impl Default for Transfers {
    fn default() -> Self {
        Self {
            notice:None,encoded:Default::default(),resident:Default::default(),recent_channel:None,recent_keys:Default::default(),preview_owners:Default::default(),visible_keys:Default::default(),static_previews:Default::default(),
            decode_limiter:std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
            animation_limiter:std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            poster_limiter:std::sync::Arc::new(tokio::sync::Semaphore::new(1)),poster_attempted:Default::default(),
            sent_attachments:Default::default(),
            authors:Default::default(),external:Default::default(),
            viewer_jobs:Default::default(),viewer_limiter:std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            clipboard_request: None,
            media_jobs:Default::default(),saving:0,
            revision: 0,normalized_revision:None,
            #[cfg(test)]metadata_visits:0,
            #[cfg(test)]candidate_visits:0,
            dismissed_images:Default::default(),wanted:Default::default(),sizes:Default::default(),frames:Default::default(),opening:Default::default(),embeds:Default::default(),video_files:Default::default(),video_order:Default::default(),scope:Default::default(),priority:Default::default(),dirty:Default::default(),failed:Default::default(),resolved:Default::default(),payloads:Default::default(),payload_order:Default::default(),
            gif_requested:Default::default(),gif_resolved:Default::default(),giphy:Default::default(),animations:Default::default(),image_tokens:Default::default(),preview_tokens:Default::default(), epoch: Uuid::new_v4(), limiter: std::sync::Arc::new(tokio::sync::Semaphore::new(4)), textures: HashMap::new(), order: VecDeque::new(), requested: HashSet::new(), revealed: HashSet::new(), jobs: Vec::new(), uploads: HashMap::new(), progress: 0, video_requests:Default::default(),videos: Vec::new(),images:Vec::new()
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
    pub(in crate::ui::chat) fn pinned_media(&self, messages: &[Message]) -> PinnedMedia {
        let keys: HashSet<_> = messages.iter().flat_map(|m| m.attachments.iter().flatten().map(|a| a.id).chain(m.embeds.iter().flatten().map(|e| e.id))).collect();
        PinnedMedia {
            textures: self.textures.iter().filter(|(id,_)| keys.contains(id)).map(|(id,v)| (*id,v.clone())).collect(),
            animations: self.animations.iter().filter(|(id,_)| keys.contains(id)).map(|(id,v)| (*id,v.clone())).collect(),
            sizes: self.sizes.iter().filter(|(id,_)| keys.contains(id)).map(|(id,v)| (*id,*v)).collect(),
            static_previews: self.static_previews.intersection(&keys).copied().collect(),
            embeds: self.embeds.values().filter(|e| keys.contains(&e.id)).cloned().collect(),
            external: self.external.cached(messages),
        }
    }
    pub(in crate::ui::chat) fn seed_pinned(&mut self, media: PinnedMedia) {
        for (id, image) in media.textures {
            if self.textures.contains_key(&id) { continue; }
            self.textures.insert(id,image);self.order.push_back(id);
            if let Some(animation)=media.animations.get(&id) { self.animations.insert(id,animation.clone());self.gif_resolved.insert(id); }
            if media.static_previews.contains(&id) { self.static_previews.insert(id); }
        }
        for (id,size) in media.sizes { self.sizes.entry(id).or_insert(size); }
        for embed in media.embeds { if !self.embeds.contains_key(&embed.id) { self.remember_preview(embed); } }
        self.external.seed(media.external);
        self.prune_textures();
    }
    fn media_job(&mut self,key:Uuid,job:tokio::task::JoinHandle<()>){
        self.media_jobs.entry(key).or_default().push(job.abort_handle());
        self.jobs.push(job);
    }
    fn cancel_obsolete_previews(&mut self) {
        // A fast scroll must free download/decode slots for its new destination.
        // Keep the opening batch and explicit viewers; leave displayed textures
        // and row geometry alone until the gesture has finished.
        let obsolete:Vec<_> = self.media_jobs.keys().copied().filter(|key|
            !self.visible_keys.contains(key) && !self.recent_keys.contains_key(key)
            && !self.video_requests.keys().any(|(_,id)|id==key)
            && !self.images.iter().any(|(_,id,w)|id==key&&w.is_visible())
        ).collect();
        for key in obsolete {
            if let Some(jobs)=self.media_jobs.remove(&key){for job in jobs{job.abort();}}
            self.requested.remove(&key);self.image_tokens.remove(&key);self.preview_tokens.remove(&key);
            self.gif_requested.remove(&key);
        }
    }
    pub fn release_offscreen_playback(&mut self,ids:&HashSet<Uuid>)->bool{
        let cancelled:Vec<_>=self.video_requests.keys().filter(|(message,_)|!ids.contains(message)).map(|(_,key)|*key).collect();
        let keys:Vec<_>=self.videos.iter().filter(|p|!ids.contains(&p.message)&&!p.fullscreen.is_open()).map(|p|p.key).chain(self.video_requests.keys().filter(|(message,_)|!ids.contains(message)).map(|(_,key)|*key)).collect();
        self.dirty.extend(keys.iter().copied());
        self.videos.retain(|p|ids.contains(&p.message)||p.fullscreen.is_open());self.video_requests.retain(|(message,_),_|ids.contains(message));
        for key in cancelled{if !self.video_requests.keys().any(|(_,id)|*id==key){self.requested.remove(&key);self.image_tokens.remove(&key);self.gif_requested.remove(&key);}}
        !keys.is_empty()
    }
    pub fn invalidate_message(&mut self, message: Uuid) {
        self.images.retain(|(id,_,w)|{if *id==message{w.close();false}else{true}});
        self.video_requests.retain(|(id,_),_|*id!=message);for p in self.videos.iter().filter(|p|p.message==message){self.video_files.remove(&p.key);self.video_order.retain(|key|*key!=p.key);}self.videos.retain(|p|p.message!=message);
    }
    pub fn invalidate(&mut self, id: Uuid) {
        self.resident.invalidate(id);self.static_previews.remove(&id);
        self.encoded.remove(id);
        self.poster_attempted.remove(&id);
        self.authors.invalidate(id);
        self.images.retain(|(_,key,w)|{if *key==id{w.close();false}else{true}});
        self.video_requests.retain(|(_,key),_|*key!=id);self.videos.retain(|p|p.key!=id);
        self.revealed.remove(&id);self.video_files.remove(&id);self.video_order.retain(|key|*key!=id);self.embeds.remove(&id);self.sizes.remove(&id);
        self.textures.remove(&id);self.animations.remove(&id);self.gif_requested.remove(&id);self.gif_resolved.remove(&id);
        self.payloads.remove(&id);self.payload_order.retain(|key|*key!=id);
        self.failed.remove(&id);self.resolved.remove(&id);
        self.requested.remove(&id);self.preview_tokens.remove(&id);self.image_tokens.remove(&id);
    }
    pub fn switch_channel(&mut self){
        // Reuse compressed bytes and bounded recent previews; release full GIF frames.
        let resident=std::mem::take(&mut self.resident);
        let encoded=std::mem::take(&mut self.encoded);
        let providers=std::mem::take(&mut self.giphy).into_iter().filter(|(_,key)|encoded.contains(*key)).collect();
        let sizes=std::mem::take(&mut self.sizes);let previews=std::mem::take(&mut self.embeds);let resolved=std::mem::take(&mut self.resolved);
        let mut authors=std::mem::take(&mut self.authors);authors.pause();
        let mut external=std::mem::take(&mut self.external);external.clear_queue();
        self.reset();
        self.resident=resident;self.external=external;self.sizes=sizes;self.embeds=previews;self.resolved=resolved;self.authors=authors;self.encoded=encoded;self.giphy=providers;
    }
    pub(super) fn release_distant(&mut self,messages:&[Message])->bool {
        self.images.retain(|(_,_,window)|window.is_visible());
        let mut changed=self.release_offscreen_playback(&self.scope.clone());
        let mut keep=HashSet::new();
        for message in messages.iter().filter(|m|self.scope.contains(&m.id)){
            keep.extend(message.attachments.iter().flatten().map(|a|a.id));keep.extend(message.embeds.iter().flatten().map(|e|e.id));
            for url in crate::media::gif_links::links(message.content.as_deref().unwrap_or("")){if let Some(key)=self.giphy.get(&url){keep.insert(*key);}}
        }
        if let Some(first)=messages.iter().position(|m|self.scope.contains(&m.id)){
            let last=messages.iter().rposition(|m|self.scope.contains(&m.id)).unwrap_or(first);
            for message in &messages[first.saturating_sub(crate::models::INITIAL_MESSAGE_LIMIT)..(last+crate::models::INITIAL_MESSAGE_LIMIT+1).min(messages.len())]{
                keep.extend(message.attachments.iter().flatten().filter(|a|self.visible(a)).map(|a|a.id));keep.extend(message.embeds.iter().flatten().map(|e|e.id));
            }
        }
        keep.extend(self.recent_keys.keys().copied());
        keep.extend(self.images.iter().filter(|(_,_,w)|w.is_visible()).map(|(_,key,_)|*key));
        keep.extend(self.videos.iter().filter(|p|p.fullscreen.is_open()).map(|p|p.key));
        self.authors.release(&keep);
        let keys:HashSet<_>=self.textures.keys().chain(self.animations.keys()).chain(self.video_files.keys()).chain(self.payloads.keys()).chain(self.requested.iter()).chain(self.image_tokens.keys()).chain(self.preview_tokens.keys()).chain(self.media_jobs.keys()).copied().filter(|key|!keep.contains(key)).collect();
        for key in keys {
            if let Some(jobs)=self.media_jobs.remove(&key){for job in jobs{job.abort();}}
            let displayed=self.textures.remove(&key).is_some()|self.animations.remove(&key).is_some();
            if displayed{self.dirty.insert(key);changed=true;}
            self.video_files.remove(&key);self.payloads.remove(&key);self.requested.remove(&key);
            self.image_tokens.remove(&key);self.preview_tokens.remove(&key);self.gif_requested.remove(&key);self.gif_resolved.remove(&key);self.static_previews.remove(&key);
            // Keep frame geometry and provider IDs so unloading cannot collapse a row.
        }
        self.order.retain(|key|keep.contains(key));self.video_order.retain(|key|keep.contains(key));self.payload_order.retain(|key|keep.contains(key));
        changed
    }
    pub(super) fn trim_provider_ids(&mut self,messages:&[Message]){
        self.sent_attachments.retain(|id|messages.iter().any(|m|m.attachments.iter().flatten().any(|a|a.id==*id)));
        let keep:HashSet<_>=messages.iter().flat_map(|m|m.attachments.iter().flatten().map(|a|a.id).chain(m.embeds.iter().flatten().map(|e|e.id))).chain(self.recent_keys.keys().copied()).collect();
        self.frames.borrow_mut().retain(|(key,_),_|keep.contains(key));
        self.failed.retain(|key,_|keep.contains(key));self.resolved.retain(|key|keep.contains(key));
        self.poster_attempted.retain(|key|keep.contains(key));self.revealed.retain(|key|keep.contains(key));
        let urls:HashSet<_>=messages.iter().flat_map(|m|crate::media::gif_links::links(m.content.as_deref().unwrap_or(""))).collect();
        self.giphy.retain(|url,_|urls.contains(url));
    }
    pub fn reset(&mut self) {
        self.notice=None;self.encoded.clear();self.resident.clear();self.recent_channel=None;self.recent_keys.clear();self.visible_keys.clear();self.static_previews.clear();
        self.poster_attempted.clear();
        self.sent_attachments.clear();
        self.authors=Default::default();self.external=Default::default();
        for (_,job) in self.viewer_jobs.drain(){job.abort();}
        self.clipboard_request=None;
        self.media_jobs.clear();self.saving=0;
        self.normalized_revision=None;
        self.epoch = Uuid::new_v4();self.wanted.clear();self.sizes.clear();self.frames.borrow_mut().clear();self.embeds.clear();self.video_files.clear();self.video_order.clear();self.dismissed_images.clear();
        self.scope.clear();self.priority.clear();self.dirty.clear();self.failed.clear();self.resolved.clear();self.payloads.clear();self.payload_order.clear();
        self.requested.clear();self.preview_tokens.clear();self.image_tokens.clear();self.gif_requested.clear();self.gif_resolved.clear();
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
        self.request_embed(key,false)
    }
    fn poster_capacity(&self)->bool{
        // A serialized local poster decoder must not occupy all eight pending
        // media requests. Leave admission room for ordinary images and GIFs.
        self.poster_attempted.iter().filter(|key|self.requested.contains(key)).take(2).count()<2
    }
    fn request_embed(&mut self,key:Uuid,author_needed:bool)->bool{
        self.wanted.insert(key);
        if self.textures.contains_key(&key){self.order.retain(|id|*id!=key);self.order.push_back(key);if !author_needed&&!(self.static_previews.contains(&key)&&self.visible_keys.contains(&key)){return false;}}
        if !author_needed&&!self.visible_keys.contains(&key)&&self.recent_channel.is_some_and(|channel|self.resident.contains(channel,key)){return false;}
        // Decoding a cached item adds no HTTP request and must not wait behind
        // slow downloads. The decode semaphore and candidate cap bound its work.
        if (self.requested.len()>=8&&(author_needed||!self.encoded.contains(key)))||(!author_needed&&self.resolved.contains(&key))||self.failed.get(&key).is_some_and(|(attempt,at)|*attempt>=3||*at>std::time::Instant::now()){return false;}
        self.requested.insert(key)
    }
    fn request_gif(&mut self,key:Uuid)->bool{
        if self.gif_resolved.contains(&key)||self.animations.get(&key).is_some_and(|(a,_)|a.animated()){return false;}
        if self.gif_requested.len()>=2||self.requested.len()>=8||self.image_tokens.contains_key(&key)||self.video_requests.keys().any(|(_,id)|*id==key)||self.failed.get(&key).is_some_and(|(attempt,at)|*attempt>=3||*at>std::time::Instant::now()){return false;}
        if self.requested.insert(key){self.gif_requested.insert(key);true}else{false}
    }
    fn gif_ready(&mut self,key:Uuid,ready:bool){if self.gif_requested.remove(&key)&&ready{self.gif_resolved.insert(key);}}
    fn failed(&mut self,key:Uuid){
        self.dirty.insert(key);self.revision=self.revision.wrapping_add(1);
        self.requested.remove(&key);let attempt=self.failed.get(&key).map_or(1,|(n,_)|n+1);
        self.failed.insert(key,(attempt,std::time::Instant::now()+std::time::Duration::from_secs(2u64.pow(attempt.min(5)))));
    }
    pub(super) fn authorized(&self,messages:&[Message],key:Uuid)->bool{
        messages.iter().any(|m|self.giphy_key(m,key)||m.embeds.iter().flatten().any(|p|p.id==key)||m.attachments.iter().flatten().any(|a|a.id==key&&self.visible(a)))
    }
    pub fn visible(&self, a: &Attachment) -> bool {
        match a.moderation_status.as_deref() {
            Some("blocked") => false,
            // The sender already has this image. An acknowledged upload can
            // use its authorized thumbnail without waiting for moderation's
            // background notification. Explicit sensitive/blocked states win.
            Some("pending"|"processing") => self.sent_attachments.contains(&a.id),
            Some("sensitive") => self.revealed.contains(&a.id), _ => true
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
        let pixels=|textures:&HashMap<Uuid,gtk::gdk::Texture>|textures.values().map(|t|t.width() as u64*t.height() as u64*4).sum::<u64>();
        while self.order.len()>64 || pixels(&self.textures)>64<<20 {
            let index=self.order.iter().position(|id|!self.visible_keys.contains(id)).or_else(||self.order.iter().position(|id|!self.wanted.contains(id))).unwrap_or(0);
            if let Some(old)=self.order.remove(index){self.dirty.insert(old);self.textures.remove(&old);self.animations.remove(&old);self.gif_resolved.remove(&old);}else{break;}
        }
    }
    fn remember_preview(&mut self,mut preview:Embed){
        preview.image_data=None;
        preview.author_image_data=None;
        if !self.sizes.contains_key(&preview.id){
            if let Some((width,height))=[preview.video.as_ref().filter(|_|preview.has_video()),preview.thumbnail.as_ref(),preview.image.as_ref()].into_iter().flatten().find_map(|m|m.width.zip(m.height).filter(|(w,h)|*w>0&&*h>0)){self.remember_size(preview.id,width,height);}
        }
        if self.embeds.len()>=128&&!self.embeds.contains_key(&preview.id){if let Some(old)=self.embeds.keys().copied().find(|id|!self.wanted.contains(id)){self.embeds.remove(&old);self.resolved.remove(&old);}}
        self.embeds.insert(preview.id,preview);
    }
    fn remember_video(&mut self,key:Uuid,file:std::sync::Arc<TemporaryFile>){
        let size=file.0.metadata().map_or(u64::MAX,|m|m.len());if size>128<<20{return;}
        self.video_files.insert(key,(file,size));self.video_order.retain(|id|*id!=key);self.video_order.push_back(key);
        while self.video_files.len()>4||self.video_files.values().map(|(_,size)|size).sum::<u64>()>128<<20{if let Some(old)=self.video_order.pop_front(){self.video_files.remove(&old);}else{break;}}
    }
    fn cache_image(&mut self,id:Uuid,image:crate::media::PreparedImage){
        // The decoded dimensions supersede a placeholder's guessed geometry.
        // The chat applies that layout at idle with its reading anchor intact.
            self.dirty.insert(id);
            self.requested.remove(&id);self.failed.remove(&id);
            self.revision=self.revision.wrapping_add(1);
            let texture=image.texture();if !self.sizes.contains_key(&id){self.remember_size(id,texture.width(),texture.height());}
            self.remember_resident(id,&texture);
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
        chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,key,token,result:Some(crate::media::PreparedImage{width:32,height:32,pixels:vec![255;32*32*4]})}));
    }
    settle(context);
    let model=chat.model();let near:HashSet<_>=messages.iter().filter(|m|model.transfers.scope.contains(&m.id)).flat_map(|m|m.attachments.iter().flatten().map(|a|a.id)).collect();
    assert_eq!(model.transfers.textures.len(),24,"the recent opening batch stays warm after a decode burst");
    assert!(near.iter().all(|key|model.transfers.textures.contains_key(key)),"nearby previews belong to the warm opening batch");
    assert!((1..=2).contains(&(model.render_passes-passes)),"coalesce the decode burst and any subsequent viewport eviction");drop(model);
    let passes=chat.model().render_passes;
    for message in &messages{
        chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch:Uuid::new_v4(),key:message.attachments.as_ref().unwrap()[0].id,token:Uuid::new_v4(),result:None}));
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
        crate::media::gif_links::links(message.content.as_deref().unwrap_or("")).iter().any(|id|self.giphy.get(id)==Some(&key))
    }
    fn image_frame(&self,key:Uuid,child:&impl IsA<gtk::Widget>,embedded:bool,dimensions:Option<(i32,i32)>)->crate::media::frame::Frame {
        // A painted placeholder is only a fallback until dimensions arrive.
        // Using its guessed ratio after decoding would permanently letterbox
        // the image on the first visit. Server thumbnail geometry also keeps
        // original GIF bytes from changing a reserved box. Learned sizes
        // survive unloading when the server has no dimensions.
        let mut frames=self.frames.borrow_mut();
        if frames.len()>=512&&!frames.contains_key(&(key,embedded)) {
            if let Some(old)=frames.keys().copied().find(|(id,_)|!self.wanted.contains(id)){frames.remove(&old);}
        }
        let known=dimensions.or_else(||self.sizes.get(&key).copied()).or_else(||self.textures.get(&key).map(|t|(t.width(),t.height())));
        let (width,height)=frames.get(&(key,embedded)).filter(|_|known.is_none()&&!self.opening.get()).copied().unwrap_or_else(|| {
            let (w,h)=known.unwrap_or((640,360));
            crate::media::frame::fit_size(w,h,embedded)
        });
        drop(frames);
        let frame=crate::media::frame::frame(child,width,height);
        let frames=self.frames.clone();let opening=self.opening.clone();
        frame.on_first_snapshot(move || {
            let mut frames=frames.borrow_mut();
            if frames.len()<512||frames.contains_key(&(key,embedded)) {
                if opening.get(){frames.insert((key,embedded),(width,height));}
                else{frames.entry((key,embedded)).or_insert((width,height));}
            }
        });
        frame
    }
    fn placeholder(&self,key:Uuid,child:&impl IsA<gtk::Widget>,embedded:bool)->crate::media::frame::Frame {
        self.image_frame(key,child,embedded,None)
    }
    fn picture(&self,key:Uuid)->Option<gtk::Picture>{
        if let Some((animation,_))=self.animations.get(&key){Some(animation.picture())}else{self.textures.get(&key).map(crate::media::preview::picture)}
    }
    fn cache_animation(&mut self,key:Uuid,prepared:crate::media::animation::PreparedAnimation){
        self.dirty.insert(key);
        self.requested.remove(&key);self.failed.remove(&key);
        self.revision=self.revision.wrapping_add(1);
        let bytes=prepared.bytes();let animation=prepared.paintable();let first=animation.first();if !self.sizes.contains_key(&key){self.remember_size(key,first.width(),first.height());}
        self.remember_resident(key,&first);
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

enum ImageInput{Bytes(Vec<u8>),Shared(std::sync::Arc<[u8]>),Base64(String)}
#[cfg(test)]mod scheduling_tests{
    use super::*;
    #[test]fn gif_network_waits_leave_room_for_images_and_cached_decodes(){
        let mut transfers=Transfers::default();
        assert!(transfers.request_gif(Uuid::new_v4()));assert!(transfers.request_gif(Uuid::new_v4()));assert!(!transfers.request_gif(Uuid::new_v4()));
        for _ in 0..6{assert!(transfers.request(Uuid::new_v4()));}assert!(!transfers.request(Uuid::new_v4()));
        let cached=Uuid::new_v4();transfers.encoded.insert(cached,Uuid::new_v4(),Uuid::new_v4(),std::sync::Arc::from([1u8]));
        assert!(transfers.request(cached),"cached pixels must decode even while all HTTP slots wait");
        assert!(!transfers.request(cached),"duplicate decodes must still coalesce");
        assert!(!transfers.request_embed(Uuid::new_v4(),true),"metadata requests retain the HTTP limit");
    }
}
fn queue_poster(transfers:&mut Transfers,api:crate::api::ApiClient,sender:&ComponentSender<ChatModel>,message:Uuid,key:Uuid,embedded:bool,thumbnail:bool){
    transfers.poster_attempted.insert(key);
    if !crate::platform::CURRENT.isolated_media_helpers&&(!thumbnail||embedded){transfers.requested.remove(&key);return;}
    let token=Uuid::new_v4();transfers.image_tokens.insert(key,token);let epoch=transfers.epoch;
    let limit=transfers.poster_limiter.clone();let work=transfers.limiter.clone();let output=sender.input_sender().clone();
    transfers.media_job(key,tokio::spawn(async move{
        let result=async {
            if thumbnail&&!embedded {
                let _work=work.clone().acquire_owned().await?;
                match api.media_bytes(&format!("/attachments/{key}/thumbnail"),4<<20).await {
                    Ok(bytes)=>return Ok(bytes),
                    Err(error) if error.downcast_ref::<crate::api::ApiError>().is_some_and(|e|e.status==reqwest::StatusCode::NOT_FOUND)=>{},
                    Err(error)=>return Err(error),
                }
            }
            anyhow::ensure!(crate::platform::CURRENT.isolated_media_helpers,"Conversão isolada de mídia indisponível nesta plataforma.");
            let _poster=limit.acquire_owned().await?;let _work=work.acquire_owned().await?;
            let endpoint=if embedded{format!("/embeds/{key}/video")}else{format!("/attachments/{key}")};
            let file=api.video_preview_file(&endpoint).await?;
            drop(_work);
            crate::media::poster::extract(&file.0).await
        }.await;
        let _=output.send(ChatMsg::Transfer(TransferMsg::Bytes{epoch,message,key,token:Some(token),result}));
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
        if !self.access.read{return;}
        // An opening unread search can replace this intermediate page. Reserve
        // download slots and decode work for the destination that will be shown.
        if self.initial_read.is_some()&&(!self.history_ready||self.history.loading()||self.older_requested){return;}
        if let Some(channel)=self.active_channel.as_ref().map(|c|c.id()){self.transfers.select_recent(channel,&self.history.messages,self.render_window.range.clone(),!self.history_detached);}
        if self.viewport.scrolling(){self.transfers.cancel_obsolete_previews();}
        let Some(api) = self.actions.api.clone() else {
            return;
        };
        #[cfg(test)]{self.media_hydrations+=1;}
        self.transfers.jobs.retain(|j|!j.is_finished());
        self.transfers.media_jobs.retain(|_,jobs|{jobs.retain(|j|!j.is_finished());!jobs.is_empty()});
        let epoch = self.transfers.epoch;
        // A normalized URL can share a preview ID across several messages.
        // Reuse full metadata already hydrated in this history for later summaries.
        // Move encoded payloads out before sharing metadata: cloning a preview
        // used to copy megabytes of base64 for every message referencing it.
        self.transfers.wanted.clear();
        let mut images:HashMap<Uuid,(Option<String>,Option<String>)>=HashMap::new();
        let mut known:HashMap<Uuid,Embed>=HashMap::new();
        let revision=self.history.media_revision();
        if self.transfers.normalized_revision!=Some(revision){
        #[cfg(test)]{self.transfers.metadata_visits+=self.history.messages.len();}
        for p in self.history.messages.iter_mut().flat_map(|m|m.embeds.iter_mut().flatten()){
            self.transfers.authors.invalidate_stale(p);
            if self.transfers.embeds.get(&p.id).is_some_and(|cached|cached.version()==p.version()&&!p.fetch_method.is_empty()&&cached.author!=p.author){self.transfers.invalidate(p.id);}
            if let Some(cached)=self.transfers.embeds.get(&p.id){
                if cached.version()>=p.version(){if p.image_data.is_none()&&p.author_image_data.is_none(){*p=cached.clone();}}
                else{self.transfers.invalidate(p.id);}
            }
            let data=p.image_data.take().filter(|_|!gifs::separate_animation(p));
            let author_data=p.author_image_data.take();
            let score=|p:&Embed|usize::from(p.title.is_some())+usize::from(p.description.is_some())+usize::from(p.thumbnail.is_some())+usize::from(p.author.is_some())+usize::from(p.has_video());
            let same_version=known.get(&p.id).is_some_and(|old|p.version()==old.version());
            if known.get(&p.id).is_none_or(|old|p.version()>old.version()||(same_version&&(score(p)>score(old)||(score(p)==score(old)&&(data.is_some()||author_data.is_some()))))){
                let previous=images.remove(&p.id).filter(|_|same_version).unwrap_or_default();
                let data=data.or(previous.0);let author_data=author_data.or(previous.1);
                if data.is_some()||author_data.is_some(){images.insert(p.id,(data,author_data));}
                known.insert(p.id,p.clone());
            }
        }
        for preview in known.values(){self.transfers.remember_preview(preview.clone());}
        for (key,(data,author_data)) in images{if let Some(preview)=known.get(&key){if let Some(data)=data{self.transfers.payload(key,preview.version(),data);}if let Some(data)=author_data{self.transfers.authors.payload(key,data);}}}
        self.transfers.normalized_revision=Some(revision);
        }
        let scope=self.transfers.scope.clone();let mut candidates=0;let mut gif_videos=vec![];
        // Only inspect the mounted range and the fixed-size opening batch.
        // Older retained history does not add work to each decode completion.
        let recent_start=self.history.messages.len().saturating_sub(crate::models::INITIAL_MESSAGE_LIMIT);
        let range=self.render_window.range.clone();
        let start=range.start.min(recent_start);let end=range.end.min(recent_start);
        let (older,recent)=self.history.messages.split_at_mut(recent_start);
        let mut messages:Vec<_>=older[start..end].iter_mut().filter(|m|scope.contains(&m.id)).chain(recent.iter_mut()).collect();
        #[cfg(test)]{self.transfers.candidate_visits+=messages.len();}
        messages.sort_by_key(|m|(!scope.contains(&m.id),self.transfers.priority.get(&m.id).copied().unwrap_or(u64::MAX),std::cmp::Reverse(m.created_at)));
        self.transfers.external.clear_queue();
        self.transfers.external.request(messages.iter().filter(|m|scope.contains(&m.id)).map(|m|&**m),&api,sender);
        for m in messages {
            let mut links=crate::media::gif_links::links(m.content.as_deref().unwrap_or(""));
            for p in m.embeds.iter().flatten().filter(|p|p.animated_link()){for id in gifs::keys(p){if !links.contains(&id){links.push(id);}}}
            for id in links {
                if candidates>=24{break;}
                let embed=m.embeds.iter().flatten().find(|p|p.animated_link()&&gifs::keys(p).contains(&id));
                // Link discovery and downloads belong to the backend. A link
                // without metadata remains readable until message_embeds arrives.
                let Some(embed)=embed else{continue;};
                let key=embed.id;self.transfers.giphy.insert(id,key);
                if gifs::separate_animation(embed)&&crate::platform::CURRENT.isolated_media_helpers{
                    self.transfers.wanted.insert(key);
                    if gif_videos.len()+self.transfers.video_requests.len()<2&&self.transfers.request_gif(key){candidates+=1;gif_videos.push((m.id,key));}
                }
            }
            for a in m.attachments.iter().flatten() {
                let video=a.is_video();
                if candidates>=24{break;}
                if video&&!crate::platform::CURRENT.isolated_media_helpers&&a.thumbnail_id.is_none()&&!self.transfers.encoded.contains(a.id){continue;}
                if video&&a.thumbnail_id.is_none()&&!self.transfers.encoded.contains(a.id)&&!self.transfers.textures.contains_key(&a.id)&&!self.transfers.poster_capacity(){continue;}
                if (a.mime_type.starts_with("image/")||video) && self.transfers.visible(a)
                && self.transfers.request(a.id) {
                    candidates+=1;
                    if let Some(bytes)=self.transfers.encoded.get(a.id){queue_image(&mut self.transfers,sender,m.id,a.id,ImageInput::Shared(bytes));continue;}
                    if video{queue_poster(&mut self.transfers,api.clone(),sender,m.id,a.id,false,a.thumbnail_id.is_some());continue;}
                    let api = api.clone();
                    let s = sender.clone();
                    let key = a.id;let token=Uuid::new_v4();self.transfers.image_tokens.insert(key,token);
                    // Small GIF originals preserve motion even when the server
                    // generated a still thumbnail. Other images keep thumbnails.
                    let endpoint=if a.mime_type=="image/gif"&&a.size_bytes<=4<<20{format!("/attachments/{key}")}else{format!("/attachments/{key}/thumbnail")};
                    let message = m.id;
                    let limiter = self.transfers.limiter.clone();
                    self.transfers.media_job(key,tokio::spawn(async move {
                        let Ok(_permit) = limiter.acquire_owned().await else {
                            return;
                        };
                        let result = api.media_bytes(&endpoint, 4<<20).await;
                        s.input(ChatMsg::Transfer(TransferMsg::Bytes {
                            epoch, message, key, token:Some(token), result
                        }));
                    }));
                }
            }
            for p in m.embeds.iter_mut().flatten() {
                if p.author.as_ref().is_some_and(|a|a.media.is_some()){self.transfers.wanted.insert(p.id);}
                if scope.contains(&m.id){self.transfers.queue_author_payload(sender,p.id);}
                // New history/WS payloads already carry complete rich metadata.
                // Only thumbnails need an authorized GET for their bytes.
                let author_needed=scope.contains(&m.id)&&self.transfers.authors.needs(p);
                if !scope.contains(&m.id)&&!p.fetch_method.is_empty()&&p.thumbnail.is_none()&&p.image.is_none()&&!p.has_video(){continue;}
                if p.animated_link(){
                    // One owner for an animated link: a backend rendition beats
                    // a blocked provider page and a static thumbnail cannot race it.
                    self.transfers.wanted.insert(p.id);
                    if crate::platform::CURRENT.isolated_media_helpers&&candidates<24&&gifs::separate_animation(p)&&gif_videos.len()+self.transfers.video_requests.len()<2&&self.transfers.request_gif(p.id){candidates+=1;gif_videos.push((m.id,p.id));}
                    if !author_needed&&crate::platform::CURRENT.isolated_media_helpers&&gifs::separate_animation(p)&&(p.has_video()||!p.fetch_method.is_empty()||self.transfers.resolved.contains(&p.id)){continue;}
                }
                if crate::platform::CURRENT.isolated_media_helpers&&p.has_video()&&p.thumbnail.is_none()&&!p.fetch_method.is_empty()&&!author_needed&&!p.animated_link(){
                    if candidates>=24{break;}
                    if !self.transfers.encoded.contains(p.id)&&!self.transfers.textures.contains_key(&p.id)&&!self.transfers.poster_capacity(){continue;}
                    if self.transfers.request(p.id){
                        candidates+=1;
                        if let Some(bytes)=self.transfers.encoded.get(p.id){queue_image(&mut self.transfers,sender,m.id,p.id,ImageInput::Shared(bytes));}
                        else{queue_poster(&mut self.transfers,api.clone(),sender,m.id,p.id,true,false);}
                    }
                    if !author_needed{continue;}
                }
                if !p.fetch_method.is_empty()&&p.thumbnail.is_none()&&p.image.is_none()&&!self.transfers.payloads.contains_key(&p.id)&&!author_needed{continue;}
                if candidates>=24{break;}
                if let Some(full)=known.get(&p.id).or_else(||self.transfers.embeds.get(&p.id)){if full.version()>=p.version()&&full!=p{*p=full.clone();}}
                if !gifs::separate_animation(p)&&self.transfers.payloads.contains_key(&p.id){
                    if self.transfers.request(p.id){
                        candidates+=1;
                        let (_,data)=self.transfers.payloads.remove(&p.id).unwrap();self.transfers.payload_order.retain(|key|*key!=p.id);
                        queue_image(&mut self.transfers,sender,m.id,p.id,ImageInput::Base64(data));
                    }
                }else if !author_needed&&(!self.transfers.textures.contains_key(&p.id)||(self.transfers.static_previews.contains(&p.id)&&self.transfers.visible_keys.contains(&p.id)))&&self.transfers.encoded.contains(p.id){
                    if self.transfers.request(p.id){candidates+=1;if let Some(bytes)=self.transfers.encoded.get(p.id){queue_image(&mut self.transfers,sender,m.id,p.id,ImageInput::Shared(bytes));}}
                }else if self.transfers.request_embed(p.id,author_needed){
                    if p.fetch_method=="gif"{self.transfers.gif_requested.insert(p.id);}
                    candidates+=1;
                    let api = api.clone();
                    let s = sender.clone();
                    let key = p.id;
                    let message = m.id;
                    let token=Uuid::new_v4();self.transfers.preview_tokens.insert(key,token);
                    let limiter = self.transfers.limiter.clone();
                    self.transfers.media_job(key,tokio::spawn(async move {
                        let Ok(_permit) = limiter.acquire_owned().await else {
                            return;
                        };
                        let result=api.get_embed(key).await;
                        let _=s.input_sender().send(ChatMsg::Transfer(TransferMsg::Preview{epoch,message,key,token,result}));
                    }));
                }
            }
        }
        for(message,key) in gif_videos{self.start_download(message,key,None,true,sender);}
    }
    pub(super) fn handle_transfer(&mut self, msg: TransferMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        match msg {
            TransferMsg::InvalidateWebhookAvatar(id)=>{
                self.transfers.external.invalidate(id);let _=sender.output(ChatOutput::ExternalAvatarsChanged);
                let ids:Vec<_>=self.history.messages.iter().filter(|m|m.webhook_id==Some(id)).map(|m|m.id).collect();for id in ids{self.history.mark_dirty(id);}
                self.transfers.revision=self.transfers.revision.wrapping_add(1);
            },
            TransferMsg::AvatarBudgetReady(token)=>{if self.transfers.external.wake_ready(token){if let Some(api)=&self.actions.api{self.transfers.external.start(api,sender);}}},
            TransferMsg::RetryExternalAvatar(id)=>{
                if self.access.read{if let Some(message)=self.history.messages.iter().find(|m|m.id==id){
                    if self.transfers.external.retry(message){if let Some(api)=&self.actions.api{self.transfers.external.request(std::iter::once(message),api,sender);}}
                    else{let token=Uuid::new_v4();let epoch=self.transfers.epoch;self.transfers.notice=Some((token,"O provedor pediu uma pausa antes de tentar carregar o avatar novamente.".into()));let output=sender.input_sender().clone();gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(4),move ||{let _=output.send(ChatMsg::Transfer(TransferMsg::DismissNotice{epoch,token}));});}
                }}
            },
            TransferMsg::ExternalAvatar{key,token,image,failure}=>{
                if self.transfers.external.finish(key.clone(),token,image,failure){
                    let _=sender.output(ChatOutput::ExternalAvatarsChanged);
                    if let Some(api)=self.actions.api.as_ref(){self.transfers.external.start(api,sender);}
                    let ids:Vec<_>=self.history.messages.get(self.render_window.range.clone()).unwrap_or(&[]).iter().filter(|m|external::Avatars::matches(m,&key)).map(|m|m.id).collect();
                    let relevant=!ids.is_empty()||self.actions.pinned.iter().any(|m|external::Avatars::matches(m,&key));
                    for id in ids{self.history.mark_dirty(id);}
                    if relevant{self.transfers.revision=self.transfers.revision.wrapping_add(1);self.render_pins(sender);}
                }
            },
            TransferMsg::DismissNotice{epoch,token}=>{if epoch==self.transfers.epoch&&self.transfers.notice.as_ref().is_some_and(|(id,_)|*id==token){self.transfers.notice=None;}},
            TransferMsg::FirstFrame{epoch,message,key,token,bytes,dimensions,image}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token)||!self.transfers.authorized(&self.history.messages,key){return;}
                if let Some(m)=self.history.messages.iter().find(|m|m.id==message){self.transfers.encoded.insert(key,m.channel_id,message,bytes);}
                if !self.transfers.sizes.contains_key(&key){if let Some((w,h))=dimensions{self.transfers.remember_size(key,w as i32,h as i32);}}
                self.transfers.cache_image(key,image);self.transfers.requested.insert(key);
                crate::performance::record("media_first_frames",1);
            },
            TransferMsg::Decoded{epoch,message,key,token,bytes,dimensions,preview_only,image,animation}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token)||!self.transfers.authorized(&self.history.messages,key){return;}
                if preview_only{self.transfers.static_previews.insert(key);}else{self.transfers.static_previews.remove(&key);}
                if image.is_some()||animation.is_some(){if let Some(m)=self.history.messages.iter().find(|m|m.id==message){self.transfers.encoded.insert(key,m.channel_id,message,bytes);}}
                if (image.is_some()||animation.is_some())&&!self.transfers.sizes.contains_key(&key){if let Some((w,h))=dimensions{self.transfers.remember_size(key,w as i32,h as i32);}}
                let ready=if animation.is_some(){TransferMsg::AnimationReady{epoch,message,key,token,from_video:false,encoded:None,result:animation}}else{TransferMsg::ImageReady{epoch,key,token,result:image}};
                self.handle_transfer(ready,sender,root);
            },
            TransferMsg::AuthorReady{epoch,key,token,result}=>{
                if self.access.read{self.transfers.author_ready(epoch,key,token,result,&self.history.messages);}
            },
            TransferMsg::ViewerClosed{epoch,token}=>{
                if epoch!=self.transfers.epoch{return;}
                if let Some(job)=self.transfers.viewer_jobs.remove(&token){job.abort();}
                self.transfers.images.retain(|(_,_,window)|window.widget_name()!=format!("viewer-{token}"));
                sender.input(ChatMsg::MediaViewport);
            },
            TransferMsg::ViewerReady{epoch,message,key,token,result}=>self.viewer_ready(epoch,message,key,token,result,sender),
            TransferMsg::AnimationReady{epoch,message,key,token,from_video,encoded,result}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token){return;}
                let valid=self.transfers.authorized(&self.history.messages,key);
                if valid&&result.is_some(){if let Some(bytes)=encoded{if let Some(m)=self.history.messages.iter().find(|m|m.id==message){self.transfers.encoded.insert_video(key,m.channel_id,message,bytes);}}}
                if valid&&result.is_none()&&from_video{
                    self.transfers.video_files.remove(&key);self.transfers.video_order.retain(|id|*id!=key);self.transfers.encoded.remove(key);
                }
                let fallback=valid&&result.is_none()&&!from_video&&self.transfers.gif_requested.contains(&key);
                self.transfers.image_tokens.remove(&key);self.transfers.requested.remove(&key);self.transfers.gif_ready(key,valid&&result.is_some());if valid{if let Some(animation)=result{self.transfers.cache_animation(key,animation);}else{self.transfers.failed(key);}}
                if fallback{self.gif_fallback(key,sender);}
            },
            TransferMsg::ImageReady{epoch,key,token,result}=>{
                if epoch!=self.transfers.epoch||!self.access.read||self.transfers.image_tokens.get(&key)!=Some(&token){return;}
                let valid=self.transfers.authorized(&self.history.messages,key);
                let fallback=valid&&result.is_none()&&self.transfers.gif_requested.contains(&key);
                self.transfers.image_tokens.remove(&key);self.transfers.requested.remove(&key);self.transfers.gif_ready(key,valid&&result.is_some());if valid{if let Some(image)=result{self.transfers.cache_image(key,image);}else{self.transfers.failed(key);}}
                if fallback{self.gif_fallback(key,sender);}
            },
            TransferMsg::AttachmentVideo{message,attachment}=>{if self.access.read&&self.history.messages.iter().find(|m|m.id==message).is_some_and(|m|m.attachments.iter().flatten().any(|a|a.id==attachment&&a.is_video()&&self.transfers.visible(a))){self.start_download(message,attachment,None,false,sender);}},
            TransferMsg::CloseVideo{message,key}=>{self.transfers.video_requests.remove(&(message,key));self.videos_retain(message,key);},
            TransferMsg::Preview{epoch,message,key,token,result}=>{
                if epoch!=self.transfers.epoch||self.transfers.preview_tokens.get(&key)!=Some(&token)||!self.access.read{return;}
                if !self.history.messages.iter().any(|m|m.embeds.iter().flatten().any(|p|p.id==key)){self.transfers.preview_tokens.remove(&key);self.transfers.requested.remove(&key);return;}
                self.transfers.preview_tokens.remove(&key);
                match result{
                    Ok(mut preview) if preview.id==key=>{
                        self.transfers.revision=self.transfers.revision.wrapping_add(1);
                        let author_data=preview.author_image_data.take();
                        if self.history.messages.iter().any(|m|self.transfers.scope.contains(&m.id)&&m.embeds.iter().flatten().any(|p|p.id==key)){self.transfers.queue_author(sender,&preview,author_data);}
                        else if let Some(data)=author_data{self.transfers.authors.payload(key,data);}
                        let image_data=preview.image_data.take();
                        if gifs::separate_animation(&preview){self.transfers.requested.remove(&key);self.transfers.resolved.insert(key);}
                        else if let Some(data)=image_data{queue_image(&mut self.transfers,sender,message,key,ImageInput::Base64(data));}
                        else if preview.has_video()&&!preview.animated_link(){if let Some(api)=self.actions.api.clone(){queue_poster(&mut self.transfers,api,sender,message,key,true,false);}}
                        else{self.transfers.requested.remove(&key);self.transfers.resolved.insert(key);if preview.fetch_method=="gif"{self.transfers.gif_requested.remove(&key);self.transfers.failed(key);}}
                        self.transfers.remember_preview(preview.clone());
                        let ids:Vec<_>=self.history.messages.iter().filter(|m|m.embeds.iter().flatten().any(|p|p.id==key)).map(|m|m.id).collect();
                        for id in ids{self.history.apply(Change::Preview(id,preview.clone()));}
                        if preview.fetch_method=="gif"&&self.transfers.failed.contains_key(&key){self.gif_fallback(key,sender);}
                    },
                    Err(error)=>{
                        self.transfers.gif_requested.remove(&key);self.transfers.failed(key);
                        if crate::api::is_session_error(&error){let _=sender.output(ChatOutput::ActionError(error));}
                        else if !crate::api::is_permission_error(&error){self.gif_fallback(key,sender);}
                    },_=>{self.transfers.gif_requested.remove(&key);self.transfers.failed(key);},
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
                let window=gtk::Window::builder().title("Imagem").default_width(900).default_height(680).build();if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));window.set_application(parent.application().as_ref());}
                let token=Uuid::new_v4();let epoch=self.transfers.epoch;window.set_widget_name(&format!("viewer-{token}"));
                let picture=self.transfers.animations.get(&key).map(|(animation,_)|animation.picture()).unwrap_or_else(||gtk::Picture::for_paintable(texture));viewer::install(&window,&picture);dismiss_image(&window,&picture);
                let embed=self.history.messages.iter().find(|m|m.id==message).and_then(|m|m.embeds.iter().flatten().find(|p|p.id==key));
                if embed.is_some_and(Embed::animated_link)||self.transfers.giphy.values().any(|id|*id==key){let source=gifs::source(&self.transfers,message,key,&self.history.messages);gifs::viewer(&window,embed,source.as_deref());}
                if let Some(attachment)=self.history.messages.iter().find(|m|m.id==message).and_then(|m|m.attachments.iter().flatten().find(|a|a.id==key)) {
                    if !attachment.original_file_name.trim().is_empty(){window.set_title(Some(&attachment.original_file_name));}
                    if let Some(header)=window.child().and_then(|root|root.first_child()).and_downcast::<adw::HeaderBar>() {
                        let save=gtk::Button::from_icon_name("papo-folder-download-symbolic");save.set_tooltip_text(Some("Baixar arquivo"));
                        save.update_property(&[gtk::accessible::Property::Label("Baixar arquivo")]);
                        let attachment=attachment.clone();let input=sender.input_sender().clone();
                        save.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::Download{message,attachment:attachment.clone()}));});
                        header.pack_end(&save);
                    }
                }
                let input=sender.input_sender().clone();window.connect_close_request(move |_|{let input=input.clone();gtk::glib::idle_add_local_once(move ||{let _=input.send(ChatMsg::Transfer(TransferMsg::ViewerClosed{epoch,token}));});gtk::glib::Propagation::Proceed});
                window.present();self.transfers.images.retain(|(_,_,w)|w.is_visible());self.transfers.images.push((message,key,window));
                self.load_viewer(message,key,token,sender);
            },
            TransferMsg::PasteImage(epoch)=>self.paste_image(epoch,sender,root),
            TransferMsg::ClipboardTexture{epoch,channel,request,result}=>self.prepare_clipboard_image(epoch,channel,request,result,sender),
            TransferMsg::PasteReady{epoch,channel,request,result}=>{
                if epoch!=self.transfers.epoch||self.transfers.clipboard_request!=Some(request){return;}
                self.transfers.clipboard_request=None;
                self.handle_transfer(TransferMsg::Selected{epoch,channel,result},sender,root);
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
                if epoch!=self.transfers.epoch||self.active_channel.as_ref().map(|c|c.id())!=Some(channel)||self.draft.is_sending()||!self.access.send||!self.access.attachments {
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
                if !self.transfers.requested.contains(&id){self.transfers.failed.remove(&id);self.transfers.resolved.remove(&id);self.transfers.gif_resolved.remove(&id);self.transfers.dirty.insert(id);}
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
                epoch, message, key, token, result
            } => {
                if epoch!=self.transfers.epoch||!self.access.read||token.is_some_and(|token|self.transfers.image_tokens.get(&key)!=Some(&token)) {
                    return;
                }
                let valid=self.transfers.authorized(&self.history.messages,key);
                if !valid{self.transfers.requested.remove(&key);}
                if valid {
                    match result {
                        Ok(bytes) => queue_image(&mut self.transfers,sender,message,key,ImageInput::Bytes(bytes)), Err(e) => {
                            self.transfers.gif_requested.remove(&key);self.transfers.failed(key);self.gif_fallback(key,sender);
                            if self.transfers.poster_attempted.contains(&key){self.transfers.failed.insert(key,(3,std::time::Instant::now()));self.transfers.dirty.insert(key);self.transfers.revision=self.transfers.revision.wrapping_add(1);}
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
                if !self.access.read||!self.history.messages.iter().any(|m|m.id==message&&m.attachments.iter().flatten().any(|a|a.id==attachment.id&&self.transfers.visible(a))) {
                    return;
                }
                let epoch = self.transfers.epoch;
                let parent = root.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                let s = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    if let Ok(file) = gtk::FileDialog::builder().title("Salvar arquivo").initial_name(crate::platform::files::suggested_filename(&attachment.original_file_name)).build().save_future(parent.as_ref()).await {
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
                if epoch!=self.transfers.epoch||!self.access.read||!self.history.messages.iter().any(|m|m.id==message&&m.attachments.iter().flatten().any(|a|a.id==attachment&&self.transfers.visible(a))) {
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
                if epoch==self.transfers.epoch&&destination.is_some(){self.transfers.saving=self.transfers.saving.saturating_sub(1);}
                if epoch!=self.transfers.epoch || !self.access.read||!self.history.messages.iter().any(|m|m.id==message&&if destination.is_some() {
                    m.attachments.iter().flatten().any(|a|a.id==key&&self.transfers.visible(a))
                }
                else {
                    m.embeds.iter().flatten().any(|p|p.id==key&&p.has_video())||m.attachments.iter().flatten().any(|a|a.id==key&&a.is_video()&&self.transfers.visible(a))
                }) {
                    return;
                }
                if let Some(token)=token{if self.transfers.video_requests.get(&(message,key))!=Some(&token){return;}self.transfers.video_requests.remove(&(message,key));}
                match result {
                    Ok(file) => {
                        if self.draft.error.as_deref().is_some_and(|text|matches!(text,"Carregando vídeo…"|"Baixando arquivo…")){self.draft.error=None;}
                        if let Some(destination) = destination {
                            self.transfers.saving+=1;
                            let s = sender.clone();
                            self.transfers.jobs.push(tokio::spawn(async move {
                                let result = crate::api::features::save_download(file, destination).await;
                                s.input(ChatMsg::Transfer(TransferMsg::Saved {
                                    epoch, message, result
                                }));
                            }));
                        }
                        else {
                            let file=std::sync::Arc::new(file);self.transfers.remember_video(key,file.clone());
                            if self.gif_preview(key).is_some(){self.queue_gif_video(message,key,file,sender);}else{self.play_video(message,key,file,sender);}
                        }
                    }, Err(e) => {
                        if self.gif_preview(key).is_some(){
                            self.transfers.gif_requested.remove(&key);self.transfers.failed(key);
                            if crate::api::is_session_error(&e){let _=sender.output(ChatOutput::ActionError(e));}
                        }else{self.draft.error=Some(e.to_string());let _=sender.output(ChatOutput::ActionError(e));}
                    }
                }
            }
            TransferMsg::Saved {
                epoch, message, result
            } => {
                if epoch==self.transfers.epoch{self.transfers.saving=self.transfers.saving.saturating_sub(1);}
                if epoch!=self.transfers.epoch||!self.history.messages.iter().any(|m|m.id==message) {
                    return;
                }
                match result {
                    Ok(())=>{let token=Uuid::new_v4();self.transfers.notice=Some((token,"Arquivo salvo.".into()));let output=sender.input_sender().clone();gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(3),move ||{let _=output.send(ChatMsg::Transfer(TransferMsg::DismissNotice{epoch,token}));});},
                    Err(e)=>{self.transfers.notice=None;self.draft.error=Some(format!("Falha ao salvar: {e}"));}
                }
            }
        }
    }
    fn gif_preview(&self,key:Uuid)->Option<Uuid>{
        self.history.messages.iter().find(|m|m.embeds.iter().flatten().any(|p|p.id==key&&p.animated_link()&&p.has_video())).map(|m|m.id)
    }
    fn gif_fallback(&mut self,key:Uuid,sender:&ComponentSender<Self>){
        // Bound automatic video downloads; decoding uses the shared limiter
        // and an isolated process, without replacing a user-started player.
        if !crate::platform::CURRENT.isolated_media_helpers||self.transfers.video_requests.len()>=2{return;}
        if let Some(message)=self.gif_preview(key){self.start_download(message,key,None,true,sender);}
    }
    fn queue_gif_video(&mut self,message:Uuid,key:Uuid,file:std::sync::Arc<TemporaryFile>,sender:&ComponentSender<Self>){
        let token=Uuid::new_v4();self.transfers.image_tokens.insert(key,token);self.transfers.gif_requested.insert(key);self.transfers.requested.insert(key);let epoch=self.transfers.epoch;let path=file.0.clone();let limiter=self.transfers.decode_limiter.clone();let animations=self.transfers.animation_limiter.clone();let output=sender.input_sender().clone();
        self.transfers.media_job(key,tokio::spawn(async move{
            let Ok(_animation)=animations.acquire_owned().await else{return;};
            let Ok(_permit)=limiter.acquire_owned().await else{return;};
            let result=crate::media::animation::PreparedAnimation::video(&path).await.ok();
            let encoded=if result.is_some()&&tokio::fs::metadata(&path).await.is_ok_and(|m|m.len()<=4<<20){tokio::fs::read(&path).await.ok().map(std::sync::Arc::from)}else{None};drop(file);
            let _=output.send(ChatMsg::Transfer(TransferMsg::AnimationReady{epoch,message,key,token,from_video:true,encoded,result}));
        }));
    }
    fn videos_retain(&mut self,message:Uuid,key:Uuid){self.transfers.videos.retain(|p|p.message!=message||p.key!=key);}
    fn play_video(&mut self,message:Uuid,key:Uuid,file:std::sync::Arc<TemporaryFile>,sender:&ComponentSender<Self>){
                            if self.transfers.videos.len()>=2{self.transfers.videos.remove(0);}
                            let view=gtk::Box::new(gtk::Orientation::Vertical,6);view.set_widget_name(&format!("video-{message}-{key}"));let player=gtk::Box::new(gtk::Orientation::Vertical,6);
                            // Keep the chat action outside the player moved to fullscreen.
                            let close=gtk::Button::with_label("Fechar vídeo");close.set_halign(gtk::Align::End);let input=sender.input_sender().clone();close.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::CloseVideo{message,key}));});view.append(&close);view.append(&player);
                            let media=gtk::MediaFile::for_filename(&file.0);let video=video_surface(&media);video.set_hexpand(true);let frame=crate::media::frame::frame(&video,640,360);player.append(&frame);let controls=super::video::controls(&video,&media);player.append(&controls);
                            let fullscreen=super::video::Fullscreen::new(&view,&player,&frame,&media,&controls);
                            let error=gtk::Label::new(None);error.set_wrap(true);error.add_css_class("error");player.append(&error);let error_now=error.clone();media.connect_error_notify(move |m|{if let Some(e)=m.error(){error.set_text(&format!("Não foi possível reproduzir: {e}"));}});if let Some(e)=media.error(){error_now.set_text(&format!("Não foi possível reproduzir: {e}"));}
                            media.play();self.transfers.videos.push(Playback{message,key,view,media,_file:file,fullscreen});
    }
    fn start_download(&mut self, message: Uuid, key: Uuid, destination: Option<PathBuf>, video: bool, sender: &ComponentSender<Self>) {
        let Some(api) = self.actions.api.clone() else {
            return;
        };
        let inline=destination.is_none();
        if inline&&(self.transfers.video_requests.contains_key(&(message,key))||self.transfers.videos.iter().any(|p|p.message==message&&p.key==key)){return;}
        if inline{if let Some((file,_))=self.transfers.video_files.get(&key){let file=file.clone();self.transfers.video_order.retain(|id|*id!=key);self.transfers.video_order.push_back(key);if self.gif_preview(key).is_some(){self.queue_gif_video(message,key,file,sender);}else{self.play_video(message,key,file,sender);}return;}}
        let token=if inline{let token=Uuid::new_v4();self.transfers.video_requests.insert((message,key),token);Some(token)}else{None};
        let s = sender.clone();
        let epoch = self.transfers.epoch;let gif=self.gif_preview(key).is_some();
        if !gif&&inline{self.draft.error=Some(if video{"Carregando vídeo…"}else{"Baixando arquivo…"}.into());}
        let endpoint = if video {
            format!("/embeds/{key}/video")
        }
        else {
            format!("/attachments/{key}")
        };
        if !inline{self.transfers.saving+=1;}
        let cached=if gif&&inline{self.transfers.encoded.get_video(key)}else{None};
        let job=tokio::spawn(async move {
            let result=async {
                let file=if let Some(bytes)=cached{gif_file(bytes).await?}else{api.download_temporary(&endpoint,if video{if gif{8<<20}else{256<<20}}else{MAX_FILE}).await?};
                if inline{let path=file.0.clone();tokio::task::spawn_blocking(move||crate::media::container::validate(&path)).await??;}
                Ok(file)
            }.await;
            s.input(ChatMsg::Transfer(TransferMsg::File {
                epoch, message, key,token,destination, result
            }));
        });
        if inline{self.transfers.media_job(key,job);}else{self.transfers.jobs.push(job);}
    }
}
// The same isolated video decoder consumes cached compressed renditions. No
// decoded frames or cross-channel playback pipelines are retained in the cache.
async fn gif_file(bytes:std::sync::Arc<[u8]>)->anyhow::Result<TemporaryFile>{
    use tokio::io::AsyncWriteExt;
    let path=std::env::temp_dir().join(format!("papo-gif-{}",Uuid::new_v4()));
    let (temporary,mut file)=crate::api::features::create_temporary(path).await?;file.write_all(&bytes).await?;file.flush().await?;drop(file);Ok(temporary)
}
pub(super) fn attachment_context(row:&gtk::Widget,message:Uuid,x:f64,y:f64,messages:&[Message],media:&Transfers,sender:&ComponentSender<ChatModel>)->bool {
    let mut picked=row.pick(x,y,gtk::PickFlags::DEFAULT);
    while let Some(widget)=picked {
        let key=widget.widget_name().strip_prefix("attachment-").and_then(|name|name.get(..36)).and_then(|id|Uuid::parse_str(id).ok());
        if let Some(a)=key.and_then(|key|messages.iter().find(|m|m.id==message)?.attachments.iter().flatten().find(|a|a.id==key&&media.visible(a))) {
            // Anchor on the actual row, independent of its offscreen toolbar.
            let popover=gtk::Popover::new();popover.set_widget_name("attachment-context-menu");popover.add_css_class("papo-menu");popover.set_parent(row);
            popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x.round() as i32,y.round() as i32,1,1)));
            let save=gtk::Button::with_label("Baixar arquivo");save.add_css_class("flat");let attachment=a.clone();let input=sender.input_sender().clone();let weak=popover.downgrade();
            save.connect_clicked(move |_|{if let Some(p)=weak.upgrade(){p.popdown();}let _=input.send(ChatMsg::Transfer(TransferMsg::Download{message,attachment:attachment.clone()}));});
            popover.set_child(Some(&save));popover.connect_closed(|p|p.unparent());let weak=popover.downgrade();row.connect_destroy(move |_|{if let Some(p)=weak.upgrade(){if p.parent().is_some(){p.unparent();}}});popover.popup();return true;
        }
        if widget==*row{break;}picked=widget.parent();
    }
    false
}

pub(super) fn attachment_widget(a: &Attachment, message: Uuid, media: &Transfers, sender: &ComponentSender<ChatModel>)->gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    card.set_halign(gtk::Align::Start);let visual=a.mime_type.starts_with("image/")||a.is_video();if !visual{card.add_css_class("card");}
    if media.visible(a) {
        if a.is_video(){append_video(&card,media,message,a.id,false,a.video_dimensions().or_else(||a.thumbnail_dimensions()),sender);}
        if a.mime_type.starts_with("image/")&&media.textures.contains_key(&a.id) {
            let image = media.picture(a.id).unwrap();
            image.set_can_shrink(true);
            let open=gtk::Button::new();open.set_has_frame(false);open.set_halign(gtk::Align::Start);open.add_css_class("papo-inline-image");open.set_tooltip_text(Some("Abrir imagem"));open.set_child(Some(&media.image_frame(a.id,&image,false,a.thumbnail_dimensions())));let s=sender.clone();let key=a.id;open.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::OpenImage{message,key})));card.append(&open);
        }
        else if a.mime_type.starts_with("image/") {
            let button = gtk::Button::with_label("Carregar imagem");
            let key = a.id;
            let s = sender.clone();
            button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryImage(key))));
            card.append(&media.image_frame(a.id,&button,false,a.thumbnail_dimensions()));
        }
        if !visual {
            // Non-previewable files retain a named, clickable file tile.
            let button=gtk::Button::with_label(&a.original_file_name);button.add_css_class("flat");
            let attachment=a.clone();let input=sender.input_sender().clone();
            button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::Download{message,attachment:attachment.clone()}));});
            card.append(&button);
        }
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
fn append_video(card:&gtk::Box,media:&Transfers,message:Uuid,key:Uuid,preview:bool,hint:Option<(i32,i32)>,sender:&ComponentSender<ChatModel>){
    if let Some(playback)=media.videos.iter().find(|v|v.message==message&&v.key==key){
        let playing=playback.media.is_playing();
        if let Some(parent)=playback.view.parent().and_downcast::<gtk::Box>(){parent.remove(&playback.view);}else if let Some(parent)=playback.view.parent().and_downcast::<adw::Clamp>(){parent.set_child(None::<&gtk::Widget>);}
        card.append(&playback.view);if playing{playback.media.play();}
    }else{
        let overlay=gtk::Overlay::new();
        if let Some(texture)=media.textures.get(&key){overlay.set_child(Some(&crate::media::preview::picture(texture)));}
        else{
            let placeholder=gtk::Box::new(gtk::Orientation::Vertical,6);placeholder.set_halign(gtk::Align::Center);placeholder.set_valign(gtk::Align::End);placeholder.set_margin_bottom(16);
            let icon=gtk::Image::from_icon_name("papo-video-x-generic-symbolic");icon.set_pixel_size(32);icon.add_css_class("dim-label");placeholder.append(&icon);
            if media.failed.contains_key(&key){let label=gtk::Label::new(Some("Prévia indisponível"));label.add_css_class("dim-label");placeholder.append(&label);}
            overlay.set_child(Some(&placeholder));
        }
        let button=gtk::Button::from_icon_name("papo-media-playback-start-symbolic");button.set_tooltip_text(Some("Reproduzir vídeo"));button.update_property(&[gtk::accessible::Property::Label("Reproduzir vídeo")]);button.add_css_class("papo-media-play");button.set_halign(gtk::Align::Center);button.set_valign(gtk::Align::Center);
        button.set_sensitive(!media.video_requests.contains_key(&(message,key)));let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(if preview{TransferMsg::Video{message,preview:key}}else{TransferMsg::AttachmentVideo{message,attachment:key}}));});overlay.add_overlay(&button);
        let dimensions=hint.or_else(||media.embeds.get(&key).and_then(|p|[p.video.as_ref(),p.thumbnail.as_ref()].into_iter().flatten().find_map(|m|m.width.zip(m.height).filter(|(w,h)|*w>0&&*h>0))))
            .or_else(||media.sizes.get(&key).copied()).or_else(||media.textures.get(&key).map(|t|(t.width(),t.height())));
        let(width,height)=dimensions.map_or((640,360),|(w,h)|crate::media::frame::fit_size(w,h,false));
        card.append(&crate::media::frame::frame(&overlay,width,height));
    }
}

pub(super) fn preview_widget(p: &Embed, message: Uuid, media: &Transfers, users:&HashMap<Uuid,UserSummary>, actions:&Actions, sender: &ComponentSender<ChatModel>)->gtk::Box {
    if p.animated_link(){return gifs::widget(Some(p),p.url.as_deref(),message,p.id,media,sender);}
    let card=super::rich::body(p,media.authors.textures.get(&p.id),users,actions,sender);
    if !p.has_video(){if media.textures.contains_key(&p.id) {
        let image = media.picture(p.id).unwrap();
        image.set_can_shrink(true);
        let open=gtk::Button::new();open.set_has_frame(false);open.set_halign(gtk::Align::Start);open.add_css_class("papo-inline-image");open.set_tooltip_text(Some("Abrir imagem"));open.set_child(Some(&media.image_frame(p.id,&image,true,None)));let s=sender.clone();let key=p.id;open.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::OpenImage{message,key})));card.append(&open);
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
    if p.has_video(){append_video(&card,media,message,p.id,true,None,sender);}
    super::rich::footer(&card,p,false,users,actions,sender);

    // Keep the existing 640-pixel video surface while bounding text-only cards.
    // A clamp centers its child when tightening. Make its natural width equal
    // to the capped card width and align the whole clamp with the message text.
    let maximum=if p.has_video(){700}else{520};
    // Both axes use Start so GTK negotiates width before height. With
    // vertical Fill it asks the clamp for a width at the old row height,
    // which can be smaller than a newly wrapped/loaded card's minimum.
    let clamp=adw::Clamp::builder().maximum_size(maximum).tightening_threshold(maximum).halign(gtk::Align::Start).valign(gtk::Align::Start).child(&card).build();
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
    let composer=composer::field(chat.widget().upcast_ref());composer.grab_focus();
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
    assert!(chat.model().transfers.videos[0]._file.0.exists());assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.is::<gtk::Video>()),"video belongs to the timeline");
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
    let file_path=chat.model().transfers.videos[0]._file.0.clone();find_button(chat.widget().upcast_ref(),"Fechar vídeo").emit_clicked();pump(context);assert!(chat.model().transfers.videos.is_empty());assert!(!stream.is_playing()&&stream.file().is_none());assert!(file_path.exists(),"closing playback retains its bounded session cache");
    for _ in 0..4{
        find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());
        let stream=chat.model().transfers.videos[0].media.clone();stream.set_muted(true);until(context,||stream.is_prepared()||stream.error().is_some());assert!(stream.error().is_none());
        let video=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Video>().ok()).unwrap();let path=chat.model().transfers.videos[0]._file.0.clone();find_button(chat.widget().upcast_ref(),"Fechar vídeo").emit_clicked();pump(context);
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
        epoch, message, key: attachment, result: Ok(image.clone())
    }));
    pump(context);
    assert!(!chat.model().transfers.textures.contains_key(&attachment));
    let video_message=Uuid::new_v4();let video_attachment=Uuid::new_v4();chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!({"id":video_message,"channel_id":channel,"author_id":user,"created_at":"2026-10-03T12:01:00Z","attachments":[{"id":video_attachment,"mime_type":"video/webm","original_file_name":"clip.webm","size_bytes":8293,"created_at":"2026-10-03T12:01:00Z"}]})).unwrap()));pump(context);
    chat.emit(ChatMsg::Transfer(TransferMsg::AttachmentVideo{message:video_message,attachment:video_attachment}));chat.emit(ChatMsg::Transfer(TransferMsg::CloseVideo{message:video_message,key:video_attachment}));pump(context);until(context,||chat.model().transfers.jobs.iter().all(|job|job.is_finished()));pump(context);assert!(chat.model().transfers.videos.is_empty(),"closing while downloading must reject the delayed file");
    find_button(chat.widget().upcast_ref(),"Reproduzir vídeo").emit_clicked();until(context,||!chat.model().transfers.videos.is_empty());assert!(descendants(chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("video-{video_message}-{video_attachment}")));let attachment_stream=chat.model().transfers.videos[0].media.clone();attachment_stream.set_muted(true);find_button(chat.widget().upcast_ref(),"Tela cheia").emit_clicked();super::performance::settle(context);let fullscreen=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Vídeo em tela cheia")&&w.is_visible())).unwrap();let delete_epoch=chat.model().actions.epoch;let path=chat.model().transfers.videos[0]._file.0.clone();chat.emit(ChatMsg::Action(ActionMsg::DeleteFinished{epoch:delete_epoch,id:video_message,result:Ok(())}));pump(context);assert!(!path.exists()&&!fullscreen.is_visible(),"deleting the attachment also closes fullscreen");chat.emit(ChatMsg::DeleteMessage(video_message));pump(context);assert!(chat.model().transfers.videos.is_empty()&&!attachment_stream.is_playing()&&attachment_stream.file().is_none(),"deleting a message stops attachment playback");
    let gif_message=Uuid::new_v4();let gif_key=Uuid::new_v4();let gif=crate::media::animation::fixture();
    use base64::Engine;
    chat.emit(ChatMsg::AddMessage(serde_json::from_value(json!({"id":gif_message,"channel_id":channel,"content":"Animated fixture","created_at":"2026-10-03T12:02:00Z","previews":[{"id":gif_key,"url":"https://example.test/animated","kind":"og","image_mime_type":"image/gif","image_data":base64::engine::general_purpose::STANDARD.encode(gif),"fetched_at":"2026-10-03T12:02:00Z"}]})).unwrap()));
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(gif_message)));
    until(context,||chat.model().transfers.animations.contains_key(&gif_key));
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message:gif_message,key:gif_key}));pump(context);let viewer=chat.model().transfers.images.last().unwrap().2.clone();until(context,||viewer.is_mapped());
    let picture=viewer::picture(&viewer).unwrap();assert!(picture.paintable().is_some_and(|p|p.is::<crate::media::animation::Animation>()),"the viewer also keeps the GIF animated");
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
    chat.emit(ChatMsg::DeleteMessage(references[0]));chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch:chat.model().transfers.epoch,key:shared,token,result:Some(crate::media::PreparedImage{width:3,height:1,pixels:vec![255;12]})}));
    until(context,||chat.model().transfers.textures[&shared].width()==3);
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message:references[1],key:shared}));pump(context);let viewer=chat.model().transfers.images.last().unwrap().2.clone();
    chat.emit(ChatMsg::Action(ActionMsg::DeleteFinished{epoch:chat.model().actions.epoch,id:references[1],result:Ok(())}));pump(context);assert!(!viewer.is_visible());assert!(chat.model().transfers.images.is_empty());
    let mut bounded = Transfers::default();let first=Uuid::new_v4();bounded.cache(first,&image);
    for _ in 0..80 {bounded.cache(Uuid::new_v4(), &image);}
    assert_eq!(bounded.textures.len(), 64);assert!(!bounded.textures.contains_key(&first));assert!(bounded.request(first),"eviction must make visible media eligible again");assert!(!bounded.request(first),"in-flight work must stay deduplicated");bounded.failed(first);assert!(!bounded.request(first),"failures use a retry delay");bounded.failed.remove(&first);assert!(bounded.request(first));
    chat.emit(ChatMsg::ClearChannel);
    chat.emit(ChatMsg::Transfer(TransferMsg::Bytes { token:None,
        epoch, message, key: preview, result: Ok(image)
    }));
    pump(context);
    assert!(chat.model().transfers.textures.is_empty());
    test_window.set_content(None::<&gtk::Widget>);test_window.close();
    // Original source geometry survives a smaller decoded thumbnail and viewer revocation.
    let picture_id=Uuid::new_v4();let message_id=Uuid::new_v4();
    let image_message=serde_json::from_value(json!({"id":message_id,"channel_id":channel,"author_id":user,"created_at":"2026-10-03T12:00:00Z","attachments":[{"id":picture_id,"mime_type":"image/png","original_file_name":"example.png","size_bytes":2048,"created_at":"2026-10-03T12:00:00Z"}]})).unwrap();
    let photo=ChatModel::builder().launch(ChatInit{active_channel:Some(serde_json::from_value(json!({"id":channel,"name":"images","type":"text","created_at":"2026-10-03T12:00:00Z"})).unwrap()),messages:vec![image_message],user_id:Some(user),..Default::default()}).detach();photo.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access{read:true,..Default::default()}});pump(context);
    let epoch=photo.model().transfers.epoch;let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1024,256,image::Rgba([48,120,180,255]))).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
    photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{token:None,epoch,message:message_id,key:picture_id,result:Ok(bytes.into_inner())}));until(context,||photo.model().transfers.textures.contains_key(&picture_id));assert_eq!((photo.model().transfers.textures[&picture_id].width(),photo.model().transfers.textures[&picture_id].height()),(680,170));
    let window=adw::Window::builder().default_width(900).default_height(650).content(photo.widget()).build();window.present();for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let open=find_button(photo.widget().upcast_ref(),"Abrir imagem");let picture=descendants(open.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Picture>().ok()).unwrap();
    assert!((620..=640).contains(&picture.width()),"actual picture width: {}",picture.width());assert!((150..=165).contains(&picture.height()));
    assert!(descendants(photo.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).all(|b|!b.is_mapped()||(b.label().as_deref()!=Some("Baixar arquivo")&&b.icon_name().as_deref()!=Some("papo-folder-download-symbolic"))),"visual attachments expose downloads only in context menus and viewers");crate::ui::main_window::layout::tests::preview(&window,"inline-image",context);open.emit_clicked();pump(context);assert!(photo.model().transfers.images[0].2.is_visible());let viewer=photo.model().transfers.images[0].2.clone();
    let mut tiny=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(32,32,image::Rgba([48,120,180,255]))).write_to(&mut tiny,image::ImageFormat::Png).unwrap();
    photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{token:None,epoch,message:message_id,key:picture_id,result:Ok(tiny.into_inner())}));
    until(context,||photo.model().transfers.textures[&picture_id].width()==32);
    for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let small=descendants(find_button(photo.widget().upcast_ref(),"Abrir imagem").upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Picture>().ok()).unwrap();assert!((620..=640).contains(&small.width())&&(150..=165).contains(&small.height()),"a replacement thumbnail must keep the known original aspect ratio: {}x{}",small.width(),small.height());
    crate::ui::chat::actions::tests::resize_window(&window,400,650);for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    assert!(small.width()<400&&small.width()>150,"media must shrink within a narrow chat: {}",small.width());assert!((small.width()-4*small.height()).abs()<=4);crate::ui::main_window::layout::tests::preview(&window,"inline-image-narrow",context);assert!((small.width()-4*small.height()).abs()<=4,"media aspect ratio remains stable after resize settles: {}x{}",small.width(),small.height());
    let limiter=photo.model().transfers.decode_limiter.clone();let permit=tokio::runtime::Handle::current().block_on(limiter.acquire_many_owned(2)).unwrap();
    let mut encoded=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(32,32).write_to(&mut encoded,image::ImageFormat::Png).unwrap();photo.emit(ChatMsg::Transfer(TransferMsg::Bytes{token:None,epoch,message:message_id,key:picture_id,result:Ok(encoded.into_inner())}));pump(context);assert!(photo.model().transfers.image_tokens.contains_key(&picture_id));
    let ticks=std::rc::Rc::new(std::cell::Cell::new(0));let count=ticks.clone();let timer=gtk::glib::timeout_add_local(std::time::Duration::from_millis(5),move ||{count.set(count.get()+1);gtk::glib::ControlFlow::Continue});for _ in 0..20{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}timer.remove();assert!(ticks.get()>5,"GTK remains responsive while image work waits off-thread");
    photo.emit(ChatMsg::ApplyChange(Change::Moderation(message_id,picture_id,"blocked".into())));pump(context);assert!(!viewer.is_visible());assert!(photo.model().transfers.images.is_empty());drop(permit);until(context,||photo.model().transfers.jobs.iter().all(|job|job.is_finished()));pump(context);assert!(!photo.model().transfers.textures.contains_key(&picture_id),"delayed decoding cannot restore blocked media");window.close();
}

pub(super) fn giphy_widgets(message:&Message,media:&Transfers,sender:&ComponentSender<ChatModel>)->Vec<gtk::Widget>{
    crate::media::gif_links::links(message.content.as_deref().unwrap_or("")).iter().filter_map(|id|{
        let key=*media.giphy.get(id)?;
        if message.embeds.iter().flatten().any(|p|p.id==key){return None;}
        let source=gifs::source(media,message.id,key,std::slice::from_ref(message));
        Some(gifs::widget(None,source.as_deref(),message.id,key,media,sender).upcast())
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
    let keys:Vec<_>=(0..160).map(|_|Uuid::new_v4()).collect();let messages:Vec<Message>=keys.iter().enumerate().map(|(i,key)|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":start+chrono::Duration::seconds(i as i64),"content":format!("Picture {i}"),"attachments":[{"id":key,"mime_type":"image/png","original_file_name":"wide.png","size_bytes":128,"created_at":start}]})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();chat.state().get_mut().model.actions.api=Some(api);chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access{read:true,..Default::default()}});
    let window=adw::Window::builder().default_width(900).default_height(650).content(chat.widget()).build();window.present();
    for index in (0..160).step_by(4){chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[index].id)));until(context,||chat.model().transfers.textures.contains_key(&keys[index]));super::performance::settle(context);}
    assert!(requests.lock().unwrap().len()>64,"scrolling must exercise actual cache eviction");assert!(chat.model().transfers.textures.len()<=64);
    let before=requests.lock().unwrap().get(&keys[0]).copied().unwrap();assert!(!chat.model().transfers.textures.contains_key(&keys[0]));
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[0].id)));until(context,||chat.model().transfers.textures.contains_key(&keys[0]));assert_eq!(requests.lock().unwrap()[&keys[0]],before,"returning to an evicted texture restores compressed cache data without another download");
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

pub(super) fn update_media_box(container:&gtk::Box,message:&Message,media:&Transfers,users:&HashMap<Uuid,UserSummary>,actions:&Actions,sender:&ComponentSender<ChatModel>){
    let mut existing=HashMap::new();let mut child=container.first_child();while let Some(w)=child{child=w.next_sibling();existing.insert(w.widget_name().to_string(),w);}
    let state=|key:Uuid|format!("{}-{}-{}-{}-{}-{}",key,media.textures.get(&key).map_or(0,|t|t.as_ptr() as usize),media.animations.get(&key).map_or(0,|(a,_)|a.as_ptr() as usize),media.videos.iter().find(|p|p.message==message.id&&p.key==key).map_or(0,|p|p.view.as_ptr() as usize),media.failed.contains_key(&key),media.requested.contains(&key));
    let mut ordered=vec![];
    for attachment in message.attachments.iter().flatten(){let name=format!("attachment-{}-{}-{}",state(attachment.id),media.visible(attachment),media.video_requests.contains_key(&(message.id,attachment.id)));let widget=existing.remove(&name).unwrap_or_else(||attachment_widget(attachment,message.id,media,sender).upcast());widget.set_widget_name(&name);ordered.push(widget);}
    for id in crate::media::gif_links::links(message.content.as_deref().unwrap_or("")){
        if let Some(key)=media.giphy.get(&id).filter(|key|!message.embeds.iter().flatten().any(|p|p.id==**key)){let name=format!("giphy-{}",state(*key));let widget=existing.remove(&name).unwrap_or_else(||{let mut single=message.clone();single.content=Some(crate::media::gif_links::content(&id));giphy_widgets(&single,media,sender).remove(0)});widget.set_widget_name(&name);ordered.push(widget);}
    }
    for preview in message.embeds.iter().flatten(){let name=format!("preview-{}-{}-{}-{}",state(preview.id),media.authors.textures.get(&preview.id).map_or(0,|t|t.as_ptr() as usize),media.video_requests.contains_key(&(message.id,preview.id)),super::rich::display_signature(preview,users,actions));let widget=existing.remove(&name).unwrap_or_else(||preview_widget(&gifs::attribution(preview,message),message.id,media,users,actions,sender).upcast());widget.set_widget_name(&name);ordered.push(widget);}
    for widget in existing.into_values(){container.remove(&widget);}
    let mut previous=None;for widget in ordered{if widget.parent().is_none(){container.insert_child_after(&widget,previous.as_ref());}else if widget.prev_sibling()!=previous{container.reorder_child_after(&widget,previous.as_ref());}previous=Some(widget);}
    container.set_visible(container.first_child().is_some());
}

#[cfg(test)]
pub(crate) fn exercise_retention(context:&gtk::glib::MainContext){
    use adw::prelude::*;use actions::tests::{descendants,pump,until};use performance::settle;
    let user=Uuid::new_v4();let a=Uuid::new_v4();let b=Uuid::new_v4();let keys:[Uuid;4]=std::array::from_fn(|_|Uuid::new_v4());let created=chrono::Utc::now();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"memory","created_at":created})).unwrap();
    let messages:Vec<Message>=(0..600).map(|i|{
        let attachment=[5,7,205,599].iter().position(|n|*n==i).map(|n|serde_json::json!({"id":keys[n],"mime_type":if n==0{"image/gif"}else{"image/png"},"original_file_name":"photo.png","size_bytes":128,"created_at":created}));
        serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":a,"author_id":user,"content":format!("Message {i}"),"created_at":created+chrono::Duration::seconds(i),"attachments":attachment.map(|v|vec![v])})).unwrap()
    }).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a)),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();settle(context);
    let scroll=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok()).unwrap();
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[5].id)));settle(context);assert!(chat.model().viewing_old);
    let image=||crate::media::PreparedImage{width:64,height:64,pixels:vec![255;64*64*4]};
    let (old_texture,old_animation,token,epoch)={
        let mut state=chat.state().get_mut();let transfers=&mut state.model.transfers;
        transfers.cache_image(keys[1],image());
        transfers.cache_animation(keys[0],crate::media::animation::PreparedAnimation{frames:vec![(image(),std::time::Duration::from_millis(60)),(image(),std::time::Duration::from_millis(60))]});
        let old_texture=transfers.textures[&keys[1]].downgrade();let old_animation=transfers.animations[&keys[0]].0.downgrade();
        let token=Uuid::new_v4();transfers.image_tokens.insert(keys[2],token);transfers.requested.insert(keys[2]);
        let render=Uuid::new_v4();let epoch=transfers.epoch;state.model.media_render_pending=Some(render);chat.emit(ChatMsg::RenderMedia(render));
        (old_texture,old_animation,token,epoch)
    };
    settle(context);assert!(old_texture.upgrade().is_some()&&old_animation.upgrade().is_some());
    let (media_job,background_job)={let mut state=chat.state().get_mut();let media=tokio::spawn(std::future::pending::<()>());let handle=media.abort_handle();state.model.transfers.media_job(keys[1],media);let background=tokio::spawn(std::future::pending::<()>());let background_handle=background.abort_handle();state.model.transfers.jobs.push(background);(handle,background_handle)};
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[100].id)));settle(context);
    assert!(chat.model().rendered.contains_key(&format!("message-{}",messages[7].id)),"this check must retain the row while releasing its picture");
    assert!(old_texture.upgrade().is_some()&&old_animation.upgrade().is_none(),"bounded residency keeps a static preview while distant animations unload");assert!(!chat.model().transfers.textures.contains_key(&keys[1])&&!chat.model().transfers.animations.contains_key(&keys[0]),"distant rows must release active media ownership");
    until(context,||media_job.is_finished());assert!(!background_job.is_finished(),"evicting media must not cancel history or other background work");
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[5].id)));settle(context);
    let(old_texture,old_animation)={let mut state=chat.state().get_mut();state.model.transfers.cache_image(keys[1],image());state.model.transfers.cache_animation(keys[0],crate::media::animation::PreparedAnimation{frames:vec![(image(),std::time::Duration::from_millis(60)),(image(),std::time::Duration::from_millis(60))]});let texture=state.model.transfers.textures[&keys[1]].downgrade();let animation=state.model.transfers.animations[&keys[0]].0.downgrade();let render=Uuid::new_v4();state.model.media_render_pending=Some(render);chat.emit(ChatMsg::RenderMedia(render));(texture,animation)};settle(context);
    // Returning by scrolling, rather than only the Latest button, releases real widget owners.
    let adjustment=scroll.vadjustment();adjustment.set_value((adjustment.upper()-adjustment.page_size()).max(0.0));
    until(context,||chat.model().history.messages.len()==cache::MESSAGE_LIMIT);settle(context);
    assert_eq!(chat.model().history.messages[0].id,messages[400].id);assert!(chat.model().has_more);
    assert!(old_texture.upgrade().is_some()&&old_animation.upgrade().is_none(),"only the bounded static LRU survives releasing older rows and GIF timers");assert!(!chat.model().transfers.textures.contains_key(&keys[1]));
    assert!((adjustment.upper()-adjustment.page_size()-adjustment.value()).abs()<2.0);
    chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,key:keys[2],token,result:Some(image())}));pump(context);assert!(!chat.model().transfers.textures.contains_key(&keys[2]),"late decoding must not refill released media");
    // Released pages remain available through normal pagination.
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[400].id)));settle(context);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:Some(MessageCursor::from(&messages[400]))});
    chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:a,messages:messages[200..400].to_vec(),has_more:true})});settle(context);assert_eq!(chat.model().history.messages.len(),400);
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[205].id)));settle(context);
    let (texture,row)={let mut state=chat.state().get_mut();state.model.transfers.cache_image(keys[2],image());let texture=state.model.transfers.textures[&keys[2]].downgrade();let row=state.model.rendered[&format!("message-{}",messages[205].id)].1.downgrade();let render=Uuid::new_v4();state.model.media_render_pending=Some(render);chat.emit(ChatMsg::RenderMedia(render));(texture,row)};settle(context);
    chat.emit(ChatMsg::SetChannel(channel(b)));settle(context);assert!(texture.upgrade().is_some()&&row.upgrade().is_none(),"older rows unload on channel switch; their static preview belongs only to the bounded LRU");
    assert!(chat.model().transfers.textures.is_empty()&&chat.model().transfers.animations.is_empty()&&chat.model().transfers.video_files.is_empty());
    chat.emit(ChatMsg::SetChannel(channel(a)));settle(context);assert_eq!(chat.model().history.messages.len(),200);assert_eq!(chat.model().history.messages[0].id,messages[400].id);
    // A normalized embed used by a nearby message must survive another owner's eviction.
    let key=Uuid::new_v4();let mut shared=messages[599].clone();let mut distant=messages[5].clone();let embed:Embed=serde_json::from_value(serde_json::json!({"id":key,"source_type":"custom","fetch_method":"manual","created_at":created})).unwrap();for m in [&mut shared,&mut distant]{m.attachments=None;m.embeds=Some(vec![embed.clone()]);}
    {let mut state=chat.state().get_mut();state.model.transfers.cache_image(key,image());state.model.transfers.scope=HashSet::from([shared.id]);assert!(!state.model.transfers.release_distant(&[distant,shared]));assert!(state.model.transfers.textures.contains_key(&key));}
    {let mut state=chat.state().get_mut();state.model.transfers.cache_image(keys[3],image());}
    chat.emit(ChatMsg::Transfer(TransferMsg::OpenImage{message:messages[599].id,key:keys[3]}));pump(context);
    let viewer=chat.model().transfers.images.last().unwrap().2.downgrade();chat.model().transfers.images.last().unwrap().2.close();
    until(context,||chat.model().transfers.images.is_empty());assert!(viewer.upgrade().is_none(),"closed viewers must release their widget trees");
    // A user-started save keeps its authorization metadata until the save finishes.
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:Some(MessageCursor::from(&messages[400]))});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:a,messages:messages[200..400].to_vec(),has_more:true})});pump(context);
    chat.state().get_mut().model.transfers.saving=1;chat.emit(ChatMsg::Latest);settle(context);assert_eq!(chat.model().history.messages.len(),400);
    let epoch=chat.model().transfers.epoch;chat.emit(ChatMsg::Transfer(TransferMsg::Saved{epoch,message:messages[205].id,result:Ok(())}));pump(context);assert_eq!(chat.model().history.messages.len(),200);
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
    until(context,||background_job.is_finished());
    exercise_performance(context);exercise_cache_scroll(context);
}

#[cfg(test)]pub(super) fn exercise_gif_providers(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use actions::tests::{descendants,pump,until};use performance::settle;
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let message=Uuid::new_v4();let created=chrono::Utc::now();
    let urls=["https://tenor.com/view/two-cats-gif-16152841183863087986","https://klipy.com/gifs/hop-on-google-docs-1"];
    let keys=[Uuid::new_v4(),Uuid::new_v4()];let mut messages=vec![];
    for (url,key) in urls.iter().zip(keys){messages.push(serde_json::from_value::<Message>(serde_json::json!({"id":if messages.is_empty(){message}else{Uuid::new_v4()},"channel_id":channel,"author_id":user,"content":url,"created_at":created,"embeds":[{"id":key,"source_type":"link","fetch_method":"opengraph","url":url,"thumbnail":{"url":"https://media.tenor.com/test.gif","mime_type":"image/gif"},"video":{"url":"https://media.tenor.com/test.mp4","mime_type":"video/mp4"},"created_at":created}]})).unwrap());}
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"GIF providers","created_at":created})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(800).default_height(700).content(chat.widget()).build();window.present();settle(context);
    let epoch=chat.model().transfers.epoch;
    for (i,key) in keys.iter().copied().enumerate(){
        let token=Uuid::new_v4();{let mut state=chat.state().get_mut();
            state.model.transfers.cache_image(key,crate::media::PreparedImage{width:8,height:8,pixels:vec![255;256]});
            assert!(state.model.transfers.request_gif(key),"a cached static thumbnail must still upgrade to the real GIF");assert!(!state.model.transfers.request_gif(key),"an in-flight GIF upgrade must be deduplicated");
            state.model.transfers.giphy.insert(urls[i].into(),key);state.model.transfers.image_tokens.insert(key,token);}
        chat.emit(ChatMsg::Transfer(TransferMsg::Bytes{epoch,message:messages[i].id,key,token:Some(token),result:Ok(crate::media::animation::fixture())}));
    }
    until(context,||chat.model().transfers.animations.len()==2&&chat.model().media_render_pending.is_none());settle(context);
    let pictures:Vec<_>=descendants(chat.widget().upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::Picture>().ok()).filter(|p|p.paintable().is_some_and(|a|a.is::<crate::media::animation::Animation>())).collect();
    assert_eq!(pictures.len(),2,"provider embeds animate once each, without duplicate raw-link previews");
    assert!(!descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Reproduzir vídeo")),"GIFs do not require a play button");
    for key in keys{assert!(!chat.state().get_mut().model.transfers.request_gif(key),"a resolved animation must not be downloaded repeatedly");}
    let first=pictures[0].paintable().unwrap().current_image();until(context,||pictures[0].paintable().unwrap().current_image()!=first);
    // The backend's video fallback must loop without sound, preserving the
    // normal video's audio policy and fullscreen controls.
    let path=std::env::temp_dir().join(format!("papo-gif-test-{}.webm",Uuid::new_v4()));std::fs::write(&path,include_bytes!("../../../tests/fixtures/chat-video.webm")).unwrap();
    chat.state().get_mut().model.transfers.invalidate(keys[1]);
    chat.emit(ChatMsg::Transfer(TransferMsg::File{epoch,message:messages[1].id,key:keys[1],token:None,destination:None,result:Ok(crate::api::features::TemporaryFile(path.clone()))}));until(context,||chat.model().transfers.animations.contains_key(&keys[1])&&chat.model().media_render_pending.is_none());assert!(chat.model().transfers.videos.is_empty(),"automatic GIF fallback uses frames rather than a GTK player");
    chat.state().get_mut().model.transfers.invalidate(keys[1]);
    let cancelled=std::env::temp_dir().join(format!("papo-gif-cancel-{}.webm",Uuid::new_v4()));std::fs::write(&cancelled,include_bytes!("../../../tests/fixtures/chat-video.webm")).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::File{epoch,message:messages[1].id,key:keys[1],token:None,destination:None,result:Ok(crate::api::features::TemporaryFile(cancelled.clone()))}));
    chat.emit(ChatMsg::ClearChannel);pump(context);until(context,||!cancelled.exists());assert!(!path.exists());assert!(chat.model().transfers.animations.is_empty());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}
