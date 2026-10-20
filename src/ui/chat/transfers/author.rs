//! EN-US: Small session-local author avatars survive scrolling and channel switches. Bytes still come only from authorized embed responses, never remote URLs.
//! PT-BR: Pequenos avatares de autores permanecem na sessão entre rolagens e trocas de canal; os bytes vêm apenas de respostas autorizadas.
use super::*;

const CACHE_LIMIT:usize=256;
type Version=(chrono::DateTime<chrono::Utc>,Option<crate::models::EmbedAuthor>);
#[derive(Default)]
pub(super) struct Authors {
    pub(super) textures:HashMap<Uuid,gtk::gdk::Texture>,
    payloads:HashMap<Uuid,String>,
    loaded:HashSet<Uuid>,
    tokens:HashMap<Uuid,Uuid>,
    versions:HashMap<Uuid,Version>,
    order:VecDeque<Uuid>,
}
impl Authors {
    fn version(embed:&Embed)->Version{(embed.version(),embed.author.clone())}
    // Inspect incoming metadata before a richer cached summary can replace it.
    pub(super) fn invalidate_stale(&mut self,embed:&Embed){
        if embed.fetch_method.is_empty()&&embed.author.is_none(){return;}
        if self.versions.get(&embed.id).is_some_and(|old|old.0!=embed.version()||old.1.as_ref()!=embed.author.as_ref()){self.invalidate(embed.id);}
    }
    fn touch(&mut self,key:Uuid){self.order.retain(|id|*id!=key);self.order.push_back(key);}
    fn trim(&mut self)->Vec<Uuid>{
        let mut removed=Vec::new();
        while self.order.len()>CACHE_LIMIT{if let Some(old)=self.order.pop_front(){self.invalidate(old);removed.push(old);}}
        removed
    }
    pub(super) fn needs(&mut self,embed:&Embed)->bool{
        self.invalidate_stale(embed);
        if self.loaded.contains(&embed.id){self.touch(embed.id);}
        embed.author.as_ref().is_some_and(|a|a.media.is_some())&&!self.loaded.contains(&embed.id)
    }
    pub(super) fn payload(&mut self,key:Uuid,data:String){
        if self.loaded.contains(&key)||data.len()>3<<20{return;}
        self.payloads.insert(key,data);
        while self.payloads.len()>32||self.payloads.values().map(String::len).sum::<usize>()>8<<20{
            let Some(old)=self.payloads.keys().copied().find(|id|*id!=key)else{break;};self.payloads.remove(&old);
        }
    }
    pub(super) fn invalidate(&mut self,key:Uuid){self.textures.remove(&key);self.payloads.remove(&key);self.loaded.remove(&key);self.tokens.remove(&key);self.versions.remove(&key);self.order.retain(|id|*id!=key);}
    pub(super) fn release(&mut self,keep:&HashSet<Uuid>){
        // Completed 72px avatars are cheap and reusable. Only cancel unfinished
        // work outside the viewport; large chat images have their own eviction.
        let pending:Vec<_>=self.tokens.keys().copied().filter(|key|!keep.contains(key)).collect();
        for key in pending{self.invalidate(key);}
        self.payloads.retain(|key,_|keep.contains(key));
    }
    pub(super) fn pause(&mut self){self.release(&HashSet::new());}
}
impl Transfers {
    pub(super) fn queue_author(&mut self,sender:&ComponentSender<ChatModel>,embed:&Embed,data:Option<String>){
        let key=embed.id;
        if !embed.author.as_ref().is_some_and(|a|a.media.is_some()){return;}
        self.authors.invalidate_stale(embed);
        if self.authors.loaded.contains(&key){self.authors.touch(key);return;}
        self.authors.versions.insert(key,Authors::version(embed));self.authors.loaded.insert(key);self.authors.touch(key);
        self.dirty.extend(self.authors.trim());
        let Some(data)=data else{return;};
        let token=Uuid::new_v4();self.authors.tokens.insert(key,token);let epoch=self.epoch;let s=sender.clone();
        self.media_job(key,tokio::spawn(async move{
            let result=crate::media::avatars::prepare_blob(Some(data),72).await;
            s.input(ChatMsg::Transfer(TransferMsg::AuthorReady{epoch,key,token,result}));
        }));
    }
    pub(super) fn queue_author_payload(&mut self,sender:&ComponentSender<ChatModel>,key:Uuid){
        if let Some(data)=self.authors.payloads.remove(&key){if let Some(embed)=self.embeds.get(&key).cloned(){self.queue_author(sender,&embed,Some(data));}}
    }
    pub(super) fn author_ready(&mut self,epoch:Uuid,key:Uuid,token:Uuid,result:Option<crate::media::PreparedImage>,messages:&[Message]){
        if epoch!=self.epoch||self.authors.tokens.get(&key)!=Some(&token){return;}
        self.authors.tokens.remove(&key);
        if !self.authorized(messages,key){self.authors.invalidate(key);return;}
        if let Some(image)=result{
            // Never let the author's avatar become the card's main image.
            self.authors.textures.insert(key,image.texture());self.authors.touch(key);
            self.dirty.extend(self.authors.trim());
            self.dirty.insert(key);self.revision=self.revision.wrapping_add(1);
        }
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,mock,pump,until};
    use adw::prelude::*;
    use base64::Engine;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let key=Uuid::new_v4();let image_key=Uuid::new_v4();let missing_key=Uuid::new_v4();let now=chrono::Utc::now();
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(128,128).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
    let icon=base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
    let embed=serde_json::json!({"id":key,"source_type":"custom","fetch_method":"manual","site_name":"Discord","description":"Bridged message","color":"#5865F2","author":{"name":"Discord sender","media":{"mime_type":"image/png","size_bytes":76610}},"footer":{"text":"Discord · #general"},"created_at":now});
    let mut full=embed.clone();full["author_image_data"]=serde_json::json!(icon);full["image_data"]=serde_json::Value::Null;
    let mut with_image=full.clone();with_image["id"]=serde_json::json!(image_key);with_image["thumbnail"]=serde_json::json!({"mime_type":"image/png"});with_image["image_data"]=serde_json::json!(actions::tests::tiny_png_base64());
    let mut missing=embed.clone();missing["id"]=serde_json::json!(missing_key);let mut missing_full=missing.clone();missing_full["author_image_data"]=serde_json::json!("invalid base64");
    {let mut b=backend.lock().unwrap();for e in [&full,&with_image,&missing_full]{b.embed_values.insert(e["id"].as_str().unwrap().into(),e.clone());}}
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":now,"embeds":[embed.clone(),with_image.clone(),missing]})).unwrap();
    let mut shared=message.clone();shared.id=Uuid::new_v4();shared.created_at+=chrono::Duration::seconds(1);shared.embeds=Some(vec![serde_json::from_value(embed).unwrap()]);
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"bridge","created_at":now})).unwrap();
    // Hold the worker so the reserved avatar geometry is checked before pixels arrive.
    let permit=crate::media::avatars::hold_decoders();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target),messages:vec![message.clone(),shared],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();
    until(context,||chat.model().transfers.authors.tokens.contains_key(&key));
    super::super::performance::settle(context);
    let author_name=format!("embed-author-{key}");
    let avatar=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::Avatar>().ok().filter(|a|a.widget_name()==author_name)).unwrap();
    assert_eq!(avatar.size(),24);assert!(avatar.custom_image().is_none());let height=avatar.height();drop(avatar);drop(permit);
    until(context,||chat.model().transfers.authors.textures.contains_key(&key)&&chat.model().transfers.authors.textures.contains_key(&image_key)&&chat.model().transfers.authors.loaded.contains(&missing_key)&&chat.model().transfers.authors.tokens.is_empty());
    until(context,||descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).filter(|a|a.widget_name()==author_name&&a.custom_image().is_some()).count()==2);
    super::super::performance::settle(context);
    for avatar in descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).filter(|a|a.widget_name()==author_name){assert_eq!(avatar.height(),height);}
    assert_eq!(chat.model().transfers.authors.textures[&key].width(),72);assert!(!chat.model().transfers.textures.contains_key(&key),"author images must not appear as the card's main photo");
    assert_eq!(chat.model().transfers.textures[&image_key].width(),1);assert_eq!(chat.model().transfers.authors.textures[&image_key].width(),72);
    assert!(!chat.model().transfers.authors.textures.contains_key(&missing_key));
    for _ in 0..3{chat.emit(ChatMsg::ClearStaleTyping);pump(context);}
    let requests=backend.lock().unwrap().requests.clone();for id in [key,image_key,missing_key]{assert_eq!(requests.iter().filter(|(m,p,_)|m=="GET"&&p==&format!("/embeds/{id}")).count(),if id==image_key{0}else{1},"shared cards and failed icons must not create repeated requests; supplied bytes need no GET");}
    assert!(chat.model().history.messages.iter().flat_map(|m|m.embeds.iter().flatten()).all(|e|e.author_image_data.is_none()&&e.image_data.is_none()));
    crate::ui::main_window::layout::tests::preview(&window,"bridge-author",context);
    // Reopening a channel must reuse decoded icons, even with decoders held.
    let cached=chat.model().transfers.authors.textures[&key].clone();
    let other:Channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"other","created_at":now})).unwrap();
    let original:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"bridge","created_at":now})).unwrap();
    let permit=crate::media::avatars::hold_decoders();
    for _ in 0..3{
        chat.emit(ChatMsg::SetChannel(other.clone()));pump(context);chat.emit(ChatMsg::SetChannel(original.clone()));pump(context);
        assert_eq!(chat.model().transfers.authors.textures[&key],cached);
        assert!(chat.model().transfers.authors.tokens.is_empty(),"cache hits must not queue new avatar decoding");
        assert!(descendants(chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).any(|a|a.widget_name()==author_name&&a.custom_image()==Some(cached.clone().upcast())));
    }
    super::super::performance::settle(context);
    for id in [key,missing_key]{assert_eq!(backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p==&format!("/embeds/{id}")).count(),1,"return visits must not refetch avatar-only cards, including missing icons");}
    drop(permit);
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
    // Small avatars survive viewport/channel changes; actual LRU eviction and
    // session reset release them, and late jobs cannot resurrect old pixels.
    let mut media=Transfers::default();let token=Uuid::new_v4();let epoch=media.epoch;media.authors.loaded.insert(key);media.authors.tokens.insert(key,token);
    let image=||crate::media::PreparedImage{width:2,height:2,pixels:vec![0;16]};
    media.author_ready(epoch,key,token,Some(image()),std::slice::from_ref(&message));let weak=media.authors.textures[&key].downgrade();
    media.scope.insert(message.id);assert!(!media.release_distant(std::slice::from_ref(&message)));assert!(weak.upgrade().is_some());
    media.scope.clear();assert!(!media.release_distant(std::slice::from_ref(&message)));assert!(weak.upgrade().is_some());assert!(media.authors.loaded.contains(&key));
    media.author_ready(epoch,key,token,Some(image()),std::slice::from_ref(&message));assert!(weak.upgrade().is_some());
    let pending=Uuid::new_v4();media.authors.loaded.insert(pending);media.authors.tokens.insert(pending,token);media.switch_channel();media.author_ready(epoch,pending,token,Some(image()),std::slice::from_ref(&message));assert!(!media.authors.textures.contains_key(&pending));assert!(!media.authors.loaded.contains(&pending));assert!(weak.upgrade().is_some());
    let mut sample=message.clone();sample.embeds.as_mut().unwrap().truncate(1);
    for _ in 0..CACHE_LIMIT{
        let id=Uuid::new_v4();sample.embeds.as_mut().unwrap()[0].id=id;let token=Uuid::new_v4();media.authors.loaded.insert(id);media.authors.tokens.insert(id,token);media.authors.versions.insert(id,Authors::version(&sample.embeds.as_ref().unwrap()[0]));
        media.author_ready(media.epoch,id,token,Some(crate::media::PreparedImage{width:72,height:72,pixels:vec![0;72*72*4]}),std::slice::from_ref(&sample));
    }
    assert!(weak.upgrade().is_none());assert_eq!(media.authors.textures.len(),CACHE_LIMIT);assert!(media.authors.loaded.len()<=CACHE_LIMIT);assert!(media.authors.versions.len()<=CACHE_LIMIT);
    assert!(media.authors.textures.values().map(|t|(t.width()*t.height()*4) as usize).sum::<usize>()<=CACHE_LIMIT*72*72*4);
    let weak=media.authors.textures.values().next().unwrap().downgrade();media.reset();assert!(weak.upgrade().is_none());assert!(media.authors.loaded.is_empty());assert!(media.authors.versions.is_empty());
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn cached_author_identity_survives_summaries_but_not_edits_or_refetches(){
        let embed:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","created_at":"2026-10-13T12:00:00Z","author":{"name":"Bot author","media":{"mime_type":"image/png","size_bytes":100}}})).unwrap();
        let mut cache=Authors::default();cache.loaded.insert(embed.id);cache.versions.insert(embed.id,Authors::version(&embed));cache.touch(embed.id);
        assert!(!cache.needs(&embed));let mut summary=embed.clone();summary.fetch_method.clear();summary.author=None;cache.invalidate_stale(&summary);assert!(!cache.needs(&embed),"sparse history must retain the resolved avatar");
        let mut renamed=embed.clone();renamed.author.as_mut().unwrap().name=Some("Changed author".into());assert!(cache.needs(&renamed));assert!(!cache.loaded.contains(&embed.id));
        cache.loaded.insert(embed.id);cache.versions.insert(embed.id,Authors::version(&embed));let mut newer=embed;newer.fetched_at=Some(newer.created_at+chrono::Duration::seconds(1));assert!(cache.needs(&newer),"a newer embed version must refresh its author icon");
    }
}
