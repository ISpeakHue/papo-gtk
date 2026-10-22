//! EN-US: Media decoder facade; isolation belongs to the platform adapter.
//! PT-BR: Fachada dos decodificadores; o isolamento pertence ao adaptador da plataforma.
pub(super) use crate::platform::sandbox::{command,prepare,timeout};

#[cfg(all(test,target_os="linux"))]mod tests{
    use super::*;
    use std::path::Path;
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
