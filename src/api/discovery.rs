//! EN-US: Authenticated history navigation, search, notifications and read acknowledgments.
//! PT-BR: Navegação autenticada no histórico, busca, notificações e confirmações de leitura.
use super::*;

impl ApiClient {
    pub async fn messages_around(&self,channel:Uuid,target:MessageCursor)->Result<(MessageListResponse,bool)>{
        let inclusive=if let Some(id)=target.id.as_u128().checked_add(1){MessageCursor{created_at:target.created_at,id:Uuid::from_u128(id)}}else{MessageCursor{created_at:target.created_at.checked_add_signed(chrono::Duration::microseconds(1)).context("Data fora do intervalo")?,id:Uuid::nil()}};
        let mut before=self.message_page(channel,Some(inclusive),30).await?;
        let after=self.newer_page(channel,target,30).await?;
        let has_newer=after.has_more;
        anyhow::ensure!(before.messages.iter().any(|m|m.id==target.id),"A mensagem foi removida ou não está mais acessível");
        before.messages.extend(after.messages);before.messages.sort_by_key(|m|(m.created_at,m.id));before.messages.dedup_by_key(|m|m.id);Ok((before,has_newer))
    }

    pub async fn messages_after(&self,channel:Uuid,cursor:MessageCursor)->Result<MessageListResponse>{
        self.newer_page(channel,cursor,100).await
    }
    async fn newer_page(&self,channel:Uuid,cursor:MessageCursor,limit:u32)->Result<MessageListResponse>{
        let mut url=self.url(&format!("/channels/{channel}/messages"));
        url.query_pairs_mut().append_pair("order","asc").append_pair("limit",&limit.to_string()).append_pair("since",&cursor.created_at.to_rfc3339()).append_pair("last_id",&cursor.id.to_string());
        let page:MessageListResponse=Self::decode(self.inner.get(url).send_paced(&self.requests).await?,"messages").await?;
        anyhow::ensure!(page.channel_id==channel&&page.messages.iter().all(|m|m.channel_id==channel&&(m.created_at,m.id)>(cursor.created_at,cursor.id)),"Página de mensagens seguintes inválida");
        anyhow::ensure!(!page.has_more||!page.messages.is_empty(),"Página de mensagens seguintes sem progresso");
        Ok(page)
    }

    /// Small, read-only page including the pending message. The successor UUID
    /// makes the exclusive descending cursor inclusive without skipping ties.
    pub async fn moderation_messages(&self, channel:Uuid, message:&Message)->Result<MessageListResponse>{
        anyhow::ensure!(message.channel_id==channel,"Mensagem de outro canal.");
        let cursor=if let Some(next)=message.id.as_u128().checked_add(1){
            MessageCursor{created_at:message.created_at,id:Uuid::from_u128(next)}
        }else{
            MessageCursor{created_at:message.created_at.checked_add_signed(chrono::Duration::microseconds(1)).context("Data da mensagem fora do intervalo")?,id:Uuid::nil()}
        };
        self.message_page(channel,Some(cursor),30).await
    }
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

    /// Listing history does not acknowledge it. Call only for a viewed message.
    pub async fn mark_channel_read(&self, channel:Uuid, message:Uuid)->Result<()> {
        Self::check_response(self.inner.put(self.url(&format!("/channels/{channel}/read")))
            .json(&serde_json::json!({"message_id":message})).send_paced(&self.requests).await?,
            "marcar canal lido").await?;
        Ok(())
    }
    /// Explicit user action: resolve the current head without opening the chat.
    pub async fn mark_latest_read(&self,channel:Uuid)->Result<Option<Message>>{
        let page=self.message_page(channel,None,1).await?;
        let newest=page.messages.into_iter().filter(|m|m.channel_id==channel).max_by_key(|m|(m.created_at,m.id));
        if let Some(message)=&newest{self.mark_channel_read(channel,message.id).await?;}
        Ok(newest)
    }
}
