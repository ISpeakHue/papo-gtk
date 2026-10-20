//! Session-local compressed media; decoded textures and GIF frames remain disposable.
use std::{collections::{HashMap,HashSet,VecDeque},sync::Arc};
use uuid::Uuid;
use crate::ui::chat::state::Change;

const BYTE_LIMIT:usize=192<<20;
const ENTRY_LIMIT:usize=512;
const OLD_BYTES:usize=64<<20;
const OLD_ENTRIES:usize=192;
struct Entry {channel:Uuid,message:Uuid,bytes:Arc<[u8]>,video:bool,touched:std::time::Instant}
#[derive(Default)]
pub(in crate::ui::chat) struct Cache {entries:HashMap<Uuid,Entry>,order:VecDeque<Uuid>,bytes:usize,recent:HashMap<Uuid,HashSet<Uuid>>,channels:VecDeque<Uuid>}
impl Cache {
    pub fn set_recent(&mut self,channel:Uuid,keys:HashSet<Uuid>){
        self.channels.retain(|id|*id!=channel);self.channels.push_back(channel);self.recent.insert(channel,keys);
        while self.channels.len()>8{if let Some(old)=self.channels.pop_front(){self.invalidate_channel(old);}}
        self.maintain();
    }
    pub fn contains(&self,key:Uuid)->bool{self.entries.contains_key(&key)}
    pub fn get(&mut self,key:Uuid)->Option<Arc<[u8]>> {
        self.get_kind(key,false)
    }
    pub fn get_video(&mut self,key:Uuid)->Option<Arc<[u8]>>{self.get_kind(key,true)}
    fn get_kind(&mut self,key:Uuid,video:bool)->Option<Arc<[u8]>> {
        crate::performance::record(if self.entries.get(&key).is_some_and(|e|e.video==video){"encoded_cache_hits"}else{"encoded_cache_misses"},1);
        if self.entries.get(&key)?.video!=video{return None;}
        let entry=self.entries.get_mut(&key)?;entry.touched=std::time::Instant::now();let bytes=entry.bytes.clone();
        self.order.retain(|id|*id!=key);self.order.push_back(key);Some(bytes)
    }
    pub fn insert(&mut self,key:Uuid,channel:Uuid,message:Uuid,bytes:Arc<[u8]>) {
        self.insert_kind(key,channel,message,bytes,false);
    }
    pub fn insert_video(&mut self,key:Uuid,channel:Uuid,message:Uuid,bytes:Arc<[u8]>) {
        self.insert_kind(key,channel,message,bytes,true);
    }
    fn insert_kind(&mut self,key:Uuid,channel:Uuid,message:Uuid,bytes:Arc<[u8]>,video:bool) {
        self.remove(key);if bytes.is_empty()||bytes.len()>25<<20{return;}
        self.bytes+=bytes.len();self.entries.insert(key,Entry{channel,message,bytes,video,touched:std::time::Instant::now()});self.order.push_back(key);crate::performance::record("encoded_cache_bytes",self.bytes as u64);
        while self.bytes>BYTE_LIMIT||self.entries.len()>ENTRY_LIMIT {
            let Some(old)=self.order.iter().find(|id|!self.recent.values().any(|keys|keys.contains(id))).or_else(||self.order.front()).copied()else{break;};self.remove(old);
        }
        self.maintain();
    }
    pub fn maintain(&mut self){
        let old:Vec<_>=self.order.iter().copied().filter(|id|!self.recent.values().any(|keys|keys.contains(id))).collect();
        let mut bytes:usize=old.iter().filter_map(|id|self.entries.get(id)).map(|e|e.bytes.len()).sum();let mut count=old.len();
        for id in old{
            let Some(entry)=self.entries.get(&id)else{continue;};
            if bytes>OLD_BYTES||count>OLD_ENTRIES||entry.touched.elapsed().as_secs()>=300{
                bytes-=entry.bytes.len();count-=1;self.remove(id);
            }
        }
    }
    pub fn remove(&mut self,key:Uuid) {
        if let Some(entry)=self.entries.remove(&key){self.bytes-=entry.bytes.len();}
        self.order.retain(|id|*id!=key);
    }
    fn retain(&mut self,keep:impl Fn(&Entry)->bool) {
        let remove:Vec<_>=self.entries.iter().filter(|(_,entry)|!keep(entry)).map(|(id,_)|*id).collect();
        for id in remove{self.remove(id);}
    }
    pub fn retain_channels(&mut self,allowed:&[Uuid]){self.recent.retain(|id,_|allowed.contains(id));self.channels.retain(|id|allowed.contains(id));self.retain(|entry|allowed.contains(&entry.channel));}
    pub fn invalidate_channel(&mut self,id:Uuid){self.recent.remove(&id);self.channels.retain(|channel|*channel!=id);self.retain(|entry|entry.channel!=id);}
    pub fn clear(&mut self){*self=Self::default();}
    pub fn apply(&mut self,change:&Change) {
        match change {
            Change::Delete(message)=>self.retain(|entry|entry.message!=*message),
            Change::Moderation(_,key,status) if !matches!(status.as_str(),"clean"|"failed"|"")=>self.remove(*key),
            _=>{},
        }
    }
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn idle_history_expires_but_recent_batches_keep_their_bytes(){
        let mut cache=Cache::default();let channel=Uuid::new_v4();let message=Uuid::new_v4();let head=Uuid::new_v4();let old=Uuid::new_v4();
        cache.set_recent(channel,HashSet::from([head]));
        for key in [head,old]{cache.insert(key,channel,message,Arc::from([1u8]));cache.entries.get_mut(&key).unwrap().touched-=std::time::Duration::from_secs(301);}
        cache.maintain();assert!(cache.contains(head));assert!(!cache.contains(old));
        cache.set_recent(channel,HashSet::new());assert!(!cache.contains(head));
    }
    #[test]fn retains_larger_recent_images_without_a_second_download(){
        let mut cache=Cache::default();let key=Uuid::new_v4();let bytes:Arc<[u8]>=vec![1;8<<20].into();cache.insert(key,Uuid::new_v4(),Uuid::new_v4(),bytes.clone());assert!(Arc::ptr_eq(&bytes,&cache.get(key).unwrap()));
    }
    #[test]fn video_renditions_share_the_budget_but_never_enter_the_image_decoder(){
        let mut cache=Cache::default();let key=Uuid::new_v4();let channel=Uuid::new_v4();let message=Uuid::new_v4();
        cache.insert_video(key,channel,message,Arc::from([1u8]));assert!(cache.contains(key));assert!(cache.get(key).is_none());assert!(cache.get_video(key).is_some());
        cache.insert(key,channel,message,Arc::from([2u8]));assert!(cache.get_video(key).is_none());assert_eq!(&*cache.get(key).unwrap(),&[2]);
        cache.apply(&Change::Delete(message));assert!(cache.get_video(key).is_none()&&!cache.contains(key));
    }
    #[test]fn shares_compressed_bytes_and_bounds_lru_memory(){
        let mut cache=Cache::default();let channel=Uuid::new_v4();let message=Uuid::new_v4();let first=Uuid::new_v4();
        let bytes:Arc<[u8]>=vec![1;4<<20].into();cache.insert(first,channel,message,bytes.clone());
        assert!(Arc::ptr_eq(&bytes,&cache.get(first).unwrap()));
        let mut keys=vec![first];for _ in 0..(OLD_BYTES/(4<<20)-1){let key=Uuid::new_v4();keys.push(key);cache.insert(key,channel,message,bytes.clone());}
        cache.get(first);cache.insert(Uuid::new_v4(),channel,message,bytes);
        assert!(cache.get(first).is_some());assert!(cache.get(keys[1]).is_none());assert!(cache.bytes<=BYTE_LIMIT);
        for _ in 0..600{cache.insert(Uuid::new_v4(),channel,message,Arc::from([1u8]));}assert!(cache.entries.len()<=OLD_ENTRIES);
    }
    #[test]fn recent_opening_batches_take_priority_over_old_scroll_history(){
        let mut cache=Cache::default();let channel=Uuid::new_v4();let message=Uuid::new_v4();let key=Uuid::new_v4();
        let bytes:Arc<[u8]>=vec![1;4<<20].into();cache.set_recent(channel,HashSet::from([key]));cache.insert(key,channel,message,bytes.clone());
        for _ in 0..(BYTE_LIMIT/(4<<20)+10){cache.insert(Uuid::new_v4(),channel,message,bytes.clone());}
        assert!(cache.contains(key),"scrolling older pages cannot displace the protected opening batch first");assert!(cache.bytes<=BYTE_LIMIT);
        cache.invalidate_channel(channel);assert!(cache.recent.is_empty()&&cache.entries.is_empty());
    }
    #[test]fn invalidates_deleted_moderated_and_revoked_content(){
        let mut cache=Cache::default();let a=Uuid::new_v4();let b=Uuid::new_v4();let message=Uuid::new_v4();let key=Uuid::new_v4();
        cache.insert(key,a,message,Arc::from([1u8]));cache.apply(&Change::Moderation(message,key,"clean".into()));assert!(cache.get(key).is_some());
        cache.apply(&Change::Moderation(message,key,"sensitive".into()));assert!(cache.get(key).is_none());
        cache.insert(key,a,message,Arc::from([1u8]));cache.apply(&Change::Delete(message));assert!(cache.get(key).is_none());
        cache.insert(key,a,message,Arc::from([1u8]));cache.retain_channels(&[b]);assert!(cache.get(key).is_none());
        cache.insert(key,b,message,Arc::from([1u8]));cache.invalidate_channel(b);assert_eq!(cache.bytes,0);
        cache.insert(key,b,message,Arc::from([1u8]));cache.clear();assert!(cache.entries.is_empty());
    }
}
