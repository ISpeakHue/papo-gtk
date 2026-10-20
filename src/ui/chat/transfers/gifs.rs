//! GIF media has one bare surface in chat; metadata belongs to its open viewer.
use super::*;

pub(super) fn keys(embed:&Embed)->Vec<String>{
    let mut keys=vec![];
    for url in embed.url.iter().chain(embed.title.iter()).chain(embed.description.iter()).chain([embed.video.as_ref(),embed.image.as_ref(),embed.thumbnail.as_ref()].into_iter().flatten().filter_map(|m|m.url.as_ref())){
        for key in crate::media::gif_links::links(url){if !keys.contains(&key){keys.push(key);}}
    }keys
}
pub(super) fn source(media:&Transfers,message:Uuid,key:Uuid,messages:&[Message])->Option<String>{
    if let Some(embed)=messages.iter().find(|m|m.id==message).and_then(|m|m.embeds.iter().flatten().find(|p|p.id==key)){
        if let Some(url)=embed.url.as_ref().filter(|u|super::super::rich::safe_url(u).is_some()){return Some(url.clone());}
        for text in embed.description.iter().chain(embed.title.iter()){
            if let Some((start,end,_))=crate::media::gif_links::ranges(text).first(){if super::super::rich::safe_url(&text[*start..*end]).is_some(){return Some(text[*start..*end].into());}}
        }
    }
    media.giphy.iter().find(|(_,id)|**id==key).map(|(url,_)|if url.starts_with("https://"){url.clone()}else{format!("https://giphy.com/gifs/{url}")})
}
pub(super) fn separate_animation(embed:&Embed)->bool{embed.animated_link()&&embed.has_video()&&embed.fetch_method!="gif"}
fn info(embed:Option<&Embed>,source:Option<&str>)->gtk::Box{
    let body=gtk::Box::new(gtk::Orientation::Vertical,6);body.set_margin_start(12);body.set_margin_end(12);body.set_margin_top(10);body.set_margin_bottom(10);
    if let Some(title)=embed.and_then(|p|p.title.as_deref()).filter(|s|!s.is_empty()){
        let label=gtk::Label::new(Some(title));label.add_css_class("heading");label.set_xalign(0.0);label.set_wrap(true);label.set_max_width_chars(60);label.set_lines(2);label.set_ellipsize(pango::EllipsizeMode::End);body.append(&label);
    }
    if let Some(url)=source.and_then(super::super::rich::safe_url){
        let link=gtk::LinkButton::with_label(url.as_str(),url.as_str());link.set_halign(gtk::Align::Start);link.set_tooltip_text(Some(url.as_str()));if let Some(label)=link.child().and_downcast::<gtk::Label>(){label.set_max_width_chars(60);label.set_ellipsize(pango::EllipsizeMode::Middle);}body.append(&link);
    }
    if let Some(description)=embed.and_then(|p|p.description.as_deref()).filter(|s|!s.is_empty()){
        let label=gtk::Label::new(Some(description));label.set_xalign(0.0);label.set_wrap(true);label.set_max_width_chars(60);label.set_lines(3);label.set_ellipsize(pango::EllipsizeMode::End);body.append(&label);
    }
    if body.first_child().is_none(){body.append(&gtk::Label::new(Some("GIF")));}body
}
pub(super) fn widget(embed:Option<&Embed>,source:Option<&str>,message:Uuid,key:Uuid,media:&Transfers,sender:&ComponentSender<ChatModel>)->gtk::Box{
    let container=gtk::Box::new(gtk::Orientation::Vertical,0);container.set_widget_name(&format!("gif-{key}"));container.set_halign(gtk::Align::Start);
    if let Some(picture)=media.picture(key){
        let button=gtk::Button::new();button.set_has_frame(false);button.set_halign(gtk::Align::Start);button.add_css_class("papo-inline-image");button.set_tooltip_text(Some("Abrir GIF"));button.set_child(Some(&media.image_frame(key,&picture,true,None)));
        let input=sender.input_sender().clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::OpenImage{message,key}));});container.append(&button);
    }else{
        let placeholder=gtk::Box::new(gtk::Orientation::Vertical,6);placeholder.set_halign(gtk::Align::Center);placeholder.set_valign(gtk::Align::Center);
        let failed=media.failed.contains_key(&key)&&!media.requested.contains(&key);
        if failed{let icon=gtk::Image::from_icon_name("papo-image-missing-symbolic");icon.set_pixel_size(24);placeholder.append(&icon);}else{placeholder.append(&gtk::Spinner::builder().spinning(true).build());}
        let button=gtk::MenuButton::new();button.set_has_frame(false);button.add_css_class("papo-inline-image");button.set_tooltip_text(Some(if failed{"GIF indisponível — informações e tentar novamente"}else{"Carregando GIF — informações"}));button.set_child(Some(&media.placeholder(key,&placeholder,true)));
        let popover=gtk::Popover::new();let body=info(embed,source);
        if failed{let label=gtk::Label::new(Some("Não foi possível carregar o GIF."));label.add_css_class("dim-label");body.append(&label);}
        let retry=gtk::Button::with_label("Tentar novamente");let input=sender.input_sender().clone();let weak=popover.downgrade();retry.connect_clicked(move |_|{if let Some(popover)=weak.upgrade(){popover.popdown();}let _=input.send(ChatMsg::Transfer(TransferMsg::RetryImage(key)));});body.append(&retry);popover.set_child(Some(&body));button.set_popover(Some(&popover));container.append(&button);
    }container
}
pub(super) fn viewer(window:&gtk::Window,embed:Option<&Embed>,source:Option<&str>){
    let Some(root)=window.child().and_downcast::<gtk::Box>()else{return;};window.set_title(Some("GIF"));
    let body=info(embed,source);body.set_widget_name("gif-information");
    root.append(&body);let weak=body.downgrade();window.connect_fullscreened_notify(move |window|{if let Some(body)=weak.upgrade(){body.set_visible(!window.is_fullscreen());}});
}
// Mask supported sources before hydration too, so the original URL cannot flash
// or survive in a reused text row. At most four distinct links get media tiles.
fn without_links(text:&str,links:&[String])->String{
    let mut result=String::new();let mut start=0;
    for(begin,end,key) in crate::media::gif_links::ranges(text){
        if links.contains(&key){result.push_str(&text[start..begin]);start=end;}
    }
    if start==0{return text.into();}result.push_str(&text[start..]);result.trim().into()
}
pub(in crate::ui::chat) fn display_text(message:&Message)->String{
    let text=message.content.as_deref().unwrap_or("");
    let available:Vec<_>=message.embeds.iter().flatten().filter(|p|p.animated_link()).flat_map(keys).collect();
    without_links(text,&available)
}
pub(super) fn attribution<'a>(embed:&'a Embed,message:&Message)->std::borrow::Cow<'a,Embed>{
    use std::borrow::Cow;
    if embed.animated_link(){return Cow::Borrowed(embed);}
    let Some(description)=embed.description.as_deref()else{return Cow::Borrowed(embed);};
    let links:Vec<_>=message.embeds.iter().flatten().filter(|p|p.animated_link()).flat_map(keys).chain(crate::media::gif_links::links(message.content.as_deref().unwrap_or(""))).collect();
    let text=without_links(description,&links);
    if text==description{return Cow::Borrowed(embed);}
    let mut copy=embed.metadata();copy.description=(!text.is_empty()).then_some(text);Cow::Owned(copy)
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn source_links_are_hidden_before_hydration_but_prose_and_code_remain(){
        let url="https://tenor.com/nihxadXt69o.gif";
        let mut msg:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"created_at":chrono::Utc::now(),"content":format!("Here {url} hello\n`{url}`")})).unwrap();
        assert_eq!(display_text(&msg),msg.content.as_deref().unwrap(),"unresolved links remain readable");
        msg.embeds=Some(vec![serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"fetch_method":"gif","url":url,"thumbnail":{"mime_type":"image/gif"}})).unwrap()]);
        assert_eq!(display_text(&msg),format!("Here  hello\n`{url}`"));
        msg.content=Some(url.into());assert!(display_text(&msg).is_empty());
        msg.content=Some("giphy:a giphy:b giphy:c giphy:d giphy:e".into());assert_eq!(display_text(&msg),msg.content.as_deref().unwrap(),"unresolved links remain accessible");
    }
    #[test]fn bridge_identity_is_preserved_while_its_duplicate_gif_url_is_hidden(){
        let url="https://klipy.com/gifs/halloween-scary-22";
        let mut msg:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"created_at":chrono::Utc::now(),"embeds":[
            {"id":Uuid::new_v4(),"source_type":"custom","site_name":"Sender","author":{"name":"Sender"},"description":url,"footer":{"text":"Discord"}},
            {"id":Uuid::new_v4(),"source_type":"custom","site_name":"klipy.com","url":url,"video":{"url":"https://images-ext-1.discordapp.net/example.mp4"}}
        ]})).unwrap();
        let embeds=msg.embeds.as_ref().unwrap();let displayed=attribution(&embeds[0],&msg);
        assert!(displayed.description.is_none());assert_eq!(displayed.author,embeds[0].author);assert_eq!(displayed.footer,embeds[0].footer);
        assert_eq!(embeds[0].description.as_deref(),Some(url),"wire data stays intact");
        msg.embeds.as_mut().unwrap().truncate(1);assert_eq!(attribution(&msg.embeds.as_ref().unwrap()[0],&msg).description.as_deref(),Some(url),"a source without a media tile must remain accessible");
    }
}
