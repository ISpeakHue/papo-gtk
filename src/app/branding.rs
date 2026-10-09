//! Embedded application artwork, available even without a desktop installation.

use std::sync::Once;

pub const APP_ID: &str = "br.com.papo.gtk";
const ICON_RESOURCE_PATH: &str = "/br/com/papo/gtk/icons";

pub fn install() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        gtk::gio::resources_register_include!("papo.gresource")
            .expect("Failed to register Papo's embedded icons");
    });
    let display = gtk::gdk::Display::default().expect("GTK must be initialized before branding");
    let theme = gtk::IconTheme::for_display(&display);
    if !theme.resource_path().iter().any(|path| path == ICON_RESOURCE_PATH) {
        theme.add_resource_path(ICON_RESOURCE_PATH);
    }
    gtk::Window::set_default_icon_name(APP_ID);
}
