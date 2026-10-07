//! Application root — wires Relm4, the async runtime and all top-level
//! components together.

use relm4::prelude::*;

use crate::ui::root::RootModel;

/// Entry point called from `main`.
pub fn run() {
    // Initialize libadwaita — required for dark mode / system color scheme support
    adw::init().expect("Failed to initialize libadwaita");

    let app = RelmApp::new("br.com.papo.gtk");
    app.run::<RootModel>(());
}
