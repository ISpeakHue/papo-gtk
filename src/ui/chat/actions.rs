//! Message and emoji workflows, with request identities scoped to the selected channel.
#[cfg(test)]
pub(crate) mod tests;
use super::*;
use crate::api::ApiClient;
use crate::models::{Emoji, CreateEmojiRequest, ReactionGroup};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Actions {
    pub api: Option<ApiClient>,
    pub epoch: Uuid,
    pub pinned: Vec<Message>,
    pub pins_ready: bool,
    pins_request: Option<Uuid>,
    pins_window: Option<(gtk::Window, gtk::Box)>,
    pub emojis: Vec<Emoji>,
    pub textures: HashMap<Uuid, gtk::gdk::Texture>,
    pub reaction_hints:std::rc::Rc<std::cell::RefCell<HashMap<(Uuid,Option<Uuid>,Option<String>),String>>>,
    emojis_request: Option<Uuid>,
    pub busy: HashSet<Uuid>,
    reaction_requests: HashMap<Uuid, Uuid>,
    reaction_dirty: HashSet<Uuid>,
    reaction_groups: HashMap<Uuid, Vec<ReactionGroup>>,
    edit: Option<EditView>,
    confirmation: Option<gtk::Window>,
    picker: Option<(gtk::Window, gtk::Box, Uuid)>,
    participants: Option<(gtk::Window, gtk::Box, Uuid)>,
    manager: Option<EmojiManager>,
    pub navigation: Option<Uuid>,
    pub highlight: Option<Uuid>,
    navigation_cursor: Option<MessageCursor>,
}

struct EditView {
    token: Uuid, message: Message, window: gtk::Window, buffer: gtk::TextBuffer,
    save: gtk::Button, error: gtk::Label, text: gtk::TextView, pending: bool,
}
struct EmojiManager {
    token: Uuid, window: gtk::Window, list: gtk::Box, name: gtk::Entry,
    file: gtk::Label, error: gtk::Label, choose: gtk::Button, upload: gtk::Button,
    bytes: Option<Vec<u8>>, pending: bool,
}

#[derive(Debug)]
pub enum ActionMsg {
    Reload,
    OpenEdit(Message), SaveEdit(Uuid), CancelEdit(Uuid),
    EditFinished { epoch: Uuid, token: Uuid, result: anyhow::Result<Message> },
    ConfirmDelete(Message), DeleteConfirmed { epoch: Uuid, id: Uuid },
    DeleteFinished { epoch: Uuid, id: Uuid, result: anyhow::Result<()> },
    Pin(Uuid), PinEvent { id: Uuid, pinned: bool }, OpenPins,
    PinsLoaded { epoch: Uuid, token: Uuid, result: anyhow::Result<Vec<Message>> },
    PinFinished { epoch: Uuid, id: Uuid, result: anyhow::Result<()> },
    Navigate(Uuid), CancelNavigation,
    OpenPicker(Uuid), OpenParticipants(Uuid), Reconcile(Uuid),HoverReactions(Uuid),
    ToggleReaction { id: Uuid, emoji_id: Option<Uuid>, unicode: Option<String> },
    ReactionsLoaded { epoch: Uuid, token: Uuid, id: Uuid, mutation: bool, result: anyhow::Result<Vec<ReactionGroup>> },
    OpenEmojiManager, ReloadEmojis,
    EmojisLoaded { token: Uuid, result: anyhow::Result<Vec<Emoji>> },
    ChooseEmojiFile(Uuid), EmojiFileLoaded { token: Uuid, result: anyhow::Result<Vec<u8>> },
    UploadEmoji(Uuid), ConfirmDeleteEmoji(Uuid), DeleteEmoji { token: Uuid, id: Uuid },
    EmojiSaved { token: Uuid, result: anyhow::Result<()> },
}

pub(crate) fn window(root: &gtk::Box, title: &str) -> (gtk::Window, gtk::Box) {
    let window = gtk::Window::builder().title(title).default_width(460).default_height(360).modal(true).build();
    if let Some(parent) = root.root().and_then(|r| r.downcast::<gtk::Window>().ok()) { window.set_transient_for(Some(&parent)); }
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 10);
    for (set, value) in [(0, 16), (1, 16), (2, 16), (3, 16)] {
        match set { 0 => outer.set_margin_start(value), 1 => outer.set_margin_end(value), 2 => outer.set_margin_top(value), _ => outer.set_margin_bottom(value) }
    }
    window.set_child(Some(&outer));
    (window, outer)
}
fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text)); label.set_wrap(true); label.set_xalign(0.0); label
}
fn clear(container: &gtk::Box) { while let Some(child) = container.first_child() { container.remove(&child); } }
fn button(text: &str, sender: &ComponentSender<ChatModel>, make: impl Fn() -> ActionMsg + 'static) -> gtk::Button {
    let b = gtk::Button::with_label(text); let sender = sender.clone();
    b.connect_clicked(move |_| sender.input(ChatMsg::Action(make()))); b
}
fn scroll_box(outer: &gtk::Box) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let scroll = gtk::ScrolledWindow::new(); scroll.set_vexpand(true); scroll.set_min_content_height(180);
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never); scroll.set_child(Some(&content)); outer.append(&scroll); content
}

impl Actions {
    pub fn new(api: Option<ApiClient>) -> Self { let mut state = Self::default(); state.api = api; state }
    pub fn emoji_label(&self, id: Uuid) -> String {
        self.emojis.iter().find(|e| e.id == id).map(|e| format!(":{}:", e.name)).unwrap_or_else(|| "Emoji personalizado".into())
    }
    pub fn is_pinned(&self, id: Uuid) -> bool { self.pinned.iter().any(|m| m.id == id) }
    pub fn reset_channel(&mut self) {
        self.epoch = Uuid::new_v4(); self.pinned.clear(); self.pins_ready = false;
        self.pins_request = None; self.busy.clear(); self.reaction_requests.clear(); self.reaction_dirty.clear(); self.reaction_groups.clear();self.reaction_hints.borrow_mut().clear();
        self.navigation = None; self.highlight = None; self.navigation_cursor = None;
        if let Some(view) = self.edit.take() { view.window.close(); }
        if let Some(w) = self.confirmation.take() { w.close(); }
        if let Some((w, _)) = self.pins_window.take() { w.close(); }
        if let Some((w, _, _)) = self.picker.take() { w.close(); }
        if let Some((w, _, _)) = self.participants.take() { w.close(); }
    }
}
impl Drop for Actions {
    fn drop(&mut self) { self.reset_channel(); if let Some(m) = self.manager.take() { m.window.close(); } }
}

impl ChatModel {
    fn action_error(&mut self, error: anyhow::Error, sender: &ComponentSender<Self>, visible: bool) {
        if visible { self.draft.error = Some(error.to_string()); }
        // The parent handles session expiry/access refresh. Dialogs retain their own errors.
        let _ = sender.output(ChatOutput::ActionError(error));
    }
    fn request_pins(&mut self, sender: &ComponentSender<Self>) {
        let (Some(api), Some(channel)) = (self.actions.api.clone(), self.active_channel.as_ref().map(|c| c.id())) else { return; };
        if !self.access.read { return; }
        let epoch = self.actions.epoch; let token = Uuid::new_v4(); self.actions.pins_request = Some(token);
        let sender = sender.clone(); tokio::spawn(async move {
            let result = api.pinned_messages(channel).await;
            sender.input(ChatMsg::Action(ActionMsg::PinsLoaded { epoch, token, result }));
        });
    }
    fn request_emojis(&mut self, sender: &ComponentSender<Self>) {
        let Some(api) = self.actions.api.clone() else { return; };
        let token = Uuid::new_v4(); self.actions.emojis_request = Some(token);
        let sender = sender.clone(); tokio::spawn(async move {
            let result = api.all_emojis().await;
            sender.input(ChatMsg::Action(ActionMsg::EmojisLoaded { token, result }));
        });
    }
    fn request_reactions(&mut self, id: Uuid, sender: &ComponentSender<Self>) {
        let (Some(api), Some(channel)) = (self.actions.api.clone(), self.active_channel.as_ref().map(|c| c.id())) else { return; };
        if !self.access.read { return; }
        if self.actions.busy.contains(&id) { self.actions.reaction_dirty.insert(id); return; }
        if !self.history.messages.iter().any(|m| m.id == id) { return; }
        if self.actions.reaction_requests.contains_key(&id) { self.actions.reaction_dirty.insert(id); return; }
        let epoch = self.actions.epoch; let token = Uuid::new_v4(); self.actions.reaction_requests.insert(id, token);
        let sender = sender.clone(); tokio::spawn(async move {
            let result = api.all_reactions(channel, id).await;
            sender.input(ChatMsg::Action(ActionMsg::ReactionsLoaded { epoch, token, id, mutation: false, result }));
        });
    }

    pub(super) fn handle_action(&mut self, action: ActionMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        match action {
            ActionMsg::Reload => { self.request_pins(sender); self.request_emojis(sender); }
            ActionMsg::OpenEdit(message) => {
                let Some(user) = self.user_id else { return; };
                if !self.access.can_edit(user, message.author_id) || self.active_channel.as_ref().map(|c| c.id()) != Some(message.channel_id) { return; }
                let _ = sender.output(ChatOutput::RefreshAccess);
                if let Some(old) = self.actions.edit.take() { old.window.close(); }
                let token = Uuid::new_v4(); let (w, outer) = window(root, "Editar mensagem");
                let text = gtk::TextView::new(); text.set_wrap_mode(gtk::WrapMode::WordChar);
                text.buffer().set_text(message.content.as_deref().unwrap_or(""));
                let scroll = gtk::ScrolledWindow::new(); scroll.set_vexpand(true); scroll.set_child(Some(&text)); outer.append(&scroll);
                let error = label(""); error.add_css_class("error"); outer.append(&error);
                let save = button("Salvar edição", sender, move || ActionMsg::SaveEdit(token)); save.add_css_class("suggested-action"); outer.append(&save);
                outer.append(&button("Cancelar edição", sender, move || ActionMsg::CancelEdit(token)));
                self.actions.edit = Some(EditView { token, message, window: w.clone(), buffer: text.buffer(), save, error, text, pending: false });
                w.present();
            }
            ActionMsg::CancelEdit(token) => {
                if self.actions.edit.as_ref().is_some_and(|e| e.token == token) { self.actions.edit.take().unwrap().window.close(); }
            }
            ActionMsg::SaveEdit(token) => {
                let (Some(api), Some(user)) = (self.actions.api.clone(), self.user_id) else { return; };
                let Some(edit) = self.actions.edit.as_mut().filter(|e| e.token == token && !e.pending) else { return; };
                if !self.access.can_edit(user, edit.message.author_id) { edit.error.set_text("Sem permissão para editar esta mensagem."); return; }
                let text = edit.buffer.text(&edit.buffer.start_iter(), &edit.buffer.end_iter(), false).to_string();
                if text.chars().count() > 8192 { edit.error.set_text("Use até 8192 caracteres."); return; }
                edit.pending = true; edit.text.set_sensitive(false); edit.save.set_sensitive(false); edit.error.set_text("Salvando…");
                let id = edit.message.id; let epoch = self.actions.epoch; let sender = sender.clone();
                tokio::spawn(async move { let result = api.edit_message(id, &text).await;
                    sender.input(ChatMsg::Action(ActionMsg::EditFinished { epoch, token, result })); });
            }
            ActionMsg::EditFinished { epoch, token, result } => {
                let current = epoch == self.actions.epoch;
                match result {
                    Ok(message) if current => {
                        self.history.apply(Change::Edit(message.id, message.content.unwrap_or_default(), message.edited_at));
                        if self.actions.edit.as_ref().is_some_and(|e| e.token == token) { self.actions.edit.take().unwrap().window.close(); }
                    }
                    Err(error) => {
                        if current { if let Some(edit) = self.actions.edit.as_mut().filter(|e| e.token == token) {
                            edit.pending = false; edit.text.set_sensitive(true); edit.save.set_sensitive(true); edit.error.set_text(&error.to_string());
                        } }
                        self.action_error(error, sender, current);
                    }
                    _ => {}
                }
            }
            ActionMsg::ConfirmDelete(message) => {
                let Some(user) = self.user_id else { return; };
                if !self.access.can_delete(user, message.author_id) { return; }
                let (w, outer) = window(root, "Excluir mensagem?");w.set_default_size(360,-1);w.set_resizable(false);
                let heading=label("Excluir esta mensagem?");heading.add_css_class("title-3");outer.append(&heading);
                outer.append(&label("A mensagem será excluída para todos os membros."));
                let buttons=gtk::Box::new(gtk::Orientation::Horizontal,8);buttons.set_halign(gtk::Align::End);buttons.set_margin_top(8);
                let epoch = self.actions.epoch; let id = message.id;
                let confirm = button("Excluir mensagem", sender, move || ActionMsg::DeleteConfirmed { epoch, id }); confirm.add_css_class("destructive-action");
                let cancel = gtk::Button::with_label("Cancelar"); let weak = w.downgrade(); cancel.connect_clicked(move |_| { if let Some(w) = weak.upgrade() { w.close(); } });buttons.append(&cancel);buttons.append(&confirm);outer.append(&buttons);cancel.grab_focus();
                if let Some(old) = self.actions.confirmation.replace(w.clone()) { old.close(); } w.present();
            }
            ActionMsg::DeleteConfirmed { epoch, id } => {
                if epoch != self.actions.epoch || self.actions.busy.contains(&id) { return; }
                let (Some(api), Some(user), Some(m)) = (self.actions.api.clone(), self.user_id, self.history.messages.iter().find(|m| m.id == id)) else { return; };
                if !self.access.can_delete(user, m.author_id) { return; }
                if let Some(w) = self.actions.confirmation.take() { w.close(); }
                self.actions.busy.insert(id); let sender = sender.clone();
                tokio::spawn(async move { let result = api.delete_message(id).await;
                    sender.input(ChatMsg::Action(ActionMsg::DeleteFinished { epoch, id, result })); });
            }
            ActionMsg::DeleteFinished { epoch, id, result } => {
                let current = epoch == self.actions.epoch;
                if current { self.actions.busy.remove(&id); }
                match result {
                    Ok(()) if current => { self.history.apply(Change::Delete(id)); self.actions.pinned.retain(|m| m.id != id); self.request_pins(sender); }
                    Err(error) => self.action_error(error, sender, current), _ => {}
                }
            }
            ActionMsg::Pin(id) => {
                if !self.access.pin || !self.actions.pins_ready || self.actions.busy.contains(&id) { return; }
                let (Some(api), Some(channel)) = (self.actions.api.clone(), self.active_channel.as_ref().map(|c| c.id())) else { return; };
                let remove = self.actions.is_pinned(id); let epoch = self.actions.epoch; self.actions.busy.insert(id);
                let sender = sender.clone(); tokio::spawn(async move {
                    let result = if remove { api.unpin_message(channel, id).await } else { api.pin_message(channel, id).await };
                    sender.input(ChatMsg::Action(ActionMsg::PinFinished { epoch, id, result }));
                });
            }
            ActionMsg::PinFinished { epoch, id, result } => {
                let current = epoch == self.actions.epoch; if current { self.actions.busy.remove(&id); }
                match result { Ok(()) if current => self.request_pins(sender), Err(error) => self.action_error(error, sender, current), _ => {} }
                if current && self.actions.reaction_dirty.remove(&id) { self.request_reactions(id, sender); }
            }
            ActionMsg::PinEvent { id, pinned } => {
                if !pinned { self.actions.pinned.retain(|m| m.id != id); }
                else if !self.actions.is_pinned(id) { if let Some(m) = self.history.messages.iter().find(|m| m.id == id) { self.actions.pinned.push(m.clone()); } }
                self.request_pins(sender);
            }
            ActionMsg::OpenPins => {
                if !self.access.read { return; }
                let (w, outer) = window(root, "Mensagens fixadas"); let list = scroll_box(&outer);
                outer.append(&button("Atualizar mensagens fixadas", sender, || ActionMsg::OpenPins));
                if let Some((old, _)) = self.actions.pins_window.replace((w.clone(), list)) { old.close(); }
                self.render_pins(sender); self.request_pins(sender); w.present();
            }
            ActionMsg::PinsLoaded { epoch, token, result } => {
                let current = epoch == self.actions.epoch && self.actions.pins_request == Some(token);
                if current { self.actions.pins_request = None; }
                match result {
                    Ok(pinned) if current => { self.actions.pinned = pinned.into_iter().filter(|m| !self.history.is_deleted(m.id)).collect(); self.actions.pins_ready = true; self.render_pins(sender); }
                    Err(error) => { if current { self.actions.pins_ready = false; if let Some((_, list)) = &self.actions.pins_window { clear(list); list.append(&label(&format!("Não foi possível carregar: {error}"))); } } self.action_error(error, sender, current); }
                    _ => {}
                }
            }
            ActionMsg::Navigate(id) => {
                if !self.access.read { return; }
                self.actions.navigation = Some(id); self.actions.highlight = None; self.actions.navigation_cursor = None;
                self.continue_navigation(sender);
            }
            ActionMsg::CancelNavigation => { self.actions.navigation = None; self.actions.highlight = None; }
            ActionMsg::OpenPicker(id) => {
                let _ = sender.output(ChatOutput::RefreshAccess);
                if !self.access.send { return; }
                let (w, outer) = window(root, "Escolher reação"); let list = scroll_box(&outer);
                if let Some((old, _, _)) = self.actions.picker.replace((w.clone(), list, id)) { old.close(); }
                self.render_picker(sender); self.request_emojis(sender); w.present();
            }
            ActionMsg::OpenParticipants(id) => {
                if !self.access.read { return; }
                let (w, outer) = window(root, "Quem reagiu"); let list = scroll_box(&outer);
                list.append(&label("Carregando reações…"));
                outer.append(&button("Atualizar reações", sender, move || ActionMsg::Reconcile(id)));
                if let Some((old, _, _)) = self.actions.participants.replace((w.clone(), list, id)) { old.close(); }
                self.request_reactions(id, sender); w.present();
            }
            ActionMsg::HoverReactions(id)=>{if !self.actions.reaction_groups.contains_key(&id)&&!self.actions.reaction_requests.contains_key(&id)&&!self.actions.busy.contains(&id){self.request_reactions(id,sender);}},
            ActionMsg::Reconcile(id) => {self.actions.reaction_hints.borrow_mut().retain(|(message,_,_),_|*message!=id);self.request_reactions(id, sender);},
            ActionMsg::ToggleReaction { id, emoji_id, unicode } => {
                if !self.access.read || self.actions.busy.contains(&id) { return; }
                let (Some(api), Some(user), Some(channel)) = (self.actions.api.clone(), self.user_id, self.active_channel.as_ref().map(|c| c.id())) else { return; };
                if !self.history.messages.iter().any(|m| m.id == id) { return; }
                if emoji_id.is_some() == unicode.is_some() || unicode.as_ref().is_some_and(|s| s.is_empty() || s.chars().count() > 16) {
                    self.draft.error = Some("Escolha um emoji de até 16 caracteres.".into()); return;
                }
                // Refresh membership before toggling; count-only events cannot identify our own reaction.
                let can_add = self.access.send; let epoch = self.actions.epoch; let token = Uuid::new_v4();
                self.actions.busy.insert(id); self.actions.reaction_requests.insert(id, token);
                let sender = sender.clone(); tokio::spawn(async move {
                    let result = async {
                        let groups = api.all_reactions(channel, id).await?;
                        let own = groups.iter().any(|g| g.emoji_id == emoji_id && g.unicode == unicode && g.users.iter().any(|u| u.user_id == user));
                        if own { api.remove_reaction(channel, id, emoji_id, unicode.as_deref()).await?; }
                        else { anyhow::ensure!(can_add, "Sem permissão para adicionar reações."); api.add_reaction(channel, id, emoji_id, unicode.as_deref()).await?; }
                        api.all_reactions(channel, id).await
                    }.await;
                    sender.input(ChatMsg::Action(ActionMsg::ReactionsLoaded { epoch, token, id, mutation: true, result }));
                });
            }
            ActionMsg::ReactionsLoaded { epoch, token, id, mutation, result } => {
                let current = epoch == self.actions.epoch && self.actions.reaction_requests.get(&id) == Some(&token);
                if current { self.actions.reaction_requests.remove(&id); if mutation { self.actions.busy.remove(&id); } }
                match result {
                    Ok(groups) if current => {
                        // Do not briefly overwrite a newer event with an older read snapshot.
                        if !mutation && self.actions.reaction_dirty.remove(&id) { self.request_reactions(id, sender); return; }
                        self.history.apply(Change::Reactions(id, groups.clone(), self.user_id.unwrap_or_default()));
                        self.actions.reaction_groups.insert(id, groups);self.refresh_reaction_hints();root.trigger_tooltip_query(); self.render_participants(id);
                        if self.actions.reaction_dirty.remove(&id) { self.request_reactions(id, sender); }
                    }
                    Err(error) => { if current { if let Some((_, list, target)) = &self.actions.participants { if *target == id { clear(list); list.append(&label(&format!("Não foi possível carregar: {error}"))); } } } self.action_error(error, sender, current);
                        if current && self.actions.reaction_dirty.remove(&id) { self.request_reactions(id, sender); }
                    }
                    _ => {}
                }
            }
            ActionMsg::OpenEmojiManager => {
                let _ = sender.output(ChatOutput::RefreshAccess);
                let token = Uuid::new_v4(); let (w, outer) = window(root, "Emojis do servidor");
                let list = scroll_box(&outer); let name = gtk::Entry::new(); name.set_placeholder_text(Some("Nome do emoji (até 32 caracteres)")); name.set_max_length(32);
                name.set_visible(self.access.manage_server); outer.append(&name);
                let file = label("PNG, JPEG, GIF ou WebP; até 256 KB e 512 × 512 pixels."); file.set_visible(self.access.manage_server); outer.append(&file);
                let choose = button("Escolher imagem do emoji", sender, move || ActionMsg::ChooseEmojiFile(token)); choose.set_visible(self.access.manage_server); outer.append(&choose);
                let error = label(""); error.add_css_class("error"); outer.append(&error);
                let upload = button("Criar emoji", sender, move || ActionMsg::UploadEmoji(token)); upload.set_sensitive(false); upload.set_visible(self.access.manage_server); outer.append(&upload);
                outer.append(&button("Atualizar emojis", sender, || ActionMsg::ReloadEmojis));
                let manager = EmojiManager { token, window: w.clone(), list, name, file, error, choose, upload, bytes: None, pending: false };
                if let Some(old) = self.actions.manager.replace(manager) { old.window.close(); }
                self.render_manager(sender); self.request_emojis(sender); w.present();
            }
            ActionMsg::ReloadEmojis => self.request_emojis(sender),
            ActionMsg::EmojisLoaded { token, result } => {
                let current = self.actions.emojis_request == Some(token); if current { self.actions.emojis_request = None; }
                match result {
                    Ok(mut emojis) if current => {
                        self.actions.textures.retain(|id, _| emojis.iter().any(|e| e.id == *id));
                        for e in &mut emojis {
                            if !self.actions.textures.contains_key(&e.id) {
                                if let Some(t) = e.image_blob.as_deref().and_then(crate::media::emoji_texture) { self.actions.textures.insert(e.id, t); }
                            }
                            // Emoji blobs are immutable by ID; retain compact textures only.
                            e.image_blob = None;
                        }
                        self.actions.emojis = emojis; self.render_picker(sender); self.render_manager(sender);
                    }
                    Err(error) => { if current { if let Some(m) = &self.actions.manager { m.error.set_text(&error.to_string()); } if let Some((_, list, _)) = &self.actions.picker { list.append(&label(&format!("Emojis personalizados indisponíveis: {error}"))); } } self.action_error(error, sender, current); }
                    _ => {}
                }
            }
            ActionMsg::ChooseEmojiFile(token) => {
                let Some(m) = self.actions.manager.as_ref().filter(|m| m.token == token) else { return; };
                if !self.access.manage_server || m.pending { return; }
                let dialog = gtk::FileDialog::builder().title("Imagem do emoji").build();
                let parent = m.window.clone(); let sender = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    if let Ok(file) = dialog.open_future(Some(&parent)).await {
                        let result = match file.path() { Some(path) => read_emoji_file(path).await, None => Err(anyhow::anyhow!("Escolha um arquivo local.")) };
                        sender.input(ChatMsg::Action(ActionMsg::EmojiFileLoaded { token, result }));
                    }
                });
            }
            ActionMsg::EmojiFileLoaded { token, result } => {
                if let Some(m) = self.actions.manager.as_mut().filter(|m| m.token == token && !m.pending) {
                    match result {
                        Ok(bytes) => { m.file.set_text(&format!("Imagem selecionada: {} bytes", bytes.len())); m.bytes = Some(bytes); m.upload.set_sensitive(self.access.manage_server); m.error.set_text(""); }
                        Err(error) => { m.error.set_text(&error.to_string()); }
                    }
                }
            }
            ActionMsg::UploadEmoji(token) => {
                let Some(api) = self.actions.api.clone() else { return; };
                let Some(m) = self.actions.manager.as_mut().filter(|m| m.token == token && !m.pending) else { return; };
                if !self.access.manage_server { m.error.set_text("Sem permissão para criar emojis."); return; }
                let Some(bytes) = &m.bytes else { m.error.set_text("Escolha uma imagem."); return; };
                let request = match CreateEmojiRequest::from_image(&m.name.text(), bytes) { Ok(r) => r, Err(error) => { m.error.set_text(&error.to_string()); return; } };
                m.pending = true; m.upload.set_sensitive(false); m.error.set_text("Enviando…");
                let sender = sender.clone(); tokio::spawn(async move {
                    let result = api.create_emoji(&request).await.map(|_| ());
                    sender.input(ChatMsg::Action(ActionMsg::EmojiSaved { token, result }));
                });
            }
            ActionMsg::ConfirmDeleteEmoji(id) => {
                let Some(user) = self.user_id else { return; };
                let Some(emoji) = self.actions.emojis.iter().find(|e| e.id == id) else { return; };
                if !self.access.can_delete_emoji(user, emoji.created_by) { return; }
                let Some(m) = &self.actions.manager else { return; }; let token = m.token;
                let (w, outer) = window(root, "Excluir emoji?"); outer.append(&label(&format!("Excluir :{}: do servidor?", emoji.name)));
                let confirm = button("Excluir emoji", sender, move || ActionMsg::DeleteEmoji { token, id }); confirm.add_css_class("destructive-action"); outer.append(&confirm);
                let cancel = gtk::Button::with_label("Cancelar"); let weak = w.downgrade(); cancel.connect_clicked(move |_| { if let Some(w) = weak.upgrade() { w.close(); } }); outer.append(&cancel);
                if let Some(old) = self.actions.confirmation.replace(w.clone()) { old.close(); } w.present();
            }
            ActionMsg::DeleteEmoji { token, id } => {
                let (Some(api), Some(user)) = (self.actions.api.clone(), self.user_id) else { return; };
                let Some(emoji) = self.actions.emojis.iter().find(|e| e.id == id) else { return; };
                if !self.access.can_delete_emoji(user, emoji.created_by) { return; }
                let Some(m) = self.actions.manager.as_mut().filter(|m| m.token == token && !m.pending) else { return; };
                m.pending = true; m.upload.set_sensitive(false); m.error.set_text("Excluindo…");
                if let Some(w) = self.actions.confirmation.take() { w.close(); }
                let sender = sender.clone(); tokio::spawn(async move { let result = api.delete_emoji(id).await;
                    sender.input(ChatMsg::Action(ActionMsg::EmojiSaved { token, result })); });
            }
            ActionMsg::EmojiSaved { token, result } => {
                match result {
                    Ok(()) => { if let Some(m) = self.actions.manager.as_mut().filter(|m| m.token == token) { m.pending = false; m.error.set_text("Concluído."); m.upload.set_sensitive(self.access.manage_server && m.bytes.is_some()); } self.request_emojis(sender); }
                    Err(error) => { if let Some(m) = self.actions.manager.as_mut().filter(|m| m.token == token) { m.pending = false; m.error.set_text(&error.to_string()); m.upload.set_sensitive(self.access.manage_server && m.bytes.is_some()); } self.action_error(error, sender, false); }
                }
            }
        }
    }

    pub(super) fn continue_navigation(&mut self, sender: &ComponentSender<Self>) {
        let Some(id) = self.actions.navigation else { return; };
        if self.history.messages.iter().any(|m| m.id == id) {
            self.actions.highlight = Some(id); self.actions.navigation = None; self.draft.error = None; return;
        }
        if self.history.loading() || self.history_error.is_some() { return; }
        if !self.has_more || self.history.is_deleted(id) {
            self.actions.navigation = None; self.draft.error = Some("Mensagem não disponível: excluída ou fora do histórico acessível.".into()); return;
        }
        let cursor = self.history.messages.first().map(MessageCursor::from);
        if cursor.is_none() || cursor == self.actions.navigation_cursor {
            self.actions.navigation = None; self.draft.error = Some("Não foi possível avançar no histórico para localizar a mensagem.".into()); return;
        }
        self.actions.navigation_cursor = cursor;
        if let Some(channel) = &self.active_channel { let _ = sender.output(ChatOutput::LoadMoreMessages { channel_id: channel.id(), cursor }); }
    }
    fn render_pins(&self, sender: &ComponentSender<Self>) {
        let Some((_, list)) = &self.actions.pins_window else { return; }; clear(list);
        if !self.actions.pins_ready { list.append(&label("Carregando mensagens fixadas…")); }
        else if self.actions.pinned.is_empty() { list.append(&label("Nenhuma mensagem fixada.")); }
        for m in &self.actions.pinned {
            let id = m.id; let open = button(m.content.as_deref().unwrap_or("Mensagem com anexos"), sender, move || ActionMsg::Navigate(id)); open.set_tooltip_text(Some("Abrir mensagem no histórico")); list.append(&open);
        }
    }
    fn render_picker(&self, sender: &ComponentSender<Self>) {
        let Some((_, list, id)) = &self.actions.picker else { return; }; clear(list);
        let id = *id;
        list.append(&label("Emojis Unicode"));
        let flow = gtk::FlowBox::new(); flow.set_selection_mode(gtk::SelectionMode::None); flow.set_max_children_per_line(8);
        for value in ["❤️", "👍", "👎", "😀", "😂", "🎉", "🔥", "👀", "🙏", "✅", "❌", "😢", "🤔", "👏", "🚀", "💯", "🥳", "😮", "☕", "🐈"] {
            let b = button(value, sender, move || ActionMsg::ToggleReaction { id, emoji_id: None, unicode: Some(value.into()) });
            b.set_sensitive(self.access.send && !self.actions.busy.contains(&id)); flow.insert(&b, -1);
        } list.append(&flow);
        let custom = gtk::Entry::new(); custom.set_placeholder_text(Some("Outro emoji Unicode (cole aqui)")); custom.set_max_length(16); custom.set_sensitive(self.access.send && !self.actions.busy.contains(&id));
        let s = sender.clone(); custom.connect_activate(move |entry| { s.input(ChatMsg::Action(ActionMsg::ToggleReaction { id, emoji_id: None, unicode: Some(entry.text().to_string()) })); }); list.append(&custom);
        list.append(&label("Emojis personalizados"));
        for emoji in &self.actions.emojis {
            let emoji_id = emoji.id; let b = button(&format!(":{}:", emoji.name), sender, move || ActionMsg::ToggleReaction { id, emoji_id: Some(emoji_id), unicode: None });
            if let Some(t) = self.actions.textures.get(&emoji.id) { let row = gtk::Box::new(gtk::Orientation::Horizontal, 8); row.append(&emoji_image(t)); row.append(&label(&emoji.name)); b.set_child(Some(&row)); }
            b.set_sensitive(self.access.send && !self.actions.busy.contains(&id)); list.append(&b);
        }
        if self.actions.emojis.is_empty() { list.append(&label("Nenhum emoji personalizado carregado.")); }
    }
    pub(super) fn refresh_reaction_hints(&self){
        let mut hints=self.actions.reaction_hints.borrow_mut();hints.clear();
        for (message,groups) in &self.actions.reaction_groups{for group in groups{
            let emoji=group.unicode.clone().or_else(||group.emoji_id.map(|id|self.actions.emoji_label(id))).unwrap_or_else(||"Emoji removido".into());
            let mut names:Vec<_>=group.users.iter().take(15).map(|u|if Some(u.user_id)==self.user_id{"Você".into()}else{self.users_map.get(&u.user_id).map(|u|u.display_name().to_owned()).unwrap_or_else(||"Membro removido".into())}).collect();
            if group.users.len()>15{names.push(format!("mais {} membros",group.users.len()-15));}
            let text=if names.is_empty(){"Nenhum membro reagiu".into()}else{format!("{} reagiu/reagiram com {emoji}",names.join(", "))};
            hints.insert((*message,group.emoji_id,group.unicode.clone()),text);
        }}
    }
    fn render_participants(&self, id: Uuid) {
        let Some((_, list, target)) = &self.actions.participants else { return; }; if *target != id { return; } clear(list);
        let Some(groups) = self.actions.reaction_groups.get(&id) else { return; };
        if groups.is_empty() { list.append(&label("Nenhuma reação.")); }
        for group in groups {
            let name = group.unicode.clone().or_else(|| group.emoji_id.and_then(|id| self.actions.emojis.iter().find(|e| e.id == id).map(|e| format!(":{}:", e.name)))).unwrap_or("Emoji removido".into());
            list.append(&label(&format!("{name} — {}", group.users.len())));
            for u in &group.users {
                let name = self.users_map.get(&u.user_id).map(|u| u.display_name().to_owned()).unwrap_or_else(|| u.user_id.to_string());
                list.append(&label(&format!("  {name}{}", if Some(u.user_id) == self.user_id { " (você)" } else { "" })));
            }
        }
    }
    pub(super) fn render_manager(&self, sender: &ComponentSender<Self>) {
        let Some(m) = &self.actions.manager else { return; }; clear(&m.list);
        m.choose.set_visible(self.access.manage_server); m.choose.set_sensitive(self.access.manage_server && !m.pending);
        m.name.set_sensitive(!m.pending); m.name.set_visible(self.access.manage_server); m.file.set_visible(self.access.manage_server);
        m.upload.set_visible(self.access.manage_server); m.upload.set_sensitive(self.access.manage_server && m.bytes.is_some() && !m.pending);
        for e in &self.actions.emojis {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if let Some(t) = self.actions.textures.get(&e.id) { row.append(&emoji_image(t)); }
            let name = label(&format!(":{}:", e.name)); name.set_hexpand(true); row.append(&name);
            if self.user_id.is_some_and(|user| self.access.can_delete_emoji(user, e.created_by)) {
                let id = e.id; let delete = button("Excluir", sender, move || ActionMsg::ConfirmDeleteEmoji(id)); delete.set_sensitive(!m.pending); row.append(&delete);
            } m.list.append(&row);
        }
    }
}
fn emoji_image(texture: &gtk::gdk::Texture) -> gtk::Image { let image = gtk::Image::from_paintable(Some(texture)); image.set_pixel_size(24); image }
async fn read_emoji_file(path: std::path::PathBuf) -> anyhow::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::new(); file.take(256 * 1024 + 1).read_to_end(&mut bytes).await?;
    // The name is validated separately at upload time.
    CreateEmojiRequest::from_image("validation", &bytes)?;
    Ok(bytes)
}
