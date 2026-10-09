//! Session-local decoded avatars and request generations for asynchronous updates.

use std::collections::{HashMap, HashSet};
use gtk::gdk::Texture;
use uuid::Uuid;

use super::PreparedImage;
use gtk::prelude::*;
use base64::Engine;
pub const AVATAR_EDGE:u32=72;
pub const AVATAR_LIMIT:usize=1024;
static DECODERS:std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>>=std::sync::LazyLock::new(||std::sync::Arc::new(tokio::sync::Semaphore::new(2)));
#[cfg(test)]pub(crate) fn hold_decoders()->tokio::sync::OwnedSemaphorePermit{DECODERS.clone().try_acquire_many_owned(2).unwrap()}
pub fn prepare(blob:&str)->Option<PreparedImage>{decode(blob,AVATAR_EDGE)}
fn decode(blob:&str,edge:u32)->Option<PreparedImage>{
    if blob.len()>3<<20{return None;}
    let bytes=base64::engine::general_purpose::STANDARD.decode(blob.split_once(',').map_or(blob,|(_,data)|data)).ok()?;
    PreparedImage::decode_thumbnail(&bytes,edge,edge)
}
pub async fn prepare_blob(blob:Option<String>,edge:u32)->Option<PreparedImage>{
    let permit=DECODERS.clone().acquire_owned().await.ok()?;
    tokio::task::spawn_blocking(move ||{let _permit=permit;blob.as_deref().and_then(|blob|decode(blob,edge))}).await.ok().flatten()
}
pub async fn prepare_profiles(profiles:Vec<crate::models::UserProfile>)->Vec<(Uuid,Option<PreparedImage>)>{
    let Ok(permit)=DECODERS.clone().acquire_owned().await else{return vec![];};
    tokio::task::spawn_blocking(move ||{let _permit=permit;profiles.into_iter().map(|p|(p.id,p.avatar_blob.as_deref().and_then(prepare))).collect()}).await.unwrap_or_default()
}

#[derive(Default)]
pub struct AvatarCache {
    pub textures: HashMap<Uuid, Texture>,
    loaded: HashSet<Uuid>,
    pending: HashMap<Uuid, Uuid>,
    order:std::collections::VecDeque<Uuid>,
}

impl AvatarCache {
    fn insert(&mut self,id:Uuid,texture:Texture){
        self.order.retain(|key|*key!=id);self.order.push_back(id);self.textures.insert(id,texture);
        while self.textures.len()>AVATAR_LIMIT{if let Some(id)=self.order.pop_front(){self.textures.remove(&id);self.loaded.remove(&id);}}
    }
    #[cfg(test)]pub fn bytes(&self)->usize{self.textures.values().map(|t|t.width() as usize*t.height() as usize*4).sum()}
    pub fn begin(&mut self, ids: &[Uuid], force: bool) -> Option<(Uuid, Vec<Uuid>)> {
        let request_id = Uuid::new_v4();
        let mut seen = HashSet::new();
        let ids: Vec<_> = ids.iter().copied().filter(|id| {
            seen.insert(*id) && (force || (!self.loaded.contains(id) && !self.pending.contains_key(id)))
        }).collect();
        if ids.is_empty() { return None; }
        for id in &ids { self.pending.insert(*id, request_id); }
        Some((request_id, ids))
    }

    /// A newer avatar_update supersedes any older initial/reconnect response.
    /// Omitted profiles and null/invalid blobs clear the picture to its fallback.
    pub fn finish(&mut self, request_id: Uuid, ids: &[Uuid], profiles: Vec<(Uuid,Option<PreparedImage>)>) -> bool {
        let mut profiles: HashMap<_, _> = profiles.into_iter().collect();
        let mut changed = false;
        for id in ids {
            if self.pending.get(id) != Some(&request_id) { continue; }
            self.pending.remove(id);
            self.loaded.insert(*id);
            if let Some(image)=profiles.remove(id).flatten(){self.insert(*id,image.texture());}else{self.textures.remove(id);self.order.retain(|key|key!=id);}
            changed = true;
        }
        changed
    }

    pub fn fail(&mut self, request_id: Uuid, ids: &[Uuid]) {
        for id in ids {
            if self.pending.get(id) == Some(&request_id) { self.pending.remove(id); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_pending_and_loaded_members_do_not_refetch_profiles() {
        let id = Uuid::new_v4();
        let mut cache = AvatarCache::default();
        let (request, ids) = cache.begin(&[id, id], false).unwrap();
        assert_eq!(ids, vec![id]);
        assert!(cache.begin(&[id], false).is_none());
        assert!(cache.finish(request, &ids, Vec::new()));
        assert!(cache.begin(&[id], false).is_none());
        assert!(cache.textures.is_empty());
        assert!(cache.begin(&[id], true).is_some());
    }

    #[test]
    fn avatar_update_supersedes_old_profile_results_and_errors() {
        let id = Uuid::new_v4();
        let mut cache = AvatarCache::default();
        let (old, ids) = cache.begin(&[id], false).unwrap();
        let (new, _) = cache.begin(&[id], true).unwrap();
        cache.fail(old, &ids);
        assert!(!cache.finish(old, &ids, Vec::new()));
        assert_eq!(cache.pending.get(&id), Some(&new));
        assert!(cache.finish(new, &ids, Vec::new()));
        assert!(!cache.finish(old, &ids, Vec::new()));
    }

    #[test]
    fn failed_fetch_can_retry_and_caches_are_isolated_per_session() {
        let id = Uuid::new_v4();
        let mut cache = AvatarCache::default();
        let (request, ids) = cache.begin(&[id], false).unwrap();
        cache.fail(request, &ids);
        let (retry, ids) = cache.begin(&[id], false).unwrap();
        assert!(cache.finish(retry, &ids, Vec::new()));
        assert!(AvatarCache::default().begin(&[id], false).is_some());
    }
}

#[cfg(test)]pub(crate) fn exercise_corrections(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{pump,until};
    let mut cache=AvatarCache::default();
    for _ in 0..AVATAR_LIMIT+20{let id=Uuid::new_v4();let (request,ids)=cache.begin(&[id],false).unwrap();cache.finish(request,&ids,vec![(id,Some(PreparedImage{width:AVATAR_EDGE,height:AVATAR_EDGE,pixels:vec![255;(AVATAR_EDGE*AVATAR_EDGE*4)as usize]}))]);}
    assert_eq!(cache.textures.len(),AVATAR_LIMIT);assert!(cache.bytes()<=AVATAR_LIMIT*(AVATAR_EDGE*AVATAR_EDGE*4)as usize);
    let id=Uuid::new_v4();let (old,ids)=cache.begin(&[id],true).unwrap();let (new,_)=cache.begin(&[id],true).unwrap();cache.finish(new,&ids,vec![(id,Some(PreparedImage{width:2,height:2,pixels:vec![0;16]}))]);assert!(!cache.finish(old,&ids,vec![(id,Some(PreparedImage{width:3,height:3,pixels:vec![0;36]}))]));assert_eq!(cache.textures[&id].width(),2);
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(512,512).write_to(&mut bytes,image::ImageFormat::Png).unwrap();let blob=base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
    let profiles=(0..128).map(|_|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"username":"member","created_at":"2026-10-08T12:00:00Z","avatar_blob":blob})).unwrap()).collect();
    let (tx,rx)=std::sync::mpsc::channel();tokio::spawn(async move{let prepared=prepare_profiles(profiles).await;tx.send(prepared).unwrap();});
    let ticks=std::rc::Rc::new(std::cell::Cell::new(0));let count=ticks.clone();let timer=gtk::glib::timeout_add_local(std::time::Duration::from_millis(1),move ||{count.set(count.get()+1);gtk::glib::ControlFlow::Continue});
    let mut prepared=None;let start=std::time::Instant::now();while prepared.is_none(){pump(context);prepared=rx.try_recv().ok();assert!(start.elapsed()<std::time::Duration::from_secs(10));std::thread::sleep(std::time::Duration::from_millis(1));}timer.remove();assert!(ticks.get()>0,"GTK must keep dispatching while avatars decode");
    assert_eq!(prepared.as_ref().unwrap().len(),128);assert!(prepared.unwrap().iter().all(|(_,image)|image.as_ref().is_some_and(|i|i.width<=AVATAR_EDGE&&i.height<=AVATAR_EDGE)));
    let idle=std::rc::Rc::new(std::cell::Cell::new(false));let flag=idle.clone();gtk::glib::idle_add_local_once(move ||flag.set(true));until(context,||idle.get());
}
