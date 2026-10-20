//! Message, attachment, embed and reaction models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Match the first text page and the recent media prefetch/residency window.
pub const INITIAL_MESSAGE_LIMIT: usize = 30;

// ── Attachment ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub id: Uuid,
    pub mime_type: String,
    pub original_file_name: String,
    pub size_bytes: i64,
    pub width:Option<i32>,
    pub height:Option<i32>,
    pub thumbnail_id: Option<Uuid>,
    pub thumbnail_width:Option<i32>,
    pub thumbnail_height:Option<i32>,
    pub created_at: DateTime<Utc>,
    pub moderation_status: Option<String>,
}

impl Attachment {
    pub fn thumbnail_dimensions(&self)->Option<(i32,i32)>{self.thumbnail_width.zip(self.thumbnail_height).filter(|(w,h)|(1..=16384).contains(w)&&(1..=16384).contains(h))}

    /// Match the backend's nullable container metadata bounds before sizing UI.
    pub fn video_dimensions(&self)->Option<(i32,i32)>{
        self.width.zip(self.height).filter(|(w,h)|self.is_video()&&(1..=16384).contains(w)&&(1..=16384).contains(h)
            &&i64::from(*w)*i64::from(*h)<=100_000_000&&i64::from(*w)<=i64::from(*h)*10&&i64::from(*h)<=i64::from(*w)*10)
    }

    /// Preview classification only; downloaded bytes still pass container validation.
    pub fn is_video(&self)->bool{
        let mime=self.mime_type.split(';').next().unwrap_or("").trim();
        // Newly detected containers outside the native validator stay ordinary
        // download tiles rather than presenting an unusable playback control.
        if ["video/x-msvideo","video/x-matroska"].iter().any(|kind|mime.eq_ignore_ascii_case(kind)){return false;}
        if mime.get(..6).is_some_and(|prefix|prefix.eq_ignore_ascii_case("video/")){return true;}
        if ["application/quicktime","application/x-quicktimeplayer"].iter().any(|kind|mime.eq_ignore_ascii_case(kind)){return true;}
        // Older records can retain generic MIME even when download responses
        // now report a detected video type. Classify supported suffixes only
        // for unknown MIME; the downloaded container is validated separately.
        let unknown=mime.is_empty()||["application/octet-stream","binary/octet-stream"].iter().any(|kind|mime.eq_ignore_ascii_case(kind));
        unknown&&std::path::Path::new(&self.original_file_name).extension().is_some_and(|ext|["mov","mp4","m4v","webm"].iter().any(|suffix|ext.eq_ignore_ascii_case(suffix)))
    }
}

#[cfg(test)]mod attachment_tests{
    use super::*;
    #[test]fn original_video_geometry_obeys_the_backend_bounds_and_older_servers(){
        let base=serde_json::json!({"id":Uuid::new_v4(),"mime_type":"video/mp4","original_file_name":"clip.mp4","size_bytes":1,"created_at":"2026-10-20T12:00:00Z"});
        assert_eq!(serde_json::from_value::<Attachment>(base.clone()).unwrap().video_dimensions(),None);
        for (w,h,expected) in [
            (Some(1920),Some(1080),Some((1920,1080))),(Some(720),Some(1280),Some((720,1280))),
            (Some(10000),Some(10000),Some((10000,10000))),(Some(16384),Some(1639),Some((16384,1639))),
            (None,None,None),(Some(720),None,None),(None,Some(1280),None),
            (Some(0),Some(10),None),(Some(-1),Some(10),None),(Some(i32::MAX),Some(1),None),
            (Some(16384),Some(16384),None),(Some(10001),Some(10000),None),
            (Some(1000),Some(10),None),(Some(10),Some(1000),None)
        ]{
            let mut value=base.clone();value["width"]=serde_json::json!(w);value["height"]=serde_json::json!(h);
            assert_eq!(serde_json::from_value::<Attachment>(value).unwrap().video_dimensions(),expected,"{w:?} × {h:?}");
        }
        let mut image=base;image["mime_type"]=serde_json::json!("image/avif");image["width"]=serde_json::json!(1920);image["height"]=serde_json::json!(1080);
        assert_eq!(serde_json::from_value::<Attachment>(image).unwrap().video_dimensions(),None);
    }
    #[test]fn thumbnail_geometry_is_optional_and_requires_two_valid_dimensions(){
        for (w,h,expected) in [(Some(640),Some(120),Some((640,120))),(None,None,None),(Some(640),None,None),(Some(0),Some(10),None),(Some(-1),Some(10),None),(Some(i32::MAX),Some(1),None)]{
            let a:Attachment=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"mime_type":"image/png","original_file_name":"photo.png","size_bytes":1,"thumbnail_width":w,"thumbnail_height":h,"created_at":"2026-10-19T12:00:00Z"})).unwrap();assert_eq!(a.thumbnail_dimensions(),expected);
        }
    }
    #[test]fn recognizes_supported_legacy_video_names_only_for_generic_mime(){
        for (mime,name,expected) in [
            ("video/quicktime","clip.mov",true),("VIDEO/QUICKTIME; codecs=avc1","clip",true),
            ("application/octet-stream","phone.MOV",true),("application/octet-stream; charset=binary","phone.mov",true),
            ("binary/octet-stream","clip.mov",true),("","clip.mov",true),
            ("application/quicktime","clip",true),("application/x-quicktimeplayer","clip.mov",true),
            ("video/mp4","clip.mp4",true),("video/webm","clip.webm",true),
            ("application/octet-stream","clip.MP4",true),("application/octet-stream","clip.m4v",true),
            ("binary/octet-stream","clip.WEBM",true),("","clip.mp4",true),
            ("application/octet-stream","notes.pdf",false),("application/octet-stream","clip.mov.exe",false),
            ("application/octet-stream","clip.webm.html",false),("application/octet-stream","clip.mkv",false),
            ("image/png","clip.mov",false),("text/plain","clip.mov",false),
            ("image/avif","clip.mp4",false),("image/heic","clip.webm",false),("audio/mp4","clip.mp4",false),
            ("video/x-msvideo","clip.avi",false),("video/x-matroska","clip.mkv",false)
        ]{
            let a:Attachment=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"mime_type":mime,"original_file_name":name,"size_bytes":1,"created_at":"2026-10-19T12:00:00Z"})).unwrap();
            assert_eq!(a.is_video(),expected,"{mime}: {name}");
        }
    }
}

// ── Reactions ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MessageReactionSummary {
    pub emoji_id: Option<Uuid>,
    pub unicode: Option<String>,
    pub count: i32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MessageUserReaction {
    pub id: Uuid,
    pub emoji_id: Option<Uuid>,
    pub unicode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReactionRequest {
    pub emoji_id: Option<Uuid>,
    pub unicode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionUser { pub id: Uuid, pub user_id: Uuid, pub created_at: DateTime<Utc> }
#[derive(Debug, Clone, Deserialize)]
pub struct ReactionGroup {
    pub emoji_id: Option<Uuid>, pub unicode: Option<String>, pub count: i32, pub users: Vec<ReactionUser>,
}
#[derive(Debug, Deserialize)]
pub struct ReactionList { pub message_id: Uuid, pub reactions: Vec<ReactionGroup>, pub has_more: bool }
#[derive(Debug, Deserialize)]
pub struct PinnedList { pub channel_id: Uuid, pub pinned: Vec<Message> }

// ── Message ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: Uuid,
    pub channel_id: Uuid,
    #[serde(default, deserialize_with = "optional_author")]
    pub author_id: Option<Uuid>,
    pub webhook_id: Option<Uuid>,
    pub external_author: Option<super::ExternalAuthor>,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    /// ID of the message being replied to (may point to deleted message).
    pub reply_to: Option<Uuid>,
    pub attachments: Option<Vec<Attachment>>,
    #[serde(alias="previews")]
    pub embeds: Option<Vec<super::Embed>>,
    pub reactions: Option<Vec<MessageReactionSummary>>,
    pub user_reactions: Option<Vec<MessageUserReaction>>,
}

/// Only metadata needed by notifications and read cursors crosses into the
/// account model. No attachment bytes, embed payloads, or reaction collections.
#[derive(Debug,Clone)]
pub struct MessageObservation {
    pub id:Uuid,pub channel_id:Uuid,pub author_id:Option<Uuid>,pub webhook_id:Option<Uuid>,
    pub external_author:Option<super::ExternalAuthor>,pub content:Option<String>,
    pub created_at:DateTime<Utc>,pub reply_to:Option<Uuid>,
}
impl MessageObservation {
    pub fn external(&self)->Option<&super::ExternalAuthor>{if self.author_id.is_none()&&self.webhook_id.is_some(){self.external_author.as_ref()}else{None}}
    pub fn bytes(&self)->usize{std::mem::size_of::<Self>()+self.content.as_ref().map_or(0,String::capacity)+self.external_author.as_ref().map_or(0,|a|a.source.capacity()+a.id.capacity()+a.name.capacity()+a.avatar_url.capacity()+a.profile_url.capacity())}
    pub fn into_message(self)->Message{Message{id:self.id,channel_id:self.channel_id,author_id:self.author_id,webhook_id:self.webhook_id,external_author:self.external_author,content:self.content,created_at:self.created_at,reply_to:self.reply_to,edited_at:None,attachments:None,embeds:None,reactions:None,user_reactions:None}}
}
impl From<&Message> for MessageObservation {
    fn from(m:&Message)->Self{Self{id:m.id,channel_id:m.channel_id,author_id:m.author_id,webhook_id:m.webhook_id,external_author:m.external().cloned(),content:m.content.clone(),created_at:m.created_at,reply_to:m.reply_to}}
}
impl From<Message> for MessageObservation {fn from(m:Message)->Self{let external_author=if m.author_id.is_none()&&m.webhook_id.is_some(){m.external_author}else{None};Self{id:m.id,channel_id:m.channel_id,author_id:m.author_id,webhook_id:m.webhook_id,external_author,content:m.content,created_at:m.created_at,reply_to:m.reply_to}}}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageListResponse {
    pub channel_id: Uuid,
    pub messages: Vec<Message>,
    pub has_more: bool,
}

// ── Requests ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CreateMessageRequest {
    pub channel_id: Uuid,
    pub content: Option<String>,
    pub reply_to: Option<Uuid>,
    pub embeds:Vec<super::EmbedInput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateMessageRequest {
    pub content: String,
    /// Omitted for text edits; an explicit empty list removes custom embeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embeds:Option<Vec<super::EmbedInput>>,
}


// The REST response uses null; realtime messages use an empty string for a deleted author.
pub(crate) fn optional_author<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Uuid>, D::Error> {
    let value = Option::<String>::deserialize(deserializer)?;
    value.filter(|value| !value.is_empty()).map(|value| value.parse()
        .map_err(serde::de::Error::custom)).transpose()
}

/// Both values are required to paginate messages that share a timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

impl From<&Message> for MessageCursor {
    fn from(message: &Message) -> Self { Self { created_at: message.created_at, id: message.id } }
}

impl Message {
    /// A webhook can relay many people. Never group them by webhook ID alone,
    /// or treat two missing/deleted human authors as the same person.
    pub fn same_sender(&self,other:&Self)->bool{
        if self.channel_id!=other.channel_id{return false;}
        if let Some(id)=self.author_id{return other.author_id==Some(id);}
        match (self.external(),other.external()) {
            (Some(a),Some(b))=>self.webhook_id==other.webhook_id&&!a.id.trim().is_empty()&&!a.source.trim().is_empty()&&a.id==b.id&&a.source==b.source,
            _=>false,
        }
    }
    /// External identities are display metadata and never a Papo account.
    pub fn external(&self) -> Option<&super::ExternalAuthor> {
        if self.author_id.is_none() && self.webhook_id.is_some() { self.external_author.as_ref() } else { None }
    }
    pub fn sender_name<'a>(&'a self, users: &'a std::collections::HashMap<Uuid, super::UserSummary>) -> &'a str {
        self.author_id.and_then(|id| users.get(&id)).map(|u| u.display_name())
            .or_else(|| self.external().map(|a| a.name.as_str())).filter(|n| !n.trim().is_empty())
            .unwrap_or(if self.webhook_id.is_some() { "Webhook" } else if self.author_id.is_some() { "Usuário" } else { "Usuário removido" })
    }
}
