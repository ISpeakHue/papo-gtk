//! EN-US: Capture sources are OS-specific; WebRTC signaling and decoding stay shared.
//! PT-BR: Fontes de captura dependem do SO; sinalização WebRTC e decodificação continuam compartilhadas.
#[cfg(target_os="linux")]
pub const WORKER:&str=concat!(include_str!("../../voice/screen_portal.py"),"\n",include_str!("linux.py"),"\n",include_str!("../../voice/engine.py"));
#[cfg(target_os="windows")]
pub const WORKER:&str=concat!(include_str!("windows.py"),"\n",include_str!("../../voice/engine.py"));
#[cfg(not(any(target_os="linux",target_os="windows")))]
pub const WORKER:&str=concat!(include_str!("unavailable.py"),"\n",include_str!("../../voice/engine.py"));

// EN-US: Windows preview stays opt-in until attended source/peer acceptance passes.
// PT-BR: A prévia Windows continua opcional até passar a aceitação assistida de fonte/cliente.
pub fn available()->bool{
    #[cfg(target_os="windows")]
    { std::env::var_os("PAPO_WINDOWS_CAPTURE_PREVIEW").as_deref()==Some(std::ffi::OsStr::new("1"))
        && super::runtime::directory().is_some_and(|root|root.join("bin/papo-capture.dll").is_file()) }
    #[cfg(not(target_os="windows"))]
    { super::CURRENT.screen_capture }
}
