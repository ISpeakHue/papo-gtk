//! EN-US: Recognize GIF source links without contacting provider pages or CDNs.
//! PT-BR: Reconhece links de GIF sem acessar páginas ou CDNs dos provedores.
use reqwest::Url;

fn page(value:&str)->Option<Url>{
    let url=Url::parse(value).ok()?;
    if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.port().is_some()||value.len()>2048{return None;}
    let parts:Vec<_>=url.path_segments()?.filter(|part|!part.is_empty()).collect();
    if matches!(url.host_str(),Some("tenor.com"|"www.tenor.com"))&&parts.len()==1&&parts[0].strip_suffix(".gif").is_some_and(|id|!id.is_empty()&&id.bytes().all(|b|b.is_ascii_alphanumeric())){return Some(url);}
    let mut path=parts.iter().copied();
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
pub fn is_video(bytes:&[u8])->bool{bytes.get(4..8)==Some(b"ftyp")||bytes.starts_with(&[0x1a,0x45,0xdf,0xa3])}
pub fn provider_name(value:&str)->bool{
    matches!(value.trim().to_ascii_lowercase().trim_end_matches('/'),"giphy"|"giphy.com"|"www.giphy.com"|"tenor"|"tenor.com"|"www.tenor.com"|"klipy"|"klipy.com"|"www.klipy.com")
}
fn video_url(value:&str)->Option<Url>{
    let url=Url::parse(value).ok()?;
    if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.port().is_some()||value.len()>2048{return None;}
    if !matches!(url.host_str()?,"media.tenor.com"|"media1.tenor.com"|"media2.tenor.com"|"media3.tenor.com"|"media4.tenor.com"|"static.klipy.com"|"static1.klipy.com"|"static2.klipy.com"){return None;}
    let path=url.path().to_ascii_lowercase();if ![".mp4",".webm"].iter().any(|ext|path.ends_with(ext)){return None;}Some(url)
}
fn key(value:&str)->Option<String>{
    if let Some(url)=page(value).or_else(||media_url(value)).or_else(||video_url(value)){return Some(url.to_string());}
    let value=value.trim_matches(|c|matches!(c,'<'|'>'|'('|')'|','|'.'|'"'|'\''));
    super::giphy::id(value).or_else(||page(value).or_else(||media_url(value)).or_else(||video_url(value)).map(|u|u.to_string()))
}
pub fn ranges(text:&str)->Vec<(usize,usize,String)>{
    let mut found=vec![];let mut i=0;
    while i<text.len(){let rest=&text[i..];
        if rest.starts_with('`'){let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];i+=rest[count..].find(delimiter).map_or(rest.len(),|end|count+end+count);continue;}
        if rest.starts_with(char::is_whitespace){i+=rest.chars().next().unwrap().len_utf8();continue;}
        let end=i+rest.find(|c:char|c.is_whitespace()||c=='`').unwrap_or(rest.len());
        let token=&text[i..end];
        if let Some(link)=key(token){
            let direct=page(token).or_else(||media_url(token)).or_else(||video_url(token)).is_some();
            let visible=if direct{token}else{token.trim_matches(|c|matches!(c,'<'|'>'|'('|')'|','|'.'|'"'|'\''))};
            let begin=i+token.find(visible).unwrap_or(0);found.push((begin,begin+visible.len(),link));
        }i=end;
    }found
}
pub fn links(text:&str)->Vec<String>{
    let mut links=vec![];for(_,_,link) in ranges(text){if !links.contains(&link)&&links.len()<4{links.push(link);}}links
}
pub fn content(key:&str)->String{if key.starts_with("https://"){key.into()}else{format!("giphy:{key}")}}
pub fn is_provider(value:&str)->bool{super::giphy::id(value).is_some()||page(value).is_some()||media_url(value).is_some()||video_url(value).is_some()}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn links_preserve_code_and_reject_untrusted_provider_hosts(){
        let tenor="https://tenor.com/view/two-cats-gif-16152841183863087986";let klipy="https://klipy.com/gifs/hop-on-google-docs-1";
        assert_eq!(links(&format!("{tenor} {klipy} giphy:abc {tenor}")),[tenor,klipy,"abc"]);
        assert!(links(&format!("`{tenor}` ```\n{klipy}\n``` ")).is_empty());
        for bad in ["https://tenor.com.evil/view/a","https://user@tenor.com/view/a","https://klipy.com:8080/gifs/a","http://klipy.com/gifs/a"]{assert!(!is_provider(bad));}
        assert!(is_provider("https://tenor.com/nihxadXt69o.gif"));
        assert!(is_video(include_bytes!("../../tests/fixtures/chat-h264.mp4")));assert!(!is_video(b"GIF89a"));
    }
}
