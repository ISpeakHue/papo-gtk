//! EN-US: Capture sources are OS-specific; WebRTC signaling and decoding stay shared.
//! PT-BR: Fontes de captura dependem do SO; sinalização WebRTC e decodificação continuam compartilhadas.
#[cfg(target_os="linux")]
pub const WORKER:&str=concat!(include_str!("../../voice/screen_portal.py"),"\n",include_str!("linux.py"),"\n",include_str!("../../voice/engine.py"));
#[cfg(target_os="windows")]
pub const WORKER:&str=concat!(include_str!("windows.py"),"\n",include_str!("../../voice/engine.py"));
#[cfg(not(any(target_os="linux",target_os="windows")))]
pub const WORKER:&str=concat!(include_str!("unavailable.py"),"\n",include_str!("../../voice/engine.py"));

// EN-US: Windows sharing needs its bundled bridge; the worker checks WGC support before selection.
// PT-BR: Compartilhar no Windows exige a ponte incluída; o worker verifica WGC antes da seleção.
pub fn available()->bool{
    #[cfg(target_os="windows")]
    { super::runtime::directory().is_some_and(|root|root.join("bin/papo-capture.dll").is_file()) }
    #[cfg(not(target_os="windows"))]
    { super::CURRENT.screen_capture }
}
