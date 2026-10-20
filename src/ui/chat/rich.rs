//! EN-US: Render native rich cards from backend embed metadata without executing remote HTML.
//! PT-BR: Renderiza cartões nativos com metadados fornecidos pelo backend, sem executar HTML remoto.
use super::*;
mod flow;

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
fn content(value:&str,users:&HashMap<Uuid,UserSummary>,actions:&Actions,sender:&ComponentSender<ChatModel>)->gtk::Widget{
    if !emoji_details::has_unicode(value)&&!value.contains('@')&&!value.contains(['*','_','`','~','#','>','[','|','\\','\n'])&&!text::parts_index(value,&actions.emoji_names).iter().any(|part|matches!(part,text::Part::Emoji(..))){return label(value).upcast();}
    let input=sender.input_sender().clone();
    let view=text::with_profiles(value,users,actions,move |id|{let _=input.send(ChatMsg::OpenProfile(id));});
    flow::wrap(&view).upcast()
}
/// Include resolved names in the media key so late member metadata and nickname
/// changes refresh reused cards without replacing their surrounding chat row.
pub(super) fn display_signature(p:&Embed,users:&HashMap<Uuid,UserSummary>,actions:&Actions)->u64{
    use std::hash::{Hash,Hasher};let mut hash=std::collections::hash_map::DefaultHasher::new();
    for value in p.description.iter().chain(p.fields.iter().map(|f|&f.value)).chain(p.footer.iter().filter_map(|f|f.text.as_ref())){
        let displayed=mentions::DisplayText::read(value,users);displayed.text.hash(&mut hash);
        for span in displayed.spans{span.start.hash(&mut hash);span.end.hash(&mut hash);span.id.hash(&mut hash);}
        for part in text::parts_index(value,&actions.emoji_names){if let text::Part::Emoji(id,_)=part{id.hash(&mut hash);actions.emoji_paintable(id).map(|p|p.as_ptr() as usize).hash(&mut hash);}}
    }
    hash.finish()
}
pub(super) fn body(p:&Embed,author_texture:Option<&gtk::gdk::Texture>,users:&HashMap<Uuid,UserSummary>,actions:&Actions,sender:&ComponentSender<ChatModel>)->gtk::Box{
    let card=gtk::Box::new(gtk::Orientation::Vertical,6);card.set_widget_name(&format!("embed-{}",p.id));card.set_valign(gtk::Align::Start);card.set_overflow(gtk::Overflow::Hidden);card.set_margin_bottom(8);card.add_css_class("card");card.add_css_class("papo-link-preview");
    if p.source_type=="custom"{card.add_css_class("papo-custom-embed");}
    crate::ui::style::embed_accent(&card,p.color.as_deref());
    if let Some(provider)=p.site_name.as_deref().or(p.provider.as_deref()){let l=label(provider);l.add_css_class("caption");l.add_css_class("dim-label");card.append(&l);}
    if let Some(author)=&p.author{
        let header=gtk::Box::new(gtk::Orientation::Horizontal,6);header.set_valign(gtk::Align::Start);
        if author.media.is_some()||author_texture.is_some(){
            let avatar=crate::media::avatar_image(author_texture,24);avatar.set_widget_name(&format!("embed-author-{}",p.id));avatar.set_valign(gtk::Align::Center);header.append(&avatar);
        }
        if let Some(name)=author.name.as_deref(){let w=linked(name,author.url.as_deref());w.set_valign(gtk::Align::Center);w.add_css_class("heading");header.append(&w);}
        if header.first_child().is_some(){card.append(&header);}
    }
    if let Some(title)=p.title.as_deref().filter(|s|!s.is_empty()).or(p.url.as_deref()) {let w=linked(title,p.url.as_deref());w.add_css_class("heading");card.append(&w);}
    if let Some(description)=p.description.as_deref(){card.append(&content(description,users,actions,sender));}
    let grid=gtk::Grid::new();grid.set_column_spacing(12);grid.set_row_spacing(8);grid.set_column_homogeneous(true);grid.set_hexpand(true);
    let mut fields:Vec<_>=p.fields.iter().collect();fields.sort_by_key(|f|f.position);let(mut row,mut col)=(0,0);
    for field in fields{
        if !field.inline&&col!=0{row+=1;col=0;}
        let group=gtk::Box::new(gtk::Orientation::Vertical,3);group.set_hexpand(true);let name=label(&field.name);name.add_css_class("heading");group.append(&name);group.append(&content(&field.value,users,actions,sender));
        grid.attach(&group,col,row,if field.inline{1}else{2},1);
        if field.inline&&col==0{col=1;}else{row+=1;col=0;}
    }
    if !p.fields.is_empty(){card.append(&grid);}
    card
}
pub(super) fn footer(card:&gtk::Box,p:&Embed,image_available:bool,users:&HashMap<Uuid,UserSummary>,actions:&Actions,sender:&ComponentSender<ChatModel>){
    // Full-image and footer-icon bytes are not exposed by the current API.
    // Keep their metadata; don't bypass channel authorization
    // by downloading their original URLs or content hashes.
    if p.image.is_some()&&!image_available{let l=label("Imagem incorporada indisponível no servidor.");l.add_css_class("dim-label");card.append(&l);}
    if let Some(text)=p.footer.as_ref().and_then(|f|f.text.as_deref()){let l=content(text,users,actions,sender);l.add_css_class("caption");l.add_css_class("dim-label");card.append(&l);}
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
    let mut quote=embed.clone();quote.source_type="link".into();quote.provider=Some("X".into());
    quote.title=Some("Author (@author)".into());quote.description=Some("Comment on the quoted post".into());
    quote.fields=vec![crate::models::EmbedField{position:0,name:"Quoted author (@quoted)".into(),value:"The quoted text 🌎".into(),inline:false}];
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![quote])));settle(context);
    let all=descendants(chat.widget().upcast_ref());
    for value in ["Author (@author)","Comment on the quoted post","Quoted author (@quoted)","The quoted text 🌎"]{
        assert!(all.iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()==value)
            ||w.downcast_ref::<gtk::TextView>().is_some_and(|view|{let buffer=view.buffer();buffer.text(&buffer.start_iter(),&buffer.end_iter(),false)==value})),"link fields must render quoted tweet metadata: {value}");
    }
    // A complete list clears cards; no stale single-preview union remains.
    chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![])));settle(context);assert!(!descendants(chat.widget().upcast_ref()).iter().any(|w|w.has_css_class("papo-link-preview")));
    // Use the actual composer form, not synthetic draft assignment.
    find_button(chat.widget().upcast_ref(),"Embeds personalizados").emit_clicked();pump(context);let editor=chat.model().actions.embed_editor.as_ref().unwrap().0.clone();until(context,||editor.is_mapped());find_button(editor.upcast_ref(),"Adicionar embed").emit_clicked();pump(context);
    let title=descendants(editor.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();title.set_text("Draft embed");find_button(editor.upcast_ref(),"Aplicar embeds").emit_clicked();settle(context);assert_eq!(chat.model().draft.embeds[0].title,"Draft embed");assert!(find_button(chat.widget().upcast_ref(),"Enviar mensagem").is_sensitive());
    let mut sent=message.clone();sent.id=Uuid::new_v4();sent.embeds=Some(vec![]);let request=chat.state().get_mut().model.draft.begin().unwrap();chat.emit(ChatMsg::AddMessage(sent.clone()));pump(context);chat.emit(ChatMsg::ApplyChange(Change::Embeds(sent.id,vec![embed.clone()])));pump(context);
    chat.emit(ChatMsg::SendFinished{request_id:request,channel_id:channel,result:Ok(sent.clone())});settle(context);assert_eq!(chat.model().history.messages.iter().find(|m|m.id==sent.id).unwrap().embeds.as_ref().unwrap().len(),1,"POST's custom-only list must not erase the complete WS list");assert!(chat.model().draft.embeds.is_empty());
    // Preserve session caches for another authorized message sharing this card.
    let mut shared=embed.clone();shared.thumbnail=Some(crate::models::EmbedMedia{mime_type:Some("image/png".into()),..Default::default()});chat.emit(ChatMsg::ApplyChange(Change::Embeds(message.id,vec![shared.clone()])));chat.emit(ChatMsg::ApplyChange(Change::Embeds(sent.id,vec![shared.clone()])));pump(context);
    let epoch=chat.model().transfers.epoch;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(embed.id,token);chat.emit(ChatMsg::Transfer(transfers::TransferMsg::ImageReady{epoch,key:embed.id,token,result:Some(crate::media::PreparedImage{width:32,height:32,pixels:vec![128;32*32*4]})}));settle(context);
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
    exercise_accent_channels(context);
    exercise_text_width(context);
    super::transfers::author_exercise(context);
    exercise_mentions(context);
}

#[cfg(test)]fn exercise_text_width(context:&gtk::glib::MainContext){
    use adw::prelude::*;use actions::tests::{descendants,pump};use performance::settle;
    let peer=Uuid::new_v4();let now=chrono::Utc::now();let users=HashMap::from([(peer,serde_json::from_value(serde_json::json!({"id":peer,"username":"bob","nickname":"Bob","created_at":now})).unwrap())]);
    // The sentence must fit the wide card but exceed the toolbar-constrained
    // minimum window width; a shorter fixture can still fit after resizing.
    let source=format!("Uma **mensagem** para @mention(<@{peer}>) sem quebra inesperada na frase.");
    let expected="Uma mensagem para @Bob sem quebra inesperada na frase.";
    let channel=Uuid::new_v4();let user=Uuid::new_v4();
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"Bot text width","created_at":now})).unwrap();
    let embed:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","author":{"name":"a"},"description":source,"created_at":now})).unwrap();
    let mut message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"created_at":now})).unwrap();message.embeds=Some(vec![embed]);
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![message],user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});chat.emit(ChatMsg::SetUsers(users.into_values().collect()));
    let window=adw::Window::builder().default_width(850).default_height(600).content(chat.widget()).build();window.present();settle(context);
    let card=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.has_css_class("papo-custom-embed")).unwrap();
    let view=descendants(card.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok()).unwrap();let buffer=view.buffer();
    assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false),expected);
    assert_eq!(view.iter_location(&buffer.start_iter()).y(),view.iter_location(&buffer.end_iter()).y(),"a fitting bot sentence must not wrap in a wide chat: card={}, text={}",card.width(),view.width());
    window.set_default_size(260,600);settle(context);
    assert!(view.width()<=chat.widget().width());assert!(view.iter_location(&buffer.end_iter()).y()>view.iter_location(&buffer.start_iter()).y(),"narrow cards still wrap: window={}, chat={}, text={}",window.width(),chat.widget().width(),view.width());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

#[cfg(test)]fn exercise_mentions(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use actions::tests::{descendants,pump,until};use std::{rc::Rc,cell::RefCell};
    let user=Uuid::new_v4();let peer=Uuid::new_v4();let channel=Uuid::new_v4();let message=Uuid::new_v4();let embed=Uuid::new_v4();let now=chrono::Utc::now();
    let token=format!("@mention(<@{peer}>)");
    let target=serde_json::from_value(serde_json::json!({"id":channel,"name":"bridge mentions","created_at":now})).unwrap();
    let msg=serde_json::from_value(serde_json::json!({"id":message,"channel_id":channel,"author_id":user,"created_at":now,"embeds":[{"id":embed,"source_type":"custom","fetch_method":"manual","description":format!("é review {token}"),"fields":[{"name":"Reply","value":token}],"footer":{"text":token},"created_at":now}]})).unwrap();
    let opened=Rc::new(RefCell::new(Vec::new()));let profiles=opened.clone();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:vec![msg],user_id:Some(user),..Default::default()}).connect_receiver(move |_,output|{if let ChatOutput::OpenProfile(id)=output{profiles.borrow_mut().push(id);}});
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(680).content(chat.widget()).build();window.present();performance::settle(context);
    let row=chat.model().rendered[&format!("message-{message}")].1.clone();
    let descriptions=||descendants(row.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::TextView>().ok()).collect::<Vec<_>>();
    assert_eq!(descriptions().len(),3);assert!(descriptions().iter().all(|v|!v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false).contains(&peer.to_string())),"embed mentions must never expose IDs while names load");
    let member=|name:&str|serde_json::from_value::<UserSummary>(serde_json::json!({"id":peer,"username":"bob","nickname":name,"created_at":now})).unwrap();
    for name in ["ISpeakHue","<b>Íris & x</b>"]{
        chat.emit(ChatMsg::SetUsers(vec![member(name)]));performance::settle(context);
        assert_eq!(chat.model().rendered[&format!("message-{message}")].1,row,"late member data updates the card without replacing its chat row");
        let views=descriptions();assert_eq!(views.len(),3);
        for view in &views{let buffer=view.buffer();let value=buffer.text(&buffer.start_iter(),&buffer.end_iter(),false);assert!(value.contains(&format!("@{name}")));assert!(!value.contains("@mention("));let tag=buffer.tag_table().lookup("papo-mention").unwrap();assert!(buffer.iter_at_offset(value[..value.find('@').unwrap()].chars().count() as i32).has_tag(&tag));}
        let view=views.iter().find(|v|v.buffer().text(&v.buffer().start_iter(),&v.buffer().end_iter(),false).starts_with("é review")).unwrap();
        let buffer=view.buffer();let rect=view.iter_location(&buffer.iter_at_offset(10));let(x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x()+1,rect.y()+1);
        let controllers=view.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>()).unwrap();
        click.emit_by_name::<()>("pressed",&[&1i32,&f64::from(x),&f64::from(y)]);click.emit_by_name::<()>("released",&[&1i32,&f64::from(x),&f64::from(y)]);until(context,||opened.borrow().last()==Some(&peer));
        let count=opened.borrow().len();buffer.select_range(&buffer.start_iter(),&buffer.end_iter());click.emit_by_name::<()>("pressed",&[&1i32,&f64::from(x),&f64::from(y)]);click.emit_by_name::<()>("released",&[&1i32,&f64::from(x),&f64::from(y)]);pump(context);assert_eq!(opened.borrow().len(),count,"selecting bridge text must not open profiles");
    }
    assert_eq!(opened.borrow().len(),2);
    window.set_default_size(380,680);performance::settle(context);for view in descriptions(){assert!(view.width()<=row.width(),"mentions must wrap inside narrow cards");}
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn youtube_actions_accept_only_the_backend_embed_pattern(){
        assert!(youtube_url("https://www.youtube.com/embed/dQw4w9WgXcQ").is_some());
        for url in ["javascript:alert(1)","https://www.youtube.com.evil.test/embed/dQw4w9WgXcQ","https://user@www.youtube.com/embed/dQw4w9WgXcQ","http://www.youtube.com/embed/dQw4w9WgXcQ","https://www.youtube.com/embed/invalid"]{assert!(youtube_url(url).is_none());}
    }
}

#[cfg(test)]
pub(crate) fn exercise_accent_channels(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,pump,until};
    use adw::prelude::*;
    let user=Uuid::new_v4();let a=Uuid::new_v4();let b=Uuid::new_v4();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"Palette cache","created_at":chrono::Utc::now()})).unwrap();
    let first=Uuid::new_v4();let second=Uuid::new_v4();
    let message=|id,embed,color:&str|serde_json::from_value::<Message>(serde_json::json!({"id":Uuid::new_v4(),"channel_id":id,"author_id":user,"content":"Cached colored card","created_at":chrono::Utc::now(),"embeds":[{"id":embed,"source_type":"custom","fetch_method":"manual","title":"Color","color":color,"created_at":chrono::Utc::now()}]})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a)),messages:vec![message(a,first,"#123abc")],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(800).default_height(600).content(chat.widget()).build();window.present();
    let assert_color=|id,pixel|{
        let find=||descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("embed-{id}")&&w.width()>0);
        until(context,||find().is_some());performance::settle(context);
        assert_eq!(crate::ui::style::sample_accent(&find().unwrap(),window.upcast_ref()),pixel);
    };
    assert_color(first,[18,58,188,255]);
    chat.emit(ChatMsg::SetChannel(channel(b)));pump(context);
    let request=Uuid::new_v4();
    chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});
    chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:b,messages:vec![message(b,second,"#d32f2f")],has_more:false})});
    assert_color(second,[211,47,47,255]);
    for _ in 0..3{
        chat.emit(ChatMsg::SetChannel(channel(a)));assert_color(first,[18,58,188,255]);
        chat.emit(ChatMsg::SetChannel(channel(b)));assert_color(second,[211,47,47,255]);
    }
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}
