//! EN-US: Stable account identity is independent of credentials and login generations.
//! PT-BR: A identidade estável da conta independe de credenciais e gerações de login.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from="AccountWire")]
pub struct AccountId {
    server: String,
    user: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountWire { server:String, user:Uuid }

impl TryFrom<AccountWire> for AccountId {
    type Error=anyhow::Error;
    fn try_from(wire:AccountWire)->Result<Self,Self::Error>{
        let server=crate::api::ApiClient::parse_base_url(&wire.server)?;
        Ok(Self{server:server.to_string(),user:wire.user})
    }
}

impl AccountId {
    pub fn new(api: &crate::api::ApiClient, user: Uuid) -> Self {
        Self { server: api.base_url().to_owned(), user }
    }

    pub fn server(&self) -> &str { &self.server }
    pub fn user(&self) -> Uuid { self.user }

    // EN-US: Versioned opaque storage keys keep URL paths and user IDs out of filenames/logs.
    // PT-BR: Chaves opacas versionadas evitam caminhos de URL e IDs de usuário em arquivos/logs.
    pub fn namespace(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"papo-account-v1\0");
        hash.update(self.server.as_bytes());
        hash.update(b"\0");
        hash.update(self.user.as_bytes());
        format!("v1-{:x}", hash.finalize())
    }
}

impl std::fmt::Debug for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AccountId").field(&self.namespace()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_server_and_user_define_identity_not_login_or_display_name() {
        let user = Uuid::new_v4();
        let a = crate::api::ApiClient::new("https://EXAMPLE.test:443/api").unwrap();
        let b = crate::api::ApiClient::new("https://example.test/api/").unwrap();
        let identity = AccountId::new(&a, user);
        assert_eq!(identity, AccountId::new(&b, user));
        assert_ne!(a.session_owner(), b.session_owner());
        for other in ["https://example.test/other/", "https://other.test/api/", "https://example.test:444/api/"] {
            assert_ne!(identity, AccountId::new(&crate::api::ApiClient::new(other).unwrap(), user));
        }
        assert_ne!(identity, AccountId::new(&a, Uuid::new_v4()));
        assert_eq!(serde_json::from_str::<AccountId>(&serde_json::to_string(&identity).unwrap()).unwrap(), identity);
        assert!(!format!("{identity:?}").contains("example.test"));
        assert!(identity.namespace().bytes().all(|c| c.is_ascii_hexdigit() || b"v-".contains(&c)));
        let decoded:AccountId=serde_json::from_value(serde_json::json!({"server":"https://EXAMPLE.test:443/api","user":user})).unwrap();
        assert_eq!(decoded,identity);
        for server in ["https://user:secret@example.test/","https://example.test/?token=secret","http://remote.test/"]{
            assert!(serde_json::from_value::<AccountId>(serde_json::json!({"server":server,"user":user})).is_err());
        }
    }
}
