//! EN-US: Anonymous, bounded release discovery, isolated from authenticated server clients.
//! PT-BR: Consulta pública e limitada de versões, isolada dos clientes autenticados do servidor.
use serde::{Deserialize,Serialize};
use semver::Version;
pub const PAGE:&str="https://github.com/ISpeakHue/papo-gtk/releases";
const FEED:&str="https://api.github.com/repos/ISpeakHue/papo-gtk/releases?per_page=100";
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Channel {#[default] Stable,Experimental}
#[derive(Debug,Clone,Deserialize)]
pub struct Asset {pub name:String,pub browser_download_url:String,#[serde(default)]pub state:String}
#[derive(Debug,Clone,Deserialize)]
pub struct Release {pub tag_name:String,pub html_url:String,#[serde(default)]pub body:Option<String>,#[serde(default)]pub draft:bool,#[serde(default)]pub prerelease:bool,#[serde(default)]pub assets:Vec<Asset>}
impl Release {pub fn version(&self)->Option<Version>{Version::parse(self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name)).ok()}}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Installation {Flatpak,AppImage,WindowsInstaller,WindowsPortable,Source}
impl Installation {
    pub fn current()->Self{
        if cfg!(target_os="linux"){
            if std::env::var_os("FLATPAK_ID").is_some()||std::path::Path::new("/.flatpak-info").is_file(){return Self::Flatpak;}
            if std::env::var_os("APPIMAGE").is_some_and(|s|!s.is_empty()){return Self::AppImage;}
        }
        if cfg!(windows){if let Ok(exe)=std::env::current_exe(){for parent in exe.ancestors().skip(1).take(4){if parent.join("BUILD-INFO.json").is_file(){return if parent.join("install.ini").is_file(){Self::WindowsInstaller}else{Self::WindowsPortable};}}}}
        Self::Source
    }
    pub fn guidance(self)->&'static str{crate::i18n::tr(match self{
        Self::Flatpak=>"Atualize pelo aplicativo de software ou execute flatpak update br.com.papo.gtk. Se instalado de um arquivo sem remoto, baixe o novo .flatpak e instale-o com flatpak install --user CAMINHO. O canal do remoto Flatpak é independente desta consulta.",
        Self::AppImage=>"Baixe a nova AppImage, feche o Papo e substitua o arquivo anterior. Mantenha a permissão de execução.",
        Self::WindowsInstaller=>"Feche o Papo e execute o novo instalador. O suporte ao Windows permanece experimental.",
        Self::WindowsPortable=>"Feche o Papo e extraia o novo pacote portátil em uma pasta separada. O suporte ao Windows permanece experimental.",
        Self::Source=>"Compilação local: consulte as notas e siga as instruções de compilação da versão escolhida."
    })}
}
fn github(url:&str)->Option<reqwest::Url>{let u=reqwest::Url::parse(url).ok()?;(u.scheme()=="https"&&u.host_str()==Some("github.com")&&u.username().is_empty()&&u.password().is_none()&&u.port().is_none()&&u.query().is_none()&&u.fragment().is_none()).then_some(u)}
pub fn valid_page(release:&Release)->bool{let Some(u)=github(&release.html_url)else{return false;};let mut expected=reqwest::Url::parse(&format!("{PAGE}/tag/")).unwrap();expected.path_segments_mut().unwrap().pop_if_empty().push(&release.tag_name);u==expected}
pub fn select(rows:Vec<Release>,channel:Channel)->Option<Release>{
    rows.into_iter().filter(|r|!r.draft&&valid_page(r)&&r.version().is_some_and(|v|channel==Channel::Experimental||(!r.prerelease&&v.pre.is_empty())))
        .max_by(|a,b|a.version().unwrap().cmp_precedence(&b.version().unwrap()))
}
pub fn download(release:&Release,installation:Installation,arch:&str)->Option<Asset>{
    // EN-US: Match complete package names; never recommend a dependency source archive or another CPU.
    // PT-BR: Confere nomes completos; nunca oferece código de dependências nem outra arquitetura.
    let arch=match arch{"x86_64"=>"x86_64","aarch64"=>"aarch64",_=>return None};let version=release.version()?;
    let names:Vec<String>=match installation{
        Installation::AppImage=>vec![format!("Papo-{version}-{arch}.AppImage")],
        Installation::Flatpak=>vec![format!("Papo-{version}-{arch}.flatpak")],
        Installation::WindowsInstaller=>vec![format!("Papo-{version}-windows-{arch}.exe"),format!("Papo-{version}-windows-{arch}.msi")],
        Installation::WindowsPortable=>vec![format!("Papo-{version}-windows-{arch}-dev.zip"),format!("Papo-{version}-windows-{arch}.zip")],
        Installation::Source=>return None,
    };
    names.into_iter().find_map(|name|release.assets.iter().find(|a|a.name==name&&a.state=="uploaded").filter(|a|{
        let Some(u)=github(&a.browser_download_url)else{return false;};let mut expected=reqwest::Url::parse(&format!("{PAGE}/download/")).unwrap();expected.path_segments_mut().unwrap().pop_if_empty().push(&release.tag_name).push(&a.name);u==expected
    }).cloned())
}
pub async fn check(channel:Channel)->anyhow::Result<Option<Release>>{
    // A fresh client has no cookie jar, backend headers, redirects or automatic retries.
    let client=reqwest::Client::builder().user_agent(concat!("Papo/",env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none()).connect_timeout(std::time::Duration::from_secs(8)).timeout(std::time::Duration::from_secs(20)).build()?;
    fetch(&client,FEED,channel).await
}
async fn fetch(client:&reqwest::Client,endpoint:&str,channel:Channel)->anyhow::Result<Option<Release>>{
    let mut response=client.get(endpoint).header("Accept","application/vnd.github+json").send().await?.error_for_status()?;
    let limit=2<<20;anyhow::ensure!(response.content_length().unwrap_or(0)<=limit,"release response exceeds limit");let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await?{anyhow::ensure!(bytes.len()+chunk.len()<=limit as usize,"release response exceeds limit");bytes.extend_from_slice(&chunk);}
    let rows:Vec<Release>=serde_json::from_slice(&bytes)?;anyhow::ensure!(rows.len()<=100,"release count exceeds limit");Ok(select(rows,channel))
}
#[cfg(test)]mod tests{
    use super::*;
    fn release(v:&str,pre:bool)->Release{Release{tag_name:format!("v{v}"),html_url:format!("{PAGE}/tag/v{v}"),prerelease:pre,draft:false,body:None,assets:vec![]}}
    #[test]fn semantic_versions_and_release_channels(){let rows=vec![release("1.9.0",false),release("1.10.0",false),release("2.0.0-beta.2",true),release("3.0.0-beta.1",false)];assert_eq!(select(rows.clone(),Channel::Stable).unwrap().tag_name,"v1.10.0");assert_eq!(select(rows,Channel::Experimental).unwrap().tag_name,"v3.0.0-beta.1");let mut draft=release("9.0.0",false);draft.draft=true;assert!(select(vec![draft],Channel::Experimental).is_none());}
    #[test]fn links_and_packages_must_match_repo_format_and_cpu(){let mut r=release("1.1.1",false);for name in ["Papo-1.1.1-x86_64.AppImage","Papo-1.1.1-aarch64.AppImage","Papo-1.1.1-windows-x86_64.exe","Papo-1.1.1-x86_64.flatpak","gtk-4.24.1-source.tar.gz"]{r.assets.push(Asset{name:name.into(),browser_download_url:format!("{PAGE}/download/v1.1.1/{name}"),state:"uploaded".into()});}assert!(download(&r,Installation::AppImage,"x86_64").unwrap().name.ends_with("x86_64.AppImage"));assert!(download(&r,Installation::WindowsInstaller,"x86_64").unwrap().name.ends_with(".exe"));assert!(download(&r,Installation::Flatpak,"x86_64").unwrap().name.ends_with(".flatpak"));assert!(download(&r,Installation::Source,"x86_64").is_none());assert!(download(&r,Installation::AppImage,"arm").is_none());r.assets[0].browser_download_url="https://evil.test/update.AppImage".into();assert!(download(&r,Installation::AppImage,"x86_64").is_none());r.html_url="https://github.com/another/repo/releases/tag/v1.1.1".into();assert!(select(vec![r],Channel::Stable).is_none());}
    #[tokio::test]async fn anonymous_bounded_feed_and_http_errors(){use tokio::io::{AsyncReadExt,AsyncWriteExt};let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/",listener.local_addr().unwrap());let server=tokio::spawn(async move{let(mut socket,_)=listener.accept().await.unwrap();let mut request=[0u8;4096];let n=socket.read(&mut request).await.unwrap();let headers=String::from_utf8_lossy(&request[..n]).to_ascii_lowercase();assert!(!headers.contains("authorization:"));assert!(!headers.contains("cookie:"));let body=serde_json::to_string(&serde_json::json!([{"tag_name":"v1.1.1","html_url":format!("{PAGE}/tag/v1.1.1")}])).unwrap();socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();});assert_eq!(fetch(&reqwest::Client::new(),&url,Channel::Stable).await.unwrap().unwrap().tag_name,"v1.1.1");server.await.unwrap();}
    #[tokio::test]async fn rate_limits_redirects_malformed_and_oversized_responses_fail_closed(){
        use tokio::io::{AsyncReadExt,AsyncWriteExt};
        for response in [
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 302 Found\r\nLocation: https://evil.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            "HTTP/1.1 200 OK\r\nContent-Length: 2097153\r\nConnection: close\r\n\r\n",
        ] {
            let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}/",listener.local_addr().unwrap());
            let server=tokio::spawn(async move{let(mut socket,_)=listener.accept().await.unwrap();let mut request=[0u8;4096];socket.read(&mut request).await.unwrap();socket.write_all(response.as_bytes()).await.unwrap();});
            let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(2)).build().unwrap();
            assert!(fetch(&client,&url,Channel::Stable).await.is_err());server.await.unwrap();
        }
    }
}
