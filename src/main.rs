//! Papo GTK — Application entry point.
//! EN-US: Initializes tracing/logging, Tokio multi-threaded async runtime, and hands off execution to the Relm4/GTK4 application.
//! PT-BR: Ponto de entrada da aplicação. Inicializa rastreamento/logs, runtime assíncrono multi-thread do Tokio e transfere o controle para a aplicação Relm4/GTK4.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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
mod platform;

fn main() {
    let argument=std::env::args().nth(1);
    if argument.as_deref()==Some("--version"){println!("Papo {}",env!("CARGO_PKG_VERSION"));return;}
    if argument.as_deref()==Some("--platform-info"){
        println!("{}",serde_json::to_string_pretty(&platform::CURRENT).expect("platform metadata is serializable"));return;
    }
    #[cfg(windows)] if argument.as_deref()==Some("--check-activation-policy"){
        if let Err(error)=platform::activation::check_policy(){eprintln!("Activation policy failed: {error}");std::process::exit(1);}
        println!("Windows account-bound activation policy OK");return;
    }
    platform::runtime::configure();
    // EN-US: Initialize structured logging/tracing subsystem.
    // PT-BR: Inicializa o subsistema de rastreamento e logging estruturado.
    logging::init();

    // EN-US: Create a Tokio runtime to allow reqwest HTTP client, WebSockets, and other async tasks to execute.
    // PT-BR: Cria um runtime Tokio para permitir a execução do cliente HTTP reqwest, WebSockets e outras tarefas assíncronas.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    if argument.as_deref()==Some("--check-packaged-runtime"){
        if let Err(error)=rt.block_on(platform::runtime::check_packaged()){eprintln!("Packaged runtime check failed: {error}");std::process::exit(1);}
        println!("Papo {}: packaged runtime OK",env!("CARGO_PKG_VERSION"));return;
    }
    #[cfg(windows)]if argument.as_deref()==Some("--check-notifications"){
        if let Err(error)=rt.block_on(platform::notifications::check()){eprintln!("Notification check failed: {error}");std::process::exit(1);}return;
    }

    if argument.as_deref()==Some("--check-media-runtime"){
        if let Err(error)=rt.block_on(media::runtime::check()){eprintln!("Media runtime check failed: {error}");std::process::exit(1);}
        println!("Papo {}: isolated media runtime OK",env!("CARGO_PKG_VERSION"));return;
    }

    // EN-US: Private synthetic parent-exit fixture; never starts normal UI or reads real sessions.
    // PT-BR: Fixture sintética privada de saída do processo pai; não inicia a interface nem lê sessões reais.
    #[cfg(windows)]
    if argument.as_deref()==Some("--windows-media-parent-probe"){
        let args:Vec<_>=std::env::args_os().skip(2).collect();
        if args.len()!=2{std::process::exit(2);}
        if let Err(error)=rt.block_on(platform::sandbox::parent_probe(std::path::Path::new(&args[0]),std::path::Path::new(&args[1]))){
            eprintln!("Media parent probe failed: {error}");std::process::exit(1);
        }
        return;
    }

    // EN-US: Explicit acceptance uses disposable namespaces and loopback fixtures before GUI/session startup.
    // PT-BR: A aceitação explícita usa namespaces descartáveis e testes locais antes de iniciar a interface/sessão.
    #[cfg(all(windows,not(test)))]
    if matches!(argument.as_deref(),Some("--check-windows-storage"|"--windows-storage-child")){
        let result=if argument.as_deref()==Some("--windows-storage-child"){
            rt.block_on(platform::storage_check::child(&std::env::args().skip(2).collect::<Vec<_>>()))
        }else{platform::storage_check::run(&rt)};
        if let Err(error)=result{eprintln!("Windows storage acceptance failed: {error}");std::process::exit(1);}
        return;
    }

    #[cfg(windows)]
    let _windows_startup = match rt.block_on(platform::startup::start()) {
        Ok(Some(guard)) => guard,
        Ok(None) => return,
        Err(error) => { eprintln!("Windows startup failed: {error}"); std::process::exit(1); }
    };

    // EN-US: Enter the runtime context. The guard will keep it active for the thread.
    // PT-BR: Entra no contexto do runtime Tokio. A guarda mantém o contexto ativo para a thread principal.
    let _guard = rt.enter();

    // EN-US: Performance tracking report handle.
    // PT-BR: Manipulador do relatório de rastreamento de desempenho.
    let _performance_report=performance::Report;

    // EN-US: Start the Relm4 UI loop and present the application window.
    // PT-BR: Inicia o loop de interface gráfica Relm4 e exibe a janela da aplicação.
    app::run();

    // EN-US: Gracefully shut down async background tasks upon application exit with a 2-second timeout.
    // PT-BR: Encerramento gracioso das tarefas assíncronas em segundo plano ao sair da aplicação com timeout de 2 segundos.
    rt.shutdown_timeout(std::time::Duration::from_secs(2));
}
