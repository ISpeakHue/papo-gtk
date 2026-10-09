//! Protocol regressions from the adjacent papo-backend handlers and storage cursors.
use super::*;
use serde_json::{json, Value};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, time::timeout};

const ID: &str = "12345678-1234-4234-8234-123456789abc";
const CHANNEL: &str = "12345678-1234-4234-8234-123456789abd";
const DATE: &str = "2026-10-03T12:34:56.123456789Z";

struct Request { method: String, url: Url, headers: String, body: String }

fn message() -> Value {
    json!({"id":ID, "channel_id":CHANNEL, "author_id":null, "content":"Olá 🌎", "created_at":DATE})
}

async fn mock(responses: Vec<(u16, Value)>) -> (ApiClient, tokio::task::JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = ApiClient::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (status, response) in responses {
            let (mut stream, _) = timeout(Duration::from_secs(5), listener.accept()).await.unwrap().unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let (headers, body) = loop {
                let length = timeout(Duration::from_secs(5), stream.read(&mut buffer)).await.unwrap().unwrap();
                assert_ne!(length, 0, "incomplete request");
                bytes.extend_from_slice(&buffer[..length]);
                if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                    let length: usize = headers.lines().filter_map(|line| line.split_once(':'))
                        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                        .map(|(_, value)| value.trim().parse().unwrap()).unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break (headers, String::from_utf8(bytes[end+4..end+4+length].to_vec()).unwrap());
                    }
                }
            };
            let mut line = headers.lines().next().unwrap().split_whitespace();
            let method = line.next().unwrap().into();
            let url = Url::parse(&format!("http://localhost{}", line.next().unwrap())).unwrap();
            requests.push(Request { method, url, headers, body });
            let body = if status == 204 { String::new() } else { response.to_string() };
            stream.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        }
        requests
    });
    (client, server)
}

#[tokio::test]
async fn text_messages_use_multipart_and_optional_reply_field() {
    let (client, server) = mock(vec![(201, message()), (201, message())]).await;
    let channel = CHANNEL.parse().unwrap();
    for reply_to in [None, Some(Uuid::parse_str(ID).unwrap())] {
        let sent = client.send_message(&CreateMessageRequest { channel_id:channel, content:Some("Olá 🌎".into()), reply_to,embeds:vec![] }).await.unwrap();
        assert_eq!(sent.author_id, None);
    }
    let requests = server.await.unwrap();
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url.path(), "/messages");
        assert!(request.headers.to_lowercase().contains("content-type: multipart/form-data; boundary="));
        assert!(request.body.contains(&format!("name=\"channel_id\"\r\n\r\n{CHANNEL}\r\n")));
        assert!(request.body.contains("name=\"content\"\r\n\r\nOlá 🌎\r\n"));
        assert_eq!(request.body.contains("name=\"reply_to\""), index == 1);
        if index == 1 { assert!(request.body.contains(&format!("name=\"reply_to\"\r\n\r\n{ID}\r\n"))); }
    }
}

#[tokio::test]
async fn history_pagination_sends_timestamp_and_id_in_descending_order() {
    let response = json!({"channel_id":CHANNEL,"messages":[message()],"has_more":true});
    let (client, server) = mock(vec![(200,response.clone()),(200,response)]).await;
    let first = client.list_messages(CHANNEL.parse().unwrap(), None).await.unwrap();
    assert!(first.has_more);
    let cursor = MessageCursor::from(&first.messages[0]);
    client.list_messages(CHANNEL.parse().unwrap(), Some(cursor)).await.unwrap();
    let requests = server.await.unwrap();
    assert_eq!(requests[0].url.query(), Some("order=desc"));
    let query: std::collections::HashMap<_,_> = requests[1].url.query_pairs().into_owned().collect();
    assert_eq!(query.len(),3);
    assert_eq!(query["order"], "desc");
    assert_eq!(query["last_id"], ID);
    assert_eq!(query["since"].parse::<chrono::DateTime<chrono::Utc>>().unwrap(), cursor.created_at);
    assert_eq!(cursor.created_at.timestamp_subsec_nanos(),123456789);
}

#[tokio::test]
async fn embeds_use_new_endpoint_and_custom_payloads_on_create_and_edit(){
    let embed=json!({"id":ID,"source_type":"custom","fetch_method":"manual","title":"Rich","color":"#123ABC","created_at":DATE,"fields":[{"position":0,"name":"One","value":"Two","inline":true}],"thumbnail":{"mime_type":"image/png"},"image_data":"AA=="});
    let mut response=message();response["embeds"]=json!([embed.clone()]);
    let (client,server)=mock(vec![(200,embed),(200,response.clone()),(200,response.clone()),(200,response)]).await;
    let e=client.get_embed(ID.parse().unwrap()).await.unwrap();assert!(e.fetched_at.is_none());assert_eq!(e.fields[0].value,"Two");
    let input=EmbedInput{title:"Rich 🌎".into(),color:"#123ABC".into(),fields:vec![EmbedFieldInput{name:"One".into(),value:"Two".into(),inline:true}],..Default::default()};
    let create=CreateMessageRequest{channel_id:CHANNEL.parse().unwrap(),content:None,reply_to:None,embeds:vec![input.clone()]};
    client.send_message(&create).await.unwrap();client.send_with_files(&create,&[],|_|{}).await.unwrap();client.edit_message_embeds(ID.parse().unwrap(),"",&[input.clone()]).await.unwrap();
    let r=server.await.unwrap();assert_eq!(r[0].url.path(),format!("/embeds/{ID}"));
    for r in &r[1..3]{assert_eq!(r.method,"POST");assert!(r.body.contains("name=\"embeds\""));assert!(r.body.contains(&serde_json::to_string(&vec![input.clone()]).unwrap()));}
    assert_eq!(serde_json::from_str::<Value>(&r[3].body).unwrap(),json!({"content":"","embeds":[input]}));
}

#[tokio::test]
async fn wrong_channel_history_is_rejected() {
    let (client, server) = mock(vec![(200,json!({"channel_id":ID,"messages":[],"has_more":false}))]).await;
    assert!(client.list_messages(CHANNEL.parse().unwrap(), None).await.unwrap_err().to_string().contains("outro canal"));
    server.await.unwrap();
}

#[tokio::test]
async fn member_pagination_and_summary_batch_follow_backend_contract() {
    let first = json!({"id":ID,"username":"alice","created_at":DATE});
    let second = json!({"id":CHANNEL,"username":"bob","created_at":DATE});
    let (client, server) = mock(vec![
        (200,json!({"users":[first.clone()],"has_more":true})),
        (200,json!({"users":[second],"has_more":false})),
        (200,json!([first])),
    ]).await;
    let users = client.list_all_users().await.unwrap();
    assert_eq!(users.len(),2);
    assert_eq!(client.user_summaries(vec![ID.parse().unwrap()]).await.unwrap()[0].username,"alice");
    let requests = server.await.unwrap();
    assert_eq!(requests[0].url.query(),Some("order=asc"));
    let query: std::collections::HashMap<_,_> = requests[1].url.query_pairs().into_owned().collect();
    assert_eq!(query["last_id"],ID);
    assert_eq!(query["order"],"asc");
    assert_eq!(requests[2].url.path(),"/users/user_summary_batch");
    assert_eq!(serde_json::from_str::<Value>(&requests[2].body).unwrap(),json!({"ids":[ID]}));
}

#[tokio::test]
async fn nonadvancing_member_pagination_fails_instead_of_looping() {
    let response = json!({"users":[{"id":ID,"username":"alice","created_at":DATE}],"has_more":true});
    let (client, server) = mock(vec![(200,response.clone()),(200,response)]).await;
    assert!(client.list_all_users().await.is_err());
    server.await.unwrap();
}

#[tokio::test]
async fn pin_route_and_custom_reaction_preserve_message_and_emoji_ids() {
    let (client, server) = mock(vec![(204,Value::Null),(201,json!({"message_id":ID}))]).await;
    client.pin_message(CHANNEL.parse().unwrap(), ID.parse().unwrap()).await.unwrap();
    client.add_reaction(CHANNEL.parse().unwrap(), ID.parse().unwrap(), Some(ID.parse().unwrap()), None).await.unwrap();
    let requests = server.await.unwrap();
    assert_eq!(requests[0].url.path(),format!("/channels/{CHANNEL}/messages/{ID}/pin"));
    assert!(requests[0].body.is_empty());
    assert_eq!(serde_json::from_str::<Value>(&requests[1].body).unwrap(),json!({"emoji_id":ID,"unicode":null}));
}

#[tokio::test]
async fn rest_errors_distinguish_expired_banned_and_permission_denied_sessions() {
    for (status,code,expired) in [(401,"unauthorized",true),(401,"connection-reused",true),(403,"banned",true),(403,"forbidden",false)] {
        let (client,server) = mock(vec![(status,json!({"type":format!("https://papo.test/problems/{code}"),"detail":"Test error"}))]).await;
        let error = client.list_channels().await.unwrap_err();
        assert_eq!(is_session_error(&error),expired);
        assert_eq!(error.to_string(),"Test error");
        server.await.unwrap();
    }
}

#[test]
fn messages_accept_deleted_authors_and_preview_video_metadata() {
    for author in [Value::Null, json!(""), json!(ID)] {
        let mut value = message();
        value["author_id"] = author.clone();
        let parsed: Message = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.author_id.is_some(), author == json!(ID));
    }
    let preview: Embed = serde_json::from_value(json!({"id":ID,"url":"https://example.test/video", "kind":"video", "video_url":format!("/embeds/{ID}/video"),"fetched_at":DATE})).unwrap();
    assert!(preview.video.unwrap().url.unwrap().ends_with("/video"));
}

#[tokio::test]
async fn avatar_profiles_use_authenticated_batches_of_at_most_fifty() {
    let ids: Vec<_> = (1..=51).map(Uuid::from_u128).collect();
    let (client, server) = mock(vec![
        (200, json!({"profiles":[{"id":ids[0],"username":"alice","created_at":DATE,
            "avatar_blob":"encoded-avatar","avatar_format":"WEBP"}]})),
        (200, json!({"profiles":[{"id":ids[50],"username":"bob","created_at":DATE,
            "avatar_blob":null,"avatar_format":""}]})),
    ]).await;
    client.cookies.add_cookie_str("Auth=session; HttpOnly; Path=/", &client.base);
    let profiles = client.user_profiles(&ids).await.unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].avatar_blob.as_deref(), Some("encoded-avatar"));
    assert_eq!(profiles[0].avatar_format.as_deref(), Some("WEBP"));
    assert!(profiles[1].avatar_blob.is_none());
    let requests = server.await.unwrap();
    for (request, expected_ids) in requests.iter().zip([&ids[..50], &ids[50..]]) {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url.path(), "/users/profile_batch");
        assert!(request.headers.to_lowercase().contains("cookie: auth=session"));
        assert_eq!(serde_json::from_str::<Value>(&request.body).unwrap(), json!({"ids":expected_ids}));
    }
    assert!(client.user_profiles(&[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn permissions_use_full_roles_and_identity_checked_channel_overrides() {
    let (client, server) = mock(vec![
        (200, json!({"roles":[{"id":ID,"name":"staff","created_at":DATE,"permissions":{"pin_message":true}}]})),
        (200, json!({"channel_id":CHANNEL,"permissions":[{"role_id":ID,"role_name":"staff","permissions":{"read_channel":true,"send_messages":false}}]})),
        (200, json!({"channel_id":ID,"permissions":[]})),
    ]).await;
    client.cookies.add_cookie_str("Auth=contract-session; Path=/", &client.base);
    assert_eq!(client.list_roles().await.unwrap()[0].permissions.pin_message, Some(true));
    assert_eq!(client.channel_permissions(CHANNEL.parse().unwrap()).await.unwrap()[0].permissions.send_messages, Some(false));
    assert!(client.channel_permissions(CHANNEL.parse().unwrap()).await.is_err());
    let requests = server.await.unwrap();
    assert_eq!(requests[0].url.path(), "/roles");
    assert_eq!(requests[1].url.path(), format!("/channels/{CHANNEL}/permissions"));
    assert!(requests.iter().all(|r| r.headers.to_lowercase().contains("cookie: auth=contract-session")));
}

#[tokio::test]
async fn edit_delete_pin_unpin_and_pinned_list_follow_backend_routes() {
    let (client, server) = mock(vec![(200,message()), (204,json!(null)), (201,json!({})), (204,json!(null)),
        (200,json!({"channel_id":CHANNEL,"pinned":[message()]})), (200,json!({"channel_id":ID,"pinned":[]}))]).await;
    let id = ID.parse().unwrap(); let channel = CHANNEL.parse().unwrap();
    client.edit_message(id, "edited 🌎").await.unwrap(); client.delete_message(id).await.unwrap();
    client.pin_message(channel, id).await.unwrap(); client.unpin_message(channel, id).await.unwrap();
    assert_eq!(client.pinned_messages(channel).await.unwrap()[0].id, id);
    assert!(client.pinned_messages(channel).await.is_err());
    let r = server.await.unwrap();
    assert_eq!(r[0].method,"PUT"); assert_eq!(r[0].url.path(),format!("/messages/{ID}"));
    assert_eq!(serde_json::from_str::<Value>(&r[0].body).unwrap(),json!({"content":"edited 🌎","embeds":[]}));
    assert_eq!(r[1].method,"DELETE"); assert_eq!(r[2].method,"POST"); assert_eq!(r[3].method,"DELETE");
    assert_eq!(r[2].url.path(),format!("/channels/{CHANNEL}/messages/{ID}/pin"));
    assert_eq!(r[2].url.path(),r[3].url.path());
}

#[tokio::test]
async fn emoji_pagination_keeps_equal_timestamp_ids_and_create_delete_payloads() {
    let a = json!({"id":ID,"name":"first","image_blob":null,"format":"PNG","created_by":null,"created_at":DATE});
    let b = json!({"id":CHANNEL,"name":"second","image_blob":"aW1hZ2U=","format":"GIF","created_by":ID,"created_at":DATE});
    let (client, server) = mock(vec![(200,json!({"emojis":[a],"has_more":true})),
        (200,json!({"emojis":[b.clone()],"has_more":false})),(201,b),(204,json!(null))]).await;
    let list = client.all_emojis().await.unwrap(); assert_eq!(list.len(),2); assert_eq!(list[1].created_by,Some(ID.parse().unwrap()));
    client.create_emoji(&CreateEmojiRequest { name:"second".into(),format:"GIF".into(),image_blob:"aW1hZ2U=".into() }).await.unwrap();
    client.delete_emoji(CHANNEL.parse().unwrap()).await.unwrap();
    let r = server.await.unwrap(); assert_eq!(r[0].url.query(),Some("order=asc"));
    let q: std::collections::HashMap<_,_> = r[1].url.query_pairs().into_owned().collect();
    assert_eq!(q["last_id"],ID); assert_eq!(q["since"].parse::<chrono::DateTime<chrono::Utc>>().unwrap().timestamp_subsec_nanos(),123456789);
    assert_eq!(serde_json::from_str::<Value>(&r[2].body).unwrap(),json!({"name":"second","format":"GIF","image_blob":"aW1hZ2U="}));
    assert_eq!(r[3].method,"DELETE"); assert_eq!(r[3].url.path(),format!("/emojis/{CHANNEL}"));
}

#[tokio::test]
async fn nonadvancing_emoji_page_fails_instead_of_looping() {
    let (client, server) = mock(vec![(200,json!({"emojis":[],"has_more":true}))]).await;
    assert!(client.all_emojis().await.unwrap_err().to_string().contains("não avançou")); server.await.unwrap();
}

#[tokio::test]
async fn grouped_reaction_pagination_uses_oldest_row_not_group_order() {
    let old_id = Uuid::from_u128(1); let newer_id = Uuid::from_u128(2);
    let group = |id, unicode, user| json!({"emoji_id":null,"unicode":unicode,"count":1,"users":[{"id":id,"user_id":user,"created_at":DATE}]});
    let (client, server) = mock(vec![
        (200,json!({"message_id":ID,"reactions":[group(old_id,"❤️",ID),group(newer_id,"👍",CHANNEL)],"has_more":true})),
        (200,json!({"message_id":ID,"reactions":[{"emoji_id":null,"unicode":"❤️","count":1,"users":[{"id":Uuid::from_u128(0),"user_id":CHANNEL,"created_at":"2026-10-02T00:00:00Z"}]}],"has_more":false})),
        (204,json!(null)),
    ]).await;
    let id = ID.parse().unwrap(); let channel = CHANNEL.parse().unwrap();
    let groups = client.all_reactions(channel,id).await.unwrap(); assert_eq!(groups.len(),2);
    assert_eq!(groups[0].count,2); assert_eq!(groups[0].users.len(),2);
    client.remove_reaction(channel,id,Some(id),None).await.unwrap();
    let r = server.await.unwrap(); let q: std::collections::HashMap<_,_> = r[1].url.query_pairs().into_owned().collect();
    assert_eq!(q["order"],"desc"); assert_eq!(q["last_id"],old_id.to_string());
    assert_eq!(r[2].method,"DELETE"); assert_eq!(serde_json::from_str::<Value>(&r[2].body).unwrap(),json!({"emoji_id":ID,"unicode":null}));
}

#[tokio::test]
async fn reactions_reject_wrong_message_and_nonadvancing_pages() {
    let (client, server) = mock(vec![(200,json!({"message_id":CHANNEL,"reactions":[],"has_more":false})),
        (200,json!({"message_id":ID,"reactions":[],"has_more":true}))]).await;
    assert!(client.all_reactions(CHANNEL.parse().unwrap(),ID.parse().unwrap()).await.is_err());
    assert!(client.all_reactions(CHANNEL.parse().unwrap(),ID.parse().unwrap()).await.unwrap_err().to_string().contains("não avançou"));
    server.await.unwrap();
}

#[tokio::test]
async fn message_limit_counts_unicode_characters_not_utf8_bytes() {
    let (client, server) = mock(vec![(200,message())]).await;
    client.edit_message(ID.parse().unwrap(), &"🌎".repeat(8192)).await.unwrap();
    assert!(client.edit_message(ID.parse().unwrap(), &"🌎".repeat(8193)).await.is_err());
    assert_eq!(server.await.unwrap().len(),1);
}

#[tokio::test]
async fn attachment_messages_stream_repeated_files_and_preserve_reply() {
    use crate::api::features::{UploadFile, TemporaryFile};
    let path=std::env::temp_dir().join(format!("papo-upload-test-{}",Uuid::new_v4()));
    tokio::fs::write(&path,b"file bytes\n").await.unwrap();let _cleanup=TemporaryFile(path.clone());
    let file=UploadFile::inspect(path).await.unwrap();
    let (client,server)=mock(vec![(201,message()),(201,message())]).await;
    let total=Arc::new(std::sync::atomic::AtomicU64::new(0));
    for content in [None,Some("Reply with files".into())] {
        let count=total.clone();
        client.send_with_files(&CreateMessageRequest{channel_id:CHANNEL.parse().unwrap(),content,reply_to:Some(ID.parse().unwrap()),embeds:vec![]},&[file.clone(),file.clone()],move |bytes| {count.store(bytes,std::sync::atomic::Ordering::Relaxed);}).await.unwrap();
    }
    assert_eq!(total.load(std::sync::atomic::Ordering::Relaxed),22);
    for r in server.await.unwrap(){assert_eq!(r.method,"POST");assert_eq!(r.url.path(),"/messages");
        assert_eq!(r.body.matches("name=\"attachments\"; filename=").count(),2);
        assert_eq!(r.body.matches("file bytes\n").count(),2);assert!(r.body.contains("name=\"reply_to\""));
    }
}

#[tokio::test]
async fn account_writes_follow_separate_profile_status_image_and_settings_contracts() {
    use crate::models::*;
    let config=UserConfig::default();
    let (client,server)=mock(vec![(200,json!({"response":"saved"})),(200,json!({"response":"saved"})),
        (200,json!({"response":"saved"})),(200,json!({"response":"saved"})),
        (200,json!({"user_id":ID,"version":1,"config":config,"updated_at":"2026-10-03T00:00:00Z"})),(200,json!({"response":"saved"}))]).await;
    let id=ID.parse().unwrap();
    client.update_profile(id,&UpdateUserRequest{nickname:"New name".into(),status:"Custom status".into(),description:"Description".into(),typing:Some("Custom typing".into())}).await.unwrap();
    client.update_status(id,None).await.unwrap();client.update_image(id,false,&[]).await.unwrap();client.update_image(id,true,&[]).await.unwrap();
    let mut config=config;let d=config.display.as_mut().unwrap();d.show_avatars=Some(false);d.show_timestamps=Some(false);config.notifications.as_mut().unwrap().sound=Some(false);
    client.save_settings(&config).await.unwrap();client.channel_notifications(CHANNEL.parse().unwrap(),id,NotificationSettings::OnlyMentions).await.unwrap();
    let requests=server.await.unwrap();let bodies:Vec<Value>=requests.iter().map(|r|serde_json::from_str(&r.body).unwrap()).collect();
    assert_eq!(bodies[0],json!({"nickname":"New name","status":"Custom status","description":"Description","typing":"Custom typing"}));
    assert_eq!(requests[0].url.path(),format!("/users/{ID}"));assert_eq!(requests[1].url.path(),format!("/users/{ID}/status"));assert_eq!(bodies[1],json!({"status":null}));
    assert_eq!(bodies[2],json!({"avatar":"","avatar_format":""}));assert_eq!(bodies[3],json!({"banner":"","banner_format":""}));
    assert_eq!(bodies[4],json!({"config":{"theme":"system","notifications":{"enabled":true,"messagePreview":true,"sound":false,"mentions":true},"display":{"fontSize":"medium","messageDensity":"normal","showTimestamps":false,"showAvatars":false}}}));
    assert_eq!(requests[5].method,"POST");assert_eq!(requests[5].url.path(),format!("/channels/{CHANNEL}/user/{ID}/settings"));assert_eq!(bodies[5],json!({"notification_settings":"only_mentions"}));
    assert!(!requests.iter().any(|r|r.body.contains("last_read")));
}

#[tokio::test]
async fn authenticated_media_is_bounded_streamed_saved_and_cleaned() {
    use crate::api::features::save_download;
    let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let client=ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
    client.cookies.add_cookie_str("Auth=media-session; Path=/",&client.base);
    let data:Vec<u8>=(0..200_000).map(|i|(i%256)as u8).collect();let sent=data.clone();
    let server=tokio::spawn(async move{for expected in ["/attachments/file","/embeds/preview/video","/media/hash"] {
        let (mut socket,_)=listener.accept().await.unwrap();let mut headers=vec![0;4096];let count=socket.read(&mut headers).await.unwrap();let headers=String::from_utf8_lossy(&headers[..count]);
        assert!(headers.starts_with(&format!("GET {expected} ")));assert!(headers.to_ascii_lowercase().contains("cookie: auth=media-session"));
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",sent.len()).as_bytes()).await.unwrap();let _=socket.write_all(&sent).await;
    }});
    let file=client.download_temporary("/attachments/file",200_000).await.unwrap();let temp=file.0.clone();assert_eq!(tokio::fs::read(&temp).await.unwrap(),data);
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&temp).unwrap().permissions().mode()&0o777,0o600);}
    let destination=std::env::temp_dir().join(format!("papo-save-test-{}",Uuid::new_v4()));let _cleanup=crate::api::features::TemporaryFile(destination.clone());
    save_download(file,destination.clone()).await.unwrap();assert_eq!(tokio::fs::read(&destination).await.unwrap(),data);assert!(!temp.exists());
    let video=client.download_temporary("/embeds/preview/video",200_000).await.unwrap();let path=video.0.clone();drop(video);assert!(!path.exists());
    assert!(client.media_bytes("/media/hash",1024).await.is_err());server.await.unwrap();
}

#[tokio::test]
async fn avatar_and_banner_uploads_send_validated_base64_and_format() {
    use base64::Engine;
    let mut image=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(1,1).write_to(&mut image,image::ImageFormat::Png).unwrap();let bytes=image.into_inner();
    let (client,server)=mock(vec![(200,json!({"response":"saved"})),(200,json!({"response":"saved"}))]).await;
    client.update_image(ID.parse().unwrap(),false,&bytes).await.unwrap();client.update_image(ID.parse().unwrap(),true,&bytes).await.unwrap();
    for (r,banner) in server.await.unwrap().iter().zip([false,true]) {
        let body:Value=serde_json::from_str(&r.body).unwrap();let field=if banner{"banner"}else{"avatar"};
        assert_eq!(r.method,"PUT");assert_eq!(r.url.path(),format!("/users/{ID}/{field}"));assert_eq!(body[format!("{field}_format")],"PNG");
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(body[field].as_str().unwrap()).unwrap(),bytes);
    }
}

#[tokio::test]
async fn password_change_uses_self_reset_then_password_and_never_issues_an_admin_link() {
    let (api,server)=mock(vec![(200,json!({"response":"User password is set to reset"})),(200,json!({"response":"saved"}))]).await;
    api.change_own_password(ID.parse().unwrap(),"NewSecret!9").await.unwrap();let requests=server.await.unwrap();
    assert_eq!(requests[0].method,"POST");assert_eq!(requests[0].url.path(),format!("/users/{ID}/reset"));assert!(requests[0].body.is_empty());
    assert_eq!(requests[1].method,"PUT");assert_eq!(requests[1].url.path(),format!("/users/{ID}/password"));assert_eq!(serde_json::from_str::<Value>(&requests[1].body).unwrap(),json!({"password":"NewSecret!9"}));
}
#[tokio::test]
async fn recovery_sends_secrets_in_body_and_redacts_untrusted_error_details() {
    let (api,server)=mock(vec![(200,json!({"response":"saved"})),(410,json!({"type":"https://test/reset-link-invalid","detail":"TOKEN_SECRET password_secret"})),(400,json!({"type":"https://test/invalid-param","detail":"TOKEN_SECRET password_secret"}))]).await;
    api.recover_password("https://papo.cyberasilo.online/passwordchange/TOKEN_SECRET","Password!9").await.unwrap();
    for _ in 0..2{let error=api.recover_password("TOKEN_SECRET","password_secret").await.unwrap_err();assert!(!format!("{error:?}").contains("SECRET"));assert!(!error.to_string().contains("password_secret"));}
    let requests=server.await.unwrap();for r in requests{assert_eq!(r.url.path(),"/auth/password_reset");assert!(r.url.query().is_none());assert_eq!(serde_json::from_str::<Value>(&r.body).unwrap()["token"],"TOKEN_SECRET");}
}
#[test]
fn recovery_rejects_bad_links_and_debug_output_hides_credentials(){
    for input in ["", "ftp://example.test/passwordchange/token", "https://user:pass@example.test/passwordchange/token","https://example.test/passwordchange/token/extra","https://example.test/","token\nother"]{assert!(security::recovery_token(input).is_err());}
    assert_eq!(security::recovery_token(" https://papo.cyberasilo.online/passwordchange/Ab_-12 ").unwrap(),"Ab_-12");
    assert_eq!(security::recovery_token("https://papo.test/reset?token=Ab_-12").unwrap(),"Ab_-12");
    assert!(!format!("{:?}",crate::ui::login::LoginMsg::SetPassword("hidden_secret".into())).contains("hidden_secret"));
}
#[tokio::test]
async fn connected_devices_and_revocation_match_auth_contract(){
    let (api,server)=mock(vec![(200,json!({"connections":[{"id":ID,"created_at":DATE,"expires_at":DATE}]})),(200,json!({"dropped":1})),(200,json!({"dropped":2}))]).await;
    assert_eq!(api.connected_devices().await.unwrap().len(),1);assert_eq!(api.drop_connection(ID).await.unwrap().dropped,1);assert_eq!(api.drop_connection("ALL").await.unwrap().dropped,2);
    let requests=server.await.unwrap();assert_eq!(requests[0].url.path(),"/auth/connected_devices");for (r,id)in requests[1..].iter().zip([ID,"ALL"]){assert_eq!(r.url.path(),"/auth/drop_connection");assert_eq!(serde_json::from_str::<Value>(&r.body).unwrap(),json!({"connection_id":id}));}
}
#[test]
fn search_validates_real_filters_dates_and_optional_false(){
    assert!(SearchRequest::default().validate().is_err());assert!(SearchRequest{order:"desc".into(),..Default::default()}.validate().is_err());
    assert!(SearchRequest{contains_attachment:Some(false),..Default::default()}.validate().is_ok());
    for request in [SearchRequest{channel_id:"invalid".into(),..Default::default()},SearchRequest{date_start:"2026-02-30".into(),..Default::default()},SearchRequest{date_start:"2026-10-06".into(),date_end:"2026-10-05".into(),..Default::default()},SearchRequest{has:"attachment".into(),..Default::default()}]{assert!(request.validate().is_err());}
}
#[tokio::test]
async fn search_encodes_every_filter_and_equal_timestamp_cursor(){
    let result=json!({"type":"message","id":ID,"content":null,"channel_id":CHANNEL,"channel_name":"general","author_id":null,"author_username":null,"created_at":DATE});
    let (api,server)=mock(vec![(200,json!({"results":[result.clone()],"has_more":true})),(200,json!({"results":[result],"has_more":false}))]).await;
    let request=SearchRequest{text:"hello".into(),author:ID.into(),channel_id:CHANNEL.into(),mention:ID.into(),has:"link".into(),contains_attachment:Some(false),date_start:"2026-10-01".into(),date_end:"2026-10-06".into(),order:"asc".into()};
    let first=api.search(&request,None).await.unwrap();let cursor=MessageCursor{created_at:first.results[0].created_at,id:first.results[0].id};api.search(&request,Some(cursor)).await.unwrap();
    let requests=server.await.unwrap();assert_eq!(requests[0].method,"POST");assert_eq!(requests[0].url.path(),"/search");assert!(requests[0].url.query().is_none());assert_eq!(serde_json::from_str::<Value>(&requests[0].body).unwrap(),serde_json::to_value(request).unwrap());
    let query:std::collections::HashMap<_,_>=requests[1].url.query_pairs().into_owned().collect();assert_eq!(query["last_id"],ID);assert_eq!(query["since"].parse::<chrono::DateTime<chrono::Utc>>().unwrap(),cursor.created_at);assert_eq!(cursor.created_at.timestamp_subsec_nanos(),123456789);
}
#[tokio::test]
async fn notification_pages_and_read_wrapper_are_separate_from_channel_settings(){
    let notice=json!({"id":ID,"message_id":CHANNEL,"channel_id":CHANNEL,"author_id":null,"message_content":"Hi","read":false,"created_at":DATE});
    let(api,server)=mock(vec![(200,json!({"notifications":[notice.clone()],"has_more":true})),(200,json!({"notifications":[notice],"has_more":false})),(200,json!({"updated":1}))]).await;
    let user=ID.parse().unwrap();let first=api.notifications(user,None).await.unwrap();let n=&first.notifications[0];api.notifications(user,Some(MessageCursor{created_at:n.created_at,id:n.id})).await.unwrap();assert_eq!(api.read_notifications(user,&[n.id]).await.unwrap(),1);
    let requests=server.await.unwrap();assert_eq!(requests[0].url.path(),format!("/users/{ID}/notifications"));let query:std::collections::HashMap<_,_>=requests[1].url.query_pairs().into_owned().collect();assert_eq!(query["last_id"],ID);assert_eq!(query["order"],"desc");assert_eq!(requests[2].method,"PUT");assert_eq!(requests[2].url.path(),format!("/users/{ID}/read_notification"));assert_eq!(serde_json::from_str::<Value>(&requests[2].body).unwrap(),json!({"notification_ids":[ID]}));
}

fn dm()->Value {json!({"id":CHANNEL,"user":{"id":ID,"username":"peer","created_at":DATE},"created_at":DATE,"last_message":null,"last_read_message":null,"last_read_at":null,"unread_count":0})}
#[tokio::test]
async fn direct_routes_accept_existing_and_created_and_preserve_blocked_session(){
    let own=Uuid::nil();let peer=ID.parse().unwrap();let id=CHANNEL.parse().unwrap();
    let (api,server)=mock(vec![(200,json!({"dms":[dm()]})),(201,dm()),(200,dm()),(200,dm()),(204,Value::Null),
        (403,json!({"type":"https://test/dm-blocked","detail":"Peer blocked"})),(404,json!({"detail":"Not a participant"}))]).await;
    assert!(api.open_direct(own,own).await.is_err());
    assert_eq!(api.list_direct().await.unwrap()[0].id,id);
    assert_eq!(api.open_direct(own,peer).await.unwrap().id,id);
    assert_eq!(api.open_direct(own,peer).await.unwrap().id,id);
    assert_eq!(api.get_direct(id).await.unwrap().user.id,peer);
    api.hide_direct(id).await.unwrap();let e=api.open_direct(own,peer).await.unwrap_err();assert!(is_permission_error(&e));assert!(!is_session_error(&e));
    assert!(!is_session_error(&api.get_direct(id).await.unwrap_err()));
    let requests=server.await.unwrap();assert_eq!(requests.iter().map(|r|r.method.as_str()).collect::<Vec<_>>(),["GET","POST","POST","GET","DELETE","POST","GET"]);
    assert_eq!(requests[0].url.path(),"/dms");assert_eq!(requests[3].url.path(),format!("/dms/{CHANNEL}"));assert_eq!(serde_json::from_str::<Value>(&requests[1].body).unwrap(),json!({"user_id":ID}));
}
#[tokio::test]
async fn blocking_contract_uses_post_and_delete_and_never_blocks_self(){
    let (api,server)=mock(vec![(200,json!({"users":[{"id":ID,"username":"peer","created_at":DATE}]})),(204,Value::Null),(204,Value::Null)]).await;
    assert!(api.block_user(Uuid::nil(),Uuid::nil(),true).await.is_err());assert_eq!(api.list_blocks().await.unwrap().len(),1);
    api.block_user(Uuid::nil(),ID.parse().unwrap(),true).await.unwrap();api.block_user(Uuid::nil(),ID.parse().unwrap(),false).await.unwrap();
    let r=server.await.unwrap();assert_eq!(r.iter().map(|r|(r.method.as_str(),r.url.path())).collect::<Vec<_>>(),[("GET","/users/blocks"),("POST","/users/12345678-1234-4234-8234-123456789abc/block"),("DELETE","/users/12345678-1234-4234-8234-123456789abc/block")]);
}
fn role()->Value {json!({"id":ID,"name":"Moderator","color":"#AABBCC","permissions":{"manage_roles":true},"created_at":DATE})}
#[tokio::test]
async fn all_role_management_routes_use_complete_permission_objects(){
    let role_id=ID.parse().unwrap();let user=CHANNEL.parse().unwrap();
    let p:RolePermissions=serde_json::from_value(json!({"manage_server":false,"manage_channels":true,"manage_roles":true,"ban_members":false,"pin_message":true,"everyone_message":false,"send_attachment":true})).unwrap();
    let create=CreateRoleRequest{name:"Moderator".into(),color:Some("#AABBCC".into()),permissions:Some(p.clone())};
    let (api,server)=mock(vec![(200,json!({"roles":[role()]})),(201,role()),(200,role()),(201,json!({"user_id":CHANNEL,"role_id":ID,"assigned_at":DATE})),(204,Value::Null),(204,Value::Null)]).await;
    assert_eq!(api.list_roles().await.unwrap().len(),1);api.create_role(&create).await.unwrap();api.update_role(role_id,&UpdateRoleRequest{name:"Moderator".into(),color:None,permissions:Some(p)}).await.unwrap();api.assign_role(user,role_id).await.unwrap();api.remove_role(user,role_id).await.unwrap();api.delete_role(role_id).await.unwrap();
    let r=server.await.unwrap();assert_eq!(r.iter().map(|r|r.method.as_str()).collect::<Vec<_>>(),["GET","POST","PUT","POST","DELETE","DELETE"]);let body:Value=serde_json::from_str(&r[1].body).unwrap();assert_eq!(body["permissions"].as_object().unwrap().len(),7);assert_eq!(body["permissions"]["everyone_message"],false);assert!(serde_json::from_str::<Value>(&r[2].body).unwrap()["color"].is_null());assert_eq!(r[4].url.path(),format!("/users/{CHANNEL}/roles/{ID}"));
}
#[tokio::test]
async fn administration_preserves_omitted_fields_and_uses_flat_position_contract(){
    let server_value=json!({"id":ID,"name":"Test","created_at":DATE,"public":true});
    let channel=json!({"id":CHANNEL,"name":"general","type":"voice","position":2,"created_at":DATE});
    let (api,server)=mock(vec![(200,server_value.clone()),(200,server_value),(201,channel.clone()),(200,channel.clone()),(200,channel),(200,json!({})),(204,Value::Null),(204,Value::Null)]).await;
    api.create_server(&ServerWrite{name:Some("Test".into()),public:Some(true),..Default::default()}).await.unwrap();
    api.patch_server(&ServerWrite{name:Some("Renamed".into()),..Default::default()}).await.unwrap();
    let id=CHANNEL.parse().unwrap();let role=ID.parse().unwrap();assert_eq!(api.create_channel(&CreateChannelRequest{name:"voice".into(),channel_type:Some(ChannelType::Voice),topic:None}).await.unwrap().channel_type,Some(ChannelType::Voice));
    api.update_channel(id,&UpdateChannelRequest{name:"renamed".into(),topic:Some(String::new())}).await.unwrap();api.move_channel(id,&ChangeChannelPositionRequest{old_position:1,new_position:2}).await.unwrap();
    api.set_override(id,role,&ChannelPermissions{read_channel:Some(true),send_messages:Some(false),delete_messages:Some(false),connect_voice:Some(true)}).await.unwrap();api.remove_override(id,role).await.unwrap();api.delete_channel(id).await.unwrap();
    let r=server.await.unwrap();assert_eq!(r[1].method,"PATCH");assert_eq!(serde_json::from_str::<Value>(&r[1].body).unwrap(),json!({"name":"Renamed"}));assert_eq!(serde_json::from_str::<Value>(&r[2].body).unwrap()["type"],"voice");assert_eq!(serde_json::from_str::<Value>(&r[4].body).unwrap(),json!({"old_position":1,"new_position":2}));assert_eq!(r[6].url.path(),format!("/channels/{CHANNEL}/role/{ID}"));
}
#[tokio::test]
async fn server_password_errors_keep_status_without_echoing_secrets(){
    let (api,server)=mock(vec![(400,json!({"type":"https://test/invalid-param","detail":"Secret123! rejected"}))]).await;
    let error=api.patch_server(&ServerWrite{password:Some("Secret123!".into()),public:Some(false),..Default::default()}).await.unwrap_err();assert!(!format!("{error:?}").contains("Secret123!"));assert!(!is_session_error(&error));assert_eq!(error.downcast_ref::<ApiError>().unwrap().code.as_deref(),Some("invalid-param"));server.await.unwrap();
}

#[tokio::test]
async fn moderation_uses_url_target_and_preserves_false_ban_state() {
    let (client,server)=mock(vec![(200,json!({})),(200,json!({})),(200,json!({"reset_url":"https://test/reset?token=SECRET","expires_at":"2099-10-03T12:00:00Z"}))]).await;
    let user=CHANNEL.parse().unwrap();client.set_user_banned(user,true).await.unwrap();client.set_user_banned(user,false).await.unwrap();
    assert!(client.create_recovery_link(user,user).await.is_err());let link=client.create_recovery_link(ID.parse().unwrap(),user).await.unwrap();assert!(!format!("{link:?}").contains("SECRET"));
    let requests=server.await.unwrap();for (i,r) in requests[..2].iter().enumerate(){assert_eq!(r.method,"PUT");assert_eq!(r.url.path(),format!("/users/{CHANNEL}/ban"));assert_eq!(serde_json::from_str::<Value>(&r.body).unwrap(),json!({"ban_state":i==0}));}
    assert_eq!(requests[2].method,"POST");assert_eq!(requests[2].url.path(),format!("/users/{CHANNEL}/reset"));assert!(requests[2].body.is_empty());
}
#[tokio::test]
async fn audit_filters_keep_nanoseconds_and_cursor_order() {
    let entry=json!({"id":CHANNEL,"actor_username":"alice","action":"user.ban","entity_type":"user","target_user_id":ID,"metadata":{"token":"SECRET"},"created_at":DATE});
    let (client,server)=mock(vec![(200,json!({"logs":[entry],"has_more":true})),(200,json!({"logs":[],"has_more":false}))]).await;
    let f=AuditFilter{action:"user.ban".into(),actor_id:Some(ID.parse().unwrap()),entity_type:"user".into(),since:Some(DATE.parse().unwrap()),until:Some("2099-10-03T12:00:00Z".parse().unwrap()),ascending:true};
    let first=client.audit_logs(&f,None).await.unwrap();assert!(!format!("{first:?}").contains("SECRET"));client.audit_logs(&f,Some(first.logs[0].id)).await.unwrap();
    let requests=server.await.unwrap();let q:std::collections::HashMap<_,_>=requests[1].url.query_pairs().into_owned().collect();assert_eq!(q["order"],"asc");assert_eq!(q["last_id"],CHANNEL);assert_eq!(q["actor_id"],ID);assert_eq!(q["action"],"user.ban");assert_eq!(q["entity_type"],"user");assert_eq!(q["since"],DATE);assert_eq!(requests[0].method,"GET");assert_eq!(requests[0].url.path(),"/admin/audit-logs");
}
#[tokio::test]
async fn audit_rejects_invalid_ranges_and_nonadvancing_pages() {
    let (client,server)=mock(vec![(200,json!({"logs":[],"has_more":true}))]).await;
    let f=AuditFilter{since:Some("2099-10-03T12:00:00Z".parse().unwrap()),until:Some(DATE.parse().unwrap()),..Default::default()};assert!(client.audit_logs(&f,None).await.is_err());assert!(client.audit_logs(&AuditFilter::default(),None).await.unwrap_err().to_string().contains("progresso"));server.await.unwrap();
}
#[tokio::test]
async fn recovery_errors_do_not_echo_tokens_and_keep_session_status() {
    let (client,server)=mock(vec![(403,json!({"type":"https://test/banned","detail":"SECRET"}))]).await;
    let error=client.create_recovery_link(ID.parse().unwrap(),CHANNEL.parse().unwrap()).await.unwrap_err();assert!(is_session_error(&error));assert!(!format!("{error:?}").contains("SECRET"));server.await.unwrap();
}
#[tokio::test]
async fn voice_ice_credentials_are_ephemeral_and_redacted() {
    let (client,server)=mock(vec![(200,json!({"ice_servers":[{"urls":["stun:stun.test:3478"]},{"urls":["turn:turn.test:3478?transport=udp","turns:turn.test:5349?transport=tcp"],"username":"ephemeral-user","credential":"SECRET"}]})),(401,json!({"detail":"expired"}))]).await;
    let config=client.voice_ice_servers().await.unwrap();assert_eq!(config.ice_servers[1].credential.as_deref(),Some("SECRET"));assert!(!format!("{config:?}").contains("SECRET"));assert!(!format!("{config:?}").contains("ephemeral-user"));assert!(is_session_error(&client.voice_ice_servers().await.unwrap_err()));let requests=server.await.unwrap();assert_eq!(requests[0].method,"GET");assert_eq!(requests[0].url.path(),"/voice/ice-servers");
}
