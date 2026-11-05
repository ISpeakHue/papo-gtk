//! EN-US: Private, bounded, account-scoped draft snapshots; credentials and media bytes never enter them.
//! PT-BR: Snapshots privados e limitados de rascunhos por conta, sem credenciais nem bytes de mídia.
use std::{collections::BTreeMap,path::{Path,PathBuf}};
use anyhow::{Context,Result};
use serde::{Deserialize,Serialize};
use uuid::Uuid;
use crate::{account::AccountId,api::features::UploadFile,models::{EmbedInput,Message},ui::chat::state::Draft};

const LIMIT:usize=2<<20;
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct Reply {pub id:Uuid,pub channel:Uuid,pub created_at:chrono::DateTime<chrono::Utc>}
impl Reply {
    pub fn placeholder(&self)->Message{
        serde_json::from_value(serde_json::json!({"id":self.id,"channel_id":self.channel,"created_at":self.created_at,"content":crate::i18n::tr("Carregando resposta…")})).expect("local reply metadata")
    }
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct FileRef {path:PathBuf,name:String,size:u64,temporary:bool}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]
pub struct SavedDraft {
    pub text:String,
    pub embeds:Vec<EmbedInput>,
    files:Vec<FileRef>,
    #[serde(default)] missing_attachments:usize,
    pub reply:Option<Reply>,
}
impl SavedDraft {
    pub fn capture(draft:&Draft)->Self{
        Self{text:draft.text.clone(),embeds:draft.embeds.clone(),missing_attachments:draft.missing_attachments,
            files:draft.files.iter().map(|f|FileRef{path:f.path.clone(),name:f.name.clone(),size:f.size,temporary:f._temporary.is_some()}).collect(),
            reply:draft.reply.as_ref().map(|r|Reply{id:r.id,channel:r.channel_id,created_at:r.created_at})}
    }
    pub fn is_empty(&self)->bool{self.text.is_empty()&&self.embeds.is_empty()&&self.files.is_empty()&&self.reply.is_none()&&self.missing_attachments==0}
    pub async fn restore(self)->Draft{
        let mut draft=Draft::default();draft.text=self.text;draft.embeds=self.embeds;
        draft.reply=self.reply.as_ref().map(Reply::placeholder);draft.reply_unresolved=draft.reply.is_some();
        let mut missing=self.missing_attachments;
        for file in self.files{
            if file.temporary||!file.path.is_absolute(){missing+=1;continue;}
            match UploadFile::inspect(file.path).await{
                Ok(selected) if selected.size==file.size=>draft.files.push(selected),_=>missing+=1,
            }
        }
        draft.missing_attachments=missing;
        if missing>0{draft.recovery_notice=Some(crate::i18n::plural("missing_attachments",missing));}
        draft
    }
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct SavedPending {
    pub id:Uuid,pub channel:Uuid,pub created_at:chrono::DateTime<chrono::Utc>,
    pub draft:SavedDraft,pub attempted:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct Snapshot {version:u8,account:AccountId,pub drafts:BTreeMap<Uuid,SavedDraft>,pub pending:Vec<SavedPending>}
impl Snapshot {
    pub fn empty(account:AccountId)->Self{Self{version:1,account,drafts:Default::default(),pending:vec![]}}
    fn validate(&self,account:&AccountId)->Result<()>{
        anyhow::ensure!(self.version==1&&&self.account==account,crate::i18n::fmt("Arquivo de rascunhos de outra conta ou versão.", &[]));
        anyhow::ensure!(self.drafts.len()<=128&&self.pending.len()<=20,crate::i18n::fmt("Limite de rascunhos excedido.", &[]));
        let mut ids=std::collections::HashSet::new();
        for pending in &self.pending{anyhow::ensure!(ids.insert(pending.id),crate::i18n::fmt("Envio pendente repetido.", &[]));}
        for (channel,draft) in self.drafts.iter().map(|(c,d)|(*c,d)).chain(self.pending.iter().map(|p|(p.channel,&p.draft))){
            anyhow::ensure!(draft.text.chars().count()<=65536&&draft.files.len()<=10&&draft.missing_attachments<=10,crate::i18n::fmt("Rascunho grande demais.", &[]));
            anyhow::ensure!(draft.reply.as_ref().is_none_or(|r|r.channel==channel),crate::i18n::fmt("Resposta de outro canal.", &[]));
            crate::models::validate_embeds(&draft.embeds)?;
            anyhow::ensure!(draft.files.iter().all(|f|f.size<=crate::api::features::MAX_FILE&&f.path.as_os_str().len()<=4096&&f.name.len()<=4096),crate::i18n::fmt("Referência de arquivo inválida.", &[]));
        }
        Ok(())
    }
}

pub fn path(account:&AccountId)->Result<PathBuf>{
    Ok(crate::config::AppConfig::path()?.parent().context(crate::i18n::tr("Diretório de configuração indisponível."))?.join("drafts").join(format!("{}.json",account.namespace())))
}
pub fn load_at(path:&Path,account:&AccountId)->Result<Snapshot>{
    use std::io::Read;
    let file=match std::fs::File::open(path){Ok(f)=>f,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Snapshot::empty(account.clone())),Err(e)=>return Err(e.into())};
    let mut bytes=Vec::new();file.take((LIMIT+1) as u64).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len()<=LIMIT,crate::i18n::fmt("Arquivo de rascunhos grande demais.", &[]));
    let snapshot:Snapshot=serde_json::from_slice(&bytes)?;snapshot.validate(account)?;Ok(snapshot)
}
fn save_at(path:&Path,snapshot:&Snapshot)->Result<()>{
    snapshot.validate(&snapshot.account)?;let bytes=serde_json::to_vec(snapshot)?;
    anyhow::ensure!(bytes.len()<=LIMIT,crate::i18n::fmt("Limite de armazenamento dos rascunhos excedido.", &[]));
    let parent=path.parent().context(crate::i18n::tr("Diretório de configuração indisponível."))?;
    let mut directory=std::fs::DirBuilder::new();directory.recursive(true);
    #[cfg(unix)]{use std::os::unix::fs::DirBuilderExt;directory.mode(0o700);}
    directory.create(parent)?;
    crate::platform::files::atomic_write(path,&bytes)
}

// EN-US: A superseded account writer cannot overwrite a newer login's snapshot.
// PT-BR: Um escritor substituído não pode sobrescrever o snapshot de um login mais recente.
struct Registered {token:Uuid,latest:Option<Snapshot>,gate:std::sync::Arc<std::sync::Mutex<()>>}
static WRITERS:std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<PathBuf,Registered>>>=std::sync::LazyLock::new(Default::default);
pub struct Writer {sender:tokio::sync::watch::Sender<Option<Snapshot>>,path:PathBuf,token:Uuid}

// EN-US: A quick logout/login must see the latest debounce buffer before its replacement writer starts.
// PT-BR: Login após logout imediato lê o buffer mais recente antes de iniciar o escritor substituto.
pub fn restore_at(path:&Path,account:&AccountId)->Result<Snapshot>{
    if let Some(snapshot)=WRITERS.lock().unwrap().get(path).and_then(|w|w.latest.clone()){
        snapshot.validate(account)?;return Ok(snapshot);
    }
    load_at(path,account)
}
// EN-US: Normal application exit flushes pending buffers before shutting down Tokio.
// PT-BR: A saída normal grava os buffers pendentes antes de encerrar o Tokio.
fn save_registered(path:&Path,token:Uuid)->Result<(),String>{
    let gate=WRITERS.lock().unwrap().get(path).filter(|w|w.token==token).map(|w|w.gate.clone());
    let Some(gate)=gate else{return Ok(());};let _saving=gate.lock().unwrap();
    let snapshot=WRITERS.lock().unwrap().get(path).filter(|w|w.token==token).and_then(|w|w.latest.clone());
    // EN-US: The GTK-side registry lock never spans file writes/fsync; only blocking workers take this gate.
    // PT-BR: A trava usada pelo GTK nunca cobre escrita/fsync; só tarefas de disco aguardam esta trava por arquivo.
    if let Some(snapshot)=snapshot{save_at(path,&snapshot).map_err(|_|crate::i18n::tr("Não foi possível salvar o rascunho neste dispositivo.").to_owned())?;}
    Ok(())
}
pub async fn flush_pending(){
    let _=tokio::task::spawn_blocking(||{
        let writers:Vec<_>=WRITERS.lock().unwrap().iter().map(|(path,w)|(path.clone(),w.token)).collect();
        for (path,token) in writers{let _=save_registered(&path,token);}
    }).await;
}
impl Writer {
    pub fn new(path:PathBuf,notify:impl Fn(Result<(),String>)+Send+'static)->Self{
        let token=Uuid::new_v4();{
            let mut writers=WRITERS.lock().unwrap();let gate=writers.get(&path).map(|w|w.gate.clone()).unwrap_or_default();
            writers.insert(path.clone(),Registered{token,latest:None,gate});
        }
        let writer_path=path.clone();
        let (sender,mut receiver)=tokio::sync::watch::channel(None::<Snapshot>);
        tokio::spawn(async move{
            while receiver.changed().await.is_ok(){
                let closing=loop{
                    tokio::select!{_=tokio::time::sleep(std::time::Duration::from_millis(300))=>break false,
                        changed=receiver.changed()=>if changed.is_err(){break true;}}
                };
                let latest=receiver.borrow_and_update().clone();
                if latest.is_some(){
                    let path=path.clone();
                    let result=tokio::task::spawn_blocking(move||save_registered(&path,token)).await.unwrap_or_else(|_|Err(crate::i18n::tr("Não foi possível salvar o rascunho neste dispositivo.").into()));
                    notify(result);
                }
                if closing{break;}
            }
            let mut owners=WRITERS.lock().unwrap();if owners.get(&path).is_some_and(|w|w.token==token){owners.remove(&path);}
        });
        Self{sender,path:writer_path,token}
    }
    pub fn queue(&self,snapshot:Snapshot){
        if let Some(writer)=WRITERS.lock().unwrap().get_mut(&self.path).filter(|w|w.token==self.token){writer.latest=Some(snapshot.clone());}
        let _=self.sender.send(Some(snapshot));
    }
}

#[cfg(test)]mod tests{
    use super::*;
    fn owner(server:&str)->AccountId{AccountId::new(&crate::api::ApiClient::new(server).unwrap(),Uuid::from_u128(1))}
    #[tokio::test]async fn round_trip_scope_unavailable_files_and_private_permissions(){
        let root=std::env::temp_dir().join(format!("papo-drafts-{}",Uuid::new_v4()));let path=root.join("drafts.json");
        let a=owner("https://a.test/api");let channel=Uuid::new_v4();let mut snapshot=Snapshot::empty(a.clone());
        let mut draft=SavedDraft{text:"á é @mention(<@12345678-1234-4234-8234-123456789abc>)".into(),..Default::default()};
        draft.files.push(FileRef{path:root.join("gone.png"),name:"gone.png".into(),size:1,temporary:false});snapshot.drafts.insert(channel,draft);
        save_at(&path,&snapshot).unwrap();assert_eq!(load_at(&path,&a).unwrap(),snapshot);
        assert!(load_at(&path,&owner("https://b.test/api")).is_err());
        let restored=snapshot.drafts.remove(&channel).unwrap().restore().await;assert!(restored.text.starts_with("á é"));assert!(restored.files.is_empty()&&restored.recovery_notice.is_some());
        #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode()&0o777,0o600);}
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]async fn debounce_flushes_latest_snapshot_when_writer_closes(){
        let root=std::env::temp_dir().join(format!("papo-draft-writer-{}",Uuid::new_v4()));let path=root.join("drafts.json");let account=owner("https://a.test");
        let (tx,mut rx)=tokio::sync::mpsc::unbounded_channel();let writer=Writer::new(path.clone(),move|result|{let _=tx.send(result);});
        let mut snapshot=Snapshot::empty(account.clone());let id=Uuid::new_v4();snapshot.drafts.insert(id,SavedDraft{text:"before".into(),..Default::default()});writer.queue(snapshot.clone());
        snapshot.drafts.get_mut(&id).unwrap().text="latest".into();writer.queue(snapshot.clone());drop(writer);
        tokio::time::timeout(std::time::Duration::from_secs(2),rx.recv()).await.unwrap().unwrap().unwrap();
        assert_eq!(load_at(&path,&account).unwrap(),snapshot);std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]async fn quick_relogin_and_normal_exit_keep_the_newest_debounced_edit(){
        let root=std::env::temp_dir().join(format!("papo-draft-exit-{}",Uuid::new_v4()));let path=root.join("drafts.json");let account=owner("https://exit.test");
        let old=Writer::new(path.clone(),|r|r.unwrap());let mut snapshot=Snapshot::empty(account.clone());let id=Uuid::new_v4();snapshot.drafts.insert(id,SavedDraft{text:"before".into(),..Default::default()});old.queue(snapshot.clone());
        assert_eq!(restore_at(&path,&account).unwrap(),snapshot);
        let (tx,mut rx)=tokio::sync::mpsc::unbounded_channel();let new=Writer::new(path.clone(),move|r|{let _=tx.send(r);});snapshot.drafts.get_mut(&id).unwrap().text="after".into();new.queue(snapshot.clone());
        old.queue(Snapshot::empty(account.clone()));drop(old);flush_pending().await;assert_eq!(load_at(&path,&account).unwrap(),snapshot);
        drop(new);tokio::time::timeout(std::time::Duration::from_secs(2),rx.recv()).await.unwrap().unwrap().unwrap();assert_eq!(load_at(&path,&account).unwrap(),snapshot);std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn corrupt_or_oversized_data_is_never_treated_as_a_valid_empty_store(){
        let root=std::env::temp_dir().join(format!("papo-draft-invalid-{}",Uuid::new_v4()));std::fs::create_dir(&root).unwrap();let path=root.join("drafts.json");let account=owner("https://a.test");
        std::fs::write(&path,b"{invalid").unwrap();assert!(load_at(&path,&account).is_err());
        std::fs::write(&path,vec![b' ';LIMIT+1]).unwrap();assert!(load_at(&path,&account).is_err());std::fs::remove_dir_all(root).unwrap();
    }
}
