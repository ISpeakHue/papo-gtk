use super::*;
use crate::api::ApiClient;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Calls = Arc<Mutex<Vec<(String, Instant)>>>;

async fn server(reply: impl Fn(&str, usize) -> (u16, String, Value) + Send + Sync + 'static)
    -> (ApiClient, Calls, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = ApiClient::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let calls: Calls = Default::default();
    let recorded = calls.clone();
    let reply = Arc::new(reply);
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let recorded = recorded.clone();
            let reply = reply.clone();
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    let n = socket.read(&mut buf).await.unwrap();
                    if n == 0 { return; }
                    data.extend_from_slice(&buf[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]);
                        let len = headers.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned))
                            .and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                        if data.len() >= end + 4 + len { break; }
                    }
                }
                let line = String::from_utf8_lossy(&data).lines().next().unwrap().to_owned();
                let count = { let mut calls = recorded.lock().unwrap(); calls.push((line.clone(), Instant::now())); calls.len() };
                let (status, extra, body) = reply(&line, count);
                let body = body.to_string();
                let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });
    (api, calls, task)
}

#[tokio::test]
async fn cloned_clients_and_media_share_a_paced_budget() {
    let (api, calls, task) = server(|_, _| (200, String::new(), json!({}))).await;
    let mut jobs = Vec::new();
    for i in 0..20 {
        let api = api.clone();
        jobs.push(tokio::spawn(async move {
            if i % 2 == 0 { api.health().await.unwrap(); }
            else { api.media_bytes("/attachments/test/thumbnail", 1024).await.unwrap(); }
        }));
    }
    for job in jobs { job.await.unwrap(); }
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 20);
    for pair in calls.windows(2) {
        assert!(pair[1].1.duration_since(pair[0].1) >= Duration::from_millis(100), "requests arrived in a burst");
    }
    task.abort();
}

#[tokio::test]
async fn background_downloads_do_not_keep_overtaking_a_foreground_read() {
    let (api, calls, task) = server(|_, _| (200, String::new(), json!({}))).await;
    let mut jobs = Vec::new();
    for _ in 0..4 {
        let api = api.clone();
        jobs.push(tokio::spawn(async move {
            for _ in 0..4 { api.media_bytes("/attachments/test/thumbnail", 1024).await.unwrap(); }
        }));
    }
    while calls.lock().unwrap().is_empty() { tokio::task::yield_now().await; }
    tokio::time::sleep(Duration::from_millis(20)).await;
    api.health().await.unwrap();
    let foreground = calls.lock().unwrap().iter().position(|(line, _)|line.starts_with("GET /health ")).unwrap();
    assert!(foreground <= 5, "four background workers must not run all their pages before a foreground read");
    for job in jobs { job.await.unwrap(); }
    task.abort();
}

#[tokio::test]
async fn rate_limited_reads_retry_without_waiting_for_the_ui_poll() {
    let (api, calls, task) = server(|_, n| if n == 1 {
        (429, "Retry-After: 1\r\n".into(), json!({"type":"/rate-limit","detail":"slow down"}))
    } else { (200, String::new(), json!({})) }).await;
    let started = Instant::now();
    api.health().await.unwrap();
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].1.duration_since(calls[0].1) >= Duration::from_millis(950));
    assert!(started.elapsed() < Duration::from_secs(5));
    task.abort();
}

#[tokio::test]
async fn rate_limited_mutations_are_not_replayed_and_cool_down_other_clones() {
    let (api, calls, task) = server(|line, _| if line.starts_with("DELETE") || line.starts_with("POST") {
        (429, "Retry-After: 1\r\n".into(), json!({"type":"/rate-limit","detail":"slow down"}))
    } else { (200, String::new(), json!({})) }).await;
    let error = api.delete_message(uuid::Uuid::new_v4()).await.unwrap_err();
    assert_eq!(error.downcast_ref::<crate::api::ApiError>().unwrap().status, StatusCode::TOO_MANY_REQUESTS);
    api.clone().health().await.unwrap();
    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "a rejected mutation must be sent only once");
        assert!(calls[1].1.duration_since(calls[0].1) >= Duration::from_millis(950));
    }
    assert!(api.login("alice", "test-password").await.is_err());
    assert_eq!(calls.lock().unwrap().len(), 3, "account login must not be retried automatically");
    task.abort();
}

#[tokio::test]
async fn profile_batch_posts_are_safe_reads_and_can_retry_a_429() {
    let (api, calls, task) = server(|_, n| if n == 1 {
        (429, "Retry-After: 1\r\n".into(), json!({"detail":"slow down"}))
    } else { (200, String::new(), json!({"profiles":[]})) }).await;
    assert!(api.user_profiles(&[uuid::Uuid::new_v4()]).await.unwrap().is_empty());
    assert_eq!(calls.lock().unwrap().len(), 2);
    assert!(calls.lock().unwrap().iter().all(|(line, _)|line.starts_with("POST /users/profile_batch ")));
    task.abort();
}

#[tokio::test]
async fn retries_are_bounded_without_retry_after_and_other_errors_are_not_retried() {
    for status in [429, 401, 403, 500] {
        let (api, calls, task) = server(move |_, _| (status, String::new(), json!({"detail":"failure"}))).await;
        let error = api.health().await.unwrap_err();
        assert_eq!(error.downcast_ref::<crate::api::ApiError>().unwrap().status.as_u16(), status);
        assert_eq!(calls.lock().unwrap().len(), if status == 429 { 3 } else { 1 });
        task.abort();
    }
}

#[tokio::test]
async fn channel_snapshots_reuse_embedded_overrides_and_fall_back_only_when_missing() {
    let (api, calls, task) = server(|line, _| {
        if line.starts_with("GET /channels ") {
            let channels: Vec<_> = (0..70).map(|i| json!({"id":uuid::Uuid::from_u128(i+1),"name":"test", "created_at":"2026-10-07T00:00:00Z", "permissions":[]})).collect();
            (200, String::new(), json!({"channels":channels}))
        } else { (500, String::new(), json!({"detail":"unexpected permission fetch"})) }
    }).await;
    assert_eq!(api.channels_with_permissions().await.unwrap().len(), 70);
    assert_eq!(calls.lock().unwrap().len(), 1, "70 channels must need only one request");
    task.abort();

    let id = uuid::Uuid::new_v4();
    let (api, calls, task) = server(move |line, _| {
        let body = if line.starts_with("GET /channels ") {
            json!({"channels":[{"id":id,"name":"legacy","created_at":"2026-10-07T00:00:00Z"}]})
        } else { json!({"channel_id":id,"permissions":[{"role_id":id,"role_name":"restricted","permissions":{"read_channel":false}}]}) };
        (200, String::new(), body)
    }).await;
    let channels = api.channels_with_permissions().await.unwrap();
    assert_eq!(channels[0].permissions.as_ref().unwrap()[0].permissions.read_channel, Some(false));
    assert_eq!(calls.lock().unwrap().len(), 2);
    task.abort();
}

#[tokio::test]
async fn long_retry_after_does_not_start_an_automatic_retry() {
    let (api, calls, task) = server(|_, _| (429, "Retry-After: 60\r\n".into(), json!({"detail":"slow down"}))).await;
    assert!(api.health().await.is_err());
    assert_eq!(calls.lock().unwrap().len(), 1);
    task.abort();
}

#[test]
fn retry_after_accepts_seconds_and_http_dates() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::RETRY_AFTER, "2".parse().unwrap());
    assert_eq!(retry_after(&headers), Some(Duration::from_secs(2)));
    let future = (chrono::Utc::now() + chrono::Duration::seconds(60)).format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    headers.insert(reqwest::header::RETRY_AFTER, future.parse().unwrap());
    assert!(retry_after(&headers).unwrap() >= Duration::from_secs(58));
    headers.insert(reqwest::header::RETRY_AFTER, "invalid".parse().unwrap());
    assert_eq!(retry_after(&headers), None);
}
