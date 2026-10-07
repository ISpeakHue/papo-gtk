//! WebSocket client — runs as a long-lived background tokio task.
//!
//! Connects to `ws://<host>/ws`, parses incoming events and forwards them to
//! the UI layer via an `mpsc` channel, with bounded reconnect backoff.


use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::models::*;

// ── Incoming event types ──────────────────────────────────────────────────────

/// Envelope shared by all WebSocket events.
#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(flatten)]
    payload: serde_json::Value,
}

/// Strongly-typed events surfaced to the UI layer.
#[derive(Debug, Clone)]
pub enum WsEvent {
    Voice(VoiceEvent),
    ConnectionReady(Uuid),
    Error { message: String, code: Option<String> },
    /// A new message was posted.
    NewMessage(Message),
    NewNotification(crate::models::NotificationEvent),
    /// A message was edited.
    MessageEdit { id: Uuid, content: String, channel_id: Option<Uuid>, edited_at: Option<chrono::DateTime<chrono::Utc>> },
    /// A message was deleted.
    MessageDelete { id: Uuid, channel_id: Uuid },
    /// A message was pinned.
    MessagePin { message_id: Uuid, is_pinned: bool },
    /// Reaction counts updated.
    ReactionUpdate { message_id: Uuid, reaction: MessageReactionSummary },
    /// Channel created, updated or deleted.
    ChannelsChanged,
    DirectUpdate(DirectConversation),
    /// User presence snapshot (sent once on connect).
    PresenceSync(Vec<PresenceEntry>),
    /// Incremental presence change.
    PresenceUpdate(PresenceEntry),
    /// New user registered.
    UserJoin { user_id: Uuid },
    /// Avatar or nickname changed.
    AvatarUpdate { user_id: Uuid },
    /// Role was added to or removed from a user.
    RoleAdd { user_id: Uuid, role_id: Uuid },
    RoleRemove { user_id: Uuid, role_id: Uuid },
    /// A user started typing.
    Typing { user_id: Uuid, channel_id: Uuid, is_typing: bool },
    /// WebSocket disconnected.
    Disconnected,
    SessionExpired,
    NewPreview { message_id: Uuid, preview_id: Uuid },
    RemovePreview { message_id: Uuid, preview_id: Uuid },
    PreviewUpdate { channel_id: Uuid, message_id: Uuid, preview: LinkPreview },
    AttachmentModeration { channel_id: Uuid, message_id: Uuid, attachment_id: Uuid, status: String },
    /// WebSocket connected; refresh snapshots to recover any missed events.
    Reconnected,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PresenceEntry {
    pub user_id: Uuid,
    pub status: PresenceStatus,
    pub status_message: Option<String>,
    pub typing: Option<String>,
    pub nickname: Option<String>,
    #[serde(default)]
    pub user_voice: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresenceStatus { Online, Offline, Away, Busy }

impl PresenceEntry {
    pub fn online(&self) -> bool { self.status != PresenceStatus::Offline }
}

/// Voice commands cannot survive a WebSocket reconnect. Ordinary chat frames retain
/// their existing queue behavior; only the owning connection can send media frames.
pub enum WsCommand { Text(String), Voice{connection:Uuid,text:String} }
impl From<String> for WsCommand{fn from(s:String)->Self{Self::Text(s)}}
impl From<&str> for WsCommand{fn from(s:&str)->Self{Self::Text(s.into())}}
impl WsCommand {fn for_connection(self,id:Uuid)->Option<String>{match self{Self::Text(s)=>Some(s),Self::Voice{connection,text} if connection==id=>Some(text),_=>None}}}

// ── Client ───────────────────────────────────────────────────────────────────

/// Spawns a background task that manages the WebSocket connection.
///
/// Returns an `mpsc::Receiver` of [`WsEvent`] values and an `mpsc::Sender`
/// that can be used to send raw text frames (e.g. heartbeats or typing).
pub fn spawn(client: crate::api::ApiClient) -> (mpsc::Receiver<WsEvent>, mpsc::Sender<WsCommand>) {
    let (ev_tx, ev_rx) = mpsc::channel::<WsEvent>(256);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<WsCommand>(64);

    tokio::spawn(async move {
        let mut backoff_secs = 2u64;

        loop {
            if ev_tx.is_closed() {
                break;
            }

            let request = match client.websocket_request() {
                Ok(request) => request,
                Err(e) => {
                    error!("Cannot build WebSocket request: {e}");
                    return;
                }
            };
            info!("Connecting to WebSocket");
            let connection = tokio::select! {
                _ = ev_tx.closed() => return,
                connection = connect_async(request) => connection,
            };
            match connection {
                Ok((stream, _)) => {
                    let connection_id=Uuid::new_v4();
                    if ev_tx.send(WsEvent::ConnectionReady(connection_id)).await.is_err(){return;}
                    backoff_secs = 2; // Reset backoff on successful connect
                    // Also resync after the first connection: initial HTTP loading
                    // may have completed before the socket became available.
                    if ev_tx.send(WsEvent::Reconnected).await.is_err() {
                        break;
                    }

                    let (mut write, mut read) = stream.split();
                    let mut heartbeat = tokio::time::interval(tokio::time::Duration::from_secs(25));
                    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                    loop {
                        tokio::select! {
                            _ = ev_tx.closed() => return,
                            _ = heartbeat.tick() => {
                                if write.send(WsMessage::Text(r#"{"type":"heartbeat"}"#.into())).await.is_err() { break; }
                            }
                            // Incoming server frames
                            msg = read.next() => {
                                let Some(msg) = msg else { break; };
                                match msg {
                                    Ok(WsMessage::Text(txt)) => {
                                        if let Some(event) = parse_event(&txt) {
                                            if ev_tx.send(event).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                    Ok(WsMessage::Ping(payload)) => {
                                        if write.send(WsMessage::Pong(payload)).await.is_err() { break; }
                                    }
                                    Ok(WsMessage::Close(_)) => {
                                        warn!("WebSocket closed by server");
                                        break;
                                    }
                                    Err(e) => {
                                        error!("WebSocket read error: {e}");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            // Outbound commands from the UI (typing, heartbeat)
                            cmd = cmd_rx.recv() => {
                                match cmd {
                                    Some(command) => {
                                        let Some(txt)=command.for_connection(connection_id)else{continue;};
                                        if let Err(e) = write.send(WsMessage::Text(txt.into())).await {
                                            error!("WebSocket write error: {e}");
                                            break;
                                        }
                                    }
                                    None => {
                                        // UI dropped cmd sender, terminate task
                                        return;
                                    }
                                }
                            }
                        }
                    }

                    if ev_tx.send(WsEvent::Disconnected).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    if matches!(&e, tokio_tungstenite::tungstenite::Error::Http(response)
                        if matches!(response.status().as_u16(), 401 | 403)) {
                        let _ = ev_tx.send(WsEvent::SessionExpired).await;
                        return;
                    }
                    error!("WebSocket connect error: {e}");
                }
            }

            if ev_tx.is_closed() {
                break;
            }

            // Exponential back-off before reconnect, capped at 30 seconds
            tokio::select! {
                _ = ev_tx.closed() => return,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)) => {}
            }
            backoff_secs = (backoff_secs * 2).min(30);
        }
    });

    (ev_rx, cmd_tx)
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_event(text: &str) -> Option<WsEvent> {
    let raw: RawEvent = serde_json::from_str(text)
        .map_err(|e| warn!("Unparseable WS frame: {e}"))
        .ok()?;

    match raw.event_type.as_str() {
        "voice_joined"|"voice_answer"|"voice_offer"|"voice_ice_candidate"|"voice_state_update"|"voice_leave"|"active_speaker_update"|"voice_audio_routes"=>serde_json::from_str(text).ok().map(WsEvent::Voice),
        "message" => {
            let msg: Message = serde_json::from_value(raw.payload)
                .map_err(|e| warn!("Bad message payload: {e}"))
                .ok()?;
            Some(WsEvent::NewMessage(msg))
        }
        "new_notification" => Some(WsEvent::NewNotification(crate::models::NotificationEvent {
            id:parse_uuid(&raw.payload,"id")?,user_id:parse_uuid(&raw.payload,"user_id"),
            message_id:parse_uuid(&raw.payload,"message_id")?,message_content:raw.payload["message_content"].as_str().unwrap_or_default().to_owned(),
        })),
        "error" => Some(WsEvent::Error {
            message: raw.payload["message"].as_str().unwrap_or("Erro do servidor").to_owned(),
            code: raw.payload["code"].as_str().map(str::to_owned),
        }),
        "message_edit" => {
            let id = parse_uuid(&raw.payload, "id")?;
            let content = raw.payload["content"].as_str()?.to_owned();
            let channel_id = parse_uuid(&raw.payload, "channel_id");
            let edited_at = raw.payload["edited_at"].as_str().and_then(|value| value.parse().ok());
            Some(WsEvent::MessageEdit { id, content, channel_id, edited_at })
        }
        "message_delete" => {
            let id = parse_uuid(&raw.payload, "id")?;
            let channel_id = parse_uuid(&raw.payload, "channel_id")?;
            Some(WsEvent::MessageDelete { id, channel_id })
        }
        "message_pin" => Some(WsEvent::MessagePin {
            message_id: parse_uuid(&raw.payload, "message_id")?,
            is_pinned: raw.payload["is_pinned"].as_bool()?,
        }),
        "react_update" => Some(WsEvent::ReactionUpdate {
            message_id: parse_uuid(&raw.payload, "message_id")?,
            reaction: serde_json::from_value(raw.payload).ok()?,
        }),
        "dm_update" => serde_json::from_value(raw.payload.get("dm")?.clone()).ok().map(WsEvent::DirectUpdate),
        "channel_create" | "channel_update" | "channel_delete" => {
            parse_uuid(&raw.payload, "channel_id")?;
            Some(WsEvent::ChannelsChanged)
        }
        "presence_sync" => Some(WsEvent::PresenceSync(
            serde_json::from_value(raw.payload["members"].clone()).ok()?)),
        "presence_update" => {
            let entry: PresenceEntry = serde_json::from_value(raw.payload).ok()?;
            Some(WsEvent::PresenceUpdate(entry))
        }
        "user_join" => {
            Some(WsEvent::UserJoin { user_id: parse_uuid(&raw.payload, "user_id")? })
        }
        "avatar_update" => {
            let user_id = parse_uuid(&raw.payload, "user_id")?;
            Some(WsEvent::AvatarUpdate { user_id })
        }
        "role_add" => {
            let user_id = parse_uuid(&raw.payload, "user_id")?;
            let role_id = parse_uuid(&raw.payload, "role_id")?;
            Some(WsEvent::RoleAdd { user_id, role_id })
        }
        "role_remove" => {
            let user_id = parse_uuid(&raw.payload, "user_id")?;
            let role_id = parse_uuid(&raw.payload, "role_id")?;
            Some(WsEvent::RoleRemove { user_id, role_id })
        }
        "typing" => {
            let user_id = parse_uuid(&raw.payload, "user_id")?;
            let channel_id = parse_uuid(&raw.payload, "channel_id")?;
            Some(WsEvent::Typing { user_id, channel_id, is_typing: raw.payload["is_typing"].as_bool().unwrap_or(true) })
        }
        "new_preview" => Some(WsEvent::NewPreview {
            message_id: parse_uuid(&raw.payload, "message_id")?, preview_id: parse_uuid(&raw.payload, "preview_id")?,
        }),
        "remove_preview" => Some(WsEvent::RemovePreview {
            message_id: parse_uuid(&raw.payload, "message_id")?, preview_id: parse_uuid(&raw.payload, "preview_id")?,
        }),
        "link_preview_update" => Some(WsEvent::PreviewUpdate {
            channel_id: parse_uuid(&raw.payload, "channel_id")?, message_id: parse_uuid(&raw.payload, "message_id")?,
            preview: serde_json::from_value(raw.payload["preview"].clone()).ok()?,
        }),
        "attachment_moderation_update" => Some(WsEvent::AttachmentModeration {
            channel_id: parse_uuid(&raw.payload, "channel_id")?, message_id: parse_uuid(&raw.payload, "message_id")?,
            attachment_id: parse_uuid(&raw.payload, "attachment_id")?, status: raw.payload["status"].as_str()?.into(),
        }),
        "heartbeat_ack" => None, // silently swallow
        other => {
            warn!("Unknown WS event type: {other}");
            None
        }
    }
}

fn parse_uuid(v: &serde_json::Value, key: &str) -> Option<Uuid> {
    v[key].as_str()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_author_is_not_a_channel_and_empty_author_is_supported(){
        for author in ["12345678-1234-4234-8234-123456789abc",""] {
            let raw=serde_json::json!({"type":"new_notification","id":"12345678-1234-4234-8234-123456789abd","user_id":author,"message_id":"12345678-1234-4234-8234-123456789abe","message_content":"Hi"});
            let Some(WsEvent::NewNotification(n))=parse_event(&raw.to_string())else{panic!("notification was not parsed")};assert_eq!(n.user_id.is_some(),!author.is_empty());assert_eq!(n.message_content,"Hi");
        }
    }

    #[test]
    fn test_parse_message_edit_event() {
        let msg_id = Uuid::new_v4();
        let payload = format!(r#"{{"type":"message_edit","id":"{}","content":"hello updated"}}"#, msg_id);
        let event = parse_event(&payload).expect("should parse");
        match event {
            WsEvent::MessageEdit { id, content, .. } => {
                assert_eq!(id, msg_id);
                assert_eq!(content, "hello updated");
            }
            other => panic!("Unexpected event: {other:?}"),
        }
    }

    #[test]
    fn test_parse_message_delete_event() {
        let msg_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let payload = format!(r#"{{"type":"message_delete","id":"{}","channel_id":"{}"}}"#, msg_id, channel_id);
        let event = parse_event(&payload).expect("should parse");
        match event {
            WsEvent::MessageDelete { id, channel_id: ch_id } => {
                assert_eq!(id, msg_id);
                assert_eq!(ch_id, channel_id);
            }
            other => panic!("Unexpected event: {other:?}"),
        }
    }

    #[test]
    fn test_parse_typing_event() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let payload = format!(r#"{{"type":"typing","user_id":"{}","channel_id":"{}"}}"#, user_id, channel_id);
        let event = parse_event(&payload).expect("should parse");
        match event {
            WsEvent::Typing { user_id: u_id, channel_id: ch_id, .. } => {
                assert_eq!(u_id, user_id);
                assert_eq!(ch_id, channel_id);
            }
            other => panic!("Unexpected event: {other:?}"),
        }
    }

    #[test]
    fn test_parse_heartbeat_ack_swallowed() {
        let payload = r#"{"type":"heartbeat_ack"}"#;
        assert!(parse_event(payload).is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        assert!(parse_event("not json").is_none());
    }
}


#[cfg(test)]
mod backend_contract_tests {
    use super::*;
    use serde_json::json;
    const ID: &str = "12345678-1234-4234-8234-123456789abc";

    fn parse(value: serde_json::Value) -> WsEvent { parse_event(&value.to_string()).unwrap() }

    #[test]
    fn websocket_errors_preserve_unknown_codes_and_optional_messages() {
        let WsEvent::Error { message, code } = parse(json!({"type":"error","message":"permission lost","code":"future-code"})) else { panic!("error ignored"); };
        assert_eq!(message,"permission lost"); assert_eq!(code.as_deref(),Some("future-code"));
        assert!(matches!(parse(json!({"type":"error","message":"Bad request"})),WsEvent::Error { code:None, .. }));
    }

    #[test]
    fn presence_uses_members_and_status_instead_of_online_boolean() {
        let event = parse(json!({"type":"presence_sync","members":[
            {"user_id":ID,"status":"away","status_message":"Reading", "user_voice":[ID]}
        ]}));
        let WsEvent::PresenceSync(entries) = event else { panic!("wrong event"); };
        assert!(entries[0].online());
        assert_eq!(entries[0].status,PresenceStatus::Away);
        assert_eq!(entries[0].user_voice.len(),1);
        for status in ["online","offline","busy","away"] {
            let WsEvent::PresenceUpdate(entry) = parse(json!({"type":"presence_update","user_id":ID,"status":status,"nickname":"Alice","typing":"Thinking"})) else { panic!("wrong event"); };
            assert_eq!(entry.online(),status != "offline");
            assert_eq!(entry.nickname.as_deref(),Some("Alice"));
        }
    }

    #[test]
    fn partial_channel_events_and_user_join_request_snapshot_refreshes() {
        for kind in ["channel_create","channel_update","channel_delete"] {
            assert!(matches!(parse(json!({"type":kind,"channel_id":ID,"name":"General","position":0,"topic":null,"channel_type":"text"})),WsEvent::ChannelsChanged));
        }
        assert!(matches!(parse(json!({"type":"user_join","user_id":ID})),WsEvent::UserJoin { .. }));
    }

    #[test]
    fn reaction_counts_pin_state_and_typing_stop_match_payloads() {
        for count in [0,3] {
            let WsEvent::ReactionUpdate { reaction, .. } = parse(json!({"type":"react_update","message_id":ID,"emoji_id":ID,"unicode":null,"count":count})) else { panic!("wrong event"); };
            assert_eq!(reaction.count,count);
            assert_eq!(reaction.emoji_id.unwrap().to_string(),ID);
        }
        assert!(matches!(parse(json!({"type":"message_pin","message_id":ID,"is_pinned":false})),WsEvent::MessagePin { is_pinned:false, .. }));
        assert!(matches!(parse(json!({"type":"typing","user_id":ID,"channel_id":ID,"is_typing":false})),WsEvent::Typing { is_typing:false, .. }));
        let WsEvent::MessageEdit { channel_id, edited_at, .. } = parse(json!({"type":"message_edit","id":ID,"channel_id":ID,"content":"edited","edited_at":"2026-10-03T12:00:00Z"})) else { panic!("wrong event"); };
        assert!(channel_id.is_some());
        assert_eq!(edited_at.unwrap().timestamp(),1791028800);
    }

    #[test]
    fn asynchronous_previews_and_attachment_moderation_are_delivered() {
        for kind in ["new_preview","remove_preview"] {
            assert!(parse_event(&json!({"type":kind,"message_id":ID,"preview_id":ID}).to_string()).is_some());
        }
        assert!(matches!(parse(json!({"type":"link_preview_update","channel_id":ID,"message_id":ID,"preview":{
            "id":ID,"url":"https://example.test","kind":"og","fetched_at":"2026-10-03T12:00:00Z","video_url":null
        }})),WsEvent::PreviewUpdate { .. }));
        assert!(matches!(parse(json!({"type":"attachment_moderation_update","channel_id":ID,"message_id":ID,"attachment_id":ID,"status":"sensitive"})),WsEvent::AttachmentModeration { .. }));
    }

    #[tokio::test]
    async fn ping_is_answered_and_revoked_session_stops_reconnecting() {
        use tokio::{net::TcpListener,io::AsyncWriteExt,time::{timeout,Duration}};
        use tokio_tungstenite::{accept_async,tungstenite::Message};
        timeout(Duration::from_secs(10),async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let client = crate::api::ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
            let server = tokio::spawn(async move {
                let (stream,_) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                socket.send(Message::Ping(vec![1,2,3])).await.unwrap();
                let mut got_pong = false;
                let mut got_heartbeat = false;
                while !got_pong || !got_heartbeat {
                    match socket.next().await.unwrap().unwrap() {
                        Message::Pong(bytes) => { assert_eq!(bytes,vec![1,2,3]); got_pong=true; }
                        Message::Text(text) => { assert_eq!(text,r#"{"type":"heartbeat"}"#); got_heartbeat=true; }
                        other => panic!("unexpected frame: {other:?}"),
                    }
                }
                socket.close(None).await.unwrap();
                drop(socket);
                // Server password changes revoke cookies and disconnect sockets. The
                // next handshake receives 401; it must return to login without retries.
                let (mut stream,_) = listener.accept().await.unwrap();
                stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            });
            let (mut events,commands) = spawn(client);
            assert!(matches!(events.recv().await,Some(WsEvent::ConnectionReady(_))));
            assert!(matches!(events.recv().await,Some(WsEvent::Reconnected)));
            assert!(matches!(events.recv().await,Some(WsEvent::Disconnected)));
            assert!(matches!(events.recv().await,Some(WsEvent::SessionExpired)));
            assert!(events.recv().await.is_none());
            assert!(commands.is_closed());
            server.await.unwrap();
        }).await.expect("WebSocket mock timed out");
    }
}

#[cfg(test)]
mod direct_contract_tests {
    use super::*;
    #[test]
    fn dm_updates_decode_personal_unread_snapshots_without_becoming_public_channels(){
        let value=serde_json::json!({"type":"dm_update","dm":{"id":Uuid::new_v4(),"user":{"id":Uuid::new_v4(),"username":"peer","created_at":"2026-10-03T00:00:00Z"},"created_at":"2026-10-03T00:00:00Z","unread_count":3,"last_read_message":null}});
        let Some(WsEvent::DirectUpdate(d))=parse_event(&value.to_string())else{panic!("DM event not decoded")};assert_eq!(d.unread_count,3);assert_eq!(d.user.username,"peer");assert!(parse_event(r#"{"type":"dm_update","dm":null}"#).is_none());
    }
}

#[cfg(test)]
mod voice_contract_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn voice_events_decode_actual_backend_shapes_without_logging_sdp() {
        let id=Uuid::new_v4();let user=Uuid::new_v4();
        let examples=vec![json!({"type":"voice_joined","channel_id":id,"members":[{"user_id":user,"muted":true,"camera_on":false,"screen_sharing":false}],"active_speakers":[]}),json!({"type":"voice_answer","channel_id":id,"sdp":"SECRET"}),json!({"type":"voice_ice_candidate","channel_id":id,"candidate":"candidate","sdp_mid":null,"sdp_mline_index":0}),json!({"type":"voice_state_update","channel_id":id,"user_id":user,"muted":false,"camera_on":false,"screen_sharing":false}),json!({"type":"voice_leave","channel_id":id,"user_id":user}),json!({"type":"active_speaker_update","channel_id":id,"user_ids":[user]}),json!({"type":"voice_audio_routes","channel_id":id,"routes":[{"track_id":"papo-audio-0","user_id":user}]}),json!({"type":"voice_audio_routes","channel_id":id,"routes":[]})];
        for value in examples{let event=parse_event(&value.to_string()).unwrap();assert!(!format!("{event:?}").contains("SECRET"));assert!(matches!(event,WsEvent::Voice(e) if e.channel()==id));}
        assert!(parse_event(&json!({"type":"voice_answer","channel_id":"invalid","sdp":"SECRET"}).to_string()).is_none());
    }
    #[test]
    fn reconnect_drops_old_voice_signaling_but_retains_chat_commands() {
        let old=Uuid::new_v4();let new=Uuid::new_v4();
        assert!(WsCommand::Voice{connection:old,text:"old ICE".into()}.for_connection(new).is_none());
        assert_eq!(WsCommand::Voice{connection:new,text:"current ICE".into()}.for_connection(new).as_deref(),Some("current ICE"));
        assert_eq!(WsCommand::from("chat").for_connection(new).as_deref(),Some("chat"));
    }
}

#[cfg(test)]
mod voice_reconnect_tests {
    use super::*;
    use tokio::{net::TcpListener,time::{timeout,Duration}};
    use tokio_tungstenite::{accept_async,tungstenite::Message};
    #[tokio::test]
    async fn replacement_socket_never_receives_previous_call_frames() {
        timeout(Duration::from_secs(10),async {
            let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
            let api=crate::api::ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
            let server=tokio::spawn(async move{
                let(stream,_)=listener.accept().await.unwrap();let mut first=accept_async(stream).await.unwrap();first.close(None).await.unwrap();drop(first);
                let(stream,_)=listener.accept().await.unwrap();let mut second=accept_async(stream).await.unwrap();let(mut chat,mut voice)=(false,false);
                while !chat||!voice{if let Message::Text(text)=second.next().await.unwrap().unwrap(){let value:serde_json::Value=serde_json::from_str(&text).unwrap();match value["type"].as_str().unwrap(){"heartbeat"=>{},"presence_activity"=>chat=true,"voice_mute"=>voice=true,"voice_offer"=>panic!("old SDP reached a replacement WebSocket"),_=>panic!("unexpected command")}}}
                second.close(None).await.unwrap();
            });
            let(mut events,commands)=spawn(api);
            let Some(WsEvent::ConnectionReady(old))=events.recv().await else{panic!("missing connection identity")};assert!(matches!(events.recv().await,Some(WsEvent::Reconnected)));assert!(matches!(events.recv().await,Some(WsEvent::Disconnected)));
            commands.send(WsCommand::Voice{connection:old,text:r#"{"type":"voice_offer","sdp":"old"}"#.into()}).await.unwrap();commands.send(WsCommand::from(r#"{"type":"presence_activity"}"#)).await.unwrap();
            let Some(WsEvent::ConnectionReady(new))=events.recv().await else{panic!("missing replacement identity")};assert_ne!(new,old);assert!(matches!(events.recv().await,Some(WsEvent::Reconnected)));
            commands.send(WsCommand::Voice{connection:new,text:r#"{"type":"voice_mute","muted":true}"#.into()}).await.unwrap();server.await.unwrap();drop(events);drop(commands);
        }).await.expect("voice reconnect fixture timed out");
    }
}
