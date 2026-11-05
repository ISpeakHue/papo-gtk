//! EN-US: Private keyboard/semantic/geometry fixture; native screen-reader acceptance is documented separately.
//! PT-BR: Fixture privada de teclado/semântica/geometria; aceitação com leitores reais é documentada separadamente.
use super::*;
use adw::prelude::*;
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants,find_button};
    crate::ui::chat::actions::tests::exercise_context_replacement(context);
    let (api,_backend,server)=tokio::runtime::Handle::current().block_on(mock());let own=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:own,api_client:api}).detach();
    let window=adw::Window::builder().title("Papo accessibility fixture").default_width(1000).default_height(760).content(main.widget()).build();window.present();main.emit(MainWindowMsg::StartLoading);
    until(context,||main.model().active_channel_id.is_some()&&main.model().connection.state.background_started);
    let field=crate::ui::chat::composer::field(main.model().chat.widget().upcast_ref());field.grab_focus();field.set_text("Unsent accessible draft");pump(context);
    assert!(gtk::test_accessible_has_property(&field,gtk::AccessibleProperty::Label));assert!(gtk::test_accessible_has_property(&field,gtk::AccessibleProperty::MultiLine));
    main.widget().activate_action("papo.shortcuts",None).unwrap();until(context,||gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.title().as_deref()==Some("Atalhos de teclado")&&w.is_visible()));
    let help=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.title().as_deref()==Some("Atalhos de teclado")&&w.is_visible()).unwrap();
    assert!(descendants(help.upcast_ref()).iter().filter(|w|w.is::<gtk::ShortcutLabel>()).count()>=15);
    let keys=help.observe_controllers();let key=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>().filter(|k|k.name().as_deref()==Some("papo-dialog-keys"))).unwrap();
    assert!(key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Escape,&0u32,&gtk::gdk::ModifierType::empty()]));until(context,||!help.is_visible());pump(context);assert!(field.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));assert_eq!(field.text(),"Unsent accessible draft");
    crate::ui::chat::performance::settle(context);
    main.widget().activate_action("papo.history",None).unwrap();until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.widget_name().starts_with("message-")&&w.has_focus()));
    let row=descendants(main.model().chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name().starts_with("message-")&&w.has_focus()).unwrap();
    assert!(gtk::test_accessible_has_property(&row,gtk::AccessibleProperty::Label));assert!(gtk::test_accessible_has_property(&row,gtk::AccessibleProperty::Description));
    let keys=row.observe_controllers();let mut triggered=false;for i in 0..keys.n_items(){if let Some(controller)=keys.item(i).and_downcast::<gtk::ShortcutController>(){for j in 0..controller.n_items(){let shortcut=controller.item(j).and_downcast::<gtk::Shortcut>().unwrap();if shortcut.trigger().is_some_and(|t|t.to_string()=="<Shift>F10"){assert!(shortcut.action().unwrap().activate(gtk::ShortcutActionFlags::empty(),&row,None));triggered=true;}}}}
    let name=row.widget_name();assert!(triggered);until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().filter(|w|w.widget_name()==name).any(|row|descendants(row).iter().filter_map(|w|w.downcast_ref::<gtk::Popover>()).any(|p|p.is_mapped())));
    let row=descendants(main.model().chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==name).unwrap();
    let reply=find_button(&row,"Responder");assert!(reply.is_sensitive());assert!(gtk::test_accessible_has_property(&reply,gtk::AccessibleProperty::Label));
    if std::env::var_os("PAPO_ACCESSIBILITY_TEST").is_some(){
        let result=std::sync::Arc::new(std::sync::Mutex::new(None));let output=result.clone();let pid=std::process::id();
        let helper=std::path::PathBuf::from(std::env::var_os("PAPO_TEST_HELPERS_DIR").unwrap_or_else(||"tests/helpers".into())).join("accessibility_probe.py");
        tokio::task::spawn_blocking(move||{let run=std::process::Command::new("python3").arg(helper).arg(pid.to_string()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().and_then(|mut child|{let deadline=std::time::Instant::now()+std::time::Duration::from_secs(20);while child.try_wait()?.is_none(){if std::time::Instant::now()>=deadline{child.kill()?;break;}std::thread::sleep(std::time::Duration::from_millis(10));}child.wait_with_output()});*output.lock().unwrap()=Some(run);});
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(25);while result.lock().unwrap().is_none(){pump(context);assert!(std::time::Instant::now()<deadline,"private AT-SPI probe exceeded its deadline");std::thread::sleep(std::time::Duration::from_millis(5));}let output=result.lock().unwrap().take().unwrap().unwrap();assert!(output.status.success(),"AT-SPI: {}",String::from_utf8_lossy(&output.stderr));println!("{}",String::from_utf8_lossy(&output.stdout));
    }
    for w in descendants(&row){if let Some(p)=w.downcast_ref::<gtk::Popover>(){p.popdown();}}
    main.widget().activate_action("papo.composer",None).unwrap();until(context,||field.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));
    // Large text is process-local, never a persisted desktop setting.
    let settings=gtk::Settings::default().unwrap();let old=settings.gtk_font_name();settings.set_gtk_font_name(Some("Sans 20"));window.set_default_size(760,640);pump(context);
    main.emit(MainWindowMsg::Account(AccountMsg::Preferences));until(context,||gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.title().as_deref()==Some("Configurações da conta")&&w.is_visible()));
    let prefs=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.title().as_deref()==Some("Configurações da conta")&&w.is_visible()).unwrap();
    let save=find_button(prefs.upcast_ref(),"Salvar preferências");until(context,||save.is_mapped());assert!(save.compute_bounds(&prefs).is_some_and(|r|r.y()>=0.0&&r.y()+r.height()<=prefs.height() as f32+1.0));
    if std::env::var("ADW_DEBUG_HIGH_CONTRAST").as_deref()==Ok("1") {assert!(adw::StyleManager::default().is_high_contrast());}
    settings.set_gtk_font_name(old.as_deref());prefs.close();pump(context);
    let previous=crate::media::animation::motion();crate::media::animation::set_motion(crate::config::Motion::Reduced);
    let scroll=gtk::ScrolledWindow::new();scroll.set_vadjustment(Some(&gtk::Adjustment::new(0.0,0.0,5000.0,10.0,100.0,300.0)));let wheel=std::cell::RefCell::new(None);
    crate::ui::smooth_scroll::animate_wheel(&scroll,&wheel,1.0);assert!(scroll.vadjustment().value()>0.0);assert!(wheel.borrow().is_none());crate::media::animation::set_motion(previous);
    window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}
