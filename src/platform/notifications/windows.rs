//! EN-US: Use the packaged WinRT helper and per-user DPAPI storage; never discover executables through PATH.
//! PT-BR: Usa o auxiliar WinRT do pacote e armazenamento DPAPI do usuário, sem buscar executáveis no PATH.
use super::*;
use anyhow::{Context,Result};
use std::{process::Stdio,sync::OnceLock};
use tokio::io::AsyncWriteExt;

pub(super) fn crypt(data:&[u8],encrypt:bool)->Result<Vec<u8>>{
    use windows_sys::Win32::{Foundation::LocalFree,Security::Cryptography::*};
    unsafe{
        let input=CRYPT_INTEGER_BLOB{cbData:data.len().try_into()?,pbData:data.as_ptr() as _};
        let mut output=CRYPT_INTEGER_BLOB{cbData:0,pbData:std::ptr::null_mut()};
        let entropy=b"br.com.papo.gtk.notification-tickets.v1";
        let entropy=CRYPT_INTEGER_BLOB{cbData:entropy.len() as u32,pbData:entropy.as_ptr() as _};
        let ok=if encrypt{CryptProtectData(&input,std::ptr::null(),&entropy,std::ptr::null(),std::ptr::null(),CRYPTPROTECT_UI_FORBIDDEN,&mut output)}
            else{CryptUnprotectData(&input,std::ptr::null_mut(),&entropy,std::ptr::null(),std::ptr::null(),CRYPTPROTECT_UI_FORBIDDEN,&mut output)};
        anyhow::ensure!(ok!=0,"Notification metadata protection failed");
        let bytes=std::slice::from_raw_parts(output.pbData,output.cbData as usize).to_vec();LocalFree(output.pbData.cast());Ok(bytes)
    }
}
struct Native {path:PathBuf,dry_run:bool}
impl Transport for Native{
    async fn perform(&mut self,operation:Operation)->Result<()>{
        let mut command=tokio::process::Command::new(&self.path);
        command.creation_flags(0x08000000).kill_on_drop(true).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
        let input=match operation{
            Operation::Ready=>{command.arg(if self.dry_run{"--self-check"}else{"--desktop-info"});String::new()},
            Operation::Show{id,group,xml}=>{command.args([if self.dry_run{"--check-notification"}else{"--show-notification"},&id.to_string(),&group]);xml},
            Operation::Remove{id,group}=>{if self.dry_run{return Ok(());}command.args(["--remove-notification",&id.to_string(),&group]);String::new()}
        };
        let mut child=command.spawn().context("Windows toast helper unavailable")?;
        let result=tokio::time::timeout(std::time::Duration::from_secs(5),async{
            let mut stdin=child.stdin.take().unwrap();stdin.write_all(input.as_bytes()).await?;drop(stdin);
            anyhow::ensure!(child.wait().await?.success(),"Windows toast operation failed");Ok::<_,anyhow::Error>(())
        }).await;
        match result{Ok(result)=>result,Err(_)=>{let _=child.kill().await;let _=child.wait().await;anyhow::bail!("Windows toast helper timed out")}}
    }
}
pub fn backend()->Option<Backend>{
    static BACKEND:OnceLock<Option<Backend>>=OnceLock::new();
    BACKEND.get_or_init(||{
        let mut session=0;
        if unsafe{windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId(std::process::id(),&mut session)}==0{return None;}
        let scope=session.to_string();
        let path=dirs::data_local_dir()?.join("papo-gtk").join(format!("notification-tickets-{scope}.dat"));
        let helper=std::env::current_exe().ok()?.parent()?.join("papo-shell.exe");
        if !helper.is_file(){return None;}
        Some(Backend::start(path,scope,Native{path:helper,dry_run:false}))
    }).clone()
}
pub fn forget(owner:Account){if let Some(backend)=backend(){tokio::spawn(async move{backend.forget(owner).await;});}}

// EN-US: Packaged acceptance uses real DPAPI/WinRT objects, with synthetic metadata and no visible alert.
// PT-BR: A aceitação do pacote usa objetos DPAPI/WinRT reais, com metadados sintéticos e sem avisos visíveis.
pub async fn check()->Result<()>{
    struct Scratch(PathBuf);impl Drop for Scratch{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let scratch=Scratch(std::env::temp_dir().join(format!("Papo-notifications-á-{}",Uuid::new_v4())));
    let path=scratch.0.join("tickets.dat");
    let helper=std::env::current_exe()?.parent().context("Package directory unavailable")?.join("papo-shell.exe");
    let backend=Backend::start(path.clone(),"acceptance".into(),Native{path:helper.clone(),dry_run:true});
    anyhow::ensure!(backend.ready().await,"Packaged WinRT helper check failed");
    let owner=Account{server:"https://fixture.invalid/api".into(),user:Uuid::new_v4(),session:Uuid::new_v4()};
    let target=Target{notice:Uuid::new_v4(),channel:Uuid::new_v4(),message:Uuid::new_v4()};
    anyhow::ensure!(backend.show(owner.clone(),target,"<prévia & teste>".into(),false),"Cannot queue synthetic notification");
    anyhow::ensure!(backend.ready().await,"Synthetic toast construction failed");
    let repository=Repository::open(path.clone())?;let id=*repository.registry.tickets.keys().next().context("Synthetic toast did not persist")?;
    let bytes=std::fs::read(&path)?;anyhow::ensure!(serde_json::from_slice::<Registry>(&bytes).is_err(),"Notification metadata was not encrypted");
    backend.stop().await;
    let cold=Backend::start(path,"acceptance".into(),Native{path:helper,dry_run:true});
    let wrong=Account{session:Uuid::new_v4(),..owner.clone()};
    anyhow::ensure!(cold.resolve(wrong,id,[target.channel].into()).await.is_none(),"Wrong session activated notification");
    anyhow::ensure!(cold.resolve(owner.clone(),id,[target.channel].into()).await==Some(target),"Cold activation did not resolve");
    anyhow::ensure!(cold.resolve(owner,id,[target.channel].into()).await.is_none(),"Activation ticket replayed");
    println!("{}",serde_json::json!({"schema":1,"status":"pass","encrypted_storage":true,"cold_activation":true,"winrt_toast":true,"desktop_delivery":false}));Ok(())
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn native_dpapi_roundtrip_and_tamper_rejection(){
        let clear=b"synthetic notification routing metadata";let protected=crypt(clear,true).unwrap();assert_ne!(protected,clear);assert_eq!(crypt(&protected,false).unwrap(),clear);
        let mut tampered=protected;let index=tampered.len()/2;tampered[index]^=0x80;assert!(crypt(&tampered,false).is_err());
    }
}
