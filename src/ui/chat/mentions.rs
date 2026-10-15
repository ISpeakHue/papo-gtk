//! Plain-text mention rendering keeps untrusted member names out of GTK markup.
use crate::models::UserSummary;
use std::collections::HashMap;
use uuid::Uuid;
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct Span { pub start:usize, pub end:usize, pub id:Option<Uuid> }
#[derive(Clone,Debug,Default)]
pub(super) struct DisplayText { pub text:String, pub spans:Vec<Span> }
impl DisplayText {
    /// Read-only text can link a plain nickname from another client without
    /// turning a typed draft into an outgoing mention or guessing duplicate names.
    pub fn read(text:&str,users:&HashMap<Uuid,UserSummary>)->Self {
        let mut value=Self::parse(text,users);if users.is_empty()||!value.text.contains('@'){return value;}let mut i=0;
        while i<value.text.len(){
            let rest=&value.text[i..];
            if let Some(span)=value.spans.iter().find(|s|s.start<=i&&i<s.end){i=span.end;continue;}
            if rest.starts_with("https://")||rest.starts_with("http://"){i+=rest.find(char::is_whitespace).unwrap_or(rest.len());continue;}
            if rest.starts_with('`'){let n=rest.bytes().take_while(|c|*c==b'`').count();i+=rest[n..].find(&rest[..n]).map_or(rest.len(),|end|n+end+n);continue;}
            let boundary=i==0||value.text[..i].chars().last().is_some_and(|c|!c.is_alphanumeric()&&!matches!(c,'_'|'@'|'.'|'/'));
            if rest.starts_with('@')&&boundary&&!rest.starts_with("@mention(<@"){
                let mut candidates=vec![];
                for user in users.values(){
                    for name in [user.username.as_str(),user.display_name()]{
                        if name.is_empty(){continue;}let end=i+1+name.len();
                        let matches=value.text.get(i+1..end).is_some_and(|s|s.eq_ignore_ascii_case(name))
                            &&value.text[end.min(value.text.len())..].chars().next().is_none_or(|c|!c.is_alphanumeric()&&c!='_')
                            &&!value.spans.iter().any(|s|i<s.end&&end>s.start);
                        if matches{candidates.push((end,user.id));}
                    }
                }
                if let Some(end)=candidates.iter().map(|(end,_)|*end).max(){
                    let ids:std::collections::HashSet<_>=candidates.into_iter().filter(|(e,_)|*e==end).map(|(_,id)|id).collect();
                    if ids.len()==1{value.spans.push(Span{start:i,end,id:ids.into_iter().next()});i=end;continue;}
                }
            }
            i+=rest.chars().next().unwrap().len_utf8();
        }
        value.spans.sort_by_key(|s|s.start);value
    }
    pub fn parse(text:&str,users:&HashMap<Uuid,UserSummary>)->Self {
        let lower=text.to_ascii_lowercase();let mut value=Self::default();let mut offset=0;
        while let Some(relative)=lower[offset..].find("@mention(<@") {
            let start=offset+relative;value.text.push_str(&text[offset..start]);
            if let Some(end)=lower[start+11..].find(">)") {
                let end=start+11+end;
                if let Ok(id)=Uuid::parse_str(&text[start+11..end]) {
                    let start=value.text.len();value.text.push('@');value.text.push_str(users.get(&id).map(|u|u.display_name()).unwrap_or("Usuário removido"));
                    value.spans.push(Span{start,end:value.text.len(),id:Some(id)});offset=end+2;continue;
                }
            }
            value.text.push('@');offset=start+1;
        }
        value.text.push_str(&text[offset..]);
        for(start,_) in value.text.match_indices("@everyone") {
            let end=start+9;
            let boundary=(start==0||!value.text[..start].chars().last().unwrap().is_alphanumeric())
                && value.text[end..].chars().next().is_none_or(|c|!c.is_alphanumeric()&&c!='_');
            if boundary&&!value.spans.iter().any(|s|start<s.end&&end>s.start){value.spans.push(Span{start,end,id:None});}
        }
        value.spans.sort_by_key(|s|s.start);value
    }
    pub fn wire_text(&self)->String {
        let mut text=String::new();let mut offset=0;
        for span in &self.spans {
            text.push_str(&self.text[offset..span.start]);
            text.push_str(&span.id.map(|id|format!("@mention(<@{id}>)")).unwrap_or_else(||"@everyone".into()));offset=span.end;
        }
        text.push_str(&self.text[offset..]);text
    }
    /// Exact editable offsets keep identically named members distinct.
    pub fn replace(&mut self,start:usize,end:usize,text:&str) {
        self.spans.retain_mut(|span|{
            if span.end<=start {return true;}
            if span.start>=end {span.start=span.start-end+start+text.len();span.end=span.end-end+start+text.len();return true;}
            // Editing any part of a mention turns that label into ordinary text.
            false
        });
        self.text.replace_range(start..end,text);
    }
}
pub fn render(text:&str,users:&HashMap<Uuid,UserSummary>)->String {
    DisplayText::parse(text,users).text
}
/// Highlight received mentions using the same IDs/names as the displayed text.
/// Literal code and URLs cannot address the current account.
pub fn addressed_to(text:&str,user:Uuid,users:&HashMap<Uuid,UserSummary>)->bool {
    if !text.contains('@'){return false;}
    let addressed=|text:&str|{
        if !text.contains('@'){return false;}
        let displayed=DisplayText::read(text,users);
        displayed.spans.iter().any(|span|span.id==Some(user)||span.id.is_none()&&{
            let before=displayed.text[..span.start].chars().last();
            let after=displayed.text[span.end..].chars().next();
            before.is_none_or(|c|!c.is_alphanumeric()&&!matches!(c,'_'|'@'|'.'|'/'))
                &&after.is_none_or(|c|!c.is_alphanumeric()&&!matches!(c,'_'|'.'))
        })
    };
    let mut start=0;let mut i=0;
    while i<text.len(){
        let rest=&text[i..];
        let url=rest.get(..7).is_some_and(|s|s.eq_ignore_ascii_case("http://"))
            ||rest.get(..8).is_some_and(|s|s.eq_ignore_ascii_case("https://"));
        if url||rest.starts_with('`'){
            if addressed(&text[start..i]){return true;}
            if url{i+=rest.find(char::is_whitespace).unwrap_or(rest.len());}
            else{let n=rest.bytes().take_while(|c|*c==b'`').count();i+=rest[n..].find(&rest[..n]).map_or(rest.len(),|end|n+end+n);}
            start=i;continue;
        }
        i+=rest.chars().next().unwrap().len_utf8();
    }
    addressed(&text[start..])
}
/// GTK positions count Unicode characters, whereas string offsets count bytes.
pub fn completion(text:&str,position:i32)->Option<(usize,usize,&str)> {
    let end=if position<0{text.len()}else{text.char_indices().nth(position as usize).map(|(i,_)|i).unwrap_or(text.len())};
    let start=text[..end].rfind('@')?;
    if start>0&&!text[..start].chars().last()?.is_whitespace(){return None;}
    let fragment=&text[start+1..end];if fragment.chars().any(|c|c.is_whitespace()||matches!(c,'('|'<'|'>'|')')){return None;}
    Some((start,end,fragment))
}
#[cfg(test)]pub fn insert(text:&str,position:i32,id:Option<Uuid>)->Option<(String,i32)> {
    let(start,end,_)=completion(text,position)?;let token=id.map(|id|format!("@mention(<@{id}>) ")).unwrap_or_else(||"@everyone ".into());
    let caret=(text[..start].chars().count()+token.chars().count()) as i32;
    Some((format!("{}{}{}",&text[..start],token,&text[end..]),caret))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test]fn addressed_messages_match_the_recipient_and_ignore_literal_or_ambiguous_names(){
        let user=Uuid::new_v4();let other=Uuid::new_v4();
        let member=|id,username,nickname|serde_json::from_value(serde_json::json!({"id":id,"username":username,"nickname":nickname,"created_at":"2026-10-13T00:00:00Z"})).unwrap();
        let users=HashMap::from([(user,member(user,"ISpeakHue","Íris")),(other,member(other,"bob","Bob"))]);
        let token=format!("@mention(<@{user}>)");
        for text in [token.as_str(),"Olá @ISpeakHue!","Oi @Íris, tudo bem?","@everyone!","é `código` @ISpeakHue"]{assert!(addressed_to(text,user,&users),"{text}");}
        assert!(addressed_to(&token,user,&HashMap::new()),"canonical IDs work before profiles arrive");
        for text in [format!("@mention(<@{other}>)"),format!("`{token}`"),format!("```rust\n{token}\n```"),format!("https://example.test/{token}"),"https://example.test/@ISpeakHue".into(),"HTTPS://example.test/@everyone".into(),"`@everyone`".into(),"@everyone_else".into(),"email@everyone.example".into(),"@everyone.example".into(),"_@everyone".into(),"@ISpeakHue_extra".into(),"email@ISpeakHue.example".into(),"@mention(<@invalid>)".into(),"`unclosed @ISpeakHue".into()]{assert!(!addressed_to(&text,user,&users),"{text}");}
        let duplicate=HashMap::from([(user,member(user,"one","Same")),(other,member(other,"two","Same"))]);
        assert!(!addressed_to("@Same",user,&duplicate));assert!(addressed_to("@one",user,&duplicate));
    }
    #[test] fn tokens_render_without_markup_and_unknown_users_are_safe(){let id=Uuid::new_v4();assert_eq!(render(&format!("olá @MENTION(<@{id}>) <b>!"),&HashMap::new()),"olá @Usuário removido <b>!");assert_eq!(render("@mention(<@invalid>)",&HashMap::new()),"@mention(<@invalid>)");}
    #[test] fn completion_preserves_unicode_and_text_after_caret(){let id=Uuid::new_v4();let(text,caret)=insert("é @jo depois",5,Some(id)).unwrap();assert_eq!(text,format!("é @mention(<@{id}>)  depois"));assert_eq!(caret,text[..text.find(" depois").unwrap()].chars().count() as i32);assert!(completion("email@host",10).is_none());assert!(completion(&text,caret).is_none());}
    #[test] fn duplicate_names_keep_exact_ids_through_edits_and_plain_names_never_tag(){
        let a=Uuid::new_v4();let b=Uuid::new_v4();
        let user=|id|serde_json::from_value(serde_json::json!({"id":id,"username":"same","nickname":"Íris <b>","created_at":"2026-10-03T00:00:00Z"})).unwrap();
        let wire=format!("é @mention(<@{a}>) @mention(<@{b}>) @Íris <b>");let mut text=DisplayText::parse(&wire,&HashMap::from([(a,user(a)),(b,user(b))]));
        assert_eq!(text.text,"é @Íris <b> @Íris <b> @Íris <b>");assert_eq!(text.wire_text(),wire);
        text.replace(0,0,"🐈 ");assert_eq!(text.wire_text(),format!("🐈 {wire}"));
        let first=text.spans[0].clone();text.replace(first.start,first.end+1,"");
        assert_eq!(text.wire_text(),format!("🐈 é @mention(<@{b}>) @Íris <b>"));
        let second=text.spans[0].clone();text.replace(second.start+1,second.start+1,"x");
        assert!(text.spans.is_empty());assert!(!text.wire_text().contains("@mention("));
    }
    #[test] fn edits_at_mention_boundaries_preserve_the_target(){
        let id=Uuid::new_v4();let wire=format!("@mention(<@{id}>)");let mut text=DisplayText::parse(&wire,&HashMap::new());
        text.replace(0,0,"before ");let end=text.spans[0].end;text.replace(end,end," after");
        assert_eq!(text.wire_text(),format!("before {wire} after"));
        let start=text.spans[0].start;text.replace(start,start+1,"");assert!(text.spans.is_empty());
    }
    #[test] fn everyone_survives_draft_restoration_and_does_not_match_longer_names(){
        let text=DisplayText::parse("Olá @everyone! @everyone_else email@everyone.example",&HashMap::new());
        assert_eq!(text.spans.len(),1);assert_eq!(text.wire_text(),text.text);
    }
    #[test] fn plain_read_only_mentions_resolve_unique_names_without_changing_drafts(){
        let a=Uuid::new_v4();let b=Uuid::new_v4();
        let user=|id,username,nickname|serde_json::from_value(serde_json::json!({"id":id,"username":username,"nickname":nickname,"created_at":"2026-10-03T00:00:00Z"})).unwrap();
        let users=HashMap::from([(a,user(a,"ProDesert22","Íris")),(b,user(b,"other","Same"))]);
        let text="é @ProDesert22, @Íris! email@ProDesert22.example https://test/@ProDesert22 `@Íris` @ProDesert22_extra @unknown";
        let read=DisplayText::read(text,&users);assert_eq!(read.text,text);assert_eq!(read.spans.iter().map(|s|(&read.text[s.start..s.end],s.id)).collect::<Vec<_>>(),vec![("@ProDesert22",Some(a)),("@Íris",Some(a))]);
        assert!(DisplayText::parse(text,&users).spans.is_empty(),"read-only links must not add hidden draft targets");
        let duplicate=HashMap::from([(a,user(a,"one","Same")),(b,user(b,"two","Same"))]);assert!(DisplayText::read("@Same",&duplicate).spans.is_empty());
        let canonical=format!("@mention(<@{b}>) @Same @one");let read=DisplayText::read(&canonical,&duplicate);assert_eq!(read.spans.len(),2);assert_eq!(read.spans[0].id,Some(b));assert_eq!(read.spans[1].id,Some(a));
    }
}

/// Shared suggestion row for the composer and message editor.
pub(super) fn choice(user:&UserSummary,texture:Option<&gtk::gdk::Texture>,show_avatar:bool,selected:impl Fn()+'static)->gtk::Button {
    use gtk::prelude::*;
    let button=gtk::Button::new();button.add_css_class("flat");button.set_tooltip_text(Some(&format!("@{}",user.username)));
    button.update_property(&[gtk::accessible::Property::Label(&format!("{} · @{}",user.display_name(),user.username))]);
    let line=gtk::Box::new(gtk::Orientation::Horizontal,10);line.set_valign(gtk::Align::Center);
    let avatar=crate::media::member_avatar(user.id,texture,32);avatar.set_text(Some(user.display_name()));avatar.set_valign(gtk::Align::Center);avatar.set_visible(show_avatar);line.append(&avatar);
    let name=gtk::Label::new(Some(user.display_name()));name.set_xalign(0.0);name.set_hexpand(true);name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if let Some(roles)=user.roles.as_deref(){crate::ui::style::role_color(&name,roles);}line.append(&name);
    let username=gtk::Label::new(Some(&format!("@{}",user.username)));username.add_css_class("dim-label");username.set_ellipsize(gtk::pango::EllipsizeMode::End);line.append(&username);
    button.set_child(Some(&line));button.connect_clicked(move |_|selected());button
}
