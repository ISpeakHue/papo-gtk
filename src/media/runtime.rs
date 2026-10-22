//! EN-US: Select the packaged media interpreter without changing local development.
//! PT-BR: Seleciona o interpretador de mídia do pacote sem alterar o ambiente de desenvolvimento local.

pub fn python() -> tokio::process::Command {
    crate::platform::runtime::python()
}

/// Offline package acceptance using synthetic media and no physical devices.
pub async fn check()->anyhow::Result<()>{
    anyhow::ensure!(crate::platform::CURRENT.isolated_media_helpers||cfg!(windows),"Isolated media helpers are not implemented on this platform yet.");
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let scratch=Scratch(std::env::temp_dir().join(format!("papo-runtime-{}",uuid::Uuid::new_v4())));
    std::fs::create_dir(&scratch.0)?;
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&scratch.0,std::fs::Permissions::from_mode(0o700))?;}
    let marker=scratch.0.join("private-marker");std::fs::write(&marker,b"must not reach converters")?;
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await?;let port=listener.local_addr()?.port();
    for (name,bytes) in [("clip.mp4",include_bytes!("../../tests/fixtures/chat-h264.mp4").as_slice()),("clip.mov",include_bytes!("../../tests/fixtures/chat-video.mov").as_slice()),("clip.webm",include_bytes!("../../tests/fixtures/chat-video.webm").as_slice())]{
        let path=scratch.0.join(name);std::fs::write(&path,bytes)?;
        let script=format!(r#"import os, socket, sys
assert os.path.isfile(sys.argv[1])
assert not os.path.exists({marker:?})
assert not os.environ.get('DBUS_SESSION_BUS_ADDRESS')
assert not os.environ.get('PULSE_SERVER')
try:
    open(sys.argv[1], 'wb')
except OSError:
    pass
else:
    raise AssertionError('writable decoder input')
try:
    socket.create_connection(('127.0.0.1', {port}), timeout=0.5)
except OSError:
    pass
else:
    raise AssertionError('decoder can reach parent network')
print('isolated')
"#,marker=marker.to_string_lossy());
        let output=tokio::time::timeout(std::time::Duration::from_secs(12),super::sandbox::command(&path,&script)?.output()).await??;
        anyhow::ensure!(output.status.success()&&String::from_utf8_lossy(&output.stdout).trim()=="isolated","Media sandbox isolation failed: {}",String::from_utf8_lossy(&output.stderr));
        let poster=super::poster::extract(&path).await?;anyhow::ensure!(poster.starts_with(b"\x89PNG"),"Invalid video poster");
        let animation=super::animation::PreparedAnimation::video(&path).await?;anyhow::ensure!(!animation.frames.is_empty(),"Empty converted animation");
    }
    anyhow::ensure!(tokio::time::timeout(std::time::Duration::from_millis(50),listener.accept()).await.is_err(),"Sandbox reached parent TCP listener");
    #[cfg(windows)]crate::platform::sandbox::check().await?;
    Ok(())
}
