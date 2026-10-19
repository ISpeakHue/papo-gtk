//! Paintables occupy one GTK character; protocol codes remain lossless in drafts.
use super::*;
#[derive(Default)]
pub(super) struct Catalog {
    names:HashMap<String,Uuid>,
    paintables:HashMap<Uuid,gtk::gdk::Paintable>,
    animations:HashMap<Uuid,crate::media::animation::Animation>,
}
impl Composer {
    pub fn set_emojis(&self,entry:&gtk::TextView,actions:&Actions){
        let mut catalog=self.catalog.borrow_mut();
        catalog.names=actions.emoji_names.clone();
        catalog.paintables=actions.emoji_names.values().filter_map(|id|actions.emoji_paintable(*id).map(|p|(*id,p))).collect();
        catalog.animations=actions.animations.clone();drop(catalog);self.schedule_emojis(entry);
    }
    pub(super) fn collapse(&self,value:&mut DisplayText,mut caret:i32)->i32 {
        let catalog=self.catalog.borrow();let mut start=0;let mut ranges=vec![];
        for part in text::parts_index(&value.text,&catalog.names){match part{
            text::Part::Text(text)=>start+=text.len(),
            text::Part::Emoji(id,name)=>{let code=format!(":{name}:");let end=start+code.len();
                if catalog.paintables.contains_key(&id)&&!value.spans.iter().any(|s|start<s.end&&end>s.start){ranges.push((start,end,code));}start=end;
            }
        }}
        for(start,end,code) in ranges.into_iter().rev(){
            let first=value.text[..start].chars().count() as i32;let last=value.text[..end].chars().count() as i32;
            if caret>first{caret=if caret<last{first+1}else{caret-(last-first)+1};}
            value.replace(start,end,"\u{fffc}");value.emojis.push(mentions::InlineEmoji{start,end:start+3,code});
        }
        value.emojis.sort_by_key(|s|s.start);caret
    }
    pub(super) fn refresh_at(&self,entry:&gtk::TextView,mut caret:i32)->i32 {
        // A server emoji can disappear while a draft/undo snapshot retains it.
        // Fall back to its code instead of exposing an orphan object character.
        {
            let catalog=self.catalog.borrow();let mut value=self.value.borrow_mut();
            let missing:Vec<_>=value.emojis.iter().filter(|s|catalog.names.get(s.code.trim_matches(':')).is_none_or(|id|!catalog.paintables.contains_key(id))).cloned().collect();
            for span in missing.into_iter().rev(){
                let start=value.text[..span.start].chars().count() as i32;
                if caret>start{caret+=span.code.chars().count() as i32-1;}
                value.replace(span.start,span.end,&span.code);
            }
        }
        let caret=self.collapse(&mut self.value.borrow_mut(),caret);
        self.updating.set(true);let buffer=entry.buffer();let value=self.value.borrow();
        entry.set_text(&value.text);
        for span in value.emojis.iter().rev(){
            let offset=value.text[..span.start].chars().count() as i32;
            let catalog=self.catalog.borrow();let Some(id)=catalog.names.get(span.code.trim_matches(':'))else{continue;};
            if let Some(paintable)=catalog.paintables.get(id){
                let mut at=buffer.iter_at_offset(offset);let mut end=buffer.iter_at_offset(offset+1);
                buffer.delete(&mut at,&mut end);buffer.insert_paintable(&mut at,paintable);
                if let Some(animation)=catalog.animations.get(id){animation.watch(entry,None);}
            }
        }
        drop(value);self.updating.set(false);self.highlight(entry);entry.set_position(caret);caret
    }
    pub(super) fn schedule_emojis(&self,entry:&gtk::TextView){
        if self.catalog.borrow().names.is_empty()||!self.value.borrow().text.contains(':'){return;}
        if self.pending.replace(true){return;}
        let composer=self.clone();let weak=entry.downgrade();
        gtk::glib::idle_add_local_once(move ||{
            composer.pending.set(false);let Some(entry)=weak.upgrade()else{return;};
            if composer.is_composing(){return;}
            // Replace complete codes only, rather than rewriting the buffer on
            // keystrokes. GTK maintains selection, marks and native IM state.
            let old=composer.value.borrow().clone();let mut next=old.clone();composer.collapse(&mut next,entry.position());
            if old.text==next.text{return;}
            let catalog=composer.catalog.borrow();let mut changes=vec![];let mut offset=0;
            for part in text::parts_index(&old.text,&catalog.names){match part{
                text::Part::Text(text)=>offset+=text.len(),
                text::Part::Emoji(id,name)=>{let length=name.len()+2;
                    if let Some(paintable)=catalog.paintables.get(&id).filter(|_|!old.spans.iter().any(|s|offset<s.end&&offset+length>s.start)){
                        changes.push((offset,offset+length,paintable.clone(),catalog.animations.get(&id).cloned()));
                    }offset+=length;
                }
            }}drop(catalog);
            composer.updating.set(true);let buffer=entry.buffer();buffer.begin_user_action();
            for(start,end,paintable,animation) in changes.into_iter().rev(){
                let mut at=buffer.iter_at_offset(old.text[..start].chars().count() as i32);let mut end=buffer.iter_at_offset(old.text[..end].chars().count() as i32);
                buffer.delete(&mut at,&mut end);buffer.insert_paintable(&mut at,&paintable);
                if let Some(animation)=animation{animation.watch(&entry,None);}
            }
            *composer.value.borrow_mut()=next;buffer.end_user_action();composer.updating.set(false);composer.highlight(&entry);
        });
    }
    // Native undo/paste can reinsert a paintable without insert-text. Recover
    // its code from the shared catalog instead of serializing U+FFFC.
    pub(super) fn recover_emojis(&self,entry:&gtk::TextView){
        let catalog=self.catalog.borrow();let mut value=self.value.borrow_mut();let buffer=entry.buffer();
        if !value.text.contains('\u{fffc}'){value.emojis.clear();return;}
        let previous=std::mem::take(&mut value.emojis);let mut iter=buffer.start_iter();
        loop {
            if let Some(paintable)=iter.paintable(){
                let start=byte(&value.text,iter.offset());
                let code=previous.iter().find(|s|s.start==start).map(|s|s.code.clone()).or_else(||catalog.names.iter().find(|(_,id)|catalog.paintables.get(id)==Some(&paintable)).map(|(name,_)|format!(":{name}:")));
                if let Some(code)=code{
                    value.emojis.push(mentions::InlineEmoji{start,end:start+3,code});
                }
            }
            if !iter.forward_char(){break;}
        }
    }
    fn selection_text(&self,entry:&gtk::TextView)->Option<String>{
        let(start,end)=entry.buffer().selection_bounds()?;let mut value=self.value.borrow().clone();
        let a=byte(&value.text,start.offset());let b=byte(&value.text,end.offset());
        value.replace(b,value.text.len(),"");value.replace(0,a,"");Some(value.wire_text())
    }
    pub(super) fn connect_clipboard(&self,entry:&gtk::TextView){
        let composer=self.clone();entry.connect_copy_clipboard(move |entry|{
            if let Some(text)=composer.selection_text(entry){entry.clipboard().set_text(&text);entry.stop_signal_emission_by_name("copy-clipboard");}
        });
        let composer=self.clone();entry.connect_cut_clipboard(move |entry|{
            if !entry.is_editable(){return;}
            if let Some(text)=composer.selection_text(entry){entry.clipboard().set_text(&text);entry.buffer().delete_selection(true,true);entry.stop_signal_emission_by_name("cut-clipboard");}
        });
    }
}
