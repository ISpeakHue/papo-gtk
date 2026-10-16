//! Webhook management uses the user's session; execution belongs to external integrations.
use super::*;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum AvatarFailure {Missing,Invalid,Transient,RateLimited(std::time::Duration)}
impl std::fmt::Display for AvatarFailure{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("Avatar indisponível")}}
impl std::error::Error for AvatarFailure{}
impl ApiClient {
    pub async fn list_webhooks(&self) -> Result<Vec<Webhook>> {
        let list: WebhookList = Self::decode(self.inner.get(self.url("/webhooks")).send_paced(&self.requests).await?, "listar webhooks").await?;
        Ok(list.webhooks)
    }
    pub async fn create_webhook(&self, channel: Uuid, name: &str) -> Result<CreatedWebhook> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty() && name.chars().count() <= 80, "Use um nome de 1 a 80 caracteres.");
        Self::decode(self.inner.post(self.url("/webhooks")).json(&serde_json::json!({"channel_id":channel,"name":name})).send_paced(&self.requests).await?, "criar webhook").await
    }
    pub async fn update_webhook(&self, id: Uuid, identity: &WebhookIdentity) -> Result<Webhook> {
        anyhow::ensure!(identity.display_name.as_ref().is_none_or(|n| n.chars().count() <= 100), "Use um apelido de até 100 caracteres.");
        Self::decode(self.inner.patch(self.url(&format!("/webhooks/{id}"))).json(identity).send_paced(&self.requests).await?, "editar webhook").await
    }
    pub async fn revoke_webhook(&self, id: Uuid) -> Result<()> {
        Self::check_response(self.inner.delete(self.url(&format!("/webhooks/{id}"))).send_paced(&self.requests).await?, "revogar webhook").await?;
        Ok(())
    }
    pub fn webhook_endpoint(&self, id: Uuid) -> String { self.url(&format!("/webhooks/{id}/messages")).to_string() }

    /// Public avatar requests never use the authenticated client's cookie jar.
    /// Relative URLs are accepted only for the documented webhook-avatar route.
    pub(crate) fn external_avatar_url(&self, value: &str) -> Option<Url> {
        if value.is_empty() || value.len() > 4096 { return None; }
        if let Some(path) = value.strip_prefix("/webhooks/") {
            let id = path.strip_suffix("/avatar")?.parse::<Uuid>().ok()?;
            return Some(self.url(&format!("/webhooks/{id}/avatar")));
        }
        let url = Url::parse(value).ok()?;
        if url.origin()==self.base.origin() && url.query().is_none() && url.fragment().is_none(){
            if let Some(id)=url.path().strip_prefix(self.base.path()).and_then(|p|p.strip_prefix("webhooks/")).and_then(|p|p.strip_suffix("/avatar")).and_then(|p|p.parse::<Uuid>().ok()){
                if url==self.url(&format!("/webhooks/{id}/avatar")){return Some(url);}
            }
        }
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() { return None; }
        let host = url.host_str()?;
        if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") { return None; }
        if let Ok(ip) = host.trim_matches(['[',']']).parse::<std::net::IpAddr>() {
            let public = match ip {
                std::net::IpAddr::V4(ip) => !ip.is_private() && !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast(),
                std::net::IpAddr::V6(_) => false,
            };
            if !public { return None; }
        }
        Some(url)
    }
    pub(crate) fn external_avatar_delay(&self,value:&str)->Duration{
        self.external_avatar_url(value).map_or(Duration::ZERO,|url|if url.origin()==self.base.origin(){self.requests.remaining()}else{requests::avatar_origin(&url.origin().ascii_serialization()).remaining()})
    }
    pub(crate) async fn external_avatar(&self, value: &str) -> std::result::Result<Vec<u8>,AvatarFailure> {
        use futures_util::StreamExt;
        let url = self.external_avatar_url(value).ok_or(AvatarFailure::Invalid)?;
        // No redirects, ambient proxy credentials, cookies, or Authorization.
        static PUBLIC: std::sync::LazyLock<std::result::Result<Client,reqwest::Error>> = std::sync::LazyLock::new(|| Client::builder().redirect(reqwest::redirect::Policy::none()).no_proxy().timeout(Duration::from_secs(12)).build());
        let client = PUBLIC.as_ref().map_err(|_|AvatarFailure::Transient)?;
        let same_server = url.origin() == self.base.origin();
        let budget=requests::avatar_origin(&url.origin().ascii_serialization());
        let response=client.get(url).send_paced(if same_server{&self.requests}else{&budget}).await.map_err(|_|AvatarFailure::Transient)?;
        match response.status(){
            reqwest::StatusCode::NOT_FOUND|reqwest::StatusCode::GONE=>return Err(AvatarFailure::Missing),
            reqwest::StatusCode::TOO_MANY_REQUESTS=>return Err(AvatarFailure::RateLimited(if same_server{self.requests.remaining()}else{budget.remaining()})),
            status if !status.is_success()=>return Err(AvatarFailure::Transient), _=>{}
        }
        let limit = 2 << 20;
        if response.content_length().is_some_and(|n|n>limit){return Err(AvatarFailure::Invalid);}
        let mut bytes = Vec::new(); let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await { let chunk=chunk.map_err(|_|AvatarFailure::Transient)?; if bytes.len()+chunk.len()>limit as usize{return Err(AvatarFailure::Invalid);} bytes.extend_from_slice(&chunk); }
        Ok(bytes)
    }
}
