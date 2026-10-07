//! Login and Register screen component.

mod authenticate;
pub(crate) mod recovery;
#[cfg(test)]
pub(crate) mod tests;

use adw::prelude::*;
use gtk::prelude::*;
use relm4::prelude::*;
use tracing::{error, info};

use crate::api::ApiClient;
use crate::config::AppConfig;
use crate::models::WhoamiResponse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    Login,
    Register,
}

pub struct LoginModel {
    login_root: gtk::ScrolledWindow,
    recovery: Option<recovery::Recovery>,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub server_password: String,
    clear_credential_fields: bool,
    pub mode: AuthMode,
    pub is_loading: bool,
    pub error_message: Option<String>,
    remember_session:bool,
}

pub enum LoginMsg {
    Resume, SetRememberSession(bool),
    Resumed{server:String,user:String,result:Result<Option<(WhoamiResponse,ApiClient)>,String>},
    ClearCredentials,
    OpenRecovery, Recover(uuid::Uuid), Recovered { token: uuid::Uuid, result: anyhow::Result<()> },
    SetServerUrl(String),
    SetUsername(String),
    SetPassword(String),
    SetServerPassword(String),
    ToggleMode,
    Submit,
    AuthSuccess(WhoamiResponse, ApiClient),
    AuthFailed(String),
}

#[derive(Debug)]
pub enum LoginOutput {
    Authenticated {
        whoami: WhoamiResponse,
        client: ApiClient,
    },
}

#[relm4::component(pub)]
impl SimpleComponent for LoginModel {
    type Init = AppConfig;
    type Input = LoginMsg;
    type Output = LoginOutput;

    view! {
        gtk::ScrolledWindow {
            add_css_class: "papo-login",
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vscrollbar_policy: gtk::PolicyType::Automatic,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Center,
                set_spacing: 18,
                set_margin_top: 36,
                set_margin_bottom: 36,
                set_margin_start: 16,
                set_margin_end: 16,
                set_width_request: 380,

                // App Header / Branding
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,
                    set_halign: gtk::Align::Center,

                    gtk::Image {
                        set_icon_name: Some("chat-message-new-symbolic"),
                        set_pixel_size: 40,
                        add_css_class: "papo-login-brand",
                        add_css_class: "accent",
                    },

                    gtk::Label {
                        set_text: "Papo",
                        add_css_class: "title-1",
                    },

                    gtk::Label {
                        #[watch]
                        set_text: match model.mode {
                            AuthMode::Login => "Faça login para conversar",
                            AuthMode::Register => "Crie sua nova conta",
                        },
                        add_css_class: "dim-label",
                    },
                },

                // Error message banner
                gtk::Label {
                    #[watch]
                    set_visible: model.error_message.is_some(),
                    #[watch]
                    set_text: model.error_message.as_deref().unwrap_or(""),
                    add_css_class: "error",
                    set_wrap: true,
                    set_justify: gtk::Justification::Center,
                },

                // Server Preferences Group
                adw::PreferencesGroup {
                    set_title: "Servidor",
                    #[watch]
                    set_sensitive: !model.is_loading,

                    add: server_entry = &adw::EntryRow {
                        set_title: "Endereço do Servidor",
                        set_text: &model.server_url,
                        connect_changed[sender] => move |entry| {
                            sender.input(LoginMsg::SetServerUrl(entry.text().to_string()));
                        },
                    },

                    add: server_password_entry = &adw::PasswordEntryRow {
                        set_title: "Senha do servidor (se necessário)",
                        // Only explicit resets write into editable fields. Queued
                        // typing events must not rewrite newer text or move the caret.
                        #[track(model.clear_credential_fields)]
                        #[block_signal(server_password_changed)]
                        set_text: "",
                        set_tooltip_text: Some("Deixe em branco para servidores públicos."),
                        connect_changed[sender] => move |entry| {
                            sender.input(LoginMsg::SetServerPassword(entry.text().to_string()));
                        } @server_password_changed,
                        connect_entry_activated[sender] => move |_| {
                            sender.input(LoginMsg::Submit);
                        },
                    },
                },

                // Credentials Preferences Group
                adw::PreferencesGroup {
                    #[watch]
                    set_sensitive: !model.is_loading,
                    #[watch]
                    set_title: match model.mode {
                        AuthMode::Login => "Credenciais",
                        AuthMode::Register => "Nova Conta",
                    },

                    add: user_entry = &adw::EntryRow {
                        set_title: "Usuário",
                        set_text: &model.username,
                        connect_changed[sender] => move |entry| {
                            sender.input(LoginMsg::SetUsername(entry.text().to_string()));
                        },
                    },

                    add: pass_entry = &adw::PasswordEntryRow {
                        set_title: "Senha",
                        #[track(model.clear_credential_fields)]
                        #[block_signal(password_changed)]
                        set_text: "",
                        connect_changed[sender] => move |entry| {
                            sender.input(LoginMsg::SetPassword(entry.text().to_string()));
                        } @password_changed,
                        connect_entry_activated[sender] => move |_| {
                            sender.input(LoginMsg::Submit);
                        },
                    },
                },

                gtk::Button {
                    set_label: "Tenho um link de recuperação",
                    connect_clicked => LoginMsg::OpenRecovery,
                },
                // Submit Button / Loading
                gtk::CheckButton {
                    set_label:Some("Manter sessão neste dispositivo"),set_tooltip_text:Some("A sessão é guardada no chaveiro do desktop. Sem um chaveiro disponível, entre novamente ao reabrir."),
                    #[watch] set_active:model.remember_session,
                    #[watch] set_sensitive:!model.is_loading,
                    connect_toggled[sender] => move |button|sender.input(LoginMsg::SetRememberSession(button.is_active())),
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 10,
                    set_margin_top: 8,

                    gtk::Button {
                        #[watch]
                        set_sensitive: !model.is_loading && !model.username.is_empty() && !model.password.is_empty() && !model.server_url.is_empty(),
                        add_css_class: "suggested-action",
                        add_css_class: "pill",

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_halign: gtk::Align::Center,

                            gtk::Spinner {
                                #[watch]
                                set_spinning: model.is_loading,
                                #[watch]
                                set_visible: model.is_loading,
                            },

                            gtk::Label {
                                #[watch]
                                set_text: if model.is_loading {
                                    "Conectando..."
                                } else {
                                    match model.mode {
                                        AuthMode::Login => "Entrar",
                                        AuthMode::Register => "Criar Conta",
                                    }
                                },
                            },
                        },

                        connect_clicked[sender] => move |_| {
                            sender.input(LoginMsg::Submit);
                        },
                    },

                    // Switch Login / Register mode
                    gtk::Button {
                        set_has_frame: false,
                        #[watch]
                        set_sensitive: !model.is_loading,
                        #[watch]
                        set_label: match model.mode {
                            AuthMode::Login => "Não tem uma conta? Cadastre-se",
                            AuthMode::Register => "Já possui conta? Faça login",
                        },
                        connect_clicked[sender] => move |_| {
                            sender.input(LoginMsg::ToggleMode);
                        },
                    },
                },
            },
        }
    }

    fn init(config: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let server_url = config.server_url.unwrap_or_else(|| "http://localhost:8080".to_string());
        let username = config.last_username.unwrap_or_default();

        let model = LoginModel {
            login_root: root.clone(), recovery: None,
            server_url,
            username,
            password: String::new(),
            server_password: String::new(),
            clear_credential_fields: false,
            mode: AuthMode::Login,
            is_loading: false,
            error_message: None,
            remember_session:config.remember_session.unwrap_or(true),
        };

        let widgets = view_output!();
        #[cfg(not(test))]
        if model.remember_session&&!model.server_url.is_empty()&&!model.username.is_empty(){sender.input(LoginMsg::Resume);}
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.clear_credential_fields = false;
        match message {
            LoginMsg::SetRememberSession(value)=>self.remember_session=value,
            LoginMsg::Resume=>{
                if self.is_loading||!self.remember_session||self.mode!=AuthMode::Login||self.server_url.trim().is_empty()||self.username.trim().is_empty(){return;}
                self.is_loading=true;let server=self.server_url.clone();let user=self.username.trim().to_owned();let s=sender.clone();tokio::spawn(async move{
                    let result=match ApiClient::new(&server){Ok(client)=>crate::session::resume(&client,&user).await.map(|profile|profile.map(|profile|(profile,client))).map_err(|e|e.to_string()),Err(_)=>Ok(None)};
                    s.input(LoginMsg::Resumed{server,user,result});
                });
            },
            LoginMsg::Resumed{server,user,result}=>{
                self.is_loading=false;
                if self.server_url!=server||self.username.trim()!=user||self.mode!=AuthMode::Login||!self.password.is_empty(){return;}
                match result{Ok(Some((profile,client)))=>sender.input(LoginMsg::AuthSuccess(profile,client)),Ok(None)=>{},Err(error)=>self.error_message=Some(format!("{error} Entre com a senha para continuar."))}
            },
            LoginMsg::ClearCredentials => {
                self.password.clear();
                self.server_password.clear();
                self.clear_credential_fields = true;
                self.recovery = None;
            }
            LoginMsg::OpenRecovery => self.open_recovery(&sender),
            LoginMsg::Recover(token) => self.recover(token,&sender),
            LoginMsg::Recovered { token, result } => self.recovered(token,result),
            LoginMsg::SetServerUrl(url) => {
                self.server_url = url;
            }
            LoginMsg::SetUsername(u) => {
                self.username = u;
            }
            LoginMsg::SetPassword(p) => {
                self.password = p;
            }
            LoginMsg::SetServerPassword(password) => {
                self.server_password = password;
            }
            LoginMsg::ToggleMode => {
                self.mode = match self.mode {
                    AuthMode::Login => AuthMode::Register,
                    AuthMode::Register => AuthMode::Login,
                };
                self.error_message = None;
            }
            LoginMsg::Submit => {
                if self.is_loading || self.server_url.trim().is_empty()
                    || self.username.trim().is_empty() || self.password.is_empty() {
                    return;
                }
                self.is_loading = true;
                self.error_message = None;

                let server_url = self.server_url.trim().trim_end_matches('/').to_string();
                let username = self.username.trim().to_string();
                let password = self.password.clone();
                let server_password = self.server_password.clone();
                let mode = self.mode.clone();
                let remember=self.remember_session;

                let sender_clone = sender.clone();

                // Spawn async task to authenticate
                tokio::spawn(async move {
                    let res_msg = match ApiClient::new(&server_url) {
                        Ok(client) => {
                            match authenticate::authenticate(&client, &mode, &username, &password, &server_password).await {
                                Ok(whoami) => {
                                    let stored=if remember{crate::session::save(&client,&whoami.username).await}else{crate::session::forget(&client,&whoami.username).await};
                                    if stored.is_err(){tracing::warn!("Desktop keyring unavailable; session remains in memory only");}
                                    info!("Authenticated as {}", whoami.username);
                                    LoginMsg::AuthSuccess(whoami, client)
                                }
                                Err(error) => LoginMsg::AuthFailed(format!("{error:#}")),
                            }
                        }
                        Err(e) => {
                            error!("Invalid server URL: {e}");
                            LoginMsg::AuthFailed(format!("URL do servidor inválida: {e}"))
                        }
                    };

                    sender_clone.input(res_msg);
                });
            }
            LoginMsg::AuthSuccess(whoami, client) => {
                self.password.clear();self.server_password.clear();self.recovery=None;
                self.clear_credential_fields = true;
                self.is_loading = false;
                self.error_message = None;

                // Save last server URL and username in config
                let mut cfg = AppConfig::load();
                cfg.server_url = Some(client.base_url().to_owned());
                cfg.last_username = Some(whoami.username.clone());
                cfg.remember_session=Some(self.remember_session);
                if let Err(e) = cfg.save() {
                    tracing::warn!("Failed to save config: {e}");
                }

                let _ = sender.output(LoginOutput::Authenticated {
                    whoami,
                    client,
                });
            }
            LoginMsg::AuthFailed(err) => {
                self.is_loading = false;
                self.error_message = Some(err);
            }
        }
    }
}

impl std::fmt::Debug for LoginModel { fn fmt(&self,f:&mut std::fmt::Formatter)->std::fmt::Result {f.write_str("LoginModel (credentials redacted)")} }
impl std::fmt::Debug for LoginMsg { fn fmt(&self,f:&mut std::fmt::Formatter)->std::fmt::Result {f.write_str("LoginMsg (credentials redacted)")} }
