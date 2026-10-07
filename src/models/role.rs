//! Role models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RolePermissions {
    pub manage_server: Option<bool>,
    pub manage_channels: Option<bool>,
    pub manage_roles: Option<bool>,
    pub ban_members: Option<bool>,
    pub pin_message: Option<bool>,
    pub everyone_message: Option<bool>,
    pub send_attachment: Option<bool>,
}

/// Full role with permissions (used in the role management UI).
#[derive(Debug, Clone, Deserialize)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub permissions: RolePermissions,
    pub created_at: DateTime<Utc>,
}

/// Condensed role info embedded in user objects.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RoleSummary {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
}

/// Backend roles are ordered by creation; use the first valid assigned color.
pub fn role_rgb(roles:&[RoleSummary])->Option<(u16,u16,u16)>{
    roles.iter().filter_map(|role|role.color.as_deref()).find_map(|color|{
        let hex=color.strip_prefix('#')?;if hex.len()!=6||!hex.bytes().all(|c|c.is_ascii_hexdigit()){return None;}
        let component=|start|u16::from_str_radix(&hex[start..start+2],16).ok().map(|v|v*257);
        Some((component(0)?,component(2)?,component(4)?))
    })
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn role_color_falls_back_and_never_interprets_markup(){
        let role=|color:&str|RoleSummary{id:Uuid::nil(),name:"Member".into(),color:Some(color.into())};
        assert_eq!(role_rgb(&[role("bad"),role("#12aBef"),role("#ffffff")]),Some((0x1212,0xabab,0xefef)));
        for color in ["#123","#xyzxyz","#ff0000; background:red","<span>","#é0000"]{assert_eq!(role_rgb(&[role(color)]),None);}
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserRole {
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub assigned_at: DateTime<Utc>,
}

// ── Requests ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub color: Option<String>,
    pub permissions: Option<RolePermissions>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateRoleRequest {
    pub name: String,
    pub color: Option<String>,
    pub permissions: Option<RolePermissions>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssignRoleRequest {
    pub role_id: Uuid,
}
