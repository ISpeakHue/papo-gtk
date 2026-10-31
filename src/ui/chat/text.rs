//! EN-US: Native, read-only message text with inline custom emoji paintables.
//! PT-BR: Texto nativo de mensagens, somente leitura, com paintables de emojis personalizados embutidos.
use super::*;
#[derive(Debug,PartialEq,Eq)]
pub(super) enum Part { Text(String), Emoji(Uuid,String) }

#[cfg(test)]
pub(super) fn parts(text:&str,emojis:&[crate::models::Emoji])->Vec<Part>{
    parts_index(text,&emojis.iter().map(|e|(e.name.clone(),e.id)).collect())
}
pub(super) fn parts_index(text:&str,names:&HashMap<String,Uuid>)->Vec<Part>{
    if names.is_empty()||!text.contains(':'){return if text.is_empty(){vec![]}else{vec![Part::Text(text.into())]};}
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
    build(text,users,actions,None,false)
}
pub(super) fn with_profiles(text:&str,users:&HashMap<Uuid,UserSummary>,actions:&Actions,open:impl Fn(Uuid)+'static)->gtk::TextView{
    build(text,users,actions,Some(std::rc::Rc::new(open)),false)
}
#[derive(Clone)]enum Target {Link(String),Profile(Uuid),Emoji{offset:i32,details:emoji_details::Details},Spoiler{tag:gtk::TextTag,shown:std::rc::Rc<std::cell::Cell<bool>>,images:std::rc::Rc<std::cell::RefCell<Vec<(i32,gtk::gdk::Paintable,Option<crate::media::animation::Animation>)>>>}}
fn build(text:&str,users:&HashMap<Uuid,UserSummary>,actions:&Actions,open:Option<std::rc::Rc<dyn Fn(Uuid)>>,compact:bool)->gtk::TextView{
    let view=gtk::TextView::new();view.set_editable(false);view.set_cursor_visible(false);view.set_accepts_tab(false);
    // Win32's IME context tracks its candidate-window position after every
    // layout, including for unfocused, read-only views. Hundreds of timeline
    // contexts turn each scroll frame into native IME calls. Display text
    // never accepts composition; editable composers retain the native IME.
    #[cfg(windows)] view.set_im_module(Some("simple"));
    view.set_wrap_mode(gtk::WrapMode::WordChar);view.set_vexpand(false);
    view.add_css_class("papo-message-text");
    // GtkTextView validates its lazy layout inside the allocated viewport. A
    // grouped, newly appended row otherwise gives it zero pixels, so validation
    // and its natural-height update wait until the next scroll/input event.
    let metrics=view.pango_context().metrics(None,None);
    let emoji_size=compact.then(||((metrics.ascent()+metrics.descent()+pango::SCALE-1)/pango::SCALE).max(1));
    view.set_height_request(((metrics.ascent()+metrics.descent()+pango::SCALE-1)/pango::SCALE).max(1));
    // WidgetPaintable observes allocation changes, including width changes on
    // resize. GtkTextView's own layout manager bypasses a size_allocate override.
    // Keep only a native size observer, rather than a per-row frame timer.
    let observer=gtk::WidgetPaintable::new(Some(&view));let weak=view.downgrade();
    observer.connect_invalidate_size(move |_|{if let Some(view)=weak.upgrade(){size_to_content(&view);}});
    view.connect_map(move |view|{observer.set_widget(Some(view));size_to_content(view);});
    let buffer=view.buffer();let mut iter=buffer.start_iter();let mut targets=vec![];
    let tag=gtk::TextTag::builder().foreground("#3584e4").underline(pango::Underline::Single).build();buffer.tag_table().add(&tag);
    let mention=gtk::TextTag::builder().name("papo-mention").background("#3584e4").foreground("#ffffff").build();buffer.tag_table().add(&mention);
    let mut spoilers=HashMap::<usize,Target>::new();
    let mut tags=HashMap::<markdown::Style,gtk::TextTag>::new();let mut registered=std::collections::HashSet::new();let mut code_blocks=Vec::new();
    let shade=if adw::StyleManager::default().is_dark(){"#29292d"}else{"#dddde1"};
    for run in markdown::parse(text){
        let begin=iter.offset();let mut hidden_images=Vec::new();
        let parts=if run.style.code{vec![Part::Text(run.text.clone())]}else{parts_index(&run.text,&actions.emoji_names)};
        for part in parts{match part{
            Part::Text(value)=>{
                let displayed=if run.style.code{mentions::DisplayText{text:value,..Default::default()}}else{mentions::DisplayText::read(&value,users)};
                let value=&displayed.text;let offset=iter.offset();buffer.insert(&mut iter,value);
                for span in displayed.spans{
                    let start=offset+value[..span.start].chars().count() as i32;let end=offset+value[..span.end].chars().count() as i32;
                    buffer.apply_tag(&mention,&buffer.iter_at_offset(start),&buffer.iter_at_offset(end));
                    if let Some(id)=span.id.filter(|_|open.is_some()){targets.push((start,end,Target::Profile(id)));}
                }
                if !run.style.code&&run.style.link.is_none(){
                    for(start,end,url) in links(value){let start=offset+start;let end=offset+end;buffer.apply_tag(&tag,&buffer.iter_at_offset(start),&buffer.iter_at_offset(end));targets.push((start,end,Target::Link(url)));}
                    for(start,end,details) in emoji_details::unicode(value){targets.push((offset+start,offset+end,Target::Emoji{offset:offset+start,details}));}
                }
            },
            Part::Emoji(id,name)=>{
                let start=iter.offset();let details=emoji_details::Details::custom(id,name.clone(),actions);
                if let Some(paintable)=actions.emoji_paintable(id){
                    let paintable=emoji_size.map_or_else(||paintable.clone(),|size|crate::media::inline::fit(&paintable,size));
                    let offset=iter.offset();let animation=actions.animations.get(&id);
                    if run.style.spoiler{
                        let(w,h)=(paintable.intrinsic_width().clamp(1,128),paintable.intrinsic_height().clamp(1,128));let color=gtk::gdk::RGBA::parse(shade).unwrap();
                        let mut pixels=Vec::with_capacity((w*h*4) as usize);for _ in 0..w*h{pixels.extend_from_slice(&[(color.red()*255.0) as u8,(color.green()*255.0) as u8,(color.blue()*255.0) as u8,255]);}
                        let mask=gtk::gdk::MemoryTexture::new(w,h,gtk::gdk::MemoryFormat::R8g8b8a8,&gtk::glib::Bytes::from_owned(pixels),w as usize*4);
                        buffer.insert_paintable(&mut iter,&mask);hidden_images.push((offset,paintable,animation.cloned()));
                    }else{buffer.insert_paintable(&mut iter,&paintable);if let Some(animation)=animation{animation.watch(&view,Some(offset));}}
                }else{buffer.insert(&mut iter,&format!(":{name}:"));}
                if run.style.link.is_none(){targets.push((start,iter.offset(),Target::Emoji{offset:start,details}));}
            },
        }}
        let end=iter.offset();
        if let Some(language)=run.style.code_language.as_ref(){code_blocks.push(super::syntax::Block{language:language.clone(),text:run.text.clone(),start:begin});}
        if let Some(url)=&run.style.link{buffer.apply_tag(&tag,&buffer.iter_at_offset(begin),&buffer.iter_at_offset(end));targets.push((begin,end,Target::Link(url.clone())));}
        let mut style=run.style.clone();style.spoiler=false;style.spoiler_group=0;
        let style_tag=tags.entry(style.clone()).or_insert_with(||markdown_tag(&style,shade)).clone();
        if registered.insert(style_tag.as_ptr()){buffer.tag_table().add(&style_tag);}
        buffer.apply_tag(&style_tag,&buffer.iter_at_offset(begin),&buffer.iter_at_offset(end));
        if run.style.spoiler{
            let spoiler=spoilers.entry(run.style.spoiler_group).or_insert_with(||{
                let tag=markdown_tag(&markdown::Style{spoiler:true,..Default::default()},shade);buffer.tag_table().add(&tag);
                Target::Spoiler{tag,shown:Default::default(),images:Default::default()}
            });
            if let Target::Spoiler{tag,images,..}=spoiler{tag.set_priority(buffer.tag_table().size()-1);buffer.apply_tag(tag,&buffer.iter_at_offset(begin),&buffer.iter_at_offset(end));images.borrow_mut().extend(hidden_images);}
            targets.push((begin,end,spoiler.clone()));
        }
    }
    if !code_blocks.is_empty(){super::syntax::attach(&view,code_blocks,tags.iter().filter(|(style,_)|style.code).map(|(_,tag)|tag.clone()).collect());}
    let emoji_popup=emoji_details::connect(&view);
    install_targets(&view,targets,move |view,target|match target{
        Target::Emoji{offset,details}=>emoji_details::show(view,*offset,details,&emoji_popup),
        Target::Spoiler{tag,shown,images}=>{
            shown.set(true);let buffer=view.buffer();buffer.remove_tag(tag,&buffer.start_iter(),&buffer.end_iter());
            for (offset,paintable,animation) in images.borrow().iter(){let mut start=buffer.iter_at_offset(*offset);let mut end=start;end.forward_char();buffer.delete(&mut start,&mut end);buffer.insert_paintable(&mut start,paintable);if let Some(animation)=animation{animation.watch(view,Some(*offset));}}
            size_to_content(view);
        },
        Target::Profile(id)=>{if let Some(open)=&open{open(*id);}},
        Target::Link(url)=>{let launcher=gtk::UriLauncher::new(url);let parent=view.root().and_downcast::<gtk::Window>();launcher.launch(parent.as_ref(),None::<&gtk::gio::Cancellable>,|result|{if let Err(error)=result{tracing::warn!(kind = ?error.kind::<gtk::gio::IOErrorEnum>(), "Could not open chat link");}});},
    });
    // Keep a textual description available even when a custom image is displayed.
    if spoilers.is_empty(){view.set_tooltip_text(Some(&mentions::render(text,users)));}view
}

fn markdown_tag(style:&markdown::Style,shade:&str)->gtk::TextTag{
    let tag=gtk::TextTag::new(None);
    if style.bold{tag.set_weight(700);}if style.italic{tag.set_style(pango::Style::Italic);}if style.underline{tag.set_underline(pango::Underline::Single);}if style.strike{tag.set_strikethrough(true);}
    if style.heading>0{tag.set_scale(match style.heading{1=>1.8,2=>1.6,3=>1.4,_=>1.15});}
    if style.code||style.table{tag.set_family(Some("monospace"));}if style.code{tag.set_background(Some(shade));}
    if style.quote>0{tag.set_left_margin((style.quote.min(16)*12) as i32);tag.set_style(pango::Style::Italic);}
    if style.spoiler{tag.set_foreground(Some(shade));tag.set_background(Some(shade));tag.set_underline(pango::Underline::None);tag.set_strikethrough(false);}
    tag
}

fn size_to_content(view:&gtk::TextView){
    if view.width()<=0{return;}
    // Force the lazy layout to validate through the last wrapped line. Its
    // native geometry also accounts for inline emoji paintables and text tags.
    let end=view.buffer().end_iter();let rect=view.iter_location(&end);let (y,height)=view.line_yrange(&end);
    let required=(y+height).max(rect.y()+rect.height()).saturating_add(view.bottom_margin()).max(1);
    if view.height_request()!=required{view.set_height_request(required);}
}

#[cfg(test)]fn install_links(view:&gtk::TextView,targets:Vec<(i32,i32,String)>,launch:impl Fn(&gtk::TextView,&str)+'static){
    install_targets(view,targets.into_iter().map(|(start,end,url)|(start,end,Target::Link(url))).collect(),move |view,target|{if let Target::Link(url)=target{launch(view,url);}});
}
fn install_targets(view:&gtk::TextView,targets:Vec<(i32,i32,Target)>,launch:impl Fn(&gtk::TextView,&Target)+'static){
    let targets=std::rc::Rc::new(targets);
    let click=gtk::GestureClick::new();click.set_button(1);click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed=std::rc::Rc::new(std::cell::Cell::new((0.0f64,0.0f64)));let point=pressed.clone();click.connect_pressed(move |_,_,x,y|point.set((x,y)));
    let weak=view.downgrade();let urls=targets.clone();click.connect_released(move |_,count,x,y|{
        let Some(view)=weak.upgrade()else{return;};let(px,py)=pressed.get();if count!=1||(x-px).abs()>5.0||(y-py).abs()>5.0||view.buffer().has_selection(){return;}
        if let Some(target)=target_at(&view,&urls,x,y){launch(&view,&target);}
    });view.add_controller(click);
    let motion=gtk::EventControllerMotion::new();let weak=view.downgrade();motion.connect_motion(move |_,x,y|{if let Some(view)=weak.upgrade(){view.set_cursor_from_name(Some(if target_at(&view,&targets,x,y).is_some(){"pointer"}else{"text"}));}});view.add_controller(motion);
 }
#[cfg(test)]pub(super) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{pump,until};use std::{rc::Rc,cell::RefCell};use adw::prelude::*;
    let view=gtk::TextView::new();view.set_editable(false);view.buffer().set_text("é https://example.test/hello");let opened=Rc::new(RefCell::new(Vec::new()));let captured=opened.clone();install_links(&view,links("é https://example.test/hello"),move |_,url|captured.borrow_mut().push(url.to_owned()));
    let window=adw::Window::builder().default_width(500).default_height(160).content(&view).build();window.present();until(context,||view.is_mapped());for _ in 0..10{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}
    let location=view.iter_location(&view.buffer().iter_at_offset(5));let(x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,location.x()+1,location.y()+1);let(x,y)=(f64::from(x),f64::from(y));
    let controllers=view.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>().filter(|g|g.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);assert_eq!(*opened.borrow(),["https://example.test/hello"]);
    click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&(x+25.0),&y]);assert_eq!(opened.borrow().len(),1,"dragging text must not launch links");
    view.buffer().select_range(&view.buffer().start_iter(),&view.buffer().end_iter());click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);assert_eq!(opened.borrow().len(),1,"selecting or copying must not launch links");
    window.set_content(None::<&gtk::Widget>);window.close();
}
/// Character offsets are relative to the rendered buffer, never UTF-8 bytes.
pub(super) fn links(text:&str)->Vec<(i32,i32,String)>{
    let mut links=vec![];let mut i=0;
    while i<text.len(){let rest=&text[i..];
        if rest.starts_with('`'){let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];i+=rest[count..].find(delimiter).map_or(rest.len(),|end|count+end+count);continue;}
        if (rest.starts_with("https://")||rest.starts_with("http://")||rest.starts_with("giphy:"))&&(i==0||!text[..i].chars().last().unwrap().is_alphanumeric()){
            let mut end=i+rest.find(|c:char|c.is_whitespace()||matches!(c,'<'|'>'|'"'|'\''|'`')).unwrap_or(rest.len());
            while end>i{let candidate=&text[i..end];let last=candidate.chars().last().unwrap();let trim=matches!(last,','|'.'|'!'|'?'|';')||(last==')'&&candidate.matches(')').count()>candidate.matches('(').count())||(last==']'&&candidate.matches(']').count()>candidate.matches('[').count());if trim{end-=last.len_utf8();}else{break;}}
            let value=&text[i..end];let url=if value.starts_with("giphy:"){crate::media::giphy::id(value).map(|id|format!("https://giphy.com/gifs/{id}"))}else{reqwest::Url::parse(value).ok().filter(|url|matches!(url.scheme(),"http"|"https")&&url.host_str().is_some()).map(|url|url.to_string())};
            if let Some(url)=url{links.push((text[..i].chars().count() as i32,text[..end].chars().count() as i32,url));i=end;continue;}
        }i+=rest.chars().next().unwrap().len_utf8();
    }links
}
fn target_at(view:&gtk::TextView,targets:&[(i32,i32,Target)],x:f64,y:f64)->Option<Target>{
    let(x,y)=view.window_to_buffer_coords(gtk::TextWindowType::Widget,x as i32,y as i32);let iter=view.iter_at_location(x,y)?;let rect=view.iter_location(&iter);
    if x<rect.x()||x>rect.x()+rect.width().max(1)||y<rect.y()||y>rect.y()+rect.height(){return None;}
    let hit=|(start,end,_):&&(i32,i32,Target)|iter.offset()>=*start&&iter.offset()<*end;
    targets.iter().filter(hit).find(|(_,_,target)|matches!(target,Target::Spoiler{shown,..} if !shown.get()))
        .or_else(||targets.iter().filter(hit).find(|(_,_,target)|!matches!(target,Target::Spoiler{..}))).map(|(_,_,target)|target.clone())
}
pub(super) fn compact(text:&str,users:&HashMap<Uuid,UserSummary>,actions:&Actions)->gtk::ScrolledWindow{
    let view=build(&excerpt(text),users,actions,None,true);view.set_widget_name("reply-preview-text");view.set_wrap_mode(gtk::WrapMode::None);view.set_can_target(false);view.set_focusable(false);
    view.set_height_request(-1);
    let metrics=view.pango_context().metrics(None,None);let height=((metrics.ascent()+metrics.descent())/pango::SCALE+6).max(26);
    let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::External).vscrollbar_policy(gtk::PolicyType::External).height_request(height).hexpand(true).child(&view).build();
    scroll.set_can_target(false);scroll.set_overflow(gtk::Overflow::Hidden);scroll
}
pub(super) fn excerpt(text:&str)->String{
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    #[test]fn reply_excerpts_keep_all_unicode_for_width_based_clipping(){
        assert_eq!(excerpt("  first\nsecond\tthird  "),"first second third");let long="🐈".repeat(200);assert_eq!(excerpt(&long),long);
    }
}

#[cfg(test)]mod link_tests{
    use super::*;
    #[test]fn links_preserve_unicode_offsets_balanced_punctuation_and_ignore_code(){
        let links=links("é (https://example.test/a_(b)). `https://hidden.test` giphy:VxdNf4DadRSsMYAfnv javascript:alert(1)");
        assert_eq!(links.len(),2);assert_eq!((links[0].0,links[0].1),(3,29));assert_eq!(links[0].2,"https://example.test/a_(b)");assert_eq!(links[1].2,"https://giphy.com/gifs/VxdNf4DadRSsMYAfnv");
        assert!(super::links("```https://hidden.test``` malformed https://").is_empty());
    }
}
