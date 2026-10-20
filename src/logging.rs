//! EN-US: Terminal diagnostics never enable third-party request, frame or UI dumps.
//! PT-BR: Diagnósticos de terminal sem despejos de requisições, quadros ou interface de bibliotecas externas.
use tracing_subscriber::{filter::filter_fn, fmt::MakeWriter, prelude::*, EnvFilter};

pub fn init() {
    subscriber(std::env::var("RUST_LOG").ok().as_deref(), std::io::stderr).init();
}

fn subscriber<W>(directives: Option<&str>, writer: W) -> impl tracing::Subscriber + Send + Sync
where W: for<'a> MakeWriter<'a> + Send + Sync + 'static {
    let filter = directives.filter(|s| !s.trim().is_empty())
        .and_then(|s| EnvFilter::builder().with_regex(false).parse(s).ok())
        .unwrap_or_else(|| EnvFilter::new("warn"));
    tracing_subscriber::registry()
        // Independent of RUST_LOG: a more specific transport directive cannot
        // re-enable dumps, and foreign spans cannot attach secrets to our logs.
        .with(filter_fn(|metadata| metadata.target() == "papo_gtk" || metadata.target().starts_with("papo_gtk::")))
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(writer))
}

/// Backend messages and request URLs may echo secrets. Keep only typed causes.
pub fn error_summary(error: &anyhow::Error) -> String {
    if let Some(http) = error.downcast_ref::<crate::api::ApiError>() {
        return format!("HTTP {}", http.status.as_u16());
    }
    if let Some(http) = error.downcast_ref::<reqwest::Error>() {
        if let Some(status) = http.status() { return format!("HTTP {}", status.as_u16()); }
        if http.is_timeout() { return "request timed out".into(); }
        if http.is_connect() { return "connection failed".into(); }
        if http.is_decode() { return "invalid response".into(); }
    }
    "operation failed".into()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::{io::{self, Write}, sync::{Arc, Mutex}};
    #[derive(Clone, Default)]
    pub(crate) struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.0.lock().unwrap().extend_from_slice(bytes); Ok(bytes.len()) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    impl Capture {
        pub(crate) fn text(&self) -> String { String::from_utf8(self.0.lock().unwrap().clone()).unwrap() }
        pub(crate) fn subscriber(&self, filter: &str) -> impl tracing::Subscriber + Send + Sync + 'static {
            let buffer = self.clone(); subscriber(Some(filter), move || buffer.clone())
        }
    }
    #[test]
    fn verbose_filters_cannot_enable_secret_dependency_events_or_parent_spans() {
        for filter in ["trace", "trace,reqwest=trace,tungstenite::handshake::client=trace,relm4=trace"] {
            let capture = Capture::default();
            tracing::subscriber::with_default(capture.subscriber(filter), || {
                let span = tracing::trace_span!(target: "tungstenite::handshake::client", "handshake", cookie = "AUTH_SECRET");
                let _guard = span.enter();
                tracing::trace!(target: "tungstenite::handshake::client", "Cookie: Auth=AUTH_SECRET");
                tracing::debug!(target: "reqwest::connect", "Authorization: Bearer AUTH_SECRET");
                tracing::warn!(target: "hyper::proto", "Set-Cookie: Auth=AUTH_SECRET");
                tracing::error!(target: "relm4::component", "password=PASSWORD_SECRET");
                tracing::debug!(target: "papo_gtk::api::requests", method = "POST", status = 200, "HTTP request completed");
            });
            let output = capture.text();
            assert!(output.contains("HTTP request completed") && output.contains("200"));
            assert!(!output.contains("AUTH_SECRET") && !output.contains("PASSWORD_SECRET"));
        }
    }
    #[test]
    fn normal_and_invalid_filters_keep_warnings_but_hide_request_chatter() {
        for filter in [None, Some(""), Some(" "), Some("not a valid [filter"), Some("warn")] {
            let capture = Capture::default(); let buffer = capture.clone();
            tracing::subscriber::with_default(subscriber(filter, move || buffer.clone()), || {
                tracing::debug!("HTTP request completed");
                tracing::warn!("Connection failed");
            });
            let output = capture.text();assert!(output.contains("Connection failed"));assert!(!output.contains("HTTP request completed"));
        }
    }
    #[test]
    fn server_errors_log_status_without_echoing_secrets() {
        let error = anyhow::Error::new(crate::api::ApiError { status: reqwest::StatusCode::BAD_GATEWAY, code: Some("AUTH_SECRET".into()), message: "Cookie: Auth=AUTH_SECRET".into() });
        assert_eq!(error_summary(&error), "HTTP 502");
    }
}
