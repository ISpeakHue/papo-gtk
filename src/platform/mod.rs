//! EN-US: OS integration boundaries; protocol, history and GTK components remain shared.
//! PT-BR: Fronteiras de integração com o sistema; protocolo, histórico e componentes GTK continuam compartilhados.
#[cfg(not(test))]pub mod credentials;
#[cfg(any(windows,test))]mod credential_key;
pub mod runtime;
pub mod files;
#[cfg(all(windows,not(test)))] pub mod storage_check;
pub mod sandbox;
pub mod capture;
pub mod shortcuts;
#[cfg(windows)] pub mod startup;
#[cfg(any(windows,test))] pub mod activation;

#[cfg(not(any(target_os="linux",target_os="windows",target_os="macos")))]
compile_error!("Papo currently targets Linux, Windows and macOS only.");

#[derive(Debug,Clone,Copy,PartialEq,Eq,serde::Serialize)]
#[serde(rename_all="snake_case")]
pub enum Os {Linux,Windows,Macos}

#[derive(Debug,Clone,Copy,serde::Serialize)]
pub struct Capabilities {
    pub os:Os,
    pub credential_store:&'static str,
    pub desktop_notifications:bool,
    pub screen_capture:bool,
    pub isolated_media_helpers:bool,
}
impl Os {
    // EN-US: Missing native integrations are explicit capabilities, never Linux fallbacks or weaker isolation.
    // PT-BR: Integrações nativas pendentes são capacidades explícitas, sem usar alternativas Linux nem reduzir o isolamento.
    pub const fn capabilities(self)->Capabilities{
        match self{
            Self::Linux=>Capabilities{os:self,credential_store:"secret_service",desktop_notifications:true,screen_capture:true,isolated_media_helpers:true},
            Self::Windows=>Capabilities{os:self,credential_store:"windows_credential_store",desktop_notifications:false,screen_capture:false,isolated_media_helpers:true},
            Self::Macos=>Capabilities{os:self,credential_store:"apple_keychain",desktop_notifications:false,screen_capture:false,isolated_media_helpers:false},
        }
    }
}

pub const CURRENT:Capabilities=if cfg!(target_os="windows"){Os::Windows.capabilities()}
    else if cfg!(target_os="macos"){Os::Macos.capabilities()}else{Os::Linux.capabilities()};

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn native_credentials_and_unfinished_features_are_explicit(){
        assert_eq!(Os::Windows.capabilities().credential_store,"windows_credential_store");
        assert_eq!(Os::Macos.capabilities().credential_store,"apple_keychain");
        for os in [Os::Windows,Os::Macos]{let c=os.capabilities();assert!(!c.screen_capture&&!c.desktop_notifications);}
        assert!(Os::Windows.capabilities().isolated_media_helpers);assert!(!Os::Macos.capabilities().isolated_media_helpers);
        let linux=Os::Linux.capabilities();assert!(linux.isolated_media_helpers&&linux.screen_capture&&linux.desktop_notifications);
    }
}
