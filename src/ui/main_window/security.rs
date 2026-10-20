//! EN-US: Account security controls. Secret strings stay in password widgets/request bodies.
//! PT-BR: Controles de segurança da conta; segredos permanecem nos campos de senha e corpos de requisição.
use super::*;
use crate::models::Connection;
use crate::ui::chat::actions::window;
mod design;

#[derive(Debug)]
pub enum SecurityMsg {
    Open{embedded:bool}, Reload(Uuid), SavePassword(Uuid),
    Devices { token: Uuid, result: anyhow::Result<Vec<Connection>> },
    PasswordSaved { token: Uuid, result: anyhow::Result<()> },
    ConfirmDrop { token: Uuid, id: String },
    Drop { token: Uuid, id: String },
    Dropped { token: Uuid, result: anyhow::Result<bool> },
}

#[derive(Default)]
pub(super) struct Security {
    view: Option<SecurityView>,
    confirmation: Option<gtk::Window>,
    jobs: Vec<tokio::task::JoinHandle<()>>,
    mutation: bool,
}
struct SecurityView {
    embedded:bool,
    token: Uuid,
    window: gtk::Window,
    controls: gtk::Box,
    password: gtk::PasswordEntry,
    confirmation: gtk::PasswordEntry,
    devices: gtk::Box,
    sessions_scroll: gtk::ScrolledWindow,
    error: gtk::Label,
    loading: bool,
}
impl Drop for Security {
    fn drop(&mut self) {
        for job in self.jobs.drain(..) { job.abort(); }
        if let Some(view) = self.view.take() { view.password.set_text(""); view.confirmation.set_text(""); view.window.close(); }
        if let Some(window) = self.confirmation.take() { window.close(); }
    }
}

impl MainWindowModel {
    pub(super) fn security_event(&mut self, message: SecurityMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        self.security.jobs.retain(|j| !j.is_finished());
        match message {
            SecurityMsg::Open{embedded} => {
                if embedded&&self.settings_mount(account::settings::Tab::Security).is_none(){return;}
                if let Some(view) = &self.security.view { if view.embedded==embedded&&view.window.is_visible() { view.window.present(); return; } }
                if let Some(old)=self.security.view.take(){old.password.set_text("");old.confirmation.set_text("");if old.embedded{self.clear_settings_tab(account::settings::Tab::Security,&old.window);}else{old.window.close();}}
                let token = Uuid::new_v4();
                let (w, body) = if embedded{self.settings_form(account::settings::Tab::Security).unwrap()}else{window(root, "Segurança da conta")};
                if !embedded{w.set_default_height(-1);}
                body.set_valign(gtk::Align::Start);
                body.set_spacing(24);
                let password_section=gtk::Box::new(gtk::Orientation::Vertical,12);
                password_section.append(&design::heading("Alterar senha"));body.append(&password_section);
                let (fields,password,confirmation)=design::password_fields();password_section.append(&fields);
                let policy = gtk::Label::new(Some("A senha deve atender à política do servidor, incluindo maiúscula e caractere especial."));
                policy.set_wrap(true); policy.set_xalign(0.0);policy.add_css_class("dim-label");password_section.append(&policy);
                let save = gtk::Button::with_label("Alterar senha"); let s = sender.clone();
                save.add_css_class("suggested-action");save.add_css_class("papo-security-save");save.set_halign(gtk::Align::End);save.set_margin_top(4);
                save.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::SavePassword(token))));password_section.append(&save);
                let sessions=gtk::Box::new(gtk::Orientation::Vertical,12);sessions.set_margin_top(12);body.append(&sessions);
                let heading=gtk::Box::new(gtk::Orientation::Horizontal,12);let title=design::heading("Sessões");title.set_hexpand(true);heading.append(&title);sessions.append(&heading);
                let reload = gtk::Button::from_icon_name("papo-view-refresh-symbolic");reload.set_tooltip_text(Some("Atualizar sessões"));reload.update_property(&[gtk::accessible::Property::Label("Atualizar sessões")]);let s = sender.clone();
                reload.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::Reload(token))));heading.append(&reload);
                let all = gtk::Button::with_label("Encerrar todas as sessões"); let s = sender.clone();
                all.add_css_class("destructive-action");all.set_hexpand(true);
                all.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::ConfirmDrop { token, id: "ALL".into() })));
                let devices = gtk::Box::new(gtk::Orientation::Vertical, 12);
                let scroll = gtk::ScrolledWindow::new();
                scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
                scroll.set_propagate_natural_height(true); scroll.set_max_content_height(320);
                scroll.set_child(Some(&devices));sessions.append(&scroll);
                let danger=gtk::Box::new(gtk::Orientation::Vertical,0);danger.add_css_class("papo-security-end-all");danger.append(&all);sessions.append(&danger);
                let error = gtk::Label::new(None);error.set_xalign(0.0);error.set_wrap(true);error.set_visible(false);error.connect_label_notify(|label|label.set_visible(!label.text().is_empty()));body.append(&error);
                let p=password.clone();let c=confirmation.clone();w.connect_close_request(move |_|{p.set_text("");c.set_text("");gtk::glib::Propagation::Proceed});
                w.present(); self.security.view = Some(SecurityView { embedded, token, window: w, controls: body, password, confirmation, devices, sessions_scroll: scroll, error, loading: false });
                sender.input(MainWindowMsg::Security(SecurityMsg::Reload(token)));
            }
            SecurityMsg::Reload(token) => {
                let Some(view) = self.security.view.as_mut().filter(|v| v.token == token && v.window.is_visible() && !v.loading) else { return; };
                view.loading = true; view.error.set_text("Carregando sessões…");
                let api = self.api_client.clone(); let s = sender.clone();
                self.security.jobs.push(tokio::spawn(async move { let result = api.connected_devices().await; s.input(MainWindowMsg::Security(SecurityMsg::Devices { token, result })); }));
            }
            SecurityMsg::Devices { token, result } => {
                let Some(view) = self.security.view.as_mut().filter(|v| v.token == token && v.window.is_visible()) else { return; };
                view.loading = false;
                match result {
                    Err(error) => { view.error.set_text(&error.to_string()); sender.input(MainWindowMsg::ActionError(error)); }
                    Ok(devices) => {
                        view.error.set_text(""); while let Some(child) = view.devices.first_child() { view.devices.remove(&child); }
                        for device in devices {
                            if device.expires_at<=chrono::Utc::now(){continue;}
                            let (row,drop)=design::session(&device,self.voice.connection==Some(device.id));
                            let s = sender.clone(); let id = device.id.to_string();
                            drop.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::ConfirmDrop { token, id: id.clone() })));view.devices.append(&row);
                        }
                        // Reserve the height of actual rows, bounded for long session lists.
                        let height=view.devices.measure(gtk::Orientation::Vertical,(view.window.width()-64).max(1)).1.min(320);
                        view.sessions_scroll.set_min_content_height(height);
                        if !view.embedded&&view.window.default_height()==-1 {
                            view.window.set_default_height(view.controls.measure(gtk::Orientation::Vertical,view.window.width()).1);
                        }
                    }
                }
            }
            SecurityMsg::SavePassword(token) => {
                if self.security.mutation { return; }
                let Some(view) = self.security.view.as_ref().filter(|v| v.token == token && v.window.is_visible()) else { return; };
                let password = view.password.text().to_string();
                if password.is_empty() || password != view.confirmation.text() { view.error.set_text("Informe e confirme a mesma nova senha."); return; }
                self.security.mutation = true; view.controls.set_sensitive(false); view.error.set_text("Alterando senha…");
                let api = self.api_client.clone(); let user = self.current_user.id; let s = sender.clone();
                self.security.jobs.push(tokio::spawn(async move { let result = api.change_own_password(user, &password).await; s.input(MainWindowMsg::Security(SecurityMsg::PasswordSaved { token, result })); }));
            }
            SecurityMsg::PasswordSaved { token, result } => {
                self.security.mutation = false;
                let Some(view) = self.security.view.as_ref().filter(|v| v.token == token) else { return; }; view.controls.set_sensitive(true);
                match result {
                    Ok(()) => { view.password.set_text(""); view.confirmation.set_text(""); view.error.set_text("Senha alterada. Use a nova senha no próximo login."); }
                    Err(error) => { view.error.set_text(&error.to_string()); sender.input(MainWindowMsg::ActionError(error)); }
                }
            }
            SecurityMsg::ConfirmDrop { token, id } => {
                if self.security.mutation || !self.security.view.as_ref().is_some_and(|v| v.token == token && v.window.is_visible()) { return; }
                if let Some(w) = self.security.confirmation.take() { w.close(); }
                let (w, body) = window(root, "Encerrar sessão?");
                if let Some(view)=&self.security.view{w.set_transient_for(Some(&view.window));}
                w.set_default_height(-1);
                let warning = gtk::Label::new(Some(if id == "ALL" { "Todas as sessões serão encerradas, incluindo esta." } else { "Se esta for sua sessão atual, será necessário entrar novamente." }));
                warning.set_wrap(true); body.append(&warning);
                let cancel = gtk::Button::with_label("Cancelar"); let close = w.clone(); cancel.connect_clicked(move |_| close.close()); body.append(&cancel);
                let confirm = gtk::Button::with_label("Confirmar encerramento"); let s = sender.clone(); let close = w.clone();
                confirm.connect_clicked(move |_| { close.close(); s.input(MainWindowMsg::Security(SecurityMsg::Drop { token, id: id.clone() })); }); body.append(&confirm);
                w.present(); self.security.confirmation = Some(w);
            }
            SecurityMsg::Drop { token, id } => {
                if self.security.mutation { return; }
                let Some(view) = self.security.view.as_ref().filter(|v| v.token == token && v.window.is_visible()) else { return; };
                self.security.mutation = true; view.controls.set_sensitive(false); view.error.set_text("Encerrando sessão…");
                let api = self.api_client.clone(); let s = sender.clone();
                self.security.jobs.push(tokio::spawn(async move {
                    let result = async { api.drop_connection(&id).await?; if id == "ALL" { return Ok(true); }
                        match api.whoami().await { Ok(_) => Ok(false), Err(error) if crate::api::is_session_error(&error) => Ok(true), Err(error) => Err(error) }
                    }.await;
                    s.input(MainWindowMsg::Security(SecurityMsg::Dropped { token, result }));
                }));
            }
            SecurityMsg::Dropped { token, result } => {
                self.security.mutation = false;
                match result {
                    Ok(true) => { self.voice_disconnected();let _ = sender.output(MainWindowOutput::SessionExpired); }
                    Ok(false) => { if let Some(v) = self.security.view.as_ref().filter(|v| v.token == token) { v.controls.set_sensitive(true); v.error.set_text("Sessão encerrada."); } sender.input(MainWindowMsg::Security(SecurityMsg::Reload(token))); }
                    Err(error) => { if let Some(v) = self.security.view.as_ref().filter(|v| v.token == token) { v.controls.set_sensitive(true); v.error.set_text(&error.to_string()); } sender.input(MainWindowMsg::ActionError(error)); }
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::until;
    main.emit(MainWindowMsg::Security(SecurityMsg::Open{embedded:false}));until(context,||main.model().security.view.as_ref().is_some_and(|v|!v.loading));
    exercise_controls(main,context);
}
#[cfg(test)]
pub(crate) fn exercise_controls(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{find_button,pump,until};
    let w=main.model().security.view.as_ref().unwrap().window.clone();
    let embedded=main.model().security.view.as_ref().unwrap().embedded;
    crate::ui::main_window::layout::tests::preview(&w,if embedded{"account-settings-security"}else{"security"},context);
    {let m=main.model();let v=m.security.view.as_ref().unwrap();v.password.set_text("Password!9");v.confirmation.set_text("Mismatch");}
    find_button(w.upcast_ref(),"Alterar senha").emit_clicked();pump(context);assert!(main.model().security.view.as_ref().unwrap().error.text().contains("mesma"));
    main.model().security.view.as_ref().unwrap().confirmation.set_text("Password!9");find_button(w.upcast_ref(),"Alterar senha").emit_clicked();
    until(context,||!main.model().security.mutation&&main.model().security.view.as_ref().unwrap().error.text().contains("política"));assert_eq!(main.model().security.view.as_ref().unwrap().password.text(),"Password!9");
    find_button(w.upcast_ref(),"Alterar senha").emit_clicked();until(context,||!main.model().security.mutation&&main.model().security.view.as_ref().unwrap().password.text().is_empty());
    let token=main.model().security.view.as_ref().unwrap().token;
    main.emit(MainWindowMsg::Security(SecurityMsg::ConfirmDrop{token,id:"12345678-1234-4234-8234-123456789ac0".into()}));pump(context);
    let dialog=main.model().security.confirmation.as_ref().unwrap().clone();find_button(dialog.upcast_ref(),"Cancelar").emit_clicked();pump(context);assert!(!main.model().security.mutation);
    main.emit(MainWindowMsg::Security(SecurityMsg::ConfirmDrop{token,id:"12345678-1234-4234-8234-123456789ac0".into()}));pump(context);let dialog=main.model().security.confirmation.as_ref().unwrap().clone();find_button(dialog.upcast_ref(),"Confirmar encerramento").emit_clicked();
    until(context,||!main.model().security.mutation&&!main.model().security.view.as_ref().unwrap().loading&&crate::ui::chat::actions::tests::descendants(w.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).filter(|b|b.label().as_deref()==Some("Encerrar sessão")).count()==1);
    assert!(main.model().active_channel_id.is_some());
}
#[cfg(test)]
pub(crate) fn exercise_current(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{find_button,pump};let token=main.model().security.view.as_ref().unwrap().token;
    main.emit(MainWindowMsg::Security(SecurityMsg::ConfirmDrop{token,id:main.model().current_user.id.to_string()}));pump(context);let dialog=main.model().security.confirmation.as_ref().unwrap().clone();find_button(dialog.upcast_ref(),"Confirmar encerramento").emit_clicked();
}
