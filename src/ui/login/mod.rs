//! EN-US: Login and Register screen component.
//! PT-BR: Componente das telas de login e cadastro.

mod authenticate;
pub(crate) mod recovery;
#[cfg(test)]
pub(crate) mod tests;

use adw::prelude::*;
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
    request:uuid::Uuid,scope:uuid::Uuid,
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
    Prepare{scope:uuid::Uuid,server:Option<String>,user:Option<String>,remember:bool},
    Finished{request:uuid::Uuid,result:Result<(WhoamiResponse,ApiClient),String>},
    #[cfg_attr(test, expect(dead_code, reason = "Automatic session resume is disabled in UI fixtures to avoid saved sessions."))]
    Resume, SetRememberSession(bool),
    Resumed{request:uuid::Uuid,server:String,user:String,result:Result<Option<(WhoamiResponse,ApiClient)>,String>},
    #[cfg(test)] ClearCredentials,
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
        scope:uuid::Uuid,
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
                        set_icon_name: Some(crate::app::APP_ID),
                        set_widget_name: "papo-app-icon",
                        set_pixel_size: 96,
                        add_css_class: "papo-login-brand",
                    },

                    gtk::Label {
                        set_text: "Papo",
                        add_css_class: "title-1",
                    },

                    gtk::Label {
                        #[watch]
                        set_text: match model.mode {
                            AuthMode::Login => crate::i18n::tr("Faça login para conversar"),
                            AuthMode::Register => crate::i18n::tr("Crie sua nova conta"),
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
                    set_title: crate::i18n::tr("Servidor"),
                    #[watch]
                    set_sensitive: !model.is_loading,

                    add: server_entry = &adw::EntryRow {
                        set_title: crate::i18n::tr("Endereço do Servidor"),
                        set_text: &model.server_url,
                        connect_changed[sender] => move |entry| {
                            sender.input(LoginMsg::SetServerUrl(entry.text().to_string()));
                        },
                    },

                    add: server_password_entry = &adw::PasswordEntryRow {
                        set_title: crate::i18n::tr("Senha do servidor (se necessário)"),
                        // Only explicit resets write into editable fields. Queued
                        // typing events must not rewrite newer text or move the caret.
                        #[track(model.clear_credential_fields)]
                        #[block_signal(server_password_changed)]
                        set_text: "",
                        set_tooltip_text: Some(crate::i18n::tr("Deixe em branco para servidores públicos.")),
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
                        AuthMode::Login => crate::i18n::tr("Credenciais"),
                        AuthMode::Register => crate::i18n::tr("Nova Conta"),
                    },

                    add: user_entry = &adw::EntryRow {
                        set_title: crate::i18n::tr("Usuário"),
                        set_text: &model.username,
                        connect_changed[sender] => move |entry| {
                            sender.input(LoginMsg::SetUsername(entry.text().to_string()));
                        },
                    },

                    add: pass_entry = &adw::PasswordEntryRow {
                        set_title: crate::i18n::tr("Senha"),
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
                    set_label: crate::i18n::tr("Tenho um link de recuperação"),
                    connect_clicked => LoginMsg::OpenRecovery,
                },
                // Submit Button / Loading
                gtk::CheckButton {
                    set_label:Some(crate::i18n::tr("Manter sessão neste dispositivo")),set_tooltip_text:Some(crate::i18n::tr("A sessão é guardada no chaveiro do desktop. Sem um chaveiro disponível, entre novamente ao reabrir.")),
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
                                    crate::i18n::tr("Conectando...")
                                } else {
                                    match model.mode {
                                        AuthMode::Login => crate::i18n::tr("Entrar"),
                                        AuthMode::Register => crate::i18n::tr("Criar Conta"),
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
                            AuthMode::Login => crate::i18n::tr("Não tem uma conta? Cadastre-se"),
                            AuthMode::Register => crate::i18n::tr("Já possui conta? Faça login"),
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
            request:uuid::Uuid::new_v4(),scope:uuid::Uuid::nil(),login_root: root.clone(), recovery: None,
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
            LoginMsg::Prepare{scope,server,user,remember}=>{self.scope=scope;self.request=uuid::Uuid::new_v4();self.is_loading=false;self.mode=AuthMode::Login;self.password.clear();self.server_password.clear();self.recovery=None;self.clear_credential_fields=true;self.error_message=None;self.remember_session=remember;self.server_url=server.unwrap_or_default();self.username=user.unwrap_or_default();},
            LoginMsg::Finished{request,result}=>{if request!=self.request{return;}match result{Ok((user,api))=>relm4::SimpleComponent::update(self,LoginMsg::AuthSuccess(user,api),sender),Err(e)=>relm4::SimpleComponent::update(self,LoginMsg::AuthFailed(e),sender)}},
            LoginMsg::SetRememberSession(value)=>self.remember_session=value,
            LoginMsg::Resume=>{
                if self.is_loading||!self.remember_session||self.mode!=AuthMode::Login||self.server_url.trim().is_empty()||self.username.trim().is_empty(){return;}
                self.request=uuid::Uuid::new_v4();let request=self.request;self.is_loading=true;let server=self.server_url.clone();let user=self.username.trim().to_owned();let s=sender.clone();tokio::spawn(async move{
                    let result=match ApiClient::new(&server){Ok(client)=>crate::session::resume(&client,&user).await.map(|profile|profile.map(|profile|(profile,client))).map_err(|e|e.to_string()),Err(_)=>Ok(None)};
                    s.input(LoginMsg::Resumed{request,server,user,result});
                });
            },
            LoginMsg::Resumed{request,server,user,result}=>{
                if request!=self.request{return;}self.is_loading=false;
                if self.server_url!=server||self.username.trim()!=user||self.mode!=AuthMode::Login||!self.password.is_empty(){return;}
                match result{Ok(Some((profile,client)))=>relm4::SimpleComponent::update(self,LoginMsg::AuthSuccess(profile,client),sender),Ok(None)=>{},Err(error)=>self.error_message=Some(format!("{error} Entre com a senha para continuar."))}
            },
            #[cfg(test)] LoginMsg::ClearCredentials => {
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

                self.request=uuid::Uuid::new_v4();let request=self.request;
                let sender_clone = sender.clone();

                // Spawn async task to authenticate
                tokio::spawn(async move {
                    let res_msg = match ApiClient::new(&server_url) {
                        Ok(client) => {
                            match authenticate::authenticate(&client, &mode, &username, &password, &server_password).await {
                                Ok(whoami) => {
                                    let stored=if remember{crate::session::save(&client,&whoami.username).await}else{crate::session::forget_account(&client,&whoami.username).await};
                                    if stored.is_err(){tracing::warn!("Desktop keyring unavailable; session remains in memory only");}
                                    info!("Authenticated as {}", whoami.username);
                                    LoginMsg::AuthSuccess(whoami, client)
                                }
                                Err(error) => LoginMsg::AuthFailed(format!("{error:#}")),
                            }
                        }
                        Err(e) => {
                            error!("Invalid server URL: {e}");
                            LoginMsg::AuthFailed(crate::i18n::fmt("URL do servidor inválida: {e}", &[("e", format!("{}", e))]))
                        }
                    };

                    let result=match res_msg{LoginMsg::AuthSuccess(user,api)=>Ok((user,api)),LoginMsg::AuthFailed(e)=>Err(e),_=>unreachable!()};
                    sender_clone.input(LoginMsg::Finished{request,result});
                });
            }
            LoginMsg::AuthSuccess(whoami, client) => {
                self.password.clear();self.server_password.clear();self.recovery=None;
                self.clear_credential_fields = true;
                self.is_loading = false;
                self.error_message = None;

                // Save last server URL and username in config
                if let Err(e)=AppConfig::update(|cfg|{cfg.server_url=Some(client.base_url().into());cfg.last_username=Some(whoami.username.clone());cfg.remember_session=Some(self.remember_session);}){tracing::warn!("Failed to save config: {e}");}

                let _ = sender.output(LoginOutput::Authenticated {
                    scope:self.scope,whoami,
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
