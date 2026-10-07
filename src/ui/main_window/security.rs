//! Account security controls. Secret strings stay in password widgets/request bodies.
use super::*;
use crate::models::Connection;
use crate::ui::chat::actions::window;

#[derive(Debug)]
pub enum SecurityMsg {
    Open, Reload(Uuid), SavePassword(Uuid),
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
    token: Uuid,
    window: gtk::Window,
    controls: gtk::Box,
    password: gtk::PasswordEntry,
    confirmation: gtk::PasswordEntry,
    devices: gtk::Box,
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
            SecurityMsg::Open => {
                if let Some(view) = &self.security.view { if view.window.is_visible() || self.security.mutation { view.window.present(); return; } }
                if let Some(old)=self.security.view.take(){old.password.set_text("");old.confirmation.set_text("");old.window.close();}
                let token = Uuid::new_v4();
                let (w, body) = window(root, "Segurança da conta");
                let password = gtk::PasswordEntry::new(); password.set_show_peek_icon(true);
                password.set_placeholder_text(Some("Nova senha")); body.append(&password);
                let confirmation = gtk::PasswordEntry::new(); confirmation.set_placeholder_text(Some("Repita a nova senha")); body.append(&confirmation);
                body.append(&gtk::Label::new(Some("A senha deve atender à política do servidor, incluindo maiúscula e caractere especial.")));
                let save = gtk::Button::with_label("Alterar senha"); let s = sender.clone();
                save.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::SavePassword(token)))); body.append(&save);
                let reload = gtk::Button::with_label("Atualizar sessões"); let s = sender.clone();
                reload.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::Reload(token)))); body.append(&reload);
                let all = gtk::Button::with_label("Encerrar todas as sessões"); let s = sender.clone();
                all.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::ConfirmDrop { token, id: "ALL".into() }))); body.append(&all);
                let devices = gtk::Box::new(gtk::Orientation::Vertical, 8);
                let scroll = gtk::ScrolledWindow::new(); scroll.set_min_content_height(180); scroll.set_child(Some(&devices)); body.append(&scroll);
                let error = gtk::Label::new(None); error.set_wrap(true); body.append(&error);
                let p=password.clone();let c=confirmation.clone();w.connect_close_request(move |_|{p.set_text("");c.set_text("");gtk::glib::Propagation::Proceed});
                w.present(); self.security.view = Some(SecurityView { token, window: w, controls: body, password, confirmation, devices, error, loading: false });
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
                            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                            row.append(&gtk::Label::new(Some(&format!("{}\nCriada: {} · Expira: {}", device.id, device.created_at.format("%d/%m/%Y %H:%M"), device.expires_at.format("%d/%m/%Y %H:%M")))));
                            let drop = gtk::Button::with_label("Encerrar sessão"); let s = sender.clone(); let id = device.id.to_string();
                            drop.connect_clicked(move |_| s.input(MainWindowMsg::Security(SecurityMsg::ConfirmDrop { token, id: id.clone() }))); row.append(&drop); view.devices.append(&row);
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
                body.append(&gtk::Label::new(Some(if id == "ALL" { "Todas as sessões serão encerradas, incluindo esta." } else { "Se esta for sua sessão atual, será necessário entrar novamente." })));
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
    use crate::ui::chat::actions::tests::{find_button,pump,until};
    find_button(main.widget().upcast_ref(),"Segurança").emit_clicked();until(context,||main.model().security.view.as_ref().is_some_and(|v|!v.loading));
    let w=main.model().security.view.as_ref().unwrap().window.clone();
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
