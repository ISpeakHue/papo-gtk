//! EN-US: Embedded application catalogs; protocol values and user content are never translated.
//! PT-BR: Catálogos embutidos da aplicação; valores do protocolo e conteúdo do usuário nunca são traduzidos.
use std::{collections::BTreeMap,sync::{LazyLock,atomic::{AtomicU8,Ordering}}};
use serde::{Deserialize,Serialize};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="kebab-case")]
pub enum Language { #[default] System, #[serde(rename="pt-BR")] Portuguese, #[serde(rename="en-US")] English }
impl Language {
    pub fn index(self)->u32{match self{Self::System=>0,Self::Portuguese=>1,Self::English=>2}}
    pub fn from_index(value:u32)->Self{match value{1=>Self::Portuguese,2=>Self::English,_=>Self::System}}
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Message { Text(String), Plural{one:String,other:String} }
static PT:LazyLock<BTreeMap<String,Message>>=LazyLock::new(||serde_json::from_str(include_str!("../../locales/pt-BR.json")).expect("validated Portuguese catalog"));
static EN:LazyLock<BTreeMap<String,Message>>=LazyLock::new(||serde_json::from_str(include_str!("../../locales/en-US.json")).expect("validated English catalog"));
// EN-US: Ordinary tests keep the historical Portuguese assertions independent of the runner's locale.
// PT-BR: Testes comuns mantêm as asserções históricas em português independentemente do idioma da máquina.
static CURRENT:AtomicU8=AtomicU8::new(if cfg!(test){1}else{2});

pub fn resolve(preference:Language,system:&str)->Language{
    if preference!=Language::System{return preference;}
    if system.to_ascii_lowercase().starts_with("pt"){Language::Portuguese}else{Language::English}
}
fn system_language()->String{
    #[cfg(windows)]{
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
        let mut buffer=[0u16;85];let count=unsafe{GetUserDefaultLocaleName(buffer.as_mut_ptr(),buffer.len() as i32)};
        if count>1{return String::from_utf16_lossy(&buffer[..count as usize-1]);}
    }
    for name in ["LC_ALL","LC_MESSAGES","LANGUAGE","LANG"]{
        if let Ok(value)=std::env::var(name){if !value.is_empty(){return value.split(':').next().unwrap_or("en-US").into();}}
    }
    "en-US".into()
}
pub fn init(preference:Language){
    let language=resolve(preference,&system_language());
    CURRENT.store(if language==Language::Portuguese{1}else{2},Ordering::Relaxed);
    if preference!=Language::System{std::env::set_var("LANGUAGE",if language==Language::Portuguese{"pt_BR:pt"}else{"en_US:en"});}
}
pub fn current()->Language{if CURRENT.load(Ordering::Relaxed)==1{Language::Portuguese}else{Language::English}}
pub fn text_in<'a>(language:Language,key:&'a str)->&'a str{
    let catalog=if language==Language::Portuguese{&*PT}else{&*EN};
    match catalog.get(key){Some(Message::Text(value))=>value,_=>key}
}
pub fn tr(key:&str)->&str{text_in(current(),key)}

pub fn format_in(language:Language,key:&str,values:&[(&str,String)])->String{
    let template=text_in(language,key);
    substitute(template,values)
}
pub fn fmt(key:&str,values:&[(&str,String)])->String{format_in(current(),key,values)}
fn substitute(template:&str,values:&[(&str,String)])->String{
    // EN-US: Scan the template once so braces in names/message text are never interpreted again.
    // PT-BR: Percorre o modelo uma vez para nunca reinterpretar chaves em nomes/texto de mensagens.
    let mut result=String::with_capacity(template.len());let mut rest=template;
    while let Some(start)=rest.find('{'){
        result.push_str(&rest[..start]);rest=&rest[start..];
        let Some(end)=rest.find('}')else{break;};
        let name=&rest[1..end];
        if let Some((_,value))=values.iter().find(|(key,_)|*key==name){result.push_str(value);}else{result.push_str(&rest[..=end]);}
        rest=&rest[end+1..];
    }
    result.push_str(rest);result
}
pub fn plural_in(language:Language,key:&str,count:usize)->String{
    let catalog=if language==Language::Portuguese{&*PT}else{&*EN};
    let template=match catalog.get(key){Some(Message::Plural{one,other})=>if count==1{one}else{other},_=>key};
    substitute(template,&[("count",count.to_string())])
}
pub fn plural(key:&str,count:usize)->String{plural_in(current(),key,count)}
pub fn date_pattern()->&'static str{if current()==Language::Portuguese{"%d/%m/%Y"}else{"%m/%d/%Y"}}
pub fn datetime_pattern()->&'static str{if current()==Language::Portuguese{"%d/%m/%Y %H:%M"}else{"%m/%d/%Y %I:%M %p"}}
pub fn time_pattern()->&'static str{if current()==Language::Portuguese{"%H:%M"}else{"%I:%M %p"}}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn locale_resolution_overrides_and_fallbacks(){
        assert_eq!(resolve(Language::System,"pt_BR.UTF-8"),Language::Portuguese);
        assert_eq!(resolve(Language::System,"en_US.UTF-8"),Language::English);
        assert_eq!(resolve(Language::System,"fr_FR"),Language::English);
        assert_eq!(resolve(Language::Portuguese,"en-US"),Language::Portuguese);
    }
    #[test]fn catalog_keys_and_placeholder_sets_match(){
        assert_eq!(PT.keys().collect::<Vec<_>>(),EN.keys().collect::<Vec<_>>());
        fn placeholders(text:&str)->std::collections::BTreeSet<&str>{text.split('{').skip(1).filter_map(|s|s.split_once('}').map(|(key,_)|key)).collect()}
        for (key,pt) in &*PT{match (pt,&EN[key]){
            (Message::Text(a),Message::Text(b))=>assert_eq!(placeholders(a),placeholders(b),"{key}"),
            (Message::Plural{one:a,other:b},Message::Plural{one:c,other:d})=>{assert_eq!(placeholders(a),placeholders(c),"{key}");assert_eq!(placeholders(b),placeholders(d),"{key}");},
            _=>panic!("Catalog type mismatch: {key}"),
        }}
    }
    #[test]fn plural_forms_and_user_values_are_not_reinterpreted(){
        assert_eq!(plural_in(Language::English,"messages_count",0),"0 messages");
        assert_eq!(plural_in(Language::English,"messages_count",1),"1 message");
        assert_eq!(plural_in(Language::Portuguese,"messages_count",2),"2 mensagens");
        assert_eq!(substitute("Hello {name}",&[("name","{other}".into()),("other","wrong".into())]),"Hello {other}");
    }
}
