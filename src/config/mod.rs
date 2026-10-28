//! Persistent application configuration stored in the native OS configuration directory.
//! EN-US: Manages user preferences, server URL, credentials retention, theme settings, and motion mode.
//! PT-BR: Configuração persistente no diretório nativo do SO. Gerencia preferências, servidor, retenção de sessão, tema e movimento.

use anyhow::{Context, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::path::{Path,PathBuf};

/// User-editable app config that survives restarts.
/// EN-US: Serializable configuration struct representing stored user options.
/// PT-BR: Estrutura de configuração serializável representando opções salvas do usuário que persistem entre inicializações.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// EN-US: Last server URL the user connected to.
    /// PT-BR: Última URL de servidor à qual o usuário se conectou.
    pub server_url: Option<String>,
    /// EN-US: Last logged-in username (used to pre-fill the login form).
    /// PT-BR: Último nome de usuário autenticado (usado para pré-preencher o formulário de login).
    pub last_username: Option<String>,
    /// EN-US: GTK color scheme override ("dark" | "light" | "system").
    /// PT-BR: Substituição do esquema de cores GTK ("dark" | "light" | "system").
    pub theme: Option<String>,
    /// EN-US: Flag to resume a valid session from the desktop keyring on next launch.
    /// PT-BR: Sinalizador para retomar uma sessão válida do chaveiro do sistema na próxima inicialização.
    pub remember_session: Option<bool>,
    /// EN-US: Animation/motion playback policy.
    /// PT-BR: Política de reprodução de animações e movimento.
    #[serde(default)]
    pub motion:Motion,
}

/// EN-US: Animation and motion policy: System setting, Always play, or Reduced motion.
/// PT-BR: Política de movimento e animações: Configuração do sistema, Sempre reproduzir ou Movimento reduzido.
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Motion{#[default] System,Always,Reduced}
impl Motion{
    // EN-US: Convert UI dropdown index to Motion enum.
    // PT-BR: Converte o índice do menu suspenso da interface para a enumeração Motion.
    pub fn from_index(index:u32)->Self{match index{1=>Self::Always,2=>Self::Reduced,_=>Self::System}}
    // EN-US: Convert Motion enum to UI dropdown index.
    // PT-BR: Converte a enumeração Motion para o índice do menu suspenso da interface.
    pub fn index(self)->u32{match self{Self::System=>0,Self::Always=>1,Self::Reduced=>2}}
}
impl AppConfig {
    /// EN-US: Returns the native configuration path (XDG, AppData or Application Support).
    /// PT-BR: Retorna o caminho nativo de configuração (XDG, AppData ou Application Support).
    pub fn path() -> Result<PathBuf> {
        // EN-US: GUI tests must isolate settings on every OS, regardless of XDG support.
        // PT-BR: Testes da interface isolam as configurações em todos os sistemas, independentemente do suporte XDG.
        #[cfg(test)]if let Some(root)=std::env::var_os("PAPO_TEST_CONFIG_DIR"){return Ok(PathBuf::from(root).join("config.toml"));}
        Ok(config_dir().context("Diretório de configuração indisponível.")?
            .join("papo-gtk")
            .join("config.toml"))
    }

    /// EN-US: Load config from disk; returns `Default::default()` if the file does not exist or fails parsing.
    /// PT-BR: Carrega a configuração do disco; retorna `Default::default()` caso o arquivo não exista ou falhe na análise.
    pub fn load() -> Self {
        Self::path().ok().map(|p|Self::load_at(&p)).unwrap_or_default()
    }

    pub(crate) fn load_at(path:&Path)->Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// EN-US: Persist config to disk, automatically creating parent directories if needed.
    /// PT-BR: Salva a configuração no disco, criando diretórios pai automaticamente se necessário.
    pub fn save(&self) -> Result<()> {
        self.save_at(&Self::path()?)
    }

    /// EN-US: A supplied path keeps acceptance tests out of the user's real configuration.
    /// PT-BR: Um caminho explícito isola os testes de aceitação da configuração real do usuário.
    pub(crate) fn save_at(&self,path:&Path)->Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("creating config dir")?;
        }
        let toml_str = toml::to_string_pretty(self).context("serialising config")?;
        crate::platform::files::atomic_write(path,toml_str.as_bytes()).context("writing config")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_serialize_deserialize() {
        let config = AppConfig {
            server_url: Some("http://localhost:8080".into()),
            last_username: Some("testuser".into()),
            theme: Some("dark".into()),
            remember_session:Some(true),motion:Motion::System,
        };

        let serialized = toml::to_string_pretty(&config).expect("serialization failed");
        let deserialized: AppConfig = toml::from_str(&serialized).expect("deserialization failed");

        assert_eq!(deserialized.server_url.as_deref(), Some("http://localhost:8080"));
        assert_eq!(deserialized.last_username.as_deref(), Some("testuser"));
        assert_eq!(deserialized.theme.as_deref(), Some("dark"));
    }

    #[test]
    fn test_app_config_defaults() {
        let default_config = AppConfig::default();
        assert!(default_config.server_url.is_none());
        assert!(default_config.last_username.is_none());
        assert!(default_config.theme.is_none());
    }
}
