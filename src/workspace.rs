//! EN-US: Bounded non-secret account directory. Credentials remain in the native keyring.
//! PT-BR: Diretório limitado de contas sem segredos. Credenciais permanecem no chaveiro nativo.
use crate::account::AccountId;
use serde::{Serialize,Deserialize};
pub const SAVED_LIMIT:usize=32;
pub const LIVE_LIMIT:usize=8;
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {pub id:AccountId,pub username:String,pub name:String,pub remember:bool,pub conversation:Option<uuid::Uuid>}
#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Directory {pub accounts:Vec<Saved>,pub selected:Option<AccountId>}
impl Directory {
    pub fn path()->anyhow::Result<std::path::PathBuf>{
        #[cfg(test)]anyhow::ensure!(std::env::var_os("PAPO_TEST_CONFIG_DIR").is_some(),"isolated test directory required");
        Ok(crate::config::AppConfig::path()?.with_file_name("accounts-v1.json"))}
    pub fn load(path:&std::path::Path)->anyhow::Result<Self>{
        use std::io::Read;
        let mut bytes=vec![];match std::fs::File::open(path){Ok(file)=>{file.take(256*1024+1).read_to_end(&mut bytes)?;},Err(e)if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Self::default()),Err(e)=>return Err(e.into())}
        anyhow::ensure!(bytes.len()<=256*1024,"account directory exceeds limit");let state:Self=serde_json::from_slice(&bytes)?;state.validate()?;Ok(state)
    }
    fn validate(&self)->anyhow::Result<()>{
        anyhow::ensure!(self.accounts.len()<=SAVED_LIMIT,"too many saved accounts");let mut ids=std::collections::HashSet::new();
        for a in &self.accounts{anyhow::ensure!(ids.insert(a.id.clone())&&!a.username.is_empty()&&a.username.len()<=256&&a.name.len()<=1024,"invalid saved account");}
        anyhow::ensure!(self.selected.as_ref().is_none_or(|id|ids.contains(id)),"unknown selected account");Ok(())
    }
    pub fn save(&self,path:&std::path::Path)->anyhow::Result<()>{self.validate()?;let bytes=serde_json::to_vec(self)?;anyhow::ensure!(bytes.len()<=256*1024,"account directory exceeds limit");if let Some(p)=path.parent(){std::fs::create_dir_all(p)?;}crate::platform::files::atomic_write(path,&bytes)}
    pub fn upsert(&mut self,account:Saved)->anyhow::Result<()>{
        let mut next=self.clone();if let Some(old)=next.accounts.iter_mut().find(|a|a.id==account.id){*old=account;}else{anyhow::ensure!(next.accounts.len()<SAVED_LIMIT,"saved account limit");next.accounts.push(account);}next.validate()?;*self=next;Ok(())
    }
    pub fn remove(&mut self,id:&AccountId){self.accounts.retain(|a|&a.id!=id);if self.selected.as_ref()==Some(id){self.selected=None;}}
}
/// EN-US: A single ordered writer coalesces rapid switches; older writes cannot win.
/// PT-BR: Um gravador ordenado agrupa trocas rápidas; gravações antigas não vencem.
pub struct Writer {sender:tokio::sync::watch::Sender<Option<Directory>>}
impl Writer {
    pub fn new(path:std::path::PathBuf,report:impl Fn(anyhow::Result<()>)+Send+'static)->Self{
        let(sender,mut receiver)=tokio::sync::watch::channel(None::<Directory>);
        tokio::spawn(async move{while receiver.changed().await.is_ok(){let state=receiver.borrow_and_update().clone();if let Some(state)=state{let path=path.clone();let result=tokio::task::spawn_blocking(move||state.save(&path)).await.map_err(anyhow::Error::from).and_then(|r|r);report(result);}}});Self{sender}
    }
    pub fn submit(&self,state:Directory){self.sender.send_replace(Some(state));}
}
#[cfg(test)]mod tests{
    use super::*;
    fn saved(host:&str)->Saved{let api=crate::api::ApiClient::new(host).unwrap();Saved{id:AccountId::new(&api,uuid::Uuid::nil()),username:"Alice".into(),name:"Test".into(),remember:true,conversation:None}}
    #[test]fn directory_is_bounded_validated_and_contains_no_credentials(){let mut d=Directory::default();let a=saved("https://one.test/api");let b=saved("https://two.test/api");d.upsert(a.clone()).unwrap();d.upsert(b.clone()).unwrap();d.selected=Some(b.id.clone());let text=serde_json::to_string(&d).unwrap();assert!(!text.contains("cookie")&&!text.contains("auth"));let mut copy:Directory=serde_json::from_str(&text).unwrap();copy.validate().unwrap();copy.remove(&b.id);assert!(copy.selected.is_none());assert_eq!(copy.accounts[0].id,a.id);copy.accounts.push(a);assert!(copy.validate().is_err());for _ in 0..SAVED_LIMIT{d.accounts.push(b.clone());}assert!(d.validate().is_err());}
    #[tokio::test]async fn ordered_writer_keeps_the_last_selection_and_survives_restart(){let dir=std::env::temp_dir().join(format!("papo-workspace-{}",uuid::Uuid::new_v4()));let path=dir.join("accounts.json");let writer=Writer::new(path.clone(),|r|r.unwrap());let mut state=Directory::default();let a=saved("https://one.test");let b=saved("https://two.test");state.upsert(a.clone()).unwrap();state.upsert(b.clone()).unwrap();for i in 0..30{state.selected=Some(if i%2==0{a.id.clone()}else{b.id.clone()});writer.submit(state.clone());}drop(writer);tokio::time::timeout(std::time::Duration::from_secs(5),async{loop{if Directory::load(&path).ok().as_ref()==Some(&state){break;}tokio::time::sleep(std::time::Duration::from_millis(10)).await;}}).await.unwrap();std::fs::remove_dir_all(dir).unwrap();}
}
