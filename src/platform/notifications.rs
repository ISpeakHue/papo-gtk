//! EN-US: Serialized toast delivery and durable, account-bound activation without credential arguments.
//! PT-BR: Entrega serializada e ativação persistente vinculada à conta, sem credenciais nos argumentos.
use super::activation::Account;
use serde::{Deserialize,Serialize};
use std::{collections::{HashMap,HashSet},path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
use tokio::sync::{mpsc,oneshot};
use uuid::Uuid;

#[cfg(windows)]mod windows;
#[cfg(windows)]pub use windows::{backend,forget,check};
#[cfg(all(test,not(windows)))]pub fn backend()->Option<Backend>{None}
const LIMIT:usize=128;
const LIFETIME:u64=24*60*60;
fn now()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {pub notice:Uuid,pub channel:Uuid,pub message:Uuid}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {owner:Account,target:Target,expires:u64,group:String,#[serde(default)]revoked:bool}
#[derive(Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {tickets:HashMap<Uuid,Ticket>}
fn group(owner:&Account,scope:&str)->String{
    use sha2::{Digest,Sha256};
    let data=serde_json::to_vec(&(scope,owner)).unwrap();
    format!("{:x}",Sha256::digest(data))[..16].to_owned()
}
fn escaped(value:&str)->String{
    let mut text=String::new();
    for c in value.chars().take(2048){match c{
        '&'=>text.push_str("&amp;"),'<' =>text.push_str("&lt;"),'>' =>text.push_str("&gt;"),'"'=>text.push_str("&quot;"),'\''=>text.push_str("&apos;"),
        '\t'|'\n'|'\r'=>text.push(c),c if c>=' '&&c!='\u{fffe}'&&c!='\u{ffff}'=>text.push(c),_=>{}
    }}text
}
fn xml(id:Uuid,body:&str,sound:bool)->String{
    let audio=if sound{"<audio src=\"ms-winsoundevent:Notification.Default\"/>"}else{"<audio silent=\"true\"/>"};
    format!("<toast activationType=\"protocol\" launch=\"papo-notification://open/{id}\"><visual><binding template=\"ToastGeneric\"><text>Nova mensagem no Papo</text><text>{}</text></binding></visual>{audio}</toast>",escaped(body))
}

// EN-US: Only this metadata persists. Preview text stays on a private stdin pipe, never in argv or the registry.
// PT-BR: Só estes metadados persistem. A prévia passa por stdin privado, nunca por argv nem pelo registro.
struct Repository {path:PathBuf,registry:Registry}
impl Repository {
    fn open(path:PathBuf)->anyhow::Result<Self>{
        use std::io::Read;
        let mut bytes=Vec::new();
        let registry=match std::fs::File::open(&path){
            Ok(file)=>{file.take(256*1024+1).read_to_end(&mut bytes)?;anyhow::ensure!(bytes.len()<=256*1024,"Notification registry exceeds limit");
                let registry:Registry=serde_json::from_slice(&unprotect(&bytes)?)?;
                anyhow::ensure!(registry.tickets.len()<=LIMIT,"Too many notification tickets");
                for (id,t) in &registry.tickets {anyhow::ensure!(!id.is_nil()&&!t.target.notice.is_nil()&&!t.target.channel.is_nil()&&!t.target.message.is_nil()&&t.group.len()==16&&t.group.bytes().all(|b|b.is_ascii_hexdigit()),"Invalid notification ticket");}
                registry},
            Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Registry::default(),Err(e)=>return Err(e.into())
        };Ok(Self{path,registry})
    }
    fn save(&self)->anyhow::Result<()>{
        std::fs::create_dir_all(self.path.parent().ok_or_else(||anyhow::anyhow!("Notification directory unavailable"))?)?;
        let bytes=protect(&serde_json::to_vec(&self.registry)?)?;
        anyhow::ensure!(bytes.len()<=256*1024,"Notification metadata exceeds limit");
        crate::platform::files::atomic_write(&self.path,&bytes)
    }
}
#[cfg(windows)]fn protect(data:&[u8])->anyhow::Result<Vec<u8>>{windows::crypt(data,true)}
#[cfg(windows)]fn unprotect(data:&[u8])->anyhow::Result<Vec<u8>>{windows::crypt(data,false)}
#[cfg(all(test,not(windows)))]fn protect(data:&[u8])->anyhow::Result<Vec<u8>>{Ok(data.to_vec())}
#[cfg(all(test,not(windows)))]fn unprotect(data:&[u8])->anyhow::Result<Vec<u8>>{Ok(data.to_vec())}

pub enum Operation {Ready,Show{id:Uuid,group:String,xml:String},Remove{id:Uuid,group:String}}
pub(super) trait Transport:Send+'static{
    fn perform(&mut self,operation:Operation)->impl std::future::Future<Output=anyhow::Result<()>>+Send;
}
enum Request {
    Ready(oneshot::Sender<bool>),
    Show{owner:Account,target:Target,body:String,sound:bool},
    Reconcile{owner:Account,allowed:HashSet<Uuid>,readable:HashSet<Uuid>},
    Resolve{owner:Account,id:Uuid,readable:HashSet<Uuid>,reply:oneshot::Sender<Option<Target>>},
    Forget(Account),
    Stop(oneshot::Sender<()>),
}
#[derive(Clone)]pub struct Backend {sender:mpsc::Sender<Request>}
impl Backend {
    fn start(path:PathBuf,scope:String,mut transport:impl Transport)->Self{
        let(sender,mut requests)=mpsc::channel(64);
        tokio::spawn(async move{
            // EN-US: The actor orders show/removal/logout, including completion after a delayed native Show.
            // PT-BR: O ator ordena exibição/remoção/logout, inclusive quando Show nativo demora a concluir.
            let mut repository=match Repository::open(path){Ok(r)=>r,Err(_)=>{tracing::warn!("Windows notification metadata unavailable");return;}};
            let mut cleanup=tokio::time::interval(std::time::Duration::from_secs(30));
            loop{
                let request=tokio::select!{
                    request=requests.recv()=>match request{Some(request)=>request,None=>break},
                    _=cleanup.tick()=>{
                        let expired=repository.registry.tickets.iter().filter(|(_,t)|t.revoked||t.expires<=now()).take(16).map(|(id,_)|*id).collect();
                        remove(&mut repository,&mut transport,expired).await;continue;
                    }
                };
                match request{
                    Request::Ready(reply)=>{let _=reply.send(transport.perform(Operation::Ready).await.is_ok());}
                    Request::Show{owner,target,body,sound}=>{
                        if repository.registry.tickets.len()>=LIMIT{
                            if let Some(id)=repository.registry.tickets.iter().min_by_key(|(_,t)|t.expires).map(|(id,_)|*id){remove(&mut repository,&mut transport,vec![id]).await;}
                        }
                        if repository.registry.tickets.len()>=LIMIT{continue;}
                        let id=Uuid::new_v4();let group=group(&owner,&scope);
                        repository.registry.tickets.insert(id,Ticket{owner,target,expires:now()+LIFETIME,group:group.clone(),revoked:false});
                        if repository.save().is_err(){repository.registry.tickets.remove(&id);continue;}
                        if transport.perform(Operation::Show{id,group,xml:xml(id,&body,sound)}).await.is_err(){remove(&mut repository,&mut transport,vec![id]).await;}
                    }
                    Request::Reconcile{owner,allowed,readable}=>{
                        let ids=repository.registry.tickets.iter().filter(|(_,t)|t.owner==owner&&(!allowed.contains(&t.target.notice)||!readable.contains(&t.target.channel))).map(|(id,_)|*id).collect();
                        remove(&mut repository,&mut transport,ids).await;
                    }
                    Request::Resolve{owner,id,readable,reply}=>{
                        let target=repository.registry.tickets.get(&id).filter(|t|t.owner==owner&&!t.revoked&&t.expires>now()&&readable.contains(&t.target.channel)).map(|t|t.target);
                        // EN-US: A click is single-use; even denied or expired destinations cannot be replayed.
                        // PT-BR: Um clique é de uso único; destinos negados ou expirados não podem ser repetidos.
                        if repository.registry.tickets.get(&id).is_some_and(|t|t.owner==owner){if !remove(&mut repository,&mut transport,vec![id]).await{let _=reply.send(None);continue;}}
                        let _=reply.send(target);
                    }
                    Request::Stop(reply)=>{let _=reply.send(());break;}
                    Request::Forget(owner)=>{
                        let ids=repository.registry.tickets.iter().filter(|(_,t)|t.owner==owner).map(|(id,_)|*id).collect();
                        remove(&mut repository,&mut transport,ids).await;
                    }
                }
            }
        });Self{sender}
    }
    pub async fn ready(&self)->bool{let(tx,rx)=oneshot::channel();if self.sender.send(Request::Ready(tx)).await.is_err(){return false;}rx.await.unwrap_or(false)}
    pub fn show(&self,owner:Account,target:Target,body:String,sound:bool)->bool{if owner.user.is_nil()||owner.session.is_nil()||owner.server.len()>4096||target.notice.is_nil()||target.channel.is_nil()||target.message.is_nil(){return false;}self.sender.try_send(Request::Show{owner,target,body:body.chars().take(2048).collect(),sound}).is_ok()}
    pub fn reconcile(&self,owner:Account,allowed:HashSet<Uuid>,readable:HashSet<Uuid>)->bool{self.sender.try_send(Request::Reconcile{owner,allowed,readable}).is_ok()}
    pub async fn resolve(&self,owner:Account,id:Uuid,readable:HashSet<Uuid>)->Option<Target>{let(tx,rx)=oneshot::channel();self.sender.send(Request::Resolve{owner,id,readable,reply:tx}).await.ok()?;rx.await.ok().flatten()}
    pub async fn forget(&self,owner:Account){let _=self.sender.send(Request::Forget(owner)).await;}
    async fn stop(&self){let(tx,rx)=oneshot::channel();if self.sender.send(Request::Stop(tx)).await.is_ok(){let _=rx.await;}}
}
async fn remove(repository:&mut Repository,transport:&mut impl Transport,ids:Vec<Uuid>)->bool{
    if ids.is_empty(){return true;}
    // EN-US: Revoke authority durably before touching the shell; failed removals retry after restart.
    // PT-BR: Revoga a autoridade de forma persistente antes do shell; remoções malsucedidas são repetidas ao reiniciar.
    for id in &ids{if let Some(ticket)=repository.registry.tickets.get_mut(id){ticket.revoked=true;}}
    let persisted=repository.save().is_ok();
    if !persisted{tracing::warn!("Windows notification revocation could not be persisted");}
    for id in ids{if let Some(ticket)=repository.registry.tickets.get(&id){if transport.perform(Operation::Remove{id,group:ticket.group.clone()}).await.is_ok(){repository.registry.tickets.remove(&id);}}}
    let saved=repository.save().is_ok();if !saved{tracing::warn!("Windows notification metadata cleanup failed");}persisted||saved
}

#[cfg(test)]pub(crate) fn fixture(path:PathBuf)->(Backend,std::sync::Arc<std::sync::Mutex<Vec<Operation>>>){
    #[derive(Default)]struct Capture(std::sync::Arc<std::sync::Mutex<Vec<Operation>>>);
    impl Transport for Capture{async fn perform(&mut self,operation:Operation)->anyhow::Result<()>{self.0.lock().unwrap().push(operation);Ok(())}}
    let transport=Capture::default();let calls=transport.0.clone();(Backend::start(path,"fixture".into(),transport),calls)
}

#[cfg(test)]mod tests{
    use super::*;
    use std::sync::{Arc,Mutex};
    #[derive(Clone,Default)]struct Fake {calls:Arc<Mutex<Vec<Operation>>>,fail_show:bool,fail_remove:bool}
    impl Transport for Fake{async fn perform(&mut self,operation:Operation)->anyhow::Result<()>{
        let fail=self.fail_show&&matches!(operation,Operation::Show{..})||self.fail_remove&&matches!(operation,Operation::Remove{..});self.calls.lock().unwrap().push(operation);
        anyhow::ensure!(!fail,"Synthetic native failure");Ok(())
    }}
    struct Fixture{directory:PathBuf,backend:Backend,fake:Fake,owner:Account,target:Target}
    impl Fixture{
        fn new()->Self{Self::with_fake(Fake::default())}
        fn with_fake(fake:Fake)->Self{
            let directory=std::env::temp_dir().join(format!("papo-notices-{}",Uuid::new_v4()));
            let backend=Backend::start(directory.join("tickets.dat"),"session-1".into(),fake.clone());
            Self{directory,backend,fake,owner:Account{server:"https://fixture.invalid/api".into(),user:Uuid::new_v4(),session:Uuid::new_v4()},target:Target{notice:Uuid::new_v4(),channel:Uuid::new_v4(),message:Uuid::new_v4()}}
        }
        async fn show(&self)->Uuid{
            assert!(self.backend.show(self.owner.clone(),self.target,"private <message> & test".into(),false));assert!(self.backend.ready().await);
            self.fake.calls.lock().unwrap().iter().rev().find_map(|op|match op{Operation::Show{id,..}=>Some(*id),_=>None}).unwrap()
        }
        fn readable(&self)->HashSet<Uuid>{[self.target.channel].into()}
    }
    impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.directory);}}
    #[test]fn xml_is_bounded_escaped_and_contains_only_an_opaque_launch_ticket(){
        let id=Uuid::new_v4();let text=xml(id,"</text><audio src=\"x\"/>\u{0}& á",false);
        assert!(text.contains("&lt;/text&gt;"));assert!(!text.contains("src=\"x\""));assert!(!text.contains('\0'));
        assert!(text.contains(&format!("launch=\"papo-notification://open/{id}\"")));assert!(text.contains("<audio silent=\"true\"/>"));
        assert!(xml(id,"hello",true).contains("ms-winsoundevent:Notification.Default"));assert!(xml(id,&"😀".repeat(100000),false).len()<16384);
    }
    #[tokio::test]async fn cold_start_resolves_only_the_matching_account_session_and_permission(){
        let f=Fixture::new();let id=f.show().await;f.backend.stop().await;
        // EN-US: Restart the actor from disk, without retaining an in-memory destination.
        // PT-BR: Reinicia o ator a partir do disco, sem manter o destino em memória.
        let cold=Backend::start(f.directory.join("tickets.dat"),"session-1".into(),f.fake.clone());
        for wrong in [Account{server:"https://another.invalid".into(),..f.owner.clone()},Account{user:Uuid::new_v4(),..f.owner.clone()},Account{session:Uuid::new_v4(),..f.owner.clone()}]{assert_eq!(cold.resolve(wrong,id,f.readable()).await,None);}
        assert_eq!(cold.resolve(f.owner.clone(),id,f.readable()).await,Some(f.target));
        assert_eq!(cold.resolve(f.owner.clone(),id,f.readable()).await,None);
    }
    #[tokio::test]async fn denied_permission_read_and_logout_remove_alerts_and_destinations(){
        let f=Fixture::new();let id=f.show().await;assert_eq!(f.backend.resolve(f.owner.clone(),id,HashSet::new()).await,None);
        let id=f.show().await;assert!(f.backend.reconcile(f.owner.clone(),HashSet::new(),f.readable()));assert!(f.backend.ready().await);
        assert_eq!(f.backend.resolve(f.owner.clone(),id,f.readable()).await,None);
        let id=f.show().await;f.backend.forget(f.owner.clone()).await;assert!(f.backend.ready().await);
        assert_eq!(f.backend.resolve(f.owner.clone(),id,f.readable()).await,None);
        assert!(f.fake.calls.lock().unwrap().iter().filter(|c|matches!(c,Operation::Remove{..})).count()>=3);
    }
    #[tokio::test]async fn accounts_have_separate_groups_and_reconciliation_does_not_cross_them(){
        let f=Fixture::new();let first=f.show().await;let other=Account{user:Uuid::new_v4(),..f.owner.clone()};
        assert_ne!(group(&f.owner,"session-1"),group(&other,"session-1"));assert_ne!(group(&f.owner,"session-1"),group(&f.owner,"session-2"));
        f.backend.show(other.clone(),f.target,"other".into(),true);assert!(f.backend.ready().await);
        let second=f.fake.calls.lock().unwrap().iter().rev().find_map(|c|if let Operation::Show{id,..}=c{Some(*id)}else{None}).unwrap();
        f.backend.forget(f.owner.clone()).await;assert!(f.backend.ready().await);
        assert_eq!(f.backend.resolve(f.owner.clone(),first,f.readable()).await,None);
        assert_eq!(f.backend.resolve(other,second,f.readable()).await,Some(f.target));
    }
    #[tokio::test]async fn native_show_failure_leaves_no_usable_ticket(){
        let f=Fixture::with_fake(Fake{fail_show:true,..Default::default()});let id=f.show().await;
        assert_eq!(f.backend.resolve(f.owner.clone(),id,f.readable()).await,None);
    }
    #[tokio::test]async fn failed_native_removal_is_durably_revoked_and_retried_after_restart(){
        let f=Fixture::with_fake(Fake{fail_remove:true,..Default::default()});let id=f.show().await;
        f.backend.forget(f.owner.clone()).await;assert!(f.backend.ready().await);
        let repository=Repository::open(f.directory.join("tickets.dat")).unwrap();assert!(repository.registry.tickets[&id].revoked);
        assert_eq!(f.backend.resolve(f.owner.clone(),id,f.readable()).await,None);
        f.backend.stop().await;let healthy=Fake::default();let cold=Backend::start(f.directory.join("tickets.dat"),"session-1".into(),healthy.clone());
        assert_eq!(cold.resolve(f.owner.clone(),id,f.readable()).await,None);
        assert!(healthy.calls.lock().unwrap().iter().any(|op|matches!(op,Operation::Remove{..})));
        assert!(Repository::open(f.directory.join("tickets.dat")).unwrap().registry.tickets.is_empty());
    }
    #[tokio::test]async fn logout_queued_during_native_show_cannot_resurrect_an_alert(){
        struct Slow(Fake);
        impl Transport for Slow{async fn perform(&mut self,operation:Operation)->anyhow::Result<()>{if matches!(operation,Operation::Show{..}){tokio::time::sleep(std::time::Duration::from_millis(20)).await;}self.0.perform(operation).await}}
        let f=Fixture::new();let backend=Backend::start(f.directory.join("slow.dat"),"session-1".into(),Slow(f.fake.clone()));
        assert!(backend.show(f.owner.clone(),f.target,"pending preview".into(),false));backend.forget(f.owner.clone()).await;assert!(backend.ready().await);
        let calls=f.fake.calls.lock().unwrap();let shown=calls.iter().position(|op|matches!(op,Operation::Show{..})).unwrap();let removed=calls.iter().position(|op|matches!(op,Operation::Remove{..})).unwrap();assert!(shown<removed);
        assert!(Repository::open(f.directory.join("slow.dat")).unwrap().registry.tickets.is_empty());
    }
    #[tokio::test]async fn expired_entries_are_removed_before_activation(){
        let f=Fixture::new();f.backend.stop().await;let mut repo=Repository{path:f.directory.join("tickets.dat"),registry:Registry::default()};let id=Uuid::new_v4();
        repo.registry.tickets.insert(id,Ticket{owner:f.owner.clone(),target:f.target,expires:now()-1,group:group(&f.owner,"session-1"),revoked:false});repo.save().unwrap();
        let cold=Backend::start(repo.path.clone(),"session-1".into(),f.fake.clone());assert_eq!(cold.resolve(f.owner.clone(),id,f.readable()).await,None);
    }
    #[test]fn corrupt_and_oversized_storage_is_rejected(){
        let directory=std::env::temp_dir().join(format!("papo-notice-bounds-{}",Uuid::new_v4()));std::fs::create_dir(&directory).unwrap();let path=directory.join("tickets.dat");
        for bytes in [b"invalid".to_vec(),vec![b'x';256*1024+1]]{std::fs::write(&path,bytes).unwrap();assert!(Repository::open(path.clone()).is_err());}
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[tokio::test]async fn previews_are_not_persisted_and_metadata_stays_bounded(){
        let f=Fixture::new();
        for _ in 0..LIMIT+4{assert!(f.backend.show(f.owner.clone(),f.target,"private preview text".into(),false));assert!(f.backend.ready().await);}
        let bytes=std::fs::read(f.directory.join("tickets.dat")).unwrap();let plain=unprotect(&bytes).unwrap();assert!(!String::from_utf8_lossy(&plain).contains("private preview text"));
        let repository=Repository::open(f.directory.join("tickets.dat")).unwrap();assert_eq!(repository.registry.tickets.len(),LIMIT);
        let calls=f.fake.calls.lock().unwrap();for call in calls.iter(){match call{Operation::Show{group,xml,..}=>{assert_eq!(group.len(),16);assert!(xml.contains("silent=\"true\""));},Operation::Remove{group,id}=>{assert_eq!(group.len(),16);assert!(!id.is_nil());},Operation::Ready=>{}}}
    }
}
