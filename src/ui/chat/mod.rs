//! Chat view component — displays channel header, message history, attachments, link previews, reactions, and composer.
pub(crate) mod mentions;

pub(crate) mod state;
pub(crate) mod actions;
mod transfers;
mod viewport;
mod text;
use viewport::{Viewport,Position};
#[cfg(test)]pub(crate) mod bugs;
use transfers::{Transfers, TransferMsg};
use actions::{Actions, ActionMsg};
use state::{Change, Draft, History};

use gtk::pango;
use gtk::prelude::*;
use relm4::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use crate::media::{avatar_image, format_file_size};
use crate::models::{Attachment, Channel, ConversationTarget, DirectConversation, LinkPreview, Message, MessageListResponse, MessageCursor, UserSummary};

#[derive(Debug, Clone, Default)]
pub struct ChatInit {
    pub active_channel: Option<Channel>,
    pub messages: Vec<Message>,
    pub api: Option<crate::api::ApiClient>,
    pub user_id: Option<Uuid>,
}

pub struct ChatModel {
    pub active_channel: Option<ConversationTarget>,
    history: History,
    viewport:Viewport,
    rendered:HashMap<String,(String,gtk::ListBoxRow)>,
    viewing_old:bool,
    mention_auto_open:std::rc::Rc<std::cell::Cell<bool>>,mention_manual:bool,mention_signature:String,mention_dismissed:bool,
    older_requested:bool,highlight_generation:u64,
    access: crate::models::Access,
    user_id: Option<Uuid>,
    actions: Actions,
    transfers: Transfers,
    config: crate::models::UserConfig,
    draft: Draft,
    saved_drafts: HashMap<Uuid, Draft>,
    pub users_map: HashMap<Uuid, UserSummary>,
    avatars: HashMap<Uuid, gtk::gdk::Texture>,
    pub typing_users: HashMap<Uuid, Instant>,
    pub last_typing_sent: Option<Instant>,
    has_more: bool,
    history_error: Option<String>,
    retry_cursor: Option<MessageCursor>,
    unread_notifications:usize,more_notifications:bool,
}

#[derive(Debug)]
pub enum ChatMsg {
    ViewportChanged(bool), Latest,
    CopyMessage(String), ContextMenu(Uuid), ContextAt{ id:Uuid,x:f64,y:f64 },
    AutoOlder, ExpireHighlight{epoch:Uuid,id:Uuid,generation:u64},
    Notifications, NotificationCount{count:usize,more:bool},
    Action(ActionMsg),
    Transfer(TransferMsg),
    SetConfig(crate::models::UserConfig),
    OpenProfile(Uuid),
    ChannelPreferences,
    SetAccess { user_id: Uuid, access: crate::models::Access },
    SetChannel(Channel),
    SetDirect(DirectConversation),
    SetTarget(ConversationTarget),
    UpdateDirect(DirectConversation),
    UpdateChannel(Channel),
    ClearChannel,
    LoadOlder,
    ApplyChange(Change),
    BeginHistory { request_id: Uuid, cursor: Option<MessageCursor> },
    OperationError(String),
    HistoryLoaded { request_id: Uuid, result: Result<MessageListResponse, String>, append: bool },
    AddMessage(Message),

    DeleteMessage(Uuid),
    SetUsers(Vec<UserSummary>),
    SetAvatars(HashMap<Uuid, gtk::gdk::Texture>),
    UserTyping { user_id: Uuid, is_typing: bool },
    ClearStaleTyping,
    InputChanged(String),
    MentionSelected(Option<Uuid>),OpenMentions,MentionCursorChanged,CloseMentions,
    SetReply(Option<Message>),
    SendClicked,
    SendFinished { request_id: Uuid, channel_id: Uuid, result: Result<Message, String> },
    ToggleUserList,
    ToggleNavigation,
    ReactionClicked { message_id: Uuid, emoji_id: Option<Uuid>, unicode: Option<String> },
}

#[derive(Debug)]
pub enum ChatOutput {
    Notifications,
    HistoryObserved { channel_id: Uuid, messages: Vec<Message>, latest:bool },
    OpenProfile(Uuid),
    ChannelPreferences,
    ActionError(anyhow::Error),
    RefreshAccess,
    Navigate { channel_id: Uuid, message_id: Uuid },
    UserTyping(Uuid),
    LoadMoreMessages {
        channel_id: Uuid,
        cursor: Option<MessageCursor>,
    },
    ToggleUserList,
    ToggleNavigation,
}

#[relm4::component(pub)]
impl Component for ChatModel {
    type Init = ChatInit;
    type Input = ChatMsg;
    type Output = ChatOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,
            add_css_class: "papo-chat",

            // Conversation toolbar stays useful when sidebars are overlaid.
            gtk::Box {
                set_spacing:8,add_css_class:"papo-chat-heading",
                gtk::Button {set_icon_name:"sidebar-show-symbolic",set_has_frame:false,set_tooltip_text:Some("Canais e conversas (F9)"),connect_clicked => ChatMsg::ToggleNavigation},
                gtk::Label {set_text:"#",add_css_class:"papo-channel-symbol",add_css_class:"dim-label",#[watch] set_visible:!model.active_channel.as_ref().is_some_and(|c|c.is_direct())},
                gtk::Image {set_icon_name:Some("avatar-default-symbolic"),#[watch] set_visible:model.active_channel.as_ref().is_some_and(|c|c.is_direct())},
                gtk::Label {
                    #[watch] set_text:model.active_channel.as_ref().map(|c|c.name()).unwrap_or("Selecione um canal"),
                    add_css_class:"heading",set_ellipsize:pango::EllipsizeMode::End,set_xalign:0.0,
                },
                gtk::Label {
                    #[watch] set_text:model.active_channel.as_ref().and_then(|c|c.topic()).unwrap_or(""),
                    set_xalign:0.0,set_hexpand:true,add_css_class:"caption",add_css_class:"dim-label",set_ellipsize:pango::EllipsizeMode::End,
                },
                gtk::Button {
                    set_icon_name:"view-pin-symbolic",set_has_frame:false,set_tooltip_text:Some("Fixadas"),
                    #[watch] set_sensitive:model.access.read&&model.active_channel.as_ref().is_some_and(|c|!c.is_direct()),
                    connect_clicked => ChatMsg::Action(ActionMsg::OpenPins),
                },
                gtk::Overlay {
                    gtk::Button {set_icon_name:"preferences-system-notifications-symbolic",set_has_frame:false,set_tooltip_text:Some("Notificações"),connect_clicked => ChatMsg::Notifications},
                    add_overlay = &gtk::Label {add_css_class:"papo-notification-count",set_halign:gtk::Align::End,set_valign:gtk::Align::Start,
                        #[watch] set_visible:model.unread_notifications>0,
                        #[watch] set_text:&format!("{}{}",model.unread_notifications.min(99),if model.unread_notifications>99||model.more_notifications{"+"}else{""}),
                    },
                },
                gtk::Button {set_icon_name:"system-users-symbolic",set_has_frame:false,set_tooltip_text:Some("Lista de Membros"),connect_clicked => ChatMsg::ToggleUserList},
                gtk::MenuButton {
                    set_icon_name:"view-more-symbolic",set_has_frame:false,set_tooltip_text:Some("Opções da conversa"),
                    #[wrap(Some)] set_popover = &gtk::Popover {
                        add_css_class:"papo-menu",
                        gtk::Box {
                            set_orientation:gtk::Orientation::Vertical,set_spacing:2,
                            gtk::Button {set_label:"Emojis",add_css_class:"flat",connect_clicked => ChatMsg::Action(ActionMsg::OpenEmojiManager)},
                            gtk::Button {set_label:"Preferências do canal",add_css_class:"flat",connect_clicked => ChatMsg::ChannelPreferences},
                        },
                    },
                },
            },

            gtk::Label {
                set_text:"Carregando mensagens anteriores…",add_css_class:"dim-label",add_css_class:"caption",
                #[watch] set_visible:model.history.loading()&&model.retry_cursor.is_some(),
            },
            gtk::Label {
                #[watch]
                set_visible: model.history_error.is_some(),
                #[watch]
                set_text: model.history_error.as_deref().unwrap_or(""),
                set_wrap: true,
                add_css_class: "error",
            },
            gtk::Button {
                #[watch]
                set_visible: model.history_error.is_some() && !model.history.loading(),
                set_label: "Tentar novamente",
                connect_clicked => ChatMsg::LoadOlder,
            },
            gtk::Button {
                set_label: "Cancelar busca da mensagem",
                #[watch]
                set_visible: model.actions.navigation.is_some(),
                connect_clicked => ChatMsg::Action(ActionMsg::CancelNavigation),
            },
            // ── Messages Area ────────────────────────────────────────────────
            #[name = "scrolled_window"]
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                set_vexpand: true,

                #[name = "messages_list"]
                gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    add_css_class: "papo-chat-history",
                },
            },

            // ── Reply Indicator Banner ───────────────────────────────────────
            gtk::Box {
                #[watch] set_visible:model.viewing_old,
                set_spacing:8,add_css_class:"papo-history-banner",
                gtk::Label {set_text:"Você está vendo mensagens antigas",set_hexpand:true,set_xalign:0.0,set_ellipsize:pango::EllipsizeMode::End},
                gtk::Button {set_label:"Mais recentes",set_tooltip_text:Some("Avançar para mensagens recentes"),add_css_class:"flat",connect_clicked => ChatMsg::Latest},
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,
                set_margin_start: 16,
                set_margin_end: 16,
                set_margin_top: 4,
                #[watch]
                set_visible: model.draft.reply.is_some(),
                add_css_class: "dim-label",

                gtk::Image {
                    set_icon_name: Some("mail-reply-sender-symbolic"),
                    set_pixel_size: 14,
                },

                gtk::Label {
                    #[watch]
                    set_text: &format!(
                        "Respondendo a: {}",
                        model.draft.reply.as_ref().and_then(|m| m.author_id.and_then(|id| model.users_map.get(&id))).map(|u| u.display_name()).unwrap_or("Mensagem")
                    ),
                    set_hexpand: true,
                    set_xalign: 0.0,
                    add_css_class: "caption",
                },

                gtk::Button {
                    set_icon_name: "window-close-symbolic",
                    set_has_frame: false,
                    set_tooltip_text: Some("Cancelar resposta"),
                    connect_clicked[sender] => move |_| {
                        sender.input(ChatMsg::SetReply(None));
                    },
                },
            },

            // ── Typing Indicator ─────────────────────────────────────────────
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 6,
                set_margin_start: 16,
                set_margin_end: 16,
                set_margin_top: 2,
                #[watch]
                set_visible: !model.typing_users.is_empty(),

                gtk::Spinner {
                    set_spinning: true,
                },

                gtk::Label {
                    #[watch]
                    set_text: &get_typing_text(&model.typing_users, &model.users_map),
                    add_css_class: "caption",
                    add_css_class: "dim-label",
                    set_xalign: 0.0,
                },
            },

            gtk::Label {
                #[watch]
                set_visible: model.draft.error.is_some(),
                #[watch]
                set_text: model.draft.error.as_deref().unwrap_or(""),
                set_wrap: true,
                add_css_class: "error",
            },

            #[name = "selected_files"]
            gtk::Box { set_orientation: gtk::Orientation::Vertical, set_spacing: 4 },
            gtk::Label {
                #[watch]
                set_visible: model.draft.is_sending(),
                #[watch]
                set_text: &if model.draft.files.is_empty(){"Enviando mensagem…".into()}else{format!("Enviando: {} / {}", format_file_size(model.transfers.progress as i64), format_file_size(model.draft.files.iter().map(|f| f.size).sum::<u64>() as i64))},
            },
            gtk::Button {
                set_label: "Cancelar envio",
                #[watch]
                set_visible: model.draft.is_sending(),
                connect_clicked => ChatMsg::Transfer(TransferMsg::Cancel),
            },
            // ── Message Composer ─────────────────────────────────────────────
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                add_css_class:"papo-composer",

                gtk::Button {
                    set_icon_name: "mail-attachment-symbolic",
                    set_tooltip_text: Some("Adicionar arquivos"),set_has_frame:false,
                    #[watch]
                    set_sensitive: model.access.send && model.access.attachments && !model.draft.is_sending(),
                    connect_clicked => ChatMsg::Transfer(TransferMsg::Choose),
                },
                #[name="mention_button"]
                gtk::MenuButton {
                    set_label: "@",
                    set_tooltip_text: Some("Mencionar membro (digite @ para filtrar)"),set_has_frame:false,
                    #[watch]
                    set_sensitive: model.access.send && !model.draft.is_sending(),
                    #[wrap(Some)]
                    #[name="mention_popover"]
                    set_popover = &gtk::Popover {set_autohide:false,
                        gtk::ScrolledWindow {set_hscrollbar_policy:gtk::PolicyType::Never,set_max_content_height:280,set_propagate_natural_height:true,
                        #[name = "mention_choices"]
                        gtk::Box { set_orientation: gtk::Orientation::Vertical, set_spacing: 4 },
                        },
                    },
                },
                #[name = "message_entry"]
                gtk::Entry {
                    #[watch]
                    set_placeholder_text: Some(&match model.active_channel.as_ref(){Some(c) if c.is_direct()=>format!("Mensagem para {}",c.name()),Some(c)=>format!("Conversar em #{}",c.name()),None=>"Selecione um canal para conversar".into()}),
                    set_hexpand: true,set_width_chars:1,
                    #[watch]
                    set_sensitive: model.active_channel.is_some() && model.access.send,
                    #[watch] set_editable:!model.draft.is_sending(),

                    connect_changed[sender] => move |entry| {
                        sender.input(ChatMsg::InputChanged(entry.text().to_string()));
                    },

                    connect_activate[sender] => move |_| {
                        sender.input(ChatMsg::SendClicked);
                    },
                },

                gtk::Button {
                    set_icon_name: "mail-send-symbolic",
                    add_css_class: "suggested-action",
                    #[watch]
                    set_sensitive: model.active_channel.is_some() && model.access.send && (!model.draft.text.trim().is_empty() || !model.draft.files.is_empty()) && !model.draft.is_sending(),
                    set_tooltip_text: Some("Enviar mensagem"),

                    connect_clicked[sender] => move |_| {
                        sender.input(ChatMsg::SendClicked);
                    },
                },
            },
        }
    }

    fn init(init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let mut model = ChatModel {
            active_channel: init.active_channel.clone().map(ConversationTarget::Channel),
            access: Default::default(),
            user_id: init.user_id,
            actions: Actions::new(init.api),
            transfers: Transfers::default(),
            config: Default::default(),
            history: {
                let mut history = History::default();
                if let Some(channel) = &init.active_channel { history.select(channel.id); }
                for message in init.messages { history.apply(Change::Upsert(message)); }
                history
            },
            draft: Draft::default(),
            saved_drafts: HashMap::new(),
            users_map: HashMap::new(),
            avatars: HashMap::new(),
            typing_users: HashMap::new(),
            last_typing_sent: None,
            has_more: false,
            history_error: None,
            retry_cursor: None,mention_auto_open:Default::default(),mention_manual:false,mention_signature:String::new(),mention_dismissed:false,older_requested:false,highlight_generation:0,viewport:Viewport::default(),rendered:HashMap::new(),viewing_old:false,unread_notifications:0,more_notifications:false,
        };

        let widgets = view_output!();
        crate::ui::style::close_popovers_on_action(&root);
        let auto=model.mention_auto_open.clone();let input=sender.input_sender().clone();widgets.mention_button.connect_active_notify(move |b|{if b.is_active()&&!auto.get(){let _=input.send(ChatMsg::OpenMentions);}});
        let input=sender.input_sender().clone();widgets.message_entry.connect_notify_local(Some("cursor-position"),move |_,_|{let _=input.send(ChatMsg::MentionCursorChanged);});
        let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);let input=sender.input_sender().clone();let weak=widgets.mention_popover.downgrade();keys.connect_key_pressed(move |_,key,_,_|{if let Some(p)=weak.upgrade(){if p.is_visible(){if key==gtk::gdk::Key::Escape{let _=input.send(ChatMsg::CloseMentions);return gtk::glib::Propagation::Stop;}if key==gtk::gdk::Key::Down{p.child_focus(gtk::DirectionType::TabForward);return gtk::glib::Propagation::Stop;}}}gtk::glib::Propagation::Proceed});widgets.message_entry.add_controller(keys);
        let focus=gtk::EventControllerFocus::new();let entry=widgets.message_entry.downgrade();let popover=widgets.mention_popover.downgrade();let button=widgets.mention_button.downgrade();let input=sender.input_sender().clone();focus.connect_leave(move |_|{let entry=entry.clone();let popover=popover.clone();let button=button.clone();let input=input.clone();gtk::glib::idle_add_local_once(move ||{let within=|w:&gtk::Widget|w.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN);if entry.upgrade().is_some_and(|w|within(w.upcast_ref()))||popover.upgrade().is_some_and(|w|within(w.upcast_ref()))||button.upgrade().is_some_and(|w|within(w.upcast_ref())){return;}let _=input.send(ChatMsg::CloseMentions);});});widgets.message_entry.add_controller(focus);
        model.viewport.connect(&widgets.scrolled_window,&widgets.messages_list,&sender);
        rebuild_messages_view(&widgets.messages_list, &mut model.rendered, &model.history.messages, &model.users_map, &model.avatars, &model.access, model.user_id, &model.actions, &model.transfers, &model.config, &sender);

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        let message=match message {
            ChatMsg::SetChannel(c)=>ChatMsg::SetTarget(ConversationTarget::Channel(c)),
            ChatMsg::SetDirect(d)=>ChatMsg::SetTarget(ConversationTarget::Direct(d)),other=>other,
        };
        let update_mentions=matches!(&message,ChatMsg::OpenMentions|ChatMsg::MentionCursorChanged|ChatMsg::CloseMentions|ChatMsg::SendClicked|ChatMsg::SendFinished{..}|ChatMsg::InputChanged(_)|ChatMsg::MentionSelected(_)|ChatMsg::SetUsers(_)|ChatMsg::SetAccess{..}|ChatMsg::SetTarget(_)|ChatMsg::ClearChannel);
        let update_files=matches!(&message,ChatMsg::SetTarget(_)|ChatMsg::ClearChannel|ChatMsg::SendClicked|ChatMsg::SendFinished{..}|ChatMsg::Transfer(TransferMsg::Selected{..}|TransferMsg::Remove(_)|TransferMsg::Cancel));
        let following=self.viewport.following();let mut position=None;let mut render_history=false;
        match message {
            ChatMsg::ViewportChanged(old)=>{self.viewing_old=old;if widgets.scrolled_window.vadjustment().value()<=120.0{sender.input(ChatMsg::AutoOlder);}},
            ChatMsg::AutoOlder=>{if self.has_more&&!self.history.loading()&&!self.older_requested&&self.history_error.is_none()&&self.actions.navigation.is_none()&&self.access.read&&widgets.scrolled_window.is_mapped()&&widgets.scrolled_window.vadjustment().value()<=120.0{sender.input(ChatMsg::LoadOlder);}},
            ChatMsg::ExpireHighlight{epoch,id,generation}=>{if self.actions.epoch==epoch&&self.highlight_generation==generation&&self.actions.highlight==Some(id){self.actions.highlight=None;render_history=true;}},
            ChatMsg::Latest=>{self.actions.navigation=None;self.actions.highlight=None;render_history=true;position=Some(Position::Bottom);},
            ChatMsg::CopyMessage(text)=>{if let Some(display)=gtk::gdk::Display::default(){display.clipboard().set_text(&text);}},
            ChatMsg::ContextMenu(id)=>{let mut row=widgets.messages_list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){show_context_menu(&w,None);break;}row=w.next_sibling();}},
            ChatMsg::ContextAt{id,x,y}=>{let mut row=widgets.messages_list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){show_context_menu(&w,Some((x,y)));break;}row=w.next_sibling();}},
            ChatMsg::Notifications=>{let _=sender.output(ChatOutput::Notifications);},
            ChatMsg::NotificationCount{count,more}=>{self.unread_notifications=count;self.more_notifications=more;},
            ChatMsg::OpenProfile(id) => {let _=sender.output(ChatOutput::OpenProfile(id));}
            ChatMsg::ChannelPreferences => {let _=sender.output(ChatOutput::ChannelPreferences);}
            ChatMsg::SetConfig(config) => {render_history=self.config.complete()!=config.complete();self.config=config.complete();},
            ChatMsg::Transfer(msg) => {render_history=matches!(&msg,TransferMsg::AttachmentVideo{..}|TransferMsg::File{..}|TransferMsg::Video{..}|TransferMsg::CloseVideo{..}|TransferMsg::ImageReady{..}|TransferMsg::Preview{..}|TransferMsg::Reveal{..}|TransferMsg::RetryImage(_));self.handle_transfer(msg,&sender,root);},
            ChatMsg::SetAccess { user_id, access } => {
                render_history=self.user_id!=Some(user_id)||self.access!=access;
                self.user_id = Some(user_id); self.access = access;
                if !self.access.read { self.actions.reset_channel(); self.transfers.reset(); }
                self.render_manager(&sender);
            }
            ChatMsg::Action(action) => {
                let highlight=self.actions.highlight;
                let navigate=matches!(&action,ActionMsg::Navigate(_));
                render_history=!matches!(&action,ActionMsg::HoverReactions(_)|ActionMsg::OpenPicker(_)|ActionMsg::OpenParticipants(_)|ActionMsg::OpenEdit(_)|ActionMsg::ConfirmDelete(_)|ActionMsg::OpenPins|ActionMsg::OpenEmojiManager);
                self.handle_action(action, &sender, root);
                if navigate||self.actions.highlight!=highlight{if let Some(id)=self.actions.highlight{position=Some(Position::Message(id));}}
            }
            ChatMsg::SetChannel(_) | ChatMsg::SetDirect(_) => {},
            ChatMsg::UpdateDirect(dm) => { if self.active_channel.as_ref().is_some_and(|c|c.id()==dm.id){self.active_channel=Some(ConversationTarget::Direct(dm));} },
            ChatMsg::SetTarget(channel) => {
                if self.active_channel.as_ref().map(|c| c.id()) != Some(channel.id()) {
                    if let Some(previous) = &self.active_channel {
                        self.saved_drafts.insert(previous.id(), std::mem::take(&mut self.draft));
                    }
                    self.draft = self.saved_drafts.remove(&channel.id()).unwrap_or_default();
                }
                self.actions.reset_channel();
                self.transfers.reset();
                self.has_more = false;self.older_requested=false;
                self.history_error = None;
                self.last_typing_sent = None;
                self.history.select(channel.id());position=Some(Position::Bottom);self.viewing_old=false;
                self.active_channel = Some(channel);
                sender.input(ChatMsg::Action(ActionMsg::Reload));
                self.typing_users.clear();
                widgets.message_entry.set_text(&self.draft.text);
                render_history=true;
            }
            ChatMsg::UpdateChannel(channel) => self.active_channel = Some(ConversationTarget::Channel(channel)),
            ChatMsg::ClearChannel => {
                self.actions.reset_channel();
                self.transfers.reset();
                self.access = Default::default();
                if let Some(channel) = self.active_channel.take() {
                    self.saved_drafts.insert(channel.id(), std::mem::take(&mut self.draft));
                }
                self.history = History::default();
                self.has_more = false;self.older_requested=false;
                self.history_error = None;
                self.typing_users.clear();
                widgets.message_entry.set_text("");
                render_history=true;position=Some(Position::Bottom);
            }
            ChatMsg::LoadOlder => {
                if !self.history.loading()&&!self.older_requested&&self.access.read {
                    if let Some(channel) = &self.active_channel {
                        let cursor = if self.history_error.is_some() { self.retry_cursor }
                            else { self.history.messages.first().map(MessageCursor::from) };
                        self.older_requested=true;let _ = sender.output(ChatOutput::LoadMoreMessages { channel_id: channel.id(), cursor });
                    }
                }
            }
            ChatMsg::ApplyChange(change) => {
                if let Change::Moderation(message, attachment, status) = &change { self.transfers.invalidate(*attachment); if status=="blocked" {self.transfers.invalidate_message(*message);} }
                if let Change::Preview(_, preview) = &change { self.transfers.invalidate(preview.id); }
                if let Change::RemovePreview(_, id) = &change { self.transfers.invalidate(*id); }
                self.history.apply(change);
                render_history=true;
            }
            ChatMsg::OperationError(error) => self.draft.error = Some(error),
            ChatMsg::BeginHistory { request_id, cursor } => {
                self.older_requested=false;self.history.begin(request_id); self.history_error = None; self.retry_cursor = cursor;
            }
            ChatMsg::HistoryLoaded { request_id, result, append } => {
                match result {
                    Ok(response) => {
                        let observed=response.messages.clone();
                        if self.history.finish(request_id, response.messages, append) {
                            let _=sender.output(ChatOutput::HistoryObserved{channel_id:response.channel_id,messages:observed,latest:!append});
                            self.has_more = response.has_more;
                            self.history_error = None;
                            let highlight=self.actions.highlight;
                            self.continue_navigation(&sender);
                            render_history=true;
                            if self.actions.highlight!=highlight{if let Some(id)=self.actions.highlight{position=Some(Position::Message(id));}}
                            else if !append && following && self.actions.navigation.is_none(){position=Some(Position::Bottom);}
                        }
                    }
                    Err(error) => {
                        if self.history.fail(request_id) { self.older_requested=false;self.history_error = Some(error); }
                    }
                }
            }
            ChatMsg::AddMessage(msg) => {
                if let Some(author) = msg.author_id { self.typing_users.remove(&author); }
                self.history.apply(Change::Upsert(msg));
                render_history=true;
                if following{position=Some(Position::Bottom);}
            }
            ChatMsg::DeleteMessage(id) => {
                self.transfers.invalidate_message(id);
                self.history.apply(Change::Delete(id));
                self.actions.pinned.retain(|m| m.id != id);
                sender.input(ChatMsg::Action(ActionMsg::PinEvent { id, pinned: false }));
                render_history=true;
            }
            ChatMsg::SetAvatars(avatars) => {
                render_history=self.avatars!=avatars;self.avatars=avatars;
            }
            ChatMsg::SetUsers(users) => {
                let users:HashMap<_,_>=users.into_iter().map(|u| (u.id, u)).collect();render_history=self.users_map.len()!=users.len()||users.iter().any(|(id,u)|self.users_map.get(id).is_none_or(|old|old.display_name()!=u.display_name()||old.roles!=u.roles));self.users_map=users;self.refresh_reaction_hints();
            }
            ChatMsg::UserTyping { user_id, is_typing } => {
                if is_typing { self.typing_users.insert(user_id, Instant::now()); }
                else { self.typing_users.remove(&user_id); }
            }
            ChatMsg::ClearStaleTyping => {
                let now = Instant::now();
                self.typing_users.retain(|_, last| now.duration_since(*last).as_secs() < 4);
            }
            ChatMsg::OpenMentions=>{self.mention_manual=true;self.mention_dismissed=false;},
            ChatMsg::MentionCursorChanged=>{},
            ChatMsg::CloseMentions=>{self.mention_dismissed=true;self.mention_manual=false;widgets.mention_popover.popdown();},
            ChatMsg::MentionSelected(id) => {
                if !self.access.send || self.draft.is_sending() || (id.is_none()&&!self.access.everyone){return;}
                self.mention_manual=false;self.mention_dismissed=true;widgets.mention_popover.popdown();
                let text=widgets.message_entry.text();let position=widgets.message_entry.position();
                let insertion=mentions::insert(&text,position,id).or_else(||{let prefix=format!("{}{}@",text,if text.is_empty()||text.ends_with(char::is_whitespace){""}else{" "});mentions::insert(&prefix,-1,id)});
                if let Some((text,caret))=insertion {self.draft.text=text.clone();widgets.message_entry.set_text(&text);widgets.message_entry.set_position(caret);widgets.message_entry.grab_focus();}
            }
            ChatMsg::InputChanged(txt) => {
                // Restoring/clearing a saved draft also emits GTK's changed signal.
                if self.draft.text == txt { return; }
                self.mention_manual=false;self.mention_dismissed=false;self.draft.text = txt;
                let should_send = self
                    .last_typing_sent
                    .map_or(true, |t| t.elapsed().as_secs() >= 3);
                if should_send {
                    if let Some(channel) = &self.active_channel {
                        self.last_typing_sent = Some(Instant::now());
                        let _ = sender.output(ChatOutput::UserTyping(channel.id()));
                    }
                }
            }
            ChatMsg::SetReply(msg) => {
                if !self.draft.is_sending() && (msg.is_none()||self.access.send) { self.draft.reply = msg;widgets.message_entry.grab_focus(); }
            }
            ChatMsg::SendClicked => {
                if !self.access.send { return; }
                if self.draft.text.chars().count() > 8192 {
                    self.draft.error = Some("A mensagem deve ter até 8192 caracteres.".into());
                    self.update_view(widgets, sender);
                    return;
                }
                if let Some(channel) = &self.active_channel {
                    if let Some(request_id) = self.draft.begin() {
                        if !self.draft.files.is_empty() && !self.access.attachments {self.draft.finish(request_id,Err("Sem permissão para anexos.".into()));}
                        else {widgets.mention_popover.popdown();self.mention_manual=false;widgets.message_entry.grab_focus();self.start_send(&sender, request_id, channel.id());}
                    }
                }
            }
            ChatMsg::SendFinished { request_id, channel_id, result } => {
                let outcome = result.as_ref().map(|_| ()).map_err(Clone::clone);
                if let Ok(message) = result {if self.active_channel.as_ref().is_some_and(|c|c.id()==channel_id){
                    self.history.apply(Change::Upsert(message));render_history=true;position=Some(Position::Bottom);
                }}
                if self.active_channel.as_ref().map(|c| c.id()) == Some(channel_id) {
                    if self.draft.finish(request_id, outcome) {
                        widgets.message_entry.set_text(&self.draft.text);
                    }
                } else if let Some(draft) = self.saved_drafts.get_mut(&channel_id) {
                    draft.finish(request_id, outcome);
                }
            }
            ChatMsg::ToggleNavigation => {let _=sender.output(ChatOutput::ToggleNavigation);},
            ChatMsg::ToggleUserList => {
                let _ = sender.output(ChatOutput::ToggleUserList);
            }
            ChatMsg::ReactionClicked { message_id, emoji_id, unicode } => {
                sender.input(ChatMsg::Action(ActionMsg::ToggleReaction { id: message_id, emoji_id, unicode }));
            }

        }
        if update_mentions{
            let entry_text=widgets.message_entry.text();let completion=mentions::completion(&entry_text,widgets.message_entry.position());
            if self.active_channel.is_some()&&self.access.send&&!self.draft.is_sending()&&!self.mention_dismissed&&(self.mention_manual||completion.is_some()){
            let filter=if self.mention_manual{String::new()}else{completion.as_ref().map(|(_,_,f)|f.to_lowercase()).unwrap_or_default()};
            let mut members:Vec<_>=self.users_map.values().filter(|u|!u.banned&&(u.display_name().to_lowercase().contains(&filter)||u.username.to_lowercase().contains(&filter))).collect();members.sort_by_key(|u|u.display_name().to_lowercase());
            let everyone=self.access.everyone&&"everyone".contains(&filter);
            let signature=format!("{filter}|{everyone}|{:?}",members.iter().take(50).map(|u|(u.id,u.display_name())).collect::<Vec<_>>());
            if self.mention_signature!=signature{
                self.mention_signature=signature;while let Some(child)=widgets.mention_choices.first_child(){widgets.mention_choices.remove(&child);}
                if members.is_empty()&&!everyone{let label=gtk::Label::new(Some("Nenhum membro encontrado"));label.set_margin_top(8);label.set_margin_bottom(8);label.add_css_class("dim-label");widgets.mention_choices.append(&label);}
                for u in members.into_iter().take(50){let button=gtk::Button::with_label(u.display_name());button.set_tooltip_text(Some(&format!("@{}",u.username)));let id=u.id;let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::MentionSelected(Some(id)));});widgets.mention_choices.append(&button);}
                if everyone{let button=gtk::Button::with_label("@everyone");let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::MentionSelected(None));});widgets.mention_choices.append(&button);}
            }
                if !widgets.mention_popover.is_visible()&&widgets.message_entry.is_mapped(){self.mention_auto_open.set(true);widgets.mention_button.popup();self.mention_auto_open.set(false);widgets.message_entry.grab_focus();}
            }else{widgets.mention_popover.popdown();self.mention_manual=false;}
        }
        if let Some(Position::Message(id))=position.as_ref(){
            self.highlight_generation=self.highlight_generation.wrapping_add(1);let generation=self.highlight_generation;let epoch=self.actions.epoch;let id=*id;let input=sender.input_sender().clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(2500),move ||{let _=input.send(ChatMsg::ExpireHighlight{epoch,id,generation});});
        }
        let anchor=if render_history{Some(self.viewport.capture(&widgets.scrolled_window,&widgets.messages_list))}else{None};
        if render_history{self.hydrate_media(&sender);}
        if render_history{rebuild_messages_view(&widgets.messages_list, &mut self.rendered, &self.history.messages, &self.users_map, &self.avatars, &self.access, self.user_id, &self.actions, &self.transfers, &self.config, &sender);self.viewport.restore(&widgets.scrolled_window,&widgets.messages_list,position.unwrap_or_else(||anchor.unwrap()),&sender);}
        else if let Some(position)=position{self.viewport.restore(&widgets.scrolled_window,&widgets.messages_list,position,&sender);}
        if update_files{
        while let Some(child)=widgets.selected_files.first_child(){widgets.selected_files.remove(&child);}
        for (index,file) in self.draft.files.iter().enumerate(){
            let button=gtk::Button::with_label(&format!("Remover {} ({})",file.name,format_file_size(file.size as i64)));
            button.set_sensitive(!self.draft.is_sending());let s=sender.clone();button.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::Remove(index))));widgets.selected_files.append(&button);
        }
        }
        self.update_view(widgets, sender);
    }
}

fn get_typing_text(typing_users: &HashMap<Uuid, Instant>, users_map: &HashMap<Uuid, UserSummary>) -> String {
    let names: Vec<&str> = typing_users
        .keys()
        .filter_map(|id| users_map.get(id).map(|u| u.display_name()))
        .collect();

    match names.len() {
        0 => String::new(),
        1 => {let phrase=typing_users.keys().find_map(|id|users_map.get(id).and_then(|u|u.typing.as_deref())).filter(|t|!t.is_empty()).unwrap_or("está digitando...");format!("{} {}",names[0],phrase)},
        2 => format!("{} e {} estão digitando...", names[0], names[1]),
        _ => "Várias pessoas estão digitando...".to_string(),
    }
}

fn rebuild_messages_view(
    list_box: &gtk::ListBox,
    rendered:&mut HashMap<String,(String,gtk::ListBoxRow)>,
    messages: &[Message],
    users_map: &HashMap<Uuid, UserSummary>,
    avatars: &HashMap<Uuid, gtk::gdk::Texture>,
    access: &crate::models::Access,
    user_id: Option<Uuid>,
    actions: &Actions,
    media: &Transfers,
    config: &crate::models::UserConfig,
    sender: &ComponentSender<ChatModel>,
) {
    if messages.is_empty() {
        rendered.clear();while let Some(child)=list_box.first_child(){list_box.remove(&child);}
        let empty_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty_box.set_valign(gtk::Align::Center);
        empty_box.set_halign(gtk::Align::Center);
        empty_box.set_margin_top(48);

        let icon = gtk::Image::from_icon_name("chat-message-new-symbolic");
        icon.set_pixel_size(48);
        icon.add_css_class("dim-label");
        empty_box.append(&icon);

        let label = gtk::Label::new(Some("Nenhuma mensagem por aqui ainda.\nComece a conversa!"));
        label.set_justify(gtk::Justification::Center);
        label.add_css_class("dim-label");
        empty_box.append(&label);

        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&empty_box));
        list_box.append(&row);
        return;
    }

    let by_id:HashMap<_,_>=messages.iter().map(|m|(m.id,m)).collect();
    let mut ordered=vec![];
    for (index,msg) in messages.iter().enumerate() {
        let previous=index.checked_sub(1).and_then(|i|messages.get(i));
        let date=msg.created_at.with_timezone(&chrono::Local).date_naive();
        if previous.is_none_or(|p|p.created_at.with_timezone(&chrono::Local).date_naive()!=date){
            let key=format!("date-{date}");if let Some((_,row))=rendered.get(&key){ordered.push(row.clone());}else{
            let divider=gtk::Box::new(gtk::Orientation::Horizontal,12);divider.add_css_class("papo-date-divider");
            for before in [true,false]{let line=gtk::Separator::new(gtk::Orientation::Horizontal);line.set_hexpand(true);line.set_valign(gtk::Align::Center);divider.append(&line);if before{let date=gtk::Label::new(Some(&date.format("%d/%m/%Y").to_string()));date.add_css_class("caption");date.add_css_class("dim-label");divider.append(&date);}}
            let row=gtk::ListBoxRow::new();row.set_widget_name(&key);row.set_activatable(false);row.set_selectable(false);row.set_child(Some(&divider));rendered.insert(key,(String::new(),row.clone()));ordered.push(row);
        }}
        let grouped=msg.author_id.is_some() && msg.reply_to.is_none() && msg.edited_at.is_none() && !actions.is_pinned(msg.id) && previous.is_some_and(|p|p.author_id==msg.author_id&&p.created_at.with_timezone(&chrono::Local).date_naive()==date&&(0..300).contains(&msg.created_at.signed_duration_since(p.created_at).num_seconds()));
        let key=format!("message-{}",msg.id);
        let author=msg.author_id.and_then(|id|users_map.get(&id));
        let text=msg.content.as_deref().map(|text|mentions::render(text,users_map));
        let reply_text=msg.reply_to.map(|id|mentions::render(by_id.get(&id).and_then(|m|m.content.as_deref()).unwrap_or("Abrir mensagem respondida"),users_map));
        let avatar=msg.author_id.and_then(|id|avatars.get(&id)).map_or(0,|t|t.as_ptr() as usize);
        let emoji_state:Vec<_>=msg.reactions.iter().flatten().filter_map(|r|r.emoji_id.map(|id|(id,actions.emoji_label(id),actions.textures.get(&id).map_or(0,|t|t.as_ptr() as usize)))).collect();
        let signature=format!("{msg:?}|{grouped}|{access:?}|{user_id:?}|{config:?}|{:?}|{avatar}|{}|{}|{}|{:?}|{:?}|{}",author.map(|u|(u.display_name(),&u.roles)),actions.is_pinned(msg.id),actions.busy.contains(&msg.id),actions.pins_ready,media.visual_state(msg),emoji_state,msg.created_at.with_timezone(&chrono::Local).format("%H:%M"));
        let inline=text::parts(msg.content.as_deref().unwrap_or(""),&actions.emojis);let inline_state:Vec<_>=inline.iter().filter_map(|p|if let text::Part::Emoji(id,name)=p{Some((*id,name,actions.textures.get(id).map_or(0,|t|t.as_ptr() as usize)))}else{None}).collect();
        let reply_author=msg.reply_to.and_then(|id|by_id.get(&id)).and_then(|m|m.author_id).and_then(|id|users_map.get(&id)).map(|u|u.display_name());
        let signature=format!("{signature}|{text:?}|{reply_text:?}|{reply_author:?}|{inline_state:?}");
        if let Some((old,row))=rendered.get(&key){if old==&signature{if actions.highlight==Some(msg.id){row.add_css_class("papo-message-highlight");}else{row.remove_css_class("papo-message-highlight");}ordered.push(row.clone());continue;}}
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(&key);
        row.set_focusable(true);
        if config.display.as_ref().and_then(|d|d.show_timestamps).unwrap_or(true){row.set_tooltip_text(Some(&msg.created_at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()));}
        if actions.highlight == Some(msg.id) { row.add_css_class("papo-message-highlight"); }
        row.set_activatable(false);
        row.set_selectable(false);

        let box_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let spacing=match config.display.as_ref().and_then(|d|d.message_density.as_ref()){Some(crate::models::MessageDensity::Compact)=>2,Some(crate::models::MessageDensity::Comfortable)=>12,_=>6};
        box_row.set_margin_top(if grouped{2}else{spacing+6});
        box_row.set_margin_bottom(spacing);
        box_row.set_margin_start(16);
        box_row.set_margin_end(16);

        // Author user avatar (falling back to generic symbolic icon)
        let author_user = msg.author_id.and_then(|id| users_map.get(&id));
        let avatar_widget = avatar_image(msg.author_id.and_then(|id| avatars.get(&id)), 36);
        avatar_widget.set_text(Some(author_user.map(|u|u.display_name()).unwrap_or("Usuário")));
        avatar_widget.set_valign(gtk::Align::Start);
        avatar_widget.set_visible(config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true));
        let avatar_button=gtk::Button::new();avatar_button.add_css_class("flat");avatar_button.add_css_class("papo-message-avatar");avatar_button.set_valign(gtk::Align::Start);avatar_button.set_tooltip_text(Some("Abrir perfil"));avatar_button.set_child(Some(&avatar_widget));
        if let Some(id)=msg.author_id{let s=sender.clone();avatar_button.connect_clicked(move |_|s.input(ChatMsg::OpenProfile(id)));}else{avatar_button.set_sensitive(false);}
        avatar_button.set_visible(avatar_widget.is_visible());
        if grouped&&avatar_widget.is_visible(){let gutter=gtk::Box::new(gtk::Orientation::Horizontal,0);gutter.set_width_request(36);box_row.append(&gutter);}else{box_row.append(&avatar_button);}

        // Message Content Body
        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content_box.set_hexpand(true);

        // Header (author name + time + edited tag)
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);header_box.add_css_class("papo-message-header");header_box.set_visible(!grouped);

        let author_name = author_user
            .map(|u| u.display_name())
            .unwrap_or("Usuário");

        let name_label = gtk::Label::new(Some(author_name));
        if let Some(roles)=author_user.and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&name_label,roles);}
        name_label.set_ellipsize(pango::EllipsizeMode::End);
        name_label.add_css_class("heading");
        let name_button=gtk::Button::new();name_button.add_css_class("flat");name_button.add_css_class("papo-author-button");name_button.set_child(Some(&name_label));
        if let Some(id)=msg.author_id{let s=sender.clone();name_button.connect_clicked(move |_|s.input(ChatMsg::OpenProfile(id)));}else{name_button.set_sensitive(false);}header_box.append(&name_button);

        let time_str = msg.created_at.with_timezone(&chrono::Local).format("%H:%M").to_string();
        let time_label = gtk::Label::new(Some(&time_str));
        time_label.add_css_class("caption");
        time_label.add_css_class("dim-label");
        time_label.set_visible(config.display.as_ref().and_then(|d|d.show_timestamps).unwrap_or(true));
        header_box.append(&time_label);

        if msg.edited_at.is_some() {
            let edited_label = gtk::Label::new(Some("(editado)"));
            edited_label.add_css_class("caption");
            edited_label.add_css_class("dim-label");
            header_box.append(&edited_label);
        }

        if actions.is_pinned(msg.id) { header_box.append(&gtk::Label::new(Some("📌 Fixada"))); }
        content_box.append(&header_box);
        if let Some(reply) = msg.reply_to {
            let channel_id = msg.channel_id; let s = sender.clone();
            let reply_button=gtk::Button::new();reply_button.add_css_class("flat");reply_button.add_css_class("papo-reply-reference");reply_button.set_halign(gtk::Align::Fill);reply_button.set_tooltip_text(Some("Ir para a mensagem respondida"));
            let line=gtk::Box::new(gtk::Orientation::Horizontal,6);line.append(&gtk::Image::from_icon_name("mail-reply-sender-symbolic"));
            let original=by_id.get(&reply);let name=gtk::Label::new(Some(original.and_then(|m|m.author_id).and_then(|id|users_map.get(&id)).map(|u|u.display_name()).unwrap_or("Mensagem")));name.add_css_class("heading");name.add_css_class("caption");name.set_ellipsize(pango::EllipsizeMode::End);name.set_max_width_chars(20);line.append(&name);
            let snippet=gtk::Label::new(Some(&text::excerpt(reply_text.as_deref().unwrap_or("Abrir mensagem respondida"))));snippet.add_css_class("caption");snippet.add_css_class("dim-label");snippet.set_xalign(0.0);snippet.set_hexpand(true);snippet.set_ellipsize(pango::EllipsizeMode::End);line.append(&snippet);reply_button.set_child(Some(&line));
            reply_button.connect_clicked(move |_|{let _=s.output(ChatOutput::Navigate{channel_id,message_id:reply});});content_box.append(&reply_button);
        }

        if let Some(raw)=&msg.content{content_box.append(&text::widget(raw,users_map,actions));}

        // Attachments Rendering
        if let Some(attachments) = &msg.attachments {
            for att in attachments {
                let att_card = transfers::attachment_widget(att, msg.id, media, sender);
                content_box.append(&att_card);
            }
        }

        // Link Previews Rendering
        if let Some(previews) = &msg.previews {
            for preview in previews {
                let preview_card = transfers::preview_widget(preview, msg.id, media, sender);
                content_box.append(&preview_card);
            }
        }

        // Reactions Flow / Pill List
        let reactions_box = gtk::FlowBox::new();
        reactions_box.set_halign(gtk::Align::Start);
        reactions_box.set_selection_mode(gtk::SelectionMode::None);
        reactions_box.set_min_children_per_line(1);
        reactions_box.set_max_children_per_line(8);
        reactions_box.set_column_spacing(6);
        reactions_box.set_row_spacing(4);
        reactions_box.set_margin_top(4);

        if let Some(reactions) = &msg.reactions {
            for reaction in reactions {
                let own = msg.user_reactions.as_ref().is_some_and(|items| items.iter().any(|r| r.emoji_id == reaction.emoji_id && r.unicode == reaction.unicode));
                let custom_label = reaction.emoji_id.map(|id| actions.emoji_label(id)).unwrap_or_else(|| "Emoji removido".into());
                let emoji = reaction.unicode.as_deref().unwrap_or(&custom_label);
                let count = reaction.count;
                let btn = gtk::Button::with_label(&format!("{} {}", emoji, count));
                btn.add_css_class("flat");
                btn.add_css_class("pill");btn.add_css_class("papo-reaction");
                if own { btn.add_css_class("own"); }
                btn.set_tooltip_text(Some(if own { "Remover minha reação" } else { "Adicionar minha reação" }));
                let hints=actions.reaction_hints.clone();let input=sender.input_sender().clone();let key=(msg.id,reaction.emoji_id,reaction.unicode.clone());
                let motion=gtk::EventControllerMotion::new();let hover_input=input.clone();let message=msg.id;
                motion.connect_enter(move |_,_,_|{let _=hover_input.send(ChatMsg::Action(ActionMsg::HoverReactions(message)));});btn.add_controller(motion);
                btn.connect_query_tooltip(move |_,_,_,_,tooltip|{if let Some(text)=hints.borrow().get(&key){tooltip.set_text(Some(text));}else{tooltip.set_text(Some("Carregando quem reagiu…"));let _=input.send(ChatMsg::Action(ActionMsg::HoverReactions(key.0)));}true});
                btn.set_sensitive(!actions.busy.contains(&msg.id) && (access.send || (access.read && own)));
                if let Some(texture) = reaction.emoji_id.and_then(|id| actions.textures.get(&id)) {
                    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                    let img = gtk::Image::from_paintable(Some(texture)); img.set_pixel_size(20); inner.append(&img);
                    inner.append(&gtk::Label::new(Some(&count.to_string()))); btn.set_child(Some(&inner));
                }

                let msg_id = msg.id;
                let unicode = reaction.unicode.clone();
                let emoji_id = reaction.emoji_id;
                let sender_clone = sender.clone();
                btn.connect_clicked(move |_| {
                    sender_clone.input(ChatMsg::ReactionClicked {
                        message_id: msg_id,
                        emoji_id,
                        unicode: unicode.clone(),
                    });
                });
                reactions_box.insert(&btn, -1);
            }
        }

        let toolbar=gtk::Box::new(gtk::Orientation::Horizontal,2);toolbar.add_css_class("papo-message-actions");
        let menu=gtk::MenuButton::new();menu.set_icon_name("view-more-symbolic");menu.add_css_class("flat");menu.set_tooltip_text(Some("Mais ações da mensagem"));
        let popover=gtk::Popover::new();popover.add_css_class("papo-menu");
        let message_actions=gtk::Box::new(gtk::Orientation::Vertical,2);popover.set_child(Some(&message_actions));menu.set_popover(Some(&popover));
        let msg_id = msg.id;
        let add_react_btn = gtk::Button::from_icon_name("face-smile-symbolic");
        add_react_btn.set_tooltip_text(Some("Adicionar reação"));
        add_react_btn.add_css_class("flat"); add_react_btn.set_sensitive(access.send && !actions.busy.contains(&msg.id));
        let s = sender.clone(); add_react_btn.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::OpenPicker(msg_id)))); toolbar.append(&add_react_btn);
        let add=gtk::Button::with_label("Adicionar reação");add.add_css_class("flat");add.set_sensitive(access.send&&!actions.busy.contains(&msg.id));let input=sender.input_sender().clone();add.connect_clicked(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::OpenPicker(msg_id)));});message_actions.append(&add);
        let who = gtk::Button::with_label("Quem reagiu"); who.add_css_class("flat"); who.set_sensitive(access.read);
        let s = sender.clone(); who.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::OpenParticipants(msg_id)))); message_actions.append(&who);
        let reply=gtk::Button::with_label("Responder");reply.add_css_class("flat");reply.set_sensitive(access.send);let s=sender.clone();let message=msg.clone();reply.connect_clicked(move |_|s.input(ChatMsg::SetReply(Some(message.clone()))));message_actions.append(&reply);
        if let Some(text)=text{let copy=gtk::Button::with_label("Copiar texto");copy.add_css_class("flat");let s=sender.clone();copy.connect_clicked(move |_|s.input(ChatMsg::CopyMessage(text.clone())));message_actions.append(&copy);}
        if let Some(user) = user_id {
            if access.can_edit(user, msg.author_id) {
                let edit = gtk::Button::with_label("Editar"); edit.add_css_class("flat"); edit.set_sensitive(!actions.busy.contains(&msg.id));
                let s = sender.clone(); let m = msg.clone(); edit.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::OpenEdit(m.clone())))); message_actions.append(&edit);
            }
            if access.can_delete(user, msg.author_id) {
                let delete = gtk::Button::with_label("Excluir"); delete.add_css_class("flat");delete.add_css_class("papo-destructive"); delete.set_sensitive(!actions.busy.contains(&msg.id));
                let s = sender.clone(); let m = msg.clone(); delete.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::ConfirmDelete(m.clone())))); message_actions.append(&delete);
            }
        }
        if access.pin {
            let pin = gtk::Button::with_label(if actions.is_pinned(msg.id) { "Desafixar" } else { "Fixar" }); pin.add_css_class("flat");
            pin.set_sensitive(actions.pins_ready && !actions.busy.contains(&msg.id));
            let s = sender.clone(); pin.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::Pin(msg_id)))); message_actions.append(&pin);
        }

        // Reply quick button
        let reply_btn = gtk::Button::from_icon_name("mail-reply-sender-symbolic");
        reply_btn.add_css_class("flat");
        reply_btn.add_css_class("dim-label");
        reply_btn.set_tooltip_text(Some("Responder"));
        reply_btn.set_sensitive(access.send);
        let msg_clone = msg.clone();
        let sender_reply = sender.clone();
        reply_btn.connect_clicked(move |_| {
            sender_reply.input(ChatMsg::SetReply(Some(msg_clone.clone())));
        });
        toolbar.append(&reply_btn);toolbar.append(&menu);

        reactions_box.set_visible(msg.reactions.as_ref().is_some_and(|r|!r.is_empty()));
        content_box.append(&reactions_box);

        box_row.append(&content_box);
        let overlay=gtk::Overlay::new();overlay.set_child(Some(&box_row));
        let revealer=gtk::Revealer::new();revealer.set_transition_type(gtk::RevealerTransitionType::Crossfade);revealer.set_transition_duration(100);revealer.set_halign(gtk::Align::End);revealer.set_valign(gtk::Align::Start);revealer.set_child(Some(&toolbar));overlay.add_overlay(&revealer);
        let hover=std::rc::Rc::new(std::cell::Cell::new(false));
        let motion=gtk::EventControllerMotion::new();let h=hover.clone();let reveal=revealer.clone();motion.connect_enter(move |_,_,_|{h.set(true);reveal.set_reveal_child(true);});
        let h=hover.clone();let reveal=revealer.clone();let r=row.downgrade();let p=popover.downgrade();motion.connect_leave(move |_|{h.set(false);if !r.upgrade().is_some_and(|r|r.has_focus()||r.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN))&&!p.upgrade().is_some_and(|p|p.is_visible()){reveal.set_reveal_child(false);}});row.add_controller(motion);
        let focus=gtk::EventControllerFocus::new();let reveal=revealer.clone();focus.connect_enter(move |_|reveal.set_reveal_child(true));let reveal=revealer.clone();let h=hover.clone();focus.connect_leave(move |_|{if !h.get(){reveal.set_reveal_child(false);}});row.add_controller(focus);
        let reveal=revealer.clone();let h=hover.clone();let r=row.downgrade();popover.connect_closed(move |popover|{popover.set_pointing_to(None);if !h.get()&&!r.upgrade().is_some_and(|r|r.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN)){reveal.set_reveal_child(false);}});
        crate::ui::style::close_popovers_on_action(&overlay);
        row.set_child(Some(&overlay));
        let click=gtk::GestureClick::new();click.set_button(3);click.set_propagation_phase(gtk::PropagationPhase::Capture);let s=sender.clone();click.connect_pressed(move |gesture,_,x,y|{gesture.set_state(gtk::EventSequenceState::Claimed);s.input(ChatMsg::ContextAt{id:msg_id,x,y});});row.add_controller(click);
        let hold=gtk::GestureLongPress::new();hold.set_touch_only(true);let s=sender.clone();hold.connect_pressed(move |_,x,y|s.input(ChatMsg::ContextAt{id:msg_id,x,y}));row.add_controller(hold);
        let keys=gtk::ShortcutController::new();keys.set_scope(gtk::ShortcutScope::Local);
        for trigger in ["Menu","<Shift>F10"]{let s=sender.clone();keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger),Some(gtk::CallbackAction::new(move |_,_|{s.input(ChatMsg::ContextMenu(msg_id));gtk::glib::Propagation::Stop}))));}row.add_controller(keys);
        rendered.insert(key,(signature,row.clone()));ordered.push(row);
    }
    let names:std::collections::HashSet<_>=ordered.iter().map(|r|r.widget_name().to_string()).collect();rendered.retain(|name,_|names.contains(name));
    let keep:std::collections::HashSet<_>=ordered.iter().cloned().collect();let mut child=list_box.first_child();
    while let Some(widget)=child{child=widget.next_sibling();if !widget.downcast_ref::<gtk::ListBoxRow>().is_some_and(|row|keep.contains(row)){list_box.remove(&widget);}}
    for (index,row) in ordered.iter().enumerate(){if row.parent().is_none(){list_box.insert(row,index as i32);}else if row.index()!=index as i32{list_box.remove(row);list_box.insert(row,index as i32);}}
}

fn show_context_menu(row:&gtk::Widget,point:Option<(f64,f64)>){
    fn visit(widget:&gtk::Widget,row:&gtk::Widget,point:Option<(f64,f64)>){
        if let Some(reveal)=widget.downcast_ref::<gtk::Revealer>(){reveal.set_reveal_child(true);}
        if let Some(menu)=widget.downcast_ref::<gtk::MenuButton>(){
            let weak=menu.downgrade();let row=row.downgrade();let frames=std::cell::Cell::new(0);
            widget.add_tick_callback(move |_,_|{frames.set(frames.get()+1);if frames.get()<2{return gtk::glib::ControlFlow::Continue;}
                if let Some(menu)=weak.upgrade(){if let Some(popover)=menu.popover(){
                    let anchor=point.and_then(|(x,y)|row.upgrade().and_then(|row|row.compute_point(&menu,&gtk::graphene::Point::new(x as f32,y as f32))));
                    popover.set_pointing_to(anchor.as_ref().map(|p|gtk::gdk::Rectangle::new(p.x().round() as i32,p.y().round() as i32,1,1)).as_ref());popover.set_position(gtk::PositionType::Bottom);
                }menu.popup();}gtk::glib::ControlFlow::Break});return;
        }
        let mut child=widget.first_child();while let Some(w)=child{visit(&w,row,point);child=w.next_sibling();}
    }
    row.grab_focus();visit(row,row,point);
}
