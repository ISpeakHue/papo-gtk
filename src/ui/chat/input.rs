//! Small editing helpers keep composer offsets in GTK character coordinates.
use super::*;
mod diagnostics;
pub(super) mod shortcuts;
/// Choose one input transport before any login/chat editable is constructed.
/// Mixing a protocol-backed login field with a Compose-only composer can leave
/// the compositor handling dead keys for the old field during view changes.
/// This setting belongs only to this process, never the desktop configuration.
pub(crate) fn configure_desktop_im(){
    let Some(display)=gtk::gdk::Display::default()else{return;};
    if display.type_().name()!="GdkWaylandDisplay"{return;}
    let settings=gtk::Settings::for_display(&display);apply_desktop_im(&settings);
    thread_local!{static CONNECTED:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
    if !CONNECTED.with(|connected|connected.replace(true)){
        settings.connect_notify_local(Some("gtk-im-module"),move |settings,_|apply_desktop_im(settings));
    }
}
fn apply_desktop_im(settings:&gtk::Settings){
    let env=std::env::var("GTK_IM_MODULE").ok();let modifiers=std::env::var("XMODIFIERS").ok();let current=settings.gtk_im_module();
    let point=gtk::gio::IOExtensionPoint::lookup("gtk-im-module");
    if let Some(module)=im_selection(true,env.as_deref(),current.as_deref(),modifiers.as_deref(),|name|point.as_ref().is_some_and(|p|p.extension_by_name(name).is_some())){
        if current.as_deref()!=Some(module){settings.set_gtk_im_module(Some(module));}
    }
}
/// Prefer an available desktop IM client on Wayland, with a Compose fallback
/// even when the terminal has no XMODIFIERS hint. Leave valid GTK preferences
/// to GTK; an unavailable module must not silently select a broken fallback.
pub(super) fn configure_im(view:&gtk::TextView){
    apply_im(view);view.connect_map(apply_im);
    // Portal settings can arrive after construction, or change while running.
    // Updating only when the selection differs preserves an active preedit.
    let settings=view.settings();let weak=view.downgrade();
    let handler=settings.connect_notify_local(Some("gtk-im-module"),move |_,_|{if let Some(view)=weak.upgrade(){apply_im(&view);}});
    let handler=std::cell::RefCell::new(Some(handler));view.connect_destroy(move |_|{if let Some(handler)=handler.borrow_mut().take(){settings.disconnect(handler);}});
    diagnostics::connect(view);
}
fn apply_im(view:&gtk::TextView){
    let env=std::env::var("GTK_IM_MODULE").ok();let modifiers=std::env::var("XMODIFIERS").ok();let settings=view.settings().gtk_im_module();
    let point=gtk::gio::IOExtensionPoint::lookup("gtk-im-module");
    let module=im_selection(view.display().type_().name()=="GdkWaylandDisplay",env.as_deref(),settings.as_deref(),modifiers.as_deref(),|name|point.as_ref().is_some_and(|p|p.extension_by_name(name).is_some()));
    if view.im_module().as_deref()!=module{view.set_im_module(module);}
}
fn im_selection(wayland:bool,env:Option<&str>,settings:Option<&str>,modifiers:Option<&str>,available:impl Fn(&str)->bool)->Option<&'static str>{
    if !wayland{return None;}
    for preference in [env,settings].into_iter().flatten(){
        // GTK accepts a colon-separated list and normalizes these aliases in
        // environment/settings values, but not in the TextView property.
        if preference.split(':').any(|name|{let name=match name{"simple"=>"gtk-im-context-simple",other=>other};matches!(name,"none"|"gtk-im-context-none")||name!="xim"&&name!="gtk-im-context-xim"&&!name.is_empty()&&available(name)}){return None;}
    }
    let candidates:&[&str]=match modifiers{Some("@im=ibus")=>&["ibus"],Some("@im=fcitx5")=>&["fcitx5","fcitx"],Some("@im=fcitx")=>&["fcitx","fcitx5"],_=>&[]};
    candidates.iter().copied().find(|name|available(name)).or(Some("gtk-im-context-simple"))
}
pub(crate) trait InputExt {
    fn text(&self)->gtk::glib::GString;
    fn set_text(&self,text:&str);
    fn position(&self)->i32;
    fn set_position(&self,position:i32);
    fn select_region(&self,start:i32,end:i32);
    fn selection_bounds(&self)->Option<(i32,i32)>;
    fn insert_text(&self,text:&str,position:&mut i32);
    fn delete_text(&self,start:i32,end:i32);
    fn grab_focus_without_selecting(&self)->bool;
}
fn iter(buffer:&gtk::TextBuffer,position:i32)->gtk::TextIter{buffer.iter_at_offset(if position<0{buffer.char_count()}else{position.min(buffer.char_count())})}
impl InputExt for gtk::TextView {
    fn text(&self)->gtk::glib::GString{let b=self.buffer();b.text(&b.start_iter(),&b.end_iter(),false)}
    fn set_text(&self,text:&str){self.buffer().set_text(text);}
    fn position(&self)->i32{self.buffer().cursor_position()}
    fn set_position(&self,position:i32){let b=self.buffer();b.place_cursor(&iter(&b,position));self.scroll_mark_onscreen(&b.get_insert());}
    fn select_region(&self,start:i32,end:i32){let b=self.buffer();b.select_range(&iter(&b,end),&iter(&b,start));}
    fn selection_bounds(&self)->Option<(i32,i32)>{self.buffer().selection_bounds().map(|(a,b)|(a.offset(),b.offset()))}
    fn insert_text(&self,text:&str,position:&mut i32){let b=self.buffer();let mut at=iter(&b,*position);b.insert(&mut at,text);*position=at.offset();}
    fn delete_text(&self,start:i32,end:i32){let b=self.buffer();b.delete(&mut iter(&b,start),&mut iter(&b,end));}
    fn grab_focus_without_selecting(&self)->bool{self.grab_focus()}
}

/// Measure the native wrapped layout; only the input viewport grows, up to a
/// bounded height. Large drafts scroll vertically and never widen the chat.
pub(super) fn connect_size(view:&gtk::TextView,scroll:&gtk::ScrolledWindow){
    use std::{rc::Rc,cell::Cell};
    let pending=Rc::new(Cell::new(false));let weak=view.downgrade();let weak_scroll=scroll.downgrade();
    let update:Rc<dyn Fn()>=Rc::new(move ||{
        if pending.replace(true){return;}
        let pending=pending.clone();let weak=weak.clone();let weak_scroll=weak_scroll.clone();
        gtk::glib::idle_add_local_once(move ||{
            pending.set(false);let(Some(view),Some(scroll))=(weak.upgrade(),weak_scroll.upgrade())else{return;};
            if view.width()<=0{return;}
            let end=view.buffer().end_iter();let(y,height)=view.line_yrange(&end);
            let content=y+height+view.top_margin()+view.bottom_margin();
            let policy=if content>160{gtk::PolicyType::Automatic}else{gtk::PolicyType::External};
            if scroll.vscrollbar_policy()!=policy{scroll.set_vscrollbar_policy(policy);}
            let height=content.clamp(40,160);
            if scroll.height_request()!=height{scroll.set_height_request(height);}
        });
    });
    let refresh=update.clone();view.buffer().connect_changed(move |_|refresh());
    let refresh=update.clone();view.connect_preedit_changed(move |_,_|refresh());
    let observer=gtk::WidgetPaintable::new(Some(view));let refresh=update.clone();observer.connect_invalidate_size(move |_|refresh());
    view.connect_map(move |view|{observer.set_widget(Some(view));update();});
}
#[cfg(test)]pub(crate) fn find(root:&gtk::Widget)->gtk::TextView{
    actions::tests::descendants(root).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok().filter(|v|v.widget_name()=="message-composer")).expect("message composer")
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn wayland_compose_fallback_does_not_require_a_terminal_framework_hint(){
        let built_in=|name:&str|matches!(name,"gtk-im-context-simple"|"wayland");
        for modifiers in [None,Some(""),Some("@im=ibus"),Some("@im=fcitx5"),Some("@im=unknown")]{assert_eq!(im_selection(true,None,None,modifiers,built_in),Some("gtk-im-context-simple"));}
        for preference in ["ibus","fcitx5","missing:ibus"]{assert_eq!(im_selection(true,Some(preference),None,None,built_in),Some("gtk-im-context-simple"));assert_eq!(im_selection(true,None,Some(preference),None,built_in),Some("gtk-im-context-simple"));}
        assert_eq!(im_selection(false,None,None,None,built_in),None);
    }
    #[test]fn available_input_engines_and_gtk_preference_lists_keep_their_priority(){
        let available=|name:&str|matches!(name,"gtk-im-context-simple"|"wayland"|"ibus"|"fcitx5");
        for preference in ["simple","gtk-im-context-simple","none","gtk-im-context-none","wayland","ibus","missing:fcitx5","missing:simple"]{assert_eq!(im_selection(true,Some(preference),None,None,available),None);assert_eq!(im_selection(true,None,Some(preference),None,available),None);}
        assert_eq!(im_selection(true,Some("missing"),Some("ibus"),None,available),None);
        assert_eq!(im_selection(true,None,None,Some("@im=ibus"),available),Some("ibus"));
        for hint in ["@im=fcitx5","@im=fcitx"]{assert_eq!(im_selection(true,None,None,Some(hint),available),Some("fcitx5"));}
    }
}
