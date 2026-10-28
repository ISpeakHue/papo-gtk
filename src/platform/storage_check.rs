//! EN-US: Explicit disposable Windows storage acceptance, never ordinary startup or tests.
//! PT-BR: Aceitação Windows explícita e descartável, nunca executada no uso ou testes comuns.
use anyhow::{Context,Result};
use crate::{api::{ApiClient,features::{TemporaryFile,UploadFile,save_download}},config::AppConfig,session::Store};
use std::{path::{Path,PathBuf},sync::{Arc,atomic::{AtomicUsize,Ordering}},time::Duration};
use tokio::io::{AsyncReadExt,AsyncWriteExt};
use gtk::prelude::*;
use uuid::Uuid;
const USER:&str="papo-probe-áéç";
const SEED:&str="papo-disposable-storage-seed";
const ROTATED:&str="papo-disposable-storage-rotated";
fn key(api:&ApiClient)->String{format!("{}\n{USER}",api.base_url())}
fn token(store:Store,api:&ApiClient)->Result<Option<String>>{
    store.read(&key(api))?.map(|s|serde_json::from_str::<serde_json::Value>(&s).map(|v|v["auth"].as_str().unwrap_or("").to_owned()).map_err(Into::into)).transpose()
}
struct CredentialCleanup{store:Store,key:String}
impl Drop for CredentialCleanup{fn drop(&mut self){let _=self.store.write(&self.key,None);}}
// EN-US: Raw legacy fixtures also need cleanup when their intentionally invalid payload cannot prove ownership.
// PT-BR: Testes antigos brutos precisam de limpeza quando a carga propositalmente inválida não comprova posse.
struct LegacyCleanup{service:String,key:String}
impl Drop for LegacyCleanup{fn drop(&mut self){if let Ok(entry)=keyring::Entry::new(&self.service,&self.key){let _=entry.delete_credential();}}}
struct Scratch(PathBuf);
impl Drop for Scratch{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}

// EN-US: Children accept only a random probe namespace and loopback port, never production credentials.
// PT-BR: Filhos aceitam apenas namespace aleatório de teste e porta local, nunca credenciais reais.
pub async fn child(args:&[String])->Result<()>{
    anyhow::ensure!(args.len()==3,"Expected probe ID, port and phase");
    let id=Uuid::parse_str(&args[0])?;anyhow::ensure!(!id.is_nil()&&id.to_string()==args[0],"Invalid probe ID");
    let port:u16=args[1].parse()?;anyhow::ensure!(port!=0,"Invalid loopback port");
    let store=Store::probe(id);let url=format!("http://127.0.0.1:{port}");let api=ApiClient::new(&url)?;
    match args[2].as_str(){
        "save"=>{api.restore_auth_cookie(SEED)?;store.save(&api,USER).await?;anyhow::ensure!(token(store,&api)?.as_deref()==Some(SEED),"Save failed");},
        "resume"=>{
            let before:serde_json::Value=serde_json::from_str(&store.read(&key(&api))?.context("Missing remembered session")?)?;
            let profile=store.resume(&api,USER).await?.context("Resume failed")?;
            anyhow::ensure!(profile.username==USER&&api.auth_cookie().as_deref()==Some(ROTATED)&&token(store,&api)?.as_deref()==Some(ROTATED),"Resume/rotation failed");
            anyhow::ensure!(before["owner"].as_str()==Some(api.session_owner().to_string().as_str()),"Session owner changed");
        },
        "logout"=>{
            let saved:serde_json::Value=serde_json::from_str(&store.read(&key(&api))?.context("Missing remembered session")?)?;
            api.adopt_session_owner(Uuid::parse_str(saved["owner"].as_str().context("Owner missing")?)?);
            api.restore_auth_cookie(saved["auth"].as_str().context("Cookie missing")?)?;store.forget(&api,USER).await?;
            anyhow::ensure!(token(store,&api)?.is_none(),"Logout retained credential");
        },
        "empty"=>{anyhow::ensure!(store.resume(&api,USER).await?.is_none()&&token(store,&api)?.is_none(),"Forgotten account returned");},
        "revoked"=>{anyhow::ensure!(store.resume(&api,USER).await?.is_none()&&token(store,&api)?.is_none(),"Revocation retained credential");},
        "offline"=>{anyhow::ensure!(store.resume(&api,USER).await.is_err()&&token(store,&api)?.as_deref()==Some(SEED),"Transient failure discarded session");store.forget_account(&api,USER).await?;},
        "ownership"=>{
            api.restore_auth_cookie(SEED)?;store.save(&api,USER).await?;
            let newer=ApiClient::new(&url)?;newer.restore_auth_cookie(ROTATED)?;store.save(&newer,USER).await?;
            store.forget(&api,USER).await?;store.rotated(&api,USER,SEED).await?;anyhow::ensure!(token(store,&api)?.as_deref()==Some(ROTATED),"Stale operation replaced newer login");
            store.forget(&newer,USER).await?;store.rotated(&newer,USER,ROTATED).await?;anyhow::ensure!(token(store,&api)?.is_none(),"Rotation resurrected logout");
            store.save(&api,USER).await?;store.forget_account(&api,USER).await?;anyhow::ensure!(token(store,&api)?.is_none(),"Remember opt-out failed");
        },
        "unavailable"=>{
            api.restore_auth_cookie(SEED)?;let unavailable=Store::unavailable();
            anyhow::ensure!(unavailable.save(&api,USER).await.is_err()&&unavailable.resume(&api,USER).await.is_err()&&unavailable.forget_account(&api,USER).await.is_err(),"Store failure did not propagate");
            anyhow::ensure!(token(store,&api)?.is_none(),"Unavailable store created a credential");
        },
        _=>anyhow::bail!("Unknown storage probe phase")
    }
    println!("{}",serde_json::json!({"phase":args[2],"status":"pass"}));Ok(())
}

async fn credentials()->Result<serde_json::Value>{
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await?;let port=listener.local_addr()?.port();
    let api=ApiClient::new(&format!("http://127.0.0.1:{port}"))?;let id=Uuid::new_v4();let store=Store::probe(id);
    let _cleanup=CredentialCleanup{store,key:key(&api)};
    // EN-US: Verify migration from earlier UTF-16 keyring entries and the UTF-8 blob budget.
    // PT-BR: Verifica migração de entradas UTF-16 antigas e o limite de blobs UTF-8.
    let service=format!("br.com.papo.gtk.session.v1.test.{id}");let entry=keyring::Entry::new(&service,&key(&api))?;
    let _legacy=LegacyCleanup{service:service.clone(),key:key(&api)};
    let legacy=serde_json::json!({"version":1,"base":api.base_url(),"username":USER,"auth":"disposable legacy áéç"}).to_string();
    entry.set_password(&legacy)?;anyhow::ensure!(store.read(&key(&api))?.as_deref()==Some(legacy.as_str()),"Legacy credential read failed");
    let large=format!("{}áéç","x".repeat(1800));store.write(&key(&api),Some(large.as_str()))?;anyhow::ensure!(store.read(&key(&api))?.as_deref()==Some(large.as_str()),"UTF-8 credential read failed");
    anyhow::ensure!(matches!(entry.get_password(),Err(keyring::Error::NoEntry)),"Legacy entry was not migrated");
    let rejected=store.write(&key(&api),Some(&"x".repeat(10000))).err().context("Oversized Windows credential was accepted")?;
    anyhow::ensure!(!rejected.to_string().contains(&"x".repeat(20))&&store.read(&key(&api))?.as_deref()==Some(large.as_str()),"Rejected credential leaked or replaced data");store.write(&key(&api),None)?;
    // EN-US: WinCred folds target case; guarded legacy lookup and hashed targets must preserve both accounts.
    // PT-BR: WinCred ignora caixa; a busca antiga protegida e os hashes devem preservar ambas as contas.
    let upper=format!("{}\nCaseUser",api.base_url());let lower=format!("{}\ncaseuser",api.base_url());
    let _upper=CredentialCleanup{store,key:upper.clone()};let _lower=CredentialCleanup{store,key:lower.clone()};
    let old=keyring::Entry::new(&service,&upper)?;
    let old_value=serde_json::json!({"version":1,"base":api.base_url(),"username":"CaseUser","auth":"disposable case fixture"}).to_string();old.set_password(&old_value)?;
    anyhow::ensure!(store.read(&lower)?.is_none(),"Legacy lookup crossed account case");store.write(&lower,None)?;
    anyhow::ensure!(old.get_password()?==old_value,"Legacy deletion crossed account case");
    store.write(&lower,Some("lower fixture"))?;store.write(&upper,Some("upper fixture"))?;
    anyhow::ensure!(store.read(&lower)?.as_deref()==Some("lower fixture")&&store.read(&upper)?.as_deref()==Some("upper fixture"),"Hashed account targets collided");
    store.write(&upper,None)?;anyhow::ensure!(store.read(&lower)?.as_deref()==Some("lower fixture"),"Deletion crossed hashed accounts");store.write(&lower,None)?;
    let long_key=format!("{}{}\nLongUser",api.base_url(),"x".repeat(1000));let _long=CredentialCleanup{store,key:long_key.clone()};
    store.write(&long_key,Some("long key fixture"))?;anyhow::ensure!(store.read(&long_key)?.as_deref()==Some("long key fixture"),"Long credential key failed");store.write(&long_key,None)?;
    entry.set_secret(b"PAPO-UTF8\0\xff")?;anyhow::ensure!(store.read(&key(&api))?.is_none(),"Malformed legacy value was returned");
    store.write(&key(&api),Some("valid fixture"))?;store.write(&key(&api),None)?;anyhow::ensure!(store.read(&key(&api))?.is_none(),"Malformed legacy value blocked logout");entry.delete_credential()?;
    let mode=Arc::new(AtomicUsize::new(0));let control=mode.clone();let count=Arc::new(AtomicUsize::new(0));let requests=count.clone();
    let server=tokio::spawn(async move{
        loop{let(mut socket,_)=listener.accept().await?;let mut request=Vec::new();
            loop{let mut bytes=[0;1024];let n=socket.read(&mut bytes).await?;anyhow::ensure!(n>0&&request.len()+n<=16384,"Invalid fixture request");request.extend_from_slice(&bytes[..n]);if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}}
            let request=String::from_utf8(request)?;let refresh=request.starts_with("POST /auth/refresh HTTP/1.1");
            anyhow::ensure!(refresh||request.starts_with("GET /auth/whoami HTTP/1.1"),"Unexpected fixture endpoint");
            anyhow::ensure!(request.to_ascii_lowercase().contains(&format!("cookie: auth={SEED}")),"Missing synthetic cookie");requests.fetch_add(1,Ordering::SeqCst);
            let status=match control.load(Ordering::SeqCst){1=>401,2 if refresh=>401,3=>503,_=>200};
            let body=if status!=200{serde_json::json!({"title":"Synthetic storage fixture error"})}else if refresh{serde_json::json!({"connection":{"id":Uuid::new_v4(),"created_at":"2026-10-22T12:00:00Z","expires_at":"2099-10-23T12:00:00Z"}})}else{serde_json::json!({"id":"b66883dd-64d4-4321-a74e-7acee60fe4bc","username":USER,"created_at":"2026-10-22T12:00:00Z"})}.to_string();
            let cookie=if refresh&&status==200{format!("Set-Cookie: Auth={ROTATED}; HttpOnly; Path=/\r\n")}else{String::new()};
            socket.write_all(format!("HTTP/1.1 {status} fixture\r\nContent-Type: application/json\r\n{cookie}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await?;
        }
        #[allow(unreachable_code)] Ok::<(),anyhow::Error>(())
    });
    struct Stop(tokio::task::JoinHandle<Result<()>>);impl Drop for Stop{fn drop(&mut self){self.0.abort();}}
    let _server=Stop(server);let mut phases=Vec::new();
    for (phase,state) in [("save",0),("resume",0),("logout",0),("empty",0),("save",0),("revoked",1),("empty",0),("save",0),("revoked",2),("empty",0),("save",0),("offline",3),("ownership",0),("unavailable",0),("empty",0)]{
        mode.store(state,Ordering::SeqCst);
        let mut command=tokio::process::Command::new(std::env::current_exe()?);
        command.args(["--windows-storage-child",&id.to_string(),&port.to_string(),phase]).creation_flags(0x08000000).kill_on_drop(true);
        let output=tokio::time::timeout(Duration::from_secs(30),command.output()).await.context("Storage child timed out")??;
        anyhow::ensure!(output.status.success(),"Storage child failed: {phase}");
        let result:serde_json::Value=serde_json::from_slice(&output.stdout)?;anyhow::ensure!(result["status"]=="pass","Storage child assertion failed");phases.push(result);
    }
    anyhow::ensure!(count.load(Ordering::SeqCst)==6,"Unexpected auth request count");
    store.write(&key(&api),None)?;anyhow::ensure!(store.read(&key(&api))?.is_none(),"Probe cleanup failed");
    Ok(serde_json::json!({"status":"pass","probe_id":id,"phases":phases,"requests":count.load(Ordering::SeqCst),"legacy_utf16":true,"legacy_case_guard":true,"malformed_legacy_ignored":true,"exact_account_targets":true,"long_credential_key":true,"utf8_blob":true,"oversized_rejected":true,"unavailable_injected":true,"removed":true}))
}

fn temporary(path:&Path,bytes:&[u8])->Result<TemporaryFile>{std::fs::write(path,bytes)?;Ok(TemporaryFile(path.to_owned()))}
async fn files(root:&Path)->Result<serde_json::Value>{
    let config=AppConfig::path()?;let expected=dirs::config_dir().context("Config root missing")?.join("papo-gtk/config.toml");anyhow::ensure!(config==expected,"Incorrect native config location");
    let cache=dirs::cache_dir().context("Cache root missing")?;anyhow::ensure!(cache==dirs::data_local_dir().context("Local app data missing")?,"Incorrect Windows cache location");
    let path=root.join("config.toml");let config=AppConfig{server_url:Some("http://localhost:8080".into()),last_username:Some(USER.into()),remember_session:Some(true),..Default::default()};
    config.save_at(&path)?;anyhow::ensure!(AppConfig::load_at(&path).last_username.as_deref()==Some(USER),"Config round trip failed");
    let before=std::fs::read(&path)?;
    let lock=std::fs::OpenOptions::new().read(true).share_mode(0).open(&path)?;
    anyhow::ensure!(config.save_at(&path).is_err(),"Locked config replacement succeeded");drop(lock);anyhow::ensure!(std::fs::read(&path)?==before,"Locked config was damaged");
    let mut long=root.to_owned();for _ in 0..7{long=long.join("áéç pasta de teste com nome longo");}std::fs::create_dir_all(&long)?;
    let destination=long.join("imagem á é ç.png");anyhow::ensure!(destination.as_os_str().encode_wide().count()>260,"Fixture path is not long");
    let bytes=include_bytes!("../../tests/fixtures/animated-emoji.png");
    let source=root.join("download-source");save_download(temporary(&source,bytes)?,destination.clone()).await?;anyhow::ensure!(!source.exists()&&std::fs::read(&destination)?==bytes,"Long Unicode download failed");
    save_download(temporary(&source,b"replacement")?,destination.clone()).await?;anyhow::ensure!(std::fs::read(&destination)?==b"replacement","Download overwrite failed");
    let lock=std::fs::OpenOptions::new().read(true).share_mode(0).open(&destination)?;
    anyhow::ensure!(save_download(temporary(&source,b"blocked")?,destination.clone()).await.is_err(),"Locked download replaced");drop(lock);anyhow::ensure!(!source.exists()&&std::fs::read(&destination)?==b"replacement","Locked download lost data");
    for name in ["CON.txt","NUL","bad:stream","COM¹.png","name.","name "]{
        anyhow::ensure!(save_download(temporary(&source,b"rejected")?,root.join(name)).await.is_err()&&!source.exists(),"Reserved destination accepted");
        let safe=super::files::suggested_filename(name);save_download(temporary(&source,b"safe")?,root.join(safe)).await?;
    }
    // EN-US: Cancel while copying, then require both temporary files gone and the old destination intact.
    // PT-BR: Cancela durante a cópia e exige limpeza dos temporários e preservação do destino anterior.
    let input=std::fs::File::create(&source)?;input.set_len(128<<20)?;drop(input);
    let held=TemporaryFile(source.clone());let target=destination.clone();let task=tokio::spawn(async move{save_download(held,target).await});
    let mut saw_stage=false;
    for _ in 0..1000{
        if std::fs::read_dir(&long)?.filter_map(|r|r.ok()).any(|entry|entry.file_name().to_string_lossy().starts_with(".papo-download-")){saw_stage=true;break;}
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    anyhow::ensure!(saw_stage,"Copy stage was not observed");task.abort();let _=task.await;
    for _ in 0..100{if !source.exists()&&!std::fs::read_dir(&long)?.filter_map(|r|r.ok()).any(|entry|entry.file_name().to_string_lossy().starts_with(".papo-download-")){break;}tokio::time::sleep(Duration::from_millis(10)).await;}
    anyhow::ensure!(!source.exists()&&std::fs::read(&destination)?==b"replacement","Cancelled save changed destination or leaked input");
    anyhow::ensure!(!std::fs::read_dir(&long)?.filter_map(|r|r.ok()).any(|entry|entry.file_name().to_string_lossy().starts_with(".papo-download-")),"Cancelled stage leaked");
    let clipboard=UploadFile::clipboard_png(bytes).await?;let path=clipboard.path.clone();let owner=clipboard.clone();drop(clipboard);anyhow::ensure!(path.exists(),"Draft owner lost staged clipboard");drop(owner);anyhow::ensure!(!path.exists(),"Clipboard temporary leaked");
    anyhow::ensure!(!String::from_utf8_lossy(&std::fs::read(root.join("config.toml"))?).contains(SEED),"Credential leaked into config");
    Ok(serde_json::json!({"status":"pass","config":"RoamingAppData/papo-gtk/config.toml","cache":"LocalAppData","long_path_units":destination.as_os_str().encode_wide().count(),"unicode":true,"overwrite":true,"locked_config":true,"locked_download":true,"reserved_names":true,"cancel_copy_cleanup":true,"clipboard_lifetime":true}))
}
use std::os::windows::{fs::OpenOptionsExt,ffi::OsStrExt};

fn desktop(rt:&tokio::runtime::Runtime,root:&Path)->Result<serde_json::Value>{
    gtk::init()?;let context=gtk::glib::MainContext::default();let _guard=rt.enter();
    let window=gtk::Window::builder().title("Papo — storage acceptance").default_width(420).default_height(180).build();window.present();
    let png=include_bytes!("../../tests/fixtures/animated-emoji.png");let texture=gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_static(png))?;
    let clipboard=window.clipboard();clipboard.set_texture(&texture);
    let read=context.block_on(async{let timeout=gtk::glib::timeout_future_seconds(10);let read=clipboard.read_texture_future();futures_util::pin_mut!(timeout,read);match futures_util::future::select(read,timeout).await{futures_util::future::Either::Left((result,_))=>result.map_err(Into::into),_=>anyhow::bail!("Clipboard timed out")}})?.context("Clipboard image missing")?;
    anyhow::ensure!(read.width()==texture.width()&&read.height()==texture.height(),"Clipboard texture changed");
    let result=std::rc::Rc::new(std::cell::RefCell::new(None));let answer=result.clone();let cancellable=gtk::gio::Cancellable::new();
    let dialog=gtk::FileDialog::builder().title("Papo — cancel save test").initial_folder(&gtk::gio::File::for_path(root)).initial_name("cancelled.png").build();
    dialog.save(Some(&window),Some(&cancellable),move|value|{answer.replace(Some(value));});
    gtk::glib::timeout_add_local_once(Duration::from_millis(700),move||{cancellable.cancel();});
    let deadline=std::time::Instant::now()+Duration::from_secs(10);
    while result.borrow().is_none()&&std::time::Instant::now()<deadline{while context.pending(){context.iteration(false);}std::thread::sleep(Duration::from_millis(10));}
    let answer=result.borrow_mut().take().context("Save dialog cancellation timed out")?;
    anyhow::ensure!(answer.as_ref().err().is_some_and(|e|e.matches(gtk::DialogError::Cancelled))&&!root.join("cancelled.png").exists(),"Save dialog did not report cancellation");
    clipboard.set_text("");window.close();while context.pending(){context.iteration(false);}
    Ok(serde_json::json!({"status":"pass","native_clipboard_texture":true,"native_save_cancel":true}))
}

pub fn run(rt:&tokio::runtime::Runtime)->Result<()>{
    let root=std::env::temp_dir().join(format!("Papo storage áéç {}",Uuid::new_v4()));std::fs::create_dir(&root)?;let scratch=Scratch(root);
    let credentials=rt.block_on(credentials())?;let files=rt.block_on(files(&scratch.0))?;let desktop=desktop(rt,&scratch.0)?;
    std::fs::remove_dir_all(&scratch.0)?;anyhow::ensure!(!scratch.0.exists(),"Scratch cleanup failed");
    println!("{}",serde_json::json!({"schema":1,"status":"pass","credentials":credentials,"files":files,"desktop":desktop,"scratch_removed":true}));Ok(())
}
