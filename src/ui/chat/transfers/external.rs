//! Small, bounded webhook avatars, shared by URL across messages/channels.
use super::*;
#[derive(Default)]
pub(in crate::ui::chat) struct Avatars {
    entries:HashMap<String,(std::time::Instant,Option<gtk::gdk::Texture>)>,
    order:VecDeque<String>,
    pending:HashMap<String,(Uuid,tokio::task::JoinHandle<()>)>,
}
impl Drop for Avatars { fn drop(&mut self){self.pause();} }
impl Avatars {
    pub(in crate::ui::chat) fn invalidate(&mut self,id:Uuid){
        let suffix=format!("/webhooks/{id}/avatar");let keys:Vec<_>=self.entries.keys().chain(self.pending.keys()).filter(|key|key.ends_with(&suffix)).cloned().collect();
        for key in keys{self.entries.remove(&key);self.order.retain(|k|k!=&key);if let Some((_,job))=self.pending.remove(&key){job.abort();}}
    }
    pub(in crate::ui::chat) fn pause(&mut self){for (_,( _,job)) in self.pending.drain(){job.abort();}}
    pub(in crate::ui::chat) fn texture(&self,message:&Message)->Option<&gtk::gdk::Texture>{
        self.entries.get(&message.external()?.avatar_url)?.1.as_ref()
    }
    pub(in crate::ui::chat) fn request(&mut self,message:&Message,api:&crate::api::ApiClient,sender:&ComponentSender<ChatModel>){
        let Some(author)=message.external() else{return;}; let url=&author.avatar_url;
        if api.external_avatar_url(url).is_none(){return;}
        if let Some((at,image))=self.entries.get(url){
            if at.elapsed().as_secs()<if image.is_some(){3600}else{60}{
                self.order.retain(|key|key!=url);self.order.push_back(url.clone());return;
            }
        }
        if self.pending.contains_key(url)||self.pending.len()>=4{return;}
        let token=Uuid::new_v4();let api=api.clone();let output=sender.input_sender().clone();let value=url.clone();
        let job=tokio::spawn(async move{
            let image=match api.external_avatar(&value).await{Ok(bytes)=>crate::media::avatars::prepare_bytes(bytes).await,Err(_)=>None};
            let _=output.send(ChatMsg::Transfer(TransferMsg::ExternalAvatar{url:value,token,image}));
        });
        self.pending.insert(url.clone(),(token,job));
    }
    pub(in crate::ui::chat) fn finish(&mut self,url:String,token:Uuid,image:Option<crate::media::PreparedImage>)->bool{
        if !self.pending.get(&url).is_some_and(|(id,_)|*id==token){return false;}
        self.pending.remove(&url);self.order.retain(|key|key!=&url);self.order.push_back(url.clone());
        self.entries.insert(url,(std::time::Instant::now(),image.map(|i|i.texture())));
        while self.order.len()>256{if let Some(key)=self.order.pop_front(){self.entries.remove(&key);}}
        true
    }
}
