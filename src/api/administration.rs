//! DM, block and administration contracts verified against the Go handlers.
use super::*;
impl ApiClient {
    pub async fn list_direct(&self)->Result<Vec<DirectConversation>>{
        let r:DirectList=Self::decode(self.inner.get(self.url("/dms")).send_paced(&self.requests).await?,"listar DMs").await?;Ok(r.dms)
    }
    pub async fn open_direct(&self,own:Uuid,user:Uuid)->Result<DirectConversation>{
        anyhow::ensure!(own!=user,"Não é possível enviar uma mensagem direta para si mesmo.");
        Self::decode(self.inner.post(self.url("/dms")).json(&serde_json::json!({"user_id":user})).send_paced(&self.requests).await?,"abrir DM").await
    }
    pub async fn get_direct(&self,id:Uuid)->Result<DirectConversation>{
        let d:DirectConversation=Self::decode(self.inner.get(self.url(&format!("/dms/{id}"))).send_paced(&self.requests).await?,"consultar DM").await?;
        anyhow::ensure!(d.id==id,"Conversa retornada para outro ID");Ok(d)
    }
    pub async fn hide_direct(&self,id:Uuid)->Result<()>{Self::check_response(self.inner.delete(self.url(&format!("/dms/{id}"))).send_paced(&self.requests).await?,"ocultar DM").await?;Ok(())}
    pub async fn list_blocks(&self)->Result<Vec<UserSummary>>{let r:BlockList=Self::decode(self.inner.get(self.url("/users/blocks")).send_paced(&self.requests).await?,"listar bloqueios").await?;Ok(r.users)}
    pub async fn block_user(&self,own:Uuid,user:Uuid,block:bool)->Result<()>{
        anyhow::ensure!(own!=user,"Não é possível bloquear a si mesmo.");
        let url=self.url(&format!("/users/{user}/block"));
        let request=if block{self.inner.post(url)}else{self.inner.delete(url)};
        Self::check_response(request.send_paced(&self.requests).await?,"alterar bloqueio").await?;Ok(())
    }
    pub async fn create_role(&self,r:&CreateRoleRequest)->Result<Role>{Self::decode(self.inner.post(self.url("/roles")).json(r).send_paced(&self.requests).await?,"criar cargo").await}
    pub async fn update_role(&self,id:Uuid,r:&UpdateRoleRequest)->Result<Role>{Self::decode(self.inner.put(self.url(&format!("/roles/{id}"))).json(r).send_paced(&self.requests).await?,"editar cargo").await}
    pub async fn delete_role(&self,id:Uuid)->Result<()>{Self::check_response(self.inner.delete(self.url(&format!("/roles/{id}"))).send_paced(&self.requests).await?,"excluir cargo").await?;Ok(())}
    pub async fn assign_role(&self,user:Uuid,role:Uuid)->Result<UserRole>{Self::decode(self.inner.post(self.url(&format!("/users/{user}/roles"))).json(&AssignRoleRequest{role_id:role}).send_paced(&self.requests).await?,"atribuir cargo").await}
    pub async fn remove_role(&self,user:Uuid,role:Uuid)->Result<()>{Self::check_response(self.inner.delete(self.url(&format!("/users/{user}/roles/{role}"))).send_paced(&self.requests).await?,"remover cargo").await?;Ok(())}
    async fn decode_server(response:reqwest::Response,r:&ServerWrite)->Result<Server>{
        let result=Self::decode(response,"salvar servidor").await;
        result.map_err(|e|{
            if r.password.is_some(){if let Some(error)=e.downcast_ref::<ApiError>(){return ApiError{status:error.status,code:error.code.clone(),message:"Não foi possível salvar o servidor. Verifique nome, ícone e política de senha.".into()}.into();}}
            e
        })
    }
    pub async fn create_server(&self,r:&ServerWrite)->Result<Server>{Self::decode_server(self.inner.post(self.url("/server")).json(r).send_paced(&self.requests).await?,r).await}
    pub async fn patch_server(&self,r:&ServerWrite)->Result<Server>{Self::decode_server(self.inner.patch(self.url("/server")).json(r).send_paced(&self.requests).await?,r).await}
    pub async fn create_channel(&self,r:&CreateChannelRequest)->Result<Channel>{Self::decode(self.inner.post(self.url("/channels")).json(r).send_paced(&self.requests).await?,"criar canal").await}
    pub async fn update_channel(&self,id:Uuid,r:&UpdateChannelRequest)->Result<Channel>{Self::decode(self.inner.put(self.url(&format!("/channels/{id}"))).json(r).send_paced(&self.requests).await?,"editar canal").await}
    pub async fn delete_channel(&self,id:Uuid)->Result<()>{Self::check_response(self.inner.delete(self.url(&format!("/channels/{id}"))).send_paced(&self.requests).await?,"excluir canal").await?;Ok(())}
    pub async fn move_channel(&self,id:Uuid,r:&ChangeChannelPositionRequest)->Result<Channel>{Self::decode(self.inner.put(self.url(&format!("/channels/{id}/change_position"))).json(r).send_paced(&self.requests).await?,"mover canal").await}
    pub async fn set_override(&self,channel:Uuid,role:Uuid,r:&ChannelPermissions)->Result<()>{Self::check_response(self.inner.put(self.url(&format!("/channels/{channel}/permissions/{role}"))).json(&UpdateChannelPermissionsRequest{permissions:r.clone()}).send_paced(&self.requests).await?,"alterar permissões").await?;Ok(())}
    pub async fn remove_override(&self,channel:Uuid,role:Uuid)->Result<()>{Self::check_response(self.inner.delete(self.url(&format!("/channels/{channel}/role/{role}"))).send_paced(&self.requests).await?,"remover permissões").await?;Ok(())}
}
