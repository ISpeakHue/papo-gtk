//! Root component — manages application views (Login / Main).

use adw::prelude::*;
use gtk::prelude::*;
use relm4::prelude::*;
use tracing::info;

use crate::api::ApiClient;
use crate::config::AppConfig;
use crate::models::WhoamiResponse;
use crate::ui::login::{LoginModel, LoginOutput, LoginMsg};
use crate::ui::main_window::{MainWindowInit, MainWindowModel, MainWindowOutput};

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
enum View {
    Login,
    Main,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut widgets = vec![widget.clone()];
        let mut child = widget.first_child();
        while let Some(current) = child {
            widgets.extend(descendants(&current));
            child = current.next_sibling();
        }
        widgets
    }

    #[test]
    #[ignore = "requires a GTK display; CI runs this under Xvfb"]
    fn ui_smoke_window_has_header_controls_and_server_password() {
        adw::init().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let controller = RootModel::builder().launch(()).detach();
        let window = controller.widget();
        window.present();
        let context = gtk::glib::MainContext::default();
        for _ in 0..100 {
            if !context.pending() { break; }
            context.iteration(false);
        }
        assert!(window.content().unwrap().is::<adw::ToolbarView>());
        // AdwApplicationWindow also owns a hidden internal title bar. Inspect
        // the application's content, not that implementation detail.
        let widgets = descendants(&window.content().unwrap());
        let header = widgets.iter().find_map(|widget| widget.downcast_ref::<adw::HeaderBar>()).unwrap();
        assert!(header.is_visible());
        assert!(header.shows_start_title_buttons());
        assert!(header.shows_end_title_buttons());
        let stack = widgets.iter().find_map(|widget| widget.downcast_ref::<gtk::Stack>()).unwrap();
        assert_eq!(stack.visible_child_name().as_deref(), Some("login"));
        let passwords: Vec<_> = widgets.iter()
            .filter_map(|widget| widget.downcast_ref::<adw::PasswordEntryRow>()).collect();
        assert_eq!(passwords.len(), 2);
        assert!(passwords.iter().any(|row| row.title().starts_with("Senha do servidor") && row.is_visible()));
        assert!(passwords.iter().any(|row| row.title() == "Senha"));
        crate::ui::login::tests::exercise_input(&controller.model().login_controller, &context);
        // Exercise chat pagination controls with the actual component, without a backend.
        use crate::ui::chat::{ChatInit, ChatModel, ChatMsg, ChatOutput};
        use crate::models::{Channel, Message, MessageListResponse};
        use uuid::Uuid;
        use std::{cell::RefCell, rc::Rc};
        let drain = || {
            for _ in 0..500 {
                if !context.pending() { break; }
                context.iteration(false);
            }
        };
        let outputs = Rc::new(RefCell::new(Vec::new()));
        let captured = outputs.clone();
        let chat = ChatModel::builder().launch(ChatInit::default())
            .connect_receiver(move |_, output| captured.borrow_mut().push(output));
        let channel_id = Uuid::new_v4();
        let channel: Channel = serde_json::from_value(serde_json::json!({
            "id":channel_id,"name":"general","channel_type":"text","position":0,
            "created_at":"2026-10-03T12:00:00Z"
        })).unwrap();
        let message: Message = serde_json::from_value(serde_json::json!({
            "id":Uuid::new_v4(),"channel_id":channel_id,"author_id":channel_id,"content":"hello",
            "created_at":"2026-10-03T12:00:00Z"
        })).unwrap();
        let request_id = Uuid::new_v4();
        chat.emit(ChatMsg::SetChannel(channel));
        chat.emit(ChatMsg::SetAccess{user_id:channel_id,access:crate::models::Access{read:true,..Default::default()}});
        chat.emit(ChatMsg::BeginHistory { request_id, cursor:None });
        chat.emit(ChatMsg::HistoryLoaded { request_id, append:false, result:Ok(MessageListResponse {
            channel_id, messages:vec![message.clone()], has_more:true,
        }) });
        drain();
        let chat_widgets = descendants(chat.widget().upcast_ref());
        assert!(!chat_widgets.iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Carregar mensagens anteriores")));
        chat.emit(ChatMsg::LoadOlder);
        drain();
        assert!(outputs.borrow().iter().any(|output| matches!(output,
            ChatOutput::LoadMoreMessages { channel_id:id, cursor:Some(cursor) }
            if *id == channel_id && cursor.id == message.id && cursor.created_at == message.created_at)));
        let request_id = Uuid::new_v4();
        chat.emit(ChatMsg::BeginHistory { request_id, cursor:Some((&message).into()) });
        drain();
        let count=outputs.borrow().len();chat.emit(ChatMsg::LoadOlder);drain();assert_eq!(outputs.borrow().len(),count);
        chat.emit(ChatMsg::HistoryLoaded { request_id, append:true, result:Err("offline".into()) });
        drain();
        let retry = chat_widgets.iter().filter_map(|widget| widget.downcast_ref::<gtk::Button>())
            .find(|button| button.label().as_deref() == Some("Tentar novamente")).unwrap();
        assert!(retry.is_visible() && retry.is_sensitive());
        chat.emit(ChatMsg::ClearChannel);
        drain();
        assert!(!retry.is_visible());
        assert!(!chat_widgets.iter().find_map(|widget| widget.downcast_ref::<gtk::Entry>()).unwrap().is_sensitive());

        // Presence snapshots received before member lists must still render away/busy.
        use crate::ui::user_list::{UserListInit, UserListModel, UserListMsg};
        use crate::models::UserSummary;
        let members = UserListModel::builder().launch(UserListInit::default()).detach();
        let entry = serde_json::from_value(serde_json::json!({"user_id":channel_id,"status":"away"})).unwrap();
        let user: UserSummary = serde_json::from_value(serde_json::json!({
            "id":channel_id,"username":"alice","created_at":"2026-10-03T12:00:00Z"
        })).unwrap();
        members.emit(UserListMsg::PresenceSync(vec![entry]));
        members.emit(UserListMsg::SetUsers(vec![user]));
        drain();
        assert!(descendants(members.widget().upcast_ref()).iter()
            .filter_map(|widget| widget.downcast_ref::<gtk::Image>())
            .any(|image| image.icon_name().as_deref() == Some("user-idle-symbolic")));

        // Full profiles supply cached textures in chat and member rows. Avatar
        // changes replace those textures; removal/corrupt data restores icons.
        use crate::media::avatars::AvatarCache;
        use crate::models::UserProfile;
        use base64::prelude::*;
        let mut avatar_cache = AvatarCache::default();
        chat.emit(ChatMsg::SetChannel(serde_json::from_value(serde_json::json!({
            "id":channel_id,"name":"general","channel_type":"text","position":0,
            "created_at":"2026-10-03T12:00:00Z"
        })).unwrap()));
        chat.emit(ChatMsg::AddMessage(message.clone()));
        let assert_picture = |root: &gtk::Widget, size, texture: &gtk::gdk::Texture| {
            assert!(descendants(root).iter().filter_map(|widget| widget.downcast_ref::<adw::Avatar>())
                .any(|image| image.size() == size && image.custom_image().as_ref() == Some(texture.upcast_ref())
                    && image.measure(gtk::Orientation::Horizontal, -1).1 == size));
        };
        let assert_fallback = |root: &gtk::Widget, size| {
            assert!(descendants(root).iter().filter_map(|widget| widget.downcast_ref::<adw::Avatar>())
                .any(|image| image.size() == size && image.custom_image().is_none()
                    && image.icon_name().as_deref() == Some("avatar-default-symbolic")));
        };
        for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg,
            image::ImageFormat::Gif, image::ImageFormat::WebP] {
            let dimension = if format == image::ImageFormat::Png { 256 } else { 2 };
            let pixels = image::RgbImage::from_pixel(dimension, dimension, image::Rgb([255, 0, 0]));
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(pixels).write_to(&mut bytes, format).unwrap();
            let blob = BASE64_STANDARD.encode(bytes.into_inner());
            let profile: UserProfile = serde_json::from_value(serde_json::json!({
                "id":channel_id,"username":"alice","created_at":"2026-10-03T12:00:00Z",
                "avatar_blob":blob
            })).unwrap();
            let (request, ids) = avatar_cache.begin(&[channel_id], true).unwrap();
            assert!(avatar_cache.finish(request, &ids, vec![profile]));
            let texture = avatar_cache.textures[&channel_id].clone();
            assert_eq!(texture.width(), dimension as i32);
            chat.emit(ChatMsg::SetAvatars(avatar_cache.textures.clone()));
            members.emit(UserListMsg::SetAvatars(avatar_cache.textures.clone()));
            drain();
            assert_picture(chat.widget().upcast_ref(), 36, &texture);
            assert_picture(members.widget().upcast_ref(), 24, &texture);
        }
        // A stale initial batch cannot undo a newer avatar removal.
        let (old_request, ids) = avatar_cache.begin(&[channel_id], true).unwrap();
        let (new_request, _) = avatar_cache.begin(&[channel_id], true).unwrap();
        assert!(avatar_cache.finish(new_request, &ids, Vec::new()));
        assert!(!avatar_cache.finish(old_request, &ids, Vec::new()));
        for blob in [serde_json::Value::Null, serde_json::json!("invalid-base64!")] {
            let profile = serde_json::from_value(serde_json::json!({
                "id":channel_id,"username":"alice","created_at":"2026-10-03T12:00:00Z","avatar_blob":blob
            })).unwrap();
            let (request, ids) = avatar_cache.begin(&[channel_id], true).unwrap();
            avatar_cache.finish(request, &ids, vec![profile]);
            chat.emit(ChatMsg::SetAvatars(avatar_cache.textures.clone()));
            members.emit(UserListMsg::SetAvatars(avatar_cache.textures.clone()));
            drain();
            assert_fallback(chat.widget().upcast_ref(), 36);
            assert_fallback(members.widget().upcast_ref(), 24);
        }

        crate::ui::chat::actions::tests::exercise(&context);
        crate::ui::chat::bugs::exercise(&context);
        crate::media::sound::exercise(&context);

        // Authenticating must actually switch the watched stack to the main view.
        let whoami = serde_json::from_value(serde_json::json!({
            "id":channel_id,"username":"alice","created_at":"2026-10-03T12:00:00Z"
        })).unwrap();
        controller.emit(RootMsg::Authenticated {
            whoami, client:ApiClient::new("http://127.0.0.1:0").unwrap(),
        });
        drain();
        assert_eq!(controller.model().view, View::Main);
        assert_eq!(stack.visible_child_name().as_deref(), Some("main"));
        assert!(controller.model().main_controller.is_some());
        assert!(stack.measure(gtk::Orientation::Horizontal,-1).0<=360,"the hidden login form must not restrict the adaptive chat width");
        // Revoked sessions return to login with an actionable message.
        controller.emit(RootMsg::SessionExpired);
        drain();
        assert_eq!(controller.model().view, View::Login);
        assert_eq!(stack.visible_child_name().as_deref(), Some("login"));
        assert!(controller.model().main_controller.is_none());
        assert!(controller.model().current_user.is_none());
        assert!(descendants(&window.content().unwrap()).iter()
            .filter_map(|widget| widget.downcast_ref::<gtk::Label>())
            .any(|label| label.text().contains("Sua sessão terminou")));
        window.close();
    }
}

pub struct RootModel {
    view: View,
    config: AppConfig,
    login_controller: Controller<LoginModel>,
    main_controller: Option<Controller<MainWindowModel>>,
    // Authenticated session state
    current_user: Option<WhoamiResponse>,
    api_client: Option<ApiClient>,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum RootMsg {
    /// Received from Login component upon successful auth
    Authenticated {
        whoami: WhoamiResponse,
        client: ApiClient,
    },
    /// Log out and return to the login screen
    Logout,
    SessionExpired,
}

// ── Relm4 Component ───────────────────────────────────────────────────────────

#[relm4::component(pub)]
impl Component for RootModel {
    type Init = ();
    type Input = RootMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Papo"),
            set_default_size: (1280, 760),

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    set_show_start_title_buttons: true,
                    set_show_end_title_buttons: true,
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: "Papo",
                        #[watch]
                        set_subtitle: model.current_user.as_ref()
                            .map(|user| user.username.as_str()).unwrap_or(""),
                    },
                },

                #[name = "stack"]
                #[wrap(Some)]
                set_content = &gtk::Stack {
                    set_vexpand: true,
                    set_hhomogeneous:false,set_vhomogeneous:false,
                    set_transition_type: gtk::StackTransitionType::Crossfade,
                    set_transition_duration: 250,
                    add_named[Some("login")] = model.login_controller.widget(),
                    #[watch]
                    set_visible_child_name: match model.view {
                        View::Login => "login",
                        View::Main => "main",
                    },
                },
            },
        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        crate::ui::style::install();
        let config = AppConfig::load();

        // Initialize Login child component controller
        let login_controller = LoginModel::builder()
            .launch(config.clone())
            .forward(sender.input_sender(), |output| match output {
                LoginOutput::Authenticated {
                    whoami,
                    client,
                } => RootMsg::Authenticated {
                    whoami,
                    client,
                },
            });

        let model = RootModel {
            view: View::Login,
            config,
            login_controller,
            main_controller: None,
            current_user: None,
            api_client: None,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            RootMsg::Authenticated {
                whoami,
                client,
            } => {
                info!("Root transitioning to main window for user {}", whoami.username);

                // Initialize MainWindow component
                let main_controller = MainWindowModel::builder()
                    .launch(MainWindowInit {
                        current_user: whoami.clone(),
                        api_client: client.clone(),
                    })
                    .forward(sender.input_sender(), |output| match output {
                        MainWindowOutput::Logout => RootMsg::Logout,
                        MainWindowOutput::SessionExpired => RootMsg::SessionExpired,
                    });

                if let Some(old_mc) = self.main_controller.take() {
                    widgets.stack.remove(old_mc.widget());
                } else if let Some(old_child) = widgets.stack.child_by_name("main") {
                    widgets.stack.remove(&old_child);
                }

                widgets.stack.add_named(main_controller.widget(), Some("main"));

                self.main_controller = Some(main_controller);
                self.current_user = Some(whoami);
                self.api_client = Some(client);
                self.view = View::Main;
            }
            RootMsg::SessionExpired => {
                if let (Some(client),Some(user))=(&self.api_client,&self.current_user){let client=client.clone();let user=user.username.clone();tokio::spawn(async move{let _=crate::session::forget(&client,&user).await;});}
                if let Some(controller) = self.main_controller.take() { widgets.stack.remove(controller.widget()); }
                self.current_user = None;
                self.api_client = None;
                self.view = View::Login;
                self.login_controller.emit(LoginMsg::ClearCredentials);
                self.login_controller.emit(LoginMsg::AuthFailed(
                    "Sua sessão terminou. Entre novamente e informe a senha atual do servidor, se necessário.".into()));
            }
            RootMsg::Logout => {
                info!("Root logging out");
                self.login_controller.emit(LoginMsg::ClearCredentials);
                if let Some(client) = &self.api_client {
                    let client = client.clone();
                    let user=self.current_user.as_ref().map(|u|u.username.clone());
                    tokio::spawn(async move {
                        if let Some(user)=user{let _=crate::session::forget(&client,&user).await;}
                        let _ = client.logout().await;
                    });
                }

                if let Some(mc) = self.main_controller.take() {
                    widgets.stack.remove(mc.widget());
                }

                self.current_user = None;
                self.api_client = None;
                self.view = View::Login;
            }
        }
        self.update_view(widgets, sender);
    }
}
