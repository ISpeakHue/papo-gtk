//! Message, attachment, link-preview and reaction models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Attachment ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub mime_type: String,
    pub original_file_name: String,
    pub size_bytes: i64,
    pub thumbnail_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub moderation_status: Option<String>,
}

// ── Link preview ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct LinkPreview {
    pub id: Uuid,
    pub url: String,
    pub kind: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub provider_name: Option<String>,
    pub embed_url: Option<String>,
    pub video_url: Option<String>,
    pub image_mime_type: Option<String>,
    pub image_size_bytes: Option<i64>,
    pub fetched_at: DateTime<Utc>,
    /// Only present on GET /link-previews/:id
    pub image_data: Option<String>,
}

// ── Reactions ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct MessageReactionSummary {
    pub emoji_id: Option<Uuid>,
    pub unicode: Option<String>,
    pub count: i32,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub channel_id: Uuid,
    #[serde(default, deserialize_with = "optional_author")]
    pub author_id: Option<Uuid>,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    /// ID of the message being replied to (may point to deleted message).
    pub reply_to: Option<Uuid>,
    pub attachments: Option<Vec<Attachment>>,
    pub previews: Option<Vec<LinkPreview>>,
    pub reactions: Option<Vec<MessageReactionSummary>>,
    pub user_reactions: Option<Vec<MessageUserReaction>>,
}

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
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateMessageRequest {
    pub content: String,
}


// The REST response uses null; realtime messages use an empty string for a deleted author.
fn optional_author<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Uuid>, D::Error> {
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
