//! Shared authentication flow used by the login UI and protocol regression tests.

use anyhow::{Context, Result};

use crate::{api::ApiClient, models::WhoamiResponse};
use super::AuthMode;

pub async fn authenticate(
    client: &ApiClient,
    mode: &AuthMode,
    username: &str,
    password: &str,
    server_password: &str,
) -> Result<WhoamiResponse> {
    if let Err(error) = client.health().await {
        if let Some(http) = error.downcast_ref::<reqwest::Error>() {
            if http.is_connect() {
                anyhow::bail!("Não foi possível conectar a {}. Confirme que o backend está rodando nesse endereço e porta. Detalhes: {}",
                    client.base_url(), error.root_cause());
            }
            if http.is_timeout() {
                anyhow::bail!("O servidor em {} demorou para responder. Tente novamente.", client.base_url());
            }
        }
        return Err(error.context("O servidor respondeu com um erro"));
    }

    if !server_password.is_empty() {
        // The user's explicit password selects server authentication. Some
        // deployments only expose server metadata after authorization.
        client.login_server(server_password).await.context("Falha ao validar a senha do servidor")?;
    } else if client.server_is_private().await.context("Erro ao consultar o servidor")? {
        anyhow::bail!("Este servidor exige uma senha. Preencha o campo Senha do servidor para continuar.");
    }
    if *mode == AuthMode::Register {
        client.register(username, password).await.context("Falha ao criar conta")?;
    }
    let login = client.login(username, password).await.context(
        if *mode == AuthMode::Register { "Conta criada, mas falha ao entrar" } else { "Falha no login" }
    )?;
    let mut user = client.whoami().await.context("Erro ao obter perfil")?;
    user.connection_violation = Some(user.connection_violation.unwrap_or(false) || login.connection_violation.unwrap_or(false));
    Ok(user)
}
