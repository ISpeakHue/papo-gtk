//! Serial renewal with an expiry margin and bounded outage recovery.
use crate::api::ApiClient;
use std::time::Duration;
use chrono::{DateTime,Utc};

fn renewal_delay(expiry:DateTime<Utc>,now:DateTime<Utc>)->Duration {
    // Renew halfway through a normal 24-hour session, always at least five
    // minutes before expiry when resuming a nearly expired token.
    let remaining=(expiry-now).to_std().unwrap_or_default();
    remaining.saturating_sub(Duration::from_secs(300)).min(Duration::from_secs(12*3600))
}
fn retry_delay(attempt:u32,jitter:u64)->Duration {
    Duration::from_millis((2000u64.saturating_mul(1u64<<attempt.min(7))).min(300_000)+jitter%1000)
}

pub async fn run(api:ApiClient,user:String,mut report:impl FnMut(anyhow::Error)->bool) {
    let mut expiry=api.session_expiry().unwrap_or_else(||Utc::now()+chrono::Duration::hours(24));
    loop {
        tokio::time::sleep(renewal_delay(expiry,Utc::now())).await;
        let mut attempt=0;
        let mut ambiguous_since=None;
        loop {
            if Utc::now()>=expiry {
                report(crate::api::ApiError{status:reqwest::StatusCode::UNAUTHORIZED,code:None,message:"A sessão expirou durante a reconexão. Entre novamente.".into()}.into());return;
            }
            let previous=api.auth_cookie();
            let started=tokio::time::Instant::now();
            let result=if let Some(since)=ambiguous_since{
                match tokio::time::timeout_at(since+Duration::from_secs(55),api.refresh()).await{
                    Ok(result)=>result,Err(_)=>Err(anyhow::anyhow!("Tempo esgotado ao confirmar a renovação da sessão.")),
                }
            }else{api.refresh().await};
            match result {
                Ok(response)=>{
                    expiry=response.connection.expires_at;
                    if let Some(previous)=previous {if crate::session::rotated(&api,&user,&previous).await.is_err(){tracing::warn!("Could not update remembered session in desktop keyring");}}
                    break;
                }
                Err(error)=>{
                    let terminal=crate::api::is_session_error(&error)||error.downcast_ref::<crate::api::ApiError>().is_some_and(|e|e.status.is_client_error()&&e.status!=reqwest::StatusCode::TOO_MANY_REQUESTS);
                    if terminal {report(error);return;}
                    if api.auth_cookie().is_some()&&api.auth_cookie()!=previous{
                        // Set-Cookie may arrive even when the JSON body is lost.
                        // Keep the accepted rotation instead of replaying it.
                        if let Some(previous)=previous{let _=crate::session::rotated(&api,&user,&previous).await;}
                        expiry=api.session_expiry().unwrap_or_else(||Utc::now()+chrono::Duration::hours(1));
                        if !report(error){return;}break;
                    }
                    let ambiguous=error.downcast_ref::<crate::api::ApiError>().is_none()
                        && !error.downcast_ref::<reqwest::Error>().is_some_and(|e|e.is_connect())
                        && api.auth_cookie()==previous;
                    // Backend rotation has a 60-second grace period. Recover a
                    // lost response promptly, but never replay an old mutation
                    // after that window (which could trigger reuse detection).
                    if ambiguous {ambiguous_since.get_or_insert(started);}
                    if ambiguous_since.is_some_and(|since|since.elapsed()>=Duration::from_secs(45)) {
                        report(anyhow::anyhow!("Não foi possível confirmar a renovação da sessão. Entre novamente para continuar com segurança."));return;
                    }
                    if attempt==0&&!report(error){return;}
                    let jitter=u64::from_le_bytes(uuid::Uuid::new_v4().as_bytes()[..8].try_into().unwrap());
                    let delay=if ambiguous_since.is_some(){Duration::from_secs(1)}else{retry_delay(attempt,jitter)};
                    let remaining=(expiry-Utc::now()).to_std().unwrap_or_default();
                    tokio::time::sleep(delay.min(remaining)).await;
                    attempt+=1;
                }
            }
        }
    }
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn retries_precede_expiry_and_use_bounded_backoff(){
        let now=Utc::now();let expiry=now+chrono::Duration::hours(24);
        assert_eq!(renewal_delay(expiry,now),Duration::from_secs(12*3600));
        assert_eq!(renewal_delay(now+chrono::Duration::minutes(4),now),Duration::ZERO);
        assert!(retry_delay(0,0)<Duration::from_secs(3));
        assert!(retry_delay(99,999)<=Duration::from_secs(301));
        assert!(now+chrono::Duration::from_std(renewal_delay(expiry,now)+retry_delay(0,0)).unwrap()<expiry);
    }
}

#[cfg(test)]mod recovery_tests{
    use super::*;
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    async fn server(statuses:Vec<u16>)->(ApiClient,tokio::sync::mpsc::Receiver<u16>,tokio::task::JoinHandle<()>){
        use base64::Engine;
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let api=ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
        let claims=serde_json::json!({"exp":(Utc::now()+chrono::Duration::minutes(4)).timestamp()}).to_string();let token=format!("header.{}.signature",base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims));api.restore_auth_cookie(&token).unwrap();
        let (tx,rx)=tokio::sync::mpsc::channel(4);
        let server=tokio::spawn(async move{for status in statuses{let lost=status==299;let status=if lost{200}else{status};
            let (mut socket,_)=listener.accept().await.unwrap();let mut request=Vec::new();loop{let mut data=[0;2048];let n=socket.read(&mut data).await.unwrap();if n==0{break;}request.extend_from_slice(&data[..n]);if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}}
            assert!(String::from_utf8_lossy(&request).starts_with("POST /auth/refresh "));
            let body=if status==200{serde_json::json!({"connection":{"id":uuid::Uuid::new_v4(),"created_at":Utc::now(),"expires_at":Utc::now()+chrono::Duration::hours(24)}})}else{serde_json::json!({"detail":"temporary failure"})}.to_string();
            let cookie=if status==200{"Set-Cookie: Auth=renewed; Secure; HttpOnly; Path=/\r\n"}else{""};
            socket.write_all(format!("HTTP/1.1 {status} response\r\n{cookie}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",(body.len()+if lost{20}else{0})).as_bytes()).await.unwrap();let _=tx.send(status).await;
        }});(api,rx,server)
    }
    #[tokio::test]async fn transient_refresh_recovers_before_expiry_and_cancels_on_logout(){
        let (api,mut arrivals,server)=server(vec![503,200]).await;let client=api.clone();let task=tokio::spawn(async move{run(client,"test".into(),|_|true).await});
        assert_eq!(arrivals.recv().await,Some(503));assert_eq!(tokio::time::timeout(Duration::from_secs(5),arrivals.recv()).await.unwrap(),Some(200));server.await.unwrap();
        for _ in 0..50{if api.auth_cookie().as_deref()==Some("renewed"){break;}tokio::time::sleep(Duration::from_millis(5)).await;}
        assert_eq!(api.auth_cookie().as_deref(),Some("renewed"));task.abort();assert!(task.await.unwrap_err().is_cancelled());
    }
    #[tokio::test]async fn terminal_unauthorized_ends_renewal_without_retry(){
        let (api,_,server)=server(vec![401]).await;let (tx,mut rx)=tokio::sync::mpsc::unbounded_channel();
        tokio::time::timeout(Duration::from_secs(2),run(api,"test".into(),move |error|{tx.send(crate::api::is_session_error(&error)).unwrap();true})).await.unwrap();assert_eq!(rx.recv().await,Some(true));server.await.unwrap();
    }
    #[tokio::test]async fn received_rotation_survives_a_lost_response_body_without_replaying_refresh(){
        let (api,mut arrivals,server)=server(vec![299]).await;crate::session::save(&api,"body-loss").await.unwrap();let client=api.clone();let task=tokio::spawn(async move{run(client,"body-loss".into(),|_|true).await});
        assert_eq!(arrivals.recv().await,Some(200));server.await.unwrap();
        let key=crate::session::account(api.base_url(),"body-loss");
        let mut saved=None;for _ in 0..200{saved=crate::session::read(&key).unwrap().and_then(|v|serde_json::from_str::<crate::session::SavedSession>(&v).ok());if saved.as_ref().is_some_and(|s|s.auth=="renewed"){break;}tokio::time::sleep(Duration::from_millis(5)).await;}
        assert_eq!(saved.unwrap().auth,"renewed");assert!(!task.is_finished());task.abort();
    }

}
