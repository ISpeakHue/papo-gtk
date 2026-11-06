//! Root component — manages application views (Login / Main).

use adw::prelude::*;
use relm4::prelude::*;

mod workspace;
use crate::api::ApiClient;
use crate::config::AppConfig;
use crate::models::WhoamiResponse;
use crate::ui::login::{LoginModel, LoginOutput, LoginMsg};
use crate::ui::main_window::{MainWindowInit, MainWindowModel};

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
enum View {
    Login,
    Main,
}

#[cfg(test)]
mod tests {
    use super::*;
    // Settings tests exercise persistence, but must never load remembered real
    // sessions or write animation preferences into the developer's app config.
    struct IsolatedConfig { path:std::path::PathBuf, previous:Option<std::ffi::OsString> }
    impl IsolatedConfig {
        fn new()->Self{
            let path=std::env::temp_dir().join(format!("papo-gui-config-{}",uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();let previous=std::env::var_os("PAPO_TEST_CONFIG_DIR");std::env::set_var("PAPO_TEST_CONFIG_DIR",&path);Self{path,previous}
        }
    }
    impl Drop for IsolatedConfig {
        fn drop(&mut self){if let Some(previous)=&self.previous{std::env::set_var("PAPO_TEST_CONFIG_DIR",previous);}else{std::env::remove_var("PAPO_TEST_CONFIG_DIR");}let _=std::fs::remove_dir_all(&self.path);}
    }

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
        let baseline_started=std::time::Instant::now();
        let baseline_memory=crate::performance::resident_bytes();
        let _config=IsolatedConfig::new();
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("translations"){crate::i18n::init(crate::i18n::Language::English);}
        let _report=crate::performance::Report;
        crate::platform::runtime::configure();
        if std::env::var("PAPO_UI_CASE").is_ok_and(|c|c.starts_with("accessibility")){if std::env::var_os("PAPO_ACCESSIBILITY_TEST").is_none(){std::env::set_var("GTK_A11Y","test");}if std::env::var("PAPO_UI_CASE").as_deref()==Ok("accessibility-contrast"){std::env::set_var("ADW_DEBUG_HIGH_CONTRAST","1");}}
        adw::init().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let controller = RootModel::builder().launch(()).detach();
        let window = controller.widget();
        window.present();
        let context = gtk::glib::MainContext::default();
        // EN-US: Identify the active full-suite fixture when native code aborts without a Rust panic.
        // PT-BR: Identifica a fixture ativa quando código nativo encerra sem panic Rust.
        macro_rules! stage {
            ($check:path) => {{
                eprintln!("Papo GUI stage: {}", stringify!($check));
                $check(&context);
                eprintln!("Papo GUI complete: {}", stringify!($check));
            }};
        }
        if let Some(renderer)=window.renderer(){println!("Papo GUI renderer: {}",renderer.type_().name());}
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
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("multi-account-restart"){workspace::accounts_restart(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("multi-server"){workspace::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("conversation-switcher"){crate::ui::main_window::switcher::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("disk-thumbnails"){crate::ui::chat::transfers::disk_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-browser"){crate::ui::main_window::media_browser::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").is_ok_and(|c|c.starts_with("accessibility")){crate::ui::main_window::accessibility::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("notification-controls"){crate::ui::main_window::notifications::exercise_controls(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("release-discovery"){crate::ui::main_window::utilities::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("audio-setup"){crate::ui::main_window::voice::audio::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("chat-behavior"){crate::ui::chat::behavior_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-policies"){crate::ui::chat::transfers::policy::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-preferences"){crate::ui::main_window::account::exercise_media_preferences(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("draft-recovery"){crate::ui::chat::steps456_tests::drafts(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("outgoing-queue"){crate::ui::chat::steps456_tests::outgoing(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("translations"){crate::ui::chat::steps456_tests::translations(&context,window);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("account-scope"){crate::ui::chat::account_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("performance-baselines"){
            let first_paint=std::rc::Rc::new(std::cell::Cell::new(false));let seen=first_paint.clone();
            let clock=window.frame_clock().unwrap();let handler=clock.connect_after_paint(move |_|seen.set(true));window.queue_draw();
            crate::ui::chat::actions::tests::until(&context,||first_paint.get());clock.disconnect(handler);
            crate::ui::chat::baseline::measurement("startup_ui_ready",baseline_started,baseline_memory,serde_json::json!({"scope":"test entry to login-window paint; excludes process loader and authentication","renderer":window.renderer().map(|r|r.type_().name().to_string()),"gtk":format!("{}.{}.{}",gtk::major_version(),gtk::minor_version(),gtk::micro_version())}));
            crate::ui::chat::baseline::run(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref() == Ok("symbolic-icons") {
            crate::app::branding::exercise_symbols();
            window.close();
            return;
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
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("windows-notifications"){crate::ui::main_window::notifications::exercise_windows(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("notification-window"){crate::ui::main_window::notifications::exercise_page_window(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("recovery"){crate::ui::main_window::recovery::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report11"){crate::ui::chat::composer::exercise_inline(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("backend-contracts"){crate::ui::chat::transfers::exercise_metadata(&context);crate::ui::main_window::search::exercise_backend_identity(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("history-navigation"){crate::ui::chat::exercise_navigation(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("timeline-controls"){stage!(crate::ui::chat::bugs::exercise);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("rich-width"){crate::ui::chat::rich::exercise_text_width(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("history-retention"){crate::ui::chat::performance::exercise_corrections(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("report10"){crate::ui::chat::polish_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("search-recycling"){crate::ui::main_window::search::exercise_recycling(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("avatar-viewport"){crate::ui::user_list::exercise_viewport(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("scroll-frames"){
            #[cfg(windows)] crate::ui::style::exercise_windows_text();
            crate::ui::chat::performance::exercise_scroll_frames(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("perf-baseline"){crate::ui::chat::performance::exercise_corrections(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("gnome-layout"){
            exercise_gnome_layout(&controller,&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("presence-selector"){
            crate::ui::main_window::account::exercise_presence_selector(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("voice-chat"){crate::ui::main_window::voice::chat_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("voice-controls"){crate::ui::main_window::voice::tests::exercise_controls(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("pins-media"){crate::ui::chat::transfers::pins_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("call-sounds"){crate::media::sound::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("multiline"){crate::ui::chat::multiline::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("desktop-input"){crate::ui::chat::input::desktop_tests::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("full-app-input"){crate::ui::chat::multiline::exercise_full_app(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("login-input"){
            exercise_login_input(&controller,&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("mention-background"){crate::ui::chat::mention_background::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("notification-filters"){
            crate::ui::main_window::notifications::exercise_filters(&context);crate::ui::main_window::notifications::exercise_tabs(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("window-scrolling"){crate::ui::smooth_scroll::exercise(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-categories"){crate::ui::chat::exercise_emoji_categories(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-grid"){
            crate::ui::chat::exercise_animated_emojis(&context);crate::ui::chat::bugs8::exercise(&context);window.close();return;
        }
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-details"){crate::ui::chat::exercise_emoji_details(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("emoji-notifications"){crate::media::animation::exercise(&context);crate::ui::chat::exercise_animated_emojis(&context);crate::ui::main_window::notifications::exercise_tabs(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-emojis"){crate::media::animation::exercise(&context);crate::ui::chat::exercise_media_emojis(&context);window.close();return;}
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
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("mov-media"){crate::ui::chat::transfers::exercise_mov(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("attachment-media"){crate::ui::chat::transfers::exercise_attachments(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("media-cache"){crate::ui::chat::transfers::exercise_cache(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("security-icons"){crate::ui::main_window::administration::exercise_icon_security(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("syntax-highlighting"){crate::ui::chat::exercise_syntax(&context);window.close();return;}
        if std::env::var("PAPO_UI_CASE").as_deref()==Ok("reaction-participants"){crate::ui::chat::actions::exercise_participants(&context);window.close();return;}
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
        stage!(crate::ui::style::exercise_accents);
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
                    && image.icon_name().as_deref() == Some("papo-avatar-default-symbolic")));
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

        stage!(crate::ui::chat::actions::tests::exercise);
        stage!(crate::ui::chat::bugs::exercise);
        stage!(crate::ui::chat::bugs5::exercise);
        stage!(crate::ui::chat::bugs6::exercise);
        stage!(crate::ui::chat::bugs7::exercise);
        stage!(crate::ui::chat::clipboard::exercise);
        stage!(crate::ui::chat::actions::exercise_edit_mentions);
        stage!(crate::ui::chat::composer::exercise);
        stage!(crate::ui::main_window::account::exercise_profile_emojis);
        stage!(crate::ui::main_window::account::exercise_presence_selector);
        stage!(crate::ui::status::exercise);
        stage!(crate::ui::chat::transfers::exercise_viewer);
        stage!(crate::ui::chat::transfers::exercise_retention);
        stage!(crate::ui::chat::transfers::exercise_geometry);
        stage!(crate::ui::chat::rich::exercise);
        crate::ui::chat::video::exercise_initial_audio();
        stage!(crate::ui::chat::bugs6::exercise_send_feedback);
        stage!(crate::ui::chat::video::exercise_fullscreen);
        stage!(crate::ui::chat::performance::exercise_channel_cache);
        stage!(crate::ui::chat::performance::exercise_startup);
        stage!(crate::ui::sidebar::exercise_startup);
        stage!(crate::ui::chat::performance::exercise_media_scroll);
        stage!(crate::ui::chat::performance::exercise_opening);
        stage!(crate::ui::chat::performance::exercise_smooth_updates);
        stage!(crate::ui::smooth_scroll::exercise);
        stage!(crate::ui::chat::performance::exercise);
        stage!(crate::ui::chat::performance::exercise_corrections);
        stage!(crate::ui::user_list::exercise_corrections);
        stage!(crate::media::avatars::exercise_corrections);
        stage!(crate::media::animation::exercise);
        stage!(crate::ui::chat::multiline::exercise);
        stage!(crate::ui::chat::multiline::exercise_full_app);
        stage!(crate::ui::chat::mention_background::exercise);
        stage!(crate::ui::chat::exercise_animated_emojis);
        stage!(crate::ui::main_window::notifications::exercise_filters);
        stage!(crate::ui::main_window::notifications::exercise_tabs);
        stage!(crate::ui::main_window::webhooks::exercise);
        stage!(crate::media::sound::exercise);

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
        let native=cfg!(target_os="linux")&&std::env::var("PAPO_NATIVE_INPUT_TEST").as_deref()==Ok("1");
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

/// EN-US: Root state coordinator holding references to child controllers and session identity.
/// PT-BR: Coordenador de estado raiz contendo referências aos controladores filhos e identidade da sessão.
pub struct RootModel {
    view: View,
    workspace:workspace::Workspace,
    login_controller: Controller<LoginModel>,
    main_controller: Option<Controller<MainWindowModel>>,
    // Authenticated session state
    current_user: Option<WhoamiResponse>,
    api_client: Option<ApiClient>,
    #[cfg(windows)] native_activation:Option<uuid::Uuid>,
}

// ── Messages ──────────────────────────────────────────────────────────────────

/// EN-US: Messages that transition the top-level application state.
/// PT-BR: Mensagens que realizam a transição do estado de nível superior da aplicação.
#[derive(Debug)]
pub enum RootMsg {
    LoginAuthenticated{scope:uuid::Uuid,whoami:WhoamiResponse,client:ApiClient},
    WorkspaceStart, AddAccount, DirectNavigation, CancelAccountLogin, SelectAccount(crate::account::AccountId),
    AccountOutput{id:crate::account::AccountId,session:uuid::Uuid,output:crate::ui::main_window::MainWindowOutput},
    AccountResumed{id:crate::account::AccountId,ticket:uuid::Uuid,result:anyhow::Result<Option<(WhoamiResponse,ApiClient)>>},
    RemoveAccount(crate::account::AccountId), RemoveAccountConfirmed(crate::account::AccountId), DirectorySaved(Result<(),String>),
    /// Received from Login component upon successful auth
    /// EN-US: Emitted when authentication succeeds with user profile and authenticated API client.
    /// PT-BR: Emitido após autenticação bem-sucedida contendo o perfil do usuário e cliente de API autenticado.
    #[cfg(test)] Authenticated {
        whoami: WhoamiResponse,
        client: ApiClient,
    },
    /// Log out and return to the login screen
    /// EN-US: Triggered by user requesting manual logout.
    /// PT-BR: Disparado quando o usuário solicita desconexão manual.
    #[cfg(test)] Logout,
    /// EN-US: Triggered when token validation fails or session expires, redirecting to login.
    /// PT-BR: Disparado quando a validação de token falha ou a sessão expira, redirecionando para a tela de login.
    #[cfg(test)] SessionExpired,
    #[cfg(windows)] NativeActivation(crate::platform::activation::Activation),
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
                    pack_start = &gtk::Button {
                        set_label:crate::i18n::tr("Voltar ao servidor"),
                        #[watch] set_visible:model.main_controller.is_some(),
                        connect_clicked=>RootMsg::CancelAccountLogin,
                    },
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: "Papo",
                        #[watch]
                        set_subtitle: model.current_user.as_ref()
                            .map(|user| user.username.as_str()).unwrap_or(""),
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation:gtk::Orientation::Horizontal,
                    #[local_ref] workspace_rail -> gtk::Box {},
                #[name = "stack"]
                gtk::Stack {
                    set_hexpand:true,
                    set_vexpand: true,
                    set_hhomogeneous:false,set_vhomogeneous:false,
                    // Each view owns its header; avoid crossfading two sets
                    // of window controls during authentication or logout.
                    // EN-US: Disables crossfading transitions to prevent conflicting window controls during view switch.
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
            },
        }
    }


    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        #[cfg(windows)] {
            let window=root.downgrade();let activation_sender=sender.input_sender().clone();
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(50),move ||{
                let Some(window)=window.upgrade()else{return gtk::glib::ControlFlow::Break;};
                let _=window;
                for activation in crate::platform::startup::drain(){let _=activation_sender.send(RootMsg::NativeActivation(activation));}
                gtk::glib::ControlFlow::Continue
            });
        }
        crate::app::branding::install();
        crate::ui::style::install();
        crate::ui::chat::input::configure_desktop_im();
        let mut config = AppConfig::load();
        let workspace=workspace::Workspace::new(&sender);
        if !workspace.directory.accounts.is_empty(){config.remember_session=Some(false);}
        crate::media::animation::set_motion(config.motion);

        // Initialize Login child component controller
        let login_controller = LoginModel::builder()
            .launch(config)
            .forward(sender.input_sender(), |output| match output {
                LoginOutput::Authenticated {
                    scope,whoami,
                    client,
                } => RootMsg::LoginAuthenticated {
                    scope,whoami,
                    client,
                },
            });

        let model = RootModel {
            view: View::Login,workspace,
            login_controller,
            main_controller: None,
            current_user: None,
            api_client: None,
            #[cfg(windows)] native_activation:Default::default(),
        };

        let workspace_rail=&model.workspace.rail;
        let widgets = view_output!();
        widgets.stack.add_named(&model.workspace.pages,Some("main"));
        sender.input(RootMsg::WorkspaceStart);
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
            RootMsg::LoginAuthenticated{scope,whoami,client}=>{if scope==self.workspace.login_scope{let local=AppConfig::load();self.attach_account(whoami,client,local.remember_session.unwrap_or(false),true,&sender);}},
            #[cfg(test)] RootMsg::Authenticated{whoami,client}=>{
                let local=AppConfig::load();let remember=local.server_url.as_deref()==Some(client.base_url())&&local.last_username.as_deref()==Some(&whoami.username)&&local.remember_session.unwrap_or(false);
                self.attach_account(whoami,client,remember,true,&sender);
            },
            #[cfg(test)] RootMsg::Logout|RootMsg::SessionExpired=>{
                if let Some(controller)=&self.main_controller{let id=controller.model().connection.account.clone();let session=self.workspace.generations[&id];let output=if matches!(message,RootMsg::Logout){crate::ui::main_window::MainWindowOutput::Logout}else{crate::ui::main_window::MainWindowOutput::SessionExpired};self.workspace_message(RootMsg::AccountOutput{id,session,output},&sender,_root);}
            },
            #[cfg(windows)]RootMsg::NativeActivation(activation)=>{
                _root.present();if let crate::platform::activation::Activation::Ticket(id)=activation{self.native_activation=Some(id);
                    for c in self.main_controller.iter().chain(self.workspace.inactive.values()){c.emit(crate::ui::main_window::MainWindowMsg::Notification(crate::ui::main_window::notifications::NoticeMsg::NativeTicket(id)));}
                }
            },
            message=>self.workspace_message(message,&sender,_root),
        }
        self.update_view(widgets, sender);
    }
}
