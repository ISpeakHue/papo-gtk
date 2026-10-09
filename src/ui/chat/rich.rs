//! Native rich cards. Remote HTML and original media URLs are never loaded.
use super::*;

pub(super) fn safe_url(value:&str)->Option<reqwest::Url>{
    reqwest::Url::parse(value).ok().filter(|u|matches!(u.scheme(),"http"|"https")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none())
}
pub(super) fn youtube_url(value:&str)->Option<reqwest::Url>{
    safe_url(value).filter(|u|u.scheme()=="https"&&u.host_str()==Some("www.youtube.com")&&u.path().starts_with("/embed/")&&u.path_segments().is_some_and(|mut parts|{parts.next()==Some("embed")&&parts.next().is_some_and(|id|id.len()==11&&id.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-')))&&parts.next().is_none()}))
}
fn label(text:&str)->gtk::Label{
    let l=gtk::Label::new(Some(text));l.set_xalign(0.0);l.set_wrap(true);l.set_wrap_mode(pango::WrapMode::WordChar);l.set_max_width_chars(45);l
}
fn linked(text:&str,url:Option<&str>)->gtk::Widget{
    if let Some(url)=url.and_then(safe_url){let link=gtk::LinkButton::with_label(url.as_str(),text);link.set_halign(gtk::Align::Start);if let Some(l)=link.child().and_downcast::<gtk::Label>(){l.set_max_width_chars(45);l.set_ellipsize(pango::EllipsizeMode::End);}link.upcast()}
    else{label(text).upcast()}
}
pub(super) fn body(p:&Embed)->gtk::Box{
    let card=gtk::Box::new(gtk::Orientation::Vertical,6);card.set_widget_name(&format!("embed-{}",p.id));card.set_valign(gtk::Align::Start);card.set_overflow(gtk::Overflow::Hidden);card.set_margin_bottom(8);card.add_css_class("card");card.add_css_class("papo-link-preview");
    if let Some(color)=p.color.as_deref().filter(|s|s.len()==7&&s.starts_with('#')&&s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)){
        let css=gtk::CssProvider::new();css.load_from_string(&format!(".papo-link-preview{{border-left-color:{color};}}"));card.style_context().add_provider(&css,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    if let Some(provider)=p.site_name.as_deref().or(p.provider.as_deref()){let l=label(provider);l.add_css_class("caption");l.add_css_class("dim-label");card.append(&l);}
    if let Some(author)=&p.author{if let Some(name)=author.name.as_deref(){let w=linked(name,author.url.as_deref());w.add_css_class("heading");card.append(&w);}}
    if let Some(title)=p.title.as_deref().filter(|s|!s.is_empty()).or(p.url.as_deref()) {let w=linked(title,p.url.as_deref());w.add_css_class("heading");card.append(&w);}
    if let Some(description)=p.description.as_deref(){card.append(&label(description));}
    let grid=gtk::Grid::new();grid.set_column_spacing(12);grid.set_row_spacing(8);grid.set_column_homogeneous(true);grid.set_hexpand(true);
    let mut fields:Vec<_>=p.fields.iter().collect();fields.sort_by_key(|f|f.position);let(mut row,mut col)=(0,0);
    for field in fields{
        if !field.inline&&col!=0{row+=1;col=0;}
        let group=gtk::Box::new(gtk::Orientation::Vertical,3);group.set_hexpand(true);let name=label(&field.name);name.add_css_class("heading");group.append(&name);group.append(&label(&field.value));
        grid.attach(&group,col,row,if field.inline{1}else{2},1);
        if field.inline&&col==0{col=1;}else{row+=1;col=0;}
    }
    if !p.fields.is_empty(){card.append(&grid);}
    card
}
pub(super) fn footer(card:&gtk::Box,p:&Embed){
    // The current GET /embeds API supplies thumbnail bytes only. Keep image,
    // author-icon and footer-icon metadata; don't bypass channel authorization
    // by downloading their original URLs or content hashes.
    if p.image.is_some(){let l=label("Imagem incorporada indisponível no servidor.");l.add_css_class("dim-label");card.append(&l);}
    if let Some(text)=p.footer.as_ref().and_then(|f|f.text.as_deref()){let l=label(text);l.add_css_class("caption");l.add_css_class("dim-label");card.append(&l);}
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use actions::tests::{descendants,find_button,pump,until};
    use performance::settle;
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let created=chrono::Utc::now();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"embeds","created_at":created})).unwrap();
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":"Rich message","created_at":created})).unwrap();
    let embed:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","title":"<b>Plain title</b>","description":"Rich description","color":"#123ABC","created_at":created,"author":{"name":"Card author","url":"javascript:alert(1)"},"footer":{"text":"Card footer"},"fields":[{"position":1,"name":"Second","value":"B","inline":true},{"position":0,"name":"First","value":"A","inline":true}]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target.clone()),messages:vec![message.clone()],user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();settle(context);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![embed.clone()])));settle(context);
    let all=descendants(chat.widget().upcast_ref());let card=all.iter().find(|w|w.has_css_class("papo-link-preview")).unwrap();let children=descendants(card);
    assert!(card.width()>0&&card.height()>0);
    let media_box=all.iter().find(|w|w.widget_name()=="message-media").unwrap();
    assert!(card.compute_bounds(media_box).unwrap().x().abs()<=1.0,"rich cards must start at the message's left edge");
    for text in ["<b>Plain title</b>","Card author","Card footer","First","Second"]{assert!(children.iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()==text&&!l.uses_markup()));}
    assert!(!children.iter().filter_map(|w|w.downcast_ref::<gtk::LinkButton>()).any(|b|b.uri().starts_with("javascript:")));
    let grid=children.iter().find_map(|w|w.downcast_ref::<gtk::Grid>()).unwrap();let first=grid.child_at(0,0).unwrap();assert!(descendants(&first).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="First"));assert!(grid.child_at(1,0).is_some());
    // Wide inline fields must remain left aligned when their natural width
    // exceeds the card cap, and still wrap inside a narrow timeline.
    let mut wide=embed.clone();for field in &mut wide.fields{field.value="Rich embed field text ".repeat(20);}
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![wide])));settle(context);
    for width in [850,400]{
        window.set_default_size(width,680);settle(context);let all=descendants(chat.widget().upcast_ref());let card=all.iter().find(|w|w.has_css_class("papo-link-preview")).unwrap();let media_box=all.iter().find(|w|w.widget_name()=="message-media").unwrap();let bounds=card.compute_bounds(media_box).unwrap();
        assert!(bounds.x().abs()<=1.0&&bounds.width()<=media_box.width() as f32+1.0,"wide field cards must fit and align with message text");
    }
    window.set_default_size(850,680);settle(context);
    // A complete list clears cards; no stale single-preview union remains.
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![])));settle(context);assert!(!descendants(chat.widget().upcast_ref()).iter().any(|w|w.has_css_class("papo-link-preview")));
    // Use the actual composer form, not synthetic draft assignment.
    find_button(chat.widget().upcast_ref(),"Embeds personalizados").emit_clicked();pump(context);let editor=chat.model().actions.embed_editor.as_ref().unwrap().0.clone();until(context,||editor.is_mapped());find_button(editor.upcast_ref(),"Adicionar embed").emit_clicked();pump(context);
    let title=descendants(editor.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();title.set_text("Draft embed");find_button(editor.upcast_ref(),"Aplicar embeds").emit_clicked();settle(context);assert_eq!(chat.model().draft.embeds[0].title,"Draft embed");assert!(find_button(chat.widget().upcast_ref(),"Enviar mensagem").is_sensitive());
    let mut sent=message.clone();sent.id=Uuid::new_v4();sent.embeds=Some(vec![]);let request=chat.state().get_mut().model.draft.begin().unwrap();chat.emit(ChatMsg::AddMessage(sent.clone()));pump(context);chat.emit(ChatMsg::ApplyChange(Change::Embeds(sent.id,vec![embed.clone()])));pump(context);
    chat.emit(ChatMsg::SendFinished{request_id:request,channel_id:channel,result:Ok(sent.clone())});settle(context);assert_eq!(chat.model().history.messages.iter().find(|m|m.id==sent.id).unwrap().embeds.as_ref().unwrap().len(),1,"POST's custom-only list must not erase the complete WS list");assert!(chat.model().draft.embeds.is_empty());
    // Preserve session caches for another authorized message sharing this card.
    let mut shared=embed.clone();shared.thumbnail=Some(crate::models::EmbedMedia{mime_type:Some("image/png".into()),..Default::default()});chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![shared.clone()])));chat.emit(ChatMsg::ApplyChange(Change::Embeds(sent.id,vec![shared.clone()])));pump(context);
    let epoch=chat.model().transfers.epoch;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(embed.id,token);chat.emit(ChatMsg::Transfer(transfers::TransferMsg::ImageReady{epoch,message:message.id,key:embed.id,token,result:Some(crate::media::PreparedImage{width:32,height:32,pixels:vec![128;32*32*4]})}));settle(context);
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![])));pump(context);assert!(chat.model().transfers.textures.contains_key(&embed.id));chat.emit(ChatMsg::ApplyChange(Change::Embeds(sent.id,vec![])));pump(context);assert!(!chat.model().transfers.textures.contains_key(&embed.id));
    let mut latest=embed.clone();latest.id=Uuid::new_v4();latest.title=Some("Newer custom card".into());
    chat.emit(ChatMsg::ApplyChange(Change::Edit(sent.id,"Newer edit".into(),Some(created+chrono::Duration::seconds(2)))));
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(sent.id,vec![latest.clone()])));pump(context);
    let mut stale=sent.clone();stale.edited_at=Some(created+chrono::Duration::seconds(1));stale.embeds=Some(vec![embed.clone()]);
    let epoch=chat.model().actions.epoch;chat.emit(ChatMsg::Action(actions::ActionMsg::EditFinished{epoch,token:Uuid::new_v4(),result:Ok(stale)}));settle(context);
    let current=chat.model().history.messages.iter().find(|m|m.id==sent.id).unwrap().clone();assert_eq!(current.content.as_deref(),Some("Newer edit"));assert_eq!(current.embeds,Some(vec![latest]),"a delayed PUT must not replace a newer socket edit's cards");
    window.set_default_size(400,680);settle(context);
    let card=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.has_css_class("papo-link-preview")).unwrap();
    assert!(card.width()>0&&card.width()<=chat.widget().width(),"rich cards must fit a narrow chat");
    for l in descendants(&card).into_iter().filter(|w|w.is::<gtk::Label>()){
        let bounds=l.compute_bounds(&card).unwrap();assert!(bounds.y()+bounds.height()<=card.height() as f32+1.0,"rich text must not be clipped vertically");
    }
    window.set_content(None::<&gtk::Widget>);window.close();
    // Restored custom-media forms must demand replacement URLs instead of
    // dropping their existing stored media when an unrelated field is edited.
    let input=crate::models::EmbedInput{title:"Stored".into(),thumbnail:Some(crate::models::EmbedMediaInput{url:String::new(),mime_type:"image/png".into()}),..Default::default()};let form=embed_editor::Editors::new(vec![input]);assert!(form.read().is_err());find_button(form.root.upcast_ref(),"Remover embed").emit_clicked();assert!(form.read().unwrap().is_empty());
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn youtube_actions_accept_only_the_backend_embed_pattern(){
        assert!(youtube_url("https://www.youtube.com/embed/dQw4w9WgXcQ").is_some());
        for url in ["javascript:alert(1)","https://www.youtube.com.evil.test/embed/dQw4w9WgXcQ","https://user@www.youtube.com/embed/dQw4w9WgXcQ","http://www.youtube.com/embed/dQw4w9WgXcQ","https://www.youtube.com/embed/invalid"]{assert!(youtube_url(url).is_none());}
    }
}
