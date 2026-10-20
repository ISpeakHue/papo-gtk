//! Chat view component — displays channel header, message history, attachments, link previews, reactions, and composer.
//! EN-US: Core messaging interface managing virtualized message history, media attachments, link embeds, emoji/mention autocompletion, and composer.
//! PT-BR: Componente de visualização de chat — exibe cabeçalho do canal, histórico de mensagens virtualizado, anexos, prévias de links, reações, autocompletar de emojis/menções e editor de texto.

mod window;
mod navigation;
#[cfg(test)]pub(crate) use navigation::exercise as exercise_navigation;
mod cache;
mod surface_cache;
pub(crate) mod video;
pub(crate) mod rich;
mod embed_editor;
mod draft_preview;
#[cfg(test)]pub(crate) use draft_preview::exercise as exercise_draft_previews;
pub(crate) mod mentions;
#[cfg(test)]pub(crate) mod mention_background;
#[cfg(test)]pub(crate) mod polish_tests;
#[cfg(test)]pub(crate) mod multiline;
pub(crate) mod composer;
pub(crate) mod input;
pub(crate) use input::InputExt;

pub(crate) mod state;
pub(crate) mod actions;
pub(crate) mod transfers;
pub(crate) mod clipboard;
mod viewport;
mod text;
mod markdown;
mod syntax;
#[cfg(test)]pub(crate) use syntax::exercise as exercise_syntax;
mod moderation;
#[cfg(test)]pub(crate) use moderation::exercise as exercise_moderation;
mod emoji;
#[cfg(test)]pub(crate) use emoji::exercise_categories as exercise_emoji_categories;
mod emoji_details;
#[cfg(test)]pub(crate) use emoji_details::exercise as exercise_emoji_details;
use viewport::{Viewport,Position};
#[cfg(test)]pub(crate) mod bugs;
#[cfg(test)]pub(crate) mod bugs5;
#[cfg(test)]pub(crate) mod bugs6;
#[cfg(test)]pub(crate) mod bugs7;
#[cfg(test)]pub(crate) mod bugs8;
#[cfg(test)]pub(crate) mod bugs9;
#[cfg(test)]pub(crate) mod performance;
#[cfg(test)]mod display_frames;
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
use crate::models::{Attachment, Channel, ConversationTarget, DirectConversation, Embed, Message, MessageListResponse, MessageCursor, UserSummary};

/// EN-US: Initialization data for mounting the ChatModel component.
/// PT-BR: Dados de inicialização para montagem do componente ChatModel.
#[derive(Debug, Clone, Default)]
pub struct ChatInit {
    pub active_channel: Option<Channel>,
    pub messages: Vec<Message>,
    pub api: Option<crate::api::ApiClient>,
    pub user_id: Option<Uuid>,
    pub(crate) pinned:bool,
    pub(crate) pinned_inbox:Option<std::rc::Rc<std::cell::RefCell<Option<actions::pins::Snapshot>>>>,
}

/// EN-US: Central state for the Chat view, managing history, media transfers, drafts, and UI row recycling.
/// PT-BR: Estado central da tela de chat, gerenciando histórico, transferências de mídia, rascunhos e reciclagem de linhas de interface.
pub struct ChatModel {
    pub active_channel: Option<ConversationTarget>,
    server_name:String,

    history: History,
    conversations: cache::Conversations,
    surfaces:surface_cache::Surfaces,
    history_ready: bool,
    newer:navigation::Newer,
    reconcile_request:Option<Uuid>,
    edit_lookup:Option<Uuid>,
    media_render_pending: Option<Uuid>,
    moderation:moderation::Poll,
    #[cfg(test)]
    render_passes: usize,
    #[cfg(test)]media_hydrations:usize,
    initial_read:Option<viewport::ReadBoundary>,
    unread_start:Option<Uuid>,
    viewport:Viewport,
    rendered:HashMap<String,(String,gtk::ListBoxRow)>,
    render_window:window::RenderWindow,
    connection:Option<String>,
    visible_authors:std::collections::HashSet<Uuid>,
    viewing_old:bool,history_detached:bool,
    trim_when_idle:bool,
    emoji_visible:bool,emoji_manual:bool,emoji_dismissed:bool,emoji_signature:String,unicode_emojis:Vec<emoji::Choice>,
    mention_visible:bool,mention_manual:bool,mention_signature:String,mention_dismissed:bool,
    older_requested:bool,highlight_generation:u64,
    access: crate::models::Access,
    user_id: Option<Uuid>,
    actions: Actions,
    transfers: Transfers,
    config: crate::models::UserConfig,
    draft: Draft,
    draft_previews:draft_preview::Previews,
    composer:composer::Composer,
    saved_drafts: HashMap<Uuid, Draft>,
    pub users_map: HashMap<Uuid, UserSummary>,
    avatars: HashMap<Uuid, gtk::gdk::Texture>,
    pub typing_users: HashMap<Uuid, Instant>,
    pub last_typing_sent: Option<Instant>,
    has_more: bool,
    history_error: Option<String>,
    error_timer:Option<(Uuid,gtk::glib::SourceId)>,
    retry_cursor: Option<MessageCursor>,
    unread_notifications:usize,more_notifications:bool,
}

#[derive(Debug)]
pub enum ChatMsg {
    ExternalAvatars(Vec<crate::models::ExternalAuthor>),
    CheckModeration(Uuid),
    ModerationChecked{request:Uuid,result:anyhow::Result<MessageListResponse>},
    RenderMedia(Uuid),
    EditLatest(u64),
    LatestOwn{epoch:u64,request:Uuid,result:anyhow::Result<Option<Message>>},
    CancelInitialRead(u64),ViewportChanged(bool), MediaViewport, Latest, Page(i32),
    CopyMessage(String), ContextMenu(Uuid), ContextAt{ id:Uuid,x:f64,y:f64 },
    AutoOlder, ExpireHighlight{epoch:Uuid,id:Uuid,generation:u64},
    Search, Notifications, NotificationCount{count:usize,more:bool},
    Action(ActionMsg),
    JumpLoaded{epoch:Uuid,request:Uuid,id:Uuid,result:anyhow::Result<(MessageListResponse,bool)>},
    DraftPreviewReady{token:Uuid,image:Option<crate::media::PreparedImage>},
    PinnedSnapshot,
    Transfer(TransferMsg),
    SetConfig(crate::models::UserConfig),
    OpenProfile(Uuid),
    ChannelPreferences,
    SetAccess { user_id: Uuid, access: crate::models::Access },
    #[cfg(test)]
    SetChannel(Channel),
    SetTarget(ConversationTarget),
    SelectTarget { target:ConversationTarget,user_id:Uuid,access:crate::models::Access },
    UpdateDirect(DirectConversation),
    UpdateChannel(Channel),
    ClearChannel,
    LoadOlder, LoadNewer,
    NewerLoaded{epoch:Uuid,request:Uuid,result:anyhow::Result<MessageListResponse>},
    ApplyChange(Change),
    RequestHistory { channel_id: Uuid, cursor: Option<MessageCursor> },
    RetainConversations(Vec<Uuid>), ClearConversations, InvalidateConversation(Uuid), CacheChange(Change),
    #[cfg(test)]
    BeginHistory { request_id: Uuid, cursor: Option<MessageCursor> },
    RevalidateAvatars, Connection(Option<String>), OperationError(String), DismissError(Uuid), HistoryFailure{request_id:Uuid,error:anyhow::Error},
    HistoryLoaded { request_id: Uuid, result: Result<MessageListResponse, String>, append: bool },
    AddMessage(Message),

    DeleteMessage(Uuid),
    Resync,
    Reconciled{request:Uuid,result:anyhow::Result<MessageListResponse>},
    #[cfg(test)]
    BeginSnapshot(Uuid),
    SetUsers(Vec<UserSummary>),
    ServerName(String),
    SetAvatars(HashMap<Uuid, gtk::gdk::Texture>),
    UserTyping { user_id: Uuid, is_typing: bool },
    ClearStaleTyping,
    InputChanged(String),
    OpenEmojis,CloseEmojis,ComposerClosed(Uuid),EmojiSelected(String),InsertEmoji(String),UnicodePicker,
    OpenEmbeds,SaveEmbeds(Uuid),EmbedsClosed(Uuid),
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
    ExternalAvatarsChanged,
    EmojisChanged,
    ConversationLoaded,
    Search, Notifications,
    HistoryObserved { channel_id: Uuid, messages: Vec<crate::models::MessageObservation>, latest:bool },
    AuthorMetadata{channel:Uuid,ids:Vec<Uuid>},
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

            // Kept here for standalone conversations; the main layout places
            // this same header above the chat and member pane together.
            #[name="conversation_header"]
            adw::HeaderBar {
                set_widget_name:"conversation-header",
                #[watch] set_visible:!model.actions.pinned_view,
                pack_start = &gtk::Button {set_icon_name:"sidebar-show-symbolic",set_tooltip_text:Some("Canais e conversas (F9)"),connect_clicked => ChatMsg::ToggleNavigation},
                #[name="header_title_area"]
                #[wrap(Some)]
                set_title_widget = &gtk::Box {
                    #[name="conversation_title"]
                    adw::WindowTitle {
                        set_widget_name:"conversation-title",
                        #[watch] set_title:&match model.active_channel.as_ref(){Some(c) if c.is_direct()=>c.name().to_owned(),Some(c) if c.is_voice()=>format!("Voz · {}",c.name()),Some(c)=>format!("# {}",c.name()),None=>"Selecione um canal".into()},
                        #[watch] set_subtitle:model.active_channel.as_ref().and_then(|c|c.topic()).unwrap_or(""),
                        #[watch] set_tooltip_text:model.active_channel.as_ref().and_then(|c|c.topic()),
                    },
                    #[name="header_title_offset"]
                    gtk::Box {set_width_request:0},
                },
                #[name="header_actions"]
                pack_end = &gtk::Box {
                    set_spacing:6,
                    #[name="header_pins"]
                    gtk::Button {
                        set_icon_name:"view-pin-symbolic",set_tooltip_text:Some("Fixadas"),
                        #[watch] set_sensitive:model.access.read&&model.active_channel.as_ref().is_some_and(|c|!c.is_direct()),
                        connect_clicked => ChatMsg::Action(ActionMsg::OpenPins),
                    },
                    #[name="header_search"]
                    gtk::Button {set_icon_name:"system-search-symbolic",set_tooltip_text:Some("Buscar mensagens (Ctrl+K)"),connect_clicked => ChatMsg::Search},
                    gtk::Overlay {
                        gtk::Button {set_icon_name:"preferences-system-notifications-symbolic",set_tooltip_text:Some("Notificações"),connect_clicked => ChatMsg::Notifications},
                        add_overlay = &gtk::Label {add_css_class:"papo-notification-count",set_halign:gtk::Align::End,set_valign:gtk::Align::Start,
                            #[watch] set_visible:model.unread_notifications>0,
                            #[watch] set_text:&format!("{}{}",model.unread_notifications.min(99),if model.unread_notifications>99||model.more_notifications{"+"}else{""}),
                        },
                    },
                    #[name="header_members"]
                    gtk::Button {set_icon_name:"system-users-symbolic",set_tooltip_text:Some("Lista de Membros"),connect_clicked => ChatMsg::ToggleUserList},
                    gtk::MenuButton {
                        set_icon_name:"view-more-symbolic",set_tooltip_text:Some("Opções da conversa"),
                        #[wrap(Some)] set_popover = &gtk::Popover {
                            add_css_class:"papo-menu",
                            gtk::Box {
                                set_orientation:gtk::Orientation::Vertical,set_spacing:2,
                                #[name="compact_header_actions"]
                                gtk::Box {
                                    set_visible:false,set_orientation:gtk::Orientation::Vertical,set_spacing:2,
                                    gtk::Button {set_label:"Mensagens fixadas",add_css_class:"flat",#[watch] set_sensitive:model.access.read&&model.active_channel.as_ref().is_some_and(|c|!c.is_direct()),connect_clicked => ChatMsg::Action(ActionMsg::OpenPins)},
                                    gtk::Button {set_label:"Pesquisar mensagens",add_css_class:"flat",connect_clicked => ChatMsg::Search},
                                    gtk::Button {set_label:"Lista de membros",add_css_class:"flat",connect_clicked => ChatMsg::ToggleUserList},
                                    gtk::Separator {},
                                },
                                gtk::Button {set_label:"Emojis",add_css_class:"flat",connect_clicked => ChatMsg::Action(ActionMsg::OpenEmojiManager)},
                                gtk::Button {set_label:"Preferências do canal",add_css_class:"flat",connect_clicked => ChatMsg::ChannelPreferences},
                            },
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
            gtk::Overlay {
                set_vexpand: true,
                #[name = "scrolled_window"]
                #[wrap(Some)]
                set_child = &gtk::ScrolledWindow {
                    add_css_class: "papo-history-surface",
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_vscrollbar_policy: gtk::PolicyType::Automatic,
                    set_vexpand: true,
                    #[name = "messages_list"]
                    gtk::ListBox {
                        set_selection_mode: gtk::SelectionMode::None,
                        add_css_class: "papo-chat-history",
                    },
                },
                #[name = "history_cover"]
                add_overlay = &gtk::Box {
                    set_visible: false,
                    set_can_target: false,
                    set_halign: gtk::Align::Fill,
                    set_valign: gtk::Align::Fill,
                    set_widget_name: "history-cover",
                    add_css_class: "papo-history-cover",
                },
            },

            gtk::Label {
                set_text:"Carregando mensagens seguintes…",add_css_class:"dim-label",add_css_class:"caption",
                #[watch] set_visible:model.newer.request.is_some(),
            },
            gtk::Button {
                #[watch] set_visible:model.newer.error.is_some(),
                #[watch] set_tooltip_text:model.newer.error.as_deref(),
                set_label:"Tentar carregar mensagens seguintes novamente",
                connect_clicked => ChatMsg::LoadNewer,
            },

            // ── Reply Indicator Banner ───────────────────────────────────────
            gtk::Box {
                #[watch] set_visible:model.viewing_old&&!model.actions.pinned_view,
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
                add_css_class: "papo-reply-banner",

                gtk::Image {
                    set_icon_name: Some("mail-reply-sender-symbolic"),
                    set_pixel_size: 14,
                },

                gtk::Label {set_text:"Respondendo a:",add_css_class:"caption"},
                #[name="reply_author"]
                gtk::Label {
                    #[watch] set_text:model.draft.reply.as_ref().map(|m|m.sender_name(&model.users_map)).unwrap_or("Mensagem"),
                    set_xalign:0.0,set_max_width_chars:20,set_ellipsize:pango::EllipsizeMode::End,add_css_class:"heading",
                },

                #[name="reply_preview"]
                gtk::Box {set_orientation:gtk::Orientation::Vertical,set_hexpand:true},
                gtk::Button {
                    set_icon_name: "window-close-symbolic",
                    set_has_frame: false,
                    set_tooltip_text: Some("Cancelar resposta"),
                    connect_clicked[sender] => move |_| {
                        sender.input(ChatMsg::SetReply(None));
                    },
                },
            },

            gtk::Label {
                set_widget_name:"chat-connection-state",
                #[watch] set_visible:model.connection.is_some(),
                #[watch] set_text:model.connection.as_deref().unwrap_or(""),
                set_wrap:true,add_css_class:"caption",add_css_class:"dim-label",
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

            gtk::Label {
                #[watch] set_visible:model.transfers.notice.is_some(),
                #[watch] set_text:model.transfers.notice.as_ref().map_or("",|(_,text)|text.as_str()),
                add_css_class:"dim-label",set_wrap:true,
            },

            gtk::ScrolledWindow {
                set_widget_name:"draft-attachments",
                #[watch] set_visible:!model.draft.files.is_empty()&&!model.actions.pinned_view,
                set_hscrollbar_policy:gtk::PolicyType::Automatic,set_vscrollbar_policy:gtk::PolicyType::Never,
                set_propagate_natural_height:true,set_max_content_height:144,
                set_margin_start:16,set_margin_end:16,
                #[name = "selected_files"]
                gtk::Box { set_orientation: gtk::Orientation::Horizontal, set_spacing: 8 },
            },
            gtk::Label {
                #[watch] set_visible:!model.draft.embeds.is_empty(),
                #[watch] set_text:&format!("{} embed(s) personalizado(s)",model.draft.embeds.len()),
                add_css_class:"caption",set_xalign:0.0,
            },
            #[name="send_progress"]
            gtk::Label {
                set_widget_name:"send-progress",
                #[watch]
                set_visible: model.draft.is_sending()&&!model.draft.files.is_empty(),
                #[watch]
                set_text: &format!("Enviando: {} / {}", format_file_size(model.transfers.progress as i64), format_file_size(model.draft.files.iter().map(|f| f.size).sum::<u64>() as i64)),
            },
            gtk::Button {
                set_label: "Cancelar envio",
                #[watch]
                set_visible: model.draft.is_sending()&&!model.draft.files.is_empty(),
                connect_clicked => ChatMsg::Transfer(TransferMsg::Cancel),
            },
            #[name="mention_panel"]
            gtk::ScrolledWindow {
                set_widget_name: "mention-suggestions",
                #[watch] set_visible:model.mention_visible,
                set_hscrollbar_policy:gtk::PolicyType::Never,
                set_min_content_height:0,set_valign:gtk::Align::Start,set_vexpand:false,
                set_max_content_height:240,set_propagate_natural_height:true,
                add_css_class:"papo-mention-suggestions",
                add_css_class:"papo-member-suggestions",
                #[name="mention_choices"]
                gtk::Box {set_orientation:gtk::Orientation::Vertical,set_spacing:2,set_valign:gtk::Align::Start},
            },
            #[name="emoji_panel"]
            gtk::ScrolledWindow {
                set_widget_name:"emoji-suggestions",
                #[watch] set_visible:model.emoji_visible,
                set_hscrollbar_policy:gtk::PolicyType::Never,
                set_max_content_height:220,set_propagate_natural_height:true,
                add_css_class:"papo-mention-suggestions",
                #[name="emoji_choices"]
                gtk::Box{set_orientation:gtk::Orientation::Vertical,set_spacing:2},
            },
            // ── Message Composer ─────────────────────────────────────────────
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 10,
                add_css_class:"papo-composer",
                #[watch] set_visible:!model.actions.pinned_view,

                gtk::Button {
                    set_valign:gtk::Align::End,
                    set_icon_name: "mail-attachment-symbolic",
                    set_tooltip_text: Some("Adicionar arquivos"),set_has_frame:false,
                    #[watch]
                    set_sensitive: model.access.send && model.access.attachments && !model.draft.is_sending(),
                    connect_clicked => ChatMsg::Transfer(TransferMsg::Choose),
                },
                #[name="emoji_button"]
                gtk::Button {
                    set_valign:gtk::Align::End,
                    set_icon_name:"face-smile-symbolic",set_has_frame:false,
                    set_tooltip_text:Some("Emojis (digite : para filtrar)"),
                    #[watch] set_sensitive:model.access.send&&!model.draft.is_sending(),
                    connect_clicked=>ChatMsg::OpenEmojis,
                },
                gtk::Button {
                    set_valign:gtk::Align::End,
                    set_icon_name:"document-properties-symbolic",set_has_frame:false,set_tooltip_text:Some("Embeds personalizados"),
                    #[watch] set_sensitive:model.active_channel.is_some()&&model.access.send&&!model.draft.is_sending(),
                    connect_clicked=>ChatMsg::OpenEmbeds,
                },
                #[name="mention_button"]
                gtk::Button {
                    #[watch] set_visible: model.active_channel.as_ref().is_some_and(|c|!c.is_direct()),
                    set_valign:gtk::Align::End,
                    set_label: "@",
                    set_tooltip_text: Some("Mencionar membro (digite @ para filtrar)"),set_has_frame:false,
                    #[watch] set_sensitive: model.access.send && !model.draft.is_sending(),
                    connect_clicked => ChatMsg::OpenMentions,
                },
                gtk::Overlay {
                    set_hexpand:true,
                    #[name="composer_scroll"]
                    gtk::ScrolledWindow {
                        set_hexpand:true,set_vexpand:false,
                        set_height_request:40,
                        set_hscrollbar_policy:gtk::PolicyType::Never,
                        set_vscrollbar_policy:gtk::PolicyType::External,
                        #[name="message_entry"]
                        gtk::TextView {
                            set_widget_name:"message-composer",
                            add_css_class:"papo-composer-text",
                            set_wrap_mode:gtk::WrapMode::WordChar,
                            set_accepts_tab:false,
                            set_top_margin:10,set_bottom_margin:10,set_left_margin:4,set_right_margin:4,
                            set_input_purpose:gtk::InputPurpose::FreeForm,
                            set_input_hints:gtk::InputHints::NONE,
                            #[watch] set_sensitive:model.active_channel.is_some()&&model.access.send,
                            #[watch] set_editable:!model.draft.is_sending(),
                        },
                    },
                    add_overlay = composer_placeholder = &gtk::Label {
                        set_can_target:false,set_halign:gtk::Align::Fill,set_valign:gtk::Align::Start,
                        set_xalign:0.0,set_margin_start:4,set_margin_end:4,set_margin_top:10,
                        set_ellipsize:pango::EllipsizeMode::End,
                        add_css_class:"dim-label",
                        #[watch] set_visible:model.draft.text.is_empty()&&!model.composer.is_composing(),
                        #[watch] set_text:&match model.active_channel.as_ref(){Some(c) if c.is_direct()=>format!("Mensagem para {}",c.name()),Some(c) if c.is_voice()=>format!("Conversar em {}",c.name()),Some(c)=>format!("Conversar em #{}",c.name()),None=>"Selecione um canal para conversar".into()},
                    },
                },

                gtk::Button {
                    set_valign:gtk::Align::End,
                    set_icon_name: "mail-send-symbolic",
                    add_css_class: "suggested-action",
                    #[watch]
                    set_sensitive: model.active_channel.is_some() && model.access.send && (!model.draft.text.trim().is_empty() || !model.draft.files.is_empty()||!model.draft.embeds.is_empty()) && !model.draft.is_sending() && model.transfers.clipboard_request.is_none(),
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
            conversations: Default::default(),surfaces:Default::default(),
            server_name:"Servidor".into(),history_ready: !init.messages.is_empty(),reconcile_request:None,edit_lookup:None,render_window:Default::default(),connection:None,visible_authors:Default::default(),
            media_render_pending: None,
            moderation:Default::default(),
            #[cfg(test)]
            render_passes: 0,
            #[cfg(test)]media_hydrations:0,
            unread_start:None,
            initial_read:if init.pinned{None}else{init.active_channel.clone().map(ConversationTarget::Channel).as_ref().map(viewport::ReadBoundary::new)},
            emoji_visible:false,emoji_manual:false,emoji_dismissed:false,emoji_signature:String::new(),unicode_emojis:emoji::builtin(),
            active_channel: init.active_channel.clone().map(ConversationTarget::Channel),
            access: crate::models::Access{read:init.pinned,..Default::default()},
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
            draft: Draft::default(),draft_previews:Default::default(),
            composer:Default::default(),
            saved_drafts: HashMap::new(),
            users_map: HashMap::new(),
            avatars: HashMap::new(),
            typing_users: HashMap::new(),
            last_typing_sent: None,
            has_more: false,
            history_error: None,
            error_timer:None,
            newer:Default::default(),retry_cursor: None,mention_visible:false,mention_manual:false,mention_signature:String::new(),mention_dismissed:false,older_requested:false,highlight_generation:0,viewport:Viewport::default(),rendered:HashMap::new(),viewing_old:false,history_detached:false,trim_when_idle:false,unread_notifications:0,more_notifications:false,
        };

        model.actions.pinned_view=init.pinned;
        model.actions.pinned_inbox=init.pinned_inbox;
        model.transfers.opening=model.viewport.opening_flag();
        let widgets = view_output!();
        crate::performance::watch(&root);
        crate::ui::style::close_popovers_on_action(&root);
        let outside=gtk::GestureClick::new();outside.set_button(1);outside.set_propagation_phase(gtk::PropagationPhase::Capture);
        let input=sender.input_sender().clone();outside.connect_pressed(move |_,_,_,_|{let _=input.send(ChatMsg::Transfer(TransferMsg::DismissImages));});
        // Cover the whole application once rooted, including clicks on the same thumbnail.
        let controller=outside.clone();root.connect_root_notify(move |root|{if let Some(parent)=root.root().and_downcast::<gtk::Window>(){if controller.widget().is_none(){parent.add_controller(controller.clone());}}else if let Some(parent)=controller.widget(){parent.remove_controller(&controller);}});

        model.composer.connect(&widgets.message_entry,&sender);
        model.composer.set_emojis(&widgets.message_entry,&model.actions);
        input::configure_im(&widgets.message_entry);
        widgets.message_entry.bind_property("top-margin",&widgets.composer_placeholder,"margin-top").sync_create().build();
        if !model.actions.pinned_view{input::shortcuts::type_to_focus(&widgets.message_entry,&widgets.scrolled_window,model.composer.clone(),sender.input_sender().clone());}
        let placeholder=widgets.composer_placeholder.downgrade();let composing=model.composer.clone();
        widgets.message_entry.connect_preedit_changed(move |view,text|{
            if let Some(placeholder)=placeholder.upgrade(){placeholder.set_visible(text.is_empty()&&!composing.is_composing()&&view.buffer().char_count()==0);}
        });
        let input=sender.input_sender().clone();widgets.message_entry.buffer().connect_mark_set(move |_,_,mark|{if mark.name().as_deref()==Some("insert"){let _=input.send(ChatMsg::MentionCursorChanged);}});
        input::connect_size(&widgets.message_entry,&widgets.composer_scroll);
        let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        clipboard::connect(&widgets.message_entry,&sender,model.viewport.input_epoch_flag());
        let input=sender.input_sender().clone();let mentions=widgets.mention_panel.downgrade();let emojis=widgets.emoji_panel.downgrade();let entry=widgets.message_entry.downgrade();let epoch=model.viewport.input_epoch_flag();let composing=model.composer.clone();
        keys.connect_key_pressed(move |controller,key,_,mods|{
            let modifiers=mods & (gtk::gdk::ModifierType::SHIFT_MASK|gtk::gdk::ModifierType::CONTROL_MASK|gtk::gdk::ModifierType::ALT_MASK|gtk::gdk::ModifierType::SUPER_MASK|gtk::gdk::ModifierType::META_MASK|gtk::gdk::ModifierType::HYPER_MASK);
            let paste=(matches!(key,gtk::gdk::Key::v|gtk::gdk::Key::V)&&mods.contains(gtk::gdk::ModifierType::CONTROL_MASK)&&!mods.intersects(gtk::gdk::ModifierType::ALT_MASK|gtk::gdk::ModifierType::SUPER_MASK))
                || (key==gtk::gdk::Key::Insert&&mods==gtk::gdk::ModifierType::SHIFT_MASK);
            if paste&&entry.upgrade().is_some_and(|entry|clipboard::intercept(&entry,&input,epoch.get())){return gtk::glib::Propagation::Stop;}
            // Let native IM contexts commit/cancel candidates before shortcuts.
            if composing.is_composing(){return gtk::glib::Propagation::Proceed;}
            if matches!(key,gtk::gdk::Key::Return|gtk::gdk::Key::KP_Enter)&&mods.contains(gtk::gdk::ModifierType::SHIFT_MASK){return gtk::glib::Propagation::Proceed;}
            let selected=emojis.upgrade().filter(|p|p.is_visible()).map(|p|(p,true)).or_else(||mentions.upgrade().filter(|p|p.is_visible()).map(|p|(p,false)));
            // An engine can consume Enter/Tab without visible inline preedit.
            // Let it handle the real event before sending or autocompleting.
            if matches!(key,gtk::gdk::Key::Return|gtk::gdk::Key::KP_Enter)||key==gtk::gdk::Key::Tab&&selected.is_some(){
                if let (Some(view),Some(event))=(entry.upgrade(),controller.current_event()){
                    if view.im_context_filter_keypress(&event){return gtk::glib::Propagation::Stop;}
                }
            }
            let completing=selected.is_some();
            if let Some((panel,emoji))=selected{
                if key==gtk::gdk::Key::Escape{let _=input.send(if emoji{ChatMsg::CloseEmojis}else{ChatMsg::CloseMentions});return gtk::glib::Propagation::Stop;}
                if matches!(key,gtk::gdk::Key::Return|gtk::gdk::Key::KP_Enter|gtk::gdk::Key::Tab){if let Some(button)=first_mention_button(panel.upcast_ref()){button.emit_clicked();return gtk::glib::Propagation::Stop;}}
                if key==gtk::gdk::Key::Down{panel.child_focus(gtk::DirectionType::TabForward);return gtk::glib::Propagation::Stop;}
            }
            if matches!(key,gtk::gdk::Key::Return|gtk::gdk::Key::KP_Enter)&&modifiers.is_empty(){let _=input.send(ChatMsg::SendClicked);return gtk::glib::Propagation::Stop;}
            if !completing&&key==gtk::gdk::Key::Up&&modifiers.is_empty()&&entry.upgrade().is_some_and(|e|e.text().is_empty()){
                let _=input.send(ChatMsg::EditLatest(epoch.get()));return gtk::glib::Propagation::Stop;
            }gtk::glib::Propagation::Proceed
        });widgets.message_entry.add_controller(keys);
        let keys=gtk::EventControllerKey::new();let input=sender.input_sender().clone();let entry=widgets.message_entry.downgrade();
        let panel=widgets.mention_panel.downgrade();
        keys.connect_key_pressed(move |_,key,_,_|{
            if key==gtk::gdk::Key::Escape{let _=input.send(ChatMsg::CloseMentions);if let Some(e)=entry.upgrade(){e.grab_focus();}return gtk::glib::Propagation::Stop;}
            if key==gtk::gdk::Key::Up&&panel.upgrade().and_then(|p|first_mention_button(p.upcast_ref())).is_some_and(|b|b.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN)){if let Some(e)=entry.upgrade(){e.grab_focus();}return gtk::glib::Propagation::Stop;}
            gtk::glib::Propagation::Proceed
        });widgets.mention_panel.add_controller(keys);
        // Dismiss only after focus leaves the composer and completion panel.
        for target in [widgets.message_entry.clone().upcast::<gtk::Widget>(),widgets.mention_panel.clone().upcast(),widgets.mention_button.clone().upcast()]{
            let focus=gtk::EventControllerFocus::new();let entry=widgets.message_entry.downgrade();let panel=widgets.mention_panel.downgrade();let button=widgets.mention_button.downgrade();let input=sender.input_sender().clone();
            focus.connect_leave(move |_|{let entry=entry.clone();let panel=panel.clone();let button=button.clone();let input=input.clone();gtk::glib::idle_add_local_once(move ||{
                let within=|w:&gtk::Widget|w.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN);
                if entry.upgrade().is_some_and(|w|within(w.upcast_ref()))||panel.upgrade().is_some_and(|w|within(w.upcast_ref()))||button.upgrade().is_some_and(|w|within(w.upcast_ref())){return;}let _=input.send(ChatMsg::CloseMentions);
            });});target.add_controller(focus);
        }
        let keys=gtk::EventControllerKey::new();let input=sender.input_sender().clone();let entry=widgets.message_entry.downgrade();
        keys.connect_key_pressed(move |_,key,_,_|{if key==gtk::gdk::Key::Escape{let _=input.send(ChatMsg::CloseEmojis);if let Some(e)=entry.upgrade(){e.grab_focus();}gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});widgets.emoji_panel.add_controller(keys);
        for target in [widgets.message_entry.clone().upcast::<gtk::Widget>(),widgets.emoji_panel.clone().upcast(),widgets.emoji_button.clone().upcast()]{
            let focus=gtk::EventControllerFocus::new();let entry=widgets.message_entry.downgrade();let panel=widgets.emoji_panel.downgrade();let button=widgets.emoji_button.downgrade();let input=sender.input_sender().clone();
            focus.connect_leave(move |_|{let entry=entry.clone();let panel=panel.clone();let button=button.clone();let input=input.clone();gtk::glib::idle_add_local_once(move ||{let within=|w:&gtk::Widget|w.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN);if entry.upgrade().is_some_and(|w|within(w.upcast_ref()))||panel.upgrade().is_some_and(|w|within(w.upcast_ref()))||button.upgrade().is_some_and(|w|within(w.upcast_ref())){return;}let _=input.send(ChatMsg::CloseEmojis);});});target.add_controller(focus);
        }
        input::shortcuts::completion(&widgets.mention_panel,&widgets.message_entry);
        input::shortcuts::completion(&widgets.emoji_panel,&widgets.message_entry);
        widgets.message_entry.buffer().set_enable_undo(true);
        model.viewport.connect(&widgets.scrolled_window,&widgets.messages_list,&widgets.history_cover,&sender);
        let initial_position=if model.history_ready{model.initial_read.take().map_or(Position::Bottom,|boundary|boundary.position(&model.history.messages))}else{Position::Bottom};
        model.unread_start=if let Position::Unread(id)=initial_position{Some(id)}else{None};
        model.viewing_old=matches!(initial_position,Position::Unread(_)|Position::Message(_));
        model.render_window.select(&model.history.messages,&initial_position,0.0,600.0);
        rebuild_messages_view(&widgets.messages_list, &mut model.rendered, &model.history.messages, &model.render_window,None, model.unread_start, model.history_ready, &model.users_map, &model.avatars, &model.access, model.user_id, &model.actions, &model.transfers, &model.config, false, &sender);
        if model.active_channel.is_some(){
            model.viewport.begin_open(&widgets.scrolled_window);
            model.viewport.open_ready(model.history_ready);
            model.viewport.restore(&widgets.scrolled_window,&widgets.messages_list,initial_position,&sender);
        }

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        let _timing=crate::performance::timer("chat_update_us");
        let previous_error=self.draft.error.clone();
        let new_error=matches!(&message,ChatMsg::OperationError(_)|ChatMsg::SendFinished{result:Err(_),..}|ChatMsg::Transfer(TransferMsg::File{result:Err(_),..}|TransferMsg::Saved{result:Err(_),..}));
        let (message,selection)=match message {
            ChatMsg::SelectTarget{target,user_id,access}=>(ChatMsg::SetTarget(target),Some((user_id,access))),
            #[cfg(test)]
            ChatMsg::SetChannel(c)=>(ChatMsg::SetTarget(ConversationTarget::Channel(c)),None),
            ChatMsg::InsertEmoji(value)=>{self.emoji_manual=true;(ChatMsg::EmojiSelected(value),None)},other=>(other,None),
        };
        match &message {
            ChatMsg::ApplyChange(change)=>self.update_pin(change,&sender),
            ChatMsg::DeleteMessage(id)=>self.update_pin(&Change::Delete(*id),&sender),
            _=>{},
        }
        let incremental=matches!(&message,ChatMsg::AddMessage(_)|ChatMsg::ApplyChange(_)|ChatMsg::ModerationChecked{..}|ChatMsg::DeleteMessage(_)|ChatMsg::RenderMedia(_)|ChatMsg::MediaViewport|ChatMsg::SendFinished{..}|ChatMsg::Action(ActionMsg::ReactionsLoaded{..}|ActionMsg::ToggleReaction{..}));
        let update_moderation=matches!(&message,ChatMsg::CheckModeration(_)|ChatMsg::ModerationChecked{..}|ChatMsg::MediaViewport|ChatMsg::ViewportChanged(_)|ChatMsg::HistoryFailure{..});
        let hydrate=!matches!(&message,ChatMsg::RenderMedia(_));
        #[cfg(test)]
        let update_moderation=update_moderation||matches!(&message,ChatMsg::BeginHistory{..});
        let update_emojis=matches!(&message,ChatMsg::OpenEmojis|ChatMsg::CloseEmojis|ChatMsg::EmojiSelected(_)|ChatMsg::InputChanged(_)|ChatMsg::MentionCursorChanged|ChatMsg::SetTarget(_)|ChatMsg::ClearChannel|ChatMsg::SetAccess{..}|ChatMsg::SendClicked|ChatMsg::SendFinished{..}|ChatMsg::OpenMentions|ChatMsg::Action(ActionMsg::EmojiPage{..}|ActionMsg::EmojisLoaded{..}));
        let update_edit_mentions=matches!(&message,ChatMsg::SetUsers(_)|ChatMsg::SetAvatars(_)|ChatMsg::SetConfig(_)|ChatMsg::SetAccess{..});
        let update_mentions=matches!(&message,ChatMsg::OpenMentions|ChatMsg::MentionCursorChanged|ChatMsg::CloseMentions|ChatMsg::SendClicked|ChatMsg::SendFinished{..}|ChatMsg::InputChanged(_)|ChatMsg::MentionSelected(_)|ChatMsg::SetUsers(_)|ChatMsg::SetConfig(_)|ChatMsg::SetAccess{..}|ChatMsg::SetTarget(_)|ChatMsg::ClearChannel);
        let update_reply=matches!(&message,ChatMsg::SetReply(_)|ChatMsg::SetUsers(_)|ChatMsg::SetConfig(_)|ChatMsg::SetTarget(_)|ChatMsg::ClearChannel|ChatMsg::Action(ActionMsg::EmojiPage{..}|ActionMsg::EmojisLoaded{..}));
        let update_files=matches!(&message,ChatMsg::SetAccess{..}|ChatMsg::SetTarget(_)|ChatMsg::ClearChannel|ChatMsg::SendClicked|ChatMsg::SendFinished{..}|ChatMsg::Transfer(TransferMsg::Selected{..}|TransferMsg::PasteReady{..}|TransferMsg::Remove(_)|TransferMsg::Cancel));
        let following=self.viewport.following();let mut position=None;let mut render_history=false;let mut restore_avatars=false;
        // Keep inactive snapshots current without rebuilding the active chat.
        match &message {
            ChatMsg::ApplyChange(change) => {
                self.conversations.apply(change);self.transfers.encoded.apply(change);self.transfers.resident.apply(change);},
            ChatMsg::DeleteMessage(id) => {self.conversations.apply(&Change::Delete(*id));self.transfers.encoded.apply(&Change::Delete(*id));self.transfers.resident.apply(&Change::Delete(*id));},
            _ => {},
        }
        match message {
            ChatMsg::ExternalAvatars(authors)=>{if let Some(api)=&self.actions.api{self.transfers.external.request_authors(authors.iter().take(128),api,&sender);}},
            ChatMsg::CheckModeration(token)=>{if root.is_mapped(){self.check_moderation(token,&sender);}else{self.schedule_moderation(&sender,false);}},
            ChatMsg::ModerationChecked{request,result}=>{render_history=self.moderation_checked(request,result);},
            ChatMsg::RetainConversations(allowed) => {self.conversations.retain(&allowed);self.surfaces.retain(&allowed);self.transfers.encoded.retain_channels(&allowed);self.transfers.resident.retain_channels(&allowed);},
            ChatMsg::ClearConversations => {self.conversations.clear();self.surfaces.clear();self.transfers.encoded.clear();self.transfers.resident.clear();},
            ChatMsg::InvalidateConversation(id) => {self.conversations.invalidate_channel(id);self.surfaces.invalidate(id);self.transfers.encoded.invalidate_channel(id);self.transfers.resident.invalidate_channel(id);},
            ChatMsg::CacheChange(change) => {self.conversations.apply(&change);self.transfers.encoded.apply(&change);self.transfers.resident.apply(&change);},
            ChatMsg::RequestHistory { channel_id, cursor } => {
                if !self.access.read || self.active_channel.as_ref().map(|c|c.id()) != Some(channel_id) { return; }
                if cursor.is_none() && self.history_ready { sender.input(ChatMsg::Resync); }
                else if let Some(api) = self.actions.api.clone() {
                    let request_id = Uuid::new_v4();
                    self.older_requested=false; self.history.begin(request_id); self.history_error=None; self.retry_cursor=cursor;
                    let input=sender.input_sender().clone();
                    self.transfers.jobs.push(tokio::spawn(async move {
                        let result=api.list_messages(channel_id,cursor).await.map_err(|e| {
                            let message=e.to_string();
                            if crate::api::is_session_error(&e) || crate::api::is_permission_error(&e) { let _=input.send(ChatMsg::HistoryFailure{request_id,error:e}); }
                            message
                        });
                        let _=input.send(ChatMsg::HistoryLoaded{request_id,result,append:cursor.is_some()});
                    }));
                }
            }
            ChatMsg::OpenEmbeds=>{
                if self.active_channel.is_some()&&self.access.send&&!self.draft.is_sending(){
                    if let Some((window,_,_))=&self.actions.embed_editor{window.present();return;}
                    let (window,body)=actions::window(root,"Embeds personalizados");window.set_default_size(500,650);
                    let editor=embed_editor::Editors::new(self.draft.embeds.clone());body.append(&gtk::ScrolledWindow::builder().vexpand(true).child(&editor.root).build());
                    let error=gtk::Label::new(None);error.set_wrap(true);error.add_css_class("error");body.append(&error);
                    let save=gtk::Button::with_label("Aplicar embeds");save.add_css_class("suggested-action");let input=sender.input_sender().clone();let epoch=self.actions.epoch;save.connect_clicked(move |_|{let _=input.send(ChatMsg::SaveEmbeds(epoch));});body.append(&save);
                    let input=sender.input_sender().clone();window.connect_close_request(move |_|{let _=input.send(ChatMsg::EmbedsClosed(epoch));gtk::glib::Propagation::Proceed});
                    let keys=gtk::EventControllerKey::new();let weak=window.downgrade();keys.connect_key_pressed(move |_,key,_,_|{if key==gtk::gdk::Key::Escape{if let Some(window)=weak.upgrade(){window.close();}gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});window.add_controller(keys);
                    self.actions.embed_editor=Some((window.clone(),editor,error));window.present();
                }
            },
            ChatMsg::SaveEmbeds(epoch)=>{
                if epoch==self.actions.epoch&&self.access.send&&!self.draft.is_sending(){if let Some((_,editor,error))=&self.actions.embed_editor{match editor.read(){Ok(embeds)=>{self.draft.embeds=embeds;if let Some((window,_,_))=self.actions.embed_editor.take(){window.close();}if let Some(w)=root.root().and_downcast::<gtk::Window>(){w.present();}widgets.message_entry.grab_focus();},Err(e)=>error.set_text(&e.to_string())}}}
            },
            ChatMsg::EmbedsClosed(epoch)=>{if epoch==self.actions.epoch&&self.actions.embed_editor.as_ref().is_some_and(|(w,_,_)|!w.is_visible()){self.actions.embed_editor.take();if let Some(w)=root.root().and_downcast::<gtk::Window>(){w.present();}widgets.message_entry.grab_focus();}},
            ChatMsg::OpenEmojis=>{if self.access.send&&!self.draft.is_sending(){self.emoji_manual=true;self.emoji_visible=false;self.emoji_dismissed=true;self.mention_visible=false;self.mention_manual=false;self.mention_dismissed=true;self.open_composer_picker(root,&sender);}},
            ChatMsg::ComposerClosed(epoch)=>{
                if epoch==self.actions.epoch&&self.actions.composer_picker.as_ref().is_some_and(|(window,_)|!window.is_visible()){
                    self.actions.composer_picker.take();self.emoji_visible=false;self.emoji_manual=false;self.emoji_dismissed=true;
                    if let Some(window)=root.root().and_downcast::<gtk::Window>(){window.present();}
                    widgets.message_entry.grab_focus();
                }
            },
            ChatMsg::CloseEmojis=>{self.emoji_visible=false;self.emoji_manual=false;self.emoji_dismissed=true;},
            ChatMsg::UnicodePicker=>{
                self.emoji_manual=false;self.emoji_visible=false;
                let picker=gtk::EmojiChooser::new();picker.set_parent(&widgets.emoji_button);let input=sender.input_sender().clone();
                picker.connect_emoji_picked(move |_,emoji|{let _=input.send(ChatMsg::EmojiSelected(emoji.into()));});picker.connect_closed(|p|p.unparent());picker.popup();
            },
            ChatMsg::EmojiSelected(value)=>{if !self.access.send||self.draft.is_sending(){return;}let replace=!self.actions.composer_picker.as_ref().is_some_and(|(w,_)|w.is_visible())&&!self.emoji_manual&&emoji::completion(&widgets.message_entry.text(),widgets.message_entry.position()).is_some();
                let caret=self.composer.emoji(&widgets.message_entry,&value,replace);self.emoji_dismissed=true;self.emoji_manual=false;self.emoji_visible=false;self.draft.text=self.composer.wire_text();self.composer.focus_after_completion(&widgets.message_entry,caret);
            },
            ChatMsg::EditLatest(epoch)=>{
                if epoch==self.viewport.input_epoch()&&self.draft.text.is_empty()&&!self.draft.is_sending()&&self.draft.files.is_empty()&&self.draft.embeds.is_empty()&&self.draft.reply.is_none()&&!self.emoji_visible&&!self.mention_visible{
                    if let Some(user)=self.user_id.filter(|_|self.access.read){
                        if let Some(message)=self.history.messages.iter().rev().find(|m|m.author_id==Some(user)&&self.access.can_edit(user,m.author_id)).cloned(){sender.input(ChatMsg::Action(ActionMsg::OpenEdit(message)));}
                        else if self.edit_lookup.is_none(){if let (Some(api),Some(channel))=(self.actions.api.clone(),self.active_channel.as_ref().map(|c|c.id())){
                            let request=Uuid::new_v4();self.edit_lookup=Some(request);let output=sender.input_sender().clone();
                            self.transfers.jobs.push(tokio::spawn(async move{let result=api.latest_own_message(channel,user).await;let _=output.send(ChatMsg::LatestOwn{epoch,request,result});}));
                        }}
                    }
                }
            },
            ChatMsg::LatestOwn{epoch,request,result}=>{
                if self.edit_lookup!=Some(request){return;}self.edit_lookup=None;
                if epoch!=self.viewport.input_epoch()||!self.draft.text.is_empty()||self.draft.is_sending()||!self.draft.files.is_empty()||!self.draft.embeds.is_empty()||self.draft.reply.is_some()||!self.access.read{return;}
                match result{
                    Ok(Some(message)) if self.user_id==message.author_id&&self.active_channel.as_ref().is_some_and(|c|c.id()==message.channel_id)&&!self.history.is_deleted(message.id)=>{sender.input(ChatMsg::Action(ActionMsg::OpenEdit(message)));},
                    Ok(None)=>self.draft.error=Some("Nenhuma mensagem sua neste canal para editar.".into()),
                    Err(e)=>self.draft.error=Some(format!("Não foi possível localizar sua mensagem: {e}")),_=>{}
                }
            },
            ChatMsg::CancelInitialRead(epoch)=>{if self.viewport.input_epoch()==epoch{self.initial_read=None;self.viewport.cancel_open(&widgets.scrolled_window);}},
            ChatMsg::ViewportChanged(old)=>{if self.viewing_old&&!old{self.trim_when_idle=true;}if old{self.trim_when_idle=false;}self.viewing_old=old||self.history_detached;if widgets.scrolled_window.vadjustment().value()<=120.0{sender.input(ChatMsg::AutoOlder);}},
            ChatMsg::MediaViewport=>{
                let next=widgets.scrolled_window.vadjustment();
                if self.newer.has_more&&self.newer.error.is_none()&&widgets.scrolled_window.is_mapped()&&next.upper()-next.page_size()-next.value()<=240.0{sender.input(ChatMsg::LoadNewer);}
                let adj=widgets.scrolled_window.vadjustment();let mut scope=std::collections::HashSet::new();let mut priority=HashMap::new();
                let mut row=widgets.messages_list.first_child();
                while let Some(w)=row{row=w.next_sibling();if let Some(id)=w.widget_name().strip_prefix("message-").and_then(|s|Uuid::parse_str(s).ok()){
                    if let Some(b)=w.compute_bounds(&widgets.messages_list){if f64::from(b.y()+b.height())>=adj.value()-adj.page_size()&&f64::from(b.y())<=adj.value()+2.0*adj.page_size(){scope.insert(id);let distance=(f64::from(b.y())-adj.value()-adj.page_size()/2.0).abs();priority.insert(id,distance as u64);}}
                }}
                if !scope.is_empty(){self.transfers.scope=scope;self.transfers.priority=priority;}
                let authors=self.avatar_authors();if authors!=self.visible_authors{self.visible_authors=authors.clone();if let Some(channel)=&self.active_channel{let _=sender.output(ChatOutput::AuthorMetadata{channel:channel.id(),ids:authors.into_iter().collect()});}}
                if self.render_window.needs_shift(&widgets.messages_list,&widgets.scrolled_window,&self.history.messages){render_history=true;}
                if self.viewport.scrolling(){
                    // Fetch and decode ahead while moving; only mounting and
                    // resizing rows waits for idle, preserving the scroll anchor.
                    if !render_history{self.hydrate_media(&sender);}
                    self.viewport.media_when_idle(&sender);
                }else{
                    render_history|=self.transfers.release_distant(&self.history.messages);
                    if !render_history{self.hydrate_media(&sender);}
                    render_history|=!self.transfers.dirty.is_empty();
                    if let Some(token)=self.media_render_pending{sender.input(ChatMsg::RenderMedia(token));}
                }
            },
            ChatMsg::AutoOlder=>{if self.initial_read.is_none()&&self.has_more&&!self.history.loading()&&!self.older_requested&&self.history_error.is_none()&&self.actions.navigation.is_none()&&self.access.read&&widgets.scrolled_window.is_mapped()&&widgets.scrolled_window.vadjustment().value()<=120.0{sender.input(ChatMsg::LoadOlder);}},
            ChatMsg::ExpireHighlight{epoch,id,generation}=>{if self.actions.epoch==epoch&&self.highlight_generation==generation&&self.actions.highlight==Some(id){self.actions.highlight=None;render_history=true;}},
            ChatMsg::Page(direction)=>{if self.access.read{self.initial_read=None;self.viewport.page(&widgets.scrolled_window,direction);}},
            ChatMsg::Latest=>{
                self.restore_recent_history(&sender);
                self.trim_when_idle=true;self.initial_read=None;self.actions.navigation=None;self.actions.highlight=None;render_history=true;position=Some(Position::Bottom);},
            ChatMsg::CopyMessage(text)=>{if let Some(display)=gtk::gdk::Display::default(){display.clipboard().set_text(&text);}},
            ChatMsg::ContextMenu(id)=>{let mut row=widgets.messages_list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){show_context_menu(&w,None);break;}row=w.next_sibling();}},
            ChatMsg::ContextAt{id,x,y}=>{let mut row=widgets.messages_list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){
                if !self.access.read||!transfers::attachment_context(&w,id,x,y,&self.history.messages,&self.transfers,&sender){show_context_menu(&w,Some((x,y)));}break;
            }row=w.next_sibling();}},
            ChatMsg::Search=>{let _=sender.output(ChatOutput::Search);},
            ChatMsg::Notifications=>{let _=sender.output(ChatOutput::Notifications);},
            ChatMsg::NotificationCount{count,more}=>{self.unread_notifications=count;self.more_notifications=more;},
            ChatMsg::OpenProfile(id) => {let _=sender.output(ChatOutput::OpenProfile(id));}
            ChatMsg::ChannelPreferences => {let _=sender.output(ChatOutput::ChannelPreferences);}
            ChatMsg::DraftPreviewReady{token,image}=>self.draft_previews.finish(token,image),
            ChatMsg::PinnedSnapshot=>{let snapshot=self.actions.pinned_inbox.as_ref().and_then(|inbox|inbox.borrow_mut().take());if let Some(snapshot)=snapshot{snapshot.apply(self);render_history=true;}},
            ChatMsg::SetConfig(config) => {render_history=self.config.complete()!=config.complete();self.config=config.complete();self.render_pins(&sender);},
            ChatMsg::RenderMedia(token) => {
                if self.media_render_pending==Some(token){if self.viewport.scrolling(){self.viewport.media_when_idle(&sender);}else{self.media_render_pending=None;render_history=true;}}
            },
            ChatMsg::Transfer(msg) => {
                let background=matches!(&msg,TransferMsg::Bytes{..}|TransferMsg::FirstFrame{..}|TransferMsg::Decoded{..}|TransferMsg::ImageReady{..}|TransferMsg::AnimationReady{..}|TransferMsg::AuthorReady{..}|TransferMsg::ExternalAvatar{..}|TransferMsg::InvalidateWebhookAvatar(_)|TransferMsg::Preview{..});
                render_history=matches!(&msg,TransferMsg::InvalidateWebhookAvatar(_)|TransferMsg::AttachmentVideo{..}|TransferMsg::File{..}|TransferMsg::Video{..}|TransferMsg::CloseVideo{..}|TransferMsg::Reveal{..}|TransferMsg::RetryImage(_));
                let revision=self.transfers.revision;
                self.handle_transfer(msg,&sender,root);
                // Several decodes may complete together. Reconcile once per
                // batch, and ignore rejected/stale completions entirely.
                if background && self.transfers.revision!=revision && self.media_render_pending.is_none(){
                    let token=Uuid::new_v4();self.media_render_pending=Some(token);let input=sender.input_sender().clone();
                    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(16),move ||{let _=input.send(ChatMsg::RenderMedia(token));});
                }
            },
            ChatMsg::SetAccess { user_id, access } => {
                render_history=self.user_id!=Some(user_id)||self.access!=access;
                let gained_read=!self.access.read&&access.read;
                if self.user_id!=Some(user_id) { self.conversations.clear();self.surfaces.clear();self.transfers.reset(); }
                self.user_id = Some(user_id); self.access = access;
                if !self.access.read {
                    self.history_detached=false;self.newer=Default::default();self.initial_read=None;self.viewport.cancel_open(&widgets.scrolled_window);
                    self.conversations.clear();self.surfaces.clear();
                    self.actions.reset_channel(); self.transfers.reset();self.edit_lookup=None;self.reconcile_request=None;
                    if let Some(channel)=self.active_channel.as_ref(){self.history.select(channel.id());}
                    self.history_ready=false;
                }else if gained_read&&self.active_channel.is_some()&&!self.history_ready&&!self.history.loading(){sender.input(ChatMsg::Resync);}
                self.render_manager(&sender);
            }
            ChatMsg::Action(action) => {
                let highlight=self.actions.highlight;
                let navigate=matches!(&action,ActionMsg::Navigate(_));
                render_history=!matches!(&action,ActionMsg::Reconcile(_)|ActionMsg::Reload|ActionMsg::ReloadEmojis|ActionMsg::HoverReactions(_)|ActionMsg::OpenPicker(_)|ActionMsg::OpenParticipants(_)|ActionMsg::SelectParticipants{..}|ActionMsg::MoreParticipants(_)|ActionMsg::ParticipantsClosed(_)|ActionMsg::OpenEdit(_)|ActionMsg::EditMentions(_)|ActionMsg::OpenEditMentions(_)|ActionMsg::DismissEditMentions(_)|ActionMsg::EditMentionSelected(..)|ActionMsg::OpenEditEmojis(_)|ActionMsg::CloseEditEmojis(_)|ActionMsg::EditEmojiSelected(..)|ActionMsg::ConfirmDelete(_)|ActionMsg::OpenPins|ActionMsg::OpenEmojiManager);
                if let ActionMsg::ReactionsLoaded{id,..}|ActionMsg::ToggleReaction{id,..}=&action{self.history.mark_dirty(*id);}
                let emoji_state=matches!(&action,ActionMsg::EmojiPage{..}|ActionMsg::EmojisLoaded{..}).then(||(self.actions.emoji_names.clone(),self.actions.textures.len()));
                self.handle_action(action, &sender, root);
                if let Some((names,textures))=emoji_state {
                    if names==self.actions.emoji_names && textures==self.actions.textures.len() { render_history=false; }
                    else {
                        self.refresh_composer_emojis(&widgets.message_entry);
                        self.refresh_edit_mentions();let _=sender.output(ChatOutput::EmojisChanged);
                    }
                }
                if navigate{self.initial_read=None;}
                if navigate||self.actions.highlight!=highlight{if let Some(id)=self.actions.highlight{position=Some(Position::Message(id));}}
            }
            #[cfg(test)]
            ChatMsg::SetChannel(_) => {},
            ChatMsg::SelectTarget{..} | ChatMsg::InsertEmoji(_) => {},
            ChatMsg::UpdateDirect(dm) => { if self.active_channel.as_ref().is_some_and(|c|c.id()==dm.id){self.active_channel=Some(ConversationTarget::Direct(dm));} },
            ChatMsg::SetTarget(channel) => {
                self.trim_when_idle=false;
                let recent:std::collections::HashSet<_>=self.history.messages.iter().rev().take(cache::MESSAGE_LIMIT).map(|m|m.id).collect();
                let previous_surface=self.active_channel.as_ref().filter(|c|c.id()!=channel.id()&&self.history_ready&&self.access.read&&!self.history_detached).map(|c|c.id());
                let same_user=selection.as_ref().is_none_or(|(id,_)|self.user_id==Some(*id));
                if !same_user{self.conversations.clear();self.surfaces.clear();}
                if same_user && self.history_ready && self.access.read && !self.history_detached {
                    if let Some(previous)=self.active_channel.as_ref().filter(|c| c.id()!=channel.id()) {
                        self.conversations.store(previous.id(),std::mem::take(&mut self.history),self.has_more);
                    }
                }
                self.edit_lookup=None;
                self.viewport.begin_open(&widgets.scrolled_window);
                // Apply the next channel's permissions only after concealing
                // the old history; selection is one UI update, not two paints.
                if let Some((user_id,access))=selection{self.user_id=Some(user_id);self.access=access;}
                if self.active_channel.as_ref().map(|c| c.id()) != Some(channel.id()) {
                    if let Some(previous) = &self.active_channel {
                        self.draft.error=None;
                        self.saved_drafts.insert(previous.id(), std::mem::take(&mut self.draft));
                    }
                    self.draft = self.saved_drafts.remove(&channel.id()).unwrap_or_default();
                }
                self.emoji_visible=false;self.emoji_manual=false;self.emoji_dismissed=false;
                self.actions.reset_channel();
                if same_user{self.transfers.switch_channel();}else{self.transfers.reset();}
                self.render_window.measure(&widgets.messages_list);
                if let Some(previous)=previous_surface.filter(|_|same_user){
                    self.surfaces.store(previous,std::mem::take(&mut self.rendered),std::mem::take(&mut self.render_window),&widgets.messages_list,&recent);
                }
                self.viewport.reset_surface(&widgets.scrolled_window);
                self.has_more = false;self.older_requested=false;
                self.history_error = None;
                self.last_typing_sent = None;
                self.unread_start=None;
                self.history.select(channel.id());position=Some(Position::Bottom);self.viewing_old=false;self.history_detached=false;self.newer=Default::default();
                self.history_ready=false;self.media_render_pending=None;self.reconcile_request=None;
                self.initial_read=Some(viewport::ReadBoundary::new(&channel));
                if !self.access.read{self.initial_read=None;self.viewport.cancel_open(&widgets.scrolled_window);}
                if self.access.read {
                    if let Some((history,has_more))=self.conversations.take(channel.id()) {
                        self.history=history; self.has_more=has_more; self.history_ready=true;
                        if let Some((rows,window))=self.surfaces.take(channel.id()){
                            self.rendered=rows;self.render_window=window;
                            restore_avatars=true;
                        }
                        if let Some(boundary)=self.initial_read.take() {
                            if boundary.clone().older(&self.history.messages,self.has_more).is_some() { self.initial_read=Some(boundary); }
                            else { position=Some(boundary.position(&self.history.messages)); }
                        }
                    }
                }
                self.active_channel = Some(channel);
                self.typing_users.clear();
                self.composer.set_wire(&widgets.message_entry,&self.draft.text,&self.users_map);
                render_history=true;
            }
            ChatMsg::UpdateChannel(channel) => {if self.active_channel.as_ref().is_some_and(|c|c.id()==channel.id){self.active_channel=Some(ConversationTarget::Channel(channel));}},
            ChatMsg::ClearChannel => {
                self.history_detached=false;self.newer=Default::default();
                self.trim_when_idle=false;
                self.conversations.clear();self.surfaces.clear();
                self.initial_read=None;self.viewport.cancel_open(&widgets.scrolled_window);
                self.actions.reset_channel();
                self.transfers.reset();self.edit_lookup=None;
                self.access = Default::default();
                if let Some(channel) = self.active_channel.take() {
                    self.saved_drafts.insert(channel.id(), std::mem::take(&mut self.draft));
                }
                self.unread_start=None;self.edit_lookup=None;
                self.history = History::default();
                self.history_ready=false;self.media_render_pending=None;self.reconcile_request=None;
                self.has_more = false;self.older_requested=false;
                self.history_error = None;
                self.typing_users.clear();
                self.composer.set_wire(&widgets.message_entry,"",&self.users_map);
                render_history=true;position=Some(Position::Bottom);
            }
            ChatMsg::LoadNewer=>self.load_newer(&sender),
            ChatMsg::NewerLoaded{epoch,request,result}=>{
                if epoch!=self.actions.epoch||self.newer.request!=Some(request)||!self.history.is_pending(request){return;}
                self.newer.request=None;
                match result{
                    Ok(page)=>{
                        let at=self.viewport.capture(&widgets.scrolled_window,&widgets.messages_list);
                        self.newer.has_more=page.has_more;self.newer.error=None;
                        let observed=page.messages.iter().map(crate::models::MessageObservation::from).collect();
                        self.history.finish(request,page.messages,true);render_history=true;position=Some(at);
                        let _=sender.output(ChatOutput::HistoryObserved{channel_id:page.channel_id,messages:observed,latest:false});
                        if !self.newer.has_more{
                            self.history_detached=false;self.newer=Default::default();
                            if let Some(channel)=&self.active_channel{self.conversations.invalidate_channel(channel.id());}
                            sender.input(ChatMsg::Resync);
                        }
                    },
                    Err(error)=>{
                        self.history.fail(request);self.newer.error=Some(error.to_string());
                        if crate::api::is_permission_error(&error){
                            // Reuse the authoritative permission-loss cleanup.
                            self.history.begin(request);sender.input(ChatMsg::HistoryFailure{request_id:request,error});
                        }
                    }
                }
            },
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
                if self.history_detached&&matches!(&change,Change::Upsert(m) if !self.history.messages.iter().any(|old|old.id==m.id)){return;}
                if let Change::Moderation(message,attachment,status)=&change{
                    render_history=self.apply_moderation_change(*message,*attachment,status);
                }else{
                if let Change::Embeds(id,embeds)=&change{
                    if let Some(message)=self.history.messages.iter().find(|m|m.id==*id){
                        for old in message.embeds.iter().flatten(){
                            let changed=embeds.iter().find(|e|e.id==old.id).is_some_and(|e|e.metadata()!=old.metadata());
                            let removed=!embeds.iter().any(|e|e.id==old.id)&&!self.history.messages.iter().any(|m|m.id!=*id&&m.embeds.iter().flatten().any(|e|e.id==old.id));
                            if changed||removed{self.transfers.invalidate(old.id);}
                        }
                    }
                }
                if let Change::Preview(_, preview) = &change { self.transfers.invalidate(preview.id); }
                if let Change::RemovePreview(_, id) = &change { self.transfers.invalidate(*id); }
                self.history.apply(change);
                render_history=true;
                }
            }
            ChatMsg::HistoryFailure{request_id,error} => {
                if !self.history.is_pending(request_id) && self.reconcile_request!=Some(request_id) { return; }
                if self.reconcile_request==Some(request_id) { self.reconcile_request=None; }
                if crate::api::is_permission_error(&error) {
                    self.history_detached=false;self.newer=Default::default();
                    self.conversations.clear();self.surfaces.clear(); self.initial_read=None;
                    self.viewport.cancel_open(&widgets.scrolled_window);
                    self.actions.reset_channel(); self.transfers.reset();self.edit_lookup=None; self.access=Default::default();
                    if let Some(channel)=self.active_channel.as_ref() { self.history.select(channel.id()); }
                    self.history_ready=false; render_history=true;
                }
                let _=sender.output(ChatOutput::ActionError(error));
            },
            ChatMsg::RevalidateAvatars=>{render_history=false;self.transfers.external.revalidate();},
            ChatMsg::Connection(state)=>{render_history=false;self.connection=state;},
            ChatMsg::OperationError(error) => self.draft.error = Some(error),
            ChatMsg::DismissError(token)=>{if self.error_timer.as_ref().is_some_and(|(current,_)|*current==token){self.error_timer.take();self.draft.error=None;}},
            ChatMsg::Resync=>{
                if self.history_detached{return;}
                if self.access.read&&self.reconcile_request.is_none(){
                    if let (Some(api),Some(channel))=(self.actions.api.clone(),self.active_channel.as_ref()){
                        let request=Uuid::new_v4();self.reconcile_request=Some(request);self.history.begin_snapshot(request);
                        let channel_id=channel.id();let oldest=self.history.messages.first().map(MessageCursor::from);let output=sender.input_sender().clone();
                        self.transfers.jobs.push(tokio::spawn(async move{let result=api.reconcile_messages(channel_id,oldest).await;let _=output.send(ChatMsg::Reconciled{request,result});}));
                    }
                }
            },
            ChatMsg::Reconciled{request,result}=>{
                if self.reconcile_request!=Some(request){return;}
                sender.input(ChatMsg::Action(ActionMsg::Reload));
                let _=sender.output(ChatOutput::ConversationLoaded);
                match result{
                    Ok(page)=>{
                        self.reconcile_request=None;
                        let old:Vec<_>=self.history.messages.iter().map(|m|m.id).collect();let observed=page.messages.iter().map(crate::models::MessageObservation::from).collect();
                        render_history=self.history.finish_reconcile(request,page.messages,!page.has_more);self.history_ready=true;
                        if render_history&&!self.history.loading()&&!self.older_requested{if let Some(mut boundary)=self.initial_read.take(){
                            position=Some(boundary.position(&self.history.messages));
                            if let Some(cursor)=boundary.older(&self.history.messages,page.has_more){self.older_requested=true;let _=sender.output(ChatOutput::LoadMoreMessages{channel_id:page.channel_id,cursor:Some(cursor)});self.initial_read=Some(boundary);}
                        }}
                        self.has_more=page.has_more;
                        for id in old{if self.history.is_deleted(id){self.transfers.invalidate_message(id);self.actions.pinned.retain(|m|m.id!=id);}}
                        let _=sender.output(ChatOutput::HistoryObserved{channel_id:page.channel_id,messages:observed,latest:true});
                    },
                    Err(error)=>{self.history.fail_snapshot(request);sender.input(ChatMsg::HistoryFailure{request_id:request,error});},
                }
            },
            #[cfg(test)]
            ChatMsg::BeginSnapshot(request) => {if self.access.read{self.history.begin_snapshot(request);}},
            #[cfg(test)]
            ChatMsg::BeginHistory { request_id, cursor } => {
                self.older_requested=false;self.history.begin(request_id); self.history_error = None; self.retry_cursor = cursor;
            }
            ChatMsg::JumpLoaded{epoch,request,id,result}=>{
                if epoch!=self.actions.epoch||!self.history.is_pending(request)||self.actions.navigation!=Some(id){return;}
                match result{
                    Ok((page,has_newer))=>{self.newer.has_more=has_newer;self.has_more=page.has_more;self.history.finish(request,page.messages,false);self.history_ready=true;self.actions.navigation=None;if !has_newer{self.history_detached=false;self.conversations.invalidate_channel(page.channel_id);sender.input(ChatMsg::Resync);}if self.history.messages.iter().any(|m|m.id==id){self.actions.highlight=Some(id);self.draft.error=None;render_history=true;position=Some(Position::Message(id));}else{self.draft.error=Some("A mensagem foi removida.".into());sender.input(ChatMsg::Latest);}},
                    Err(error)=>{self.history.fail(request);self.actions.navigation=None;self.draft.error=Some(error.to_string());let _=sender.output(ChatOutput::ActionError(error));sender.input(ChatMsg::Latest);}
                }
            }
            ChatMsg::HistoryLoaded { request_id, result, append } => {
                if !self.history.is_pending(request_id) { return; }
                if !append { sender.input(ChatMsg::Action(ActionMsg::Reload)); let _=sender.output(ChatOutput::ConversationLoaded); }
                match result {
                    Ok(response) => {
                        let observed=response.messages.iter().map(crate::models::MessageObservation::from).collect();
                        if self.history.finish(request_id, response.messages, append) {
                            self.history_ready=true;
                            let _=sender.output(ChatOutput::HistoryObserved{channel_id:response.channel_id,messages:observed,latest:!append});
                            self.has_more = response.has_more;
                            self.history_error = None;
                            let highlight=self.actions.highlight;
                            self.continue_navigation(&sender);
                            render_history=true;
                            if self.actions.highlight!=highlight{if let Some(id)=self.actions.highlight{position=Some(Position::Message(id));}}
                            else if self.actions.navigation.is_none(){
                                if let Some(mut boundary)=self.initial_read.take(){
                                    position=Some(boundary.position(&self.history.messages));
                                    if let Some(cursor)=boundary.older(&self.history.messages,self.has_more){self.older_requested=true;let _=sender.output(ChatOutput::LoadMoreMessages{channel_id:response.channel_id,cursor:Some(cursor)});self.initial_read=Some(boundary);}
                                }else if !append&&following{position=Some(Position::Bottom);}
                            }
                        }
                    }
                    Err(error) => {
                        if self.history.fail(request_id) { self.older_requested=false;self.history_error = Some(error); }
                    }
                }
            }
            ChatMsg::AddMessage(msg) => {
                if self.history_detached&&!self.history.messages.iter().any(|m|m.id==msg.id){let own=self.draft.is_sending()&&msg.author_id==self.user_id;self.conversations.apply(&Change::Upsert(msg));if own{sender.input(ChatMsg::Latest);}return;}
                if let Some(author) = msg.author_id { self.typing_users.remove(&author); }
                let own_send=self.draft.is_sending()&&msg.author_id.is_some()&&msg.author_id==self.user_id&&msg.content.as_deref().unwrap_or("").trim()==self.draft.text.trim();
                render_history=self.history.upsert(msg);
                if (render_history&&following)||own_send{self.initial_read=None;position=Some(Position::Bottom);}
            }
            ChatMsg::DeleteMessage(id) => {
                self.delete_message(id);
                sender.input(ChatMsg::Action(ActionMsg::PinEvent { id, pinned: false }));
                render_history=true;
            }
            ChatMsg::SetAvatars(avatars) => {
                crate::media::update_member_avatars(widgets.messages_list.upcast_ref(),&self.avatars,&avatars);
                crate::media::update_member_avatars(widgets.mention_choices.upcast_ref(),&self.avatars,&avatars);self.avatars=avatars;self.render_pins(&sender);self.render_participants(&sender);
            }
            ChatMsg::ServerName(name)=>{*self.actions.emoji_source.borrow_mut()=name.clone();self.server_name=name;self.render_picker(&sender);self.render_pins(&sender);},
            ChatMsg::SetUsers(users) => {
                let users:HashMap<_,_>=users.into_iter().map(|u| (u.id, u)).collect();render_history=self.users_map.len()!=users.len()||users.iter().any(|(id,u)|self.users_map.get(id).is_none_or(|old|old.display_name()!=u.display_name()||old.roles!=u.roles));self.users_map=users;self.refresh_reaction_hints();self.render_pins(&sender);self.render_participants(&sender);
                self.composer.refresh_names(&widgets.message_entry,&self.users_map);
            }
            ChatMsg::UserTyping { user_id, is_typing } => {
                if is_typing { self.typing_users.insert(user_id, Instant::now()); }
                else { self.typing_users.remove(&user_id); }
            }
            ChatMsg::ClearStaleTyping => {
                self.transfers.resident.maintain();self.transfers.encoded.maintain();
                self.hydrate_media(&sender);
                let now = Instant::now();
                self.typing_users.retain(|_, last| now.duration_since(*last).as_secs() < 4);
            }
            ChatMsg::OpenMentions=>{if !self.active_channel.as_ref().is_some_and(|c|!c.is_direct()){return;}self.mention_manual=true;self.mention_dismissed=false;widgets.message_entry.grab_focus();},
            ChatMsg::MentionCursorChanged=>{},
            ChatMsg::CloseMentions=>{self.mention_dismissed=true;self.mention_manual=false;self.mention_visible=false;},
            ChatMsg::MentionSelected(id) => {
                if !self.active_channel.as_ref().is_some_and(|c|!c.is_direct()){return;}
                if !self.access.send || self.draft.is_sending() || (id.is_none()&&!self.access.everyone)||id.is_some_and(|id|self.users_map.get(&id).map_or(true,|u|u.banned)){return;}
                self.mention_manual=false;self.mention_dismissed=true;self.mention_visible=false;
                let caret=self.composer.mention(&widgets.message_entry,id,&self.users_map);
                self.draft.text=self.composer.wire_text();self.composer.focus_after_completion(&widgets.message_entry,caret);
            }
            ChatMsg::InputChanged(txt) => {
                // Restoring/clearing a saved draft also emits GTK's changed signal.
                if self.draft.text == txt { return; }
                self.emoji_manual=false;self.emoji_dismissed=false;self.mention_manual=false;self.mention_dismissed=false;self.draft.text = txt;
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
                if !self.access.send || self.transfers.clipboard_request.is_some() { return; }
                if self.draft.text.chars().count() > 8192 {
                    self.draft.error = Some("A mensagem deve ter até 8192 caracteres.".into());
                    self.schedule_error_timeout(previous_error.as_deref(),true,&sender);
                    self.update_view(widgets,sender);
                    return;
                }
                if let Some(channel) = &self.active_channel {
                    if let Some(request_id) = self.draft.begin() {
                        if !self.draft.files.is_empty() && !self.access.attachments {self.draft.finish(request_id,Err("Sem permissão para anexos.".into()));}
                        else {self.mention_visible=false;self.mention_manual=false;widgets.message_entry.grab_focus();self.start_send(&sender, request_id, channel.id());}
                    }
                }
            }
            ChatMsg::SendFinished { request_id, channel_id, result } => {
                let outcome = result.as_ref().map(|_| ()).map_err(Clone::clone);
                if let Ok(mut message) = result {if self.active_channel.as_ref().is_some_and(|c|c.id()==channel_id){
                    self.restore_recent_history(&sender);
                    if self.draft.pending_id()==Some(request_id)&&!self.draft.files.is_empty()&&self.user_id.is_some()&&message.author_id==self.user_id{
                        let known=self.history.messages.iter().find(|m|m.id==message.id);
                        self.transfers.sent_attachments.extend(message.attachments.iter().flatten().filter(|a|
                            matches!(a.moderation_status.as_deref(),Some("pending"|"processing"))&&!known.is_some_and(|m|m.attachments.iter().flatten().any(|old|old.id==a.id&&matches!(old.moderation_status.as_deref(),Some("clean"|"sensitive"|"failed"|"blocked"|""))))
                        ).map(|a|a.id));
                    }
                    // POST returns only custom embeds; the ordered WS list may
                    // already include automatic embeds processed in background.
                    if self.history.messages.iter().find(|m|m.id==message.id).is_some_and(|m|m.embeds.is_some()){message.embeds=None;}
                    self.initial_read=None;self.history.upsert(message);render_history=true;position=Some(Position::Bottom);
                }}
                if self.active_channel.as_ref().map(|c| c.id()) == Some(channel_id) {
                    if self.draft.finish(request_id, outcome) {
                        self.composer.set_wire(&widgets.message_entry,&self.draft.text,&self.users_map);
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
        if update_edit_mentions{self.refresh_edit_mentions();}
        if update_mentions{
            let entry_text=widgets.message_entry.text();let completion=mentions::completion(&entry_text,widgets.message_entry.position());
            if self.active_channel.as_ref().is_some_and(|c|!c.is_direct())&&self.access.send&&!self.draft.is_sending()&&!self.mention_dismissed&&(self.mention_manual||completion.is_some()){
            let filter=if self.mention_manual{String::new()}else{completion.as_ref().map(|(_,_,f)|f.to_lowercase()).unwrap_or_default()};
            let mut members:Vec<_>=self.users_map.values().filter(|u|!u.banned&&(u.display_name().to_lowercase().contains(&filter)||u.username.to_lowercase().contains(&filter))).collect();members.sort_by_cached_key(|u|(u.display_name().to_lowercase(),u.username.to_lowercase(),u.id));
            let everyone=self.access.everyone&&"everyone".contains(&filter);
            let show_avatars=self.config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true);
            let signature=format!("{filter}|{everyone}|{show_avatars}|{:?}",members.iter().take(50).map(|u|(u.id,u.display_name(),&u.username,crate::models::role::role_rgb(u.roles.as_deref().unwrap_or_default()))).collect::<Vec<_>>());
            if self.mention_signature!=signature{
                self.mention_signature=signature;while let Some(child)=widgets.mention_choices.first_child(){widgets.mention_choices.remove(&child);}
                if members.is_empty()&&!everyone{let label=gtk::Label::new(Some("Nenhum membro encontrado"));label.set_margin_top(8);label.set_margin_bottom(8);label.add_css_class("dim-label");widgets.mention_choices.append(&label);}
                for u in members.into_iter().take(50){let id=u.id;let input=sender.input_sender().clone();
                    widgets.mention_choices.append(&mentions::choice(u,self.avatars.get(&id),show_avatars,move ||{let _=input.send(ChatMsg::MentionSelected(Some(id)));}));}
                if everyone{let button=gtk::Button::with_label("@everyone");let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::MentionSelected(None));});widgets.mention_choices.append(&button);}
            }
                // GTK reserves a scrollbar's minimum height even for one row.
                // Show a scrollbar only when the choices exceed the height cap.
                let height=widgets.mention_choices.measure(gtk::Orientation::Vertical,-1).1;
                widgets.mention_panel.set_vscrollbar_policy(if height>240{gtk::PolicyType::Automatic}else{gtk::PolicyType::External});
                self.mention_visible=true;
            }else{self.mention_visible=false;self.mention_manual=false;}
        }
        if update_emojis{
            let text=widgets.message_entry.text();let completion=emoji::completion(&text,widgets.message_entry.position());
            if self.active_channel.is_some()&&self.access.send&&!self.draft.is_sending()&&!self.emoji_dismissed&&(self.emoji_manual||completion.is_some())&&!self.mention_visible{
                let filter=if self.emoji_manual{""}else{completion.as_ref().map_or("",|(_,_,f)|*f)};
                let choices=emoji::choices(&self.unicode_emojis,&self.actions.emojis,filter);
                let signature=format!("{}|{}|{:?}",self.emoji_manual,filter,choices.iter().map(|c|(&c.name,c.id,c.id.and_then(|id|self.actions.textures.get(&id)).map(|t|t.as_ptr() as usize))).collect::<Vec<_>>());
                if self.emoji_signature!=signature{
                    self.emoji_signature=signature;while let Some(child)=widgets.emoji_choices.first_child(){widgets.emoji_choices.remove(&child);}
                    for choice in choices.iter().take(if self.emoji_manual{500}else{30}){let button=gtk::Button::new();button.add_css_class("flat");let row=gtk::Box::new(gtk::Orientation::Horizontal,8);
                        if let Some(image)=choice.id.and_then(|id|self.actions.emoji_image(id,24)){row.append(&image);}else{row.append(&gtk::Label::new(Some(if choice.id.is_none(){&choice.text}else{"◌"})));}
                        let label=gtk::Label::new(Some(&format!(":{}:",choice.name)));label.set_xalign(0.0);label.set_hexpand(true);label.set_ellipsize(pango::EllipsizeMode::End);row.append(&label);button.set_child(Some(&row));let input=sender.input_sender().clone();let value=choice.text.clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::EmojiSelected(value.clone()));});widgets.emoji_choices.append(&button);
                    }
                    if self.emoji_manual{let all=gtk::Button::with_label("Todos os emojis Unicode…");let input=sender.input_sender().clone();all.connect_clicked(move |_|{let _=input.send(ChatMsg::UnicodePicker);});widgets.emoji_choices.prepend(&all);}
                    if choices.is_empty(){widgets.emoji_choices.append(&gtk::Label::new(Some("Nenhum emoji encontrado")));}
                }self.emoji_visible=true;
            }else{self.emoji_visible=false;self.emoji_manual=false;}
        }
        if let Some(Position::Message(id))=position.as_ref(){
            self.highlight_generation=self.highlight_generation.wrapping_add(1);let generation=self.highlight_generation;let epoch=self.actions.epoch;let id=*id;let input=sender.input_sender().clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(2500),move ||{let _=input.send(ChatMsg::ExpireHighlight{epoch,id,generation});});
        }
        if let Some(Position::Unread(id))=position.as_ref(){self.unread_start=Some(*id);}
        // Reserve the older-history banner's height before allocating the
        // opening rows, rather than shrinking the viewport after the reveal.
        if let Some(position)=position.as_ref(){match position{Position::Unread(_)|Position::Message(_)=>self.viewing_old=true,Position::Bottom=>self.viewing_old=self.history_detached,_=>{}}}
        if self.viewport.open_ready((self.history_ready&&self.initial_read.is_none())||self.history_error.is_some())&&!render_history&&position.is_none(){
            position=Some(self.viewport.capture(&widgets.scrolled_window,&widgets.messages_list));
        }
        if !self.history_detached&&!self.actions.pinned_view&&self.trim_when_idle&&self.transfers.saving==0&&self.initial_read.is_none()&&self.history_ready&&!self.viewport.scrolling()&&(self.viewport.following()||matches!(position,Some(Position::Bottom))){
            if self.history.trim_recent(cache::MESSAGE_LIMIT){
                self.transfers.trim_provider_ids(&self.history.messages);
                self.has_more=true;self.older_requested=false;self.retry_cursor=None;self.history_error=None;
                self.transfers.scope.clear();render_history=true;position=Some(Position::Bottom);
            }
            if !self.history.loading()&&!self.older_requested{self.trim_when_idle=false;}
        }
        if self.retain_history_window(widgets,position.as_ref()){
            render_history=true;
            if position.is_none(){position=Some(self.viewport.capture(&widgets.scrolled_window,&widgets.messages_list));}
        }
        let anchor=if render_history{Some(self.viewport.capture(&widgets.scrolled_window,&widgets.messages_list))}else{None};
        if render_history{
            self.render_window.measure(&widgets.messages_list);
            let desired=position.as_ref().unwrap_or_else(||if following{&Position::Bottom}else{anchor.as_ref().unwrap()});
            self.render_window.select(&self.history.messages,desired,widgets.scrolled_window.vadjustment().value(),widgets.scrolled_window.vadjustment().page_size());
            let visible=self.history.messages.iter().skip(self.render_window.range.start).take(self.render_window.range.len()).map(|m|m.id).collect();self.transfers.release_offscreen_playback(&visible);
        }
        if render_history&&hydrate{
            if self.transfers.scope.is_empty(){self.transfers.scope=self.history.messages.iter().rev().take(8).map(|m|m.id).collect();}
            self.hydrate_media(&sender);
        }
        if render_history&&hydrate&&self.viewport.scrolling(){self.viewport.media_when_idle(&sender);}
        let dirty=if render_history{
            let mut dirty=self.history.take_dirty();
            let media=if self.viewport.scrolling(){Default::default()}else{std::mem::take(&mut self.transfers.dirty)};
            for m in self.history.messages.iter().skip(self.render_window.range.start).take(self.render_window.range.len()){
                if m.attachments.iter().flatten().any(|a|media.contains(&a.id))||m.embeds.iter().flatten().any(|p|media.contains(&p.id))||media.iter().any(|key|self.transfers.giphy_key(m,*key)){dirty.insert(m.id);}
            }
            if incremental{Some(dirty)}else{None}
        }else{None};
        #[cfg(test)]
        if render_history{self.render_passes+=1;}
        if render_history{if !self.viewport.scrolling(){self.media_render_pending=None;}rebuild_messages_view(&widgets.messages_list, &mut self.rendered, &self.history.messages, &self.render_window,dirty.as_ref(), self.unread_start, self.history_ready, &self.users_map, &self.avatars, &self.access, self.user_id, &self.actions, &self.transfers, &self.config, self.viewport.scrolling(), &sender);self.viewport.media_after_layout(&sender);self.viewport.restore(&widgets.scrolled_window,&widgets.messages_list,position.unwrap_or_else(||if following{Position::Bottom}else{anchor.unwrap()}),&sender);}
        else if let Some(position)=position{self.viewport.restore(&widgets.scrolled_window,&widgets.messages_list,position,&sender);}
        if restore_avatars{crate::media::update_member_avatars(widgets.messages_list.upcast_ref(),&HashMap::new(),&self.avatars);self.transfers.external.restore(&widgets.messages_list,self.history.messages.iter().skip(self.render_window.range.start).take(self.render_window.range.len()));}
        if update_files{
            let context=self.active_channel.as_ref().map(|c|(c.id(),self.user_id));
            self.draft_previews.render(&widgets.selected_files,&self.draft.files,context,self.access.send&&self.access.attachments,self.draft.is_sending(),&sender);
        }
        if update_reply{while let Some(child)=widgets.reply_preview.first_child(){widgets.reply_preview.remove(&child);}if let Some(reply)=&self.draft.reply{widgets.reply_preview.append(&text::compact(reply.content.as_deref().unwrap_or("[Anexo]"),&self.users_map,&self.actions));}}
        widgets.reply_author.set_attributes(None);
        if let Some(roles)=self.draft.reply.as_ref().and_then(|m|m.author_id).and_then(|id|self.users_map.get(&id)).and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&widgets.reply_author,roles);}
        if render_history||update_moderation{self.schedule_moderation(&sender,root.is_mapped());}
        self.schedule_error_timeout(previous_error.as_deref(),new_error,&sender);
        self.update_view(widgets, sender);
    }
}

impl ChatModel {
    pub(crate) fn external_avatar(&self,author:&crate::models::ExternalAuthor)->Option<gtk::gdk::Texture>{self.transfers.external.texture_url(&author.avatar_url).cloned()}
    fn restore_recent_history(&mut self,sender:&ComponentSender<Self>){
        if !self.history_detached{return;}
        self.history_detached=false;self.newer=Default::default();self.reconcile_request=None;self.history_error=None;self.actions.navigation=None;self.actions.highlight=None;
        if let Some(channel)=self.active_channel.as_ref().map(|c|c.id()){
            self.history.select(channel);self.history_ready=false;
            if let Some((history,more))=self.conversations.take(channel){self.history=history;self.has_more=more;self.history_ready=true;}
            sender.input(ChatMsg::RequestHistory{channel_id:channel,cursor:None});
        }
    }
    fn schedule_error_timeout(&mut self,previous_error:Option<&str>,new_error:bool,sender:&ComponentSender<Self>){
        // Only temporary composer/action feedback expires. History failures
        // retain their Retry action, and connection status follows socket state.
        if self.draft.error.as_deref()!=previous_error || (self.draft.error.is_some()&&(new_error||self.error_timer.is_none())){
            if let Some((_,timer))=self.error_timer.take(){
                // The callback may already have fired while its dismissal is
                // still queued behind a newer error in the component channel.
                if let Some(source)=gtk::glib::MainContext::default().find_source_by_id(&timer){source.destroy();}
            }
            if self.draft.error.is_some(){
                let token=Uuid::new_v4();let input=sender.input_sender().clone();
                let timer=gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(5),move ||{let _=input.send(ChatMsg::DismissError(token));});
                self.error_timer=Some((token,timer));
            }
        }
    }
    /// Never acknowledge unread navigation, pagination or an unsettled opening.
    pub(crate) fn read_candidate(&self)->Option<Message> {
        if self.actions.pinned_view || !self.access.read || !self.history_ready || self.history.loading()
            || self.reconcile_request.is_some() || self.initial_read.is_some()
            || self.viewing_old || self.history_detached || !self.viewport.read_ready() {
            return None;
        }
        let channel=self.active_channel.as_ref()?.id();
        self.history.messages.iter().filter(|m|m.channel_id==channel)
            .max_by_key(|m|(m.created_at,m.id)).cloned()
    }
    pub(crate) fn profile_text(&self,value:&str)->gtk::TextView{text::widget(value,&self.users_map,&self.actions)}
    pub(crate) fn text_renderer(&self,users:HashMap<Uuid,UserSummary>)->(u64,std::rc::Rc<dyn Fn(&str)->gtk::TextView>){
        use std::hash::{Hash,Hasher};
        let mut actions=Actions::default();actions.emoji_names=self.actions.emoji_names.clone();actions.textures=self.actions.textures.clone();actions.animations=self.actions.animations.clone();actions.emoji_source=self.actions.emoji_source.clone();
        let mut hash=std::collections::hash_map::DefaultHasher::new();
        let mut ids:Vec<_>=actions.emoji_names.iter().collect();ids.sort();for(name,id)in ids{name.hash(&mut hash);id.hash(&mut hash);actions.emoji_paintable(*id).map(|p|p.as_ptr() as usize).hash(&mut hash);}
        (hash.finish(),std::rc::Rc::new(move |text|text::widget(text,&users,&actions)))
    }
    // The editable profile already shows Unicode glyphs; its extra preview is only for custom images.
    pub(crate) fn has_profile_emoji(&self,value:&str)->bool{text::parts_index(value,&self.actions.emoji_names).iter().any(|p|matches!(p,text::Part::Emoji(..)))}
}

fn first_mention_button(widget:&gtk::Widget)->Option<gtk::Button>{
    if let Some(button)=widget.downcast_ref::<gtk::Button>(){return Some(button.clone());}
    let mut child=widget.first_child();while let Some(w)=child{if let Some(button)=first_mention_button(&w){return Some(button);}child=w.next_sibling();}None
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

#[cfg(test)]
thread_local!{static FORMATTED_ROWS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}

/// Cache only data that changes the row's presentation, never encoded media.
fn message_body_signature(msg:&Message)->String{
    let embeds:Vec<_>=msg.embeds.iter().flatten().map(Embed::metadata).collect();
    format!("{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{embeds:?}",(msg.author_id,msg.webhook_id,&msg.external_author),msg.content,msg.created_at,msg.edited_at,msg.reply_to,msg.attachments)
}
#[cfg(test)]fn message_visual_signature(msg:&Message)->String{format!("{}|{:?}|{:?}",message_body_signature(msg),msg.reactions,msg.user_reactions)}

#[cfg(test)]
mod render_tests {
    use super::*;
    #[test]
    fn row_signature_omits_encoded_media_but_tracks_preview_metadata(){
        let mut message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"content":"hello","created_at":"2026-10-07T12:00:00Z","previews":[{"id":Uuid::new_v4(),"url":"https://example.test","kind":"og","title":"Photo","fetched_at":"2026-10-07T12:00:00Z"}]})).unwrap();
        let original=message_visual_signature(&message);
        message.embeds.as_mut().unwrap()[0].image_data=Some("A".repeat(4<<20));
        message.embeds.as_mut().unwrap()[0].author_image_data=Some("B".repeat(1<<20));
        assert_eq!(message_visual_signature(&message),original);
        message.embeds.as_mut().unwrap()[0].title=Some("Updated title".into());
        assert_ne!(message_visual_signature(&message),original);
        let changed=message_visual_signature(&message);message.content=Some("edited".into());assert_ne!(message_visual_signature(&message),changed);
    }
}

fn refresh_row_actions(row:&gtk::ListBoxRow,message:&Message,actions:&Actions,access:&crate::models::Access){
    fn visit(widget:&gtk::Widget,busy:bool,ready:bool,access:&crate::models::Access){
        if let Some(button)=widget.downcast_ref::<gtk::Button>(){
            if button.has_css_class("papo-reaction"){button.set_sensitive(!busy&&(access.send||(access.read&&button.has_css_class("own"))));}
            match button.widget_name().as_str(){
            "message-pin"=>button.set_sensitive(access.pin&&ready&&!busy),"message-mutate"=>button.set_sensitive(!busy),"message-react"=>button.set_sensitive(access.send&&!busy),_=>{}
        }}
        let mut child=widget.first_child();while let Some(w)=child{child=w.next_sibling();visit(&w,busy,ready,access);}
    }visit(row.upcast_ref(),actions.busy.contains(&message.id),actions.pins_ready,access);
}

fn message_action_button(label: &str, icon: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("flat");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(16);
    content.append(&image);
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    content.append(&text);
    button.set_child(Some(&content));
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

fn rebuild_messages_view(
    list_box: &gtk::ListBox,
    rendered:&mut HashMap<String,(String,gtk::ListBoxRow)>,
    messages: &[Message],
    window:&window::RenderWindow,
    dirty:Option<&std::collections::HashSet<Uuid>>,
    unread_start:Option<Uuid>,
    history_ready: bool,
    users_map: &HashMap<Uuid, UserSummary>,
    avatars: &HashMap<Uuid, gtk::gdk::Texture>,
    access: &crate::models::Access,
    user_id: Option<Uuid>,
    actions: &Actions,
    media: &Transfers,
    config: &crate::models::UserConfig,
    defer_media:bool,
    sender: &ComponentSender<ChatModel>,
) {
    if messages.is_empty() {
        if history_ready && list_box.first_child().is_some_and(|w|w.widget_name()=="empty-history"){return;}
        rendered.clear();while let Some(child)=list_box.first_child(){list_box.remove(&child);}
        // A newly selected channel has no snapshot yet. Do not flash the empty
        // conversation prompt between selection and the history response.
        if !history_ready{return;}
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
        if actions.pinned_view{label.set_text("Nenhuma mensagem fixada.");}
        label.add_css_class("dim-label");
        empty_box.append(&label);

        let row = gtk::ListBoxRow::new();
        row.set_widget_name("empty-history");
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&empty_box));
        list_box.append(&row);
        return;
    }

    let pinned:std::collections::HashSet<_>=actions.pinned.iter().map(|m|m.id).collect();
    let by_id:HashMap<_,_>=messages.iter().map(|m|(m.id,m)).collect();
    let mut ordered=vec![];
    for (index,msg) in messages.iter().enumerate().skip(window.range.start).take(window.range.len()) {
        let previous=index.checked_sub(1).and_then(|i|messages.get(i));
        let date=msg.created_at.with_timezone(&chrono::Local).date_naive();
        if previous.is_none_or(|p|p.created_at.with_timezone(&chrono::Local).date_naive()!=date){
            let key=format!("date-{date}");if let Some((_,row))=rendered.get(&key){ordered.push(row.clone());}else{
            let divider=gtk::Box::new(gtk::Orientation::Horizontal,12);divider.add_css_class("papo-date-divider");
            for before in [true,false]{let line=gtk::Separator::new(gtk::Orientation::Horizontal);line.set_hexpand(true);line.set_valign(gtk::Align::Center);divider.append(&line);if before{let date=gtk::Label::new(Some(&date.format("%d/%m/%Y").to_string()));date.add_css_class("caption");date.add_css_class("dim-label");divider.append(&date);}}
            let row=gtk::ListBoxRow::new();row.set_widget_name(&key);row.set_activatable(false);row.set_selectable(false);row.set_child(Some(&divider));rendered.insert(key,(String::new(),row.clone()));ordered.push(row);
        }}
        if unread_start==Some(msg.id){
            let key=format!("unread-start-{}",msg.id);
            let entry=rendered.entry(key.clone()).or_insert_with(||{
                let box_=gtk::Box::new(gtk::Orientation::Horizontal,10);box_.add_css_class("papo-unread-divider");
                let line=gtk::Separator::new(gtk::Orientation::Horizontal);line.set_hexpand(true);line.set_valign(gtk::Align::Center);box_.append(&line);
                let label=gtk::Label::new(Some("Mensagens não lidas"));label.add_css_class("caption-heading");box_.append(&label);
                let row=gtk::ListBoxRow::new();row.set_widget_name(&key);row.set_activatable(false);row.set_selectable(false);row.set_focusable(false);row.set_child(Some(&box_));(String::new(),row)
            });ordered.push(entry.1.clone());
        }
        let grouped=unread_start!=Some(msg.id)&&msg.reply_to.is_none() && msg.edited_at.is_none() && !pinned.contains(&msg.id) && previous.is_some_and(|p|msg.same_sender(p)&&p.created_at.with_timezone(&chrono::Local).date_naive()==date&&(0..300).contains(&msg.created_at.signed_duration_since(p.created_at).num_seconds()));
        let key=format!("message-{}",msg.id);
        if dirty.is_some_and(|ids|!ids.contains(&msg.id)){
            if let Some((_,row))=rendered.get(&key){refresh_row_actions(row,msg,actions,access);if actions.highlight==Some(msg.id){row.add_css_class("papo-message-highlight");}else{row.remove_css_class("papo-message-highlight");}ordered.push(row.clone());continue;}
        }
        let author=msg.author_id.and_then(|id|users_map.get(&id));
        let mentioned=user_id.is_some_and(|id|msg.author_id!=Some(id)&&(
            mentions::addressed_to(msg.content.as_deref().unwrap_or(""),id,users_map)
            ||msg.reply_to.and_then(|reply|by_id.get(&reply)).is_some_and(|original|original.channel_id==msg.channel_id&&original.author_id==Some(id))
        ));
        let text=msg.content.as_deref().map(|text|mentions::render(text,users_map));
        let reply_text=msg.reply_to.map(|id|mentions::render(by_id.get(&id).and_then(|m|m.content.as_deref()).unwrap_or("Abrir mensagem respondida"),users_map));
        let emoji_state:Vec<_>=msg.reactions.iter().flatten().filter_map(|r|r.emoji_id.map(|id|(id,actions.emoji_label(id),actions.textures.get(&id).map_or(0,|t|t.as_ptr() as usize)))).collect();
        #[cfg(test)]
        FORMATTED_ROWS.with(|n|n.set(n.get()+1));
        let message_state=message_body_signature(msg);
        let signature=format!("{message_state}|{grouped}|{access:?}|{user_id:?}|{config:?}|{:?}|{}|{}",author.map(|u|(u.display_name(),&u.roles)),pinned.contains(&msg.id),msg.created_at.with_timezone(&chrono::Local).format("%H:%M"));
        let inline=text::parts_index(&format!("{} {}",msg.content.as_deref().unwrap_or(""),reply_text.as_deref().unwrap_or("")),&actions.emoji_names);let inline_state:Vec<_>=inline.iter().filter_map(|p|if let text::Part::Emoji(id,name)=p{Some((*id,name,actions.textures.get(id).map_or(0,|t|t.as_ptr() as usize)))}else{None}).collect();
        let reply_author=msg.reply_to.and_then(|id|by_id.get(&id)).map(|m|m.sender_name(users_map));
        let signature=format!("{signature}|{:?}|{text:?}|{reply_text:?}|{reply_author:?}|{inline_state:?}|{mentioned}|{}",media.external.texture(msg).map(|t|t.as_ptr() as usize),media.external.failed(msg));
        let reaction_signature=format!("{:?}|{:?}|{emoji_state:?}",msg.reactions,msg.user_reactions);
        let body_signature=signature;let signature=format!("{body_signature}\u{1f}{reaction_signature}");
        if let Some((old,row))=rendered.get_mut(&key){
            if old!=&signature&&old.split_once('\u{1f}').is_some_and(|(body,_)|body==body_signature){
                if let Some(flow)=reaction_box(row.upcast_ref()){render_reactions(&flow,msg,actions,access,sender);*old=signature.clone();}
            }
            if old==&signature{
            refresh_row_actions(row,msg,actions,access);
            if !defer_media{if let Some(container)=find_media_box(row.upcast_ref()){transfers::update_media_box(&container,msg,media,users_map,actions,sender);}}
            if actions.highlight==Some(msg.id){row.add_css_class("papo-message-highlight");}else{row.remove_css_class("papo-message-highlight");}ordered.push(row.clone());continue;}}
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(&key);
        row.set_focusable(true);
        if mentioned{row.add_css_class("papo-message-mentioned");}
        if config.display.as_ref().and_then(|d|d.show_timestamps).unwrap_or(true){row.set_tooltip_text(Some(&msg.created_at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()));}
        if actions.highlight == Some(msg.id) { row.add_css_class("papo-message-highlight"); }
        row.set_activatable(false);
        row.set_selectable(false);

        // Measure the avatar and responsive body at the same allocated width.
        // GtkBox can otherwise report a natural height below its minimum when
        // shrinking an image whose height grows with its available width.
        let box_row = gtk::Grid::builder().column_spacing(12).build();
        let spacing=match config.display.as_ref().and_then(|d|d.message_density.as_ref()){Some(crate::models::MessageDensity::Compact)=>2,Some(crate::models::MessageDensity::Comfortable)=>12,_=>6};
        box_row.set_margin_top(if grouped{2}else{spacing+6});
        box_row.set_margin_bottom(spacing);
        box_row.set_margin_start(16);
        box_row.set_margin_end(16);

        // Author user avatar (falling back to generic symbolic icon)
        let author_user = msg.author_id.and_then(|id| users_map.get(&id));
        let avatar_widget = avatar_image(msg.author_id.and_then(|id| avatars.get(&id)).or_else(||media.external.texture(msg)), 36);
        if let Some(id)=msg.author_id{avatar_widget.set_widget_name(&format!("avatar-{id}"));}
        avatar_widget.set_text(Some(msg.sender_name(users_map)));
        avatar_widget.set_valign(gtk::Align::Start);
        avatar_widget.set_visible(config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true));
        let avatar_button=gtk::Button::new();avatar_button.add_css_class("flat");avatar_button.add_css_class("papo-message-avatar");avatar_button.set_valign(gtk::Align::Start);avatar_button.set_tooltip_text(Some("Abrir perfil"));avatar_button.set_child(Some(&avatar_widget));
        if let Some(id)=msg.author_id{let s=sender.clone();avatar_button.connect_clicked(move |_|s.input(ChatMsg::OpenProfile(id)));}else{avatar_button.set_can_target(false);avatar_button.set_focusable(false);avatar_button.set_tooltip_text(None);}
        avatar_button.set_visible(avatar_widget.is_visible());
        if grouped&&avatar_widget.is_visible(){let gutter=gtk::Box::new(gtk::Orientation::Horizontal,0);gutter.set_width_request(36);box_row.attach(&gutter,0,0,1,1);}else{box_row.attach(&avatar_button,0,0,1,1);}

        // Message Content Body
        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content_box.set_hexpand(true);

        // Header (author name + time + edited tag)
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);header_box.add_css_class("papo-message-header");header_box.set_visible(!grouped);

        let author_name = msg.sender_name(users_map);

        let name_label = gtk::Label::new(Some(author_name));
        if let Some(roles)=author_user.and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&name_label,roles);}
        name_label.set_ellipsize(pango::EllipsizeMode::End);
        name_label.add_css_class("heading");
        let name_button=gtk::Button::new();name_button.add_css_class("flat");name_button.add_css_class("papo-author-button");name_button.set_child(Some(&name_label));
        if let Some(id)=msg.author_id{let s=sender.clone();name_button.connect_clicked(move |_|s.input(ChatMsg::OpenProfile(id)));}else{name_button.set_can_target(false);name_button.set_focusable(false);}header_box.append(&name_button);
        if let Some(external)=msg.external(){let badge=gtk::Label::new(Some("APP"));badge.add_css_class("caption-heading");badge.add_css_class("accent");badge.set_tooltip_text(Some(&format!("Webhook · {}",external.source)));header_box.append(&badge);}

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

        if pinned.contains(&msg.id) { header_box.append(&gtk::Label::new(Some("📌 Fixada"))); }
        content_box.append(&header_box);
        if let Some(reply) = msg.reply_to {
            let channel_id = msg.channel_id; let s = sender.clone();
            let reply_button=gtk::Button::new();reply_button.add_css_class("flat");reply_button.add_css_class("papo-reply-reference");reply_button.set_halign(gtk::Align::Fill);reply_button.set_tooltip_text(Some("Ir para a mensagem respondida"));
            let line=gtk::Box::new(gtk::Orientation::Horizontal,6);line.append(&gtk::Image::from_icon_name("mail-reply-sender-symbolic"));
            let original=by_id.get(&reply);let name=gtk::Label::new(Some(original.map(|m|m.sender_name(users_map)).unwrap_or("Mensagem")));name.add_css_class("heading");name.add_css_class("caption");name.set_ellipsize(pango::EllipsizeMode::End);name.set_max_width_chars(20);if let Some(roles)=original.and_then(|m|m.author_id).and_then(|id|users_map.get(&id)).and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&name,roles);}line.append(&name);
            let snippet=text::compact(reply_text.as_deref().unwrap_or("Abrir mensagem respondida"),users_map,actions);snippet.add_css_class("dim-label");line.append(&snippet);reply_button.set_child(Some(&line));
            reply_button.connect_clicked(move |_|{let _=s.output(ChatOutput::Navigate{channel_id,message_id:reply});});content_box.append(&reply_button);
        }

        if msg.content.is_some(){let raw=transfers::gifs::display_text(msg);if !raw.is_empty(){let input=sender.input_sender().clone();content_box.append(&text::with_profiles(&raw,users_map,actions,move |id|{let _=input.send(ChatMsg::OpenProfile(id));}));}}

        let media_box=gtk::Box::new(gtk::Orientation::Vertical,4);media_box.set_widget_name("message-media");
        transfers::update_media_box(&media_box,msg,media,users_map,actions,sender);media_box.set_visible(media_box.first_child().is_some());content_box.append(&media_box);

        if actions.pinned_view{
            let open=gtk::Button::with_label("Abrir mensagem no histórico");open.add_css_class("flat");open.set_halign(gtk::Align::Start);
            let output=sender.clone();let channel_id=msg.channel_id;let message_id=msg.id;
            open.connect_clicked(move |_|{let _=output.output(ChatOutput::Navigate{channel_id,message_id});});content_box.append(&open);
        }

        // Reactions Flow / Pill List
        let reactions_box = gtk::FlowBox::new();
        reactions_box.set_halign(gtk::Align::Start);
        reactions_box.set_selection_mode(gtk::SelectionMode::None);
        reactions_box.set_min_children_per_line(1);
        reactions_box.set_max_children_per_line(8);
        reactions_box.set_column_spacing(6);
        reactions_box.set_row_spacing(4);
        reactions_box.set_margin_top(0);

        reactions_box.set_widget_name("message-reactions");render_reactions(&reactions_box,msg,actions,access,sender);

        let toolbar=gtk::Box::new(gtk::Orientation::Horizontal,2);toolbar.add_css_class("papo-message-actions");
        let menu=gtk::MenuButton::new();menu.set_icon_name("view-more-symbolic");menu.add_css_class("flat");menu.set_tooltip_text(Some("Mais ações da mensagem"));
        let popover=gtk::Popover::new();popover.add_css_class("papo-menu");
        let message_actions=gtk::Box::new(gtk::Orientation::Vertical,2);popover.set_child(Some(&message_actions));menu.set_popover(Some(&popover));
        let msg_id = msg.id;
        let add_react_btn = gtk::Button::from_icon_name("face-smile-symbolic");add_react_btn.set_widget_name("message-react");
        add_react_btn.set_tooltip_text(Some("Adicionar reação"));
        add_react_btn.add_css_class("flat"); add_react_btn.set_sensitive(access.send && !actions.busy.contains(&msg.id));
        let s = sender.clone(); add_react_btn.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::OpenPicker(msg_id)))); toolbar.append(&add_react_btn);
        let add=message_action_button("Adicionar reação", "face-smile-symbolic");add.set_widget_name("message-react");add.set_sensitive(access.send&&!actions.busy.contains(&msg.id));let input=sender.input_sender().clone();add.connect_clicked(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::OpenPicker(msg_id)));});message_actions.append(&add);
        if media.external.failed(msg){let retry=message_action_button("Tentar carregar o avatar novamente", "view-refresh-symbolic");let s=sender.clone();retry.connect_clicked(move |_|s.input(ChatMsg::Transfer(TransferMsg::RetryExternalAvatar(msg_id))));message_actions.append(&retry);}
        let who = message_action_button("Quem reagiu", "system-users-symbolic"); who.set_sensitive(access.read);
        let s = sender.clone(); who.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::OpenParticipants(msg_id)))); message_actions.append(&who);
        let reply=message_action_button("Responder", "mail-reply-sender-symbolic");reply.set_sensitive(access.send);let s=sender.clone();let message=msg.clone();reply.connect_clicked(move |_|s.input(ChatMsg::SetReply(Some(message.clone()))));message_actions.append(&reply);
        if let Some(text)=text{let copy=message_action_button("Copiar texto", "edit-copy-symbolic");let s=sender.clone();copy.connect_clicked(move |_|s.input(ChatMsg::CopyMessage(text.clone())));message_actions.append(&copy);}
        if let Some(user) = user_id.filter(|_|!actions.pinned_view) {
            if access.can_edit(user, msg.author_id) {
                let edit = message_action_button("Editar", "document-edit-symbolic");edit.set_widget_name("message-mutate"); edit.set_sensitive(!actions.busy.contains(&msg.id));
                let s = sender.clone(); let m = msg.clone(); edit.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::OpenEdit(m.clone())))); message_actions.append(&edit);
            }
            if access.can_delete(user, msg.author_id) {
                let delete = message_action_button("Excluir", "edit-delete-symbolic");delete.set_widget_name("message-mutate"); delete.add_css_class("papo-destructive"); delete.set_sensitive(!actions.busy.contains(&msg.id));
                let s = sender.clone(); let m = msg.clone(); delete.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::ConfirmDelete(m.clone())))); message_actions.append(&delete);
            }
        }
        if access.pin {
            let pin = message_action_button(if pinned.contains(&msg.id) { "Desafixar" } else { "Fixar" }, "view-pin-symbolic");pin.set_widget_name("message-pin");
            pin.set_sensitive(actions.pins_ready && !actions.busy.contains(&msg.id));
            let s = sender.clone(); pin.connect_clicked(move |_| s.input(ChatMsg::Action(ActionMsg::Pin(msg_id)))); message_actions.append(&pin);
        }

        // Reply quick button
        let reply_btn = gtk::Button::from_icon_name("mail-reply-sender-symbolic");
        reply_btn.add_css_class("flat");
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

        box_row.attach(&content_box,1,0,1,1);
        let overlay=gtk::Overlay::new();overlay.set_child(Some(&box_row));
        let revealer=gtk::Revealer::new();revealer.set_transition_type(gtk::RevealerTransitionType::Crossfade);revealer.set_transition_duration(100);revealer.set_halign(gtk::Align::End);revealer.set_valign(gtk::Align::Start);revealer.set_child(Some(&toolbar));overlay.add_overlay(&revealer);
        let hover=std::rc::Rc::new(std::cell::Cell::new(false));
        let motion=gtk::EventControllerMotion::new();let h=hover.clone();let reveal=revealer.clone();motion.connect_enter(move |_,_,_|{h.set(true);reveal.set_reveal_child(true);});
        let h=hover.clone();let reveal=revealer.clone();let r=row.downgrade();let p=popover.downgrade();motion.connect_leave(move |_|{h.set(false);if !r.upgrade().is_some_and(|r|r.has_focus()||r.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN))&&!p.upgrade().is_some_and(|p|p.is_visible()){reveal.set_reveal_child(false);}});row.add_controller(motion);
        let focus=gtk::EventControllerFocus::new();let reveal=revealer.clone();focus.connect_enter(move |_|reveal.set_reveal_child(true));let reveal=revealer.clone();let h=hover.clone();let p=popover.downgrade();focus.connect_leave(move |_|{if !h.get()&&!p.upgrade().is_some_and(|p|p.is_visible()){reveal.set_reveal_child(false);}});row.add_controller(focus);
        let reveal=revealer.clone();let h=hover.clone();let r=row.downgrade();popover.connect_closed(move |popover|{popover.set_pointing_to(None);if !h.get()&&!r.upgrade().is_some_and(|r|r.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN)){reveal.set_reveal_child(false);}});
        crate::ui::style::close_popovers_on_action(&overlay);
        row.set_child(Some(&overlay));
        let click=gtk::GestureClick::new();click.set_button(3);click.set_propagation_phase(gtk::PropagationPhase::Capture);let s=sender.clone();click.connect_pressed(move |gesture,_,x,y|{gesture.set_state(gtk::EventSequenceState::Claimed);s.input(ChatMsg::ContextAt{id:msg_id,x,y});});row.add_controller(click);
        let hold=gtk::GestureLongPress::new();hold.set_touch_only(true);let s=sender.clone();hold.connect_pressed(move |_,x,y|s.input(ChatMsg::ContextAt{id:msg_id,x,y}));row.add_controller(hold);
        let keys=gtk::ShortcutController::new();keys.set_scope(gtk::ShortcutScope::Local);
        for trigger in ["Menu","<Shift>F10"]{let s=sender.clone();keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger),Some(gtk::CallbackAction::new(move |_,_|{s.input(ChatMsg::ContextMenu(msg_id));gtk::glib::Propagation::Stop}))));}row.add_controller(keys);
        rendered.insert(key,(signature,row.clone()));ordered.push(row);
    }
    for before in [true,false]{
        let height=window.spacer(messages,before);if height>0{
            let key=if before{"history-before"}else{"history-after"};
            let entry=rendered.entry(key.into()).or_insert_with(||{let row=gtk::ListBoxRow::new();row.set_widget_name(key);row.set_activatable(false);row.set_selectable(false);row.set_can_focus(false);row.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical,0)));(String::new(),row)});
            entry.1.set_height_request(height);if before{ordered.insert(0,entry.1.clone());}else{ordered.push(entry.1.clone());}
        }
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

impl ChatModel {
    pub(crate) fn avatar_authors(&self)->std::collections::HashSet<Uuid>{
        self.history.messages.iter().skip(self.render_window.range.start).take(self.render_window.range.len()).filter(|m|self.transfers.scope.is_empty()||self.transfers.scope.contains(&m.id)).filter_map(|m|m.author_id).collect()
    }
    fn delete_message(&mut self, id: Uuid) {
        let keys:Vec<_>=self.history.messages.iter().filter(|m|m.id==id).flat_map(|m|m.attachments.iter().flatten().map(|a|a.id).chain(m.embeds.iter().flatten().map(|p|p.id))).collect();
        self.transfers.invalidate_message(id);
        self.history.apply(Change::Delete(id));
        for key in keys{if !self.transfers.authorized(&self.history.messages,key){self.transfers.invalidate(key);}}
        self.actions.pinned.retain(|m| m.id != id);
    }
}

fn find_media_box(widget:&gtk::Widget)->Option<gtk::Box>{
    if widget.widget_name()=="message-media"{return widget.clone().downcast().ok();}
    let mut child=widget.first_child();while let Some(w)=child{if let Some(found)=find_media_box(&w){return Some(found);}child=w.next_sibling();}None
}

#[cfg(test)]pub(crate) fn exercise_animated_emojis(context:&gtk::glib::MainContext){emoji::exercise_resize(context);emoji::exercise(context);}

#[cfg(test)]pub(crate) fn exercise_read_candidate(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use actions::tests::pump;
    use performance::settle;
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let now=chrono::Utc::now();
    let messages:Vec<Message>=(0..80).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":now+chrono::Duration::seconds(i),"content":format!("Unread message {i}")})).unwrap()).collect();
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Unread channel","created_at":now,"last_read_message":messages[0].id,"last_message":{"id":messages[79].id,"created_at":messages[79].created_at}})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    assert!(chat.model().read_candidate().is_none(),"opening before allocation cannot acknowledge history");
    let window=adw::Window::builder().default_width(850).default_height(650).content(chat.widget()).build();window.present();settle(context);
    assert!(chat.model().read_candidate().is_none(),"opening at the unread boundary is not reading the latest message");
    chat.emit(ChatMsg::Latest);settle(context);assert_eq!(chat.model().read_candidate().unwrap().id,messages[79].id);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:Some(MessageCursor::from(&messages[0]))});pump(context);
    assert!(chat.model().read_candidate().is_none(),"history loading cannot acknowledge a cached message");
    chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:channel,messages:vec![],has_more:false})});settle(context);
    assert_eq!(chat.model().read_candidate().unwrap().id,messages[79].id);
    chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});settle(context);assert!(chat.model().read_candidate().is_none());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

impl ChatModel {
    pub(crate) fn adapt_header(controller:&Controller<Self>,bin:&adw::BreakpointBin,members:&adw::OverlaySplitView)->(gtk::Settings,gtk::glib::SignalHandlerId){
        use adw::prelude::BreakpointBinExt;
        let widgets=controller.widgets();
        widgets.conversation_header.set_centering_policy(adw::CenteringPolicy::Strict);
        widgets.header_title_area.set_hexpand(true);
        widgets.conversation_title.set_hexpand(true);
        let compact=adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth,720.0,adw::LengthUnit::Sp));
        compact.add_setters(&[(&widgets.header_pins,"visible",false),(&widgets.header_search,"visible",false),(&widgets.header_members,"visible",false)]);
        compact.add_setters(&[(&widgets.compact_header_actions,"visible",true)]);
        // Balance the compact action group against native window controls
        // without changing the title's centering policy or duplicating actions.
        let header=widgets.conversation_header.clone();let actions=widgets.header_actions.clone();
        compact.connect_apply(move |_|{header.remove(&actions);header.pack_start(&actions);});
        let header=widgets.conversation_header.clone();let actions=widgets.header_actions.clone();
        compact.connect_unapply(move |_|{header.remove(&actions);header.pack_end(&actions);});
        // Reserve the docked member column inside the title area, so the
        // title itself is centered above the chat. An overlay takes no width.
        let offset=widgets.header_title_offset.downgrade();let pane=members.downgrade();
        let condition=compact.clone();
        let align=std::rc::Rc::new(move ||{
            if let (Some(offset),Some(pane))=(offset.upgrade(),pane.upgrade()){
                let docked=!pane.is_collapsed()&&pane.shows_sidebar();
                let width=if docked{pane.sidebar_width_unit().to_px(pane.max_sidebar_width(),Some(&offset.settings())).round() as i32+1}else{0};
                offset.set_width_request(width);
                let limit=720.0+if docked{pane.max_sidebar_width()}else{0.0};
                condition.set_condition(Some(&adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth,limit,adw::LengthUnit::Sp)));
            }
        });
        let update=align.clone();members.connect_collapsed_notify(move |_|update());
        let update=align.clone();members.connect_show_sidebar_notify(move |_|update());
        align();
        // The split view uses Sp units; keep the offset in sync with desktop
        // font scaling. Layout disconnects this global settings handler on drop.
        let settings=widgets.conversation_header.settings();let update=align.clone();
        let handler=settings.connect_notify_local(None,move |_,property|{if matches!(property.name(),"gtk-xft-dpi"|"gtk-font-name"){update();}});
        bin.add_breakpoint(compact);
        (settings,handler)
    }

    /// Move the live header into the main pane's ToolbarView without duplicating
    /// its watched channel state, badges or action handlers.
    pub(crate) fn take_header(controller:&Controller<Self>)->adw::HeaderBar {
        let header=controller.widgets().conversation_header.clone();
        controller.widget().remove(&header);
        header
    }
}

fn render_reactions(reactions_box:&gtk::FlowBox,msg:&Message,actions:&Actions,access:&crate::models::Access,sender:&ComponentSender<ChatModel>){
    while let Some(child)=reactions_box.first_child(){reactions_box.remove(&child);}
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
                if let Some(img) = reaction.emoji_id.and_then(|id| actions.emoji_image(id,20)) {
                    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                    inner.append(&img);
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

    reactions_box.set_visible(msg.reactions.as_ref().is_some_and(|r|!r.is_empty()));
}
fn reaction_box(widget:&gtk::Widget)->Option<gtk::FlowBox>{
    if widget.widget_name()=="message-reactions"{return widget.clone().downcast().ok();}
    let mut child=widget.first_child();while let Some(w)=child{child=w.next_sibling();if let Some(found)=reaction_box(&w){return Some(found);}}None
}
