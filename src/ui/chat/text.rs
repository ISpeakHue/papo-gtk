//! Native, read-only message text with inline custom emoji paintables.
use super::*;

#[derive(Debug,PartialEq,Eq)]
pub(super) enum Part { Text(String), Emoji(Uuid,String) }

pub(super) fn parts(text:&str,emojis:&[crate::models::Emoji])->Vec<Part>{
    let names:HashMap<_,_>=emojis.iter().map(|e|(e.name.as_str(),e.id)).collect();
    let mut parts=vec![];let mut start=0;let mut i=0;
    while i<text.len(){
        let rest=&text[i..];
        if rest.starts_with("http://")||rest.starts_with("https://"){
            i+=rest.find(char::is_whitespace).unwrap_or(rest.len());continue;
        }
        if rest.starts_with('`'){
            let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];
            i+=rest[count..].find(delimiter).map_or(rest.len(),|end|count+end+count);continue;
        }
        if rest.starts_with(':'){
            if let Some(end)=rest[1..].find(':'){
                let name=&rest[1..end+1];
                if let Some(id)=names.get(name){
                    if start<i{parts.push(Part::Text(text[start..i].into()));}
                    parts.push(Part::Emoji(*id,name.into()));i+=end+2;start=i;continue;
                }
            }
        }
        i+=rest.chars().next().unwrap().len_utf8();
    }
    if start<text.len(){parts.push(Part::Text(text[start..].into()));}parts
}

pub(super) fn widget(text:&str,users:&HashMap<Uuid,UserSummary>,actions:&Actions)->gtk::TextView{
    let view=gtk::TextView::new();view.set_editable(false);view.set_cursor_visible(false);view.set_accepts_tab(false);
    view.set_wrap_mode(gtk::WrapMode::WordChar);view.set_vexpand(false);view.add_css_class("papo-message-text");
    let buffer=view.buffer();let mut iter=buffer.start_iter();
    for part in parts(text,&actions.emojis){match part{
        Part::Text(text)=>buffer.insert(&mut iter,&mentions::render(&text,users)),
        Part::Emoji(id,name)=>{if let Some(texture)=actions.textures.get(&id){buffer.insert_paintable(&mut iter,texture);}else{buffer.insert(&mut iter,&format!(":{name}:"));}},
    }}
    // Keep a textual description available even when a custom image is displayed.
    view.set_tooltip_text(Some(&mentions::render(text,users)));view
}

pub(super) fn excerpt(text:&str)->String{
    let mut text=text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count()>160{text=text.chars().take(160).collect();text.push('…');}text
}

#[cfg(test)]mod tests{
    use super::*;
    fn emoji(name:&str)->crate::models::Emoji{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":name,"format":"PNG","created_at":"2026-10-07T12:00:00Z"})).unwrap()}
    #[test]fn known_inline_emojis_preserve_unicode_unknown_names_code_and_urls(){
        let e=emoji("gambiarra");let unicode=emoji("🐈");let list=vec![e.clone(),unicode.clone()];
        assert_eq!(parts("Olá :gambiarra: :🐈: :unknown:",&list),vec![Part::Text("Olá ".into()),Part::Emoji(e.id,e.name),Part::Text(" ".into()),Part::Emoji(unicode.id,unicode.name),Part::Text(" :unknown:".into())]);
        for literal in ["`:gambiarra:`","```\n:gambiarra:\n```","https://example.test/:gambiarra:","<b>literal</b>","incomplete :gambiarra"]{assert_eq!(parts(literal,&list),vec![Part::Text(literal.into())]);}
        assert_eq!(parts(":gambiarra:",&[]),vec![Part::Text(":gambiarra:".into())]);
    }
    #[test]fn reply_excerpts_are_single_line_and_bounded_by_unicode_characters(){
        assert_eq!(excerpt("  first\nsecond\tthird  "),"first second third");let long="🐈".repeat(200);assert_eq!(excerpt(&long).chars().count(),161);
    }
}
