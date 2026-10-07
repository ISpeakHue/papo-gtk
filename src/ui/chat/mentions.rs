//! Plain-text mention rendering keeps untrusted member names out of GTK markup.
use crate::models::UserSummary;
use std::collections::HashMap;
use uuid::Uuid;
pub fn render(text:&str,users:&HashMap<Uuid,UserSummary>)->String {
    let lower=text.to_ascii_lowercase();let mut out=String::new();let mut offset=0;
    while let Some(relative)=lower[offset..].find("@mention(<@") {
        let start=offset+relative;out.push_str(&text[offset..start]);
        if let Some(end)=lower[start+11..].find(">)") {
            let end=start+11+end;
            if let Ok(id)=Uuid::parse_str(&text[start+11..end]) {
                out.push('@');out.push_str(users.get(&id).map(|u|u.display_name()).unwrap_or("Usuário removido"));offset=end+2;continue;
            }
        }
        out.push_str(&text[start..start+1]);offset=start+1;
    }out.push_str(&text[offset..]);out
}
/// GTK positions count Unicode characters, whereas string offsets count bytes.
pub fn completion(text:&str,position:i32)->Option<(usize,usize,&str)> {
    let end=if position<0{text.len()}else{text.char_indices().nth(position as usize).map(|(i,_)|i).unwrap_or(text.len())};
    let start=text[..end].rfind('@')?;
    if start>0&&!text[..start].chars().last()?.is_whitespace(){return None;}
    let fragment=&text[start+1..end];if fragment.chars().any(|c|c.is_whitespace()||matches!(c,'('|'<'|'>'|')')){return None;}
    Some((start,end,fragment))
}
pub fn insert(text:&str,position:i32,id:Option<Uuid>)->Option<(String,i32)> {
    let(start,end,_)=completion(text,position)?;let token=id.map(|id|format!("@mention(<@{id}>) ")).unwrap_or_else(||"@everyone ".into());
    let caret=(text[..start].chars().count()+token.chars().count()) as i32;
    Some((format!("{}{}{}",&text[..start],token,&text[end..]),caret))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn tokens_render_without_markup_and_unknown_users_are_safe(){let id=Uuid::new_v4();assert_eq!(render(&format!("olá @MENTION(<@{id}>) <b>!"),&HashMap::new()),"olá @Usuário removido <b>!");assert_eq!(render("@mention(<@invalid>)",&HashMap::new()),"@mention(<@invalid>)");}
    #[test] fn completion_preserves_unicode_and_text_after_caret(){let id=Uuid::new_v4();let(text,caret)=insert("é @jo depois",5,Some(id)).unwrap();assert_eq!(text,format!("é @mention(<@{id}>)  depois"));assert_eq!(caret,text[..text.find(" depois").unwrap()].chars().count() as i32);assert!(completion("email@host",10).is_none());assert!(completion(&text,caret).is_none());}
}
