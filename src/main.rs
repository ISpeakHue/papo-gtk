//! Papo GTK — Application entry point.
//! EN: Initialises tracing/logging, Tokio multi-threaded async runtime, and hands off execution to the Relm4/GTK4 application.
//! PT-BR: Ponto de entrada da aplicação. Inicializa rastreamento/logs, runtime assíncrono multi-thread do Tokio e transfere o controle para a aplicação Relm4/GTK4.

mod api;
mod app;
mod config;
mod session;
mod media;
mod models;
mod ui;
mod ws;
mod voice;
mod logging;
mod performance;

fn main() {
    let argument=std::env::args().nth(1);
    if argument.as_deref()==Some("--version"){println!("Papo {}",env!("CARGO_PKG_VERSION"));return;}
    // EN: Initialize structured logging/tracing subsystem.
    // PT-BR: Inicializa o subsistema de rastreamento e logging estruturado.
    logging::init();

    // EN: Create a Tokio runtime to allow reqwest HTTP client, WebSockets, and other async tasks to execute.
    // PT-BR: Cria um runtime Tokio para permitir a execução do cliente HTTP reqwest, WebSockets e outras tarefas assíncronas.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    if argument.as_deref()==Some("--check-media-runtime"){
        if let Err(error)=rt.block_on(media::runtime::check()){eprintln!("Media runtime check failed: {error}");std::process::exit(1);}
        println!("Papo {}: isolated media runtime OK",env!("CARGO_PKG_VERSION"));return;
    }

    // EN: Enter the runtime context. The guard will keep it active for the thread.
    // PT-BR: Entra no contexto do runtime Tokio. A guarda mantém o contexto ativo para a thread principal.
    let _guard = rt.enter();

    // EN: Performance tracking report handle.
    // PT-BR: Manipulador do relatório de rastreamento de desempenho.
    let _performance_report=performance::Report;

    // EN: Start the Relm4 UI loop and present the application window.
    // PT-BR: Inicia o loop de interface gráfica Relm4 e exibe a janela da aplicação.
    app::run();

    // EN: Gracefully shut down async background tasks upon application exit with a 2-second timeout.
    // PT-BR: Encerramento gracioso das tarefas assíncronas em segundo plano ao sair da aplicação com timeout de 2 segundos.
    rt.shutdown_timeout(std::time::Duration::from_secs(2));
}
