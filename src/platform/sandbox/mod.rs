//! EN-US: Decoder isolation is implemented per OS; unavailable backends fail closed.
//! PT-BR: O isolamento dos decodificadores é específico por SO; backends ausentes bloqueiam a execução.
#[cfg(target_os="linux")]mod linux;
#[cfg(target_os="linux")]pub(crate) use linux::command;
#[cfg(windows)]mod windows;
#[cfg(windows)]pub(crate) use windows::command;
#[cfg(windows)]pub(crate) use windows::check;
#[cfg(windows)]pub(crate) use windows::parent_probe;
#[cfg(windows)]pub(crate) use windows::prepare;
#[cfg(not(windows))]pub(crate) async fn prepare()->anyhow::Result<()>{Ok(())}

#[cfg(not(any(target_os="linux",windows)))]
pub(crate) struct Command;
#[cfg(not(any(target_os="linux",windows)))]
impl Command{pub async fn output(&mut self)->std::io::Result<std::process::Output>{Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"Isolated media helpers are unavailable on this platform"))}}
#[cfg(not(any(target_os="linux",windows)))]
pub(crate) fn command(_path:&std::path::Path,_script:&str)->anyhow::Result<Command>{
    anyhow::bail!("Conversão isolada de mídia ainda indisponível nesta plataforma.")
}

#[cfg(all(test,not(any(target_os="linux",windows))))]mod tests{
    #[test]fn unsupported_converter_never_starts_a_process(){assert!(super::command(std::path::Path::new("missing.mov"),"raise AssertionError('must not run')").is_err());}
}
