//! EN-US: Opt-in local input diagnostics. Never collect key values, text or credentials.
//! PT-BR: Diagnósticos locais de entrada ativados explicitamente; não coletam teclas, texto ou credenciais.
use super::*;
use std::{cell::Cell,rc::Rc};

#[derive(Default)]
struct Counts {dead:Cell<u64>,other:Cell<u64>,preedit:Cell<u64>,nonempty_preedit:Cell<u64>,changed:Cell<u64>}
fn module(value:Option<&str>)->Option<String>{
    value.map(|value|value.split(':').map(|name|match name{
        ""|"simple"|"gtk-im-context-simple"|"none"|"gtk-im-context-none"|"wayland"|"ibus"|"fcitx"|"fcitx5"|"xim"=>name,
        _=>"custom",
    }).collect::<Vec<_>>().join(":"))
}
fn write(view:&gtk::TextView,counts:&Counts){
    let settings=view.settings().gtk_im_module();let selected=view.im_module();
    let env=std::env::var("GTK_IM_MODULE").ok();let modifiers=std::env::var("XMODIFIERS").ok();
    let point=gtk::gio::IOExtensionPoint::lookup("gtk-im-module");let registered:Vec<_>=["gtk-im-context-simple","simple","wayland","ibus","fcitx","fcitx5"].into_iter().filter(|name|point.as_ref().is_some_and(|p|p.extension_by_name(*name).is_some())).collect();
    let report=serde_json::json!({
        "build":"composer-input-4","test_build":cfg!(test),
        "gtk_version":format!("{}.{}.{}",gtk::major_version(),gtk::minor_version(),gtk::micro_version()),
        "display":view.display().type_().name(),"widget_module":module(selected.as_deref()),"settings_module":module(settings.as_deref()),"env_module":module(env.as_deref()),
        "framework_hint":modifiers.as_deref().map(|hint|match hint{"@im=ibus"|"@im=fcitx"|"@im=fcitx5"=>hint,_=>"other"}),"registered_modules":registered,
        "mapped":view.is_mapped(),"focused":view.has_focus(),"sensitive":view.is_sensitive(),"editable":view.is_editable(),
        "dead_key_events":counts.dead.get(),"other_key_events":counts.other.get(),"preedit_events":counts.preedit.get(),"nonempty_preedit_events":counts.nonempty_preedit.get(),"buffer_changes":counts.changed.get()
    });
    // The project's target directory is ignored by Git and shared with the
    // developer even when the app uses a different desktop/runtime from tests.
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/papo-input-diagnostic.json");
    if let Ok(bytes)=serde_json::to_vec_pretty(&report){let _=std::fs::write(path,bytes);}
}
pub(super) fn connect(view:&gtk::TextView){
    if std::env::var("PAPO_INPUT_DIAGNOSTICS").as_deref()!=Ok("1"){return;}
    let counts=Rc::new(Counts::default());write(view,&counts);
    let c=counts.clone();view.connect_map(move |view|write(view,&c));
    let focus=gtk::EventControllerFocus::new();let weak=view.downgrade();let c=counts.clone();focus.connect_enter(move |_|{if let Some(view)=weak.upgrade(){write(&view,&c);}});view.add_controller(focus);
    let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);let weak=view.downgrade();let c=counts.clone();keys.connect_key_pressed(move |_,key,_,_|{
        let dead=matches!(key,gtk::gdk::Key::dead_acute|gtk::gdk::Key::dead_grave|gtk::gdk::Key::dead_circumflex|gtk::gdk::Key::dead_tilde|gtk::gdk::Key::dead_diaeresis|gtk::gdk::Key::dead_cedilla);
        let count=if dead{&c.dead}else{&c.other};count.set(count.get()+1);if let Some(view)=weak.upgrade(){write(&view,&c);}gtk::glib::Propagation::Proceed
    });view.add_controller(keys);
    let c=counts.clone();view.connect_preedit_changed(move |view,text|{c.preedit.set(c.preedit.get()+1);if !text.is_empty(){c.nonempty_preedit.set(c.nonempty_preedit.get()+1);}write(view,&c);});
    let weak=view.downgrade();view.buffer().connect_changed(move |_|{counts.changed.set(counts.changed.get()+1);if let Some(view)=weak.upgrade(){write(&view,&counts);}});
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn diagnostic_preferences_do_not_expose_arbitrary_environment_values(){
        let secret="Auth=private-token;password=private-password";
        let masked=module(Some(&format!("ibus:{secret}:simple"))).unwrap();
        assert_eq!(masked,"ibus:custom:simple");assert!(!masked.contains("private"));assert_eq!(module(None),None);
    }
}
