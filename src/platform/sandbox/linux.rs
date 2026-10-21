//! EN-US: File-media converters get no network, home, desktop sockets or writable source.
//! PT-BR: Conversores de arquivos de mídia não recebem acesso à rede, pasta pessoal, sockets do desktop ou escrita no arquivo de origem.
use std::path::Path;
pub(crate) struct Command{process:tokio::process::Command,input:Option<std::path::PathBuf>}
impl Command{pub async fn output(&mut self)->std::io::Result<std::process::Output>{self.process.output().await}}
impl Drop for Command{fn drop(&mut self){if let Some(path)=&self.input{let _=std::fs::remove_file(path);}}}
pub(crate) fn command(path:&Path,script:&str)->anyhow::Result<Command>{
    let source=path.canonicalize()?;
    if Path::new("/.flatpak-info").is_file(){
        // EN-US: Flatpak denies nested user namespaces. Its portal starts a more restricted app sandbox,
        // not a host command; copy only this input to the portal exposure directory because /tmp is instance-private.
        // PT-BR: O Flatpak bloqueia namespaces de usuário aninhados. Seu portal inicia um sandbox mais restrito
        // do aplicativo, sem executar comandos no host; copia apenas esta entrada, pois /tmp é privado de cada instância.
        let home=dirs::home_dir().ok_or_else(||anyhow::anyhow!("Diretório do aplicativo indisponível"))?;
        let directory=home.join(".var/app").join(crate::app::APP_ID).join("sandbox");std::fs::create_dir_all(&directory)?;
        let name=format!("papo-media-{}",uuid::Uuid::new_v4());let input=directory.join(&name);
        let mut wrapper=Command{process:tokio::process::Command::new("/usr/bin/flatpak-spawn"),input:Some(input.clone())};
        #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&directory,std::fs::Permissions::from_mode(0o700))?;}
        std::fs::copy(&source,&input)?;
        let cmd=&mut wrapper.process;
        cmd.args(["--sandbox","--no-network","--watch-bus","--clear-env","--directory=/","--unset-env=DBUS_SESSION_BUS_ADDRESS","--unset-env=PULSE_SERVER"])
            .arg(format!("--sandbox-expose-ro={name}"))
            .args(["--env=PATH=/app/bin:/usr/bin","--env=LANG=C.UTF-8","--env=GST_DEBUG=0","--env=XDG_CACHE_HOME=/tmp/cache","--env=GST_REGISTRY_1_0=/tmp/registry.bin"]);
        for key in ["GI_TYPELIB_PATH","GST_PLUGIN_SYSTEM_PATH","GST_PLUGIN_SYSTEM_PATH_1_0","LD_LIBRARY_PATH"]{
            if let Some(value)=std::env::var_os(key){let mut arg=std::ffi::OsString::from(format!("--env={key}="));arg.push(value);cmd.arg(arg);}
        }
        cmd.args(["/usr/bin/python3","-s","-u","-c",script]).arg(input).kill_on_drop(true);return Ok(wrapper);
    }
    let app=std::env::var_os("PAPO_APPDIR").map(std::path::PathBuf::from);
    let binary=app.as_ref().map_or_else(||"/usr/bin/bwrap".into(),|p|p.join("usr/libexec/bwrap.real"));
    anyhow::ensure!(binary.is_file(),"Instale bubblewrap para gerar prévias de vídeo com segurança.");
    let mut cmd=tokio::process::Command::new(binary);
    cmd.env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").env_remove("LD_AUDIT");
    cmd.args(["--unshare-all","--die-with-parent","--new-session","--clearenv","--ro-bind","/usr","/usr","--symlink","usr/lib","/lib","--symlink","usr/lib64","/lib64","--symlink","usr/bin","/bin","--proc","/proc","--dev","/dev","--tmpfs","/tmp","--dir","/input","--ro-bind"])
        .arg(source).arg("/input/media").args(["--ro-bind-try","/etc/ld.so.cache","/etc/ld.so.cache","--setenv","PATH","/usr/bin:/bin","--setenv","LANG","C.UTF-8","--setenv","GST_DEBUG","0","--setenv","XDG_CACHE_HOME","/tmp/cache","--setenv","GST_REGISTRY_1_0","/tmp/registry.bin"]);
    if let Some(app)=app{
        cmd.arg("--ro-bind").arg(&app).arg(&app);
        for key in ["LD_LIBRARY_PATH","PYTHONHOME","PYTHONPATH","GI_TYPELIB_PATH","GST_PLUGIN_SYSTEM_PATH_1_0","GST_PLUGIN_PATH_1_0","GST_PLUGIN_SCANNER_1_0","GIO_MODULE_DIR"]{
            if let Some(value)=std::env::var_os(key){cmd.arg("--setenv").arg(key).arg(value);}
        }
    }
    let python=std::env::var_os("PAPO_PYTHON").filter(|v|!v.is_empty()).unwrap_or_else(||"/usr/bin/python3".into());
    cmd.arg("--").arg(python).args(["-s","-u","-c",script,"/input/media"]).kill_on_drop(true);Ok(Command{process:cmd,input:None})
}

