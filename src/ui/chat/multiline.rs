//! Native multiline input, wrapping, IM commits and outgoing wire-format checks.
use super::*;
use actions::tests::{mock,pump,until};
use adw::prelude::*;

fn keys(sequence:&[&str]){
    keys_for("Papo input test",sequence);
}
pub(crate) fn keys_for(title:&str,sequence:&[&str]){
    let python=crate::platform::runtime::python();let mut command=std::process::Command::new(python.as_std().get_program());
    let directory=std::env::var_os("PAPO_TEST_HELPERS_DIR").map(std::path::PathBuf::from).unwrap_or_else(||std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/helpers")));
    #[cfg(windows)]{
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000).arg(directory.join("windows_input.py")).arg(std::process::id().to_string());
    }
    #[cfg(not(windows))]command.arg(directory.join("input_keys.py"));
    let result=command.arg(title).args(sequence).status().unwrap();assert!(result.success());
}

/// Exercise the actual initially disabled chat embedded in the main window,
/// including activity handlers and asynchronous startup/member updates.
pub(crate) fn exercise_full_app(context:&gtk::glib::MainContext){
    use crate::ui::main_window::{MainWindowInit,MainWindowModel};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let user=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    let app=adw::Application::builder().application_id("br.com.papo.gtk.InputRegression").flags(gtk::gio::ApplicationFlags::NON_UNIQUE).build();app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).detach();
    let window=adw::ApplicationWindow::builder().application(&app).title("Papo full input test").default_width(1120).default_height(720).content(main.widget()).build();
    let field=composer::field(main.widget().upcast_ref());assert!(!field.is_sensitive());window.present();until(context,||field.is_sensitive()&&field.is_mapped());field.grab_focus();
    let wayland=field.display().type_().name()=="GdkWaylandDisplay";
    if wayland&&std::env::var_os("GTK_IM_MODULE").is_none()&&std::env::var_os("XMODIFIERS").is_none(){
        assert_eq!(field.settings().gtk_im_module().as_deref(),Some("gtk-im-context-simple"),"a Wayland terminal without framework hints must configure Compose across login and chat");assert!(field.im_module().is_none(),"the composer must inherit the same transport as the login fields");
        // These are process-local GTK settings in a private test session, not
        // dconf or real desktop changes. Late missing clients retain Compose;
        // a valid explicit preference hands control back to GTK.
        let settings=field.settings();let original=settings.gtk_im_module();settings.set_gtk_im_module(Some("missing-test-module"));assert_eq!(settings.gtk_im_module().as_deref(),Some("gtk-im-context-simple"));assert!(field.im_module().is_none());settings.set_gtk_im_module(Some("simple"));assert!(field.im_module().is_none());settings.set_gtk_im_module(original.as_deref());assert!(field.im_module().is_none());
    }
    if cfg!(target_os="linux")&&std::env::var("PAPO_NATIVE_INPUT_TEST").as_deref()==Ok("1"){
        assert!(!wayland,"XTest input is restricted to the private X11 regression display");
        assert!(std::process::Command::new("setxkbmap").args(["-layout","br","-variant",""]).status().unwrap().success());
        for letter in ["a","e"]{keys_for("Papo full input test",&["dead_acute"]);performance::settle(context);keys_for("Papo full input test",&[letter,"space"]);pump(context);}
        keys_for("Papo full input test",&["Control_L+Shift_L+u","e","7","Return"]);
    }else{field.emit_by_name::<()>("insert-at-cursor",&[&"á é ç"]);}
    until(context,||field.text()=="á é ç");
    if cfg!(target_os="linux")&&std::env::var("PAPO_NATIVE_INPUT_TEST").as_deref()==Ok("1"){assert!(field.has_focus(),"native typing must keep focus on the chat composer");}
    assert!(!backend.lock().unwrap().requests.iter().any(|(method,path,_)|method=="POST"&&path=="/messages"),"committing accents must never send a message");
    window.close();window.set_content(None::<&gtk::Widget>);drop(main);server.abort();pump(context);
}
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let own="12345678-1234-4234-8234-123456789abc".parse().unwrap();let peer="12345678-1234-4234-8234-123456789ac0".parse().unwrap();
    let id="12345678-1234-4234-8234-123456789abd".parse().unwrap();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"entrada","created_at":chrono::Utc::now()})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(id)),user_id:Some(own),api:Some(api),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:own,access:crate::models::Access::direct()});
    chat.emit(ChatMsg::SetUsers(vec![serde_json::from_value(serde_json::json!({"id":peer,"username":"bob","nickname":"ISpeakHue","created_at":chrono::Utc::now()})).unwrap()]));
    let window=adw::Window::builder().title("Papo input test").default_width(850).default_height(700).content(chat.widget()).build();window.present();
    let field=composer::field(chat.widget().upcast_ref());until(context,||field.is_mapped()&&field.width()>0);field.grab_focus();
    let scroll=field.parent().and_downcast::<gtk::ScrolledWindow>().unwrap();let initial=scroll.height();
    let native=cfg!(target_os="linux")&&std::env::var("PAPO_NATIVE_INPUT_TEST").as_deref()==Ok("1");
    if native{let layout=std::env::var("PAPO_NATIVE_INPUT_LAYOUT").unwrap_or_else(|_|"us".into());assert!(std::process::Command::new("setxkbmap").args(["-layout",&layout,"-variant",if layout=="us"{"intl"}else{""}]).status().unwrap().success());}
    field.set_text("á @I");field.set_position(-1);until(context,||chat.model().mention_visible);
    if native{keys(&["Tab"]);}else{chat.emit(ChatMsg::MentionSelected(Some(peer)));}
    until(context,||field.text()=="á @ISpeakHue ");assert!(field.selection_bounds().is_none());
    if native{
        keys(&["dead_acute"]);pump(context);performance::settle(context);
        keys(&["a","space"]);pump(context);
        keys(&["dead_acute"]);pump(context);performance::settle(context);
        keys(&["e"]);
    }else{field.emit_by_name::<()>("insert-at-cursor",&[&"á é"]);}
    until(context,||field.text()=="á @ISpeakHue á é");assert!(chat.model().draft.text.contains(&format!("@mention(<@{peer}>) á é")));
    if native{keys(&["Shift_L+Return"]);}else{field.emit_by_name::<()>("insert-at-cursor",&[&"\n"]);}
    until(context,||field.text().ends_with('\n'));
    if native{keys(&["Control_L+Shift_L+u","e","7","Return"]);}else{field.emit_by_name::<()>("insert-at-cursor",&[&"ç"]);}
    until(context,||field.text().ends_with("\nç"));
    let display=field.text().to_string();let wire=chat.model().draft.text.clone();assert_eq!(wire,format!("á @mention(<@{peer}>) á é\nç"));
    assert!(backend.lock().unwrap().requests.iter().all(|(m,p,_)|m!="POST"||p!="/messages"),"Shift+Enter and IM commit Enter must not send");
    until(context,||scroll.height()>initial);assert!(scroll.height()<=160);
    crate::ui::main_window::layout::tests::preview(&window,"composer-multiline",context);
    // Natural wrapping and a bounded vertical viewport at narrow widths.
    field.set_text(&"Palavras compridas e acentos á é ç. ".repeat(60));field.set_position(-1);crate::ui::chat::actions::tests::resize_window(&window,500,700);
    until(context,||scroll.height()>=150&&scroll.vadjustment().upper()>scroll.vadjustment().page_size());
    assert!(scroll.height()<=160);assert_eq!(scroll.hadjustment().value(),0.0);
    let rect=field.iter_location(&field.buffer().end_iter());assert!(rect.x()<field.width(),"long lines must wrap inside the editor");
    crate::ui::main_window::layout::tests::preview(&window,"composer-wrapped",context);
    field.set_text("");until(context,||scroll.height()==initial);
    // Restore the mention's exact ID with its multiline draft, not its display name.
    chat.model().composer.set_wire(&field,&wire,&chat.model().users_map);chat.emit(ChatMsg::InputChanged(wire.clone()));pump(context);
    chat.emit(ChatMsg::SetChannel(channel(Uuid::new_v4())));pump(context);chat.emit(ChatMsg::SetChannel(channel(id)));until(context,||field.text()==display);
    field.set_position(-1);field.grab_focus();
    if native{keys(&["Return"]);}else{chat.emit(ChatMsg::SendClicked);}
    until(context,||!chat.model().draft.is_sending()&&chat.model().draft.error.is_some());assert_eq!(field.text(),display,"a rejected send preserves every line and mention");
    if native{keys(&["Return"]);}else{chat.emit(ChatMsg::SendClicked);}
    until(context,||field.text().is_empty()&&!chat.model().draft.is_sending());
    let posts:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="POST"&&p=="/messages").cloned().collect();assert_eq!(posts.len(),2);for(_,_,body) in posts{assert!(body.as_str().unwrap().contains(&wire));}
    until(context,||scroll.height()==initial);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}

#[test]
#[ignore="interactive check on the user's actual desktop input method"]
fn desktop_accent_check(){
    use crate::ui::main_window::{MainWindowInit,MainWindowModel};
    adw::init().unwrap();crate::ui::style::install();let runtime=tokio::runtime::Runtime::new().unwrap();let _guard=runtime.enter();
    let app=adw::Application::builder().application_id("br.com.papo.gtk.InputTest").flags(gtk::gio::ApplicationFlags::NON_UNIQUE).build();app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let context=gtk::glib::MainContext::default();
    // Use the complete application hierarchy: an initially disabled composer,
    // mock startup requests, delayed channel selection and window key handlers.
    // The server is private localhost; never load a saved account or session.
    let(api,_,server)=runtime.block_on(mock());let user=runtime.block_on(api.whoami()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).detach();
    let box_=gtk::Box::new(gtk::Orientation::Vertical,12);
    let instruction=gtk::Label::new(Some("Teste á é ç usando o teclado no campo de mensagem inferior. O campo superior é uma referência GTK. Este teste usa apenas um servidor simulado local, sem sua conta."));instruction.set_wrap(true);instruction.set_margin_top(12);instruction.set_margin_start(12);instruction.set_margin_end(12);box_.append(&instruction);
    let entry=gtk::Entry::new();entry.set_placeholder_text(Some("Campo padrão: á é ç"));entry.set_margin_start(12);entry.set_margin_end(12);box_.append(&entry);box_.append(main.widget());
    let done=gtk::Button::with_label("Concluir teste");done.set_margin_start(12);done.set_margin_end(12);done.set_margin_bottom(12);box_.append(&done);
    let header=adw::HeaderBar::new();header.set_title_widget(Some(&gtk::Label::new(Some("Papo — teste completo de acentos (local)"))));let content=adw::ToolbarView::new();content.add_top_bar(&header);content.set_content(Some(&box_));
    let window=adw::ApplicationWindow::builder().application(&app).title("Papo — teste completo de acentos (local)").default_width(1120).default_height(720).content(&content).build();let weak=window.downgrade();done.connect_clicked(move |_|{if let Some(w)=weak.upgrade(){w.close();}});window.present();
    let field=composer::field(main.widget().upcast_ref());until(&context,||field.is_sensitive()&&field.is_editable());field.grab_focus();
    let start=std::time::Instant::now();while window.is_visible()&&start.elapsed()<std::time::Duration::from_secs(180){pump(&context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let accented=|text:&str|text.chars().any(|c|"áéíóúãõâêôàçÁÉÇ".contains(c));
    eprintln!("Full-app accent check: standard_field={}, composer={}, backend={}, module={:?}",accented(&entry.text()),accented(&field.text()),field.display().type_().name(),field.im_module());
    window.close();window.set_content(None::<&gtk::Widget>);drop(main);server.abort();pump(&context);
}
