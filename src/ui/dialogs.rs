//! EN-US: Escape follows each transient window's normal close/unsaved-changes path.
//! PT-BR: Escape usa o fluxo normal de fechamento e de confirmação de alterações de cada janela auxiliar.
use gtk::prelude::*;
pub(super) fn install(){
    fn observe(window:&gtk::Window){
        // EN-US: Keep only weak references; closing a dialog must not retain an old conversation.
        // PT-BR: Guarda só referências fracas; fechar um diálogo não retém uma conversa antiga.
        let previous=std::rc::Rc::new(std::cell::RefCell::new(None::<gtk::glib::WeakRef<gtk::Widget>>));
        let saved=previous.clone();window.connect_map(move |window|{
            *saved.borrow_mut()=window.transient_for().and_then(|p|gtk::prelude::RootExt::focus(&p)).map(|w|w.downgrade());
            name_controls(window.upcast_ref());
        });
        window.connect_hide(move |window|{
            let parent=window.transient_for().map(|w|w.downgrade());let focus=previous.borrow_mut().take();let closed=window.downgrade();
            gtk::glib::idle_add_local_once(move ||{
                let Some(parent)=parent.and_then(|w|w.upgrade()).filter(|w|w.is_visible())else{return;};
                let another=gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|Some(w)!=closed.upgrade().as_ref()&&w.is_visible()&&w.is_modal()&&w.transient_for().as_ref()==Some(&parent));
                if another{return;}
                if let Some(focus)=focus.and_then(|w|w.upgrade()).filter(|w|w.is_mapped()&&w.is_sensitive()&&w.root().as_ref()==Some(parent.upcast_ref())){focus.grab_focus();}
            });
        });
        let keys=gtk::EventControllerKey::new();keys.set_name(Some("papo-dialog-keys"));let weak=window.downgrade();
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

// Icon-only buttons need a name as well as a visual hover hint. Native labels
// remain the names of text buttons; no widget-tree traversal runs while scrolling.
pub(crate) fn name_controls(widget:&gtk::Widget){
    let icon_only=widget.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().is_none())||widget.is::<gtk::MenuButton>();
    if icon_only{if let Some(name)=widget.tooltip_text().filter(|s|!s.is_empty()){widget.update_property(&[gtk::accessible::Property::Label(&name)]);}}
    let mut child=widget.first_child();while let Some(w)=child{if !w.is::<gtk::Popover>(){name_controls(&w);}child=w.next_sibling();}
}
