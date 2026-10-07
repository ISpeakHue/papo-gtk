//! UI access decisions mirror services/messages.go; the backend remains authoritative.
use super::{ChannelPermissionEntry, Role, RoleSummary};
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Access {
    pub read: bool,
    pub send: bool,
    pub delete_others: bool,
    pub voice: bool,
    pub pin: bool,
    pub attachments: bool,
    pub manage_server: bool,
    pub manage_channels: bool,
    pub manage_roles: bool,
    pub ban: bool,
    pub everyone: bool,
}

impl Access {
    pub fn resolve(user: Uuid, owner: Option<Uuid>, assigned: &[RoleSummary], roles: &[Role], overrides: &[ChannelPermissionEntry]) -> Self {
        let owns = owner == Some(user);
        let has_role = |id| assigned.iter().any(|role| role.id == id);
        let server = |predicate: fn(&super::RolePermissions) -> Option<bool>| owns ||
            roles.iter().any(|role| has_role(role.id) && predicate(&role.permissions) == Some(true));
        let channel = |free, predicate: fn(&super::ChannelPermissions) -> Option<bool>| owns ||
            (free && overrides.is_empty()) || overrides.iter().any(|entry|
                has_role(entry.role_id) && predicate(&entry.permissions) == Some(true));
        let read = channel(true, |p| p.read_channel);
        Self {
            read,
            send: read && channel(true, |p| p.send_messages),
            delete_others: read && channel(false, |p| p.delete_messages),
            voice: channel(true, |p| p.connect_voice),
            pin: read && server(|p| p.pin_message),
            attachments: read && channel(true, |p| p.send_messages) && server(|p| p.send_attachment),
            manage_server: server(|p| p.manage_server),
            manage_channels: server(|p| p.manage_channels),
            manage_roles: server(|p| p.manage_roles),
            ban: server(|p| p.manage_server), // Actual ban-route guard, not ban_members.
            everyone: server(|p| p.everyone_message),
        }
    }

    pub fn without_channel(&self) -> Self {
        Self { manage_server: self.manage_server, manage_channels: self.manage_channels,
            manage_roles: self.manage_roles, ban: self.ban, everyone: self.everyone,
            ..Self::default() }
    }

    pub fn can_edit(&self, user: Uuid, author: Option<Uuid>) -> bool { self.read && author == Some(user) }
    pub fn can_delete(&self, user: Uuid, author: Option<Uuid>) -> bool { self.read && (author == Some(user) || self.delete_others) }
    pub fn can_delete_emoji(&self, user: Uuid, creator: Option<Uuid>) -> bool { self.manage_server || creator == Some(user) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn owner_open_restricted_and_multiple_roles_follow_backend_rules() {
        let user = Uuid::new_v4(); let role_id = Uuid::new_v4();
        let role: Role = serde_json::from_value(json!({"id":role_id,"name":"staff","created_at":"2026-10-03T12:00:00Z",
            "permissions":{"manage_server":true,"pin_message":true,"send_attachment":true,"ban_members":false}})).unwrap();
        let assigned: RoleSummary = serde_json::from_value(json!({"id":role_id,"name":"staff"})).unwrap();
        let entry: ChannelPermissionEntry = serde_json::from_value(json!({"role_id":role_id,"role_name":"staff",
            "permissions":{"read_channel":true,"send_messages":false,"delete_messages":true,"connect_voice":false}})).unwrap();
        let open = Access::resolve(user, None, &[], &[], &[]);
        assert!(open.read && open.send && open.voice);
        assert!(!open.delete_others && !open.pin && !open.attachments);
        assert!(open.can_delete(user, Some(user)) && open.can_edit(user, Some(user)));
        assert!(!open.can_delete(user, None));
        let denied = Access::resolve(user, None, &[], &[role.clone()], &[entry.clone()]);
        assert!(!denied.read && !denied.send && !denied.delete_others);
        let staff = Access::resolve(user, None, &[assigned.clone()], &[role.clone()], &[entry.clone()]);
        assert!(staff.read && staff.delete_others && staff.pin && staff.ban);
        assert!(!staff.send && !staff.attachments && !staff.voice);
        let owner = Access::resolve(user, Some(user), &[], &[], &[entry.clone()]);
        assert!(owner.send && owner.pin && owner.delete_others && owner.manage_roles && owner.voice);
        let other_role = Uuid::new_v4();
        let other: RoleSummary = serde_json::from_value(json!({"id":other_role,"name":"writers"})).unwrap();
        let grant: ChannelPermissionEntry = serde_json::from_value(json!({"role_id":other_role,"role_name":"writers",
            "permissions":{"send_messages":true}})).unwrap();
        let combined = Access::resolve(user, None, &[assigned, other], &[role], &[entry, grant]);
        assert!(combined.send && combined.attachments);
        assert!(!Access::default().read);
        let global = owner.without_channel();
        assert!(global.manage_server && global.manage_roles && global.ban);
        assert!(!global.read && !global.send && !global.pin && !global.attachments);
    }
}
