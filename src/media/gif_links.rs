//! Public GIF links share Giphy's bounded, session-free animation pipeline.
use futures_util::StreamExt;
use reqwest::Url;

fn page(value:&str)->Option<Url>{
    let url=Url::parse(value).ok()?;
    if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.port().is_some()||value.len()>2048{return None;}
    let mut path=url.path_segments()?;
    let section=path.next()?;let slug=path.next()?;
    if slug.is_empty()||!slug.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')||path.any(|s|!s.is_empty()){return None;}
    match url.host_str()?{
        "tenor.com"|"www.tenor.com" if section=="view"=>Some(url),
        "klipy.com"|"www.klipy.com" if section=="gifs"=>Some(url),
        _=>None,
    }
}
pub fn media_url(value:&str)->Option<Url>{
    let url=Url::parse(value).ok()?;
    if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.port().is_some()||value.len()>2048{return None;}
    if !matches!(url.host_str()?,"media.tenor.com"|"media1.tenor.com"|"media2.tenor.com"|"media3.tenor.com"|"media4.tenor.com"|"static.klipy.com"|"static1.klipy.com"|"static2.klipy.com"){return None;}
    if !url.path().to_ascii_lowercase().ends_with(".gif"){return None;}Some(url)
}
fn key(value:&str)->Option<String>{
    if let Some(url)=page(value).or_else(||media_url(value)){return Some(url.to_string());}
    let value=value.trim_matches(|c|matches!(c,'<'|'>'|'('|')'|','|'.'|'"'|'\''));
    super::giphy::id(value).or_else(||page(value).or_else(||media_url(value)).map(|u|u.to_string()))
}
pub fn links(text:&str)->Vec<String>{
    let mut links=vec![];let mut i=0;
    while i<text.len(){let rest=&text[i..];
        if rest.starts_with('`'){let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];i+=rest[count..].find(delimiter).map_or(rest.len(),|end|count+end+count);continue;}
        if rest.starts_with(char::is_whitespace){i+=rest.chars().next().unwrap().len_utf8();continue;}
        let end=i+rest.find(|c:char|c.is_whitespace()||c=='`').unwrap_or(rest.len());
        if let Some(link)=key(&text[i..end]){if !links.contains(&link)&&links.len()<4{links.push(link);}}i=end;
    }links
}
pub fn content(key:&str)->String{if key.starts_with("https://"){key.into()}else{format!("giphy:{key}")}}
pub fn is_provider(value:&str)->bool{super::giphy::id(value).is_some()||page(value).is_some()||media_url(value).is_some()}

fn unescape(value:&str)->String{
    value.replace("&amp;","&").replace("&quot;","\"").replace("&#39;","'").replace("&#x2F;","/").replace("&#47;","/")
}
// Read attributes rather than assuming their order or quote style. No remote
// script is executed, and only exact provider CDN GIF URLs can be returned.
pub fn from_html(html:&str)->Option<Url>{
    let lower=html.to_ascii_lowercase();let mut offset=0;
    while let Some(start)=lower[offset..].find("<meta"){
        let start=offset+start+5;let bytes=html.as_bytes();let mut i=start;let mut attrs=std::collections::HashMap::new();
        while i<bytes.len(){
            while i<bytes.len()&&(bytes[i].is_ascii_whitespace()||bytes[i]==b'/'){i+=1;}
            if i>=bytes.len()||bytes[i]==b'>'{break;}
            let begin=i;while i<bytes.len()&&!bytes[i].is_ascii_whitespace()&&!matches!(bytes[i],b'='|b'>'){i+=1;}
            let name=html[begin..i].to_ascii_lowercase();while i<bytes.len()&&bytes[i].is_ascii_whitespace(){i+=1;}
            if i>=bytes.len()||bytes[i]!=b'='{continue;}i+=1;while i<bytes.len()&&bytes[i].is_ascii_whitespace(){i+=1;}
            let quote=bytes.get(i).copied().filter(|b|matches!(b,b'\''|b'"'));if quote.is_some(){i+=1;}let begin=i;
            while i<bytes.len()&&quote.map_or(!bytes[i].is_ascii_whitespace()&&bytes[i]!=b'>',|q|bytes[i]!=q){i+=1;}
            attrs.insert(name,unescape(&html[begin..i]));if quote.is_some()&&i<bytes.len(){i+=1;}
        }
        offset=(i+1).min(html.len());
        if attrs.get("property").or_else(||attrs.get("name")).is_some_and(|n|matches!(n.to_ascii_lowercase().as_str(),"og:image"|"og:image:url"|"twitter:image")){
            if let Some(url)=attrs.get("content").and_then(|v|media_url(v)){return Some(url);}
        }
        if offset>=html.len(){break;}
    }None
}
pub fn from_page(html:&str,link:&str)->Option<Url>{
    let url=page(link)?;
    if matches!(url.host_str(),Some("tenor.com"|"www.tenor.com")){
        let id=url.path_segments()?.next_back()?.rsplit('-').next()?;
        if id.bytes().all(|b|b.is_ascii_digit()){
            if let Some(start)=html.find("id=\"store-cache\"").and_then(|i|html[i..].find('>').map(|n|i+n+1)){
                if let Some(end)=html[start..].find("</script>"){
                    if let Ok(json)=serde_json::from_str::<serde_json::Value>(&html[start..start+end]){
                        let formats=&json["gifs"]["byId"][id]["results"][0]["media_formats"];
                        for format in ["tinygif","gif"]{
                            if formats[format]["size"].as_u64().is_some_and(|n|n>4<<20){continue;}
                            if let Some(url)=formats[format]["url"].as_str().and_then(media_url){return Some(url);}
                        }
                    }
                }
            }
        }
    }
    from_html(html)
}
async fn bytes(url:Url,limit:usize)->anyhow::Result<Vec<u8>>{
    static CLIENT:std::sync::LazyLock<reqwest::Client>=std::sync::LazyLock::new(||reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(15)).user_agent("Papo-GTK/0.1").build().expect("public GIF client"));
    let response=CLIENT.get(url).send().await?.error_for_status()?;
    anyhow::ensure!(!response.status().is_redirection(),"O provedor redirecionou o GIF");
    anyhow::ensure!(response.content_length().unwrap_or(0)<=limit as u64,"Conteúdo do GIF maior que o limite");
    let mut data=vec![];let mut stream=response.bytes_stream();while let Some(chunk)=stream.next().await{let chunk=chunk?;anyhow::ensure!(data.len()+chunk.len()<=limit,"Conteúdo do GIF maior que o limite");data.extend_from_slice(&chunk);}Ok(data)
}
pub async fn fetch(link:&str,hint:Option<&str>)->anyhow::Result<Vec<u8>>{
    if !link.starts_with("https://"){return super::giphy::fetch(link).await;}
    if let Some(url)=hint.and_then(media_url).or_else(||media_url(link)){
        match bytes(url,4<<20).await{Ok(data)=>return Ok(data),Err(e) if page(link).is_none()=>return Err(e),Err(_)=>{}}
    }
    let url=page(link).ok_or_else(||anyhow::anyhow!("Link de GIF inválido"))?;
    let html=bytes(url,1<<20).await?;
    let url=from_page(&String::from_utf8_lossy(&html),link).ok_or_else(||anyhow::anyhow!("O provedor não disponibilizou um GIF animado"))?;
    bytes(url,4<<20).await
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn supplied_tenor_sample(){
        if let (Ok(html),Ok(gif))=(std::env::var("PAPO_GIF_HTML"),std::env::var("PAPO_GIF_SAMPLE")){
            assert!(from_page(&std::fs::read_to_string(html).unwrap(),"https://tenor.com/view/two-cats-gif-16152841183863087986").is_some());
            let animation=super::super::animation::PreparedAnimation::decode(&std::fs::read(gif).unwrap()).expect("the supplied Tenor GIF must decode");assert!(animation.frames.len()>1);eprintln!("Tenor sample: frames={}, decoded_bytes={}",animation.frames.len(),animation.bytes());
        }
    }
    #[test]fn provider_links_and_code_fences(){
        let tenor="https://tenor.com/view/two-cats-gif-16152841183863087986";let klipy="https://klipy.com/gifs/hop-on-google-docs-1";
        assert_eq!(links(&format!("{tenor} {klipy} giphy:abc {tenor}")),[tenor,klipy,"abc"]);
        assert!(links(&format!("`{tenor}` ```\n{klipy}\n``` ")).is_empty());
        for bad in ["https://tenor.com.evil/view/a","https://user@tenor.com/view/a","https://klipy.com:8080/gifs/a","http://klipy.com/gifs/a","https://tenor.com/other/a"]{assert!(links(bad).is_empty());}
    }
    #[test]fn tenor_uses_a_supplied_small_rendition_for_the_exact_post(){
        let json=serde_json::json!({"gifs":{"byId":{"123":{"results":[{"media_formats":{"gif":{"url":"https://media.tenor.com/full.gif","size":13<<20},"tinygif":{"url":"https://media.tenor.com/tiny.gif","size":1000}}}]}}}});
        let html=format!("<script id=\"store-cache\" type=\"text/x-cache\">{json}</script>");
        assert_eq!(from_page(&html,"https://tenor.com/view/cat-123").unwrap().as_str(),"https://media.tenor.com/tiny.gif");
        assert!(from_page(&html,"https://tenor.com/view/other-456").is_none());
    }
    #[test]fn metadata_must_point_to_a_public_provider_gif(){
        let gif="https://media1.tenor.com/m/4Cprea11o3IAAAAC/two-cats.gif";
        assert_eq!(from_html(&format!("<meta content='{gif}?a=1&amp;b=2' PROPERTY='og:image'>")).unwrap().as_str(),format!("{gif}?a=1&b=2"));
        assert!(from_html("<meta property='og:image' content='http://127.0.0.1/a.gif'>").is_none());
        assert!(media_url("https://static.klipy.com.evil/a.gif").is_none());
        assert!(media_url("https://static.klipy.com/a.gif?delivery=original").is_some());
        assert_eq!(links("https://static.klipy.com/a.gif?delivery=original.")[0],"https://static.klipy.com/a.gif?delivery=original.");
        assert!(from_html("<meta foo <meta").is_none());
    }
}
