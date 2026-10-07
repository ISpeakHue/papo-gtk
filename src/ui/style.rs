//! App-specific geometry and surfaces; Adwaita supplies theme/accent/contrast.
use std::cell::Cell;
thread_local! { static INSTALLED: Cell<bool> = const { Cell::new(false) }; }
pub fn install() {
    INSTALLED.with(|installed| {
        if installed.get() { return; }
        let Some(display) = gtk::gdk::Display::default() else { return; };
        let provider = gtk::CssProvider::new();
        provider.load_from_string(include_str!("style.css"));
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        installed.set(true);
    });
}

pub fn role_color(label:&gtk::Label,roles:&[crate::models::RoleSummary]){
    use gtk::prelude::*;
    if let Some((r,g,b))=crate::models::role::role_rgb(roles){let attributes=gtk::pango::AttrList::new();attributes.insert(gtk::pango::AttrColor::new_foreground(r,g,b));label.set_attributes(Some(&attributes));label.add_css_class("papo-role-name");}
}

/// Custom popover buttons close their menu before opening a dialog or navigating.
pub fn close_popovers_on_action(root:&impl gtk::prelude::IsA<gtk::Widget>) {
    use gtk::prelude::*;
    fn buttons(widget:&gtk::Widget, popover:&gtk::Popover) {
        if let Some(button)=widget.downcast_ref::<gtk::Button>() {
            let weak=popover.downgrade();button.connect_clicked(move |_|{if let Some(p)=weak.upgrade(){p.popdown();}});
        }
        let mut child=widget.first_child();while let Some(w)=child {buttons(&w,popover);child=w.next_sibling();}
    }
    fn walk(widget:&gtk::Widget) {
        if let Some(popover)=widget.downcast_ref::<gtk::Popover>(){if let Some(child)=popover.child(){buttons(&child,popover);}return;}
        let mut child=widget.first_child();while let Some(w)=child{walk(&w);child=w.next_sibling();}
    }
    walk(root.as_ref());
}
