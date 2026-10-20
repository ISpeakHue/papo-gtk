//! EN-US: Server and channel models.
//! PT-BR: Modelos do servidor e dos canais.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Server ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub icon_blob: Option<String>,
    #[expect(dead_code, reason = "Retained server response metadata and typed counts for API compatibility.")]
    pub icon_format: Option<String>,
    pub owner_id: Option<Uuid>,
    #[expect(dead_code, reason = "Retained server response metadata and typed counts for API compatibility.")]
    pub owner_username: Option<String>,
    pub public: Option<bool>,
    #[expect(dead_code, reason = "Retained server response metadata and typed counts for API compatibility.")]
    pub created_at: DateTime<Utc>,
    #[expect(dead_code, reason = "Retained server response metadata and typed counts for API compatibility.")]
    pub channel_count: Option<i32>,
    #[expect(dead_code, reason = "Retained server response metadata and typed counts for API compatibility.")]
    pub member_count: Option<i32>,
    #[expect(dead_code, reason = "Retained server response metadata and typed counts for API compatibility.")]
    pub role_count: Option<i32>,
}

#[derive(Clone, Default, Serialize)]
pub struct ServerWrite {
    #[serde(skip_serializing_if="Option::is_none")] pub name: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub icon_blob: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub icon_format: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub password: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub public: Option<bool>,
}
impl std::fmt::Debug for ServerWrite {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        f.debug_struct("ServerWrite").field("name",&self.name).field("public",&self.public)
        .field("icon",&self.icon_blob.is_some()).field("password",&"[REDACTED]").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_omissions_and_explicit_removals_differ_and_debug_redacts_secrets(){
        let r=ServerWrite{name:Some("Renamed".into()),..Default::default()};assert_eq!(serde_json::to_value(r).unwrap(),serde_json::json!({"name":"Renamed"}));
        let r=ServerWrite{icon_blob:Some(String::new()),icon_format:Some(String::new()),public:Some(false),password:Some("Secret123!".into()),..Default::default()};
        assert!(!format!("{r:?}").contains("Secret123!"));let json=serde_json::to_value(r).unwrap();assert_eq!(json["icon_blob"],"");assert_eq!(json["public"],false);assert!(json.get("name").is_none());
    }
}
