//! EN-US: Native/packaged helper paths and child-process behavior, without changing desktop preferences.
//! PT-BR: Caminhos nativos e empacotados dos auxiliares e configuração dos processos filhos, sem alterar preferências do desktop.
use super::{Os, CURRENT};
use std::{ffi::{OsStr, OsString}, path::{Path, PathBuf}};

fn runtime_root(exe: &Path, os: Os) -> Option<PathBuf> {
    let directory = exe.parent()?;
    Some(match os {
        Os::Macos => directory.join("../Resources/runtime"),
        Os::Windows if directory.file_name() == Some(OsStr::new("bin")) => directory.parent()?.to_path_buf(),
        _ => directory.join("runtime"),
    })
}

fn python_path(os: Os, override_path: Option<OsString>, runtime: Option<OsString>, exe: Option<&Path>, exists: impl Fn(&Path) -> bool) -> OsString {
    if let Some(path) = override_path.filter(|p| !p.is_empty()) { return path; }
    let root = runtime.filter(|p| !p.is_empty()).map(PathBuf::from)
        .or_else(|| exe.and_then(|exe| runtime_root(exe, os)));
    let bundled = if os == Os::Linux {
        exe.and_then(Path::parent).map(|directory| directory.join("../libexec/papo/python3"))
    } else {
        root.map(|root| root.join(if os == Os::Windows { "bin/python.exe" } else { "bin/python3" }))
    };
    // EN-US: Missing Windows runtime files must fail, never select an unrelated system Python.
    // PT-BR: Arquivos ausentes no runtime Windows devem falhar, nunca selecionar outro Python do sistema.
    if let Some(path) = bundled.filter(|p| os == Os::Windows || exists(p)) { return path.into_os_string(); }
    OsString::from(match os { Os::Linux => "/usr/bin/python3", Os::Windows => "runtime/bin/python.exe", Os::Macos => "python3" })
}

pub fn python() -> tokio::process::Command {
    let exe = std::env::current_exe().ok();
    let mut cmd = tokio::process::Command::new(python_path(CURRENT.os, std::env::var_os("PAPO_PYTHON"),
        std::env::var_os("PAPO_RUNTIME_DIR"), exe.as_deref(), Path::is_file));
    // EN-US: Background helpers must not open extra console windows on Windows.
    // PT-BR: Auxiliares em segundo plano não devem abrir janelas extras de console no Windows.
    #[cfg(windows)] cmd.creation_flags(0x08000000);
    #[cfg(not(windows))] let _ = &mut cmd;
    cmd
}

pub fn configure() {
    if CURRENT.os == Os::Linux { return; }
    let root = std::env::var_os("PAPO_RUNTIME_DIR").filter(|p| !p.is_empty()).map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok().and_then(|exe| runtime_root(&exe, CURRENT.os)));
    let Some(root) = root.filter(|p| p.is_dir()) else { return; };
    let python = root.join(if CURRENT.os == Os::Windows { "bin/python.exe" } else { "bin/python3" });
    if python.is_file() {
        set_default("PAPO_PYTHON", python.as_os_str());
        if CURRENT.os == Os::Windows && std::env::var_os("PAPO_PYTHON").as_deref() == Some(python.as_os_str()) {
            set_default("PYTHONHOME", root.as_os_str());
            set_default("PYTHONNOUSERSITE", OsStr::new("1"));
            set_default("PYTHONSAFEPATH", OsStr::new("1"));
        }
    }
    let scanner = if CURRENT.os == Os::Windows { "libexec/gstreamer-1.0/gst-plugin-scanner.exe" } else { "libexec/gstreamer-1.0/gst-plugin-scanner" };
    for (name, suffix) in [("GI_TYPELIB_PATH", "lib/girepository-1.0"), ("GSETTINGS_SCHEMA_DIR", "share/glib-2.0/schemas"),
                           ("GST_PLUGIN_SYSTEM_PATH_1_0", "lib/gstreamer-1.0"), ("GST_PLUGIN_SCANNER_1_0", scanner)] {
        let path = root.join(suffix);
        if path.exists() { set_default(name, path.as_os_str()); }
    }
}
fn set_default(key: &str, value: &OsStr) {
    if std::env::var_os(key).is_none_or(|v| v.is_empty()) { std::env::set_var(key, value); }
}

/// EN-US: Validate a shipped runtime without reading settings, credentials or contacting a server.
/// PT-BR: Valida o runtime distribuído sem ler configurações, credenciais nem acessar um servidor.
pub async fn check_packaged() -> anyhow::Result<()> {
    anyhow::ensure!(CURRENT.os == Os::Windows, "This check currently validates the Windows package only.");
    let exe = std::env::current_exe()?;
    let root = runtime_root(&exe, CURRENT.os).ok_or_else(|| anyhow::anyhow!("Runtime directory unavailable"))?;
    anyhow::ensure!(root.join("RUNTIME-MANIFEST.json").is_file(), "Runtime manifest missing");
    let result = tokio::time::timeout(std::time::Duration::from_secs(120),
        python().args(["-s", "-P"]).arg(root.join("share/papo-gtk/runtime_check.py")).arg(&exe).output()).await??;
    anyhow::ensure!(result.status.success(), "Packaged Python/GI/media check failed: {}\n{}",
        String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    println!("{}", String::from_utf8_lossy(&result.stdout));
    use gtk::prelude::*;
    gtk::init()?;
    adw::init()?;
    crate::app::branding::install();
    let texture = gtk::gdk::Texture::from_resource("/br/com/papo/gtk/icons/128x128/apps/br.com.papo.gtk.png");
    anyhow::ensure!(texture.width() == 128, "Invalid embedded window icon");
    let label = gtk::Label::new(Some("Papo — á é ç — runtime"));
    let window = adw::Window::builder().title("Papo — runtime check").default_width(420).default_height(180).content(&label).build();
    window.present();
    let context = gtk::glib::MainContext::default();
    for _ in 0..20 {
        while context.pending() { context.iteration(false); }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let mapped = window.is_mapped() && window.width() > 0;
    window.close();
    anyhow::ensure!(mapped, "Packaged GTK window did not map");
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn packaged_python_keeps_spaces_and_explicit_overrides() {
        let exe = Path::new("/Applications/Papo Preview.app/Contents/MacOS/papo-gtk");
        let mac = python_path(Os::Macos, None, None, Some(exe), |_| true);
        assert_eq!(Path::new(&mac), exe.parent().unwrap().join("../Resources/runtime/bin/python3"));
        let explicit = OsString::from("/custom path/python");
        assert_eq!(python_path(Os::Windows, Some(explicit.clone()), None, Some(exe), |_| true), explicit);
        let exe = Path::new("/Papo á/runtime/bin/papo-gtk.exe");
        assert_eq!(Path::new(&python_path(Os::Windows, None, None, Some(exe), |_| true)), exe.parent().unwrap().join("python.exe"));
    }
    #[test] fn missing_windows_runtime_never_uses_path_python() {
        let exe = Path::new("/Papo Preview/runtime/bin/papo-gtk.exe");
        let selected = python_path(Os::Windows, None, None, Some(exe), |_| false);
        assert_eq!(Path::new(&selected), Path::new("/Papo Preview/runtime/bin/python.exe"));
        let explicit_root = OsString::from("/development runtime");
        assert_eq!(Path::new(&python_path(Os::Windows, None, Some(explicit_root), Some(exe), |_| false)),
                   Path::new("/development runtime/bin/python.exe"));
    }
    #[test] fn other_platform_development_fallbacks_are_preserved() {
        for (os, expected) in [(Os::Linux, "/usr/bin/python3"), (Os::Macos, "python3")] {
            assert_eq!(python_path(os, Some(OsString::new()), None, None, |_| false), OsString::from(expected));
        }
    }
}
