//! EN-US: Native/packaged helper paths and child-process behavior, without changing desktop preferences.
//! PT-BR: Caminhos nativos e empacotados dos auxiliares e configuração dos processos filhos, sem alterar preferências do desktop.
use super::{Os,CURRENT};
use std::{ffi::{OsStr,OsString},path::{Path,PathBuf}};

fn python_path(os:Os,override_path:Option<OsString>,exe:Option<&Path>,exists:impl Fn(&Path)->bool)->OsString{
    if let Some(path)=override_path.filter(|p|!p.is_empty()){return path;}
    let bundled=exe.and_then(Path::parent).map(|directory|match os{
        Os::Windows=>directory.join("runtime/bin/python.exe"),
        Os::Macos=>directory.join("../Resources/runtime/bin/python3"),
        Os::Linux=>directory.join("../libexec/papo/python3"),
    });
    if let Some(path)=bundled.filter(|p|exists(p)){return path.into_os_string();}
    OsString::from(match os{Os::Linux=>"/usr/bin/python3",Os::Windows=>"python.exe",Os::Macos=>"python3"})
}
pub fn python()->tokio::process::Command{
    let exe=std::env::current_exe().ok();
    let mut cmd=tokio::process::Command::new(python_path(CURRENT.os,std::env::var_os("PAPO_PYTHON"),exe.as_deref(),Path::is_file));
    // EN-US: Background helpers must not open extra console windows on Windows.
    // PT-BR: Auxiliares em segundo plano não devem abrir janelas extras de console no Windows.
    #[cfg(windows)]cmd.creation_flags(0x08000000);
    #[cfg(not(windows))]let _=&mut cmd;
    cmd
}

fn runtime_root(exe:&Path,os:Os)->Option<PathBuf>{
    Some(match os{Os::Macos=>exe.parent()?.join("../Resources/runtime"),_=>exe.parent()?.join("runtime")})
}

pub fn configure(){
    if CURRENT.os==Os::Linux{return;}
    let root=std::env::var_os("PAPO_RUNTIME_DIR").filter(|p|!p.is_empty()).map(PathBuf::from)
        .or_else(||std::env::current_exe().ok().and_then(|exe|runtime_root(&exe,CURRENT.os)));
    let Some(root)=root.filter(|p|p.is_dir())else{return;};
    let python=root.join(if CURRENT.os==Os::Windows{"bin/python.exe"}else{"bin/python3"});
    if python.is_file(){set_default("PAPO_PYTHON",python.as_os_str());}
    for (name,suffix) in [("GI_TYPELIB_PATH","lib/girepository-1.0"),("GSETTINGS_SCHEMA_DIR","share/glib-2.0/schemas"),("GST_PLUGIN_SYSTEM_PATH_1_0","lib/gstreamer-1.0")]{
        let path=root.join(suffix);if path.is_dir(){set_default(name,path.as_os_str());}
    }
}
fn set_default(key:&str,value:&OsStr){if std::env::var_os(key).is_none_or(|v|v.is_empty()){std::env::set_var(key,value);}}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn packaged_python_keeps_spaces_and_explicit_overrides(){
        let exe=Path::new("/Applications/Papo Preview.app/Contents/MacOS/papo-gtk");
        let mac=python_path(Os::Macos,None,Some(exe),|p|p.ends_with("Resources/runtime/bin/python3"));
        assert_eq!(Path::new(&mac),exe.parent().unwrap().join("../Resources/runtime/bin/python3"));
        let explicit=OsString::from("/custom path/python");
        assert_eq!(python_path(Os::Windows,Some(explicit.clone()),Some(exe),|_|true),explicit);
        assert_eq!(python_path(Os::Windows,None,Some(Path::new("/Papo Preview/papo-gtk.exe")),|_|true),OsString::from("/Papo Preview/runtime/bin/python.exe"));
    }
    #[test]fn development_falls_back_to_the_native_interpreter(){
        for(os,expected)in[(Os::Linux,"/usr/bin/python3"),(Os::Windows,"python.exe"),(Os::Macos,"python3")]{assert_eq!(python_path(os,Some(OsString::new()),None,|_|false),OsString::from(expected));}
    }
}
