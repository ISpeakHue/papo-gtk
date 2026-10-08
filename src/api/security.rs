//! Secrets only appear in JSON request bodies. Errors never echo backend-supplied secrets.
use super::*;

pub fn recovery_token(input: &str) -> Result<String> {
    let input = input.trim();
    let token = if input.contains("://") {
        let url = Url::parse(input).map_err(|_| anyhow::anyhow!("Link de recuperação inválido."))?;
        anyhow::ensure!(matches!(url.scheme(), "http" | "https") && url.username().is_empty() && url.password().is_none(), "Link de recuperação inválido.");
        if let Some(token) = url.query_pairs().find_map(|(key, value)| (key == "token").then(|| value.into_owned())) { token }
        else { url.path().strip_prefix("/passwordchange/").filter(|s| !s.contains('/')).unwrap_or("").to_owned() }
    } else { input.to_owned() };
    anyhow::ensure!(!token.is_empty() && token.len() <= 256 && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'), "Cole um token ou link de recuperação válido.");
    Ok(token)
}
impl ApiClient {
    async fn security_response(response: reqwest::Response) -> Result<()> {
        let status = response.status();
        if status.is_success() { Self::check_non_json_response(response).await?; return Ok(()); }
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        let code = body["type"].as_str().and_then(|s| s.rsplit('/').next()).unwrap_or("");
        let (code, message) = match code {
            "reset-link-invalid" => (Some("reset-link-invalid"), "Link inválido, expirado ou já utilizado."),
            "no-reset-password" => (Some("no-reset-password"), "Não foi possível preparar a troca de senha. Tente novamente."),
            "invalid-param" => (Some("invalid-param"), "A senha não atende à política do servidor. Verifique tamanho, maiúscula e caractere especial."),
            "unauthorized" | "connection-reused" => (Some("unauthorized"), "Sua sessão terminou. Entre novamente."),
            "banned" => (Some("banned"), "Conta sem acesso ao servidor."),
            _ => (None, "Não foi possível atualizar a senha. Verifique a política de senha do servidor e tente novamente."),
        };
        Err(ApiError { status, code: code.map(str::to_owned), message: message.into() }.into())
    }
    pub async fn change_own_password(&self, user: Uuid, password: &str) -> Result<()> {
        anyhow::ensure!(!password.is_empty(), "Informe a nova senha.");
        // Self-reset returns {response:string}; it does not issue an admin recovery link.
        Self::security_response(self.inner.post(self.url(&format!("/users/{user}/reset"))).send_paced(&self.requests).await?).await?;
        Self::security_response(self.inner.put(self.url(&format!("/users/{user}/password")))
            .json(&serde_json::json!({"password":password})).send_paced(&self.requests).await?).await
    }
    pub async fn recover_password(&self, token: &str, password: &str) -> Result<()> {
        let token = recovery_token(token)?;
        anyhow::ensure!(!password.is_empty(), "Informe a nova senha.");
        Self::security_response(self.inner.post(self.url("/auth/password_reset"))
            .json(&serde_json::json!({"token":token,"password":password})).send_paced(&self.requests).await?).await
    }
    pub async fn connected_devices(&self) -> Result<Vec<Connection>> {
        let response: ConnectedDevicesResponse = Self::decode(self.inner.get(self.url("/auth/connected_devices")).send_paced(&self.requests).await?, "sessões conectadas").await?;
        Ok(response.connections)
    }
    pub async fn drop_connection(&self, id: &str) -> Result<DropConnectionResponse> {
        anyhow::ensure!(id == "ALL" || Uuid::parse_str(id).is_ok(), "Sessão inválida.");
        Self::decode(self.inner.post(self.url("/auth/drop_connection")).json(&DropConnectionRequest { connection_id: id.into() }).send_paced(&self.requests).await?, "revogar sessão").await
    }
}
