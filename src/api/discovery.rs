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
    /// Resolve the newest own message with complete metadata, so editing cannot
    /// accidentally discard custom embeds from a search result's partial fields.
    pub async fn latest_own_message(&self,channel:Uuid,user:Uuid)->Result<Option<Message>>{
        let found=self.search(&SearchRequest{channel_id:channel.to_string(),author:user.to_string(),order:"desc".into(),..Default::default()},None).await?;
        let Some(target)=found.results.into_iter().filter(|m|m.channel_id==channel&&m.author_id==Some(user)).max_by_key(|m|(m.created_at,m.id))else{return Ok(None);};
        let at=target.created_at.checked_add_signed(chrono::Duration::microseconds(1)).context("Data da mensagem fora do intervalo")?;
        let mut cursor=MessageCursor{created_at:at,id:Uuid::from_bytes([255;16])};
        for _ in 0..4{
            let page=self.list_messages(channel,Some(cursor)).await?;
            if let Some(message)=page.messages.iter().find(|m|m.id==target.id&&m.author_id==Some(user)&&m.channel_id==channel){return Ok(Some(message.clone()));}
            let next=page.messages.iter().min_by_key(|m|(m.created_at,m.id)).map(MessageCursor::from);
            if !page.has_more||next.is_none_or(|next|(next.created_at,next.id)>=(cursor.created_at,cursor.id)||next.created_at<target.created_at){break;}
            cursor=next.unwrap();
        }
        anyhow::bail!("Sua mensagem não está mais disponível. Atualize o canal e tente novamente.")
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
