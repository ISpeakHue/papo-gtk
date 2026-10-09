//! Composer emoji completion uses GTK's local Unicode catalog and server names.
use super::*;
#[derive(Clone)]
pub(super) struct Choice{pub name:String,pub text:String,pub id:Option<Uuid>}
pub(super) fn completion(text:&str,position:i32)->Option<(usize,usize,&str)>{
    let end=if position<0{text.len()}else{text.char_indices().nth(position as usize).map_or(text.len(),|(i,_)|i)};
    let start=text[..end].rfind(':')?;
    if start>0&&!text[..start].chars().last()?.is_whitespace(){return None;}
    let name=&text[start+1..end];if name.chars().any(|c|c.is_whitespace()||matches!(c,':'|'/'|'`'|'<'|'>'|'(' | ')')){return None;}
    // Do not complete inside fenced/inline code.
    let mut i=0;let prefix=&text[..start];while i<prefix.len(){let rest=&prefix[i..];if rest.starts_with('`'){let count=rest.bytes().take_while(|b|*b==b'`').count();let delimiter=&rest[..count];let end=rest[count..].find(delimiter)?;i+=count+end+count;}else{i+=rest.chars().next()?.len_utf8();}}
    Some((start,end,name))
}
pub(super) fn insert(text:&str,position:i32,value:&str,replace:bool)->(String,i32){
    let end=if position<0{text.len()}else{text.char_indices().nth(position as usize).map_or(text.len(),|(i,_)|i)};
    let(start,end)=if replace{completion(text,position).map_or((end,end),|(s,e,_)|(s,e))}else{(end,end)};
    let suffix=if text[end..].starts_with(':')&&replace{&text[end+1..]}else{&text[end..]};
    let value=format!("{value} ");let caret=(text[..start].chars().count()+value.chars().count()) as i32;
    (format!("{}{value}{suffix}",&text[..start]),caret)
}
pub(super) fn builtin()->Vec<Choice>{
    let mut names=std::collections::BTreeMap::<String,String>::new();
    if let Ok(bytes)=gtk::gio::resources_lookup_data("/org/gtk/libgtk/emoji/en.data",gtk::gio::ResourceLookupFlags::NONE){
        // GTK versions ship one of these catalog formats; validate before reading.
        for signature in ["a(aussasasu)","a(ausasu)"]{
            let Ok(kind)=gtk::glib::VariantTy::new(signature)else{continue;};
            let data=gtk::glib::Variant::from_bytes_with_type(&bytes,kind);
            if !data.is_normal_form(){continue;}
            for i in 0..data.n_children().min(4096){let item=data.child_value(i);let Some(name)=item.child_value(1).str().map(str::to_owned)else{continue;};
                let Some(codes)=item.child_value(0).get::<Vec<u32>>()else{continue;};
                let value:String=codes.into_iter().filter(|c|*c!=0).filter_map(char::from_u32).collect();
                let name:String=name.chars().flat_map(char::to_lowercase).map(|c|if c.is_alphanumeric(){c}else{'_'}).collect();
                let name=name.split('_').filter(|p|!p.is_empty()).collect::<Vec<_>>().join("_");
                if !name.is_empty()&&!value.is_empty(){names.insert(name,value);}
            }break;
        }
    }
    for (name,value) in [("smile","😄"),("smiley","😃"),("grinning","😀"),("joy","😂"),("rofl","🤣"),("heart","❤️"),("+1","👍"),("-1","👎"),("thumbsup","👍"),("thumbsdown","👎"),("tada","🎉"),("sob","😭"),("thinking","🤔"),("fire","🔥"),("eyes","👀"),("100","💯"),("wave","👋"),("laughing","😆"),("sweat_smile","😅"),("blush","😊"),("wink","😉"),("sunglasses","😎"),("slight_smile","🙂"),("upside_down","🙃"),("cry","😢"),("angry","😠"),("rage","😡"),("poop","💩"),("ok_hand","👌"),("clap","👏"),("pray","🙏"),("muscle","💪"),("rocket","🚀"),("warning","⚠️"),("white_check_mark","✅"),("x","❌"),("skull","💀")]{names.insert(name.into(),value.into());}
    names.into_iter().map(|(name,text)|Choice{name,text,id:None}).collect()
}
pub(super) fn choices(builtin:&[Choice],custom:&[crate::models::Emoji],filter:&str)->Vec<Choice>{
    let filter=filter.to_lowercase();let mut choices:Vec<_>=custom.iter().map(|e|Choice{name:e.name.clone(),text:format!(":{}:",e.name),id:Some(e.id)}).chain(builtin.iter().cloned()).filter(|c|c.name.to_lowercase().contains(&filter)||c.text.contains(&filter)).collect();
    choices.sort_by_cached_key(|c|{let common=filter.is_empty()&&["heart","thumbsup","joy","smile","fire","tada","eyes"].contains(&c.name.as_str())&&c.id.is_none();(!common,!c.name.to_lowercase().starts_with(&filter),c.id.is_none(),c.name.to_lowercase())});choices
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn colon_completion_respects_unicode_caret_and_literals(){assert_eq!(completion("é :OME après",6),Some((3,7,"OME")));for text in ["https://host", "giphy:abc", "`:smile", ":smile:","12:34"]{assert!(completion(text,-1).is_none(),"{text}");}let(text,caret)=insert("é :smi après",6,"😄",true);assert_eq!(text,"é 😄  après");assert_eq!(caret,4);}
    #[test]fn every_server_name_remains_available_and_case_is_preserved(){let e:crate::models::Emoji=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"OMEGALUL","format":"PNG","created_at":"2026-10-08T12:00:00Z"})).unwrap();let found=choices(&[],&[e],"omega");assert_eq!(found[0].text,":OMEGALUL:");}
}

/// Both pickers use one searchable catalog and bounded pages of square cells.
pub(super) fn picker(list:&gtk::Box, builtin:&[Choice], custom:&[crate::models::Emoji], textures:&HashMap<Uuid,gtk::gdk::Texture>, choose:impl Fn(Choice)+'static) {
    let previous=list.first_child().and_downcast::<gtk::SearchEntry>();
    let focused=previous.as_ref().is_some_and(|s|s.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));
    let caret=previous.as_ref().map(|s|s.position()).unwrap_or(-1);
    let previous=previous.map(|s|s.text().to_string()).unwrap_or_default();
    while let Some(child)=list.first_child(){list.remove(&child);}
    let search=gtk::SearchEntry::new();search.set_placeholder_text(Some("Buscar emoji Unicode ou do servidor"));search.set_text(&previous);list.append(&search);
    let grid=gtk::FlowBox::new();grid.set_selection_mode(gtk::SelectionMode::None);grid.set_homogeneous(true);grid.set_min_children_per_line(6);grid.set_max_children_per_line(6);grid.set_row_spacing(4);grid.set_column_spacing(4);
    let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).min_content_height(240).child(&grid).build();list.append(&scroll);
    let empty=gtk::Label::new(Some("Nenhum emoji encontrado"));list.append(&empty);
    let more=gtk::Button::with_label("Mais emojis");list.append(&more);
    let catalog=std::rc::Rc::new((builtin.to_vec(),custom.to_vec(),textures.clone()));let choose=std::rc::Rc::new(choose);let limit=std::rc::Rc::new(std::cell::Cell::new(96usize));
    let weak_search=search.downgrade();let weak_grid=grid.downgrade();let weak_more=more.downgrade();let weak_empty=empty.downgrade();let count=limit.clone();
    let render:std::rc::Rc<dyn Fn()>=std::rc::Rc::new(move ||{
        let (Some(search),Some(grid),Some(more),Some(empty))=(weak_search.upgrade(),weak_grid.upgrade(),weak_more.upgrade(),weak_empty.upgrade())else{return;};
        while let Some(child)=grid.first_child(){grid.remove(&child);}
        let choices=choices(&catalog.0,&catalog.1,search.text().trim());empty.set_visible(choices.is_empty());more.set_visible(choices.len()>count.get());
        for choice in choices.into_iter().take(count.get()){
            let button=gtk::Button::new();button.add_css_class("flat");button.add_css_class("papo-emoji-cell");button.set_size_request(44,44);button.set_halign(gtk::Align::Center);button.set_valign(gtk::Align::Center);button.set_tooltip_text(Some(&format!(":{}:",choice.name)));button.update_property(&[gtk::accessible::Property::Label(&choice.name)]);
            if let Some(texture)=choice.id.and_then(|id|catalog.2.get(&id)){let image=gtk::Image::from_paintable(Some(texture));image.set_pixel_size(28);button.set_child(Some(&image));}
            else{button.set_label(if choice.id.is_none(){&choice.text}else{"◌"});}
            let select=choose.clone();button.connect_clicked(move |_|select(choice.clone()));grid.insert(&button,-1);
        }
    });
    let update=render.clone();let count=limit.clone();search.connect_search_changed(move |_|{count.set(96);update();});
    let update=render.clone();more.connect_clicked(move |_|{limit.set(limit.get()+96);update();});render();
    if focused{search.grab_focus();search.set_position(caret);}
}
