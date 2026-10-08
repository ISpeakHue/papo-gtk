//! Resolve only public Giphy identifiers. The API session is never sent to a CDN.
use futures_util::StreamExt;
pub fn id(value:&str)->Option<String>{
    let value=value.trim_matches(|c|matches!(c,'<'|'>'|'('|')'|','|'.'|'"'|'\''));
    let id=if let Some(id)=value.strip_prefix("giphy:"){id.to_owned()}else{
        let url=reqwest::Url::parse(value).ok()?;if !matches!(url.scheme(),"https"|"http")||!url.username().is_empty()||url.password().is_some(){return None;}
        let host=url.host_str()?;
        if host=="giphy.com"||host=="www.giphy.com"{let mut path=url.path_segments()?;if !matches!(path.next()?,"gifs"|"stickers"|"embed"){return None;}path.next()?.rsplit('-').next()?.to_owned()}
        else if matches!(host,"media.giphy.com"|"media0.giphy.com"|"media1.giphy.com"|"media2.giphy.com"|"media3.giphy.com"|"media4.giphy.com"|"i.giphy.com"){
            let parts=url.path_segments()?.collect::<Vec<_>>();if parts.first()==Some(&"media"){let index=if parts.get(1)?.starts_with("v1."){2}else{1};parts.get(index)?.to_string()}else{let file=parts.first()?;file.strip_suffix(".gif").or_else(||file.strip_suffix(".webp")).or_else(||file.strip_suffix(".mp4"))?.to_owned()}
        }else{return None;}
    };
    if id.is_empty()||id.len()>128||!id.bytes().all(|c|c.is_ascii_alphanumeric()){None}else{Some(id)}
}
pub fn ids(text:&str)->Vec<String>{
    let mut ids=vec![];let mut i=0;
    while i<text.len(){let rest=&text[i..];
        if rest.starts_with('`'){let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];i+=rest[count..].find(delimiter).map_or(rest.len(),|end|count+end+count);continue;}
        if rest.starts_with(char::is_whitespace){i+=rest.chars().next().unwrap().len_utf8();continue;}
        let end=i+rest.find(|c:char|c.is_whitespace()||c=='`').unwrap_or(rest.len());
        if let Some(id)=id(&text[i..end]){if !ids.contains(&id)&&ids.len()<4{ids.push(id);}}
        i=end;
    }ids
}
pub fn url(id:&str)->Option<String>{let checked=self::id(&format!("giphy:{id}"))?;Some(format!("https://media.giphy.com/media/{checked}/200.gif"))}
pub async fn fetch(id:&str)->anyhow::Result<Vec<u8>>{
    let url=url(id).ok_or_else(||anyhow::anyhow!("Identificador Giphy inválido"))?;
    // Separate client: no API cookies, authorization, referrer, or redirects.
    static CLIENT:std::sync::LazyLock<reqwest::Client>=std::sync::LazyLock::new(||reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(15)).build().expect("public media client"));
    let response=CLIENT.get(url).send().await?.error_for_status()?;
    anyhow::ensure!(response.content_length().unwrap_or(0)<=4<<20,"GIF maior que 4 MiB");
    let mut stream=response.bytes_stream();let mut bytes=vec![];while let Some(chunk)=stream.next().await{let chunk=chunk?;anyhow::ensure!(bytes.len()+chunk.len()<=4<<20,"GIF maior que 4 MiB");bytes.extend_from_slice(&chunk);}Ok(bytes)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn giphy_tokens_urls_and_cdn_paths_resolve_without_arbitrary_hosts(){let expected="VxdNf4DadRSsMYAfnv";for value in [format!("giphy:{expected}"),format!("https://giphy.com/gifs/cat-{expected}"),format!("https://media2.giphy.com/media/{expected}/giphy.gif"),format!("https://media.giphy.com/media/v1.Y2lk=test/{expected}/giphy.gif"),format!("https://giphy.com/embed/{expected}")]{assert_eq!(id(&value).as_deref(),Some(expected));}for value in ["giphy:../../secret","giphy:","https://giphy.com.evil/gifs/foo","https://media.giphy.com@evil/media/foo/giphy.gif","javascript:foo"]{assert!(id(value).is_none());}assert_eq!(ids("giphy:abc giphy:abc giphy:def"),["abc","def"]);assert!(ids("```
giphy:abc
``` `giphy:def`").is_empty());}
}
