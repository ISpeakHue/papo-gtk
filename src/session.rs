//! Authentication cookies live in the desktop keyring, never application files.
use crate::{api::ApiClient,models::WhoamiResponse};
use anyhow::{Result,anyhow};
use serde::{Serialize,Deserialize};
const SERVICE:&str="br.com.papo.gtk.session.v1";
static STORAGE:std::sync::Mutex<()>=std::sync::Mutex::new(());
#[derive(Serialize,Deserialize)]
struct SavedSession{version:u8,base:String,username:String,auth:String}
fn account(base:&str,user:&str)->String{format!("{base}\n{user}")}

#[cfg(not(test))]
fn read(key:&str)->Result<Option<String>>{
    let entry=keyring::Entry::new(SERVICE,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match entry.get_password(){Ok(value)=>Ok(Some(value)),Err(keyring::Error::NoEntry)=>Ok(None),Err(_)=>Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))}
}
#[cfg(not(test))]
fn write(key:&str,value:Option<&str>)->Result<()>{
    let entry=keyring::Entry::new(SERVICE,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match value{Some(value)=>entry.set_password(value).map_err(|_|anyhow!("Não foi possível salvar a sessão no chaveiro.")),None=>match entry.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(anyhow!("Não foi possível remover a sessão do chaveiro."))}}
}
// UI/protocol tests must never read a real user's credentials or modify their keyring.
#[cfg(test)]static MEMORY:std::sync::Mutex<Option<std::collections::HashMap<String,String>>>=std::sync::Mutex::new(None);
#[cfg(test)]fn read(key:&str)->Result<Option<String>>{Ok(MEMORY.lock().unwrap().as_ref().and_then(|m|m.get(key).cloned()))}
#[cfg(test)]fn write(key:&str,value:Option<&str>)->Result<()>{let mut memory=MEMORY.lock().unwrap();let map=memory.get_or_insert_with(Default::default);if let Some(value)=value{map.insert(key.into(),value.into());}else{map.remove(key);}Ok(())}

pub async fn save(api:&ApiClient,user:&str)->Result<()>{
    let Some(auth)=api.auth_cookie()else{return Ok(());};
    let saved=SavedSession{version:1,base:api.base_url().into(),username:user.into(),auth};
    let key=account(&saved.base,user);let encoded=serde_json::to_string(&saved).map_err(|_|anyhow!("Não foi possível preparar a sessão."))?;
    tokio::task::spawn_blocking(move ||{let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;write(&key,Some(&encoded))}).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}
pub async fn forget(api:&ApiClient,user:&str)->Result<()>{
    let key=account(api.base_url(),user);tokio::task::spawn_blocking(move ||{let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;write(&key,None)}).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}
/// Keep the remembered cookie in sync with refresh rotation. A late refresh
/// must not recreate a logged-out credential or overwrite a newer login.
pub async fn rotated(api:&ApiClient,user:&str,previous:&str)->Result<()>{
    let Some(auth)=api.auth_cookie()else{return Ok(());};let base=api.base_url().to_owned();let user=user.to_owned();let key=account(&base,&user);let previous=previous.to_owned();
    tokio::task::spawn_blocking(move ||{
        let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;
        let Some(encoded)=read(&key)?else{return Ok(());};let Ok(mut saved)=serde_json::from_str::<SavedSession>(&encoded)else{return Ok(());};
        if saved.version!=1||saved.base!=base||saved.username!=user||saved.auth!=previous{return Ok(());}
        saved.auth=auth;let encoded=serde_json::to_string(&saved).map_err(|_|anyhow!("Não foi possível preparar a sessão."))?;write(&key,Some(&encoded))
    }).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}
pub async fn resume(api:&ApiClient,user:&str)->Result<Option<WhoamiResponse>>{
    let key=account(api.base_url(),user);let copy=key.clone();
    let Some(encoded)=tokio::task::spawn_blocking(move ||read(&copy)).await.map_err(|_|anyhow!("Chaveiro indisponível."))?? else{return Ok(None);};
    let saved=serde_json::from_str::<SavedSession>(&encoded).ok().filter(|s|s.version==1&&s.base==api.base_url()&&s.username==user);
    let Some(saved)=saved else{forget(api,user).await?;return Ok(None);};
    if api.restore_auth_cookie(&saved.auth).is_err(){forget(api,user).await?;return Ok(None);}
    match api.whoami().await{
        Ok(profile) if profile.username==user=>{
            // A remembered cookie may be close to expiry. Refresh before the
            // first WebSocket handshake so the regular 12-hour timer is safe.
            match api.refresh().await{
                Ok(_)=>{if rotated(api,user,&saved.auth).await.is_err(){tracing::warn!("Could not update remembered session in desktop keyring");}Ok(Some(profile))},
                Err(error) if crate::api::is_session_error(&error)=>{forget(api,user).await?;Ok(None)},
                Err(_)=>Err(anyhow!("Não foi possível renovar a sessão salva. Verifique a conexão e tente novamente.")),
            }
        },
        Ok(_)=>{forget(api,user).await?;Ok(None)},
        Err(error) if crate::api::is_session_error(&error)=>{forget(api,user).await?;Ok(None)},
        Err(_)=>Err(anyhow!("Não foi possível retomar a sessão salva. Verifique a conexão e tente novamente.")),
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn restored_cookies_require_safe_transport_and_never_appear_in_debug(){
        let api=ApiClient::new("https://example.test/api").unwrap();api.restore_auth_cookie("test-secret.jwt-value").unwrap();assert_eq!(api.auth_cookie().as_deref(),Some("test-secret.jwt-value"));assert!(!format!("{api:?}").contains("test-secret"));
        for token in ["","x; Domain=example.test","x\r\nAuthorization: bad"]{assert!(api.restore_auth_cookie(token).is_err());}
        assert!(ApiClient::new("http://remote.test").unwrap().restore_auth_cookie("secret").is_err());
        assert!(ApiClient::new("http://localhost:8080").unwrap().restore_auth_cookie("secret").is_ok());
    }
    #[tokio::test]async fn saved_sessions_are_scoped_and_logout_removes_them(){
        let host=format!("https://{}.test/api/",uuid::Uuid::new_v4());let api=ApiClient::new(&host).unwrap();api.restore_auth_cookie("saved-token").unwrap();save(&api,"Alice").await.unwrap();
        assert!(read(&account(api.base_url(),"Alice")).unwrap().is_some());assert!(read(&account(api.base_url(),"Bob")).unwrap().is_none());assert!(read(&account(&host.replace("/api/","/other/"),"Alice")).unwrap().is_none());forget(&api,"Alice").await.unwrap();assert!(read(&account(api.base_url(),"Alice")).unwrap().is_none());
    }
    #[tokio::test]async fn rotation_updates_only_the_remembered_session_and_cannot_resurrect_logout(){
        let api=ApiClient::new(&format!("https://{}.test",uuid::Uuid::new_v4())).unwrap();api.restore_auth_cookie("before-refresh").unwrap();save(&api,"Alice").await.unwrap();
        let token=||serde_json::from_str::<SavedSession>(&read(&account(api.base_url(),"Alice")).unwrap().unwrap()).unwrap().auth;
        api.restore_auth_cookie("after-refresh").unwrap();rotated(&api,"Alice","before-refresh").await.unwrap();assert_eq!(token(),"after-refresh");
        let newer=ApiClient::new(api.base_url()).unwrap();newer.restore_auth_cookie("new-login").unwrap();save(&newer,"Alice").await.unwrap();rotated(&api,"Alice","before-refresh").await.unwrap();assert_eq!(token(),"new-login");
        forget(&newer,"Alice").await.unwrap();rotated(&api,"Alice","before-refresh").await.unwrap();assert!(read(&account(api.base_url(),"Alice")).unwrap().is_none());
    }
    #[tokio::test]async fn reopen_rotates_the_existing_session_without_login_and_revocation_forgets_it(){
        use tokio::io::{AsyncReadExt,AsyncWriteExt};
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}",listener.local_addr().unwrap());let user=uuid::Uuid::new_v4().to_string();let name=user.clone();
        let server=tokio::spawn(async move{
            for (method,path,status,cookie) in [("GET","/auth/whoami",200,"resume-test"),("POST","/auth/refresh",200,"resume-test"),("GET","/auth/whoami",401,"resumed-cookie")]{let(mut socket,_)=listener.accept().await.unwrap();let mut request=vec![];loop{let mut chunk=[0;1024];let count=socket.read(&mut chunk).await.unwrap();if count==0{break;}request.extend_from_slice(&chunk[..count]);if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}}
                let request=String::from_utf8(request).unwrap();assert!(request.starts_with(&format!("{method} {path} HTTP/1.1")));assert!(request.to_ascii_lowercase().contains(&format!("cookie: auth={cookie}")));
                let body=if method=="POST"{serde_json::json!({"connection":{"id":uuid::Uuid::new_v4(),"created_at":"2026-10-07T12:00:00Z","expires_at":"2026-10-08T12:00:00Z"}})}else if status==200{serde_json::json!({"id":name,"username":name,"created_at":"2026-10-03T12:00:00Z"})}else{serde_json::json!({"type":"https://test/problems/unauthorized","title":"Expired"})}.to_string();
                let headers=if method=="POST"{"Set-Cookie: Auth=resumed-cookie; HttpOnly; Secure; Path=/\r\n"}else{""};
                socket.write_all(format!("HTTP/1.1 {status} response\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let api=ApiClient::new(&url).unwrap();api.restore_auth_cookie("resume-test").unwrap();save(&api,&user).await.unwrap();
        let reopened=ApiClient::new(&url).unwrap();assert_eq!(resume(&reopened,&user).await.unwrap().unwrap().username,user);
        assert_eq!(reopened.auth_cookie().as_deref(),Some("resumed-cookie"));assert_eq!(serde_json::from_str::<SavedSession>(&read(&account(api.base_url(),&user)).unwrap().unwrap()).unwrap().auth,"resumed-cookie");
        assert!(resume(&ApiClient::new(&url).unwrap(),&user).await.unwrap().is_none());server.await.unwrap();assert!(resume(&ApiClient::new(&url).unwrap(),&user).await.unwrap().is_none());
    }
    #[test]#[ignore="requires the desktop Secret Service; writes and removes a disposable secret"]
    fn desktop_keyring_round_trip(){
        let key=format!("test-{}",uuid::Uuid::new_v4());let entry=keyring::Entry::new(&format!("{SERVICE}.test"),&key).unwrap();
        struct Cleanup(keyring::Entry);impl Drop for Cleanup{fn drop(&mut self){let _=self.0.delete_credential();}}
        let entry=Cleanup(entry);entry.0.set_password("disposable-test-value").unwrap();assert_eq!(entry.0.get_password().unwrap(),"disposable-test-value");
    }
}
