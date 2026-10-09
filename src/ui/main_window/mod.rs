//! Main application window coordinating the sidebar, chat area, and online user list.

pub(crate) mod layout;
mod members;
use layout::Layout;
pub(crate) mod voice;
use voice::{Voice,VoiceMsg};
pub(crate) mod moderation;
use moderation::{Moderation,ModerationMsg};
pub(crate) mod direct;
use direct::{Direct,DirectMsg};
pub(crate) mod administration;
use administration::{Administration,AdminMsg};
pub(crate) mod account;
use account::{Account, AccountMsg};
pub(crate) mod search;
use search::{Search,SearchMsg};
pub(crate) mod notifications;
use notifications::{Notifications,NoticeMsg};
pub(crate) mod security;
use security::{Security, SecurityMsg};
use gtk::prelude::*;
use relm4::prelude::*;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::media::avatars::AvatarCache;
use crate::models::{Channel, Server, UserSummary, WhoamiResponse, MessageCursor, Embed};
use crate::ui::chat::state::Change;
use std::collections::HashMap;
use std::time::{Instant, Duration};
use crate::ui::chat::{ChatInit, ChatModel, ChatMsg, ChatOutput};
use crate::ui::sidebar::{SidebarInit, SidebarModel, SidebarMsg, SidebarOutput};
use crate::ui::user_list::{UserListInit, UserListModel, UserListMsg};
use crate::ws::{self, WsEvent};

#[derive(Debug, Clone)]
pub struct MainWindowInit {
    pub current_user: WhoamiResponse,
    pub api_client: ApiClient,
}

#[derive(Debug)]
pub struct AccessSnapshot {
    user: WhoamiResponse,
    server: Option<Server>,
    roles: Vec<crate::models::Role>,
    channels: Vec<Channel>,
    access: HashMap<Uuid, crate::models::Access>,
    server_access: crate::models::Access,
    read_version: u64,
    member_version:u64,
}

pub struct MainWindowModel {
    pub current_user: WhoamiResponse,
    pub api_client: ApiClient,
    pub ws_tx: tokio::sync::mpsc::Sender<ws::WsCommand>,
    pub server: Option<Server>,
    pub channels: Vec<Channel>,
    pub users: Vec<UserSummary>,
    pub active_channel_id: Option<Uuid>,
    pub show_user_list: bool,
    layout:Layout,
    last_activity: Option<Instant>,
    last_access_refresh: Option<Instant>,
    refresh_task: Option<tokio::task::JoinHandle<()>>,
    ws_listener: Option<tokio::task::JoinHandle<()>>,
    preview_requests: HashMap<(Uuid, Uuid), Uuid>,
    avatars: AvatarCache,
    members: members::Members,
    account: Account,
    direct: Direct,
    administration: Administration,
    moderation: Moderation,
    voice: Voice,
    server_access: crate::models::Access,
    roles: Vec<crate::models::Role>,
    managed_channels: Vec<Channel>,
    setup: bool,
    security: Security,
    search: Search,
    notifications: Notifications,
    access: HashMap<Uuid, crate::models::Access>,
    access_request: Option<Uuid>,
    access_refresh_again: bool,
    startup_started: bool,
    startup_on_socket: bool,
    background_started: bool,

    // Child component controllers
    sidebar: Controller<SidebarModel>,
    chat: Controller<ChatModel>,
    user_list: Controller<UserListModel>,
}

#[derive(Debug)]
pub enum MainWindowMsg {
    Voice(VoiceMsg),
    Moderation(ModerationMsg),
    Direct(DirectMsg),
    Administration(AdminMsg),
    Account(AccountMsg),
    Security(SecurityMsg),
    Search(SearchMsg),
    Notification(NoticeMsg),
    HistoryObserved{channel_id:Uuid,messages:Vec<crate::models::Message>,latest:bool},
    LiveRead{request:Uuid,channel_id:Uuid,result:anyhow::Result<crate::models::MessageListResponse>},
    // Initial fetch results
    AccessLoaded { request_id: Uuid, result: anyhow::Result<AccessSnapshot> },
    FlushMembers,
    MembersLoaded { token: Uuid, full: bool, ids: Vec<Uuid>, result: anyhow::Result<Vec<UserSummary>> },
    AvatarsLoaded { request_id: Uuid, ids: Vec<Uuid>, result: anyhow::Result<Vec<(Uuid,Option<crate::media::PreparedImage>)>> },
    OperationFailed(anyhow::Error),
    ActionError(anyhow::Error),
    RefreshAccess,
    StartLoading,
    Navigate { channel_id: Uuid, message_id: Uuid },
    UserActivity,
    PreviewLoaded { channel_id: Uuid, message_id: Uuid, preview_id: Uuid, request_id: Uuid, result: anyhow::Result<Embed> },

    // User interactions from children
    ChannelSelected(Channel),
    SendTyping(Uuid),
    LoadMoreMessages {
        channel_id: Uuid,
        cursor: Option<MessageCursor>,
    },
    ToggleUserList, ToggleNavigation, MembersVisible(bool), SidebarMode(bool),
    Logout,

    // Realtime WebSocket events
    WsReceived(WsEvent),
    TickCleanup,
}

#[derive(Debug)]
pub enum MainWindowOutput {
    Logout,
    SessionExpired,
}

#[relm4::component(pub)]
impl Component for MainWindowModel {
    type Init = MainWindowInit;
    type Input = MainWindowMsg;
    type Output = MainWindowOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation:gtk::Orientation::Vertical,
            set_hexpand:true,set_vexpand:true,add_css_class:"papo-app",
            #[local_ref] adaptive_layout -> adw::BreakpointBin {},
        }
    }

    fn init(init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        crate::ui::style::install();
        let sidebar = SidebarModel::builder()
            .launch(SidebarInit {
                current_user: init.current_user.clone(),
                server: None,
                channels: Vec::new(),
            })
            .forward(sender.input_sender(), |output| match output {
                SidebarOutput::ChannelSelected(ch) => MainWindowMsg::ChannelSelected(ch),
                SidebarOutput::DirectSelect(id)=>MainWindowMsg::Direct(DirectMsg::Select(id,None)),
                SidebarOutput::HideDirect(id)=>MainWindowMsg::Direct(DirectMsg::Hide(id)),
                SidebarOutput::Blocks=>MainWindowMsg::Direct(DirectMsg::Blocks),
                SidebarOutput::WatchVoice{user,kind}=>MainWindowMsg::Voice(VoiceMsg::Video(voice::video::VideoMsg::Watch{user,kind})),
                SidebarOutput::VoiceSelect(id)=>MainWindowMsg::Voice(VoiceMsg::Select(id)),
                SidebarOutput::Moderation=>MainWindowMsg::Moderation(ModerationMsg::Open),
                SidebarOutput::Administration=>MainWindowMsg::Administration(AdminMsg::Open),
                SidebarOutput::Logout => MainWindowMsg::Logout,
                SidebarOutput::Profile(id) => MainWindowMsg::Account(AccountMsg::Profile(id)),
                SidebarOutput::Search => MainWindowMsg::Search(SearchMsg::Open),
                SidebarOutput::Notifications => MainWindowMsg::Notification(NoticeMsg::Open),
                SidebarOutput::Security => MainWindowMsg::Security(SecurityMsg::Open),
                SidebarOutput::Preferences => MainWindowMsg::Account(AccountMsg::Preferences),
            });

        let chat = ChatModel::builder()
            .launch(ChatInit { api: Some(init.api_client.clone()), user_id: Some(init.current_user.id), ..ChatInit::default() })
            .forward(sender.input_sender(), |output| match output {
                ChatOutput::Search=>MainWindowMsg::Search(SearchMsg::Open),
                ChatOutput::Notifications=>MainWindowMsg::Notification(NoticeMsg::Open),
                ChatOutput::HistoryObserved{channel_id,messages,latest}=>MainWindowMsg::HistoryObserved{channel_id,messages,latest},
                ChatOutput::UserTyping(channel_id) => MainWindowMsg::SendTyping(channel_id),
                ChatOutput::LoadMoreMessages { channel_id, cursor } =>
                    MainWindowMsg::LoadMoreMessages { channel_id, cursor },
                ChatOutput::ActionError(error) => MainWindowMsg::ActionError(error),
                ChatOutput::RefreshAccess => MainWindowMsg::RefreshAccess,
                ChatOutput::Navigate { channel_id, message_id } => MainWindowMsg::Navigate { channel_id, message_id },
                ChatOutput::ToggleUserList => MainWindowMsg::ToggleUserList,
                ChatOutput::ToggleNavigation => MainWindowMsg::ToggleNavigation,
                ChatOutput::OpenProfile(id) => MainWindowMsg::Account(AccountMsg::Profile(id)),
                ChatOutput::ChannelPreferences => MainWindowMsg::Account(AccountMsg::ChannelPreferences),
            });

        let user_list = UserListModel::builder().launch(UserListInit::default()).forward(sender.input_sender(), |id| MainWindowMsg::Account(AccountMsg::Profile(id)));
        let (mut ws_rx, ws_tx) = ws::spawn(init.api_client.clone());
        let avatars = AvatarCache::default();
        let voice=Voice::new(&sender);
        sidebar.emit(SidebarMsg::VoiceDock(voice.panel.clone()));
        let layout=Layout::new(sidebar.widget(),chat.widget(),user_list.widget(),&sender);
        let mut model = MainWindowModel {
            current_user: init.current_user,
            api_client: init.api_client.clone(),
            ws_tx,
            server: None,
            channels: Vec::new(),
            users: Vec::new(),
            active_channel_id: None,
            show_user_list: true,layout,
            last_activity: None,last_access_refresh:None,
            refresh_task: None,
            ws_listener: None,
            preview_requests: HashMap::new(),
            avatars,members:Default::default(),
            account: Account::default(),
            direct:Direct::default(),administration:Administration::default(),moderation:Moderation::default(),voice,
            server_access:Default::default(),roles:Vec::new(),managed_channels:Vec::new(),setup:false,
            security: Security::default(),
            search: Search::default(),
            notifications: Notifications::default(),
            access: HashMap::new(),
            access_request: None,
            access_refresh_again: false, startup_started: false, startup_on_socket: false, background_started: false,
            sidebar,
            chat,
            user_list,
        };
        let adaptive_layout=&model.layout.widget;
        let widgets = view_output!();
        layout::shortcuts(&root,&sender);
        model.account.config=model.current_user.settings.as_ref().map(|s|s.config.complete()).unwrap_or_default();
        model.apply_config(&root);
        model.load_avatar_blob(model.current_user.id,model.current_user.avatar_blob.clone(),sender.clone());
        model.publish_avatars();
        // Capture real input throughout the window; timers never mark a user active.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let input = sender.input_sender().clone();
        keys.connect_key_pressed(move |_, _, _, _| {
            let _ = input.send(MainWindowMsg::UserActivity);
            gtk::glib::Propagation::Proceed
        });
        root.add_controller(keys);
        let clicks = gtk::GestureClick::new();
        clicks.set_propagation_phase(gtk::PropagationPhase::Capture);
        let input = sender.input_sender().clone();
        clicks.connect_pressed(move |_, _, _, _| { let _ = input.send(MainWindowMsg::UserActivity); });
        root.add_controller(clicks);
        let motion = gtk::EventControllerMotion::new();
        let input = sender.input_sender().clone();
        motion.connect_motion(move |_, _, _| { let _ = input.send(MainWindowMsg::UserActivity); });
        root.add_controller(motion);
        let sender_ws = sender.clone();
        model.ws_listener = Some(tokio::spawn(async move {
            while let Some(ev) = ws_rx.recv().await {
                if sender_ws.input_sender().send(MainWindowMsg::WsReceived(ev)).is_err() {
                    break;
                }
            }
        }));
        let sender_timer = sender.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                if sender_timer.input_sender().send(MainWindowMsg::TickCleanup).is_err() {
                    break;
                }
            }
        });
        // Subscribe before taking snapshots so the first socket needs no second
        // startup wave. HTTP remains usable when WebSocket is unavailable.
        let input = sender.input_sender().clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let _ = input.send(MainWindowMsg::StartLoading);
        });
        let client = init.api_client.clone();
        let username=model.current_user.username.clone();
        let input = sender.input_sender().clone();
        model.refresh_task = Some(tokio::spawn(async move {
            crate::session::refresh::run(client,username,move |error| {
                input.send(MainWindowMsg::OperationFailed(error)).is_ok()
            }).await;
        }));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        if matches!(&message,MainWindowMsg::ChannelSelected(_)|MainWindowMsg::Direct(DirectMsg::Select(_, _))){self.layout.dismiss_navigation();}
        match message {
            MainWindowMsg::StartLoading => self.start_loading(sender),
            MainWindowMsg::ToggleNavigation=>self.layout.navigation.set_show_sidebar(!self.layout.navigation.shows_sidebar()),
            MainWindowMsg::MembersVisible(visible)=>self.show_user_list=visible,
            MainWindowMsg::SidebarMode(direct)=>{self.sidebar.emit(if direct{SidebarMsg::ShowDirect}else{SidebarMsg::ShowChannels});self.layout.navigation.set_show_sidebar(true);},
            MainWindowMsg::Voice(msg)=>self.voice_event(msg,sender),
            MainWindowMsg::Moderation(msg)=>self.moderation_event(msg,sender,root),
            MainWindowMsg::Direct(msg)=>self.direct_event(msg,sender,root),
            MainWindowMsg::Administration(msg)=>self.admin_event(msg,sender,root),
            MainWindowMsg::Search(msg)=>self.search_event(msg,root,sender),
            MainWindowMsg::Notification(msg)=>self.notification_event(msg,root,sender),
            MainWindowMsg::HistoryObserved{channel_id,messages,latest}=>{self.load_avatars(messages.iter().filter_map(|m|m.author_id).collect(),false,sender.clone());self.history_observed(channel_id,messages,latest,&sender);},
            MainWindowMsg::LiveRead{request,channel_id,result}=>{
                if self.notifications.read_request!=Some(request){return;}self.notifications.read_request=None;
                match result {Ok(page)=>{self.history_observed(channel_id,page.messages.clone(),true,&sender);if self.active_channel_id==Some(channel_id){self.chat.emit(ChatMsg::SnapshotLoaded{request,messages:Some(page.messages)});}},Err(e)=>{self.chat.emit(ChatMsg::SnapshotLoaded{request,messages:None});self.notifications.read_pending=true;sender.input(MainWindowMsg::OperationFailed(e));}}
            }
            MainWindowMsg::Security(msg) => self.security_event(msg,&sender,root),
            MainWindowMsg::Account(msg) => {self.account_event(msg,&sender,root);self.deliver_notices(sender.clone());self.render_inbox(&sender);},
            MainWindowMsg::AccessLoaded { request_id, result } => {
                if self.access_request != Some(request_id) { return; }
                self.access_request = None;
                if std::mem::take(&mut self.access_refresh_again) { self.refresh_channels(sender.clone()); }
                match result {
                    Ok(AccessSnapshot { user, server, roles, channels, access, server_access, read_version, member_version }) => {
                        let mut user=user;
                        if self.members.changed_after(user.id,member_version){
                            user.nickname=self.current_user.nickname.clone();user.status=self.current_user.status.clone();user.status_message=self.current_user.status_message.clone();user.typing=self.current_user.typing.clone();user.avatar_blob=self.current_user.avatar_blob.clone();user.avatar_format=self.current_user.avatar_format.clone();user.status_updated_at=self.current_user.status_updated_at;
                        }
                        user.settings=Some(crate::models::WhoamiSettings {version:1,config:self.account.config.clone()});
                        user.connection_violation=Some(user.connection_violation.unwrap_or(false)||self.current_user.connection_violation.unwrap_or(false));
                        self.current_user = user;
                        if !self.background_started {
                            self.background_started = true;
                            self.refresh_notifications(None, sender.clone());
                            self.refresh_direct(sender.clone());
                            self.refresh_users(sender.clone(), None);
                        }
                        self.sidebar.emit(SidebarMsg::SetCurrentUser(self.current_user.clone()));
                        self.roles=roles;self.managed_channels=channels.clone();self.server_access=server_access.clone();self.setup=server.is_none();
                        self.sidebar.emit(SidebarMsg::SetManagement{access:server_access.clone(),setup:self.setup});
                        self.sync_admin_access();self.sync_moderation_access();
                        if self.administration.reopen{self.administration.reopen=false;sender.input(MainWindowMsg::Administration(AdminMsg::Open));}
                        let Some(server)=server else{self.server=None;if !self.admin_visible(){sender.input(MainWindowMsg::Administration(AdminMsg::Open));}return;};
                        self.server = Some(server.clone());
                        self.sidebar.emit(SidebarMsg::SetServer(server));
                        self.access = access;
                        self.channels = channels.into_iter().filter(|c| self.access.get(&c.id).is_some_and(|a| a.read)).collect();
                        self.channels.sort_by_key(|c|c.position.unwrap_or(0));
                        self.notifications.journal.reconcile(read_version,&mut self.channels);
                        self.publish_voice_channels();
                        self.sync_voice_access();
                        self.admin_refresh_lists(&sender);
                        self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);
                        if let Some(channel) = self.channels.iter().find(|c| Some(c.id) == self.active_channel_id) {
                            self.chat.emit(ChatMsg::SetAccess { user_id: self.current_user.id, access: self.access[&channel.id].clone() });
                            self.chat.emit(ChatMsg::UpdateChannel(channel.clone()));
                        } else if !self.active_channel_id.is_some_and(|id|self.direct.items.iter().any(|d|d.id==id)) {
                            self.active_channel_id = None;
                            self.preview_requests.clear();
                            self.chat.emit(ChatMsg::ClearChannel);
                            self.chat.emit(ChatMsg::SetAccess { user_id: self.current_user.id, access: server_access });
                            // A slow DM open owns navigation until it completes.
                            // Auto-selection here would cancel its request token.
                            if !self.direct_selection_pending() {
                                if let Some(channel) = self.channels.iter().find(|c| matches!(c.channel_type, None | Some(crate::models::ChannelType::Text))).cloned() {
                                    self.select_channel(channel, sender.clone());
                                }
                            }
                        } else {self.chat.emit(ChatMsg::SetAccess{user_id:self.current_user.id,access:self.direct_access()});}
                    }
                    Err(error) => {
                        self.access.clear();self.server_access=Default::default();self.sync_admin_access();self.sync_moderation_access();self.sync_voice_access();self.sidebar.emit(SidebarMsg::SetManagement{access:Default::default(),setup:false});
                        self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);
                        self.chat.emit(ChatMsg::SetAccess { user_id: self.current_user.id, access: Default::default() });
                        if crate::api::is_session_error(&error) { self.voice_disconnected();let _ = sender.output(MainWindowOutput::SessionExpired); }
                        else { self.chat.emit(ChatMsg::OperationError(format!("Não foi possível atualizar permissões: {error}"))); }
                    }
                }
            }
            MainWindowMsg::FlushMembers => {
                self.members.debounce=false;
                let ids:Vec<_>=self.members.queued.drain().collect();
                for ids in ids.chunks(50){self.fetch_members(ids.to_vec(),sender.clone());}
            }
            MainWindowMsg::MembersLoaded { token, full, ids, result } => {
                let (users,error)=match result{Ok(users)=>(Some(users),None),Err(error)=>(None,Some(error))};
                if self.members.finish(token,full,&ids,users,&mut self.users){
                    self.publish_users();self.render_inbox(&sender);
                    self.load_avatars(self.users.iter().map(|u|u.id).collect(),false,sender.clone());
                }
                if full&&std::mem::take(&mut self.members.full_again){self.refresh_users(sender.clone(),None);}
                if let Some(error)=error{sender.input(MainWindowMsg::OperationFailed(error));}
            }
            MainWindowMsg::AvatarsLoaded { request_id, ids, result } => {
                match result {
                    Ok(profiles) => {
                        if self.avatars.finish(request_id, &ids, profiles) {
                            self.publish_avatars();self.render_inbox(&sender);
                        }
                    }
                    Err(error) => {
                        self.avatars.fail(request_id, &ids);
                        if crate::api::is_session_error(&error) { sender.input(MainWindowMsg::OperationFailed(error)); }
                        else { tracing::warn!("Failed to load member avatars: {error}"); }
                    }
                }
            }
            MainWindowMsg::RefreshAccess => self.refresh_channels(sender),
            MainWindowMsg::ActionError(error) => {
                if crate::api::is_session_error(&error) { self.voice_disconnected();let _ = sender.output(MainWindowOutput::SessionExpired); }
                else if crate::api::is_permission_error(&error) { self.direct_denied(sender.clone()); self.refresh_channels(sender); }
            }
            MainWindowMsg::Navigate { channel_id, message_id } => {
                if self.direct.items.iter().any(|d|d.id==channel_id){sender.input(MainWindowMsg::Direct(DirectMsg::Select(channel_id,Some(message_id))));return;}
                if !self.access.get(&channel_id).is_some_and(|a|a.read){self.chat.emit(ChatMsg::OperationError("Canal não disponível ou sem permissão.".into()));return;}
                if let Some(channel) = self.channels.iter().find(|c| c.id == channel_id).cloned() {
                    if !matches!(channel.channel_type, None | Some(crate::models::ChannelType::Text)) {
                        self.chat.emit(ChatMsg::OperationError("Este canal não possui uma visualização de mensagens disponível.".into()));
                        return;
                    }
                    if self.active_channel_id != Some(channel_id) {
                        // Same queue: selection/history begins before navigation.
                        sender.input(MainWindowMsg::ChannelSelected(channel));
                        sender.input(MainWindowMsg::Navigate { channel_id, message_id });
                    } else { self.chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::Navigate(message_id))); }
                } else { self.chat.emit(ChatMsg::OperationError("Canal não disponível ou sem permissão.".into())); }
            }
            MainWindowMsg::OperationFailed(error) => {
                if crate::api::is_session_error(&error) {
                    self.voice_disconnected();let _ = sender.output(MainWindowOutput::SessionExpired);
                } else {
                    if crate::api::is_permission_error(&error) { self.direct_denied(sender.clone());self.refresh_channels(sender.clone()); }
                    self.chat.emit(ChatMsg::OperationError(error.to_string()));
                }
            }
            MainWindowMsg::UserActivity => self.mark_activity(),
            MainWindowMsg::ChannelSelected(channel) => self.select_channel(channel, sender),
            MainWindowMsg::SendTyping(channel_id) => {
                let _ = self.ws_tx.try_send(serde_json::json!({ "type": "typing", "channel_id": channel_id }).to_string().into());
            }
            MainWindowMsg::LoadMoreMessages { channel_id, cursor } => {
                if self.active_channel_id == Some(channel_id) { self.load_history(channel_id, cursor, sender); }
            }
            MainWindowMsg::ToggleUserList => self.layout.members.set_show_sidebar(!self.layout.members.shows_sidebar()),
            MainWindowMsg::Logout => { self.stop_voice(true,"Saindo…"); let _ = sender.output(MainWindowOutput::Logout); }
            MainWindowMsg::TickCleanup => {
                self.voice_tick();
                self.chat.emit(ChatMsg::ClearStaleTyping);
                if self.background_started { self.tick_direct(sender.clone()); }
                if self.startup_started && self.last_access_refresh.map_or(true,|t|t.elapsed()>=Duration::from_secs(30))&&self.access_request.is_none(){self.refresh_channels(sender.clone());}
                if self.notifications.read_pending&&self.notifications.read_request.is_none()&&root.root().and_downcast::<gtk::Window>().is_some_and(|w|w.is_active()){
                    if let Some(channel_id)=self.active_channel_id.filter(|id|self.can_read_target(*id)){
                        let request=Uuid::new_v4();self.notifications.read_request=Some(request);self.notifications.read_pending=false;
                        self.chat.emit(ChatMsg::BeginSnapshot(request));
                        let api=self.api_client.clone();self.notifications.jobs.push(tokio::spawn(async move{let result=api.list_messages(channel_id,None).await;sender.input(MainWindowMsg::LiveRead{request,channel_id,result});}));
                    }
                }
            },
            MainWindowMsg::PreviewLoaded { channel_id, message_id, preview_id, request_id, result } => {
                if self.active_channel_id == Some(channel_id) && self.preview_requests.get(&(message_id, preview_id)) == Some(&request_id) {
                    self.preview_requests.remove(&(message_id, preview_id));
                    match result {
                        Ok(preview) if preview.id == preview_id => self.chat.emit(ChatMsg::ApplyChange(Change::Preview(message_id, preview))),
                        Ok(_) => {},
                        Err(error) => sender.input(MainWindowMsg::OperationFailed(error)),
                    }
                }
            }
            MainWindowMsg::WsReceived(event) => match event {
                WsEvent::Voice(event)=>self.voice_received(event,sender),
                WsEvent::ConnectionReady(id)=>{if !self.startup_started { self.startup_on_socket=true; self.start_loading(sender.clone()); } if self.voice.connection!=Some(id){self.voice_disconnected();self.voice.connection=Some(id);self.render_voice();}},
                WsEvent::Disconnected=>self.voice_disconnected(),
                WsEvent::Error { message, code } => {
                    if self.voice_error(&message,code.as_deref()){return;}
                    self.chat.emit(ChatMsg::OperationError(match code { Some(code) => format!("{message} ({code})"), None => message }));
                    self.refresh_channels(sender);
                }
                WsEvent::SessionExpired => { self.voice_disconnected(); let _ = sender.output(MainWindowOutput::SessionExpired); }
                WsEvent::Reconnected => {
                    if std::mem::take(&mut self.startup_on_socket) { return; }
                    self.last_activity = None;
                    self.refresh_notifications(None,sender.clone());
                    self.refresh_direct(sender.clone());
                    if self.active_channel_id.is_some(){self.chat.emit(ChatMsg::Resync);}
                    self.refresh_channels(sender.clone());
                    self.chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::Reload));
                    self.chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::ReloadEmojis));
                    self.load_avatars(self.users.iter().map(|user| user.id).collect(), true, sender.clone());
                    self.refresh_users(sender, None);
                }
                WsEvent::MessagePin { message_id, is_pinned } => self.chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::PinEvent { id: message_id, pinned: is_pinned })),
                WsEvent::ReactionUpdate { message_id, reaction } => {
                    self.chat.emit(ChatMsg::ApplyChange(Change::Reaction(message_id, reaction)));
                    self.chat.emit(ChatMsg::Action(crate::ui::chat::actions::ActionMsg::Reconcile(message_id)));
                },
                WsEvent::DirectUpdate(dm)=>self.direct_snapshot(dm,&sender),
                WsEvent::NewNotification(event)=>self.incoming_notice(event,sender),
                WsEvent::NewMessage(message) => {
                    if self.can_read_target(message.channel_id){
                        self.direct_message(&message);
                        self.notifications.journal.message(&message,&mut self.channels);self.notifications.inbox.observe(message.clone());
                        self.publish_voice_channels();
                        self.deliver_notices(sender.clone());self.render_inbox(&sender);
                        if Some(message.channel_id) == self.active_channel_id {if let Some(id)=message.author_id{self.load_avatars(vec![id],false,sender.clone());}self.notifications.read_pending=true;self.chat.emit(ChatMsg::AddMessage(message));}
                    } else {self.notifications.inbox.observe(message);self.refresh_direct(sender);}
                }
                WsEvent::MessageEdit { id, content, channel_id, edited_at } => {
                    if channel_id.is_none() || channel_id == self.active_channel_id {
                        self.chat.emit(ChatMsg::ApplyChange(Change::Edit(id, content, edited_at)));
                    }
                }
                WsEvent::MessageDelete { id, .. } => {self.notifications.inbox.delete(id);self.chat.emit(ChatMsg::DeleteMessage(id));self.render_inbox(&sender);self.deliver_notices(sender.clone());if self.channels.iter().any(|c|c.last_message.as_ref().is_some_and(|m|m.id==id)){self.refresh_channels(sender);}},
                WsEvent::Typing { user_id, channel_id, is_typing } => {
                    if user_id != self.current_user.id && Some(channel_id) == self.active_channel_id {
                        self.chat.emit(ChatMsg::UserTyping { user_id, is_typing });
                    }
                }
                WsEvent::PresenceSync(entries) => { self.voice.presence.clear();for p in &entries{self.voice_presence(p);}self.direct.presence=entries.iter().map(|e|(e.user_id,e.status)).collect();self.sidebar.emit(SidebarMsg::Presence(self.direct.presence.clone()));self.user_list.emit(UserListMsg::PresenceSync(entries));},
                WsEvent::PresenceUpdate(entry) => { self.voice_presence(&entry);
                    let id = entry.user_id;
                    self.direct.presence.insert(id,entry.status);self.sidebar.emit(SidebarMsg::Presence(self.direct.presence.clone()));
                    let previous=self.user_list.model().presence.get(&id).cloned();
                    let profile_changed=entry.online()&&self.users.iter_mut().find(|u|u.id==id).is_some_and(|user|{
                        // Optional fields are omitted by several presence events.
                        // Confirm removals through a coalesced summary fetch;
                        // a sparse status/voice update must not erase a nickname.
                        let changed=entry.nickname.as_ref().is_some_and(|v|Some(v)!=user.nickname.as_ref())
                            ||entry.status_message.as_ref().is_some_and(|v|Some(v)!=user.status_message.as_ref())
                            ||entry.typing.as_ref().is_some_and(|v|Some(v)!=user.typing.as_ref())
                            ||previous.as_ref().is_some_and(|old|(old.nickname.is_some()&&entry.nickname.is_none())||(old.status_message.is_some()&&entry.status_message.is_none())||(old.typing.is_some()&&entry.typing.is_none()));
                        if let Some(value)=&entry.nickname{user.nickname=Some(value.clone());}
                        if let Some(value)=&entry.status_message{user.status_message=Some(value.clone());}
                        if let Some(value)=&entry.typing{user.typing=Some(value.clone());}
                        changed
                    });
                    self.user_list.emit(UserListMsg::UpdatePresence(entry));
                    sender.input(MainWindowMsg::Account(AccountMsg::RefreshProfile(id)));
                    if profile_changed{self.publish_users();self.refresh_users(sender,Some(id));}
                }
                WsEvent::AvatarUpdate { user_id } => {
                    sender.input(MainWindowMsg::Account(AccountMsg::RefreshProfile(user_id)));
                    self.load_avatars(vec![user_id], true, sender.clone());
                    self.refresh_users(sender, Some(user_id));
                }
                WsEvent::UserJoin { user_id } => self.refresh_users(sender, Some(user_id)),
                WsEvent::RoleAdd { user_id, .. } | WsEvent::RoleRemove { user_id, .. } => {
                    sender.input(MainWindowMsg::Account(AccountMsg::RefreshProfile(user_id)));
                    if user_id == self.current_user.id {
                        self.access.clear();self.server_access=Default::default();self.sync_admin_access();self.sync_moderation_access();self.sync_voice_access();self.sidebar.emit(SidebarMsg::SetManagement{access:Default::default(),setup:false});
                        self.chat.emit(ChatMsg::SetAccess { user_id, access: Default::default() });
                        self.deliver_notices(sender.clone());self.render_inbox(&sender);self.render_search(&sender);
                    }
                    self.refresh_users(sender.clone(), Some(user_id));
                    self.admin_refresh_assignments(sender.clone());
                    self.refresh_channels(sender);
                }
                WsEvent::ChannelsChanged => self.refresh_channels(sender),
                WsEvent::NewPreview { message_id, preview_id } => {
                    if let Some(channel_id) = self.active_channel_id {
                        let request_id = Uuid::new_v4();
                        self.preview_requests.insert((message_id, preview_id), request_id);
                        let client = self.api_client.clone();
                        tokio::spawn(async move {
                            let result = client.get_embed(preview_id).await;
                            sender.input(MainWindowMsg::PreviewLoaded { channel_id, message_id, preview_id, request_id, result });
                        });
                    }
                }
                WsEvent::RemovePreview { message_id, preview_id } => {
                    self.preview_requests.remove(&(message_id, preview_id));
                    self.chat.emit(ChatMsg::ApplyChange(Change::RemovePreview(message_id, preview_id)));
                }
                WsEvent::EmbedsUpdate {channel_id,message_id,embeds}=>{
                    if Some(channel_id)==self.active_channel_id&&self.can_read_target(channel_id){self.chat.emit(ChatMsg::ApplyChange(Change::Embeds(message_id,embeds)));}
                },
                WsEvent::PreviewUpdate { channel_id, message_id, preview } => {
                    self.preview_requests.remove(&(message_id, preview.id));
                    if Some(channel_id) == self.active_channel_id { self.chat.emit(ChatMsg::ApplyChange(Change::Preview(message_id, preview))); }
                }
                WsEvent::AttachmentModeration { channel_id, message_id, attachment_id, status } => {
                    if Some(channel_id) == self.active_channel_id { self.chat.emit(ChatMsg::ApplyChange(Change::Moderation(message_id, attachment_id, status))); }
                }
            },
        }
    }
}

impl MainWindowModel {
    fn publish_avatars(&self) {
        self.chat.emit(ChatMsg::SetAvatars(self.avatars.textures.clone()));
        self.user_list.emit(UserListMsg::SetAvatars(self.avatars.textures.clone()));
        self.sidebar.emit(SidebarMsg::PeerAvatars(self.avatars.textures.clone()));
        self.sidebar.emit(SidebarMsg::SetAvatar(self.avatars.textures.get(&self.current_user.id).cloned()));
    }

    fn publish_users(&mut self) {
        if let Some(u)=self.users.iter().find(|u|u.id==self.current_user.id){self.current_user.nickname=u.nickname.clone();self.current_user.status=u.status.clone();self.current_user.status_message=u.status_message.clone();self.current_user.typing=u.typing.clone();self.sidebar.emit(SidebarMsg::SetCurrentUser(self.current_user.clone()));}
        for dm in &mut self.direct.items {if let Some(u)=self.users.iter().find(|u|u.id==dm.user.id){dm.user=u.clone();}}self.publish_direct();
        self.chat.emit(ChatMsg::SetUsers(self.users.clone()));
        self.user_list.emit(UserListMsg::SetUsers(self.users.clone()));
        self.admin_member_summary();self.render_voice();
    }

    fn mark_activity(&mut self) {
        if self.last_activity.map_or(true, |last| last.elapsed() >= Duration::from_secs(30)) {
            if self.ws_tx.try_send(r#"{"type":"presence_activity"}"#.into()).is_ok() {
                self.last_activity = Some(Instant::now());
            }
        }
    }

    fn select_channel(&mut self, channel: Channel, sender: ComponentSender<Self>) {
        if !matches!(channel.channel_type, None | Some(crate::models::ChannelType::Text)) { return; }
        let channel_id = channel.id;
        let Some(access) = self.access.get(&channel_id).filter(|a| a.read).cloned() else { return; };
        self.chat.emit(ChatMsg::SetAccess { user_id: self.current_user.id, access });
        if self.active_channel_id == Some(channel_id) { return; }
        self.direct_cancel_selection();
        self.active_channel_id = Some(channel_id);
        self.sidebar.emit(SidebarMsg::SetSelection(channel_id));
        self.notifications.read_pending=false;
        self.preview_requests.clear();
        self.chat.emit(ChatMsg::SetChannel(channel));
        self.load_history(channel_id, None, sender.clone());
    }

    fn start_loading(&mut self, sender: ComponentSender<Self>) {
        if self.startup_started { return; }
        self.startup_started = true;
        self.refresh_channels(sender);
    }

    fn refresh_channels(&mut self, sender: ComponentSender<Self>) {
        if self.access_request.is_some() { self.access_refresh_again = true; return; }
        self.last_access_refresh=Some(Instant::now());
        let request_id = Uuid::new_v4();
        self.access_request = Some(request_id);
        let client = self.api_client.clone();
        let read_version=self.notifications.journal.version;
        let member_version=self.members.version();
        tokio::spawn(async move {
            let result = async {
                let user = client.whoami().await?;
                let server = match client.get_server().await {
                    Ok(s)=>Some(s),Err(e)if e.downcast_ref::<crate::api::ApiError>().is_some_and(|e|e.status==reqwest::StatusCode::NOT_FOUND)=>None,Err(e)=>return Err(e),
                };
                if server.is_none(){return Ok(AccessSnapshot{user,server,roles:vec![],channels:vec![],access:HashMap::new(),server_access:Default::default(),read_version,member_version});}
                let roles = client.list_roles().await?;
                let mut channels = client.channels_with_permissions().await?;
                let mut access = HashMap::new();
                for channel in &mut channels {
                    let permissions = channel.permissions.as_deref().unwrap_or(&[]);
                    access.insert(channel.id, crate::models::Access::resolve(user.id, server.as_ref().and_then(|s|s.owner_id),
                        user.roles.as_deref().unwrap_or(&[]), &roles, permissions));
                }
                let server_access = crate::models::Access::resolve(user.id, server.as_ref().and_then(|s|s.owner_id),
                    user.roles.as_deref().unwrap_or(&[]), &roles, &[]).without_channel();
                Ok(AccessSnapshot { user, server, roles, channels, access, server_access, read_version, member_version })
            }.await;
            sender.input(MainWindowMsg::AccessLoaded { request_id, result });
        });
    }

    fn refresh_users(&mut self, sender: ComponentSender<Self>, id: Option<Uuid>) {
        self.members.jobs.retain(|j|!j.is_finished());
        if let Some(id)=id{
            self.members.invalidate(id);self.members.queued.insert(id);
            if !self.members.debounce{
                self.members.debounce=true;
                self.members.jobs.push(tokio::spawn(async move{tokio::time::sleep(Duration::from_millis(250)).await;sender.input(MainWindowMsg::FlushMembers);}));
            }
        }else if let Some((token,_))=self.members.begin_full(){
            let client=self.api_client.clone();
            self.members.jobs.push(tokio::spawn(async move{let result=client.list_all_users().await;sender.input(MainWindowMsg::MembersLoaded{token,full:true,ids:vec![],result});}));
        }
    }
    fn fetch_members(&mut self,ids:Vec<Uuid>,sender:ComponentSender<Self>){
        let token=self.members.begin(&ids);let client=self.api_client.clone();
        self.members.jobs.push(tokio::spawn(async move{let result=client.user_summaries(ids.clone()).await;sender.input(MainWindowMsg::MembersLoaded{token,full:false,ids,result});}));
    }

    fn load_avatar_blob(&mut self,id:Uuid,blob:Option<String>,sender:ComponentSender<Self>){
        if let Some((request_id,ids))=self.avatars.begin(&[id],true){
            self.members.jobs.push(tokio::spawn(async move{let image=crate::media::avatars::prepare_blob(blob,crate::media::avatars::AVATAR_EDGE).await;sender.input(MainWindowMsg::AvatarsLoaded{request_id,ids,result:Ok(vec![(id,image)])});}));
        }
    }
    fn load_avatars(&mut self, ids: Vec<Uuid>, force: bool, sender: ComponentSender<Self>) {
        // Authors and online members precede offline fallback avatars. The
        // budget also bounds initial work on servers with thousands of members.
        let authors=self.chat.model().avatar_authors();
        let mut ids=ids;ids.sort_by_key(|id|(*id!=self.current_user.id,!authors.contains(id),self.direct.presence.get(id).is_none_or(|s|*s==crate::ws::PresenceStatus::Offline)));
        ids.truncate(crate::media::avatars::AVATAR_LIMIT);
        if let Some((request_id, ids)) = self.avatars.begin(&ids, force) {
            let client = self.api_client.clone();
            self.members.jobs.retain(|j|!j.is_finished());
            self.members.jobs.push(tokio::spawn(async move {
                for (batch,chunk) in ids.chunks(16).enumerate(){
                    let result=match client.user_profiles(chunk).await{Ok(profiles)=>Ok(crate::media::avatars::prepare_profiles(profiles).await),Err(e)=>Err(e)};
                    let failed=result.is_err();
                    sender.input(MainWindowMsg::AvatarsLoaded { request_id, ids:if failed{ids[batch*16..].to_vec()}else{chunk.to_vec()}, result });
                    if failed{break;}
                }
            }));
        }
    }

    fn load_history(&self, channel_id: Uuid, cursor: Option<MessageCursor>, sender: ComponentSender<Self>) {
        let request_id = Uuid::new_v4();
        self.chat.emit(ChatMsg::BeginHistory { request_id, cursor });
        let client = self.api_client.clone();
        let chat_sender = self.chat.sender().clone();
        tokio::spawn(async move {
            let result = client.list_messages(channel_id, cursor).await.map_err(|error| {
                let message = error.to_string();
                if crate::api::is_session_error(&error) || crate::api::is_permission_error(&error) { sender.input(MainWindowMsg::OperationFailed(error)); }
                message
            });
            let _ = chat_sender.send(ChatMsg::HistoryLoaded { request_id, result, append: cursor.is_some() });
        });
    }
}

impl Drop for MainWindowModel {
    fn drop(&mut self) {
        if let Some(task) = self.refresh_task.take() { task.abort(); }
        if let Some(task) = self.ws_listener.take() { task.abort(); }
    }
}
