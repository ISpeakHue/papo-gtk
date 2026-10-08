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
    let buffer=view.buffer();let mut iter=buffer.start_iter();let mut targets=vec![];
    let tag=gtk::TextTag::builder().foreground("#3584e4").underline(pango::Underline::Single).build();buffer.tag_table().add(&tag);
    for part in parts(text,&actions.emojis){match part{
        Part::Text(text)=>{let text=mentions::render(&text,users);let offset=iter.offset();buffer.insert(&mut iter,&text);for(start,end,url) in links(&text){let start=offset+start;let end=offset+end;buffer.apply_tag(&tag,&buffer.iter_at_offset(start),&buffer.iter_at_offset(end));targets.push((start,end,url));}},
        Part::Emoji(id,name)=>{if let Some(texture)=actions.textures.get(&id){buffer.insert_paintable(&mut iter,texture);}else{buffer.insert(&mut iter,&format!(":{name}:"));}},
    }}
    install_links(&view,targets,|view,url|{let launcher=gtk::UriLauncher::new(url);let parent=view.root().and_downcast::<gtk::Window>();launcher.launch(parent.as_ref(),None::<&gtk::gio::Cancellable>,|result|{if let Err(error)=result{tracing::warn!("Could not open chat link: {error}");}});});
    // Keep a textual description available even when a custom image is displayed.
    view.set_tooltip_text(Some(&mentions::render(text,users)));view
}

fn install_links(view:&gtk::TextView,targets:Vec<(i32,i32,String)>,launch:impl Fn(&gtk::TextView,&str)+'static){
    let targets=std::rc::Rc::new(targets);
    let click=gtk::GestureClick::new();click.set_button(1);click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed=std::rc::Rc::new(std::cell::Cell::new((0.0f64,0.0f64)));let point=pressed.clone();click.connect_pressed(move |_,_,x,y|point.set((x,y)));
    let weak=view.downgrade();let urls=targets.clone();click.connect_released(move |_,count,x,y|{
        let Some(view)=weak.upgrade()else{return;};let(px,py)=pressed.get();if count!=1||(x-px).abs()>5.0||(y-py).abs()>5.0||view.buffer().has_selection(){return;}
        if let Some(url)=target_at(&view,&urls,x,y){launch(&view,&url);}
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
fn target_at(view:&gtk::TextView,targets:&[(i32,i32,String)],x:f64,y:f64)->Option<String>{
    let(x,y)=view.window_to_buffer_coords(gtk::TextWindowType::Widget,x as i32,y as i32);let iter=view.iter_at_location(x,y)?;let rect=view.iter_location(&iter);
    if x<rect.x()||x>rect.x()+rect.width().max(1)||y<rect.y()||y>rect.y()+rect.height(){return None;}
    targets.iter().find(|(start,end,_)|iter.offset()>=*start&&iter.offset()<*end).map(|(_,_,url)|url.clone())
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

#[cfg(test)]mod link_tests{
    use super::*;
    #[test]fn links_preserve_unicode_offsets_balanced_punctuation_and_ignore_code(){
        let links=links("é (https://example.test/a_(b)). `https://hidden.test` giphy:VxdNf4DadRSsMYAfnv javascript:alert(1)");
        assert_eq!(links.len(),2);assert_eq!((links[0].0,links[0].1),(3,29));assert_eq!(links[0].2,"https://example.test/a_(b)");assert_eq!(links[1].2,"https://giphy.com/gifs/VxdNf4DadRSsMYAfnv");
        assert!(super::links("```https://hidden.test``` malformed https://").is_empty());
    }
}
