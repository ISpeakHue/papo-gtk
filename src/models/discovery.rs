//! Search summaries and notification records have their own contracts, not Message fields.
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchRequest {
    pub text: String,
    pub author: String,
    pub channel_id: String,
    pub mention: String,
    pub has: String,
    pub order: String,
    pub date_start: String,
    pub date_end: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_attachment: Option<bool>,
}

impl SearchRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.text.trim().is_empty() || !self.author.is_empty() || !self.channel_id.is_empty()
            || !self.mention.is_empty() || !self.has.is_empty() || !self.date_start.is_empty()
            || !self.date_end.is_empty() || self.contains_attachment.is_some(), "Escolha pelo menos um filtro.");
        for id in [&self.author, &self.channel_id, &self.mention] {
            if !id.is_empty() { Uuid::parse_str(id).map_err(|_| anyhow::anyhow!("Autor, canal e menção devem ser IDs válidos."))?; }
        }
        anyhow::ensure!(matches!(self.has.as_str(), "" | "link") && matches!(self.order.as_str(), "" | "asc" | "desc"), "Filtro ou ordem inválida.");
        let parse = |date: &str| -> anyhow::Result<Option<NaiveDate>> {
            if date.is_empty() { return Ok(None); }
            let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| anyhow::anyhow!("Use datas no formato AAAA-MM-DD."))?;
            anyhow::ensure!(parsed.format("%Y-%m-%d").to_string() == date, "Use datas no formato AAAA-MM-DD.");
            Ok(Some(parsed))
        };
        if let (Some(start), Some(end)) = (parse(&self.date_start)?, parse(&self.date_end)?) {
            anyhow::ensure!(start <= end, "A data inicial deve preceder a final.");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: Uuid,
    pub content: Option<String>,
    pub channel_id: Uuid,
    pub channel_name: String,
    pub author_id: Option<Uuid>,
    pub author_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub score: Option<f64>,
}
#[derive(Debug, Deserialize)]
pub struct SearchResponse { pub results: Vec<SearchResult>, pub has_more: bool }

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationSummary {
    pub id: Uuid,
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub author_id: Option<Uuid>,
    pub message_content: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Deserialize)]
pub struct NotificationList { pub notifications: Vec<NotificationSummary>, pub has_more: bool }

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationEvent {
    pub id: Uuid,
    /// Backend field user_id is the AUTHOR, not the recipient.
    pub user_id: Option<Uuid>,
    pub message_id: Uuid,
    pub message_content: String,
}
