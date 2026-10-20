//! EN-US: Cached emoji metadata in a small native popover, anchored to the clicked glyph.
//! PT-BR: Metadados de emoji em cache exibidos em um popover nativo ancorado no símbolo clicado.
use super::*;
use std::{cell::RefCell,rc::Rc};

#[derive(Clone)]
pub(super) enum Preview {
    Custom{paintable:Option<gtk::gdk::Paintable>,animation:Option<crate::media::animation::Animation>,source:Rc<RefCell<String>>},
    Unicode(String),
}
#[derive(Clone)]
pub(super) struct Details {pub name:String,pub preview:Preview}
impl Details {
    pub fn custom(id:Uuid,name:String,actions:&Actions)->Self{
        Self{name,preview:Preview::Custom{paintable:actions.emoji_paintable(id),animation:actions.animations.get(&id).cloned(),source:actions.emoji_source.clone()}}
    }
}
pub(super) type Popup=Rc<RefCell<Option<gtk::Popover>>>;
pub(super) fn dismiss(state:&Popup){
    let previous=state.borrow_mut().take();
    if let Some(popover)=previous{popover.popdown();if popover.parent().is_some(){popover.unparent();}}
}
pub(super) fn connect(view:&gtk::TextView)->Popup{
    let popup=Popup::default();let state=popup.clone();view.connect_unmap(move |_|dismiss(&state));
    let state=popup.clone();view.connect_destroy(move |_|dismiss(&state));popup
}
pub(super) fn show(view:&gtk::TextView,offset:i32,details:&Details,state:&Popup){
    dismiss(state);
    let popover=gtk::Popover::new();popover.set_widget_name("emoji-details");popover.set_autohide(true);popover.set_position(gtk::PositionType::Right);
    let body=gtk::Box::new(gtk::Orientation::Horizontal,12);body.set_margin_top(10);body.set_margin_bottom(10);body.set_margin_start(10);body.set_margin_end(10);
    let source=match &details.preview{
        Preview::Custom{paintable,animation,source}=>{
            let image=gtk::Image::from_paintable(paintable.as_ref());if paintable.is_none(){image.set_icon_name(Some("face-smile-symbolic"));}
            image.set_widget_name("emoji-details-preview");image.set_pixel_size(48);image.set_valign(gtk::Align::Center);image.update_property(&[gtk::accessible::Property::Label(&format!(":{}:",details.name))]);
            if let Some(animation)=animation{animation.watch(&image,None);}body.append(&image);
            let source=source.borrow();if source.trim().is_empty(){"Emoji personalizado deste servidor".into()}else{format!("Este emoji é do servidor {source}")}
        },
        Preview::Unicode(glyph)=>{
            let image=gtk::Label::new(Some(glyph));image.set_widget_name("emoji-details-preview");let attributes=pango::AttrList::new();attributes.insert(pango::AttrFloat::new_scale(2.2));image.set_attributes(Some(&attributes));image.set_valign(gtk::Align::Center);body.append(&image);
            "Emoji Unicode".into()
        },
    };
    let labels=gtk::Box::new(gtk::Orientation::Vertical,3);labels.set_valign(gtk::Align::Center);
    let title=gtk::Label::new(Some(&format!(":{}:",details.name)));title.set_widget_name("emoji-details-name");title.add_css_class("heading");title.set_xalign(0.0);title.set_max_width_chars(28);title.set_ellipsize(pango::EllipsizeMode::End);title.set_tooltip_text(Some(&format!(":{}:",details.name)));labels.append(&title);
    let origin=gtk::Label::new(Some(&source));origin.set_widget_name("emoji-details-source");origin.add_css_class("dim-label");origin.add_css_class("caption");origin.set_xalign(0.0);origin.set_wrap(true);origin.set_max_width_chars(32);origin.set_lines(2);origin.set_ellipsize(pango::EllipsizeMode::End);labels.append(&origin);body.append(&labels);popover.set_child(Some(&body));
    let rect=view.iter_location(&view.buffer().iter_at_offset(offset));let(x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x(),rect.y());
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x,y,rect.width().max(1),rect.height().max(1))));popover.set_parent(view);
    let weak=Rc::downgrade(state);popover.connect_closed(move |popover|{
        if popover.parent().is_some(){popover.unparent();}
        if let Some(state)=weak.upgrade(){let mut current=state.borrow_mut();if current.as_ref()==Some(popover){current.take();}}
    });
    *state.borrow_mut()=Some(popover.clone());popover.popup();
}

struct Candidate {glyph:String,name:String}
struct UnicodeIndex(HashMap<char,Vec<Candidate>>);
impl UnicodeIndex {
    fn new(choices:Vec<emoji::Choice>)->Self{
        let mut index=HashMap::<char,Vec<Candidate>>::new();
        for choice in choices{if let Some(first)=choice.text.chars().next(){let candidates=index.entry(first).or_default();if !candidates.iter().any(|old|old.glyph==choice.text){candidates.push(Candidate{glyph:choice.text,name:choice.name});}}}
        for candidates in index.values_mut(){candidates.sort_by_key(|c|std::cmp::Reverse(c.glyph.len()));}Self(index)
    }
    fn spans(&self,text:&str)->Vec<(i32,i32,Details)>{
        let mut result=vec![];let(mut byte,mut offset)=(0,0);
        while byte<text.len(){let rest=&text[byte..];let first=rest.chars().next().unwrap();
            if let Some(found)=self.0.get(&first).and_then(|candidates|candidates.iter().find(|c|rest.starts_with(&c.glyph))){
                let mut end=modifiers_end(rest,found.glyph.len());
                // The index chooses the longest known sequence. If it only
                // recognizes the start of a joined glyph, skip the complete
                // cluster rather than label one component as the whole emoji.
                if rest[end..].starts_with('\u{200d}'){
                    while rest[end..].starts_with('\u{200d}'){
                        end+='\u{200d}'.len_utf8();
                        let Some(next)=rest[end..].chars().next()else{break;};
                        end=modifiers_end(rest,end+next.len_utf8());
                    }
                    byte+=end;offset+=rest[..end].chars().count() as i32;continue;
                }
                let chars=rest[..end].chars().count() as i32;
                result.push((offset,offset+chars,Details{name:found.name.clone(),preview:Preview::Unicode(rest[..end].into())}));byte+=end;offset+=chars;continue;
            }
            byte+=first.len_utf8();offset+=1;
        }result
    }
}
// Variation selectors and skin tones belong to the same clickable glyph.
fn modifiers_end(text:&str,mut end:usize)->usize{
    while let Some(modifier)=text[end..].chars().next().filter(|c|matches!(*c,'\u{fe0f}'|'\u{1f3fb}'..='\u{1f3ff}')){end+=modifier.len_utf8();}end
}
thread_local!{static UNICODE:UnicodeIndex=UnicodeIndex::new(emoji::builtin());}
pub(super) fn unicode(text:&str)->Vec<(i32,i32,Details)>{
    if text.is_ascii(){return vec![];}UNICODE.with(|index|index.spans(text))
}
pub(super) fn has_unicode(text:&str)->bool{!unicode(text).is_empty()}

#[cfg(test)]mod tests;
#[cfg(test)]pub(crate) use tests::exercise;
