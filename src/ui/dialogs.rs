//! EN-US: Escape follows each transient window's normal close/unsaved-changes path.
//! PT-BR: Escape usa o fluxo normal de fechamento e de confirmação de alterações de cada janela auxiliar.
use gtk::prelude::*;
pub(super) fn install(){
    fn observe(window:&gtk::Window){
        let keys=gtk::EventControllerKey::new();let weak=window.downgrade();
        // Bubble so popovers, IM composition, fullscreen and editor completion
        // get the first opportunity to consume Escape.
        keys.connect_key_pressed(move |_,key,_,_|{
            if key==gtk::gdk::Key::Escape{if let Some(window)=weak.upgrade().filter(|w|w.transient_for().is_some()){
                window.close();return gtk::glib::Propagation::Stop;
            }}gtk::glib::Propagation::Proceed
        });window.add_controller(keys);
    }
    for window in gtk::Window::list_toplevels(){if let Ok(window)=window.downcast::<gtk::Window>(){observe(&window);}}
    gtk::Window::toplevels().connect_items_changed(|list,position,_,added|{
        for index in position..position+added{if let Some(window)=list.item(index).and_downcast::<gtk::Window>(){observe(&window);}}
    });
}
