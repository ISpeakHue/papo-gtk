//! EN-US: Embedded artwork and control icons, independent of the desktop's installed icon theme.
//! PT-BR: Arte e ícones de controles incorporados, independentes do tema de ícones instalado no desktop.

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
    // EN-US: Papo-prefixed names resolve our embedded symbols even when the desktop selects an unavailable theme.
    // PT-BR: Nomes com prefixo Papo resolvem os símbolos incorporados mesmo se o desktop selecionar um tema indisponível.
    if !theme.resource_path().iter().any(|path| path == ICON_RESOURCE_PATH) {
        theme.add_resource_path(ICON_RESOURCE_PATH);
    }
    gtk::Window::set_default_icon_name(APP_ID);
}

#[cfg(test)]
pub(crate) fn exercise_symbols() {
    use gtk::prelude::*;
    let directory = format!("{ICON_RESOURCE_PATH}/scalable/actions/");
    let files = gtk::gio::resources_enumerate_children(&directory, gtk::gio::ResourceLookupFlags::NONE).unwrap();
    assert!(!files.is_empty());
    // EN-US: No filesystem theme may supply a missing icon or mask an incomplete resource bundle.
    // PT-BR: Nenhum tema do sistema pode suprir ícones ausentes ou ocultar um pacote de recursos incompleto.
    for name in ["Adwaita", "breeze", "PapoUnavailableDesktopTheme"] {
        let theme = gtk::IconTheme::new();
        theme.set_search_path(&[]);
        theme.set_resource_path(&[ICON_RESOURCE_PATH]);
        theme.set_theme_name(Some(name));
        for file in &files {
            let icon_name = file.strip_suffix(".svg").unwrap();
            for scale in [1, 2] {
                assert!(theme.has_icon(icon_name), "{name}: {icon_name}");
                let icon = theme.lookup_icon(icon_name, &[], 16, scale, gtk::TextDirection::Ltr, gtk::IconLookupFlags::empty());
                assert!(icon.is_symbolic(), "{icon_name} must retain GTK foreground tinting");
                let source = icon.file().unwrap();
                assert_eq!(source.uri(), format!("resource://{directory}{file}"));
                let pixels = gtk::gdk::Texture::from_file(&source).unwrap();
                assert_eq!((pixels.width(), pixels.height()), (16, 16));
            }
        }
    }
}
