//! A scoped, read-only conversation renders pins without changing chat history.
use super::*;
use std::cell::RefCell;

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub messages: Option<Vec<Message>>,
    users: Vec<UserSummary>,
    avatars: HashMap<Uuid, gtk::gdk::Texture>,
    emojis: Vec<Emoji>,
    emoji_textures: HashMap<Uuid, gtk::gdk::Texture>,
    emoji_animations: HashMap<Uuid, crate::media::animation::Animation>,
    config: crate::models::UserConfig,
    server_name: String,
    media: super::super::transfers::PinnedMedia,
}
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedSnapshot").field("messages", &self.messages.as_ref().map(Vec::len)).finish_non_exhaustive()
    }
}
impl Snapshot {
    pub(in crate::ui::chat) fn apply(self, model: &mut ChatModel) {
        model.users_map = self.users.into_iter().map(|user| (user.id, user)).collect();
        model.avatars = self.avatars;
        model.actions.emojis = self.emojis;
        model.actions.emoji_names = model.actions.emojis.iter().map(|e| (e.name.clone(), e.id)).collect();
        model.actions.textures = self.emoji_textures;
        model.actions.animations = self.emoji_animations;
        model.config = self.config;
        *model.actions.emoji_source.borrow_mut()=self.server_name.clone();
        model.server_name = self.server_name;
        model.transfers.seed_pinned(self.media);
        if let Some(messages) = self.messages {
            let keep: HashSet<_> = messages.iter().map(|m| m.id).collect();
            let removed: Vec<_> = model.history.messages.iter().filter(|m| !keep.contains(&m.id)).map(|m| m.id).collect();
            for id in removed { model.transfers.invalidate_message(id); }
            model.history.retain_pins(&keep);
            for message in messages { model.history.apply(Change::Upsert(message)); }
        }
        model.history_ready = true;
        // IDs suppress grouping so each pin keeps its own author/date heading.
        model.actions.pinned = model.history.messages.iter().map(|m| {
            let mut m = m.clone();
            for embed in m.embeds.iter_mut().flatten() { embed.image_data = None; embed.author_image_data = None; }
            m
        }).collect();
    }
}

pub(in crate::ui::chat) struct View {
    pub window: gtk::Window,
    pub chat: Controller<ChatModel>,
    status: gtk::Label,
    // GTK animations stay on the main thread; only a wake-up crosses the input queue.
    inbox: std::rc::Rc<RefCell<Option<Snapshot>>>,
    signatures: RefCell<Vec<String>>,
}
impl View {
    pub fn new(parent: &ChatModel, root: &gtk::Box, sender: &ComponentSender<ChatModel>) -> Self {
        let (window, outer) = window(root, "Mensagens fixadas");
        window.set_default_size(680, 600);
        window.set_widget_name(&format!("pins-{}", Uuid::new_v4()));
        let status = label("Carregando mensagens fixadas…");
        outer.append(&status);
        let channel = parent.active_channel.as_ref().and_then(|target| if let crate::models::ConversationTarget::Channel(c) = target { Some(c.clone()) } else { None });
        let output = sender.clone();
        let weak = window.downgrade();
        let inbox=std::rc::Rc::new(RefCell::new(None));
        let chat = ChatModel::builder().launch(ChatInit {
            active_channel: channel, api: parent.actions.api.clone(), pinned: true, pinned_inbox:Some(inbox.clone()), ..Default::default()
        }).connect_receiver(move |_, event| {
            match event {
                ChatOutput::Navigate { .. } => {
                    let _ = output.output(event);
                    if let Some(window) = weak.upgrade() { window.close(); }
                },
                ChatOutput::OpenProfile(_) | ChatOutput::ActionError(_) | ChatOutput::RefreshAccess | ChatOutput::AuthorMetadata { .. } => { let _ = output.output(event); },
                // Viewing pins must never mark the channel read or fetch history.
                _ => {},
            }
        });
        outer.append(chat.widget());
        let token = window.widget_name().to_string();
        let input = sender.input_sender().clone();
        window.connect_close_request(move |_| {
            let token = token.clone(); let input = input.clone();
            gtk::glib::idle_add_local_once(move || { let _ = input.send(ChatMsg::Action(ActionMsg::PinsClosed(token))); });
            gtk::glib::Propagation::Proceed
        });
        Self { window, chat, status, inbox, signatures: Default::default() }
    }

    pub fn update(&self, parent: &ChatModel) {
        if !self.window.is_visible() { return; }
        self.status.set_visible(!parent.actions.pins_ready);
        self.chat.widget().set_visible(parent.actions.pins_ready);
        if !parent.actions.pins_ready { return; }
        let signatures: Vec<_> = parent.actions.pinned.iter().map(|m| format!("{}|{}|{:?}|{:?}", m.id, super::super::message_body_signature(m), m.reactions, m.user_reactions)).collect();
        let messages = if *self.signatures.borrow() != signatures {
            *self.signatures.borrow_mut() = signatures;
            Some(parent.actions.pinned.clone())
        } else { None };
        let messages=messages.or_else(||self.inbox.borrow_mut().as_mut().and_then(|snapshot|snapshot.messages.take()));
        *self.inbox.borrow_mut()=Some(Snapshot {
            messages, users: parent.users_map.values().cloned().collect(), avatars: parent.avatars.clone(),
            emojis: parent.actions.emojis.clone(), emoji_textures: parent.actions.textures.clone(),
            emoji_animations: parent.actions.animations.clone(), config: parent.config.clone(), server_name: parent.server_name.clone(),
            media: parent.transfers.pinned_media(&parent.actions.pinned),
        });
        self.chat.emit(ChatMsg::PinnedSnapshot);
    }

    pub fn show_error(&self, message: &str) {
        self.status.set_text(message); self.status.set_visible(true); self.chat.widget().set_visible(false);
    }
}

impl Drop for View {
    fn drop(&mut self) { self.window.close(); }
}

impl ChatModel {
    pub(in crate::ui::chat) fn update_pin(&mut self, change: &Change, sender: &ComponentSender<Self>) {
        if self.actions.pinned_view { return; }
        let id=match change { Change::Upsert(m)=>m.id, Change::Edit(id,..)|Change::Reaction(id,..)|Change::Reactions(id,..)|Change::Preview(id,..)|Change::Embeds(id,..)|Change::RemovePreview(id,..)|Change::Moderation(id,..)|Change::Delete(id)=>*id };
        if !self.actions.is_pinned(id) { return; }
        if let Some(view)=&self.actions.pins_window {
            view.chat.emit(if matches!(change,Change::Delete(_)){ChatMsg::DeleteMessage(id)}else{ChatMsg::ApplyChange(change.clone())});
        }
        let mut pins=History::default();
        if let Some(channel)=self.active_channel.as_ref(){pins.select(channel.id());}
        for message in &self.actions.pinned { pins.apply(Change::Upsert(message.clone())); }
        pins.apply(change.clone());self.actions.pinned=pins.messages;
        self.render_pins(sender);
    }
}
