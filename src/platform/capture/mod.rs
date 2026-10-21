//! EN-US: Capture sources are OS-specific; WebRTC signaling and decoding stay shared.
//! PT-BR: Fontes de captura dependem do SO; sinalização WebRTC e decodificação continuam compartilhadas.
#[cfg(target_os="linux")]
pub const WORKER:&str=concat!(include_str!("../../voice/screen_portal.py"),"\n",include_str!("linux.py"),"\n",include_str!("../../voice/engine.py"));
#[cfg(not(target_os="linux"))]
pub const WORKER:&str=concat!(include_str!("unavailable.py"),"\n",include_str!("../../voice/engine.py"));
