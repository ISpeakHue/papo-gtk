//! Profile and preferences dialogs; accepted writes are the source of local state.
use super::*;
use crate::models::*;
use crate::ui::chat::actions::window;
#[derive(Debug)]
pub enum AccountMsg {
    Profile(Uuid), RefreshProfile(Uuid), Preferences, ChannelPreferences, ProfileLoaded {
        token: Uuid, result: anyhow::Result<(UserProfile, Option<Vec<u8>>)>
    }, SaveProfile(Uuid), Image {
        token: Uuid, banner: bool, remove: bool
    }, ImageSelected {
        token: Uuid, banner: bool, result: anyhow::Result<Vec<u8>>
    }, ProfileSaved {
        token: Uuid, result: anyhow::Result<()>
    }, OwnUpdated {
        token: Uuid, result: anyhow::Result<WhoamiResponse>
    }, SavePreferences(Uuid), PreferencesSaved {
        token: Uuid, result: anyhow::Result<UserSettings>
    }, SaveChannel {
        token: Uuid, channel: Uuid, value: NotificationSettings
    }, ChannelSaved {
        token: Uuid, channel: Uuid, value: NotificationSettings, result: anyhow::Result<()>
    },
}
#[derive(Default)]
pub(super) struct Account {
    profile: Option<ProfileView>, preferences: Option<PreferencesView>, channel: Option<(Uuid, gtk::Window, gtk::Box, gtk::Label)>, jobs: Vec<tokio::task::JoinHandle<()>>, mutation: Option<Uuid>, settings_pending: bool, channel_pending: bool, restore_fields: Option<(Uuid, UpdateUserRequest, u32)>, pub config: UserConfig, css: Option<gtk::CssProvider>,
}
struct ProfileView {
    token: Uuid, id: Uuid, window: gtk::Window, body: gtk::Box, error: gtk::Label, fields: Option<ProfileFields>, pending: bool, image_write: bool
}
struct ProfileFields {
    nickname: gtk::Entry, status: gtk::Entry, description: gtk::TextBuffer, typing: gtk::Entry, manual: gtk::DropDown
}
struct PreferencesView {
    token: Uuid, window: gtk::Window, body: gtk::Box, error: gtk::Label, theme: gtk::DropDown, font: gtk::DropDown, density: gtk::DropDown, checks: Vec<gtk::CheckButton>, pending: bool
}
impl Drop for Account {
    fn drop(&mut self) {
        for j in self.jobs.drain(..) {
            j.abort();
        }
        if let Some(v) = self.profile.take() {
            v.window.close();
        }
        if let Some(v) = self.preferences.take() {
            v.window.close();
        }
        if let Some((_, w, _, _)) = self.channel.take() {
            w.close();
        }
        if let (Some(display), Some(css)) = (gtk::gdk::Display::default(), self.css.take()) {
            gtk::style_context_remove_provider_for_display(&display, &css);
        }
    }
}
fn entry(body: &gtk::Box, label: &str, text: &str)->gtk::Entry {
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    body.append(&l);
    let e = gtk::Entry::new();
    e.set_text(text);
    body.append(&e);
    e
}
fn dropdown(body: &gtk::Box, label: &str, values: &[&str], selected: u32)->gtk::DropDown {
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    body.append(&l);
    let e = gtk::DropDown::from_strings(values);
    e.set_selected(selected);
    body.append(&e);
    e
}
impl MainWindowModel {
    pub(super) fn profile_action_status(&self,text:&str){if let Some(v)=&self.account.profile{if v.window.is_visible(){v.error.set_text(text);}}}
    pub(super) fn close_profile_for_direct(&mut self,peer:Uuid){if self.account.profile.as_ref().is_some_and(|v|v.id==peer){if let Some(v)=self.account.profile.take(){v.window.close();}}}

    pub(super) fn apply_config(&mut self, root: &gtk::Box) {
        use adw::prelude::*;
        self.account.config = self.account.config.complete();
        let config = &self.account.config;
        adw::StyleManager::default().set_color_scheme(match config.theme {
            Some(Theme::Dark) => adw::ColorScheme::ForceDark, Some(Theme::Light) => adw::ColorScheme::ForceLight, _ => adw::ColorScheme::Default
        });
        self.chat.emit(ChatMsg::SetConfig(config.clone()));
        self.sidebar.emit(SidebarMsg::SetConfig(config.clone()));
        self.user_list.emit(UserListMsg::SetConfig(config.clone()));
        let font = match config.display.as_ref().and_then(|d|d.font_size.as_ref()) {
            Some(FontSize::Small) => 12, Some(FontSize::Huge) => 18, _ => 14
        };
        root.add_css_class("papo-app");
        let css = self.account.css.get_or_insert_with(gtk::CssProvider::new);
        css.load_from_string(&format!(".papo-app .papo-message-text {{font-size: {font}px;}}"));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }
    pub(super) fn account_event(&mut self, msg: AccountMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        self.account.jobs.retain(|j|!j.is_finished());
        match msg {
            AccountMsg::RefreshProfile(id) => {
                if self.account.profile.as_ref().is_some_and(|v|v.id==id&&v.fields.is_none()&&v.window.is_visible()) {
                    sender.input(MainWindowMsg::Account(AccountMsg::Profile(id)));
                }
            }
            AccountMsg::Profile(id) => {
                if let Some(v) = self.account.profile.take() {
                    v.window.close();
                }
                let token = Uuid::new_v4();
                let (w, body) = window(root, "Perfil");
                w.set_default_size(720,780);
                let scroll = gtk::ScrolledWindow::new();
                scroll.set_vexpand(true);scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
                let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
                scroll.set_child(Some(&content));
                body.append(&scroll);
                let error = gtk::Label::new(Some("Carregando perfil…"));
                error.set_wrap(true);
                body.append(&error);
                w.present();
                self.account.profile = Some(ProfileView {
                    token, id, window: w, body: content, error, fields: None, pending: false, image_write: false
                });
                let api = self.api_client.clone();
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = async {
                        let p = api.get_user_profile(id).await?;
                        anyhow::ensure!(p.id==id, "Perfil retornado para outro usuário");
                        let banner = if let Some(sha) = &p.banner_media {
                            api.media_bytes(&format!("/media/{sha}"), 2<<20).await.ok()
                        }
                        else {
                            None
                        };
                        Ok((p, banner))
                    }.await;
                    s.input(MainWindowMsg::Account(AccountMsg::ProfileLoaded {
                        token, result
                    }));
                }));
            }
            AccountMsg::ProfileLoaded {
                token, result
            } => {
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token&&v.window.is_visible())else {
                    return;
                };
                match result {
                    Err(e) => {
                        v.error.set_text(&format!("Perfil indisponível: {e}"));
                        sender.input(MainWindowMsg::ActionError(e));
                    }, Ok((p, banner)) => {
                        v.error.set_text("");
                        if let Some(t) = banner.as_deref().and_then(crate::media::bounded_texture) {
                            let picture = gtk::Picture::for_paintable(&t);
                            picture.set_height_request(140);
                            picture.set_can_shrink(true);
                            v.body.append(&picture);
                        }
                        self.avatars.seed(p.id, p.avatar_blob.as_deref());
                        let avatar = self.avatars.textures.get(&p.id).cloned();
                        let identity=gtk::Box::new(gtk::Orientation::Horizontal,16);let picture=crate::media::avatar_image(avatar.as_ref(),80);picture.set_text(Some(p.display_name()));identity.append(&picture);
                        let names=gtk::Box::new(gtk::Orientation::Vertical,4);names.set_valign(gtk::Align::Center);let name=gtk::Label::new(Some(p.display_name()));name.add_css_class("title-2");name.set_xalign(0.0);name.set_ellipsize(gtk::pango::EllipsizeMode::End);if let Some(roles)=p.roles.as_deref(){crate::ui::style::role_color(&name,roles);}names.append(&name);let username=gtk::Label::new(Some(&format!("@{}",p.username)));username.add_css_class("dim-label");username.set_xalign(0.0);names.append(&username);identity.append(&names);v.body.append(&identity);
                        let badges=gtk::FlowBox::new();badges.set_selection_mode(gtk::SelectionMode::None);badges.set_min_children_per_line(1);badges.set_max_children_per_line(8);badges.set_halign(gtk::Align::Start);
                        for role in p.roles.iter().flatten() {
                            let badge = gtk::Label::new(Some(&role.name));
                            badge.add_css_class("pill");
                            crate::ui::style::role_color(&badge,std::slice::from_ref(role));badges.insert(&badge,-1);
                        }
                        if badges.first_child().is_some(){v.body.append(&badges);}
                        if p.id!=self.current_user.id{v.body.append(&gtk::Label::new(Some(&format!("Presença manual: {}", match p.status {
                            Some(UserStatus::Away) => "Ausente", Some(UserStatus::Busy) => "Ocupado", None => "Automática"
                        }))));}
                        if p.id==self.current_user.id {
                            let nickname = entry(&v.body, "Apelido (até 32 caracteres)", p.nickname.as_deref().unwrap_or(""));
                            let status = entry(&v.body, "Mensagem de status (até 64 caracteres)", p.status_message.as_deref().unwrap_or(""));
                            let label = gtk::Label::new(Some("Descrição (até 512 caracteres)"));
                            label.set_xalign(0.0);
                            v.body.append(&label);
                            let text = gtk::TextView::new();
                            text.set_wrap_mode(gtk::WrapMode::WordChar);
                            text.set_height_request(100);
                            text.set_vexpand(false);text.set_valign(gtk::Align::Start);
                            let description = text.buffer();
                            description.set_text(p.description.as_deref().unwrap_or(""));
                            v.body.append(&text);
                            let typing = entry(&v.body, "Texto ao digitar (até 64 caracteres)", p.typing.as_deref().unwrap_or(""));
                            let manual = dropdown(&v.body, "Presença manual", &["Automática", "Ausente", "Ocupado"], match p.status {
                                Some(UserStatus::Away) => 1, Some(UserStatus::Busy) => 2, _ => 0
                            });
                            if let Some((id, draft, presence)) = self.account.restore_fields.take() {
                                if id==p.id {
                                    nickname.set_text(&draft.nickname);
                                    status.set_text(&draft.status);
                                    description.set_text(&draft.description);
                                    typing.set_text(draft.typing.as_deref().unwrap_or(""));
                                    manual.set_selected(presence);
                                }
                            }
                            v.fields = Some(ProfileFields {
                                nickname, status, description, typing, manual
                            });
                            let save = gtk::Button::with_label("Salvar perfil");
                            let s = sender.clone();
                            save.connect_clicked(move |_|s.input(MainWindowMsg::Account(AccountMsg::SaveProfile(token))));
                            v.body.append(&save);
                            save.add_css_class("suggested-action");save.set_halign(gtk::Align::End);
                            let image_actions=gtk::FlowBox::new();image_actions.set_selection_mode(gtk::SelectionMode::None);image_actions.set_min_children_per_line(1);image_actions.set_max_children_per_line(4);image_actions.set_column_spacing(6);image_actions.set_row_spacing(6);v.body.append(&image_actions);
                            for (label, banner, remove) in [("Alterar avatar", false, false), ("Remover avatar", false, true), ("Alterar banner", true, false), ("Remover banner", true, true)] {
                                let b = gtk::Button::with_label(label);
                                let s = sender.clone();
                                b.connect_clicked(move |_|s.input(MainWindowMsg::Account(AccountMsg::Image {
                                    token, banner, remove
                                })));
                                image_actions.insert(&b,-1);
                            }
                        }
                        else {
                            for (label,action) in [("Mensagem direta",0),("Bloquear usuário",1)] {
                                let b=gtk::Button::with_label(label);let s=sender.clone();let id=p.id;
                                b.connect_clicked(move |_|s.input(MainWindowMsg::Direct(if action==0{super::direct::DirectMsg::Open(id)}else{super::direct::DirectMsg::Block(id,true)})));v.body.append(&b);
                            }
                            for text in [p.description, p.status_message, p.typing].into_iter().flatten() {
                                let label = gtk::Label::new(Some(&text));
                                label.set_wrap(true);
                                label.set_selectable(true);
                                v.body.append(&label);
                            }
                        }
                    }
                }
                self.publish_avatars();
            }
            AccountMsg::SaveProfile(token) => {
                if self.account.mutation.is_some() {
                    return;
                }
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                let Some(f) = &v.fields else {
                    return;
                };
                let request = UpdateUserRequest {
                    nickname: f.nickname.text().into(), status: f.status.text().into(), description: f.description.text(&f.description.start_iter(), &f.description.end_iter(), false).into(), typing: Some(f.typing.text().into())
                };
                let status = match f.manual.selected() {
                    1 => Some(UserStatus::Away), 2 => Some(UserStatus::Busy), _ => None
                };
                v.pending = true;
                v.image_write = false;
                self.account.mutation = Some(token);
                v.body.set_sensitive(false);
                v.error.set_text("Salvando…");
                let id = v.id;
                let api = self.api_client.clone();
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = async {
                        api.update_profile(id, &request).await?;
                        api.update_status(id, status).await
                    }.await;
                    s.input(MainWindowMsg::Account(AccountMsg::OwnUpdated {
                        token, result: api.whoami().await
                    }));
                    s.input(MainWindowMsg::Account(AccountMsg::ProfileSaved {
                        token, result
                    }));
                }));
            }
            AccountMsg::Image {
                token, banner, remove
            } => {
                let Some(v) = self.account.profile.as_ref().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                if remove {
                    sender.input(MainWindowMsg::Account(AccountMsg::ImageSelected {
                        token, banner, result: Ok(Vec::new())
                    }));
                }
                else {
                    let parent = v.window.clone();
                    let s = sender.clone();
                    gtk::glib::spawn_future_local(async move {
                        if let Ok(file) = gtk::FileDialog::builder().title("Escolher imagem (até 2 MiB)").build().open_future(Some(&parent)).await {
                            if let Some(path) = file.path() {
                                tokio::spawn(async move {
                                    let result = async {
                                        anyhow::ensure!(tokio::fs::metadata(&path).await?.len()<=2<<20, "Use uma imagem de até 2 MiB");
                                        let bytes = tokio::fs::read(path).await?;
                                        crate::api::features::validate_profile_image(&bytes, banner)?;
                                        Ok(bytes)
                                    }.await;
                                    s.input(MainWindowMsg::Account(AccountMsg::ImageSelected {
                                        token, banner, result
                                    }));
                                });
                            }
                        }
                    });
                }
            }
            AccountMsg::ImageSelected {
                token, banner, result
            } => {
                if self.account.mutation.is_some() {
                    return;
                }
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                match result {
                    Err(e) => v.error.set_text(&e.to_string()), Ok(bytes) => {
                        v.pending = true;
                        v.image_write = true;
                        self.account.mutation = Some(token);
                        v.body.set_sensitive(false);
                        v.error.set_text("Salvando imagem…");
                        let id = v.id;
                        let api = self.api_client.clone();
                        let s = sender.clone();
                        self.account.jobs.push(tokio::spawn(async move {
                            let result = api.update_image(id, banner, &bytes).await;
                            s.input(MainWindowMsg::Account(AccountMsg::OwnUpdated {
                                token, result: api.whoami().await
                            }));
                            s.input(MainWindowMsg::Account(AccountMsg::ProfileSaved {
                                token, result
                            }));
                        }));
                    }
                }
            }
            AccountMsg::ProfileSaved {
                token, result
            } => {
                if self.account.mutation==Some(token) {
                    self.account.mutation = None;
                }
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token)else {
                    return;
                };
                v.pending = false;
                v.body.set_sensitive(true);
                match result {
                    Err(e) => {
                        v.error.set_text(&format!("Falha ao salvar: {e}"));
                        sender.input(MainWindowMsg::ActionError(e));
                    }, Ok(()) => {
                        v.error.set_text("Salvo.");
                        let id = v.id;
                        if v.image_write {
                            if let Some(f) = &v.fields {
                                self.account.restore_fields = Some((id, UpdateUserRequest {
                                    nickname: f.nickname.text().into(), status: f.status.text().into(), description: f.description.text(&f.description.start_iter(), &f.description.end_iter(), false).into(), typing: Some(f.typing.text().into())
                                }, f.manual.selected()));
                            }
                        }
                        self.load_avatars(vec![id], true, sender.clone());
                        self.refresh_users(sender.clone(), Some(id));
                        // Reopening after a successful write invalidates older read responses.
                        sender.input(MainWindowMsg::Account(AccountMsg::Profile(id)));
                    }
                }
            }
            AccountMsg::OwnUpdated {
                token, result
            } => {
                if self.account.mutation!=Some(token) {
                    return;
                }
                match result {
                    Ok(mut user) => {
                        // Settings writes have their own accepted snapshot; an overlapping profile read cannot undo them.
                        user.settings = Some(WhoamiSettings {
                            version: 1, config: self.account.config.clone()
                        });
                        self.avatars.seed(user.id, user.avatar_blob.as_deref());
                        self.current_user = user;
                        self.publish_avatars();
                        self.sidebar.emit(SidebarMsg::SetCurrentUser(self.current_user.clone()));
                        self.refresh_channels(sender.clone());
                    }, Err(e) => sender.input(MainWindowMsg::OperationFailed(e))
                }
            }, AccountMsg::Preferences => {
                if let Some(v) = self.account.preferences.take() {
                    v.window.close();
                }
                let token = Uuid::new_v4();
                let (w, body) = window(root, "Preferências");
                let c = self.account.config.complete();
                let d = c.display.as_ref().unwrap();
                let n = c.notifications.as_ref().unwrap();
                let theme = dropdown(&body, "Tema", &["Sistema", "Claro", "Escuro"], match c.theme {
                    Some(Theme::Light) => 1, Some(Theme::Dark) => 2, _ => 0
                });
                let font = dropdown(&body, "Tamanho do texto", &["Pequeno", "Médio", "Grande"], match d.font_size {
                    Some(FontSize::Small) => 0, Some(FontSize::Huge) => 2, _ => 1
                });
                let density = dropdown(&body, "Espaçamento", &["Compacto", "Normal", "Confortável"], match d.message_density {
                    Some(MessageDensity::Compact) => 0, Some(MessageDensity::Comfortable) => 2, _ => 1
                });
                let mut checks = Vec::new();
                for (label, value) in [("Mostrar horários", d.show_timestamps), ("Mostrar avatares", d.show_avatars), ("Ativar notificações", n.enabled), ("Prévia nas notificações", n.message_preview), ("Som das notificações", n.sound), ("Notificar menções", n.mentions)] {
                    let check = gtk::CheckButton::with_label(label);
                    check.set_active(value.unwrap_or(true));
                    body.append(&check);
                    checks.push(check);
                }
                let save = gtk::Button::with_label("Salvar preferências");
                let s = sender.clone();
                save.connect_clicked(move |_|s.input(MainWindowMsg::Account(AccountMsg::SavePreferences(token))));
                body.append(&save);
                let error = gtk::Label::new(None);
                error.set_wrap(true);
                w.set_child(Some(&body));
                body.append(&error);
                w.present();
                self.account.preferences = Some(PreferencesView {
                    token, window: w, body, error, theme, font, density, checks, pending: false
                });
            }
            AccountMsg::SavePreferences(token) => {
                if self.account.settings_pending {
                    return;
                }
                let Some(v) = self.account.preferences.as_mut().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                let mut config = self.account.config.complete();
                config.theme = Some(match v.theme.selected() {
                    1 => Theme::Light, 2 => Theme::Dark, _ => Theme::System
                });
                let d = config.display.as_mut().unwrap();
                d.font_size = Some(match v.font.selected() {
                    0 => FontSize::Small, 2 => FontSize::Huge, _ => FontSize::Medium
                });
                d.message_density = Some(match v.density.selected() {
                    0 => MessageDensity::Compact, 2 => MessageDensity::Comfortable, _ => MessageDensity::Normal
                });
                d.show_timestamps = Some(v.checks[0].is_active());
                d.show_avatars = Some(v.checks[1].is_active());
                let n = config.notifications.as_mut().unwrap();
                n.enabled = Some(v.checks[2].is_active());
                n.message_preview = Some(v.checks[3].is_active());
                n.sound = Some(v.checks[4].is_active());
                n.mentions = Some(v.checks[5].is_active());
                v.pending = true;
                self.account.settings_pending = true;
                v.body.set_sensitive(false);
                v.error.set_text("Salvando…");
                let api = self.api_client.clone();
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = api.save_settings(&config).await;
                    s.input(MainWindowMsg::Account(AccountMsg::PreferencesSaved {
                        token, result
                    }));
                }));
            }
            AccountMsg::PreferencesSaved {
                token, result
            } => {
                self.account.settings_pending = false;
                // Apply a committed write even when its dialog has been replaced or closed.
                match result {
                    Ok(settings) => {
                        if settings.user_id!=self.current_user.id {
                            return;
                        }
                        self.account.config = settings.config.complete();
                        self.current_user.settings = Some(WhoamiSettings {
                            version: settings.version, config: self.account.config.clone()
                        });
                        self.apply_config(root);
                        if let Some(v) = self.account.preferences.as_mut().filter(|v|v.token==token) {
                            v.pending = false;
                            v.body.set_sensitive(true);
                            v.error.set_text("Preferências salvas.");
                        }
                    }, Err(e) => {
                        if let Some(v) = self.account.preferences.as_mut().filter(|v|v.token==token) {
                            v.pending = false;
                            v.body.set_sensitive(true);
                            v.error.set_text(&format!("Falha ao salvar: {e}"));
                        }
                        sender.input(MainWindowMsg::ActionError(e));
                    }
                }
            }
            AccountMsg::ChannelPreferences => {
                let Some(channel) = self.channels.iter().find(|c|Some(c.id)==self.active_channel_id)else {
                    return;
                };
                if let Some((_, w, _, _)) = self.account.channel.take() {
                    w.close();
                }
                let token = Uuid::new_v4();
                let id = channel.id;
                let (w, body) = window(root, &format!("Notificações em #{}", channel.name));
                let mode = dropdown(&body, "Notificar", &["Nunca", "Somente menções", "Todas as mensagens"], match channel.notification_settings {
                    Some(NotificationSettings::Off) => 0, Some(NotificationSettings::All) => 2, _ => 1
                });
                let save = gtk::Button::with_label("Salvar notificações do canal");
                let s = sender.clone();
                save.connect_clicked(move |_|s.input(MainWindowMsg::Account(AccountMsg::SaveChannel {
                    token, channel: id, value: match mode.selected() {
                        0 => NotificationSettings::Off, 1 => NotificationSettings::OnlyMentions, _ => NotificationSettings::All
                    }
                })));
                body.append(&save);
                let error = gtk::Label::new(None);
                error.set_wrap(true);
                body.append(&error);
                w.present();
                self.account.channel = Some((token, w, body, error));
            }
            AccountMsg::SaveChannel {
                token, channel, value
            } => {
                if self.account.channel_pending {
                    return;
                }
                let Some((_, w, body, error)) = self.account.channel.as_ref().filter(|(t, _, _, _)|*t==token)else {
                    return;
                };
                if !w.is_visible()||!body.is_sensitive() {
                    return;
                }
                self.account.channel_pending = true;
                body.set_sensitive(false);
                error.set_text("Salvando…");
                let api = self.api_client.clone();
                let user = self.current_user.id;
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = api.channel_notifications(channel, user, value.clone()).await;
                    s.input(MainWindowMsg::Account(AccountMsg::ChannelSaved {
                        token, channel, value, result
                    }));
                }));
            }
            AccountMsg::ChannelSaved {
                token, channel, value, result
            } => {
                self.account.channel_pending = false;
                match result {
                    Ok(()) => {
                        if let Some(c) = self.channels.iter_mut().find(|c|c.id==channel) {
                            self.notifications.journal.preference(channel,value.clone());
                            c.notification_settings = Some(value);
                            if self.active_channel_id==Some(channel) {
                                self.chat.emit(ChatMsg::UpdateChannel(c.clone()));
                            }
                        }
                        self.publish_voice_channels();
                        if let Some((_, _, body, error)) = self.account.channel.as_ref().filter(|(t, _, _, _)|*t==token) {
                            body.set_sensitive(true);
                            error.set_text("Salvo.");
                        }
                        self.refresh_channels(sender.clone());
                    }, Err(e) => {
                        if let Some((_, _, body, error)) = self.account.channel.as_ref().filter(|(t, _, _, _)|*t==token) {
                            body.set_sensitive(true);
                            error.set_text(&e.to_string());
                        }
                        sender.input(MainWindowMsg::ActionError(e));
                    }
                }
            }
        }
    }
}
#[cfg(test)]
pub(crate) fn exercise_preferences_and_profiles(main: &Controller<MainWindowModel>, context: &gtk::glib::MainContext) {
    use crate::ui::chat::actions::tests:: {
        find_button, pump, until, descendants
    };
    find_button(main.widget().upcast_ref(), "Meu perfil").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|v.fields.is_some()));
    let profile=main.model().account.profile.as_ref().unwrap().window.clone();assert!(profile.default_height()>=720);crate::ui::main_window::layout::tests::preview(&profile,"profile",context);
    let image_button=find_button(profile.upcast_ref(),"Alterar avatar");until(context,||image_button.height()>0);
    let bounds=image_button.compute_bounds(&profile).unwrap();assert!(bounds.y()+bounds.height()<profile.height() as f32,"profile image controls should be visible when the window opens");
    let old = main.model().account.profile.as_ref().unwrap().token;
    {
        let m = main.model();
        let f = m.account.profile.as_ref().unwrap().fields.as_ref().unwrap();
        f.nickname.set_text("Updated name");
        f.status.set_text("Custom message");
        f.description.set_text("Updated description");
        f.typing.set_text("Custom typing");
        f.manual.set_selected(2);
    }
    let w = main.model().account.profile.as_ref().unwrap().window.clone();
    find_button(w.upcast_ref(), "Salvar perfil").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Profile failed")));
    assert_eq!(main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.text(), "Updated name");
    find_button(w.upcast_ref(), "Salvar perfil").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|v.token!=old&&v.fields.is_some()));
    assert_eq!(main.model().current_user.nickname.as_deref(), Some("Updated name"));
    assert_eq!(main.model().current_user.status, Some(UserStatus::Busy));
    let id = main.model().current_user.id;
    main.emit(MainWindowMsg::Account(AccountMsg::ProfileLoaded {
        token: old, result: Err(anyhow::anyhow!("Stale profile"))
    }));
    pump(context);
    assert!(!main.model().account.profile.as_ref().unwrap().error.text().contains("Stale"));
    let token = main.model().account.profile.as_ref().unwrap().token;
    let window = main.model().account.profile.as_ref().unwrap().window.clone();
    find_button(window.upcast_ref(), "Remover avatar").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|v.token!=token&&v.fields.is_some()));
    main.model().account.profile.as_ref().unwrap().window.close();
    find_button(main.widget().upcast_ref(), "Preferências").emit_clicked();
    pump(context);
    {
        let m = main.model();
        let v = m.account.preferences.as_ref().unwrap();
        v.theme.set_selected(2);
        v.font.set_selected(2);
        v.density.set_selected(0);
        for c in &v.checks {
            c.set_active(false);
        }
    }
    let w = main.model().account.preferences.as_ref().unwrap().window.clone();
    find_button(w.upcast_ref(), "Salvar preferências").emit_clicked();
    until(context, ||main.model().account.preferences.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Settings failed")));
    assert_eq!(main.model().account.config.theme, Some(Theme::System));
    assert_eq!(main.model().account.preferences.as_ref().unwrap().theme.selected(), 2);
    find_button(w.upcast_ref(), "Salvar preferências").emit_clicked();
    until(context, ||main.model().account.config.theme==Some(Theme::Dark));
    assert!(!main.model().account.config.display.as_ref().unwrap().show_avatars.unwrap());
    assert!(!main.model().account.config.notifications.as_ref().unwrap().sound.unwrap());
    pump(context);
    assert!(descendants(main.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).all(|a|!a.is_visible()));
    w.close();
    find_button(main.widget().upcast_ref(), "Preferências do canal").emit_clicked();
    pump(context);
    let w = main.model().account.channel.as_ref().unwrap().1.clone();
    let mode = descendants(w.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::DropDown>().ok()).unwrap();
    mode.set_selected(0);
    let read = main.model().channels[0].last_read_message;
    find_button(w.upcast_ref(), "Salvar notificações do canal").emit_clicked();
    until(context, ||main.model().channels[0].notification_settings==Some(NotificationSettings::Off));
    assert_eq!(main.model().channels[0].last_read_message, read);
    w.close();
    // Preferences returned by whoami survive a fresh application component.
    let api = main.model().api_client.clone();
    let current_user = tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    assert_eq!(current_user.id, id);
    let restored = MainWindowModel::builder().launch(MainWindowInit {
        current_user, api_client: api
    }).detach();
    pump(context);
    assert_eq!(restored.model().account.config.theme, Some(Theme::Dark));
    assert_eq!(restored.model().account.config.display.as_ref().unwrap().show_avatars, Some(false));
}
