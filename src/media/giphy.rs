//! Recognize Giphy identifiers; media resolution belongs to the backend.
//! EN: Parses and validates Giphy identifiers from raw tokens (`giphy:<id>`), web URLs, and CDN paths, enforcing safety checks.
//! PT-BR: Reconhece identificadores do Giphy; análise e validação a partir de tokens (`giphy:<id>`), URLs da web e caminhos de CDN, aplicando verificações de segurança.

/// EN: Extracts a sanitized Giphy ID from a string, URL, or token.
/// Validates alphanumeric characters (max 128 chars), rejecting malicious hosts or injection attempts.
/// PT-BR: Extrai um identificador seguro do Giphy a partir de uma string, URL ou token.
/// Valida caracteres alfanuméricos (máx 128 caracteres), rejeitando hosts maliciosos ou tentativas de injeção.
pub fn id(value:&str)->Option<String>{
    // EN: Strip wrapping markdown or punctuation delimiters.
    // PT-BR: Remove delimitadores de pontuação ou markdown ao redor do texto.
    let value=value.trim_matches(|c|matches!(c,'<'|'>'|'('|')'|','|'.'|'"'|'\''));
    let id=if let Some(id)=value.strip_prefix("giphy:"){id.to_owned()}else{
        // EN: Ensure valid HTTP/HTTPS URL and reject embedded credentials.
        // PT-BR: Garante URL HTTP/HTTPS válida e rejeita credenciais embutidas (username/password).
        let url=reqwest::Url::parse(value).ok()?;if !matches!(url.scheme(),"https"|"http")||!url.username().is_empty()||url.password().is_some(){return None;}
        let host=url.host_str()?;
        if host=="giphy.com"||host=="www.giphy.com"{let mut path=url.path_segments()?;if !matches!(path.next()?,"gifs"|"stickers"|"embed"){return None;}path.next()?.rsplit('-').next()?.to_owned()}
        else if matches!(host,"media.giphy.com"|"media0.giphy.com"|"media1.giphy.com"|"media2.giphy.com"|"media3.giphy.com"|"media4.giphy.com"|"i.giphy.com"){
            let parts=url.path_segments()?.collect::<Vec<_>>();if parts.first()==Some(&"media"){let index=if parts.get(1)?.starts_with("v1."){2}else{1};parts.get(index)?.to_string()}else{let file=parts.first()?;file.strip_suffix(".gif").or_else(||file.strip_suffix(".webp")).or_else(||file.strip_suffix(".mp4"))?.to_owned()}
        }else{return None;}
    };
    // EN: Guard against oversized or non-alphanumeric IDs.
    // PT-BR: Protege contra identificadores excessivamente longos ou com caracteres não alfanuméricos.
    if id.is_empty()||id.len()>128||!id.bytes().all(|c|c.is_ascii_alphanumeric()){None}else{Some(id)}
}

#[cfg(test)]
fn ids(text:&str)->Vec<String>{
    let mut ids=vec![];let mut i=0;
    while i<text.len(){let rest=&text[i..];
        if rest.starts_with('`'){let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];i+=rest[count..].find(delimiter).map_or(rest.len(),|end|count+end+count);continue;}
        if rest.starts_with(char::is_whitespace){i+=rest.chars().next().unwrap().len_utf8();continue;}
        let end=i+rest.find(|c:char|c.is_whitespace()||c=='`').unwrap_or(rest.len());
        if let Some(id)=id(&text[i..end]){if !ids.contains(&id)&&ids.len()<4{ids.push(id);}}
        i=end;
    }ids
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn giphy_tokens_urls_and_cdn_paths_resolve_without_arbitrary_hosts(){let expected="VxdNf4DadRSsMYAfnv";for value in [format!("giphy:{expected}"),format!("https://giphy.com/gifs/cat-{expected}"),format!("https://media2.giphy.com/media/{expected}/giphy.gif"),format!("https://media.giphy.com/media/v1.Y2lk=test/{expected}/giphy.gif"),format!("https://giphy.com/embed/{expected}")]{assert_eq!(id(&value).as_deref(),Some(expected));}for value in ["giphy:../../secret","giphy:","https://giphy.com.evil/gifs/foo","https://media.giphy.com@evil/media/foo/giphy.gif","javascript:foo"]{assert!(id(value).is_none());}assert_eq!(ids("giphy:abc giphy:abc giphy:def"),["abc","def"]);assert!(ids("```
giphy:abc
``` `giphy:def`").is_empty());}
}
