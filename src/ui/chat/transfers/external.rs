//! EN-US: Small webhook avatars, shared by resource across messages and channels.
//! PT-BR: Avatares pequenos de webhooks são compartilhados por recurso entre mensagens e canais.
use super::*;
const ACTIVE:usize=2;
const QUEUED:usize=128;
const CACHED:usize=256;

/// EN-US: Discord avatar hashes identify immutable pixels across CDN aliases and size/signature variants.
/// Normalize only the cache key; requests retain the original signed URL, and other providers keep their full key.
/// PT-BR: Hashes de avatares do Discord identificam os mesmos pixels entre aliases de CDN e variantes de tamanho e assinatura.
/// Normaliza apenas a chave do cache; requisições mantêm a URL assinada original e outros provedores preservam a chave completa.
fn key(value:&str)->String{
    if let Ok(url)=reqwest::Url::parse(value){
        if url.scheme()=="https"&&url.username().is_empty()&&url.password().is_none()&&url.fragment().is_none()
            &&matches!(url.host_str(),Some("cdn.discordapp.com"|"media.discordapp.net"))&&url.port().is_none(){
            let path=url.path();let parts:Vec<_>=path.trim_start_matches('/').split('/').collect();
            if matches!(parts.as_slice(),["avatars",id,image] if !id.is_empty()&&!image.is_empty()){
                return format!("discord-avatar:{path}");
            }
        }
    }
    value.to_owned()
}

#[derive(Clone, Default)]
pub(in crate::ui::chat) struct Cached(Vec<(String, std::time::Instant, Option<gtk::gdk::Texture>)>);
#[derive(Default)]
pub(in crate::ui::chat) struct Avatars {
    entries:HashMap<String,(std::time::Instant,Option<gtk::gdk::Texture>)>,
    order:VecDeque<String>,
    failures:HashMap<String,(crate::api::AvatarFailure,std::time::Instant)>,
    queued:VecDeque<(String,String)>,
    wake:Option<(Uuid,tokio::task::JoinHandle<()>)>,
    pending:HashMap<String,(Uuid,tokio::task::JoinHandle<()>)>,
}
impl Drop for Avatars {fn drop(&mut self){if let Some((_,job))=self.wake.take(){job.abort();}for (_,(_,job)) in self.pending.drain(){job.abort();}}}
impl Avatars {
    pub(in crate::ui::chat) fn cached(&self, messages: &[Message]) -> Cached {
        let keys: HashSet<_> = messages.iter().filter_map(|m| m.external().map(|a| key(&a.avatar_url))).collect();
        Cached(self.entries.iter().filter(|(k,_)| keys.contains(*k)).map(|(k,(at,image))| (k.clone(),*at,image.clone())).collect())
    }
    pub(in crate::ui::chat) fn seed(&mut self, cached: Cached) {
        for (key,at,image) in cached.0 {
            if self.entries.get(&key).is_some_and(|(old,_)| *old>=at) { continue; }
            self.entries.insert(key.clone(),(at,image));self.order.retain(|k|k!=&key);self.order.push_back(key);
        }
        while self.order.len()>CACHED { if let Some(key)=self.order.pop_front(){self.entries.remove(&key);self.failures.remove(&key);} }
    }
    /// Reconcile mutable avatar routes after missed socket events. Discord hash
    /// resources identify immutable pixels and need no forced re-download.
    pub(in crate::ui::chat) fn revalidate(&mut self){for (key,(at,_)) in &mut self.entries{if !key.starts_with("discord-avatar:"){*at=std::time::Instant::now()-std::time::Duration::from_secs(3601);}}}

    pub(in crate::ui::chat) fn invalidate(&mut self,id:Uuid){
        let suffix=format!("/webhooks/{id}/avatar");let keys:Vec<_>=self.entries.keys().chain(self.pending.keys()).filter(|key|key.ends_with(&suffix)).cloned().collect();
        self.queued.retain(|(key,_)|!key.ends_with(&suffix));
        for key in keys{self.entries.remove(&key);self.failures.remove(&key);self.order.retain(|k|k!=&key);if let Some((_,job))=self.pending.remove(&key){job.abort();}}
    }
    // Drop obsolete waiting work, but let the two small public avatar fetches
    // finish into the session cache instead of restarting them on each switch.
    pub(in crate::ui::chat) fn clear_queue(&mut self){self.queued.clear();if let Some((_,job))=self.wake.take(){job.abort();}}
    pub(in crate::ui::chat) fn restore<'a>(&self,list:&gtk::ListBox,messages:impl Iterator<Item=&'a Message>){
        let images:HashMap<_,_>=messages.filter_map(|m|self.texture(m).map(|image|(format!("message-{}",m.id),image))).collect();
        fn author(widget:&gtk::Widget,image:&gtk::gdk::Texture){
            if widget.has_css_class("papo-message-avatar"){if let Some(avatar)=widget.downcast_ref::<gtk::Button>().and_then(|b|b.child()).and_downcast::<adw::Avatar>(){avatar.set_custom_image(Some(image));}return;}
            let mut child=widget.first_child();while let Some(w)=child{child=w.next_sibling();author(&w,image);}
        }
        let mut row=list.first_child();while let Some(widget)=row{row=widget.next_sibling();if let Some(image)=images.get(widget.widget_name().as_str()){author(&widget,image);}}
    }
    pub(in crate::ui::chat) fn texture(&self,message:&Message)->Option<&gtk::gdk::Texture>{
        self.texture_url(&message.external()?.avatar_url)
    }
    pub(in crate::ui::chat) fn texture_url(&self,url:&str)->Option<&gtk::gdk::Texture>{self.entries.get(&key(url))?.1.as_ref()}
    fn fresh(&self,key:&str)->bool{
        if let Some((_,until))=self.failures.get(key){return *until>std::time::Instant::now();}
        self.entries.get(key).is_some_and(|(at,image)|at.elapsed().as_secs()<if image.is_some(){3600}else{60})
    }
    pub(in crate::ui::chat) fn failed(&self,message:&Message)->bool{message.external().is_some_and(|a|self.failures.contains_key(&key(&a.avatar_url)))}
    pub(in crate::ui::chat) fn retry(&mut self,message:&Message)->bool{
        let Some(author)=message.external()else{return false;};let key=key(&author.avatar_url);
        if self.failures.get(&key).is_some_and(|(failure,until)|matches!(failure,crate::api::AvatarFailure::RateLimited(_))&&*until>std::time::Instant::now()){return false;}
        self.failures.remove(&key);if let Some((at,_))=self.entries.get_mut(&key){*at=std::time::Instant::now()-std::time::Duration::from_secs(3601);}true
    }
    pub(in crate::ui::chat) fn matches(message:&Message,value:&str)->bool{
        message.external().is_some_and(|a|key(&a.avatar_url)==value)
    }
    pub(in crate::ui::chat) fn request<'a>(&mut self,messages:impl Iterator<Item=&'a Message>,api:&crate::api::ApiClient,sender:&ComponentSender<ChatModel>){
        self.request_authors(messages.filter_map(Message::external),api,sender);
    }
    pub(in crate::ui::chat) fn request_authors<'a>(&mut self,authors:impl Iterator<Item=&'a crate::models::ExternalAuthor>,api:&crate::api::ApiClient,sender:&ComponentSender<ChatModel>){
        let mut seen=HashSet::new();
        for author in authors{
            let url=&author.avatar_url;
            if api.external_avatar_url(url).is_none(){continue;}
            let key=key(url);if !seen.insert(key.clone()){continue;}
            if self.fresh(&key){crate::performance::record("external_avatar_hits",1);self.order.retain(|k|k!=&key);self.order.push_back(key);continue;}
            if !self.pending.contains_key(&key)&&!self.queued.iter().any(|(k,_)|k==&key)&&self.queued.len()<QUEUED{self.queued.push_back((key,url.clone()));}
        }
        self.start(api,sender);
    }
    pub(in crate::ui::chat) fn start(&mut self,api:&crate::api::ApiClient,sender:&ComponentSender<ChatModel>){
        if let Some((_,job))=self.wake.take(){job.abort();}
        let mut remaining=self.queued.len();let mut earliest=None::<std::time::Duration>;
        while self.pending.len()<ACTIVE&&remaining>0{
            remaining-=1;
            let Some((key,url))=self.queued.pop_front()else{break;};
            let delay=api.external_avatar_delay(&url);
            if delay>std::time::Duration::from_millis(750){earliest=Some(earliest.map_or(delay,|old|old.min(delay)));self.queued.push_back((key,url));continue;}
            if self.fresh(&key)||self.pending.contains_key(&key){continue;}
            let token=Uuid::new_v4();let api=api.clone();let output=sender.input_sender().clone();let value=key.clone();
            let job=tokio::spawn(async move{
                let (image,failure)=match api.external_avatar(&url).await{
                    Ok(bytes)=>{let image=crate::media::avatars::prepare_bytes(bytes).await;let failure=image.is_none().then_some(crate::api::AvatarFailure::Invalid);(image,failure)},Err(failure)=>(None,Some(failure))};
                let _=output.send(ChatMsg::Transfer(TransferMsg::ExternalAvatar{key:value,token,image,failure}));
            });
            self.pending.insert(key,(token,job));
        }
        if !self.queued.is_empty(){if let Some(delay)=earliest{
            let token=Uuid::new_v4();let output=sender.input_sender().clone();let job=tokio::spawn(async move{tokio::time::sleep(delay).await;let _=output.send(ChatMsg::Transfer(TransferMsg::AvatarBudgetReady(token)));});self.wake=Some((token,job));
        }}
    }
    pub(in crate::ui::chat) fn wake_ready(&mut self,token:Uuid)->bool{if self.wake.as_ref().is_some_and(|(id,_)|*id==token){self.wake.take();true}else{false}}
    pub(in crate::ui::chat) fn finish(&mut self,key:String,token:Uuid,image:Option<crate::media::PreparedImage>,failure:Option<crate::api::AvatarFailure>)->bool{
        if !self.pending.get(&key).is_some_and(|(id,_)|*id==token){return false;}
        self.pending.remove(&key);self.order.retain(|k|k!=&key);self.order.push_back(key.clone());
        // A failed refresh must not erase an avatar that was already displayed.
        let texture=image.map(|i|i.texture()).or_else(||self.entries.get(&key).and_then(|(_,t)|t.clone()));
        let now=std::time::Instant::now();
        if let Some(failure)=failure{
            let retry=match failure{crate::api::AvatarFailure::Missing|crate::api::AvatarFailure::Invalid=>std::time::Duration::from_secs(300),crate::api::AvatarFailure::Transient=>std::time::Duration::from_secs(30),crate::api::AvatarFailure::RateLimited(delay)=>delay.max(std::time::Duration::from_secs(1))};
            self.failures.insert(key.clone(),(failure,now+retry));
            // Keep the time of the last successful pixels, independently of retries.
            self.entries.entry(key).or_insert((now,texture));
        }else{self.failures.remove(&key);self.entries.insert(key,(now,texture));}
        while self.order.len()>CACHED{if let Some(key)=self.order.pop_front(){self.entries.remove(&key);self.failures.remove(&key);}}
        true
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn discord_avatar_variants_share_pixels_without_merging_other_resources(){
        let original="https://cdn.discordapp.com/avatars/123/hash.png?size=128&ex=old&hm=sig";
        for url in ["https://cdn.discordapp.com/avatars/123/hash.png?size=64&ex=new&hm=new", "https://media.discordapp.net/avatars/123/hash.png?width=72"]{assert_eq!(key(original),key(url));}
        for url in ["https://cdn.discordapp.com/avatars/456/hash.png","https://cdn.discordapp.com/avatars/123/other.png","https://example.org/avatars/123/hash.png","https://user:pass@cdn.discordapp.com/avatars/123/hash.png"]{assert_ne!(key(original),key(url));}
        for url in ["https://example.org/avatar?id=one","https://cdn.discordapp.com/attachments/a/b?signature=one"]{assert_eq!(key(url),url,"unrecognized resources keep their full cache key");}
        assert_ne!(key("https://example.org/avatar?id=one"),key("https://example.org/avatar?id=two"));
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,pump,until,descendants};
    use adw::prelude::*;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let own=Uuid::new_v4();let channel=Uuid::new_v4();let hook=Uuid::new_v4();let now=chrono::DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let paths:Vec<_>=(0..9).map(|_|format!("/webhooks/{}/avatar",Uuid::new_v4())).collect();
    let mut bytes=std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(72,72,image::Rgba([40,160,210,255]))).write_to(&mut bytes,image::ImageFormat::Png).unwrap();let png=bytes.into_inner();
    {let mut b=backend.lock().unwrap();for path in &paths{b.attachment_thumbnails.insert(path.clone(),png.clone());}}
    let message=|i:usize|->Message{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"webhook_id":hook,"external_author":{"source":"discord","id":format!("remote-{}",i/2),"name":format!("Sender {}",i/2),"avatar_url":paths[i/2]},"content":format!("Message {i}"),"created_at":now+chrono::Duration::seconds(i as i64*10)})).unwrap()};
    let messages:Vec<_>=(0..16).map(message).collect();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Webhook chat","created_at":now})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target.clone()),messages:messages.clone(),user_id:Some(own),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(820).content(chat.widget()).build();window.present();
    until(context,||chat.model().transfers.external.entries.values().filter(|(_,t)|t.is_some()).count()==8&&chat.model().transfers.external.pending.is_empty());
    super::super::performance::settle(context);
    let row=|id|descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("message-{id}")).unwrap();
    for (i,message) in messages.iter().enumerate(){
        let widgets=descendants(&row(message.id));let header=widgets.iter().find(|w|w.has_css_class("papo-message-header")).unwrap();assert_eq!(header.is_visible(),i%2==0,"each sender should have one header for consecutive messages");
        if i%2==0{
            let avatar=widgets.iter().find_map(|w|w.downcast_ref::<adw::Avatar>()).unwrap();assert!(avatar.custom_image().is_some());assert!(avatar.is_sensitive(),"external pictures must not inherit a disabled control's opacity");
            let name=widgets.iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).find(|l|l.text()==format!("Sender {}",i/2)).unwrap();assert!(name.is_sensitive());
            for w in widgets.iter().filter(|w|w.has_css_class("papo-author-button")||w.has_css_class("papo-message-avatar")){assert!(!w.can_target()&&!w.is_focusable());}
        }
    }
    let count=|path:&str|backend.lock().unwrap().requests.iter().filter(|(method,p,_)|method=="GET"&&p==path).count();
    for path in &paths[..8]{assert_eq!(count(path),1,"bursts must load every distinct avatar exactly once");}
    crate::ui::main_window::layout::tests::preview(&window,"webhook-grouped",context);
    let style=adw::StyleManager::default();let scheme=style.color_scheme();style.set_color_scheme(adw::ColorScheme::ForceDark);super::super::performance::settle(context);
    crate::ui::main_window::layout::tests::preview(&window,"webhook-grouped-dark",context);style.set_color_scheme(scheme);
    let cached=chat.model().transfers.external.texture(&messages[0]).unwrap().clone();
    let scroll=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok().filter(|s|s.vadjustment().upper()>s.vadjustment().page_size())).unwrap();
    scroll.vadjustment().set_value(0.0);super::super::performance::settle(context);chat.emit(ChatMsg::Latest);super::super::performance::settle(context);
    for path in &paths[..8]{assert_eq!(count(path),1,"scrolling must reuse avatar pixels");}
    // Hold decoding after HTTP starts: switching must retain the single fetch,
    // and the result must populate every occurrence when we return.
    let permit=crate::media::avatars::hold_decoders();let extra=message(16);chat.emit(ChatMsg::AddMessage(extra.clone()));
    until(context,||chat.model().transfers.external.pending.contains_key(&paths[8])&&count(&paths[8])==1);
    let other:Channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"other","created_at":now})).unwrap();
    chat.emit(ChatMsg::SetChannel(other));pump(context);assert!(chat.model().transfers.external.pending.contains_key(&paths[8]));
    chat.emit(ChatMsg::SetChannel(target));pump(context);assert_eq!(count(&paths[8]),1);drop(permit);
    until(context,||chat.model().transfers.external.texture(&extra).is_some());super::super::performance::settle(context);
    assert_eq!(chat.model().transfers.external.texture(&messages[0]),Some(&cached));
    for path in &paths{assert_eq!(count(path),1,"channel switches must not cancel and replay small avatar requests");}
    // Fail a refresh with a valid completion token; keep the existing picture.
    {let mut state=chat.state().get_mut();let cache=&mut state.model.transfers.external;let previous=cache.entries[&paths[0]].0;let token=Uuid::new_v4();cache.pending.insert(paths[0].clone(),(token,tokio::spawn(async{})));assert!(!cache.finish(paths[0].clone(),Uuid::new_v4(),None,Some(crate::api::AvatarFailure::Transient)));assert!(cache.finish(paths[0].clone(),token,None,Some(crate::api::AvatarFailure::Transient)));assert_eq!(cache.texture(&messages[0]),Some(&cached));assert_eq!(cache.entries[&paths[0]].0,previous,"a failed refresh cannot renew pixel freshness");assert!(cache.failed(&messages[0]));}
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::default()});pump(context);assert!(chat.model().transfers.external.entries.is_empty());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
