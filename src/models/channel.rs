//! Channel models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Text,
    Category,
    Voice,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSettings {
    Off,
    OnlyMentions,
    All,
}

// ── Channel ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelPermissions {
    pub read_channel: Option<bool>,
    pub send_messages: Option<bool>,
    pub delete_messages: Option<bool>,
    pub connect_voice: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelPermissionEntry {
    pub role_id: Uuid,
    pub role_name: String,
    pub permissions: ChannelPermissions,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelLastMessage {
    pub id: Uuid,
    pub content: Option<String>,
    pub author_id: Option<Uuid>,
    pub author_username: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: Option<ChannelType>,
    pub position: Option<i32>,
    pub topic: Option<String>,
    pub permissions: Option<Vec<ChannelPermissionEntry>>,
    pub created_at: DateTime<Utc>,
    pub last_message: Option<ChannelLastMessage>,
    pub last_read_message: Option<Uuid>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub notification_settings: Option<NotificationSettings>,
}

impl Channel {
    /// Returns true if this channel has unread messages.
    pub fn has_unread(&self) -> bool {
        match (&self.last_message, &self.last_read_message) {
            (Some(last), Some(read)) => last.id != *read,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelListResponse {
    pub channels: Vec<Channel>,
}

// ── Requests ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CreateChannelRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: Option<ChannelType>,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateChannelRequest {
    pub name: String,
    pub topic: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangeChannelPositionRequest {
    pub old_position: i32,
    pub new_position: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateChannelPermissionsRequest {
    pub permissions: ChannelPermissions,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateChannelUserSettingRequest {
    pub notification_settings: NotificationSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_has_unread() {
        let msg_id1 = Uuid::new_v4();
        let msg_id2 = Uuid::new_v4();

        let mut channel = Channel {
            id: Uuid::new_v4(),
            name: "general".into(),
            channel_type: Some(ChannelType::Text),
            position: Some(0),
            topic: None,
            permissions: None,
            created_at: Utc::now(),
            last_message: None,
            last_read_message: None,
            last_read_at: None,
            notification_settings: None,
        };

        // No last message -> no unread
        assert!(!channel.has_unread());

        // Has message, but not read yet -> unread
        channel.last_message = Some(ChannelLastMessage {
            id: msg_id1,
            content: Some("Hello".into()),
            author_id: None,
            author_username: None,
            created_at: Utc::now(),
        });
        assert!(channel.has_unread());

        // Read matches last message -> not unread
        channel.last_read_message = Some(msg_id1);
        assert!(!channel.has_unread());

        // New last message arrives -> unread
        channel.last_message = Some(ChannelLastMessage {
            id: msg_id2,
            content: Some("New message".into()),
            author_id: None,
            author_username: None,
            created_at: Utc::now(),
        });
        assert!(channel.has_unread());
    }
}
