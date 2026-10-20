//! EN-US: Theme-independent presence badges shared by members, DMs and the user footer.
//! PT-BR: Indicadores de presença independentes do tema, compartilhados por membros, conversas diretas e rodapé.
use gtk::prelude::*;
use crate::ws::PresenceStatus;

pub fn resource(status:PresenceStatus)->&'static str {
    match status {
        PresenceStatus::Online=>"/br/com/papo/gtk/icons/status/online.svg",
        PresenceStatus::Away=>"/br/com/papo/gtk/icons/status/away.svg",
        PresenceStatus::Busy=>"/br/com/papo/gtk/icons/status/busy.svg",
        PresenceStatus::Offline=>"/br/com/papo/gtk/icons/status/offline.svg",
    }
}
pub fn label(status:PresenceStatus)->&'static str {
    match status {PresenceStatus::Online=>"Disponível",PresenceStatus::Away=>"Ausente",PresenceStatus::Busy=>"Ocupado",PresenceStatus::Offline=>"Offline"}
}
pub fn manual(status:Option<&crate::models::UserStatus>)->PresenceStatus {
    match status {Some(crate::models::UserStatus::Away)=>PresenceStatus::Away,Some(crate::models::UserStatus::Busy)=>PresenceStatus::Busy,None=>PresenceStatus::Online}
}
pub fn set(image:&gtk::Image,status:PresenceStatus){
    image.set_resource(Some(resource(status)));image.set_tooltip_text(Some(label(status)));
    image.update_property(&[gtk::accessible::Property::Label(label(status))]);
}
pub fn image(status:PresenceStatus)->gtk::Image {
    let image=gtk::Image::new();image.set_pixel_size(12);image.set_halign(gtk::Align::Center);image.set_valign(gtk::Align::Center);image.add_css_class("papo-status-dot");set(&image,status);image
}

#[cfg(test)] pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use adw::prelude::AdwWindowExt;
    use crate::ui::chat::actions::tests::{pump,until};
    let content=gtk::Box::new(gtk::Orientation::Horizontal,16);
    for status in [PresenceStatus::Online,PresenceStatus::Away,PresenceStatus::Busy,PresenceStatus::Offline]{
        // A resource paintable must resolve without any desktop theme lookup.
        let data=gtk::gio::resources_lookup_data(resource(status),gtk::gio::ResourceLookupFlags::NONE).unwrap();assert!(!data.is_empty());
        let icon=image(status);icon.set_pixel_size(24);assert!(icon.paintable().is_some());assert_eq!(icon.tooltip_text().as_deref(),Some(label(status)));content.append(&icon);
        set(&icon,PresenceStatus::Offline);assert_eq!(icon.tooltip_text().as_deref(),Some("Offline"));set(&icon,status);
    }
    let window=adw::Window::builder().default_width(280).default_height(80).content(&content).build();window.present();until(context,||content.is_mapped());
    let manager=adw::StyleManager::default();let scheme=manager.color_scheme();
    for (color,name) in [(adw::ColorScheme::ForceLight,"status-light"),(adw::ColorScheme::ForceDark,"status-dark")]{manager.set_color_scheme(color);crate::ui::main_window::layout::tests::preview(&window,name,context);}
    manager.set_color_scheme(scheme);window.set_content(None::<&gtk::Widget>);window.close();pump(context);
}
