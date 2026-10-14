//! Session-local compressed media; decoded textures and GIF frames remain disposable.
use std::{collections::{HashMap,VecDeque},sync::Arc};
use uuid::Uuid;
use crate::ui::chat::state::Change;

const BYTE_LIMIT:usize=32<<20;
const ENTRY_LIMIT:usize=256;
struct Entry {channel:Uuid,message:Uuid,bytes:Arc<[u8]>,video:bool}
#[derive(Default)]
pub(in crate::ui::chat) struct Cache {entries:HashMap<Uuid,Entry>,order:VecDeque<Uuid>,bytes:usize}
impl Cache {
    pub fn contains(&self,key:Uuid)->bool{self.entries.contains_key(&key)}
    pub fn get(&mut self,key:Uuid)->Option<Arc<[u8]>> {
        self.get_kind(key,false)
    }
    pub fn get_video(&mut self,key:Uuid)->Option<Arc<[u8]>>{self.get_kind(key,true)}
    fn get_kind(&mut self,key:Uuid,video:bool)->Option<Arc<[u8]>> {
        if self.entries.get(&key)?.video!=video{return None;}
        let bytes=self.entries.get(&key)?.bytes.clone();
        self.order.retain(|id|*id!=key);self.order.push_back(key);Some(bytes)
    }
    pub fn insert(&mut self,key:Uuid,channel:Uuid,message:Uuid,bytes:Arc<[u8]>) {
        self.insert_kind(key,channel,message,bytes,false);
    }
    pub fn insert_video(&mut self,key:Uuid,channel:Uuid,message:Uuid,bytes:Arc<[u8]>) {
        self.insert_kind(key,channel,message,bytes,true);
    }
    fn insert_kind(&mut self,key:Uuid,channel:Uuid,message:Uuid,bytes:Arc<[u8]>,video:bool) {
        self.remove(key);if bytes.is_empty()||bytes.len()>4<<20{return;}
        self.bytes+=bytes.len();self.entries.insert(key,Entry{channel,message,bytes,video});self.order.push_back(key);
        while self.bytes>BYTE_LIMIT||self.entries.len()>ENTRY_LIMIT {
            let Some(old)=self.order.front().copied()else{break;};self.remove(old);
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
    pub fn retain_channels(&mut self,allowed:&[Uuid]){self.retain(|entry|allowed.contains(&entry.channel));}
    pub fn invalidate_channel(&mut self,id:Uuid){self.retain(|entry|entry.channel!=id);}
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
        let mut keys=vec![first];for _ in 0..7{let key=Uuid::new_v4();keys.push(key);cache.insert(key,channel,message,bytes.clone());}
        cache.get(first);cache.insert(Uuid::new_v4(),channel,message,bytes);
        assert!(cache.get(first).is_some());assert!(cache.get(keys[1]).is_none());assert!(cache.bytes<=BYTE_LIMIT);
        for _ in 0..300{cache.insert(Uuid::new_v4(),channel,message,Arc::from([1u8]));}assert!(cache.entries.len()<=ENTRY_LIMIT);
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
