//! EN-US: Channel-scoped machine identities; webhook secrets exist only at creation.
//! PT-BR: Identidades de integração vinculadas ao canal; segredos de webhook são expostos apenas na criação.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ExternalAuthor {
    pub source: String,
    pub id: String,
    pub name: String,
    #[serde(default)] pub avatar_url: String,
    #[serde(default)] pub profile_url: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Webhook {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub name: String,
    pub display_name: Option<String>,
    #[expect(dead_code, reason = "Retained webhook management metadata; serde validates creator and timestamp types.")]
    pub created_by: Uuid,
    #[expect(dead_code, reason = "Retained webhook management metadata; serde validates creator and timestamp types.")]
    pub created_at: DateTime<Utc>,
    #[expect(dead_code, reason = "Retained webhook management metadata; serde validates creator and timestamp types.")]
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(Deserialize)]
pub struct CreatedWebhook {
    #[serde(flatten)] pub webhook: Webhook,
    pub token: String,
}
impl std::fmt::Debug for CreatedWebhook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedWebhook").field("webhook", &self.webhook).field("token", &"[REDACTED]").finish()
    }
}
#[derive(Debug, Deserialize)]
pub struct WebhookList { pub webhooks: Vec<Webhook> }
#[derive(Default, Serialize)]
pub struct WebhookIdentity {
    #[serde(skip_serializing_if="Option::is_none")] pub display_name: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub avatar: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub avatar_format: Option<String>,
}

#[cfg(test)]mod tests{
    use super::*;
    use crate::models::*;
    use serde_json::json;
    #[test]fn identities_are_optional_and_never_confer_human_ownership(){
        let hook=Uuid::new_v4();let mut value=json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"author_id":null,"created_at":"2026-10-14T20:00:00Z","webhook_id":hook,"external_author":{"source":"discord","id":"123456789012345678","name":"Discord sender","avatar_url":"https://cdn.discordapp.com/avatars/123/a.png"}});
        let users=std::collections::HashMap::new();
        for author in [serde_json::Value::Null,json!("")]{value["author_id"]=author;let message:Message=serde_json::from_value(value.clone()).unwrap();assert_eq!(message.sender_name(&users),"Discord sender");assert_eq!(message.external().unwrap().id,"123456789012345678");assert!(!Access::direct().can_edit(hook,message.author_id));}
        value["author_id"]=json!(hook);let human:Message=serde_json::from_value(value.clone()).unwrap();assert!(human.external().is_none());
        value.as_object_mut().unwrap().remove("external_author");value.as_object_mut().unwrap().remove("webhook_id");value["author_id"]=serde_json::Value::Null;
        let deleted:Message=serde_json::from_value(value).unwrap();assert_eq!(deleted.sender_name(&users),"Usuário removido");
    }
    #[test]fn manage_webhooks_is_independent_of_other_management_permissions(){
        let id=Uuid::new_v4();let assigned:RoleSummary=serde_json::from_value(json!({"id":id,"name":"Staff"})).unwrap();
        for (permissions,allowed) in [(json!({"manage_server":true,"manage_channels":true}),false),(json!({"manage_webhooks":true}),true),(json!({}),false)]{
            let role:Role=serde_json::from_value(json!({"id":id,"name":"Staff","permissions":permissions,"created_at":"2026-10-14T20:00:00Z"})).unwrap();
            let access=Access::resolve(Uuid::nil(),None,std::slice::from_ref(&assigned),&[role],&[]);assert_eq!(access.manage_webhooks,allowed);assert_eq!(access.without_channel().manage_webhooks,allowed);
        }
        assert!(Access::resolve(id,Some(id),&[],&[],&[]).manage_webhooks);assert!(!Access::direct().manage_webhooks);
    }
    #[test]fn webhook_grouping_uses_the_external_identity_and_channel_not_just_the_hook(){
        let message:Message=serde_json::from_value(json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"webhook_id":Uuid::new_v4(),"created_at":"2026-10-15T12:00:00Z","external_author":{"source":"discord","id":"123","name":"Sender"}})).unwrap();
        let mut next=message.clone();next.id=Uuid::new_v4();assert!(next.same_sender(&message));
        for part in ["hook","channel","source","id","missing","empty"]{
            let mut other=message.clone();match part{
                "hook"=>other.webhook_id=Some(Uuid::new_v4()),"channel"=>other.channel_id=Uuid::new_v4(),
                "source"=>other.external_author.as_mut().unwrap().source="irc".into(),"id"=>other.external_author.as_mut().unwrap().id="456".into(),
                "missing"=>other.external_author=None,_=>other.external_author.as_mut().unwrap().id.clear(),
            }assert!(!other.same_sender(&message),"different {part} must split groups");
        }
        let mut deleted=message.clone();deleted.webhook_id=None;deleted.external_author=None;assert!(!deleted.same_sender(&deleted));
        let mut human=message.clone();human.author_id=Some(Uuid::new_v4());assert!(human.same_sender(&human));assert!(!human.same_sender(&message));
    }
}
