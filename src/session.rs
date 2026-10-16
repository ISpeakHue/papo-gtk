//! Authentication cookies live in the desktop keyring, never application files.
//! EN: Secure desktop session management using the OS keyring (libsecret/KWallet). Never stores plain tokens on disk.
//! PT-BR: Gerenciamento seguro de sessão desktop usando o chaveiro do SO (libsecret/KWallet). Nunca armazena tokens em texto puro no disco.

use crate::{api::ApiClient,models::WhoamiResponse};
use anyhow::{Result,anyhow};
use serde::{Serialize,Deserialize};
pub mod refresh;

// EN: Desktop keyring service identifier.
// PT-BR: Identificador do serviço no chaveiro do desktop.
const SERVICE:&str="br.com.papo.gtk.session.v1";

// EN: Mutex to serialize access to the system keyring, preventing concurrent backend race conditions.
// PT-BR: Mutex para serializar o acesso ao chaveiro do sistema, prevenindo condições de corrida no backend.
static STORAGE:std::sync::Mutex<()>=std::sync::Mutex::new(());

// EN: Keyring session payload structure storing the auth token, user, base URL, and unique session owner.
// PT-BR: Estrutura da carga útil da sessão no chaveiro guardando o token de autenticação, usuário, URL base e dono único da sessão.
#[derive(Serialize,Deserialize)]
struct SavedSession{version:u8,base:String,username:String,auth:String,#[serde(default)]owner:Option<uuid::Uuid>}

// EN: Format compound keyring lookup key from server base URL and username.
// PT-BR: Formata a chave composta de busca no chaveiro a partir da URL base do servidor e nome de usuário.
fn account(base:&str,user:&str)->String{format!("{base}\n{user}")}

#[cfg(not(test))]
// EN: Read a credential from the OS keyring by key.
// PT-BR: Lê uma credencial do chaveiro do SO pela chave informada.
fn read(key:&str)->Result<Option<String>>{
    let entry=keyring::Entry::new(SERVICE,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match entry.get_password(){Ok(value)=>Ok(Some(value)),Err(keyring::Error::NoEntry)=>Ok(None),Err(_)=>Err(anyhow!("Não foi possível abrir a sessão no chaveiro."))}
}

#[cfg(not(test))]
// EN: Write or delete a credential in the OS keyring.
// PT-BR: Grava ou remove uma credencial no chaveiro do SO.
fn write(key:&str,value:Option<&str>)->Result<()>{
    let entry=keyring::Entry::new(SERVICE,key).map_err(|_|anyhow!("Chaveiro indisponível."))?;
    match value{Some(value)=>entry.set_password(value).map_err(|_|anyhow!("Não foi possível salvar a sessão no chaveiro.")),None=>match entry.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(_)=>Err(anyhow!("Não foi possível remover a sessão do chaveiro."))}}
}

// UI/protocol tests must never read a real user's credentials or modify their keyring.
// EN: In-memory mock storage for testing environments.
// PT-BR: Armazenamento simulado em memória para ambientes de teste.
#[cfg(test)]static MEMORY:std::sync::Mutex<Option<std::collections::HashMap<String,String>>>=std::sync::Mutex::new(None);
#[cfg(test)]fn read(key:&str)->Result<Option<String>>{Ok(MEMORY.lock().unwrap().as_ref().and_then(|m|m.get(key).cloned()))}
#[cfg(test)]fn write(key:&str,value:Option<&str>)->Result<()>{let mut memory=MEMORY.lock().unwrap();let map=memory.get_or_insert_with(Default::default);if let Some(value)=value{map.insert(key.into(),value.into());}else{map.remove(key);}Ok(())}

/// EN: Save current authenticated session cookie to the desktop keyring.
/// PT-BR: Salva o cookie de sessão autenticada atual no chaveiro do sistema operacional.
pub async fn save(api:&ApiClient,user:&str)->Result<()>{
    let Some(auth)=api.auth_cookie()else{return Ok(());};
    let saved=SavedSession{version:1,base:api.base_url().into(),username:user.into(),auth,owner:Some(api.session_owner())};
    let key=account(&saved.base,user);let encoded=serde_json::to_string(&saved).map_err(|_|anyhow!("Não foi possível preparar a sessão."))?;
    tokio::task::spawn_blocking(move ||{let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;write(&key,Some(&encoded))}).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}

/// Explicit user choice (e.g. unchecking Remember) clears any saved account.
/// EN: Explicitly removes any saved credential for this account from the keyring.
/// PT-BR: Escolha explícita do usuário (ex: desmarcar Lembrar-me) remove qualquer conta salva do chaveiro.
pub async fn forget_account(api:&ApiClient,user:&str)->Result<()>{
    let key=account(api.base_url(),user);tokio::task::spawn_blocking(move ||{let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;write(&key,None)}).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}

/// Delayed logout may remove only the credential owned by this session.
/// EN: Removes the session only if it belongs to the current session owner, preventing race conditions with newer logins.
/// PT-BR: O logout atrasado só remove a credencial pertencente a esta sessão, evitando apagar um login mais recente.
pub async fn forget(api:&ApiClient,user:&str)->Result<()>{
    let key=account(api.base_url(),user);let owner=api.session_owner();let auth=api.auth_cookie();
    tokio::task::spawn_blocking(move ||{
        let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;
        let Some(encoded)=read(&key)? else{return Ok(());};
        let Ok(saved)=serde_json::from_str::<SavedSession>(&encoded) else{return Ok(());};
        if saved.owner==Some(owner)||(saved.owner.is_none()&&Some(saved.auth)==auth){write(&key,None)?;}
        Ok(())
    }).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}

/// Keep the remembered cookie in sync with refresh rotation. A late refresh
/// must not recreate a logged-out credential or overwrite a newer login.
/// EN: Synchronizes a refreshed auth cookie into the keyring without overwriting newer sessions.
/// PT-BR: Mantém o cookie salvo sincronizado com a rotação de refresh. Um refresh tardio não deve recriar uma credencial deslogada.
pub async fn rotated(api:&ApiClient,user:&str,previous:&str)->Result<()>{
    let Some(auth)=api.auth_cookie()else{return Ok(());};let base=api.base_url().to_owned();let user=user.to_owned();let key=account(&base,&user);let previous=previous.to_owned();let owner=api.session_owner();
    tokio::task::spawn_blocking(move ||{
        let _guard=STORAGE.lock().map_err(|_|anyhow!("Chaveiro indisponível."))?;
        let Some(encoded)=read(&key)?else{return Ok(());};let Ok(mut saved)=serde_json::from_str::<SavedSession>(&encoded)else{return Ok(());};
        if saved.version!=1||saved.base!=base||saved.username!=user||saved.auth!=previous||saved.owner.is_some_and(|id|id!=owner){return Ok(());}
        saved.auth=auth;saved.owner=Some(owner);let encoded=serde_json::to_string(&saved).map_err(|_|anyhow!("Não foi possível preparar a sessão."))?;write(&key,Some(&encoded))
    }).await.map_err(|_|anyhow!("Chaveiro indisponível."))?
}

/// EN: Resume a saved session from the keyring, validating and proactively refreshing tokens.
/// PT-BR: Retoma uma sessão salva do chaveiro, validando-a e renovando proativamente os tokens.
pub async fn resume(api:&ApiClient,user:&str)->Result<Option<WhoamiResponse>>{
    let key=account(api.base_url(),user);let copy=key.clone();
    let Some(encoded)=tokio::task::spawn_blocking(move ||read(&copy)).await.map_err(|_|anyhow!("Chaveiro indisponível."))?? else{return Ok(None);};
    let saved=serde_json::from_str::<SavedSession>(&encoded).ok().filter(|s|s.version==1&&s.base==api.base_url()&&s.username==user);
    let Some(saved)=saved else{return Ok(None);};
    if let Some(owner)=saved.owner{api.adopt_session_owner(owner);}
    if api.restore_auth_cookie(&saved.auth).is_err(){forget(api,user).await?;return Ok(None);}
    match api.whoami().await{
        Ok(profile) if profile.username==user=>{
            // A remembered cookie may be close to expiry. Refresh before the
            // first WebSocket handshake so the regular 12-hour timer is safe.
            // EN: Proactively refresh token before WebSocket connection to avoid expiration mid-session.
            // PT-BR: Renova o token proativamente antes do handshake WebSocket para evitar expiração durante o uso.
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

#[cfg(test)]mod ownership_tests{
    use super::*;
    #[tokio::test]async fn old_logout_preserves_new_login_but_removes_its_own_rotated_cookie(){
        let a=ApiClient::new(&format!("https://{}.test",uuid::Uuid::new_v4())).unwrap();a.restore_auth_cookie("session-a").unwrap();save(&a,"Alice").await.unwrap();
        let b=ApiClient::new(a.base_url()).unwrap();b.restore_auth_cookie("session-b").unwrap();save(&b,"Alice").await.unwrap();
        // A's cleanup runs after B has saved under the same key.
        forget(&a,"Alice").await.unwrap();let saved:SavedSession=serde_json::from_str(&read(&account(a.base_url(),"Alice")).unwrap().unwrap()).unwrap();assert_eq!(saved.auth,"session-b");
        b.restore_auth_cookie("session-b-rotated").unwrap();rotated(&b,"Alice","session-b").await.unwrap();forget(&b,"Alice").await.unwrap();assert!(read(&account(a.base_url(),"Alice")).unwrap().is_none());
    }
}
