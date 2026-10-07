//! Papo GTK — entry point.
//!
//! Initialises tracing, tokio runtime and hands off to the Relm4 application.

mod api;
mod app;
mod config;
mod session;
mod media;
mod models;
mod ui;
mod ws;
mod voice;

fn main() {
    // Initialise logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create a Tokio runtime to allow reqwest and other async operations to work.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    // Enter the runtime context. The guard will keep it active for the thread.
    let _guard = rt.enter();

    app::run();

    // Graceful shutdown of async background tasks on application exit
    rt.shutdown_timeout(std::time::Duration::from_secs(2));
}
