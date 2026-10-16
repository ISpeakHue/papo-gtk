//! Root component — manages application views (Login / Main).

use adw::prelude::*;
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
        let _report=crate::performance::Report;
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
        {
            let theme = gtk::IconTheme::new();
            theme.set_search_path(&[]);
            theme.add_resource_path("/br/com/papo/gtk/icons");
            for scale in [1, 2] {
                let icon = theme.lookup_icon(crate::app::APP_ID, &[], 128, scale,
                    gtk::TextDirection::None, gtk::IconLookupFlags::empty());
                let file = icon.file().expect("embedded app icon must resolve without installation");
                assert!(file.uri().starts_with("resource:///br/com/papo/gtk/icons/"));
                let texture = gtk::gdk::Texture::from_file(&file).unwrap();
                assert_eq!(texture.width(), 128 * scale);
                assert_eq!(texture.height(), 128 * scale);
                assert!(!icon.is_symbolic());
            }
            assert_eq!(window.icon_name().as_deref(), Some(crate::app::APP_ID));
            let widgets = descendants(&window.content().unwrap());
            let brand = widgets.iter().find(|widget| widget.widget_name() == "papo-app-icon")
                .unwrap().downcast_ref::<gtk::Image>().unwrap();
            assert_eq!(brand.icon_name().as_deref(), Some(crate::app::APP_ID));
        }
        if std::env::var("PAPO_UI_CASE").as_deref() == Ok("branding") {
            let style = adw::StyleManager::default();
            let previous = style.color_scheme();
            for (scheme, name) in [(adw::ColorScheme::ForceLight, "pelican-light"),
                (adw::ColorScheme::ForceDark, "pelican-dark")] {
                style.set_color_scheme(scheme);
                crate::ui::main_window::layout::tests::preview(window, name, &context);
            }
            style.set_color_scheme(previous);
            window.close();
            return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("notification-window"){crate::ui::main_window::notifications::exercise_page_window(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("recovery"){crate::ui::main_window::recovery::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("search-recycling"){crate::ui::main_window::search::exercise_recycling(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("avatar-viewport"){crate::ui::user_list::exercise_viewport(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("perf-baseline"){crate::ui::chat::performance::exercise_corrections(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("gnome-layout"){
            exercise_gnome_layout(&controller,&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("presence-selector"){
            crate::ui::main_window::account::exercise_presence_selector(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("voice-controls"){crate::ui::main_window::voice::tests::exercise_controls(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("pins-media"){crate::ui::chat::transfers::pins_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("call-sounds"){crate::media::sound::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("multiline"){crate::ui::chat::multiline::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("full-app-input"){crate::ui::chat::multiline::exercise_full_app(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("login-input"){
            exercise_login_input(&controller,&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("mention-background"){crate::ui::chat::mention_background::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("notification-filters"){
            crate::ui::main_window::notifications::exercise_filters(&context);crate::ui::main_window::notifications::exercise_tabs(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-categories"){crate::ui::chat::exercise_emoji_categories(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-grid"){
            crate::ui::chat::exercise_animated_emojis(&context);crate::ui::chat::bugs8::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-details"){crate::ui::chat::exercise_emoji_details(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-notifications"){crate::media::animation::exercise(&context);crate::ui::chat::exercise_animated_emojis(&context);crate::ui::main_window::notifications::exercise_tabs(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("account-notices"){crate::ui::main_window::bugs7::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("account-settings"){crate::ui::main_window::account::settings::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("webhooks"){crate::ui::main_window::webhooks::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("webhook-chat"){crate::ui::chat::transfers::exercise_external(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("backend-update"){
            crate::ui::chat::exercise_read_candidate(&context);
            crate::ui::main_window::notifications::exercise_backend_update(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("recent-media"){crate::ui::chat::transfers::exercise_recent(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-loading"){crate::ui::chat::transfers::exercise_loading(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("backend-gifs"){crate::ui::chat::transfers::exercise_backend_gifs(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("gif-embeds"){crate::ui::chat::transfers::exercise9(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report9"){crate::ui::chat::bugs9::exercise(&context);crate::ui::chat::transfers::exercise9(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report8"){
            crate::ui::user_list::exercise_corrections(&context);crate::ui::chat::bugs8::exercise(&context);crate::ui::chat::exercise_moderation(&context);crate::ui::main_window::bugs8::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("profile-emojis"){crate::ui::main_window::account::exercise_profile_emojis(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("status-viewer"){crate::ui::status::exercise(&context);crate::ui::user_list::exercise_corrections(&context);crate::ui::chat::transfers::exercise_viewer(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("attachment-media"){crate::ui::chat::transfers::exercise_attachments(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-cache"){crate::ui::chat::transfers::exercise_cache(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("draft-previews"){crate::ui::chat::exercise_draft_previews(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("clipboard"){crate::ui::chat::clipboard::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("retention"){crate::ui::chat::transfers::exercise_retention(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("mentions"){crate::ui::chat::composer::exercise(&context);crate::ui::chat::clipboard::exercise(&context);crate::ui::chat::bugs::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("mention-suggestions"){crate::ui::chat::composer::exercise_suggestions(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report7"){crate::ui::chat::bugs7::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report6"){
            crate::ui::chat::bugs6::exercise(&context);
        crate::ui::chat::bugs7::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("send-video"){
            crate::ui::chat::bugs6::exercise_send_feedback(&context);crate::ui::chat::video::exercise_fullscreen(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("startup"){
            crate::ui::chat::performance::exercise_startup(&context);crate::ui::sidebar::exercise_startup(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("scroll-media"){
            crate::ui::chat::performance::exercise_media_scroll(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("image-geometry"){
            crate::ui::chat::transfers::exercise_geometry(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("channel-cache"){
            crate::ui::chat::performance::exercise_channel_cache(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("channel-opening"){
            crate::ui::chat::performance::exercise_opening(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("smooth-updates"){
            crate::ui::chat::performance::exercise_smooth_updates(&context);
            crate::ui::chat::performance::exercise_media_scroll(&context);
            crate::ui::chat::performance::exercise_opening(&context);
            crate::ui::chat::performance::exercise_corrections(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("edit-mentions"){
            crate::ui::chat::actions::exercise_edit_mentions(&context);
            crate::ui::chat::composer::exercise(&context);
            crate::ui::chat::bugs9::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("style-palette"){crate::ui::style::exercise_accents(&context);crate::ui::chat::rich::exercise_accent_channels(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("embeds"){
            crate::ui::chat::rich::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("video-audio"){
            crate::ui::chat::video::exercise_initial_audio();window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report5"){
            crate::ui::chat::bugs5::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("cache"){
            crate::ui::chat::performance::exercise_cache(&context);window.close();return;
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
        crate::ui::style::exercise_accents(&context);
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
        assert!(!crate::ui::chat::composer::field(chat.widget().upcast_ref()).is_sensitive());

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
            .any(|image| image.has_css_class("papo-status-dot") && image.paintable().is_some()
                && image.tooltip_text().as_deref() == Some("Ausente")));

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
            assert!(avatar_cache.finish(request, &ids, vec![(profile.id,profile.avatar_blob.as_deref().and_then(crate::media::avatars::prepare))]));
            let texture = avatar_cache.textures[&channel_id].clone();
            assert_eq!(texture.width(), dimension.min(crate::media::avatars::AVATAR_EDGE) as i32);
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
            let profile:crate::models::UserProfile = serde_json::from_value(serde_json::json!({
                "id":channel_id,"username":"alice","created_at":"2026-10-03T12:00:00Z","avatar_blob":blob
            })).unwrap();
            let (request, ids) = avatar_cache.begin(&[channel_id], true).unwrap();
            avatar_cache.finish(request, &ids, vec![(profile.id,profile.avatar_blob.as_deref().and_then(crate::media::avatars::prepare))]);
            chat.emit(ChatMsg::SetAvatars(avatar_cache.textures.clone()));
            members.emit(UserListMsg::SetAvatars(avatar_cache.textures.clone()));
            drain();
            assert_fallback(chat.widget().upcast_ref(), 36);
            assert_fallback(members.widget().upcast_ref(), 24);
        }

        crate::ui::chat::actions::tests::exercise(&context);
        crate::ui::chat::bugs::exercise(&context);
        crate::ui::chat::bugs5::exercise(&context);
        crate::ui::chat::bugs6::exercise(&context);
        crate::ui::chat::bugs7::exercise(&context);
        crate::ui::chat::clipboard::exercise(&context);
        crate::ui::chat::actions::exercise_edit_mentions(&context);
        crate::ui::chat::composer::exercise(&context);
        crate::ui::main_window::account::exercise_profile_emojis(&context);
        crate::ui::main_window::account::exercise_presence_selector(&context);
        crate::ui::status::exercise(&context);
        crate::ui::chat::transfers::exercise_viewer(&context);
        crate::ui::chat::transfers::exercise_retention(&context);
        crate::ui::chat::transfers::exercise_geometry(&context);
        crate::ui::chat::rich::exercise(&context);
        crate::ui::chat::video::exercise_initial_audio();
        crate::ui::chat::bugs6::exercise_send_feedback(&context);
        crate::ui::chat::video::exercise_fullscreen(&context);
        crate::ui::chat::performance::exercise_channel_cache(&context);
        crate::ui::chat::performance::exercise_startup(&context);
        crate::ui::sidebar::exercise_startup(&context);
        crate::ui::chat::performance::exercise_media_scroll(&context);
        crate::ui::chat::performance::exercise_opening(&context);
        crate::ui::chat::performance::exercise_smooth_updates(&context);
        crate::ui::chat::performance::exercise(&context);
        crate::ui::chat::performance::exercise_corrections(&context);
        crate::ui::user_list::exercise_corrections(&context);
        crate::media::avatars::exercise_corrections(&context);
        crate::media::animation::exercise(&context);
        crate::ui::chat::multiline::exercise(&context);
        crate::ui::chat::multiline::exercise_full_app(&context);
        crate::ui::chat::mention_background::exercise(&context);
        crate::ui::chat::exercise_animated_emojis(&context);
        crate::ui::main_window::notifications::exercise_filters(&context);
        crate::ui::main_window::notifications::exercise_tabs(&context);
        crate::ui::main_window::webhooks::exercise(&context);
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

    fn exercise_gnome_layout(controller:&Controller<RootModel>,context:&gtk::glib::MainContext){
        use crate::ui::chat::actions::tests::{mock,until,pump,find_button};
        let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
        let whoami=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
        controller.emit(RootMsg::Authenticated{whoami,client:api});
        until(context,||controller.model().view==View::Main);
        let window=controller.widget();
        {
            let model=controller.model();let main=model.main_controller.as_ref().unwrap();
            crate::ui::main_window::layout::tests::exercise_header(main,window,context);
            let widgets=descendants(&window.content().unwrap());
            assert!(!widgets.iter().find(|w|w.widget_name()=="login-header").unwrap().is_mapped());
            assert_eq!(widgets.iter().filter(|w|w.is::<adw::HeaderBar>()&&w.is_mapped()).count(),1);
        }
        find_button(&window.content().unwrap(),"Desconectar").emit_clicked();
        until(context,||window.visible_dialog().is_some());
        let dialog=window.visible_dialog().unwrap();
        assert_eq!(controller.model().view,View::Main);
        find_button(dialog.upcast_ref(),"Cancelar").emit_clicked();
        until(context,||window.visible_dialog().is_none());
        assert_eq!(controller.model().view,View::Main,"cancelling must retain the session");
        assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p=="/auth/logout"));
        find_button(&window.content().unwrap(),"Desconectar").emit_clicked();
        until(context,||window.visible_dialog().is_some());
        let dialog=window.visible_dialog().unwrap();
        dialog.close();
        until(context,||window.visible_dialog().is_none());
        assert_eq!(controller.model().view,View::Main,"dismissing must retain the session");
        find_button(&window.content().unwrap(),"Desconectar").emit_clicked();
        until(context,||window.visible_dialog().is_some());
        crate::ui::main_window::layout::tests::preview(window,"disconnect-confirmation",context);
        find_button(window.visible_dialog().unwrap().upcast_ref(),"Desconectar").emit_clicked();
        until(context,||controller.model().view==View::Login);
        crate::ui::chat::performance::settle(context);
        let widgets=descendants(&window.content().unwrap());
        assert!(widgets.iter().find(|w|w.widget_name()=="login-header").unwrap().is_mapped());
        assert!(!widgets.iter().any(|w|w.widget_name()=="conversation-header"));
        server.abort();pump(context);
    }

    fn exercise_login_input(controller:&Controller<RootModel>,context:&gtk::glib::MainContext){
        use crate::ui::chat::{composer,multiline,InputExt};
        use crate::ui::chat::actions::tests::{mock,pump,until};
        let window=controller.widget();window.set_title(Some("Papo login input test"));
        let native=std::env::var_os("PAPO_NATIVE_INPUT_TEST").is_some();
        if native{assert_eq!(window.upcast_ref::<gtk::Widget>().display().type_().name(),"GdkX11Display");assert!(std::process::Command::new("setxkbmap").args(["-layout","br","-variant",""]).status().unwrap().success());}
        let row=descendants(&window.content().unwrap()).into_iter().find_map(|w|w.downcast::<adw::EntryRow>().ok().filter(|row|row.title()=="Usuário")).unwrap();
        let text=row.delegate().and_downcast::<gtk::Text>().unwrap();row.set_text("");row.grab_focus();
        until(context,||row.is_mapped());crate::ui::chat::performance::settle(context);
        if native{multiline::keys_for("Papo login input test",&["dead_acute"]);crate::ui::chat::performance::settle(context);multiline::keys_for("Papo login input test",&["a"]);}else{text.emit_by_name::<()>("insert-at-cursor",&[&"á"]);}
        until(context,||row.text()=="á");
        let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let whoami=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
        // Keep the actual login editable focused as authentication changes the
        // watched stack. The login controller stays alive, as in the real app.
        controller.emit(RootMsg::Authenticated{whoami,client:api});
        until(context,||controller.model().view==View::Main);
        let field=composer::field(&window.content().unwrap());until(context,||field.is_mapped()&&field.is_sensitive());field.grab_focus();
        if window.upcast_ref::<gtk::Widget>().display().type_().name()=="GdkWaylandDisplay"&&std::env::var_os("GTK_IM_MODULE").is_none(){assert_eq!(window.settings().gtk_im_module().as_deref(),Some("gtk-im-context-simple"));assert!(field.im_module().is_none(),"login and chat inherit one process-wide transport");}
        if native{multiline::keys_for("Papo login input test",&["dead_acute"]);crate::ui::chat::performance::settle(context);multiline::keys_for("Papo login input test",&["e","space","Control_L+Shift_L+u","e","7","Return"]);}else{field.emit_by_name::<()>("insert-at-cursor",&[&"é ç"]);}
        until(context,||field.text()=="é ç");assert!(!backend.lock().unwrap().requests.iter().any(|(m,p,_)|m=="POST"&&p=="/messages"),"an input-method Enter commit must not send after login");
        server.abort();pump(context);
    }
}

/// EN: Root state coordinator holding references to child controllers and session identity.
/// PT-BR: Coordenador de estado raiz contendo referências aos controladores filhos e identidade da sessão.
pub struct RootModel {
    view: View,
    login_controller: Controller<LoginModel>,
    main_controller: Option<Controller<MainWindowModel>>,
    // Authenticated session state
    current_user: Option<WhoamiResponse>,
    api_client: Option<ApiClient>,
}

// ── Messages ──────────────────────────────────────────────────────────────────

/// EN: Messages that transition the top-level application state.
/// PT-BR: Mensagens que realizam a transição do estado de nível superior da aplicação.
#[derive(Debug)]
pub enum RootMsg {
    /// Received from Login component upon successful auth
    /// EN: Emitted when authentication succeeds with user profile and authenticated API client.
    /// PT-BR: Emitido após autenticação bem-sucedida contendo o perfil do usuário e cliente de API autenticado.
    Authenticated {
        whoami: WhoamiResponse,
        client: ApiClient,
    },
    /// Log out and return to the login screen
    /// EN: Triggered by user requesting manual logout.
    /// PT-BR: Disparado quando o usuário solicita desconexão manual.
    Logout,
    /// EN: Triggered when token validation fails or session expires, redirecting to login.
    /// PT-BR: Disparado quando a validação de token falha ou a sessão expira, redirecionando para a tela de login.
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
            set_icon_name: Some(crate::app::APP_ID),
            set_default_size: (1280, 760),

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    set_widget_name:"login-header",
                    #[watch] set_visible:model.view==View::Login,
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
                    // Each view owns its header; avoid crossfading two sets
                    // of window controls during authentication or logout.
                    // EN: Disables crossfading transitions to prevent conflicting window controls during view switch.
                    // PT-BR: Desativa transições com crossfade para evitar controles de janela conflitantes durante a troca de visualização.
                    set_transition_type: gtk::StackTransitionType::None,
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
        crate::app::branding::install();
        crate::ui::style::install();
        crate::ui::chat::input::configure_desktop_im();
        let config = AppConfig::load();
        crate::media::animation::set_motion(config.motion);

        // Initialize Login child component controller
        let login_controller = LoginModel::builder()
            .launch(config)
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
