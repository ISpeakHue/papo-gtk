//! File-media converters get no network, home, desktop sockets or writable source.
use std::path::Path;
pub(super) fn command(path:&Path,script:&str)->anyhow::Result<tokio::process::Command>{
    let source=path.canonicalize()?;
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
    cmd.arg("--").arg(python).args(["-s","-u","-c",script,"/input/media"]).kill_on_drop(true);Ok(cmd)
}

#[cfg(test)]mod tests{
    use super::*;
    #[tokio::test]async fn helpers_have_no_host_network_or_private_files_and_keep_input_read_only(){
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let port=listener.local_addr().unwrap().port();
        let script=format!(r#"import socket, os
assert not os.path.exists('/etc/passwd')
assert not os.path.exists('/home')
try:
    open('/input/media', 'wb')
except OSError:
    pass
else:
    raise AssertionError('source was writable')
try:
    socket.create_connection(('127.0.0.1', {port}), timeout=0.5)
except OSError:
    pass
else:
    raise AssertionError('host network was reachable')
print('isolated')
"#);
        let result=command(Path::new("tests/fixtures/chat-h264.mp4"),&script).unwrap().output().await.unwrap();
        assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));assert_eq!(result.stdout,b"isolated\n");
        assert!(tokio::time::timeout(std::time::Duration::from_millis(40),listener.accept()).await.is_err());
        let image=crate::media::poster::extract(Path::new("tests/fixtures/chat-h264.mp4")).await.unwrap();assert!(image.starts_with(b"\x89PNG"));
        assert!(!crate::media::animation::PreparedAnimation::video(Path::new("tests/fixtures/chat-video.webm")).await.unwrap().frames.is_empty());
    }
}
