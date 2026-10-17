use super::*;
use tokio::{net::TcpListener,io::{AsyncReadExt,AsyncWriteExt}};
#[test]fn authenticated_redirects_require_the_same_scheme_host_and_port(){
    let origin=Url::parse("https://example.com/api/").unwrap();
    assert!(ApiClient::redirect_allowed(&origin,&Url::parse("https://example.com/api/messages?page=2").unwrap()));
    for target in ["http://example.com/api/","https://other.example.com/api/","https://example.com:8443/api/"]{
        assert!(!ApiClient::redirect_allowed(&origin,&Url::parse(target).unwrap()));
    }
}
async fn response(headers:String,body:Vec<u8>)->reqwest::Response{
    let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}",listener.local_addr().unwrap());
    tokio::spawn(async move{let(mut stream,_)=listener.accept().await.unwrap();let mut request=[0;4096];let _=stream.read(&mut request).await;let _=stream.write_all(headers.as_bytes()).await;let _=stream.write_all(&body).await;});
    reqwest::Client::new().get(url).send().await.unwrap()
}
#[tokio::test]async fn response_caps_cover_declared_chunked_and_close_delimited_bodies(){
    for (headers,bytes) in [
        ("HTTP/1.1 200 OK\r\nContent-Length: 2048\r\n\r\n".into(),vec![]),
        ("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".into(),[b"401\r\n".as_slice(),&vec![b'x';1025],b"\r\n0\r\n\r\n"].concat()),
        ("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".into(),vec![b'x';1025]),
    ]{assert!(body::read(response(headers,bytes).await,1024).await.is_err());}
    let res=response("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n".into(),b"{}".to_vec()).await;
    assert_eq!(body::read(res,2).await.unwrap(),b"{}");
    let res=response("HTTP/1.1 401 Unauthorized\r\nContent-Length: 999999\r\n\r\n".into(),vec![]).await;
    assert!(is_session_error(&ApiClient::check_response(res,"test").await.unwrap_err()));
}
#[tokio::test]async fn password_redirects_cannot_reach_another_origin(){
    for status in [301,302,307,308]{
        let origin=TcpListener::bind("127.0.0.1:0").await.unwrap();let target=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api=ApiClient::new(&format!("http://{}",origin.local_addr().unwrap())).unwrap();let location=format!("http://{}/password-target",target.local_addr().unwrap());
        let server=tokio::spawn(async move{let(mut socket,_)=origin.accept().await.unwrap();let mut bytes=[0;8192];let _=socket.read(&mut bytes).await;socket.write_all(format!("HTTP/1.1 {status} redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();});
        assert!(api.login("synthetic","synthetic-password").await.is_err());server.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(40),target.accept()).await.is_err(),"redirect must receive no request");
    }
}
