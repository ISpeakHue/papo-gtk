//! User-related models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::role::RoleSummary;

// ── Enums ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Away,
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    Small,
    Medium,
    Huge,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageDensity {
    Compact,
    Normal,
    Comfortable,
}

// ── User sub-objects ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct NotificationConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "messagePreview")]
    pub message_preview: Option<bool>,
    pub sound: Option<bool>,
    pub mentions: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DisplayConfig {
    #[serde(rename = "fontSize")]
    pub font_size: Option<FontSize>,
    #[serde(rename = "messageDensity")]
    pub message_density: Option<MessageDensity>,
    #[serde(rename = "showTimestamps")]
    pub show_timestamps: Option<bool>,
    #[serde(rename = "showAvatars")]
    pub show_avatars: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct UserConfig {
    pub theme: Option<Theme>,
    pub notifications: Option<NotificationConfig>,
    pub display: Option<DisplayConfig>,
}

impl Default for UserConfig {
    fn default() -> Self { Self {
        theme: Some(Theme::System),
        notifications: Some(NotificationConfig { enabled: Some(true), message_preview: Some(true), sound: Some(true), mentions: Some(true) }),
        display: Some(DisplayConfig { font_size: Some(FontSize::Medium), message_density: Some(MessageDensity::Normal), show_timestamps: Some(true), show_avatars: Some(true) }),
    } }
}
impl UserConfig {
    /// Fill legacy omissions without replacing explicitly saved false values.
    pub fn complete(&self) -> Self {
        let mut c = Self::default();
        if let Some(t) = &self.theme { c.theme = Some(t.clone()); }
        if let Some(n) = &self.notifications { let d = c.notifications.as_mut().unwrap();
            if n.enabled.is_some() {d.enabled=n.enabled;} if n.message_preview.is_some() {d.message_preview=n.message_preview;}
            if n.sound.is_some() {d.sound=n.sound;} if n.mentions.is_some() {d.mentions=n.mentions;}
        }
        if let Some(n) = &self.display { let d = c.display.as_mut().unwrap();
            if n.font_size.is_some() {d.font_size=n.font_size.clone();} if n.message_density.is_some() {d.message_density=n.message_density.clone();}
            if n.show_timestamps.is_some() {d.show_timestamps=n.show_timestamps;} if n.show_avatars.is_some() {d.show_avatars=n.show_avatars;}
        } c
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserSettings {
    pub user_id: Uuid,
    pub version: i32,
    pub config: UserConfig,
    pub updated_at: DateTime<Utc>,
}

// ── Core user types ──────────────────────────────────────────────────────────

/// Compact user representation used in channel user lists, mentions, etc.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UserSummary {
    #[serde(default)]
    pub banned: bool,
    pub id: Uuid,
    pub username: String,
    pub nickname: Option<String>,
    pub status: Option<UserStatus>,
    pub status_message: Option<String>,
    /// Custom typing phrase shown when the user is typing.
    pub typing: Option<String>,
    pub status_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub roles: Option<Vec<RoleSummary>>,
}

impl UserSummary {
    /// Returns nickname if set, otherwise username.
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.username)
    }
}

/// Full user profile (includes avatar blob).
#[derive(Debug, Clone, Deserialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub username: String,
    pub nickname: Option<String>,
    /// Avatar as base64-encoded bytes.
    pub avatar_blob: Option<String>,
    pub avatar_format: Option<String>,
    /// SHA-256 reference into /media/:sha_hash.
    pub banner_media: Option<String>,
    pub description: Option<String>,
    pub status: Option<UserStatus>,
    pub status_message: Option<String>,
    pub typing: Option<String>,
    pub status_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub roles: Option<Vec<RoleSummary>>,
}

impl UserProfile {
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.username)
    }
}

/// Response of GET /auth/whoami — own profile + settings.
#[derive(Debug, Clone, Deserialize)]
pub struct WhoamiResponse {
    pub id: Uuid,
    pub username: String,
    pub nickname: Option<String>,
    pub avatar_blob: Option<String>,
    pub avatar_format: Option<String>,
    pub status: Option<UserStatus>,
    pub status_message: Option<String>,
    pub typing: Option<String>,
    pub status_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub roles: Option<Vec<RoleSummary>>,
    pub settings: Option<WhoamiSettings>,
    pub connection_violation: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WhoamiSettings {
    pub version: i32,
    pub config: UserConfig,
}

// ── Request types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct UpdateUserRequest {
    pub nickname: String,
    pub status: String,
    pub description: String,
    pub typing: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateAvatarRequest {
    pub avatar: String,
    pub avatar_format: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatusRequest {
    pub status: Option<UserStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangePasswordRequest {
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileBatchRequest {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileBatchResponse {
    pub profiles: Vec<UserProfile>,
}

// ── Paginated list ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct UserListResponse {
    pub users: Vec<UserSummary>,
    pub has_more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_summary_display_name() {
        let user_without_nick = UserSummary {
            banned: false,
            id: Uuid::new_v4(),
            username: "johndoe".into(),
            nickname: None,
            status: None,
            status_message: None,
            typing: None,
            status_updated_at: None,
            created_at: Utc::now(),
            roles: None,
        };
        assert_eq!(user_without_nick.display_name(), "johndoe");

        let user_with_nick = UserSummary {
            banned: false,
            id: Uuid::new_v4(),
            username: "johndoe".into(),
            nickname: Some("John D.".into()),
            status: None,
            status_message: None,
            typing: None,
            status_updated_at: None,
            created_at: Utc::now(),
            roles: None,
        };
        assert_eq!(user_with_nick.display_name(), "John D.");
    }
}
