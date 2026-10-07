//! Restricted moderation contracts. Recovery tokens and audit metadata are never logged.
use chrono::{DateTime,Utc};
use serde::Deserialize;
use uuid::Uuid;
#[derive(Clone,Deserialize)]
pub struct RecoveryLink {pub reset_url:String,pub expires_at:DateTime<Utc>}
impl std::fmt::Debug for RecoveryLink {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("RecoveryLink").field("expires_at",&self.expires_at).finish_non_exhaustive()}}
#[derive(Clone,Deserialize)]
pub struct AuditEntry {pub id:Uuid,pub actor_username:String,pub action:String,pub entity_type:String,pub target_user_id:Option<Uuid>,pub metadata:Option<serde_json::Value>,pub created_at:DateTime<Utc>}
impl std::fmt::Debug for AuditEntry {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("AuditEntry").field("id",&self.id).field("action",&self.action).finish_non_exhaustive()}}
#[derive(Debug,Clone,Deserialize)]
pub struct AuditPage {pub logs:Vec<AuditEntry>,pub has_more:bool}
#[derive(Debug,Clone,Default)]
pub struct AuditFilter {pub action:String,pub actor_id:Option<Uuid>,pub entity_type:String,pub since:Option<DateTime<Utc>>,pub until:Option<DateTime<Utc>>,pub ascending:bool}
impl AuditFilter {pub fn validate(&self)->anyhow::Result<()>{anyhow::ensure!(!matches!((self.since,self.until),(Some(s),Some(u)) if s>u),"A data inicial deve preceder a data final.");Ok(())}}
