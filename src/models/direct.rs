//! Direct conversations are participant channels, independent of public channel roles.
use super::*;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;
#[derive(Debug, Clone, Deserialize)]
pub struct DirectConversation {
    pub id: Uuid, pub user: UserSummary, pub created_at: DateTime<Utc>,
    pub last_message: Option<ChannelLastMessage>, pub last_read_message: Option<Uuid>,
    pub last_read_at: Option<DateTime<Utc>>, pub unread_count: u32,
}
#[derive(Debug, Deserialize)]
pub struct DirectList { pub dms: Vec<DirectConversation> }
#[derive(Debug, Deserialize)]
pub struct BlockList { pub users: Vec<UserSummary> }
#[derive(Debug, Clone)]
pub enum ConversationTarget { Channel(Channel), Direct(DirectConversation) }
impl ConversationTarget {
    pub fn id(&self)->Uuid { match self {Self::Channel(c)=>c.id,Self::Direct(d)=>d.id} }
    pub fn name(&self)->&str {match self{Self::Channel(c)=>&c.name,Self::Direct(d)=>d.user.display_name()}}
    pub fn topic(&self)->Option<&str>{match self{Self::Channel(c)=>c.topic.as_deref(),Self::Direct(_)=>Some("Mensagem direta")}}
    pub fn is_direct(&self)->bool{matches!(self,Self::Direct(_))}
}
impl Access {
    /// Call only after GET/POST /dms verifies membership and the absence of a block.
    pub fn direct()->Self{Self{read:true,send:true,pin:true,attachments:true,..Self::default()}}
}
