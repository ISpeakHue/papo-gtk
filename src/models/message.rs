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
    pub thumbnail_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub moderation_status: Option<String>,
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
