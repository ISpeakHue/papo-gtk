//! Rich embed metadata and validated custom embed inputs.
use chrono::{DateTime,Utc};
use serde::{Deserialize,Serialize};
use uuid::Uuid;

#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
#[serde(from="EmbedWire")]
pub struct Embed {
    pub id: Uuid,
    #[serde(default)] pub source_type: String,
    #[serde(default)] pub fetch_method: String,
    pub provider: Option<String>,
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub author: Option<EmbedAuthor>,
    pub thumbnail: Option<EmbedMedia>,
    pub image: Option<EmbedMedia>,
    pub video: Option<EmbedMedia>,
    pub footer: Option<EmbedFooter>,
    #[serde(default, deserialize_with="nullable_fields")] pub fields: Vec<EmbedField>,
    pub embed_url: Option<String>,
    #[serde(default)] pub created_at: DateTime<Utc>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub image_data: Option<String>,
}
// Accept older history/fixture payloads while exposing the new nested contract.
#[derive(Deserialize)]
struct EmbedWire {
    pub id: Uuid,
    #[serde(default)] pub source_type: String,
    #[serde(default)] pub fetch_method: String,
    pub provider: Option<String>,
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub author: Option<EmbedAuthor>,
    pub thumbnail: Option<EmbedMedia>,
    pub image: Option<EmbedMedia>,
    pub video: Option<EmbedMedia>,
    pub footer: Option<EmbedFooter>,
    #[serde(default, deserialize_with="nullable_fields")] pub fields: Vec<EmbedField>,
    pub embed_url: Option<String>,
    #[serde(default)] pub created_at: DateTime<Utc>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub image_data: Option<String>,
    provider_name: Option<String>,
    image_mime_type: Option<String>,
    image_size_bytes: Option<i64>,
    video_url: Option<String>,
}
impl From<EmbedWire> for Embed {
    fn from(w:EmbedWire)->Self{
        Self { id:w.id,source_type:if w.source_type.is_empty(){"link".into()}else{w.source_type},fetch_method:w.fetch_method,
            provider:w.provider.or(w.provider_name),site_name:w.site_name,url:w.url,title:w.title,description:w.description,color:w.color,
            author:w.author,thumbnail:w.thumbnail.or_else(||w.image_mime_type.map(|mime|EmbedMedia{mime_type:Some(mime),size_bytes:w.image_size_bytes,..Default::default()})),
            image:w.image,video:w.video.or_else(||w.video_url.map(|url|EmbedMedia{url:Some(url),..Default::default()})),footer:w.footer,fields:w.fields,
            embed_url:w.embed_url,created_at:w.created_at,fetched_at:w.fetched_at,image_data:w.image_data }
    }
}
fn nullable_fields<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<EmbedField>,D::Error>{Ok(Option::<Vec<EmbedField>>::deserialize(d)?.unwrap_or_default())}
#[derive(Debug,Clone,Default,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedMedia {pub url:Option<String>,pub mime_type:Option<String>,pub width:Option<i32>,pub height:Option<i32>,pub size_bytes:Option<i64>}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedAuthor {pub name:Option<String>,pub url:Option<String>,pub media:Option<EmbedMedia>}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedFooter {pub text:Option<String>,pub icon:Option<EmbedMedia>}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedField {#[serde(default)] pub position:i32,pub name:String,pub value:String,#[serde(default)] pub inline:bool}
impl Embed {
    pub fn version(&self)->DateTime<Utc>{self.fetched_at.unwrap_or(self.created_at)}
    pub fn has_video(&self)->bool{self.video.as_ref().and_then(|v|v.url.as_ref()).is_some()}
    pub fn metadata(&self)->Self{
        Self{id:self.id,source_type:self.source_type.clone(),fetch_method:self.fetch_method.clone(),provider:self.provider.clone(),site_name:self.site_name.clone(),url:self.url.clone(),title:self.title.clone(),description:self.description.clone(),color:self.color.clone(),author:self.author.clone(),thumbnail:self.thumbnail.clone(),image:self.image.clone(),video:self.video.clone(),footer:self.footer.clone(),fields:self.fields.clone(),embed_url:self.embed_url.clone(),created_at:self.created_at,fetched_at:self.fetched_at,image_data:None}
    }
}

#[derive(Debug,Clone,Default,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedInput {
    #[serde(default)] pub title:String,
    #[serde(default)] pub description:String,
    #[serde(default)] pub url:String,
    #[serde(default)] pub color:String,
    #[serde(default)] pub site_name:String,
    pub author:Option<EmbedAuthorInput>,
    pub footer:Option<EmbedFooterInput>,
    pub thumbnail:Option<EmbedMediaInput>,
    pub image:Option<EmbedMediaInput>,
    pub video:Option<EmbedMediaInput>,
    #[serde(default)] pub fields:Vec<EmbedFieldInput>,
}
#[derive(Debug,Clone,Default,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedAuthorInput {#[serde(default)] pub name:String,#[serde(default)] pub url:String}
#[derive(Debug,Clone,Default,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedFooterInput {#[serde(default)] pub text:String}
#[derive(Debug,Clone,Default,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedMediaInput {pub url:String,#[serde(default)] pub mime_type:String}
#[derive(Debug,Clone,Default,Deserialize,Serialize,PartialEq,Eq)]
pub struct EmbedFieldInput {pub name:String,pub value:String,#[serde(default)] pub inline:bool}

pub fn validate_embeds(embeds:&[EmbedInput])->anyhow::Result<()> {
    use anyhow::ensure;
    fn length(s:&str,max:usize)->anyhow::Result<()>{ensure!(s.chars().count()<=max,"Campo do embed excede {max} caracteres.");Ok(())}
    fn url(s:&str,https:bool)->anyhow::Result<()>{
        length(s,2048)?;let u=reqwest::Url::parse(s.trim())?;
        ensure!(matches!(u.scheme(),"http"|"https")&&(!https||u.scheme()=="https")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none()&&u.port().is_none(),"Use uma URL {} sem credenciais ou porta alternativa.",if https{"HTTPS"}else{"HTTP/HTTPS"});Ok(())
    }
    ensure!(embeds.len()<=10,"Use até 10 embeds por mensagem.");let mut total=0;
    for e in embeds{
        length(&e.title,256)?;length(&e.description,4192)?;length(&e.site_name,256)?;
        total+=e.title.chars().count()+e.description.chars().count();
        ensure!(e.color.is_empty()||(e.color.len()==7&&e.color.starts_with('#')&&e.color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)),"A cor deve usar #RRGGBB.");
        if !e.url.is_empty(){url(&e.url,false)?;}
        if let Some(a)=&e.author{length(&a.name,256)?;total+=a.name.chars().count();if !a.url.is_empty(){url(&a.url,false)?;}}
        if let Some(f)=&e.footer{length(&f.text,2048)?;total+=f.text.chars().count();}
        ensure!(e.fields.len()<=25,"Use até 25 campos por embed.");
        for f in &e.fields{length(&f.name,256)?;length(&f.value,1024)?;total+=f.name.chars().count()+f.value.chars().count();}
        for m in [&e.thumbnail,&e.image].into_iter().flatten(){url(&m.url,true)?;ensure!(m.mime_type.is_empty()||["image/png","image/jpeg","image/webp","image/gif"].contains(&m.mime_type.to_lowercase().as_str()),"Formato de imagem não suportado.");}
        if let Some(v)=&e.video{url(&v.url,true)?;let mime=v.mime_type.split(';').next().unwrap_or("").trim().to_lowercase();ensure!(["video/mp4","video/webm","video/ogg"].contains(&mime.as_str()),"Informe o formato do vídeo: video/mp4, video/webm ou video/ogg.");}
    }
    ensure!(total<=6000,"O texto total dos embeds deve ter até 6000 caracteres.");Ok(())
}

impl EmbedInput {
    pub fn from_embed(e:&Embed)->Self{
        let media=|m:&Option<EmbedMedia>|m.as_ref().map(|m|EmbedMediaInput{url:m.url.clone().unwrap_or_default(),mime_type:m.mime_type.clone().unwrap_or_default()});
        let mut fields=e.fields.clone();fields.sort_by_key(|f|f.position);
        Self{title:e.title.clone().unwrap_or_default(),description:e.description.clone().unwrap_or_default(),url:e.url.clone().unwrap_or_default(),color:e.color.clone().unwrap_or_default(),site_name:e.site_name.clone().unwrap_or_default(),
            author:e.author.as_ref().map(|a|EmbedAuthorInput{name:a.name.clone().unwrap_or_default(),url:a.url.clone().unwrap_or_default()}),footer:e.footer.as_ref().map(|f|EmbedFooterInput{text:f.text.clone().unwrap_or_default()}),
            thumbnail:media(&e.thumbnail),image:media(&e.image),video:media(&e.video),fields:fields.into_iter().map(|f|EmbedFieldInput{name:f.name,value:f.value,inline:f.inline}).collect()}
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn custom_metadata_has_no_required_url_or_fetch_timestamp(){
        let e:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","title":"Rich card","color":"#123ABC","created_at":"2026-10-09T12:00:00Z","author":{"name":"Author"},"thumbnail":{"mime_type":"image/png","width":120,"height":80},"image":{"mime_type":"image/webp"},"footer":{"text":"Footer"},"fields":[{"position":2,"name":"Second","value":"B","inline":true},{"position":1,"name":"First","value":"A","inline":false}]})).unwrap();
        assert!(e.url.is_none()&&e.fetched_at.is_none());assert_eq!(e.version(),e.created_at);let input=EmbedInput::from_embed(&e);assert_eq!(input.fields[0].name,"First");assert!(validate_embeds(&[input]).is_err(),"missing original image URLs cannot be silently resubmitted");
    }
    #[test]fn custom_limits_count_unicode_and_total_text_across_cards(){
        let mut e=EmbedInput{title:"🐈".repeat(256),description:"a".repeat(4192),..Default::default()};assert!(validate_embeds(&[e.clone()]).is_ok());e.title.push('x');assert!(validate_embeds(&[e]).is_err());
        let a=EmbedInput{description:"x".repeat(3000),..Default::default()};assert!(validate_embeds(&[a.clone(),a.clone()]).is_ok());let mut b=a.clone();b.footer=Some(EmbedFooterInput{text:"x".into()});assert!(validate_embeds(&[a.clone(),b]).is_err());assert!(validate_embeds(&vec![a;11]).is_err());
    }
    #[test]fn custom_media_urls_and_colors_follow_backend_validation(){
        let mut e=EmbedInput{color:"#12abEF".into(),video:Some(EmbedMediaInput{url:"https://example.test/a.mp4".into(),mime_type:"video/mp4; codecs=avc1".into()}),..Default::default()};assert!(validate_embeds(&[e.clone()]).is_ok());
        for value in ["http://example.test/a.mp4","https://user:pass@example.test/a.mp4","https://example.test:8443/a.mp4","javascript:alert(1)"]{e.video.as_mut().unwrap().url=value.into();assert!(validate_embeds(&[e.clone()]).is_err());}
        e.video=None;e.color="red; background: url(https://example.test)".into();assert!(validate_embeds(&[e]).is_err());
    }
}
