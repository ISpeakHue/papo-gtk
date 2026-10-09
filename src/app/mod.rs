//! Application root — wires Relm4, the async runtime and all top-level
//! components together.

use relm4::prelude::*;

use crate::ui::root::RootModel;

pub(crate) mod branding;
pub use branding::APP_ID;

/// Entry point called from `main`.
pub fn run() {
    // Initialize libadwaita — required for dark mode / system color scheme support
    adw::init().expect("Failed to initialize libadwaita");

    gtk::glib::set_application_name("Papo");
    let app = RelmApp::new(APP_ID);
    app.run::<RootModel>(());
}
