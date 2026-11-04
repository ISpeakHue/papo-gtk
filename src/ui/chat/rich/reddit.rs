//! EN-US: Recognise Reddit permalinks and share URLs without fetching or executing their HTML.
//! PT-BR: Reconhece links permanentes e compartilhados do Reddit sem buscar ou executar seu HTML.
use super::*;

pub(super) struct Post {pub url:reqwest::Url,pub community:Option<String>,pub comment:bool}
fn identifier(value:&str)->bool{!value.is_empty()&&value.len()<=128&&value.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_')}
pub(super) fn post(embed:&Embed)->Option<Post>{
    if embed.source_type!="link"{return None;}
    let url=embed.url.as_deref().and_then(safe_url)?;
    let host=url.host_str()?;
    let parts:Vec<_>=url.path().trim_matches('/').split('/').collect();
    let (community,comment)=if matches!(host,"redd.it"|"www.redd.it"){
        if parts.len()!=1||!identifier(parts[0]){return None;}(None,false)
    }else{
        if !matches!(host,"reddit.com"|"www.reddit.com"|"old.reddit.com"|"new.reddit.com"|"sh.reddit.com"|"m.reddit.com"|"np.reddit.com"){return None;}
        match parts.as_slice(){
            ["r",community,"comments",id,tail @ ..] if identifier(community)&&identifier(id)=>(Some((*community).to_owned()),tail.get(1).is_some_and(|c|identifier(c))),
            ["r",community,"s",token] if identifier(community)&&identifier(token)=>(Some((*community).to_owned()),false),
            ["comments",id,tail @ ..] if identifier(id)=>(None,tail.get(1).is_some_and(|c|identifier(c))),
            ["user"|"u",user,"comments",id,tail @ ..] if identifier(user)&&identifier(id)=>(None,tail.get(1).is_some_and(|c|identifier(c))),
            ["gallery",id] if identifier(id)=>(None,false),
            _=>return None,
        }
    };
    Some(Post{url,community,comment})
}
pub(super) fn heading(card:&gtk::Box,post:&Post){
    let header=gtk::Box::new(gtk::Orientation::Horizontal,4);header.set_widget_name("reddit-attribution");
    let provider=label("Reddit");provider.add_css_class("caption");provider.add_css_class("dim-label");header.append(&provider);
    if let Some(community)=&post.community{
        header.append(&gtk::Label::new(Some("·")));
        let link=gtk::LinkButton::with_label(&format!("https://www.reddit.com/r/{community}/"),&format!("r/{community}"));
        if let Some(label)=link.child().and_downcast::<gtk::Label>(){label.set_ellipsize(pango::EllipsizeMode::End);label.set_max_width_chars(24);}
        link.add_css_class("caption");link.set_widget_name("reddit-community");header.append(&link);
    }
    card.append(&header);
}

#[cfg(test)]pub(super) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,descendants,find_button,pump,until};use adw::prelude::*;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let user=Uuid::new_v4();let channel=Uuid::new_v4();let now=chrono::Utc::now();let id=Uuid::new_v4();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Reddit embeds","created_at":now})).unwrap();
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":"https://www.reddit.com/r/factorio/s/S2nxyUEQsw","created_at":now})).unwrap();
    let mut embed:Embed=serde_json::from_value(serde_json::json!({"id":id,"source_type":"link","fetch_method":"oembed","url":message.content,"provider":"reddit","title":"Zenith | 3D Factorio Mod Out NOW","author":{"name":"unjustodin"},"created_at":now,"fetched_at":now})).unwrap();
    let publish=|embed:&Embed|{
        let mut payload=serde_json::to_value(embed).unwrap();
        if embed.thumbnail.is_some(){payload["image_data"]=serde_json::json!(actions::tests::tiny_png_base64());}
        backend.lock().unwrap().embed_values.insert(id.to_string(),payload);
    };
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target.clone()),messages:vec![message.clone()],user_id:Some(user),api:Some(api),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();performance::settle(context);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![embed.clone()])));performance::settle(context);
    let card=||descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("embed-{id}")).unwrap();
    assert!(card().has_css_class("papo-reddit-embed"));
    let all=descendants(&card());
    assert!(all.iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="Reddit")));
    let community=all.iter().find_map(|w|w.downcast_ref::<gtk::LinkButton>().filter(|w|w.widget_name()=="reddit-community")).unwrap();assert_eq!(community.uri(),"https://www.reddit.com/r/factorio/");
    let open=all.iter().find_map(|w|w.downcast_ref::<gtk::LinkButton>().filter(|w|w.widget_name()=="reddit-open")).unwrap();assert_eq!(open.uri(),message.content.as_deref().unwrap());
    assert!(backend.lock().unwrap().requests.iter().all(|(_,path,_)|!path.starts_with("/embeds/")),"complete text metadata needs no extra request");
    embed.fetched_at=Some(now+chrono::Duration::seconds(1));embed.thumbnail=Some(crate::models::EmbedMedia{mime_type:Some("image/png".into()),width:Some(32),height:Some(32),..Default::default()});
    publish(&embed);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![embed.clone()])));
    until(context,||chat.model().transfers.textures.contains_key(&id));performance::settle(context);find_button(&card(),"Abrir imagem");
    let requests=||backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="GET"&&path==&format!("/embeds/{id}")).count();assert_eq!(requests(),1);
    chat.emit(ChatMsg::UpdateChannel(target));performance::settle(context);assert_eq!(requests(),1,"Reddit thumbnails use the existing cache");
    embed.description=Some("Long Reddit post text with **formatting**. ".repeat(25));embed.fetched_at=Some(now+chrono::Duration::seconds(2));
    publish(&embed);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![embed.clone()])));performance::settle(context);
    for width in [850,400]{actions::tests::resize_window(&window,width,680);performance::settle(context);let row=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()=="message-media").unwrap();let bounds=card().compute_bounds(&row).unwrap();assert!(bounds.x().abs()<=1.0&&bounds.width()<=row.width() as f32+1.0);}
    crate::ui::main_window::layout::tests::preview(&window,"reddit-embed",context);
    embed.video=Some(crate::models::EmbedMedia{url:Some("https://v.redd.it/example/DASH_720.mp4".into()),mime_type:Some("video/mp4".into()),..Default::default()});embed.fetched_at=Some(now+chrono::Duration::seconds(3));embed.description=None;
    publish(&embed);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![embed.clone()])));performance::settle(context);find_button(&card(),"Reproduzir vídeo");
    embed.title=None;embed.video=None;embed.thumbnail=None;embed.url=Some("https://www.reddit.com/r/factorio/comments/1xlmd0s/title/abc123/".into());embed.fetched_at=Some(now+chrono::Duration::seconds(4));
    publish(&embed);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![embed])));performance::settle(context);find_button(&card(),"Ver comentário no Reddit");
    assert!(descendants(&card()).iter().any(|w|w.downcast_ref::<gtk::LinkButton>().is_some_and(|b|b.label().as_deref()==Some("Comentário no Reddit"))));
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![])));performance::settle(context);assert!(!descendants(chat.widget().upcast_ref()).iter().any(|w|w.has_css_class("papo-reddit-embed")));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
pub(super) fn action(card:&gtk::Box,post:&Post){
    let link=gtk::LinkButton::with_label(post.url.as_str(),if post.comment{"Ver comentário no Reddit"}else{"Ver publicação no Reddit"});
    link.set_halign(gtk::Align::Start);link.add_css_class("caption");link.set_widget_name("reddit-open");card.append(&link);
}

#[cfg(test)]mod tests{
    use super::*;
    fn embed(url:&str)->Embed{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"link","url":url})).unwrap()}
    #[test]fn reddit_post_comment_share_and_gallery_links(){
        for url in ["https://www.reddit.com/r/factorio/s/S2nxyUEQsw","https://old.reddit.com/r/rust/comments/abc123/title/","https://m.reddit.com/r/rust/comments/abc123/title/?context=3"]{
            let post=post(&embed(url)).unwrap();assert!(post.community.is_some());assert!(!post.comment);
        }
        let comment=post(&embed("https://www.reddit.com/r/rust/comments/abc123/title/def456/?context=3")).unwrap();assert!(comment.comment);assert_eq!(comment.community.as_deref(),Some("rust"));
        for url in ["https://redd.it/abc123","https://www.reddit.com/comments/abc123/","https://www.reddit.com/gallery/abc123","https://www.reddit.com/user/alice/comments/abc123/title/"]{assert!(post(&embed(url)).is_some());}
    }
    #[test]fn unrelated_or_spoofed_links_and_custom_cards_stay_generic(){
        for url in ["https://www.reddit.com/","https://www.reddit.com/r/rust/","https://reddit.com.evil.test/r/rust/comments/abc123/","https://evilreddit.com/r/rust/comments/abc123/","https://reddit.com@evil.test/r/rust/comments/abc123/","https://alice@www.reddit.com/r/rust/comments/abc123/","javascript:alert(1)","https://www.reddit.com/r/%3Cscript%3E/comments/abc123/","https://redd.it/a/b"]{assert!(post(&embed(url)).is_none(),"{url}");}
        let mut custom=embed("https://www.reddit.com/r/rust/comments/abc123/");custom.source_type="custom".into();assert!(post(&custom).is_none());
        let mut spoof=embed("https://example.test/");spoof.provider=Some("Reddit".into());assert!(post(&spoof).is_none());
    }
}
