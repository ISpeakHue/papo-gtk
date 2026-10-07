//! Runs on the existing GTK smoke-test thread, exercising real controls and HTTP.
use super::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const USER: &str = "12345678-1234-4234-8234-123456789abc";
const CHANNEL: &str = "12345678-1234-4234-8234-123456789abd";
const MESSAGE: &str = "12345678-1234-4234-8234-123456789abe";
const OLDER: &str = "12345678-1234-4234-8234-123456789abf";
const EMOJI: &str = "12345678-1234-4234-8234-123456789ac0";
const DATE: &str = "2026-10-03T12:00:00Z";
pub(crate) struct Backend {
    pub(crate) peer_banned:bool,pub(crate) fail_moderation:bool,pub(crate) fail_audit:bool,
    pub(crate) server_missing:bool, pub(crate) server_value:Value, pub(crate) admin_roles:Vec<Value>, pub(crate) admin_channels:Vec<Value>, pub(crate) dms:Vec<Value>, dm_messages:Vec<Value>, pub(crate) blocks:Vec<Value>, pub(crate) fail_role:bool, pub(crate) deny_dm:bool, pub(crate) fail_server:bool,
    message: Value, old: Value, groups: Vec<Value>, emojis: Vec<Value>, pinned: bool,
    pub(crate) profile:Value, config:Value, channel_setting:Value, fail_profile:bool, fail_settings:bool, fail_upload:bool,
    fail_password:bool, fail_recovery:bool, fail_search:bool, fail_read:bool, revoked:bool, dropped:Vec<String>, read_notices:Vec<Value>,
    fail_edit: bool, deny_edit: bool, fail_emoji: bool, pub(crate) requests: Vec<(String, String, Value)>, restricted: bool,
}
pub(crate) fn descendants(w: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut found = vec![w.clone()]; let mut child = w.first_child();
    while let Some(w) = child { found.extend(descendants(&w)); child = w.next_sibling(); } found
}
pub(crate) fn find_button(w: &gtk::Widget, text: &str) -> gtk::Button {
    descendants(w).into_iter().find_map(|w| w.downcast::<gtk::Button>().ok().filter(|b| b.label().as_deref() == Some(text) || b.tooltip_text().as_deref() == Some(text)))
        .unwrap_or_else(|| panic!("missing button: {text}"))
}
pub(crate) fn pump(context: &gtk::glib::MainContext) { for _ in 0..500 { if !context.pending() { break; } context.iteration(false); } }
#[track_caller]
pub(crate) fn until(context: &gtk::glib::MainContext, condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop { pump(context); if condition() { return; } assert!(std::time::Instant::now() < deadline,"GTK workflow timed out"); std::thread::sleep(std::time::Duration::from_millis(5)); }
}
pub(crate) async fn mock() -> (ApiClient, Arc<Mutex<Backend>>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = ApiClient::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let state = Arc::new(Mutex::new(Backend {
        peer_banned:false,fail_moderation:true,fail_audit:false,
        server_missing:false,server_value:json!({"id":CHANNEL,"name":"Test","owner_id":USER,"public":true,"created_at":DATE}),admin_roles:vec![],admin_channels:vec![json!({"id":CHANNEL,"name":"general","type":"text","position":1,"created_at":DATE}),json!({"id":"12345678-1234-4234-8234-123456789ac2","name":"other","type":"text","position":2,"created_at":DATE})],dms:vec![],dm_messages:vec![],blocks:vec![],fail_role:true,deny_dm:false,fail_server:true,
        message: json!({"id":MESSAGE,"channel_id":CHANNEL,"author_id":USER,"content":"Original","created_at":DATE,"user_reactions":[],"reactions":[]}),
        old: json!({"id":OLDER,"channel_id":CHANNEL,"author_id":USER,"content":"Older message","created_at":"2026-10-02T00:00:00Z"}),
        channel_setting:json!("all"), profile:json!({"id":USER,"username":"alice","nickname":"Alice","description":"Old description","created_at":DATE}), config:serde_json::to_value(crate::models::UserConfig::default()).unwrap(),fail_profile:true,fail_settings:true,fail_upload:true,
        fail_password:true,fail_recovery:true,fail_search:true,fail_read:true,revoked:false,dropped:vec![],read_notices:vec![],
        groups: vec![], emojis: vec![], pinned:false, fail_edit:true, deny_edit:false, fail_emoji:true, requests:vec![], restricted:false,
    }));
    let shared = state.clone(); let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream,_)) = listener.accept().await else { break; };
            let shared = shared.clone(); tokio::spawn(async move {
                let mut bytes = Vec::new(); let mut chunk = [0;4096];
                let (method,path,body) = loop {
                    let Ok(size) = stream.read(&mut chunk).await else { return; }; if size == 0 { return; } bytes.extend_from_slice(&chunk[..size]);
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length: usize = headers.lines().filter_map(|l| l.split_once(':')).find(|(k,_)| k.eq_ignore_ascii_case("content-length"))
                            .map(|(_,v)| v.trim().parse().unwrap()).unwrap_or(0);
                        if bytes.len() >= end+4+length {
                            let mut line = headers.lines().next().unwrap().split_whitespace();
                            let method = line.next().unwrap().to_owned(); let path = line.next().unwrap().to_owned();
                            let body = serde_json::from_slice(&bytes[end+4..end+4+length]).unwrap_or_else(|_|json!(String::from_utf8_lossy(&bytes[end+4..end+4+length]))); break (method,path,body);
                        }
                    }
                };
                let (status,response) = {
                    let mut b = shared.lock().unwrap(); b.requests.push((method.clone(),path.clone(),body.clone()));
                    let route = path.split('?').next().unwrap();
                    let mut peer=json!({"id":EMOJI,"username":"bob","nickname":"Bob","avatar_blob":tiny_png_base64(),"created_at":DATE});peer["banned"]=json!(b.peer_banned);
                    if route=="/voice/ice-servers"{(200,json!({"ice_servers":[]}))}
                    else if route=="/admin/audit-logs"{
                        if b.fail_audit{b.fail_audit=false;(500,json!({"detail":"Audit retry"}))}else{let more=!path.contains("last_id");(200,json!({"logs":[{"id":if more{MESSAGE}else{OLDER},"actor_username":"alice","action":"user.ban","entity_type":"user","target_user_id":EMOJI,"metadata":{"password":"never-display"},"created_at":DATE}],"has_more":more}))}
                    } else if route==format!("/users/{EMOJI}/ban"){
                        if b.fail_moderation{b.fail_moderation=false;(500,json!({"detail":"Moderation retry"}))}else{b.peer_banned=body["ban_state"].as_bool().unwrap();(200,json!({"response":"saved"}))}
                    } else if route==format!("/users/{EMOJI}/reset"){(200,json!({"reset_url":"https://test/passwordchange/issued-token","expires_at":"2099-10-03T12:00:00Z"}))}
                    else if route == "/dms" && method=="GET" {(200,json!({"dms":if b.deny_dm||!b.blocks.is_empty(){vec![]}else{b.dms.clone()}}))}
                    else if route == "/dms" && method=="POST" {
                        if b.deny_dm||!b.blocks.is_empty(){(403,json!({"type":"https://test/dm-blocked","detail":"DM blocked"}))}else{let existed=!b.dms.is_empty();if !existed{b.dms.push(json!({"id":"12345678-1234-4234-8234-123456789ac3","user":peer,"created_at":DATE,"unread_count":0}));}(if existed{200}else{201},b.dms[0].clone())}
                    } else if route.starts_with("/dms/") {
                        if method=="DELETE"{b.dms.clear();(204,Value::Null)}else if b.deny_dm||!b.blocks.is_empty(){(403,json!({"type":"https://test/dm-blocked","detail":"DM blocked"}))}else{b.dms.first().cloned().map(|d|(200,d)).unwrap_or((404,json!({"detail":"Unknown DM"})))}
                    } else if route=="/users/blocks" {(200,json!({"users":b.blocks}))}
                    else if route==format!("/users/{EMOJI}/block"){if method=="POST"{b.blocks=vec![peer];b.dms.clear();}else{b.blocks.clear();}(204,Value::Null)}
                    else if route==format!("/users/{EMOJI}/profile"){(200,peer)}
                    else if route.starts_with("/users/")&&route.ends_with("/roles")&&method=="POST"{b.profile["roles"]=json!([{ "id":body["role_id"],"name":"Moderator"}]);(201,json!({"user_id":USER,"role_id":body["role_id"],"assigned_at":DATE}))}
                    else if route.starts_with("/users/")&&route.contains("/roles/")&&method=="DELETE"{b.profile["roles"]=json!([]);(204,Value::Null)}
                    else if route.starts_with("/roles")&&method!="GET"{
                        if b.fail_role{b.fail_role=false;(500,json!({"detail":"Role failed"}))}else if method=="DELETE"{b.admin_roles.clear();(204,Value::Null)}else{let mut r=body.clone();r["id"]=json!(EMOJI);r["created_at"]=json!(DATE);b.admin_roles=vec![r.clone()];(if method=="POST"{201}else{200},r)}
                    } else if route=="/channels"&&method=="POST"{let mut c=body.clone();c["id"]=json!(Uuid::new_v4());c["position"]=json!(b.admin_channels.len()+1);c["created_at"]=json!(DATE);b.admin_channels.push(c.clone());(201,c)}
                    else if route.starts_with("/channels/")&&method=="PUT"&&route.contains("/permissions/"){let parts:Vec<_>=route.split('/').collect();let c=b.admin_channels.iter_mut().find(|c|c["id"]==parts[2]).unwrap();c["permissions"]=json!([{ "role_id":parts[4],"role_name":"Moderator","permissions":body["permissions"] }]);(200,json!({}))}
                    else if route.starts_with("/channels/")&&method=="DELETE"&&route.contains("/role/"){let parts:Vec<_>=route.split('/').collect();let c=b.admin_channels.iter_mut().find(|c|c["id"]==parts[2]).unwrap();c["permissions"]=json!([]);(204,Value::Null)}
                    else if route.starts_with("/channels/")&&method=="PUT"&&route.ends_with("/change_position"){let id=route.split('/').nth(2).unwrap();let old=body["old_position"].as_i64().unwrap();let new=body["new_position"].as_i64().unwrap();let c=b.admin_channels.iter().find(|c|c["id"]==id).unwrap();if c["position"]!=old{(409,json!({"detail":"Position conflict"}))}else{for c in &mut b.admin_channels{let p=c["position"].as_i64().unwrap();if c["id"]==id{c["position"]=json!(new);}else if new<old&&p>=new&&p<old{c["position"]=json!(p+1);}else if new>old&&p>old&&p<=new{c["position"]=json!(p-1);}}(200,b.admin_channels.iter().find(|c|c["id"]==id).unwrap().clone())}}
                    else if route.starts_with("/channels/")&&route.split('/').count()==3&&method=="PUT"{let id=route.split('/').nth(2).unwrap();let c=b.admin_channels.iter_mut().find(|c|c["id"]==id).unwrap();c["name"]=body["name"].clone();c["topic"]=body["topic"].clone();(200,c.clone())}
                    else if route.starts_with("/channels/")&&route.split('/').count()==3&&method=="DELETE"{let id=route.split('/').nth(2).unwrap();b.admin_channels.retain(|c|c["id"]!=id);(204,Value::Null)}
                    else if route=="/server"&&method=="POST"{b.server_missing=false;for key in ["name","public","icon_blob","icon_format"]{if let Some(value)=body.get(key){b.server_value[key]=value.clone();}}(200,b.server_value.clone())}
                    else if route=="/server"&&method=="PATCH"{if b.fail_server{b.fail_server=false;(500,json!({"detail":"Server failed"}))}else{for key in ["name","public","icon_blob","icon_format"]{if let Some(value)=body.get(key){b.server_value[key]=value.clone();}}if body.get("password").is_some(){b.revoked=true;}(200,b.server_value.clone())}}
                    else if route == "/auth/password_reset" {
                        if b.fail_recovery{b.fail_recovery=false;(410,json!({"type":"https://test/reset-link-invalid","detail":"secret invalid"}))}else{(200,json!({"response":"saved"}))}
                    } else if route == format!("/users/{USER}/reset"){(200,json!({"response":"User password is set to reset"}))}
                    else if route == format!("/users/{USER}/password"){
                        if b.fail_password{b.fail_password=false;(400,json!({"type":"https://test/invalid-param","detail":"password rejected"}))}else{(200,json!({"response":"saved"}))}
                    } else if route == "/auth/connected_devices" {(200,json!({"connections":([USER,EMOJI].into_iter().filter(|id|!b.dropped.iter().any(|d|d==id)).map(|id|json!({"id":id,"created_at":DATE,"expires_at":"2027-01-01T00:00:00Z"})).collect::<Vec<_>>())}))}
                    else if route == "/auth/drop_connection" {let id=body["connection_id"].as_str().unwrap().to_owned();b.revoked=id==USER||id=="ALL";b.dropped.push(id);(200,json!({"dropped":1}))}
                    else if route == "/search" {
                        if b.fail_search{b.fail_search=false;(500,json!({"detail":"Search failed"}))}else{
                            let item=|message:&Value|json!({"type":"message","id":message["id"],"content":message["content"],"channel_id":CHANNEL,"channel_name":"general","author_id":USER,"author_username":"alice","created_at":message["created_at"]});
                            (200,json!({"results":if path.contains("last_id"){vec![item(&b.message),item(&b.old)]}else{vec![item(&b.message)]},"has_more":!path.contains("last_id")}))
                        }
                    } else if route == format!("/users/{USER}/notifications") {
                        let id=if path.contains("last_id"){MESSAGE}else{EMOJI};
                        (200,json!({"notifications":[{"id":id,"message_id":OLDER,"channel_id":CHANNEL,"author_id":USER,"message_content":"notification preview","read":b.read_notices.contains(&json!(id)),"created_at":if id==EMOJI{DATE}else{"2026-10-02T00:00:00Z"}}],"has_more":id==EMOJI}))
                    } else if route == format!("/users/{USER}/read_notification") {
                        if b.fail_read{b.fail_read=false;(500,json!({"detail":"Read failed"}))}else{b.read_notices.extend(body["notification_ids"].as_array().unwrap().clone());(200,json!({"updated":1}))}
                    } else if route == "/messages" && method=="POST" {
                        if b.fail_upload{b.fail_upload=false;(500,json!({"detail":"Upload failed"}))}else{let mut message=b.message.clone();message["id"]=json!("12345678-1234-4234-8234-123456789ac1");if body.as_str().is_some_and(|s|s.contains("12345678-1234-4234-8234-123456789ac3")){message["channel_id"]=json!("12345678-1234-4234-8234-123456789ac3");message["content"]=json!("DM test");b.dm_messages.push(message.clone());}(201,message)}
                    } else if route == "/users/settings" && method=="PUT" {
                        if b.fail_settings{b.fail_settings=false;(500,json!({"detail":"Settings failed"}))}else{b.config=body["config"].clone();(200,json!({"user_id":USER,"version":1,"config":b.config,"updated_at":DATE}))}
                    } else if route == format!("/users/{USER}/profile") {(200,b.profile.clone())}
                    else if route == format!("/users/{USER}") && method=="PUT" {
                        if b.fail_profile{b.fail_profile=false;(500,json!({"detail":"Profile failed"}))}else{for key in ["nickname","description","typing"]{b.profile[key]=body[key].clone();}b.profile["status_message"]=body["status"].clone();(200,json!({"response":"saved"}))}
                    } else if route == format!("/users/{USER}/status") {b.profile["status"]=body["status"].clone();(200,json!({"response":"saved"}))}
                    else if route == format!("/users/{USER}/avatar") {b.profile["avatar_blob"]=Value::Null;(200,json!({"response":"saved"}))}
                    else if route == format!("/users/{USER}/banner") {b.profile["banner_media"]=Value::Null;(200,json!({"response":"saved"}))}
                    else if route.ends_with("/settings") && method=="POST" {b.channel_setting=body["notification_settings"].clone();(200,json!({"response":"saved"}))}
                    else if route.starts_with("/attachments/")&&route.split('/').count()==3{(200,Value::Null)}
                    else if route.starts_with("/link-previews/") {(200,json!({"id":EMOJI,"url":"https://example.test/","kind":"image","fetched_at":"2026-10-03T00:00:00Z","image_mime_type":"image/png","image_data":tiny_png_base64(),"video_url":"https://example.test/video.mp4","provider_name":"X","title":"g1 (@g1)","description":"Elefante-marinho boceja e coça a cabeça durante soneca em praia de SC"}))}
                    else if route == format!("/messages/{MESSAGE}") && method == "PUT" {
                        if b.deny_edit { (403,json!({"type":"https://test/forbidden","detail":"Editing denied"})) }
                        else if b.fail_edit { b.fail_edit = false; (500,json!({"detail":"Try again"})) }
                        else { b.message["content"] = body["content"].clone(); b.message["edited_at"] = json!("2026-10-03T12:30:00Z"); (200,b.message.clone()) }
                    } else if route == format!("/messages/{MESSAGE}") && method == "DELETE" { (204,Value::Null) }
                    else if route.ends_with("/pinned") {
                        let channel=route.split('/').nth(2).unwrap();
                        (200,json!({"channel_id":channel,"pinned":if b.pinned {if channel==CHANNEL{vec![b.message.clone()]}else{b.dm_messages.clone()}}else{vec![]}}))
                    }
                    else if route.ends_with("/pin") { b.pinned = method == "POST"; (204,Value::Null) }
                    else if route.ends_with("/reactions") {
                        if method == "POST" { b.groups = vec![json!({"emoji_id":body["emoji_id"],"unicode":body["unicode"],"count":1,"users":[{"id":USER,"user_id":USER,"created_at":DATE}]})]; }
                        if method == "DELETE" { b.groups.clear(); }
                        if method == "GET" { (200,json!({"message_id":MESSAGE,"reactions":b.groups,"has_more":false})) } else { (204,Value::Null) }
                    } else if route == "/emojis" {
                        if method == "POST" && b.fail_emoji { b.fail_emoji = false; (409,json!({"type":"https://test/emoji-name-taken","detail":"Duplicate emoji name"})) }
                        else if method == "POST" { b.emojis = vec![json!({"id":EMOJI,"name":body["name"],"image_blob":body["image_blob"],"format":body["format"],"created_by":USER,"created_at":DATE})]; (201,b.emojis[0].clone()) }
                        else { (200,json!({"emojis":b.emojis,"has_more":false})) }
                    } else if route == format!("/emojis/{EMOJI}") { b.emojis.clear(); (204,Value::Null) }
                    else if route=="/channels/12345678-1234-4234-8234-123456789ac3/messages" {
                        if b.deny_dm||!b.blocks.is_empty(){(403,json!({"type":"https://test/dm-blocked","detail":"DM blocked"}))}else{(200,json!({"channel_id":"12345678-1234-4234-8234-123456789ac3","messages":b.dm_messages,"has_more":false}))}
                    }
                    else if route.ends_with("/messages") {
                        (200,json!({"channel_id":CHANNEL,"messages":if path.contains("last_id") {vec![b.old.clone()]} else {vec![b.message.clone()]},"has_more":!path.contains("last_id")}))
                    } else if route == "/auth/whoami" && b.revoked {(401,json!({"type":"https://test/unauthorized","detail":"session ended"}))}
                    else if route == "/auth/whoami" {let mut profile=b.profile.clone();profile["settings"]=json!({"version":1,"config":b.config});(200,profile)}
                    else if route == "/server" {if b.server_missing{(404,json!({"detail":"No server"}))}else{let mut server=b.server_value.clone();if b.restricted{server["owner_id"]=Value::Null;}(200,server)}}
                    else if route == "/roles" { (200,json!({"roles":b.admin_roles})) }
                    else if route == "/channels" {let mut channels=b.admin_channels.clone();for c in &mut channels{if c["id"]==CHANNEL{c["notification_settings"]=b.channel_setting.clone();}}(200,json!({"channels":channels}))}
                    else if route.ends_with("/permissions") { (200,json!({"channel_id":route.split('/').nth(2).unwrap(),"permissions":if b.restricted {vec![json!({"role_id":EMOJI,"role_name":"restricted","permissions":{"read_channel":false}})]} else {b.admin_channels.iter().find(|c|c["id"]==route.split('/').nth(2).unwrap()).and_then(|c|c["permissions"].as_array()).cloned().unwrap_or_default()}})) }
                    else if route == "/users" { (200,json!({"users":[b.profile.clone(),peer],"has_more":false})) }
                    else if route == "/users/user_summary_batch" {let users:Vec<_>=vec![b.profile.clone(),peer].into_iter().filter(|u|body["ids"].as_array().is_some_and(|ids|ids.contains(&u["id"]))).collect();(200,json!(users))}
                    else if route == "/users/profile_batch" {let mut p=peer.clone();p["avatar_blob"]=json!(tiny_png_base64());(200,json!({"profiles":[p]})) }
                    else { (404,json!({"detail":"No such endpoint"})) }
                };
                let binary=path.ends_with("/video")||(path.starts_with("/attachments/")&&path.split('/').count()==3);
                let data=if binary{include_bytes!("../../../../tests/fixtures/chat-video.webm").to_vec()}else if status==204{Vec::new()}else{response.to_string().into_bytes()};
                let content_type=if binary{"video/webm"}else{"application/json"};let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",data.len());let mut response=header.into_bytes();response.extend_from_slice(&data);
                let _ = stream.write_all(&response).await;
            });
        }
    }); (api,state,task)
}

pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    let (api,backend,task) = tokio::runtime::Handle::current().block_on(mock());
    let user = USER.parse().unwrap(); let channel_id = CHANNEL.parse().unwrap();
    let channel: Channel = serde_json::from_value(json!({"id":CHANNEL,"name":"general","type":"text","created_at":DATE})).unwrap();
    let message: Message = serde_json::from_value(backend.lock().unwrap().message.clone()).unwrap();
    let chat = ChatModel::builder().launch(ChatInit { api: Some(api.clone()), user_id:Some(user), ..Default::default() }).detach();
    chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access::resolve(user,Some(user),&[],&[],&[]) });
    chat.emit(ChatMsg::SetChannel(channel.clone())); chat.emit(ChatMsg::AddMessage(message.clone()));
    until(context,|| chat.model().actions.pins_ready);
    assert!(find_button(chat.widget().upcast_ref(),"Editar").get_visible());
    find_button(chat.widget().upcast_ref(),"Editar").emit_clicked();
    pump(context);
    let token = chat.model().actions.edit.as_ref().unwrap().token;
    chat.model().actions.edit.as_ref().unwrap().buffer.set_text("Changed 🌎");
    chat.model().actions.edit.as_ref().unwrap().save.emit_clicked();
    until(context,|| chat.model().actions.edit.as_ref().is_some_and(|e| !e.pending && !e.error.text().is_empty()));
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().buffer.text(&chat.model().actions.edit.as_ref().unwrap().buffer.start_iter(),&chat.model().actions.edit.as_ref().unwrap().buffer.end_iter(),false).as_str(),"Changed 🌎");
    chat.model().actions.edit.as_ref().unwrap().save.emit_clicked();
    until(context,|| chat.model().actions.edit.is_none());
    assert_eq!(chat.model().history.messages[0].content.as_deref(),Some("Changed 🌎"));
    // An old completion must not close a newer edit or clear another channel.
    chat.emit(ChatMsg::Action(ActionMsg::OpenEdit(message.clone()))); pump(context);
    let newer = chat.model().actions.edit.as_ref().unwrap().token;
    chat.emit(ChatMsg::Action(ActionMsg::EditFinished { epoch:chat.model().actions.epoch, token, result:Ok(message.clone()) })); pump(context);
    assert_eq!(chat.model().actions.edit.as_ref().unwrap().token,newer);
    chat.emit(ChatMsg::Action(ActionMsg::CancelEdit(newer))); pump(context);
    // Denied editing retains the form and session; parent receives a typed API error.
    backend.lock().unwrap().deny_edit = true;
    find_button(chat.widget().upcast_ref(),"Editar").emit_clicked(); pump(context);
    chat.model().actions.edit.as_ref().unwrap().save.emit_clicked();
    until(context,|| chat.model().actions.edit.as_ref().is_some_and(|e| !e.pending && e.error.text().contains("Editing denied")));
    let t = chat.model().actions.edit.as_ref().unwrap().token;
    chat.emit(ChatMsg::Action(ActionMsg::CancelEdit(t))); pump(context); backend.lock().unwrap().deny_edit = false;
    find_button(chat.widget().upcast_ref(),"Fixar").emit_clicked();
    until(context,|| chat.model().actions.is_pinned(message.id));
    find_button(chat.widget().upcast_ref(),"Fixadas").emit_clicked(); pump(context);
    assert!(chat.model().actions.pins_window.is_some());
    find_button(chat.widget().upcast_ref(),"Desafixar").emit_clicked();
    until(context,|| !chat.model().actions.is_pinned(message.id) && !chat.model().actions.busy.contains(&message.id));
    backend.lock().unwrap().pinned = true;
    chat.emit(ChatMsg::Action(ActionMsg::PinEvent { id:message.id,pinned:true }));
    until(context,|| chat.model().actions.is_pinned(message.id));
    backend.lock().unwrap().pinned = false;
    chat.emit(ChatMsg::Action(ActionMsg::PinEvent { id:message.id,pinned:false }));
    until(context,|| !chat.model().actions.is_pinned(message.id));
    // Incoming pins drive the same panel; delayed previous-channel responses are ignored.
    let epoch = chat.model().actions.epoch; let pin_request = Uuid::new_v4();

    chat.emit(ChatMsg::ClearChannel); pump(context);
    chat.emit(ChatMsg::Action(ActionMsg::PinsLoaded { epoch, token:pin_request, result:Ok(vec![message.clone()]) })); pump(context);
    assert!(chat.model().actions.pinned.is_empty());
    chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access::resolve(user,Some(user),&[],&[],&[]) });
    chat.emit(ChatMsg::SetChannel(channel)); chat.emit(ChatMsg::AddMessage(message.clone()));
    until(context,|| chat.model().actions.pins_ready);
    // Upload keeps its draft on error and custom images replace placeholder labels.
    find_button(chat.widget().upcast_ref(),"Emojis").emit_clicked(); pump(context);
    let manager_token = chat.model().actions.manager.as_ref().unwrap().token;
    let mut png = std::io::Cursor::new(Vec::new()); image::DynamicImage::new_rgba8(1,1).write_to(&mut png,image::ImageFormat::Png).unwrap();
    chat.emit(ChatMsg::Action(ActionMsg::EmojiFileLoaded { token:manager_token, result:Ok(png.into_inner()) })); pump(context);
    chat.model().actions.manager.as_ref().unwrap().name.set_text("cat");
    chat.model().actions.manager.as_ref().unwrap().upload.emit_clicked();
    until(context,|| chat.model().actions.manager.as_ref().is_some_and(|m| !m.pending && m.error.text().contains("Duplicate emoji name")));
    assert_eq!(chat.model().actions.manager.as_ref().unwrap().name.text().as_str(),"cat");
    assert!(chat.model().actions.manager.as_ref().unwrap().bytes.is_some());
    chat.model().actions.manager.as_ref().unwrap().name.set_text("cat2");
    chat.model().actions.manager.as_ref().unwrap().upload.emit_clicked();
    until(context,|| chat.model().actions.textures.contains_key(&EMOJI.parse().unwrap()));
    assert!(chat.model().actions.emojis[0].image_blob.is_none()); // no retained large blobs
    find_button(chat.widget().upcast_ref(),"Adicionar reação").emit_clicked(); pump(context);
    let picker = chat.model().actions.picker.as_ref().unwrap().0.clone();
    find_button(picker.upcast_ref(),"❤️").emit_clicked();
    until(context,|| chat.model().history.messages[0].user_reactions.as_ref().is_some_and(|v| !v.is_empty()) && !chat.model().actions.busy.contains(&message.id));
    backend.lock().unwrap().groups.clear();
    chat.emit(ChatMsg::ApplyChange(Change::Reaction(message.id,crate::models::MessageReactionSummary { emoji_id:None,unicode:Some("❤️".into()),count:0 })));
    chat.emit(ChatMsg::Action(ActionMsg::Reconcile(message.id)));
    until(context,|| chat.model().history.messages[0].user_reactions.as_ref().is_some_and(|v| v.is_empty()));
    // Re-add through the picker to test read-only own removal next.
    chat.emit(ChatMsg::ReactionClicked { message_id:message.id,emoji_id:None,unicode:Some("❤️".into()) });
    until(context,|| chat.model().history.messages[0].user_reactions.as_ref().is_some_and(|v| !v.is_empty()) && !chat.model().actions.busy.contains(&message.id));
    assert!(descendants(chat.widget().upcast_ref()).iter().filter_map(|w| w.downcast_ref::<gtk::Button>()).any(|b| b.has_css_class("own") && b.tooltip_text().as_deref() == Some("Remover minha reação")));
    let hint_key=(message.id,None,Some("❤️".to_owned()));assert!(chat.model().actions.reaction_hints.borrow().get(&hint_key).is_some_and(|text|text.contains("Você")&&text.contains("❤️")));
    let reaction=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.has_css_class("papo-reaction"))).unwrap();assert!(reaction.has_tooltip(),"reaction buttons must expose participant tooltips");
    // A cold hover fetches participant details once; repeated queries reuse them.
    let mut hovered=message.clone();hovered.reactions=Some(vec![crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:1}]);
    let cold=ChatModel::builder().launch(ChatInit{active_channel:Some(serde_json::from_value(json!({"id":CHANNEL,"name":"hover","type":"text","created_at":DATE})).unwrap()),messages:vec![hovered],api:Some(api.clone()),user_id:Some(user),..Default::default()}).detach();cold.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access{read:true,send:true,..Default::default()}});pump(context);
    let calls=backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="GET"&&path.split('?').next().unwrap().ends_with("/reactions")).count();
    let pill=descendants(cold.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.has_css_class("papo-reaction"))).unwrap();let controllers=pill.observe_controllers();let motion=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerMotion>()).unwrap();for _ in 0..3{motion.emit_by_name::<()>("enter",&[&0.0f64,&0.0f64]);}
    until(context,||cold.model().actions.reaction_hints.borrow().get(&hint_key).is_some_and(|text|text.contains("Você")));assert_eq!(backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="GET"&&path.split('?').next().unwrap().ends_with("/reactions")).count(),calls+1);
    let contextual=descendants(chat.widget().upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::Button>().ok()).find(|b|b.label().as_deref()==Some("Adicionar reação")).expect("context menu must offer reactions");contextual.emit_clicked();pump(context);assert!(chat.model().actions.picker.as_ref().is_some_and(|(w,_,_)|w.is_visible()));
    // Own reaction removal remains available in a read-only channel.
    chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access { read:true, ..Default::default() } }); pump(context);
    chat.emit(ChatMsg::ReactionClicked { message_id:message.id, emoji_id:None, unicode:Some("❤️".into()) });
    until(context,|| chat.model().history.messages[0].user_reactions.as_ref().is_some_and(|v| v.is_empty()) && !chat.model().actions.busy.contains(&message.id));
    assert!(!find_button(chat.widget().upcast_ref(),"Adicionar reação").is_sensitive());
    chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access::resolve(user,Some(user),&[],&[],&[]) }); pump(context);
    chat.emit(ChatMsg::Action(ActionMsg::ToggleReaction { id:message.id,emoji_id:Some(EMOJI.parse().unwrap()),unicode:None }));
    until(context,|| chat.model().history.messages[0].user_reactions.as_ref().is_some_and(|v| v.first().is_some_and(|r| r.emoji_id == Some(EMOJI.parse().unwrap()))) && !chat.model().actions.busy.contains(&message.id));
    find_button(chat.widget().upcast_ref(),"Quem reagiu").emit_clicked();
    until(context,|| chat.model().actions.participants.as_ref().is_some_and(|(_,list,_)| descendants(list.upcast_ref()).iter().filter_map(|w| w.downcast_ref::<gtk::Label>()).any(|l| l.text().contains("(você)"))));
    // Deleting a custom emoji has an explicit confirmation; creator-only authorization is supported.
    chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access { read:true, send:true, ..Default::default() } }); pump(context);
    let manager = chat.model().actions.manager.as_ref().unwrap().window.clone();
    find_button(manager.upcast_ref(),"Excluir").emit_clicked(); pump(context);
    find_button(chat.model().actions.confirmation.as_ref().unwrap().upcast_ref(),"Excluir emoji").emit_clicked();
    until(context,|| chat.model().actions.emojis.is_empty());
    assert!(chat.model().actions.textures.is_empty());
    // Navigation continues into older pages using the ordinary history output.
    let outputs = Arc::new(Mutex::new(Vec::new())); let capture = outputs.clone();
    let navigation_chat = ChatModel::builder().launch(ChatInit::default()).connect_receiver(move |_,o| capture.lock().unwrap().push(o));
    navigation_chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access { read:true,..Default::default() } });
    navigation_chat.emit(ChatMsg::SetChannel(serde_json::from_value(json!({"id":CHANNEL,"name":"general","type":"text","created_at":DATE})).unwrap()));
    let t = Uuid::new_v4(); navigation_chat.emit(ChatMsg::BeginHistory { request_id:t,cursor:None });
    navigation_chat.emit(ChatMsg::HistoryLoaded { request_id:t,append:false,result:Ok(MessageListResponse { channel_id,messages:vec![message.clone()],has_more:true }) });
    navigation_chat.emit(ChatMsg::Action(ActionMsg::Navigate(OLDER.parse().unwrap()))); pump(context);
    assert!(outputs.lock().unwrap().iter().any(|o| matches!(o,ChatOutput::LoadMoreMessages { channel_id:id,cursor:Some(c) } if *id == channel_id && c.id == message.id)));
    let old: Message = serde_json::from_value(backend.lock().unwrap().old.clone()).unwrap(); let t = Uuid::new_v4();
    navigation_chat.emit(ChatMsg::BeginHistory { request_id:t,cursor:Some((&message).into()) });
    navigation_chat.emit(ChatMsg::HistoryLoaded { request_id:t,append:true,result:Ok(MessageListResponse { channel_id,messages:vec![old.clone()],has_more:false }) }); pump(context);
    assert_eq!(navigation_chat.model().actions.highlight,Some(old.id));
    assert!(descendants(navigation_chat.widget().upcast_ref()).iter().any(|w| w.widget_name().as_str() == format!("message-{}",old.id) && w.has_css_class("papo-message-highlight")));
    navigation_chat.emit(ChatMsg::Action(ActionMsg::Navigate(Uuid::new_v4()))); pump(context);
    assert!(navigation_chat.model().draft.error.as_deref().unwrap().contains("não disponível"));
    // Confirmation cancellation performs no DELETE; confirmation executes the action once.
    chat.emit(ChatMsg::SetAccess { user_id:user, access:crate::models::Access::resolve(user,Some(user),&[],&[],&[]) }); pump(context);
    assert!(find_button(chat.widget().upcast_ref(),"Excluir").has_css_class("papo-destructive"));
    find_button(chat.widget().upcast_ref(),"Excluir").emit_clicked(); pump(context);
    let confirmation = chat.model().actions.confirmation.as_ref().unwrap().clone();until(context,||confirmation.is_mapped());assert!(confirmation.height()<240&&confirmation.width()<=400,"message deletion should be compact");find_button(confirmation.upcast_ref(),"Cancelar").emit_clicked(); pump(context);
    assert!(!backend.lock().unwrap().requests.iter().any(|(m,p,_)| m == "DELETE" && p == &format!("/messages/{MESSAGE}")));
    find_button(chat.widget().upcast_ref(),"Excluir").emit_clicked(); pump(context);
    let confirmation = chat.model().actions.confirmation.as_ref().unwrap().clone(); find_button(confirmation.upcast_ref(),"Excluir mensagem").emit_clicked();
    until(context,|| chat.model().history.messages.is_empty());
    // A server administrator can manage emojis even without an active/readable channel.
    chat.emit(ChatMsg::ClearChannel);
    chat.emit(ChatMsg::SetAccess { user_id:user,access:crate::models::Access { manage_server:true, ..Default::default() } });
    pump(context);
    assert!(chat.model().actions.manager.as_ref().unwrap().upload.is_visible());
    assert!(chat.model().active_channel.is_none());
    // Full access fetching and role revocation update the actual main-window controls.
    use crate::ui::main_window::{MainWindowInit,MainWindowModel,MainWindowMsg};
    let current_user = serde_json::from_value(json!({"id":USER,"username":"alice","created_at":DATE})).unwrap();
    let outputs=Arc::new(Mutex::new(Vec::new()));let captured=outputs.clone();
    let main = MainWindowModel::builder().launch(MainWindowInit { current_user,api_client:api.clone() }).connect_receiver(move|_,output|captured.lock().unwrap().push(output));
    until(context,|| descendants(main.widget().upcast_ref()).iter().filter_map(|w| w.downcast_ref::<gtk::Button>()).any(|b| b.label().as_deref() == Some("Editar")));
    crate::ui::main_window::layout::tests::exercise(&main,context);
    let inactive_history_baseline=backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path.starts_with("/channels/12345678-1234-4234-8234-123456789ac2/messages")).count();
    crate::ui::main_window::account::exercise_preferences_and_profiles(&main,context);
    super::super::transfers::exercise(&api,context);
    crate::ui::main_window::security::exercise(&main,context);assert!(outputs.lock().unwrap().is_empty());
    crate::ui::login::recovery::exercise(&api,context);
    crate::ui::main_window::search::exercise(&main,context);
    crate::ui::main_window::notifications::exercise(&main,context);
    crate::ui::main_window::direct::exercise(&main,context);
    crate::ui::main_window::administration::exercise(&main,context);
    crate::ui::main_window::moderation::exercise(&main,context,&backend);
    crate::ui::main_window::voice::tests::exercise(&main,context,&backend);
    assert!(outputs.lock().unwrap().is_empty(),"DM and administration failures must retain login");
    backend.lock().unwrap().restricted = true;
    main.emit(MainWindowMsg::WsReceived(crate::ws::WsEvent::RoleRemove { user_id:user,role_id:EMOJI.parse().unwrap() }));
    until(context,|| main.model().channels.is_empty() && main.model().active_channel_id.is_none());
    assert!(descendants(main.widget().upcast_ref()).iter().filter_map(|w| w.downcast_ref::<gtk::Entry>()).all(|e| !e.is_sensitive()));
    main.emit(MainWindowMsg::WsReceived(crate::ws::WsEvent::Error { message:"Unknown server failure".into(),code:Some("future-code".into()) })); pump(context);
    assert!(descendants(main.widget().upcast_ref()).iter().filter_map(|w| w.downcast_ref::<gtk::Label>()).any(|l| l.text().contains("future-code")));
    crate::ui::main_window::notifications::exercise_denied(&main,context);
    let before=backend.lock().unwrap().requests.iter().filter(|(m,_,_)|m=="POST"||m=="PUT"||m=="PATCH"||m=="DELETE").count();
    crate::ui::main_window::administration::exercise_denied(&main,context);
    crate::ui::main_window::moderation::exercise_denied(&main,context);
    crate::ui::main_window::voice::tests::exercise_denied(&main,context);
    assert_eq!(before,backend.lock().unwrap().requests.iter().filter(|(m,_,_)|m=="POST"||m=="PUT"||m=="PATCH"||m=="DELETE").count());
    crate::ui::main_window::security::exercise_current(&main,context);
    until(context,||outputs.lock().unwrap().iter().any(|o|matches!(o,crate::ui::main_window::MainWindowOutput::SessionExpired)));
    let guard=backend.lock().unwrap();let requests = &guard.requests;
    assert!(requests.iter().any(|(m,p,b)| m == "POST" && p.ends_with("/reactions") && b["emoji_id"] == EMOJI && b["unicode"].is_null()));
    assert_eq!(inactive_history_baseline,requests.iter().filter(|(_,path,_)|path.starts_with("/channels/12345678-1234-4234-8234-123456789ac2/messages")).count(),"inactive history must never be fetched for unread counts");
    task.abort();
    crate::ui::main_window::administration::exercise_setup_and_password(context);
}

fn tiny_png_base64()->String{use base64::Engine;let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(1,1).write_to(&mut bytes,image::ImageFormat::Png).unwrap();base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())}
