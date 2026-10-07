//! Auth-related request/response models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Requests ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginServerRequest {
    pub server_password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DropConnectionRequest {
    /// UUID string or "ALL"
    pub connection_id: String,
}

// ── Responses ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RegisterResponse {
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

/// Returned by POST /auth/login.
/// The actual JWT is sent as an HttpOnly cookie; only the user object is in
/// the body.
#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub user: LoginUser,
    /// True if a session-token reuse was detected (all sessions revoked).
    pub connection_violation: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginUser {
    pub id: Uuid,
    pub username: String,
}

/// Active session connection (device).
#[derive(Debug, Clone, Deserialize)]
pub struct Connection {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResponse {
    pub connection: Connection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectedDevicesResponse {
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DropConnectionResponse {
    pub dropped: u32,
}
