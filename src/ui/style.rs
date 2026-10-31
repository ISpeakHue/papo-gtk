//! EN-US: App-specific geometry and surfaces; Adwaita supplies theme/accent/contrast.
//! PT-BR: Geometria e superfícies específicas do aplicativo; tema, cores de destaque e contraste vêm do Adwaita.
mod accents;
#[cfg(test)] pub(crate) use accents::{exercise as exercise_accents,sample as sample_accent};
pub(super) use accents::apply as embed_accent;
use std::cell::Cell;
thread_local! { static INSTALLED: Cell<bool> = const { Cell::new(false) }; }
pub fn install() {
    INSTALLED.with(|installed| {
        if installed.get() { return; }
        let Some(display) = gtk::gdk::Display::default() else { return; };
        #[cfg(windows)] windows_text(&display);
        let provider = gtk::CssProvider::new();
        provider.load_from_string(include_str!("style.css"));
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        super::dialogs::install();
        installed.set(true);
    });
}

#[cfg(windows)]
fn windows_text(display:&gtk::gdk::Display){
    use gtk::prelude::*;
    let settings=gtk::Settings::for_display(display);
    // Preserve the system family and larger accessibility sizes. Match the
    // 11pt Adwaita baseline instead of inheriting Win32's tiny 9pt menu font.
    let mut font=gtk::pango::FontDescription::from_string(settings.gtk_font_name().as_deref().unwrap_or("Segoe UI 9"));
    if !font.is_size_absolute()&&font.size()<11*gtk::pango::SCALE {
        font.set_size(11*gtk::pango::SCALE);settings.set_gtk_font_name(Some(&font.to_string()));
    }
    // Available since GTK 4.16; retain the project's GTK 4.12 minimum.
    if let Some(value)=settings.find_property("gtk-font-rendering")
        .and_then(|p|gtk::glib::EnumClass::with_type(p.value_type()))
        .and_then(|c|c.to_value_by_nick("manual")) {
        settings.set_property_from_value("gtk-font-rendering",&value);
    }
    settings.set_gtk_xft_antialias(1);
    settings.set_gtk_xft_hinting(1);
    settings.set_gtk_xft_hintstyle(Some("hintslight"));
    settings.set_gtk_hint_font_metrics(true);
}

#[cfg(all(test,windows))]
pub(crate) fn exercise_windows_text(){
    use gtk::prelude::*;
    let settings=gtk::Settings::default().unwrap();
    let font=gtk::pango::FontDescription::from_string(&settings.gtk_font_name().unwrap());
    assert!(font.is_size_absolute()||font.size()>=11*gtk::pango::SCALE);
    assert_eq!(settings.gtk_xft_antialias(),1);
    assert_eq!(settings.gtk_xft_hintstyle().as_deref(),Some("hintslight"));
    let label=gtk::Label::new(Some("Papo — á é ç 🙂"));
    assert_eq!(label.layout().unknown_glyphs_count(),0,"Portuguese and emoji must have font fallback");
    if std::env::var("PANGOCAIRO_BACKEND").as_deref()==Ok("fontconfig") {
        assert_eq!(label.pango_context().font_map().unwrap().type_().name(),"PangoCairoFcFontMap");
    }
    // Larger configured fonts and the system family survive repeated setup.
    settings.set_gtk_font_name(Some("Segoe UI 15"));
    windows_text(&gtk::gdk::Display::default().unwrap());
    assert_eq!(settings.gtk_font_name().as_deref(),Some("Segoe UI 15"));
    settings.set_gtk_font_name(Some(&font.to_string()));
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
