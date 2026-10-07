use super::*;
impl ApiClient {
    pub async fn set_user_banned(&self,user:Uuid,banned:bool)->Result<()>{
        Self::check_response(self.inner.put(self.url(&format!("/users/{user}/ban"))).json(&serde_json::json!({"ban_state":banned})).send().await?,"alterar banimento").await?;Ok(())
    }
    pub async fn create_recovery_link(&self,own:Uuid,user:Uuid)->Result<RecoveryLink>{
        anyhow::ensure!(own!=user,"Use a recuperação da própria conta para alterar sua senha.");
        let result=Self::decode(self.inner.post(self.url(&format!("/users/{user}/reset"))).send().await?,"gerar recuperação").await;
        // A misconfigured proxy can echo tokens in an error response. Preserve status only.
        result.map_err(|e|if let Some(e)=e.downcast_ref::<ApiError>(){ApiError{status:e.status,code:e.code.as_ref().filter(|c|matches!(c.as_str(),"banned"|"unauthorized"|"forbidden"|"not-found"|"invalid-param")).cloned(),message:"Não foi possível gerar o link de recuperação.".into()}.into()}else{anyhow::anyhow!("Resposta de recuperação inválida.")})
    }
    pub async fn audit_logs(&self,filter:&AuditFilter,cursor:Option<Uuid>)->Result<AuditPage>{
        filter.validate()?;let mut url=self.url("/admin/audit-logs");
        {let mut q=url.query_pairs_mut();q.append_pair("order",if filter.ascending{"asc"}else{"desc"});
        if !filter.action.is_empty(){q.append_pair("action",&filter.action);}if !filter.entity_type.is_empty(){q.append_pair("entity_type",&filter.entity_type);}
        if let Some(id)=filter.actor_id{q.append_pair("actor_id",&id.to_string());}if let Some(id)=cursor{q.append_pair("last_id",&id.to_string());}
        if let Some(d)=filter.since{q.append_pair("since",&d.to_rfc3339_opts(chrono::SecondsFormat::Nanos,true));}if let Some(d)=filter.until{q.append_pair("until",&d.to_rfc3339_opts(chrono::SecondsFormat::Nanos,true));}}
        let page:AuditPage=Self::decode(self.inner.get(url).send().await?,"consultar auditoria").await?;
        anyhow::ensure!(!page.has_more||page.logs.last().is_some_and(|e|Some(e.id)!=cursor),"Paginação de auditoria sem progresso.");Ok(page)
    }
}
