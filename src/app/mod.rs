//! Application root — wires Relm4, the async runtime and all top-level components together.
//! EN: Initializes libadwaita, sets desktop metadata, and starts the Relm4 root application.
//! PT-BR: Raiz da aplicação — conecta Relm4, runtime assíncrono e todos os componentes de nível superior. Inicializa libadwaita, define metadados de desktop e inicia a aplicação raiz Relm4.

use relm4::prelude::*;

use crate::ui::root::RootModel;

pub(crate) mod branding;
pub use branding::APP_ID;

/// Entry point called from `main`.
/// EN: Launches the graphical user interface.
/// PT-BR: Ponto de entrada chamado a partir de `main`. Inicia a interface gráfica do usuário.
pub fn run() {
    // EN: Initialize libadwaita — required for dark mode, animations, and system color scheme support.
    // PT-BR: Inicializa a libadwaita — obrigatório para suporte a modo escuro, animações e esquema de cores do sistema.
    adw::init().expect("Failed to initialize libadwaita");

    // EN: Configure the application user-facing title in the desktop environment.
    // PT-BR: Configura o nome amigável da aplicação no ambiente desktop.
    gtk::glib::set_application_name("Papo");

    // EN: Create Relm4 application using the unique reverse-DNS application ID.
    // PT-BR: Cria a aplicação Relm4 utilizando o identificador único reverso de DNS (APP_ID).
    let app = RelmApp::new(APP_ID);

    // EN: Run the application with `RootModel` as the root component.
    // PT-BR: Executa a aplicação com `RootModel` como o componente raiz.
    app.run::<RootModel>(());
}
