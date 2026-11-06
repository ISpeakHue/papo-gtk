//! EN-US: Private, disposable thumbnails; permission gates never wait for filesystem work.
//! PT-BR: Miniaturas privadas e descartáveis; permissões nunca aguardam operações de disco.
use std::{collections::HashSet,io::{Read,Write,BufRead},path::{Path,PathBuf},sync::{Arc,Mutex,OnceLock},time::{SystemTime,UNIX_EPOCH}};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use uuid::Uuid;
const BUDGET:u64=32<<20;
const GLOBAL_BUDGET:u64=128<<20;
const MAX_ENTRY:u64=2<<20;
const TTL:u64=7*24*3600;
static IO:OnceLock<Mutex<()>>=OnceLock::new();
static ENCODERS:OnceLock<Arc<tokio::sync::Semaphore>>=OnceLock::new();
static QUEUED:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
pub struct Key {pub channel:Uuid,pub message:Uuid,pub resource:Uuid,pub revision:String}
impl Key{fn name(&self)->String{format!("{:x}.thumb",Sha256::digest(serde_json::to_vec(self).expect("thumbnail key")))}}
#[derive(Serialize,Deserialize)]struct Header{key:Key,created:u64}
#[derive(Default)]struct Gate{allowed:HashSet<Uuid>,messages:HashSet<Uuid>,resources:HashSet<Uuid>,refresh_messages:HashSet<Uuid>,refresh_resources:HashSet<Uuid>,generation:u64,paused:bool,disabled:bool,purge_queued:bool,revoked:HashSet<Uuid>,retained:Option<HashSet<Uuid>>}
#[derive(Clone)]pub struct Cache{root:PathBuf,gate:Arc<Mutex<Gate>>}
#[derive(Clone)]pub struct Ticket{cache:Cache,key:Key,generation:u64}
fn now()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}
fn private_dir(path:&Path)->std::io::Result<()>{
    if let Ok(meta)=std::fs::symlink_metadata(path){if meta.file_type().is_symlink()||!meta.is_dir(){return Err(std::io::Error::other("invalid cache directory"));}}
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(path,std::fs::Permissions::from_mode(0o700))?;}
    Ok(())
}
fn files(root:&Path)->Vec<(PathBuf,u64,SystemTime)>{
    std::fs::read_dir(root).into_iter().flatten().filter_map(Result::ok).filter_map(|entry|{
        let path=entry.path();let name=path.file_name()?.to_str()?;
        if !name.ends_with(".thumb")&&!name.starts_with("pending-"){return None;}
        let meta=std::fs::symlink_metadata(&path).ok()?;
        if !meta.is_file()||meta.file_type().is_symlink(){return None;}
        Some((path,meta.len(),meta.modified().unwrap_or(UNIX_EPOCH)))
    }).collect()
}
fn header(path:&Path)->std::io::Result<(Header,Vec<u8>)>{read_entry(path,true)}
fn metadata(path:&Path)->std::io::Result<Header>{read_entry(path,false).map(|(h,_)|h)}
fn read_entry(path:&Path,body:bool)->std::io::Result<(Header,Vec<u8>)>{
    let meta=std::fs::symlink_metadata(path)?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_ENTRY+4096{return Err(std::io::Error::other("invalid cache file"));}
    let mut reader=std::io::BufReader::new(std::fs::File::open(path)?);let mut line=Vec::new();
    (&mut reader).take(4096).read_until(b'\n',&mut line)?;
    if line.last()!=Some(&b'\n'){return Err(std::io::Error::other("invalid thumbnail header"));}
    let h:Header=serde_json::from_slice(&line)?;if !body{return Ok((h,Vec::new()));}let mut data=Vec::new();reader.take(MAX_ENTRY+1).read_to_end(&mut data)?;
    if data.is_empty()||data.len() as u64>MAX_ENTRY||now().saturating_sub(h.created)>TTL{return Err(std::io::Error::other("expired thumbnail"));}
    Ok((h,data))
}
fn trim(root:&Path,budget:u64,limit:usize){
    let mut entries=files(root);entries.sort_by_key(|e|e.2);let mut bytes:u64=entries.iter().map(|e|e.1).sum();let mut count=entries.len();
    for (path,size,modified) in entries{if bytes>budget||count>limit||SystemTime::now().duration_since(modified).unwrap_or_default().as_secs()>TTL{let _=std::fs::remove_file(path);bytes=bytes.saturating_sub(size);count=count.saturating_sub(1);}}
}
impl Cache{
    pub fn for_account(owner:&crate::account::AccountId)->Option<Self>{
        #[cfg(test)]let base=PathBuf::from(std::env::var_os("PAPO_TEST_CONFIG_DIR")?).join("cache");
        #[cfg(not(test))]let base=dirs::cache_dir()?.join("papo-gtk");
        Some(Self::at(base.join("thumbnails-v1").join(owner.namespace())))
    }
    fn at(root:PathBuf)->Self{Self{root,gate:Default::default()}}
    pub fn permit(&self,channel:Uuid,read:bool){let mut gate=self.gate.lock().unwrap();if read{gate.allowed.insert(channel);gate.revoked.remove(&channel);if let Some(retained)=&mut gate.retained{retained.insert(channel);}}else{gate.allowed.remove(&channel);gate.revoked.insert(channel);gate.generation+=1;drop(gate);self.purge();}}
    pub fn retain(&self,channels:&[Uuid]){let mut gate=self.gate.lock().unwrap();let retained:HashSet<_>=channels.iter().copied().collect();if gate.retained.as_ref()!=Some(&retained){gate.generation+=1;}gate.allowed.retain(|c|retained.contains(c));gate.retained=Some(retained);drop(gate);self.purge();}
    pub fn invalidate(&self,message:Option<Uuid>,resource:Option<Uuid>){let mut gate=self.gate.lock().unwrap();gate.generation+=1;if let Some(id)=message{gate.messages.insert(id);}if let Some(id)=resource{gate.resources.insert(id);}if gate.messages.len()+gate.resources.len()>4096{gate.disabled=true;gate.messages.clear();gate.resources.clear();}drop(gate);self.purge();}
    // EN-US: Revision changes block old entries only until cleanup; deletions keep their tombstones.
    // PT-BR: Mudanças de versão bloqueiam entradas antigas até a limpeza; exclusões mantêm seus registros.
    pub fn refresh(&self,message:Option<Uuid>,resource:Option<Uuid>){let mut gate=self.gate.lock().unwrap();gate.generation+=1;if let Some(id)=message{gate.refresh_messages.insert(id);}if let Some(id)=resource{gate.refresh_resources.insert(id);}if gate.refresh_messages.len()+gate.refresh_resources.len()>4096{gate.disabled=true;gate.refresh_messages.clear();gate.refresh_resources.clear();}drop(gate);self.purge();}
    pub fn moderation(&self,resource:Uuid,visible:bool){if visible{self.gate.lock().unwrap().resources.remove(&resource);self.refresh(None,Some(resource));}else{self.invalidate(None,Some(resource));}}
    pub fn ticket(&self,key:Key)->Option<Ticket>{let gate=self.gate.lock().unwrap();if gate.paused||gate.disabled||!gate.allowed.contains(&key.channel)||gate.messages.contains(&key.message)||gate.resources.contains(&key.resource)||gate.refresh_messages.contains(&key.message)||gate.refresh_resources.contains(&key.resource){return None;}Some(Ticket{cache:self.clone(),key,generation:gate.generation})}
    fn purge(&self){let mut gate=self.gate.lock().unwrap();if gate.purge_queued{return;}gate.purge_queued=true;drop(gate);let cache=self.clone();tokio::task::spawn_blocking(move||{let _io=IO.get_or_init(Default::default).lock().unwrap();loop{let(generation,messages,resources,revoked,retained,disabled)={let gate=cache.gate.lock().unwrap();(gate.generation,gate.messages.union(&gate.refresh_messages).copied().collect::<HashSet<_>>(),gate.resources.union(&gate.refresh_resources).copied().collect::<HashSet<_>>(),gate.revoked.clone(),gate.retained.clone(),gate.disabled)};for(path,_,_)in files(&cache.root){if metadata(&path).map_or(true,|h|disabled||now().saturating_sub(h.created)>TTL||messages.contains(&h.key.message)||resources.contains(&h.key.resource)||revoked.contains(&h.key.channel)||retained.as_ref().is_some_and(|set|!set.contains(&h.key.channel))){let _=std::fs::remove_file(path);}}let mut gate=cache.gate.lock().unwrap();if gate.generation==generation{gate.purge_queued=false;gate.refresh_messages.clear();gate.refresh_resources.clear();break;}}});}
    pub fn clear(&self)->tokio::task::JoinHandle<()>{{let mut gate=self.gate.lock().unwrap();gate.generation+=1;gate.paused=true;}let cache=self.clone();tokio::task::spawn_blocking(move||{let _io=IO.get_or_init(Default::default).lock().unwrap();for(path,_,_)in files(&cache.root){let _=std::fs::remove_file(path);}cache.gate.lock().unwrap().paused=false;})}
    pub async fn usage(&self)->u64{let cache=self.clone();tokio::task::spawn_blocking(move||{let _io=IO.get_or_init(Default::default).lock().unwrap();trim(&cache.root,BUDGET,512);files(&cache.root).iter().map(|e|e.1).sum()}).await.unwrap_or(0)}
}
impl Ticket{
    fn valid(&self)->bool{let gate=self.cache.gate.lock().unwrap();gate.generation==self.generation&&!gate.paused&&!gate.disabled&&gate.allowed.contains(&self.key.channel)&&!gate.messages.contains(&self.key.message)&&!gate.resources.contains(&self.key.resource)&&!gate.refresh_messages.contains(&self.key.message)&&!gate.refresh_resources.contains(&self.key.resource)}
    pub async fn get(&self)->Option<Vec<u8>>{let ticket=self.clone();tokio::task::spawn_blocking(move||{let _io=IO.get_or_init(Default::default).lock().unwrap();if !ticket.valid(){return None;}let path=ticket.cache.root.join(ticket.key.name());match header(&path){Ok((h,data))if h.key==ticket.key&&ticket.valid()=>{if let Ok(file)=std::fs::File::options().write(true).open(path){let _=file.set_times(std::fs::FileTimes::new().set_modified(SystemTime::now()));}crate::performance::record("disk_thumbnail_hits",1);Some(data)},_=>{let _=std::fs::remove_file(path);crate::performance::record("disk_thumbnail_misses",1);None}}}).await.ok().flatten()}
    pub fn put_image(&self,image:&super::PreparedImage){
        if !self.valid()||image.width>680||image.height>460{return;}
        let mut queued=QUEUED.load(std::sync::atomic::Ordering::Relaxed);loop{if queued>=32{return;}match QUEUED.compare_exchange_weak(queued,queued+1,std::sync::atomic::Ordering::Relaxed,std::sync::atomic::Ordering::Relaxed){Ok(_)=>break,Err(actual)=>queued=actual}}
        let ticket=self.clone();let width=image.width;let height=image.height;let pixels=image.pixels.clone();
        tokio::spawn(async move{let permit=ENCODERS.get_or_init(||Arc::new(tokio::sync::Semaphore::new(2))).clone().acquire_owned().await;if let Ok(permit)=permit{let _=tokio::task::spawn_blocking(move||{let _permit=permit;if !ticket.valid(){return;}let mut png=std::io::Cursor::new(Vec::new());if let Some(pixels)=image::RgbaImage::from_raw(width,height,pixels){if image::DynamicImage::ImageRgba8(pixels).write_to(&mut png,image::ImageFormat::Png).is_ok(){ticket.put(&png.into_inner());}}}).await;}QUEUED.fetch_sub(1,std::sync::atomic::Ordering::Relaxed);});
    }
    fn put(&self,data:&[u8]){
        if data.is_empty()||data.len() as u64>MAX_ENTRY{return;}
        let _io=IO.get_or_init(Default::default).lock().unwrap();if !self.valid(){return;}
        let Some(parent)=self.cache.root.parent()else{return;};if private_dir(parent).and_then(|_|private_dir(&self.cache.root)).is_err(){return;}
        let path=self.cache.root.join(self.key.name());let pending=self.cache.root.join(format!("pending-{}",Uuid::new_v4()));
        let result=(||->std::io::Result<()>{let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);#[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
            let mut file=options.open(&pending)?;serde_json::to_writer(&mut file,&Header{key:self.key.clone(),created:now()})?;file.write_all(b"\n")?;file.write_all(data)?;file.flush()?;drop(file);
            if !self.valid(){return Err(std::io::Error::other("obsolete thumbnail"));}let _=std::fs::remove_file(&path);std::fs::rename(&pending,&path)
        })();if result.is_err(){let _=std::fs::remove_file(&pending);}
        trim(&self.cache.root,BUDGET,512);
        // EN-US: Accounts share a hard disk ceiling without sharing keys or pixels.
        // PT-BR: Contas compartilham um teto de disco, sem compartilhar chaves ou imagens.
        let mut entries=Vec::new();for child in std::fs::read_dir(parent).into_iter().flatten().filter_map(Result::ok){if child.file_type().is_ok_and(|t|t.is_dir()&&!t.is_symlink()){entries.extend(files(&child.path()));}}
        entries.sort_by_key(|e|e.2);let mut total:u64=entries.iter().map(|e|e.1).sum();let mut count=entries.len();for(path,size,_)in entries{if total<=GLOBAL_BUDGET&&count<=2048{break;}let _=std::fs::remove_file(path);total=total.saturating_sub(size);count=count.saturating_sub(1);}
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[tokio::test]async fn refreshed_previews_can_be_cached_again_but_deleted_messages_cannot(){
        let root=std::env::temp_dir().join(format!("papo-thumbnails-{}",Uuid::new_v4()));let cache=Cache::at(root.join("account"));let channel=Uuid::new_v4();cache.permit(channel,true);let key=Key{channel,message:Uuid::new_v4(),resource:Uuid::new_v4(),revision:"v1".into()};let old=cache.ticket(key.clone()).unwrap();old.put(b"old");cache.refresh(Some(key.message),Some(key.resource));
        for _ in 0..100{if !cache.gate.lock().unwrap().purge_queued{break;}tokio::time::sleep(std::time::Duration::from_millis(5)).await;}
        assert!(old.get().await.is_none());let mut revised=key.clone();revised.revision="v2".into();let fresh=cache.ticket(revised.clone()).unwrap();fresh.put(b"new");assert_eq!(fresh.get().await.unwrap(),b"new");cache.invalidate(Some(key.message),None);assert!(cache.ticket(revised).is_none());cache.clear().await.unwrap();let _=std::fs::remove_dir_all(root);
    }
    #[tokio::test]async fn byte_budget_evicts_old_files_and_keeps_recent_hits(){
        let root=std::env::temp_dir().join(format!("papo-thumbnails-{}",Uuid::new_v4()));let cache=Cache::at(root.join("account"));let channel=Uuid::new_v4();cache.permit(channel,true);let mut keys=Vec::new();
        // EN-US: Seed distinct recency values; Windows clock ticks can tie rapid writes.
        // PT-BR: Define acessos distintos; o relógio do Windows pode empatar gravações rápidas.
        let written=SystemTime::now()-std::time::Duration::from_secs(3600);
        for _ in 0..33{let key=Key{channel,message:Uuid::new_v4(),resource:Uuid::new_v4(),revision:"v1".into()};cache.ticket(key.clone()).unwrap().put(&vec![1;1<<20]);{let file=std::fs::File::options().write(true).open(cache.root.join(key.name())).unwrap();file.set_times(std::fs::FileTimes::new().set_modified(written+std::time::Duration::from_secs(keys.len() as u64))).unwrap();}keys.push(key);if keys.len()==16{assert!(cache.ticket(keys[0].clone()).unwrap().get().await.is_some());}}
        assert!(cache.usage().await<=BUDGET);assert!(cache.ticket(keys[0].clone()).unwrap().get().await.is_some());assert!(cache.ticket(keys[1].clone()).unwrap().get().await.is_none());
        #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&cache.root).unwrap().permissions().mode()&0o777,0o700);assert_eq!(std::fs::metadata(cache.root.join(keys.last().unwrap().name())).unwrap().permissions().mode()&0o777,0o600);}
        cache.clear().await.unwrap();let _=std::fs::remove_dir_all(root);
    }
    #[tokio::test]async fn restart_revision_permissions_and_clear_reject_late_writes(){
        let root=std::env::temp_dir().join(format!("papo-thumbnails-{}",Uuid::new_v4()));let cache=Cache::at(root.join("account-a"));let channel=Uuid::new_v4();let key=Key{channel,message:Uuid::new_v4(),resource:Uuid::new_v4(),revision:"v1".into()};cache.permit(channel,true);let ticket=cache.ticket(key.clone()).unwrap();ticket.put(b"preview");assert_eq!(ticket.get().await.unwrap(),b"preview");
        let restart=Cache::at(cache.root.clone());assert!(restart.ticket(key.clone()).is_none());restart.permit(channel,true);assert_eq!(restart.ticket(key.clone()).unwrap().get().await.unwrap(),b"preview");let mut revised=key.clone();revised.revision="v2".into();assert!(restart.ticket(revised).unwrap().get().await.is_none());
        let other=Cache::at(root.join("account-b"));other.permit(channel,true);assert!(other.ticket(key.clone()).unwrap().get().await.is_none());
        cache.invalidate(Some(key.message),None);assert!(ticket.get().await.is_none());ticket.put(b"stale");cache.clear().await.unwrap();assert_eq!(cache.usage().await,0);
        let key=Key{message:Uuid::new_v4(),..key};let ticket=cache.ticket(key.clone()).unwrap();ticket.put(b"new");cache.permit(channel,false);assert!(ticket.get().await.is_none());assert!(cache.ticket(key).is_none());cache.clear().await.unwrap();let _=std::fs::remove_dir_all(root);
    }
    #[tokio::test]async fn expired_corrupt_and_oversized_entries_are_disposable(){
        let root=std::env::temp_dir().join(format!("papo-thumbnails-{}",Uuid::new_v4()));let cache=Cache::at(root.join("account"));let channel=Uuid::new_v4();cache.permit(channel,true);let ticket=cache.ticket(Key{channel,message:Uuid::new_v4(),resource:Uuid::new_v4(),revision:"v1".into()}).unwrap();ticket.put(b"ok");let path=cache.root.join(ticket.key.name());std::fs::write(&path,b"corrupt").unwrap();assert!(ticket.get().await.is_none());ticket.put(&vec![0;MAX_ENTRY as usize+1]);assert_eq!(cache.usage().await,0);let mut h=serde_json::to_vec(&Header{key:ticket.key.clone(),created:now()-TTL-1}).unwrap();h.extend_from_slice(b"\nold");std::fs::write(path,h).unwrap();assert!(ticket.get().await.is_none());cache.clear().await.unwrap();let _=std::fs::remove_dir_all(root);
    }
}
