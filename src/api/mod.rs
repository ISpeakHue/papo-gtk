//! Async REST API client for the Papo backend.
//!
//! All methods return `anyhow::Result<T>` so callers can use `?` freely.
//! The underlying `reqwest::Client` is configured to store cookies for the
//! session (the backend sends the JWT exclusively via an HttpOnly cookie).

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode, Url};
use uuid::Uuid;
use std::sync::Arc;
use std::time::Duration;
use reqwest::cookie::{CookieStore, Jar};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, handshake::client::Request};

use crate::models::*;
pub mod features;
pub mod security;
mod discovery;
mod administration;
mod moderation;
mod voice;
mod requests;
use requests::{PacedRequest, RequestScheduler};

/// Preserve status and RFC 7807 problem codes so expired sessions reach the login view.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub status: StatusCode,
    pub code: Option<String>,
    pub message: String,
}

pub fn is_session_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ApiError>().is_some_and(|error|
        error.status == StatusCode::UNAUTHORIZED ||
        (error.status == StatusCode::FORBIDDEN && error.code.as_deref() == Some("banned")))
}

pub fn is_permission_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ApiError>().is_some_and(|e| e.status == StatusCode::FORBIDDEN) && !is_session_error(error)
}

/// Wraps a `reqwest::Client` and the base URL of the backend.
#[derive(Clone)]
pub struct ApiClient {
    inner: Client,
    base: Url,
    cookies: Arc<Jar>,
    requests: Arc<RequestScheduler>,
    session_owner: Arc<std::sync::Mutex<Uuid>>,
}
impl std::fmt::Debug for ApiClient{fn fmt(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.debug_struct("ApiClient").field("base",&self.base).finish_non_exhaustive()}}

impl ApiClient {
    /// Create a new client pointing at `base_url` (e.g. `http://localhost:8080`).
    pub fn new(base_url: &str) -> Result<Self> {
        let base = Self::parse_base_url(base_url)?;
        let cookies = Arc::new(Jar::default());
        let mut builder = Client::builder()
            .cookie_provider(cookies.clone())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30));
        // Local development must connect directly, even if a system proxy is set.
        if Self::is_loopback(&base) {
            builder = builder.no_proxy();
        }
        let inner = builder.build()
            .context("building reqwest client")?;
        Ok(Self { inner, base, cookies, requests: Arc::new(RequestScheduler::default()), session_owner: Arc::new(std::sync::Mutex::new(Uuid::new_v4())) })
    }

    fn is_loopback(url: &Url) -> bool {
        url.host_str().is_some_and(|host| {
            host == "localhost" || host.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
        })
    }

    fn parse_base_url(input: &str) -> Result<Url> {
        let input = input.trim();
        anyhow::ensure!(!input.is_empty(), "Informe o endereço do servidor.");
        let candidate = if input.contains("://") {
            input.to_owned()
        } else {
            // A bare local address is commonly pasted from a development server.
            let local = Url::parse(&format!("http://{input}"))?;
            let scheme = if Self::is_loopback(&local) { "http" } else { "https" };
            format!("{scheme}://{input}")
        };
        let mut base = Url::parse(&candidate).context("Endereço do servidor inválido")?;
        anyhow::ensure!(matches!(base.scheme(), "http" | "https") && base.host().is_some(),
            "Use um endereço HTTP ou HTTPS válido.");
        anyhow::ensure!(base.username().is_empty() && base.password().is_none(),
            "Informe as credenciais nos campos de usuário e senha.");
        anyhow::ensure!(base.query().is_none() && base.fragment().is_none(),
            "Informe o endereço base do servidor, sem parâmetros ou fragmentos.");
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        Ok(base)
    }

    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }
    pub(crate) fn auth_cookie(&self)->Option<String>{
        self.cookies.cookies(&self.url("/auth/whoami"))?.to_str().ok()?.split(';').find_map(|item|item.trim().strip_prefix("Auth=").filter(|token|!token.is_empty()).map(str::to_owned))
    }
    pub(crate) fn restore_auth_cookie(&self,token:&str)->Result<()>{
        anyhow::ensure!(self.base.scheme()=="https"||Self::is_loopback(&self.base),"Uma sessão salva exige HTTPS, exceto no localhost.");
        anyhow::ensure!(!token.is_empty()&&token.len()<=16_384&&token.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-".contains(&c)),"Sessão salva inválida.");
        self.cookies.add_cookie_str(&format!("Auth={token}; HttpOnly; Secure; Path=/; SameSite=Strict"),&self.base);Ok(())
    }

    /// Build each handshake from the current cookie jar, including refreshed cookies.
    pub fn websocket_request(&self) -> Result<Request> {
        let http_url = self.url("/ws");
        let mut ws_url = http_url.clone();
        let scheme = match http_url.scheme() {
            "https" => "wss",
            "http" => "ws",
            _ => anyhow::bail!("Server URL must use http or https"),
        };
        ws_url.set_scheme(scheme).map_err(|_| anyhow::anyhow!("Invalid WebSocket URL"))?;
        let mut request = ws_url.as_str().into_client_request()?;
        if let Some(cookies) = self.cookies.cookies(&http_url) {
            request.headers_mut().insert("Cookie", cookies);
        }
        Ok(request)
    }

    fn url(&self, path: &str) -> Url {
        self.base.join(path.trim_start_matches('/')).expect("valid API endpoint")
    }

    async fn check_response(res: reqwest::Response, op: &str) -> Result<reqwest::Response> {
        let status = res.status();
        if status.is_success() {
            return Ok(res);
        }
        let body = res.text().await.unwrap_or_default();
        let json = serde_json::from_str::<serde_json::Value>(&body).ok();
        let code = json.as_ref().and_then(|value| value["type"].as_str())
            .and_then(|value| value.rsplit('/').next()).map(str::to_owned);
        let message = json.as_ref().and_then(|value| ["detail", "message", "error"].iter()
            .find_map(|key| value[*key].as_str())).map(str::to_owned)
            .unwrap_or_else(|| {
                let text = body.trim();
                if text.starts_with('<') {
                    format!("Erro HTTP {status} ({op}): o servidor retornou uma página HTML. Verifique o endereço da API.")
                } else if !text.is_empty() && text.len() < 300 {
                    text.to_owned()
                } else {
                    format!("Erro HTTP {status} ({op})")
                }
            });
        Err(ApiError { status, code, message }.into())
    }

    async fn decode<T: serde::de::DeserializeOwned>(response: reqwest::Response, op: &str) -> Result<T> {
        Self::json_response(Self::check_response(response, op).await?).await
    }

    fn check_auth_cookie_transport(res: &reqwest::Response) -> Result<()> {
        if res.url().scheme() == "http" && !Self::is_loopback(res.url())
            && res.cookies().any(|cookie| cookie.name() == "Auth" && cookie.secure()) {
            anyhow::bail!("O servidor em {} exige cookies seguros para autenticar. Use o endereço HTTPS do servidor.", res.url());
        }
        Ok(())
    }

    fn check_api_body(body: &[u8], url: &Url, status: StatusCode, content_type: &str) -> Result<()> {
        let text = String::from_utf8_lossy(body);
        if content_type.to_ascii_lowercase().contains("text/html") || text.trim_start().starts_with('<') {
            anyhow::bail!("O endereço {url} retornou uma página HTML (HTTP {}, Content-Type: {content_type}) em vez de uma resposta da API. Verifique o endereço da API e a configuração do proxy do servidor.", status.as_u16());
        }
        Ok(())
    }

    async fn json_response<T: serde::de::DeserializeOwned>(res: reqwest::Response) -> Result<T> {
        let url = res.url().clone();
        let status = res.status();
        let content_type = res.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()).unwrap_or("não informado").to_owned();
        let body = res.bytes().await.context("Erro ao ler a resposta da API")?;
        Self::check_api_body(&body, &url, status, &content_type)?;
        anyhow::ensure!(!body.iter().all(u8::is_ascii_whitespace),
            "Resposta vazia de {url} (HTTP {}). Era esperado JSON.", status.as_u16());
        serde_json::from_slice(&body).with_context(|| format!(
            "Resposta JSON inválida de {url} (HTTP {}, Content-Type: {content_type})", status.as_u16()
        ))
    }

    async fn check_non_json_response(res: reqwest::Response) -> Result<()> {
        let url = res.url().clone();
        let status = res.status();
        let content_type = res.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()).unwrap_or("não informado").to_owned();
        let body = res.bytes().await.context("Erro ao ler a resposta da API")?;
        Self::check_api_body(&body, &url, status, &content_type)
    }

    // ── Health ───────────────────────────────────────────────────────────────

    pub async fn health(&self) -> Result<()> {
        let resp = self
            .inner
            .get(self.url("/health"))
            .send_paced(&self.requests)
            .await
            .context("GET /health")?;
        let resp = Self::check_response(resp, "GET /health").await?;
        Self::check_non_json_response(resp).await
    }

    // ── Auth ─────────────────────────────────────────────────────────────────

    /// Returns true when the server is private (requires login_server first).
    pub async fn server_is_private(&self) -> Result<bool> {
        match self.get_server().await {
            Ok(server) => Ok(!server.public.unwrap_or(true)),
            // A freshly initialized backend has no server record yet. Its
            // authentication endpoints explicitly allow this bootstrap case.
            Err(error) if error.downcast_ref::<ApiError>()
                .is_some_and(|error| error.status == StatusCode::NOT_FOUND) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub async fn login_server(&self, password: &str) -> Result<()> {
        let resp = self
            .inner
            .post(self.url("/auth/login_server"))
            .json(&LoginServerRequest {
                server_password: password.to_owned(),
            })
            .send_paced(&self.requests)
            .await
            .context("POST /auth/login_server")?;
        let resp = Self::check_response(resp, "POST /auth/login_server").await?;
        Self::check_auth_cookie_transport(&resp)?;
        Self::check_non_json_response(resp).await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<LoginResponse> {
        let resp = self
            .inner
            .post(self.url("/auth/login"))
            .json(&LoginRequest {
                username: username.to_owned(),
                password: password.to_owned(),
            })
            .send_paced(&self.requests)
            .await
            .context("POST /auth/login")?;
        let resp = Self::check_response(resp, "POST /auth/login").await?;
        Self::check_auth_cookie_transport(&resp)?;
        Self::json_response(resp).await
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<RegisterResponse> {
        let resp = self
            .inner
            .post(self.url("/auth/register"))
            .json(&RegisterRequest {
                username: username.to_owned(),
                password: password.to_owned(),
            })
            .send_paced(&self.requests)
            .await
            .context("POST /auth/register")?;
        let resp = Self::check_response(resp, "POST /auth/register").await?;
        let text = resp.text().await.unwrap_or_default();
        if text.trim().is_empty() {
            return Ok(RegisterResponse {
                id: None,
                username: Some(username.to_owned()),
                created_at: None,
            });
        }
        let parsed = serde_json::from_str::<RegisterResponse>(&text).unwrap_or(RegisterResponse {
            id: None,
            username: Some(username.to_owned()),
            created_at: None,
        });
        Ok(parsed)
    }

    pub async fn whoami(&self) -> Result<WhoamiResponse> {
        let resp = self
            .inner
            .get(self.url("/auth/whoami"))
            .send_paced(&self.requests)
            .await
            .context("GET /auth/whoami")?;
        let resp = Self::check_response(resp, "GET /auth/whoami").await?;
        Self::json_response(resp).await
    }

    pub async fn logout(&self) -> Result<()> {
        let resp = self
            .inner
            .post(self.url("/auth/logout"))
            .send_paced(&self.requests)
            .await
            .context("POST /auth/logout")?;
        Self::check_response(resp, "POST /auth/logout").await?;
        Ok(())
    }

    pub(crate) fn session_owner(&self) -> Uuid { *self.session_owner.lock().unwrap() }
    pub(crate) fn adopt_session_owner(&self, owner: Uuid) { *self.session_owner.lock().unwrap()=owner; }
    /// JWT claims are scheduling hints only; authorization remains server-side.
    pub(crate) fn session_expiry(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        use base64::Engine;
        let token=self.auth_cookie()?;
        let payload=base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(token.split('.').nth(1)?).ok()?;
        let claims:serde_json::Value=serde_json::from_slice(&payload).ok()?;
        chrono::DateTime::from_timestamp(claims.get("exp")?.as_i64()?,0)
    }

    pub async fn refresh(&self) -> Result<RefreshResponse> {
        Self::decode(self.inner.post(self.url("/auth/refresh")).send_paced(&self.requests).await?, "refresh").await
    }

    pub async fn get_server(&self) -> Result<Server> {
        Self::decode(self.inner.get(self.url("/server")).send_paced(&self.requests).await?, "server").await
    }

    pub async fn list_channels(&self) -> Result<Vec<Channel>> {
        #[derive(serde::Deserialize)]
        struct Wrapper { channels: Vec<Channel> }
        let wrapper: Wrapper = Self::decode(self.inner.get(self.url("/channels")).send_paced(&self.requests).await?, "channels").await?;
        Ok(wrapper.channels)
    }

    pub async fn list_roles(&self) -> Result<Vec<Role>> {
        #[derive(serde::Deserialize)] struct Response { roles: Vec<Role> }
        let response: Response = Self::decode(self.inner.get(self.url("/roles")).send_paced(&self.requests).await?, "roles").await?;
        Ok(response.roles)
    }

    /// Current backends include overrides in /channels. Only older responses
    /// that omit them need the per-channel endpoint; an empty list is complete.
    pub async fn channels_with_permissions(&self) -> Result<Vec<Channel>> {
        let mut channels = self.list_channels().await?;
        for channel in &mut channels {
            if channel.permissions.is_none() {
                channel.permissions = Some(self.channel_permissions(channel.id).await?);
            }
        }
        Ok(channels)
    }

    pub async fn channel_permissions(&self, id: Uuid) -> Result<Vec<ChannelPermissionEntry>> {
        #[derive(serde::Deserialize)] struct Response { channel_id: Uuid, permissions: Vec<ChannelPermissionEntry> }
        let response: Response = Self::decode(self.inner.get(self.url(&format!("/channels/{id}/permissions"))).send_paced(&self.requests).await?, "channel permissions").await?;
        anyhow::ensure!(response.channel_id == id, "Permissões retornadas para outro canal.");
        Ok(response.permissions)
    }

    /// Revalidate retained pages without discarding the reader's cached rows.
    /// Sequential requests share the normal request budget and cancellation.
    pub async fn reconcile_messages(&self,channel_id:Uuid,oldest:Option<MessageCursor>)->Result<MessageListResponse>{
        let mut cursor=None;let mut messages=Vec::new();
        loop{
            let page=self.list_messages(channel_id,cursor).await?;
            let next=page.messages.iter().min_by_key(|m|(m.created_at,m.id)).map(MessageCursor::from);
            let covered=oldest.is_none_or(|old|next.is_some_and(|next|(next.created_at,next.id)<=(old.created_at,old.id)));
            messages.extend(page.messages);
            if !page.has_more||covered{return Ok(MessageListResponse{channel_id,messages,has_more:page.has_more});}
            anyhow::ensure!(next.is_some()&&next!=cursor,"O servidor retornou uma página sem avanço no histórico.");
            cursor=next;
        }
    }
    pub async fn list_messages(&self, channel_id: Uuid, cursor: Option<MessageCursor>) -> Result<MessageListResponse> {
        let mut url = self.url(&format!("/channels/{channel_id}/messages"));
        url.query_pairs_mut().append_pair("order", "desc");
        if let Some(cursor) = cursor {
            url.query_pairs_mut().append_pair("since", &cursor.created_at.to_rfc3339())
                .append_pair("last_id", &cursor.id.to_string());
        }
        let response: MessageListResponse = Self::decode(self.inner.get(url).send_paced(&self.requests).await?, "messages").await?;
        anyhow::ensure!(response.channel_id == channel_id, "A API retornou mensagens de outro canal.");
        Ok(response)
    }

    pub async fn send_message(&self, request: &CreateMessageRequest) -> Result<Message> {
        features::validate_message(request.content.as_deref(),&[],&request.embeds)?;
        let mut form = reqwest::multipart::Form::new().text("channel_id", request.channel_id.to_string());
        if !request.embeds.is_empty(){form=form.text("embeds",serde_json::to_string(&request.embeds)?);}
        if let Some(content) = &request.content { form = form.text("content", content.clone()); }
        if let Some(reply) = request.reply_to { form = form.text("reply_to", reply.to_string()); }
        Self::decode(self.inner.post(self.url("/messages")).multipart(form).send_paced(&self.requests).await?, "send message").await
    }

    pub async fn delete_message(&self, message_id: Uuid) -> Result<()> {
        Self::check_response(self.inner.delete(self.url(&format!("/messages/{message_id}")))
            .send_paced(&self.requests).await?, "delete message").await?;
        Ok(())
    }

    pub async fn edit_message(&self, message_id: Uuid, content: &str) -> Result<Message> {
        self.edit_message_embeds(message_id,content,&[]).await
    }
    pub async fn edit_message_embeds(&self,message_id:Uuid,content:&str,embeds:&[EmbedInput])->Result<Message>{
        crate::models::validate_embeds(embeds)?;
        anyhow::ensure!(content.chars().count() <= 8192, "Use até 8192 caracteres.");
        let message: Message = Self::decode(self.inner.put(self.url(&format!("/messages/{message_id}")))
            .json(&UpdateMessageRequest { content: content.into(),embeds:embeds.to_vec() }).send_paced(&self.requests).await?, "edit message").await?;
        anyhow::ensure!(message.id == message_id, "Edição retornada para outra mensagem.");
        Ok(message)
    }

    pub async fn add_reaction(&self, channel_id: Uuid, message_id: Uuid,
        emoji_id: Option<Uuid>, unicode: Option<&str>) -> Result<()> {
        Self::check_response(self.inner.post(self.url(&format!(
            "/channels/{channel_id}/messages/{message_id}/reactions")))
            .json(&ReactionRequest { emoji_id, unicode: unicode.map(str::to_owned) })
            .send_paced(&self.requests).await?, "add reaction").await?;
        Ok(())
    }

    pub async fn pin_message(&self, channel_id: Uuid, message_id: Uuid) -> Result<()> {
        Self::check_response(self.inner.post(self.url(&format!(
            "/channels/{channel_id}/messages/{message_id}/pin"))).send_paced(&self.requests).await?, "pin message").await?;
        Ok(())
    }

    pub async fn unpin_message(&self, channel: Uuid, message: Uuid) -> Result<()> {
        Self::check_response(self.inner.delete(self.url(&format!("/channels/{channel}/messages/{message}/pin")))
            .send_paced(&self.requests).await?, "unpin message").await?;
        Ok(())
    }

    pub async fn pinned_messages(&self, channel: Uuid) -> Result<Vec<Message>> {
        let response: PinnedList = Self::decode(self.inner.get(self.url(&format!("/channels/{channel}/pinned")))
            .send_paced(&self.requests).await?, "pinned messages").await?;
        anyhow::ensure!(response.channel_id == channel && response.pinned.iter().all(|m| m.channel_id == channel), "Mensagens fixadas retornadas para outro canal.");
        Ok(response.pinned)
    }

    pub async fn remove_reaction(&self, channel: Uuid, message: Uuid, emoji_id: Option<Uuid>, unicode: Option<&str>) -> Result<()> {
        Self::check_response(self.inner.delete(self.url(&format!("/channels/{channel}/messages/{message}/reactions")))
            .json(&ReactionRequest { emoji_id, unicode: unicode.map(str::to_owned) }).send_paced(&self.requests).await?, "remove reaction").await?;
        Ok(())
    }

    pub async fn reaction_page(&self, channel: Uuid, message: Uuid, cursor: Option<MessageCursor>) -> Result<ReactionList> {
        let mut url = self.url(&format!("/channels/{channel}/messages/{message}/reactions"));
        url.query_pairs_mut().append_pair("order", "desc");
        if let Some(c) = cursor { url.query_pairs_mut().append_pair("since", &c.created_at.to_rfc3339()).append_pair("last_id", &c.id.to_string()); }
        let page: ReactionList = Self::decode(self.inner.get(url).send_paced(&self.requests).await?, "reaction participants").await?;
        anyhow::ensure!(page.message_id == message, "Reações retornadas para outra mensagem.");
        Ok(page)
    }

    pub async fn all_reactions(&self, channel: Uuid, message: Uuid) -> Result<Vec<ReactionGroup>> {
        let mut groups: Vec<ReactionGroup> = Vec::new();
        let mut cursor: Option<MessageCursor> = None;
        loop {
            let page = self.reaction_page(channel, message, cursor).await?;
            // Group order is not cursor order: choose the oldest individual row.
            let next = page.reactions.iter().flat_map(|g| &g.users).min_by_key(|u| (u.created_at, u.id))
                .map(|u| MessageCursor { created_at: u.created_at, id: u.id });
            for mut group in page.reactions {
                if let Some(existing) = groups.iter_mut().find(|g| g.emoji_id == group.emoji_id && g.unicode == group.unicode) {
                    for user in group.users.drain(..) { if !existing.users.iter().any(|u| u.id == user.id) { existing.users.push(user); } }
                    existing.count = existing.users.len() as i32;
                } else { group.count = group.users.len() as i32; groups.push(group); }
            }
            if !page.has_more { return Ok(groups); }
            anyhow::ensure!(next.is_some() && cursor.map_or(true, |c| next.is_some_and(|n| (n.created_at, n.id) < (c.created_at, c.id))), "Paginação de reações não avançou.");
            cursor = next;
        }
    }

    pub async fn emoji_page(&self, cursor: Option<MessageCursor>) -> Result<EmojiList> {
        let mut url = self.url("/emojis");
        url.query_pairs_mut().append_pair("order", "asc");
        if let Some(c) = cursor { url.query_pairs_mut().append_pair("since", &c.created_at.to_rfc3339()).append_pair("last_id", &c.id.to_string()); }
        Self::decode(self.inner.get(url).send_paced(&self.requests).await?, "emojis").await
    }

    pub async fn all_emojis(&self) -> Result<Vec<Emoji>> {
        let mut emojis: Vec<Emoji> = Vec::new();
        let mut cursor: Option<MessageCursor> = None;
        loop {
            let page = self.emoji_page(cursor).await?;
            let next = page.emojis.iter().max_by_key(|e| (e.created_at, e.id)).map(|e| MessageCursor { id: e.id, created_at: e.created_at });
            for emoji in page.emojis { if !emojis.iter().any(|e| e.id == emoji.id) { emojis.push(emoji); } }
            anyhow::ensure!(emojis.len() <= 500, "A API excedeu o limite de 500 emojis.");
            if !page.has_more { return Ok(emojis); }
            anyhow::ensure!(next.is_some() && cursor.map_or(true, |c| next.is_some_and(|n| (n.created_at, n.id) > (c.created_at, c.id))), "Paginação de emojis não avançou.");
            cursor = next;
        }
    }

    pub async fn create_emoji(&self, request: &CreateEmojiRequest) -> Result<Emoji> {
        Self::decode(self.inner.post(self.url("/emojis")).json(request).send_paced(&self.requests).await?, "create emoji").await
    }

    pub async fn delete_emoji(&self, id: Uuid) -> Result<()> {
        Self::check_response(self.inner.delete(self.url(&format!("/emojis/{id}"))).send_paced(&self.requests).await?, "delete emoji").await?;
        Ok(())
    }

    pub async fn get_embed(&self, id: Uuid) -> Result<Embed> {
        let bytes = self.media_bytes(&format!("/embeds/{id}"), 8 << 20).await?;
        let preview: Embed = serde_json::from_slice(&bytes).context("Prévia inválida")?;
        anyhow::ensure!(preview.id == id, "Prévia retornada para outro link.");
        Ok(preview)
    }

    pub async fn list_users(&self, since: Option<&str>, last_id: Option<Uuid>) -> Result<UserListResponse> {
        let mut url = self.url("/users");
        url.query_pairs_mut().append_pair("order", "asc");
        if let Some(since) = since { url.query_pairs_mut().append_pair("since", since); }
        if let Some(id) = last_id { url.query_pairs_mut().append_pair("last_id", &id.to_string()); }
        Self::decode(self.inner.get(url).send_paced(&self.requests).await?, "users").await
    }

    pub async fn list_all_users(&self) -> Result<Vec<UserSummary>> {
        let mut users = Vec::new();
        let mut cursor: Option<(String, Uuid)> = None;
        loop {
            let page = self.list_users(cursor.as_ref().map(|value| value.0.as_str()),
                cursor.as_ref().map(|value| value.1)).await?;
            let next = page.users.last().map(|user| (user.created_at.to_rfc3339(), user.id));
            let has_more = page.has_more;
            users.extend(page.users);
            if !has_more { return Ok(users); }
            anyhow::ensure!(next.is_some() && next != cursor, "Paginação de usuários não avançou.");
            cursor = next;
        }
    }

    pub async fn user_summaries(&self, ids: Vec<Uuid>) -> Result<Vec<UserSummary>> {
        Self::decode(self.inner.post(self.url("/users/user_summary_batch"))
            .json(&ProfileBatchRequest { ids }).send_paced(&self.requests).await?, "user summaries").await
    }

    pub async fn get_user_profile(&self, user_id: Uuid) -> Result<UserProfile> {
        Self::decode(self.inner.get(self.url(&format!("/users/{user_id}/profile")))
            .send_paced(&self.requests).await?, "user profile").await
    }

    /// Full profiles contain avatar blobs; compact user summaries do not.
    /// The backend accepts at most 50 IDs in each profile batch.
    pub async fn user_profiles(&self, ids: &[Uuid]) -> Result<Vec<UserProfile>> {
        let mut profiles = Vec::new();
        for batch in ids.chunks(50) {
            let response: ProfileBatchResponse = Self::decode(
                self.inner.post(self.url("/users/profile_batch"))
                    .json(&ProfileBatchRequest { ids: batch.to_vec() }).send_paced(&self.requests).await?,
                "user profiles",
            ).await?;
            profiles.extend(response.profiles);
        }
        Ok(profiles)
    }

    pub async fn get_media(&self, sha_hash: &str) -> Result<Vec<u8>> {
        let response = Self::check_response(self.inner.get(self.url(&format!("/media/{sha_hash}")))
            .send_paced(&self.requests).await?, "media").await?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Returns true if the HTTP status was 401 (unauthorised / session expired).
    pub fn is_unauthorised(status: StatusCode) -> bool {
        status == StatusCode::UNAUTHORIZED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_auth_cookies_require_https_except_on_loopback() {
        use reqwest::ResponseBuilderExt;
        use tokio_tungstenite::tungstenite::http::Response;

        for (url, allowed) in [
            ("http://remote.example.test/auth/login_server", false),
            ("https://remote.example.test/auth/login_server", true),
            ("http://localhost:8080/auth/login_server", true),
            ("http://127.0.0.1:8080/auth/login_server", true),
            ("http://[::1]:8080/auth/login_server", true),
        ] {
            let response: reqwest::Response = Response::builder()
                .url(Url::parse(url).unwrap())
                .header("Set-Cookie", "Auth=test; Secure; HttpOnly; Path=/")
                .body("").unwrap().into();
            let result = ApiClient::check_auth_cookie_transport(&response);
            assert_eq!(result.is_ok(), allowed, "{url}");
            if !allowed {
                assert!(result.unwrap_err().to_string().contains("Use o endereço HTTPS"));
            }
        }
    }

    #[test]
    fn local_addresses_and_api_prefixes_are_normalized_consistently() {
        for (input, expected) in [
            (" http://localhost:8080/ ", "http://localhost:8080/"),
            ("localhost:8080", "http://localhost:8080/"),
            ("127.0.0.1:8080", "http://127.0.0.1:8080/"),
            ("[::1]:8080", "http://[::1]:8080/"),
            ("chat.example.test", "https://chat.example.test/"),
        ] {
            let client = ApiClient::new(input).unwrap();
            assert_eq!(client.base_url(), expected);
        }
        let client = ApiClient::new("http://localhost:8080/papo/api").unwrap();
        assert_eq!(client.url("/health").as_str(), "http://localhost:8080/papo/api/health");
        assert_eq!(client.websocket_request().unwrap().uri().to_string(), "ws://localhost:8080/papo/api/ws");
    }

    #[test]
    fn invalid_server_addresses_fail_before_any_request() {
        for input in ["", "ftp://localhost:8080", "http://", "http://user:pass@localhost:8080", "http://localhost:8080?secret=value"] {
            assert!(ApiClient::new(input).is_err(), "unexpectedly accepted {input}");
        }
    }

    #[test]
    fn websocket_cookies_respect_scope_and_rotation() {
        let client = ApiClient::new("https://chat.example.test").unwrap();
        client.cookies.add_cookie_str("session=old; Secure; HttpOnly; Path=/", &client.base);
        client.cookies.add_cookie_str("private=value; Path=/auth", &client.base);
        let request = client.websocket_request().unwrap();
        assert_eq!(request.uri().to_string(), "wss://chat.example.test/ws");
        assert_eq!(request.headers()["Cookie"], "session=old");
        client.cookies.add_cookie_str("session=new; Secure; HttpOnly; Path=/", &client.base);
        let clone = client.clone();
        assert_eq!(clone.websocket_request().unwrap().headers()["Cookie"], "session=new");

        let other = ApiClient::new("https://other.example.test").unwrap();
        assert!(other.websocket_request().unwrap().headers().get("Cookie").is_none());
        let insecure = ApiClient::new("http://chat.example.test").unwrap();
        insecure.cookies.add_cookie_str("session=secret; Secure; Path=/", &client.base);
        assert!(insecure.websocket_request().unwrap().headers().get("Cookie").is_none());
    }

    #[tokio::test]
    async fn login_cookie_authenticates_websocket_and_reconnect() {
        use futures_util::SinkExt;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        use tokio::time::{timeout, Duration};
        use tokio_tungstenite::{accept_hdr_async, tungstenite::Message};
        use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

        timeout(Duration::from_secs(10), async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let client = ApiClient::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
            let id = Uuid::new_v4();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 1024];
                loop {
                    let n = stream.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        assert!(headers.starts_with("POST /auth/login HTTP/1.1"));
                        let length: usize = headers.lines().find_map(|line|
                            line.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned))
                            .unwrap().parse().unwrap();
                        if bytes.len() >= end + 4 + length { break; }
                    }
                }
                let body = format!(r#"{{"user":{{"id":"{id}","username":"alice"}}}}"#);
                stream.write_all(format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nSet-Cookie: session=test-session; HttpOnly; Path=/\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(), body).as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
                drop(stream);

                for _ in 0..2 {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = accept_hdr_async(stream, |request: &Request, response: Response| {
                        assert_eq!(request.uri().path(), "/ws");
                        assert_eq!(request.headers()["Cookie"], "session=test-session");
                        Ok(response)
                    }).await.unwrap();
                    socket.send(Message::Text(format!(
                        r#"{{"type":"message_edit","id":"{id}","content":"updated"}}"#))).await.unwrap();
                    socket.close(None).await.unwrap();
                }
            });
            assert_eq!(client.login("alice", "test-password").await.unwrap().user.id, id);
            let (mut events, commands) = crate::ws::spawn(client);
            for _ in 0..2 {
                assert!(matches!(events.recv().await, Some(crate::ws::WsEvent::ConnectionReady(_))));
                assert!(matches!(events.recv().await, Some(crate::ws::WsEvent::Reconnected)));
                assert!(matches!(events.recv().await,
                    Some(crate::ws::WsEvent::MessageEdit { id: received, content, .. })
                    if received == id && content == "updated"));
                assert!(matches!(events.recv().await, Some(crate::ws::WsEvent::Disconnected)));
            }
            drop(events);
            drop(commands);
            server.await.unwrap();
        }).await.expect("mock login/WebSocket test timed out");
    }

    #[test]
    fn test_api_client_url_construction() {
        let client = ApiClient::new("http://localhost:8080").expect("valid url");
        assert_eq!(client.url("/auth/login").as_str(), "http://localhost:8080/auth/login");
        assert_eq!(client.url("/channels").as_str(), "http://localhost:8080/channels");
    }

    #[test]
    fn test_is_unauthorised() {
        assert!(ApiClient::is_unauthorised(StatusCode::UNAUTHORIZED));
        assert!(!ApiClient::is_unauthorised(StatusCode::OK));
        assert!(!ApiClient::is_unauthorised(StatusCode::FORBIDDEN));
    }
}

#[cfg(test)]
mod contract_tests;
