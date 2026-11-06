//! EN-US: Compact channel titles with the same cached emoji paintables as messages.
//! PT-BR: Títulos compactos dos canais com os mesmos paintables de emojis em cache das mensagens.
use super::*;

#[derive(Default)]
pub(super) struct Header {name:Option<Line>,topic:Option<Line>}
#[derive(PartialEq)]
struct Signature {text:String,images:Vec<(Uuid,gtk::gdk::Paintable)>}
struct Line {signature:Signature,widget:gtk::Widget}

impl Header {
pub fn render(&mut self,container:&gtk::Box,target:Option<&ConversationTarget>,actions:&Actions){
    let name=match target{
        Some(c) if c.is_direct()=>c.name().to_owned(),
        Some(c) if c.is_voice()=>crate::i18n::fmt("Voz · {}", &[("0", format!("{}", c.name()))]),
        Some(c)=>format!("# {}",c.name()),
        None=>crate::i18n::tr("Selecione um canal").into(),
    };
    let topic=target.and_then(|c|c.topic()).unwrap_or("");
    container.set_tooltip_text(Some(topic));
    update_line(container,&mut self.name,Some(&name),actions,"papo-conversation-name",None);
    let after=self.name.as_ref().map(|line|line.widget.clone());
    update_line(container,&mut self.topic,(!topic.is_empty()).then_some(topic),actions,"papo-conversation-topic",after.as_ref());
}
}

fn update_line(container:&gtk::Box,current:&mut Option<Line>,value:Option<&str>,actions:&Actions,class:&str,after:Option<&gtk::Widget>){
    let Some(value)=value else{
        if let Some(old)=current.take(){container.remove(&old.widget);}
        return;
    };
    // Compare only paintables actually used by this line. Catalog pagination,
    // stale responses and animation frames must not remap an unchanged topic.
    let images=text::parts_index(&text::excerpt(value),&actions.emoji_names).into_iter().filter_map(|part|match part{
        text::Part::Emoji(id,_)=>actions.emoji_paintable(id).map(|image|(id,image)),_=>None,
    }).collect();
    let signature=Signature{text:value.into(),images};
    if current.as_ref().is_some_and(|line|line.signature==signature){return;}
    let widget=line(value,actions,class);
    container.insert_child_after(&widget,after);
    if let Some(old)=current.take(){container.remove(&old.widget);}
    *current=Some(Line{signature,widget});
}

fn line(value:&str,actions:&Actions,class:&str)->gtk::Widget{
    let text=text::excerpt(value);
    let parts=text::parts_index(&text,&actions.emoji_names);
    if !parts.iter().any(|p|matches!(p,text::Part::Emoji(..))){
        let label=gtk::Label::new(Some(&text));label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_hexpand(true);label.add_css_class(class);label.set_tooltip_text(Some(value));return label.upcast();
    }
    // EN-US: Clip a single rich line instead of making a long topic widen or wrap the header.
    // PT-BR: Recorta uma linha rica, sem deixar um tópico longo alargar ou quebrar o cabeçalho.
    let view=gtk::TextView::new();view.set_editable(false);view.set_cursor_visible(false);
    view.set_focusable(false);view.set_can_target(false);view.set_wrap_mode(gtk::WrapMode::None);
    view.set_justification(gtk::Justification::Center);view.add_css_class("papo-conversation-text");view.add_css_class(class);
    #[cfg(windows)]view.set_im_module(Some("simple"));
    let buffer=view.buffer();let mut iter=buffer.start_iter();
    let metrics=view.pango_context().metrics(None,None);
    let height=((metrics.ascent()+metrics.descent()+pango::SCALE-1)/pango::SCALE).max(1);
    for part in parts{match part{
        text::Part::Text(value)=>buffer.insert(&mut iter,&value),
        text::Part::Emoji(id,name)=>{
            if let Some(paintable)=actions.emoji_paintable(id){
                let offset=iter.offset();buffer.insert_paintable(&mut iter,&crate::media::inline::fit(&paintable,height));
                if let Some(animation)=actions.animations.get(&id){animation.watch(&view,Some(offset));}
            }else{buffer.insert(&mut iter,&format!(":{name}:"));}
        }
    }}
    view.update_property(&[gtk::accessible::Property::Label(value)]);
    let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::External).height_request(height).hexpand(true).child(&view).build();
    scroll.set_can_target(false);scroll.set_tooltip_text(Some(value));
    // EN-US: Font metrics settle after parenting; track native layout so image baselines cannot be clipped.
    // PT-BR: As métricas da fonte se estabilizam após anexar; acompanha o layout para não recortar as imagens.
    let weak=scroll.downgrade();let text=view.downgrade();
    let fit=std::rc::Rc::new(move ||{
        if let (Some(scroll),Some(view))=(weak.upgrade(),text.upgrade()){
            let (_,height)=view.line_yrange(&view.buffer().start_iter());let height=height.max(1);
            if view.height_request()!=height{view.set_height_request(height);}
            if scroll.height_request()!=height{scroll.set_height_request(height);}
        }
    });
    let observer=gtk::WidgetPaintable::new(Some(&view));let resize=fit.clone();observer.connect_invalidate_size(move |_|resize());
    view.connect_map(move |view|{observer.set_widget(Some(view));fit();});
    scroll.upcast()
}

#[cfg(test)]pub(super) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,descendants,pump,until};use base64::Engine;use adw::prelude::*;
    let date="2026-10-03T00:00:00Z";
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let image=Uuid::new_v4();let animated=Uuid::new_v4();
    let mut png=std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(16,16,image::Rgba([53,132,228,255]))).write_to(&mut png,image::ImageFormat::Png).unwrap();
    backend.lock().unwrap().emojis=vec![
        serde_json::json!({"id":image,"name":"cat","format":"PNG","image_blob":base64::engine::general_purpose::STANDARD.encode(png.into_inner()),"created_at":date}),
        serde_json::json!({"id":animated,"name":"dance","format":"GIF","image_blob":base64::engine::general_purpose::STANDARD.encode(crate::media::animation::fixture()),"created_at":date})
    ];
    let mut channel:Channel=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"Olá :cat:","topic":format!("Tópico :dance: :unknown: 🐈 <literal> {}","Detalhes da sala. ".repeat(25)),"type":"text","created_at":date})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel.clone()),api:Some(api),..Default::default()}).detach();
    // EN-US: Exercise the real adaptive header, including its compact action group at narrow widths.
    // PT-BR: Exercita o cabeçalho adaptativo real, incluindo as ações compactas em larguras pequenas.
    let members=adw::OverlaySplitView::new();members.set_collapsed(true);members.set_show_sidebar(false);
    members.set_content(Some(chat.widget()));members.set_sidebar(Some(&gtk::Box::new(gtk::Orientation::Vertical,0)));
    let toolbar=adw::ToolbarView::new();let header=ChatModel::take_header(&chat);toolbar.add_top_bar(&header);toolbar.set_content(Some(&members));
    let bin=adw::BreakpointBin::new();bin.set_size_request(320,240);bin.set_child(Some(&toolbar));
    let (settings,handler)=ChatModel::adapt_header(&chat,&bin,&members);
    let window=adw::Window::builder().default_width(900).default_height(600).content(&bin).build();window.present();
    let title=chat.widgets().conversation_title.clone();
    assert_eq!(title.first_child().and_downcast::<gtk::Label>().unwrap().text(),"# Olá :cat:");
    chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));
    until(context,||chat.model().actions.animations.contains_key(&animated)&&chat.model().actions.textures.contains_key(&image)&&chat.model().actions.emojis_request.is_none());
    super::performance::settle(context);
    let lines:Vec<_>=descendants(title.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::TextView>().ok()).collect();
    assert_eq!(lines.len(),2,"both the channel name and description render inline emojis");
    let pixels=|line:&gtk::TextView|{let buffer=line.buffer();(0..buffer.char_count()).filter_map(|offset|buffer.iter_at_offset(offset).paintable()).collect::<Vec<_>>()};
    assert_eq!(pixels(&lines[0]).len(),1);let emoji=pixels(&lines[1]);assert_eq!(emoji.len(),1);
    assert!(!emoji[0].flags().contains(gtk::gdk::PaintableFlags::CONTENTS),"the header keeps live animation rather than a frozen texture");
    let buffer=lines[1].buffer();let text=buffer.text(&buffer.start_iter(),&buffer.end_iter(),true);
    assert!(text.contains(":unknown: 🐈 <literal>"));assert!(!text.contains(":dance:"));
    let first=title.first_child().unwrap();chat.emit(ChatMsg::UpdateChannel(channel.clone()));pump(context);
    assert_eq!(title.first_child().unwrap(),first,"history metadata updates do not recreate header widgets");
    let topic=title.last_child().unwrap();let unmaps=std::rc::Rc::new(std::cell::Cell::new(0));let seen=unmaps.clone();
    topic.connect_unmap(move|_|seen.set(seen.get()+1));
    let requests=||backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="GET"&&path.starts_with("/emojis")).count();
    for unrelated in [false,true]{
        if unrelated{let mut backend=backend.lock().unwrap();let blob=backend.emojis[0]["image_blob"].clone();backend.emojis.push(serde_json::json!({"id":Uuid::new_v4(),"name":"unrelated","format":"PNG","image_blob":blob,"created_at":date}));}
        let before=requests();chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));
        until(context,||requests()>before&&chat.model().actions.emojis_request.is_none());
        assert_eq!(title.first_child().unwrap(),first,"catalog refreshes must preserve the unchanged name");
        assert_eq!(title.last_child().unwrap(),topic,"unchanged and unrelated emoji responses must preserve the topic");
        assert_eq!(unmaps.get(),0,"catalog updates cannot unmap a painted topic");
        assert_eq!(pixels(&lines[1]),emoji,"animated inline paintables keep their identity and playback");
    }
    crate::ui::main_window::layout::tests::preview(&window,"channel-header-emojis-wide",context);
    for width in [900,360]{
        actions::tests::resize_window(&window,width,600);super::performance::settle(context);
        assert!(title.height()<60,"emoji titles stay compact");assert!(window.width()<=width+2,"rich title at {width}px: window {}, header minimum {:?}",window.width(),header.measure(gtk::Orientation::Horizontal,-1));
        let bounds=title.compute_bounds(&header).unwrap();let chat_bounds=chat.widget().compute_bounds(&header).unwrap();
        assert!((bounds.x()+bounds.width()/2.0-chat_bounds.x()-chat_bounds.width()/2.0).abs()<=1.0,"rich title stays centered above the chat");
        for line in &lines{assert_eq!(line.wrap_mode(),gtk::WrapMode::None);let(_,height)=line.line_yrange(&line.buffer().start_iter());assert!(height<=line.height(),"the inline image and text need {height}px but were allocated {}px",line.height());}
    }
    crate::ui::main_window::layout::tests::preview(&window,"channel-header-emojis",context);
    // Switch away and return at an unread boundary, then complete history and
    // emoji refreshes while the new header is already visible.
    let messages:Vec<Message>=(0..20).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel.id,"created_at":chrono::DateTime::from_timestamp(1700000000+i,0),"content":format!("Unread header fixture {i}")})).unwrap()).collect();
    let history=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:history,cursor:None});
    chat.emit(ChatMsg::HistoryLoaded{request_id:history,append:false,result:Ok(MessageListResponse{channel_id:channel.id,has_more:false,messages:messages.clone()})});pump(context);
    chat.emit(ChatMsg::SetAccess{user_id:actions::tests::USER.parse().unwrap(),access:crate::models::Access::direct()});pump(context);
    let mut other=channel.clone();other.id=Uuid::new_v4();other.name="Other channel".into();other.topic=None;
    chat.emit(ChatMsg::SetChannel(other));pump(context);
    channel.last_read_message=Some(messages[9].id);channel.last_message=Some(crate::models::channel::ChannelLastMessage{id:messages[19].id,created_at:messages[19].created_at,content:None,author_id:None,author_username:None});
    chat.emit(ChatMsg::SetChannel(channel.clone()));super::performance::settle(context);
    assert_eq!(chat.model().unread_start,Some(messages[10].id),"the returning chat opens at its unread boundary");
    let current_topic=title.last_child().unwrap();let unmaps=std::rc::Rc::new(std::cell::Cell::new(0));let seen=unmaps.clone();current_topic.connect_unmap(move|_|seen.set(seen.get()+1));
    let history=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:history,cursor:None});
    chat.emit(ChatMsg::HistoryLoaded{request_id:history,append:false,result:Ok(MessageListResponse{channel_id:channel.id,has_more:false,messages})});
    let before=requests();chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));
    until(context,||requests()>before&&chat.model().actions.emojis_request.is_none());super::performance::settle(context);
    assert_eq!(title.last_child().unwrap(),current_topic,"unread history and emoji completions must keep the mapped description");assert_eq!(unmaps.get(),0);
    channel.name="Renamed channel".into();chat.emit(ChatMsg::UpdateChannel(channel.clone()));pump(context);
    assert_eq!(title.last_child().unwrap(),current_topic,"renaming the channel must not rebuild its description");
    channel.name="Another channel".into();channel.topic=None;chat.emit(ChatMsg::SetChannel(channel));pump(context);
    assert!(descendants(title.upcast_ref()).iter().all(|w|!w.is::<gtk::TextView>()),"switching channels removes previous header emojis");
    chat.emit(ChatMsg::ClearChannel);pump(context);assert_eq!(title.first_child().and_downcast::<gtk::Label>().unwrap().text(),"Selecione um canal");
    settings.disconnect(handler);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
