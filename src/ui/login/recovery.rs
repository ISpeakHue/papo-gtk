use super::*;

pub(super) struct Recovery {
    pub token: uuid::Uuid,
    window: gtk::Window,
    body: gtk::Box,
    link: gtk::PasswordEntry,
    password: gtk::PasswordEntry,
    confirmation: gtk::PasswordEntry,
    error: gtk::Label,
    pending: bool,
    api: ApiClient,
    job: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Recovery {
    fn drop(&mut self) { if let Some(job) = self.job.take() { job.abort(); } self.link.set_text(""); self.password.set_text(""); self.confirmation.set_text(""); self.window.close(); }
}
impl LoginModel {
    pub(super) fn open_recovery(&mut self, sender: &ComponentSender<Self>) {
        let api = match ApiClient::new(&self.server_url) { Ok(api) => api, Err(_) => { self.error_message = Some("Informe o endereço da API para recuperar a senha.".into()); return; } };
        if let Some(view) = &self.recovery { if view.window.is_visible() { view.window.present(); return; } }
        let token = uuid::Uuid::new_v4();
        let w = gtk::Window::builder().title("Recuperar senha").default_width(440).modal(true).build();
        if let Some(parent) = self.login_root.root().and_then(|r| r.downcast::<gtk::Window>().ok()) { w.set_transient_for(Some(&parent)); }
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        for edge in 0..4 { match edge { 0 => body.set_margin_top(16), 1 => body.set_margin_bottom(16), 2 => body.set_margin_start(16), _ => body.set_margin_end(16) } }
        let hint = gtk::Label::new(Some("Cole o token ou link de uso único fornecido pelo administrador.")); hint.set_wrap(true); body.append(&hint);
        let link = gtk::PasswordEntry::new(); link.set_show_peek_icon(true); link.set_placeholder_text(Some("Token ou link de recuperação")); body.append(&link);
        let password = gtk::PasswordEntry::new(); password.set_show_peek_icon(true); password.set_placeholder_text(Some("Nova senha")); body.append(&password);
        let confirmation = gtk::PasswordEntry::new(); confirmation.set_placeholder_text(Some("Repita a nova senha")); body.append(&confirmation);
        let button = gtk::Button::with_label("Salvar nova senha"); let s = sender.clone(); button.connect_clicked(move |_| s.input(LoginMsg::Recover(token))); body.append(&button);
        let error = gtk::Label::new(None); error.set_wrap(true); body.append(&error); w.set_child(Some(&body)); w.present();
        let l=link.clone();let p=password.clone();let c=confirmation.clone();w.connect_close_request(move |_|{l.set_text("");p.set_text("");c.set_text("");gtk::glib::Propagation::Proceed});
        self.recovery = Some(Recovery { token, window: w, body, link, password, confirmation, error, pending: false, api, job: None });
    }
    pub(super) fn recover(&mut self, token: uuid::Uuid, sender: &ComponentSender<Self>) {
        let Some(view) = self.recovery.as_mut().filter(|v| v.token == token && !v.pending && v.window.is_visible()) else { return; };
        let password = view.password.text().to_string();
        if password.is_empty() || password != view.confirmation.text() { view.error.set_text("Informe e confirme a mesma nova senha."); return; }
        let link = view.link.text().to_string();
        let token_value = match crate::api::security::recovery_token(&link) { Ok(t) => t, Err(error) => { view.error.set_text(&error.to_string()); return; } };
        view.pending = true; view.body.set_sensitive(false); view.error.set_text("Salvando…");
        let api = view.api.clone(); let s = sender.clone();
        view.job = Some(tokio::spawn(async move { let result = api.recover_password(&token_value, &password).await; s.input(LoginMsg::Recovered { token, result }); }));
    }
    pub(super) fn recovered(&mut self, token: uuid::Uuid, result: anyhow::Result<()>) {
        let Some(view) = self.recovery.as_mut().filter(|v| v.token == token) else { return; };
        view.pending = false; view.body.set_sensitive(true);
        match result { Ok(()) => { view.link.set_text(""); view.password.set_text(""); view.confirmation.set_text(""); view.error.set_text("Senha alterada. Entre com a nova senha."); }, Err(error) => view.error.set_text(&error.to_string()) }
    }
}
#[cfg(test)]
pub(crate) fn exercise(api:&ApiClient,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{find_button,until};
    let login=LoginModel::builder().launch(AppConfig{server_url:Some(api.base_url().into()),..Default::default()}).detach();
    find_button(login.widget().upcast_ref(),"Tenho um link de recuperação").emit_clicked();until(context,||login.model().recovery.is_some());let w=login.model().recovery.as_ref().unwrap().window.clone();
    {let m=login.model();let v=m.recovery.as_ref().unwrap();v.link.set_text("https://papo.cyberasilo.online/passwordchange/SecretToken");v.password.set_text("Password!9");v.confirmation.set_text("Password!9");}
    find_button(w.upcast_ref(),"Salvar nova senha").emit_clicked();until(context,||!login.model().recovery.as_ref().unwrap().pending&&login.model().recovery.as_ref().unwrap().error.text().contains("expirado"));assert!(!login.model().recovery.as_ref().unwrap().link.text().is_empty());
    find_button(w.upcast_ref(),"Salvar nova senha").emit_clicked();until(context,||!login.model().recovery.as_ref().unwrap().pending&&login.model().recovery.as_ref().unwrap().link.text().is_empty());assert!(login.model().recovery.as_ref().unwrap().password.text().is_empty());w.close();
}
