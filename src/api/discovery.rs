use super::*;

impl ApiClient {
    pub async fn search(&self, request: &SearchRequest, cursor: Option<MessageCursor>) -> Result<SearchResponse> {
        request.validate()?;
        let mut url = self.url("/search");
        if let Some(c) = cursor { url.query_pairs_mut().append_pair("since", &c.created_at.to_rfc3339()).append_pair("last_id", &c.id.to_string()); }
        let response: SearchResponse = Self::decode(self.inner.post(url).json(request).send_paced(&self.requests).await?, "buscar mensagens").await?;
        anyhow::ensure!(response.results.iter().all(|r| r.kind == "message"), "Tipo de resultado desconhecido.");
        Ok(response)
    }
    pub async fn notifications(&self, user: Uuid, cursor: Option<MessageCursor>) -> Result<NotificationList> {
        let mut url = self.url(&format!("/users/{user}/notifications"));
        url.query_pairs_mut().append_pair("order", "desc");
        if let Some(c) = cursor { url.query_pairs_mut().append_pair("since", &c.created_at.to_rfc3339()).append_pair("last_id", &c.id.to_string()); }
        Self::decode(self.inner.get(url).send_paced(&self.requests).await?, "notificações").await
    }
    pub async fn read_notifications(&self, user: Uuid, ids: &[Uuid]) -> Result<u32> {
        anyhow::ensure!(!ids.is_empty() && ids.len() <= 1000, "Escolha de 1 a 1000 notificações.");
        #[derive(serde::Deserialize)] struct Response { updated: u32 }
        let response: Response = Self::decode(self.inner.put(self.url(&format!("/users/{user}/read_notification")))
            .json(&serde_json::json!({"notification_ids":ids})).send_paced(&self.requests).await?, "marcar notificações lidas").await?;
        Ok(response.updated)
    }
}
