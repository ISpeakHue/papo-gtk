//! Keyboard routing keeps the native text/IM event path intact.
use super::*;

fn buttons(widget:&gtk::Widget,found:&mut Vec<gtk::Button>){
    if let Some(button)=widget.downcast_ref::<gtk::Button>(){if button.is_sensitive()&&button.is_visible(){found.push(button.clone());}return;}
    let mut child=widget.first_child();while let Some(w)=child{buttons(&w,found);child=w.next_sibling();}
}
pub(crate) fn completion(panel:&gtk::ScrolledWindow,entry:&gtk::TextView){
    let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak=panel.downgrade();let entry=entry.downgrade();
    keys.connect_key_pressed(move |_,key,_,mods|{
        if mods.intersects(gtk::gdk::ModifierType::SHIFT_MASK|gtk::gdk::ModifierType::CONTROL_MASK|gtk::gdk::ModifierType::ALT_MASK|gtk::gdk::ModifierType::SUPER_MASK){return gtk::glib::Propagation::Proceed;}
        let Some(panel)=weak.upgrade()else{return gtk::glib::Propagation::Proceed;};
        let mut choices=vec![];buttons(panel.upcast_ref(),&mut choices);
        let focused=choices.iter().position(|b|b.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));
        let Some(index)=focused else{return gtk::glib::Propagation::Proceed;};
        if matches!(key,gtk::gdk::Key::Tab|gtk::gdk::Key::Return|gtk::gdk::Key::KP_Enter){choices[index].emit_clicked();return gtk::glib::Propagation::Stop;}
        if key==gtk::gdk::Key::Down{choices[(index+1).min(choices.len()-1)].grab_focus();return gtk::glib::Propagation::Stop;}
        if key==gtk::gdk::Key::Up{if index==0{if let Some(entry)=entry.upgrade(){entry.grab_focus();}}else{choices[index-1].grab_focus();}return gtk::glib::Propagation::Stop;}
        gtk::glib::Propagation::Proceed
    });panel.add_controller(keys);
}

fn typing_key(key:gtk::gdk::Key,mods:gtk::gdk::ModifierType)->bool{
    !mods.intersects(gtk::gdk::ModifierType::CONTROL_MASK|gtk::gdk::ModifierType::ALT_MASK|gtk::gdk::ModifierType::SUPER_MASK|gtk::gdk::ModifierType::META_MASK|gtk::gdk::ModifierType::HYPER_MASK)
        &&(key.to_unicode().is_some_and(|c|!c.is_control())||key.name().is_some_and(|name|name.starts_with("dead_")))
}
fn owns_text_or_popup(mut focus:Option<gtk::Widget>)->bool{
    while let Some(widget)=focus{
        if widget.is::<gtk::Editable>()||widget.downcast_ref::<gtk::TextView>().is_some_and(|v|v.is_editable()||v.buffer().has_selection())
            ||widget.downcast_ref::<gtk::Label>().is_some_and(|l|l.selection_bounds().is_some())||widget.is::<gtk::Popover>(){return true;}
        focus=widget.parent();
    }false
}
pub(crate) fn type_to_focus(view:&gtk::TextView,scroll:&gtk::ScrolledWindow,composer:composer::Composer,input:relm4::Sender<ChatMsg>){
    use std::{rc::Rc,cell::RefCell};
    // Install on the actual window so typing after a sidebar click also works.
    // Detach on reparenting; the window never holds a strong composer reference.
    let installed=Rc::new(RefCell::new(None::<(gtk::glib::WeakRef<gtk::Window>,gtk::EventControllerKey)>));
    let scroll=scroll.downgrade();
    let refresh=move |view:&gtk::TextView|{
        if let Some((window,keys))=installed.borrow_mut().take(){if let Some(window)=window.upgrade(){window.remove_controller(&keys);}}
        let Some(window)=view.root().and_downcast::<gtk::Window>()else{return;};
        let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);let weak=view.downgrade();let scroll=scroll.clone();let composer=composer.clone();let input=input.clone();
        keys.connect_key_pressed(move |controller,key,_,mods|{
            if mods.is_empty()&&matches!(key,gtk::gdk::Key::Page_Up|gtk::gdk::Key::Page_Down|gtk::gdk::Key::End)&&!composer.is_composing(){
                if let (Some(view),Some(scroll))=(weak.upgrade(),scroll.upgrade()){
                    let focus=view.root().and_downcast::<gtk::Window>().and_then(|w|gtk::prelude::RootExt::focus(&w));
                    let in_chat=focus.as_ref().is_some_and(|f|f==view.upcast_ref::<gtk::Widget>()||f.is_ancestor(&scroll));
                    if in_chat&&scroll.is_mapped(){let _=input.send(match key{gtk::gdk::Key::End=>ChatMsg::Latest,gtk::gdk::Key::Page_Up=>ChatMsg::Page(-1),_=>ChatMsg::Page(1)});return gtk::glib::Propagation::Stop;}
                }
            }
            if !typing_key(key,mods){return gtk::glib::Propagation::Proceed;}
            let Some(view)=weak.upgrade().filter(|v|v.is_mapped()&&v.is_sensitive()&&v.is_editable())else{return gtk::glib::Propagation::Proceed;};
            let Some(window)=view.root().and_downcast::<gtk::Window>()else{return gtk::glib::Propagation::Proceed;};
            if owns_text_or_popup(gtk::prelude::RootExt::focus(&window)){return gtk::glib::Propagation::Proceed;}
            if view.grab_focus()&&controller.current_event().is_some(){
                // Forward the real event, including dead keys and IM commits.
                // Re-entry sees the focused TextView and proceeds normally.
                controller.forward(&view);return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });window.add_controller(keys.clone());*installed.borrow_mut()=Some((window.downgrade(),keys));
    };
    refresh(view);view.connect_root_notify(refresh);
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn only_unmodified_typing_and_dead_keys_move_focus(){
        use gtk::gdk::{Key,ModifierType as M};
        for key in [Key::a,Key::space,Key::dead_acute]{assert!(typing_key(key,M::empty()));assert!(typing_key(key,M::SHIFT_MASK));}
        for key in [Key::Return,Key::Tab,Key::Escape,Key::Up,Key::F1]{assert!(!typing_key(key,M::empty()));}
        for mods in [M::CONTROL_MASK,M::ALT_MASK,M::SUPER_MASK]{assert!(!typing_key(Key::a,mods));}
    }
}
