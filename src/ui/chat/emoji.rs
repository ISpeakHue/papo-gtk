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
#[cfg(test)]pub(super) fn insert(text:&str,position:i32,value:&str,replace:bool)->(String,i32){
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

/// Stable tabs preserve search, loaded cells and scroll position during API updates.
#[derive(Clone)]
pub(super) struct Picker {
    catalog:std::rc::Rc<std::cell::RefCell<Catalog>>,
    server_page:gtk::StackPage,status:gtk::Label,
    render:std::rc::Rc<dyn Fn()>,
}
#[derive(Default)]struct Catalog {
    builtin:Vec<Choice>,custom:Vec<crate::models::Emoji>,
    textures:HashMap<Uuid,gtk::gdk::Texture>,animations:HashMap<Uuid,crate::media::animation::Animation>,
}
impl Picker {
    pub fn new(root:&gtk::Box,builtin:&[Choice],choose:impl Fn(Choice)+'static)->Self {
        let search=gtk::SearchEntry::new();search.set_placeholder_text(Some("Buscar emoji"));root.append(&search);
        let tabs=gtk::Stack::new();tabs.set_widget_name("emoji-tabs");tabs.set_transition_type(gtk::StackTransitionType::None);tabs.set_vexpand(true);
        let switcher=gtk::StackSwitcher::builder().stack(&tabs).halign(gtk::Align::Center).build();root.append(&switcher);let status=gtk::Label::new(None);status.set_wrap(true);status.set_visible(false);root.append(&status);root.append(&tabs);
        let catalog=std::rc::Rc::new(std::cell::RefCell::new(Catalog{builtin:builtin.to_vec(),..Default::default()}));
        let choose:std::rc::Rc<dyn Fn(Choice)>=std::rc::Rc::new(choose);let mut renders=Vec::<std::rc::Rc<dyn Fn()>>::new();let mut server_page=None;
        for custom in [false,true] {
            let body=gtk::Box::new(gtk::Orientation::Vertical,4);
            let grid=gtk::FlowBox::new();grid.set_selection_mode(gtk::SelectionMode::None);grid.set_homogeneous(true);grid.set_min_children_per_line(1);grid.set_max_children_per_line(128);grid.set_valign(gtk::Align::Start);grid.set_row_spacing(4);grid.set_column_spacing(4);
            let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).min_content_height(240).child(&grid).build();body.append(&scroll);
            let empty=gtk::Label::new(Some("Nenhum emoji encontrado"));body.append(&empty);
            let page=tabs.add_titled(&body,Some(if custom{"server"}else{"unicode"}),if custom{"Servidor"}else{"Unicode"});if custom{server_page=Some(page);}
            let limit=std::rc::Rc::new(std::cell::Cell::new(96usize));let rendered=std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));let total=std::rc::Rc::new(std::cell::Cell::new(0usize));
            let (weak_search,weak_grid,weak_empty)=(search.downgrade(),grid.downgrade(),empty.downgrade());let data=catalog.clone();let count=limit.clone();let keys=rendered.clone();let size=total.clone();let select=choose.clone();
            let render:std::rc::Rc<dyn Fn()>=std::rc::Rc::new(move || {
                let(Some(search),Some(grid),Some(empty))=(weak_search.upgrade(),weak_grid.upgrade(),weak_empty.upgrade())else{return;};let data=data.borrow();
                let found=if custom{choices(&[],&data.custom,search.text().trim())}else{choices(&data.builtin,&[],search.text().trim())};size.set(found.len());empty.set_visible(found.is_empty());
                let found:Vec<_>=found.into_iter().take(count.get()).collect();
                let signatures:Vec<_>=found.iter().map(|c|format!("{}|{:?}|{}|{:?}|{:?}",c.name,c.id,c.text,c.id.and_then(|id|data.textures.get(&id)).map(|p|p.as_ptr()),c.id.and_then(|id|data.animations.get(&id)).map(|p|p.as_ptr()))).collect();
                let mut keys=keys.borrow_mut();if !signatures.starts_with(&keys){while let Some(child)=grid.first_child(){grid.remove(&child);}keys.clear();}
                for choice in found.into_iter().skip(keys.len()) {
                    let button=gtk::Button::new();button.add_css_class("flat");button.add_css_class("papo-emoji-cell");button.set_size_request(44,44);button.set_halign(gtk::Align::Center);button.set_valign(gtk::Align::Center);button.set_tooltip_text(Some(&format!(":{}:",choice.name)));button.update_property(&[gtk::accessible::Property::Label(&choice.name)]);
                    if let Some(paintable)=choice.id.and_then(|id|data.animations.get(&id).map(|a|a.clone().upcast::<gtk::gdk::Paintable>()).or_else(||data.textures.get(&id).map(|t|t.clone().upcast()))) {
                        let image=gtk::Image::from_paintable(Some(&paintable));image.set_pixel_size(28);if let Some(animation)=choice.id.and_then(|id|data.animations.get(&id)){animation.watch(&image,None);}button.set_child(Some(&image));
                    }else{button.set_label(if choice.id.is_none(){&choice.text}else{"◌"});}
                    let select=select.clone();button.connect_clicked(move |_|select(choice.clone()));grid.insert(&button,-1);
                }
                *keys=signatures;
            });
            let update=render.clone();let count=limit.clone();let weak=scroll.downgrade();search.connect_search_changed(move |_|{if let Some(scroll)=weak.upgrade(){scroll.vadjustment().set_value(0.0);}count.set(96);update();});
            let update=render.clone();let count=limit.clone();let size=total.clone();let weak=scroll.downgrade();
            let pending=std::rc::Rc::new(std::cell::Cell::new(false));
            let more:std::rc::Rc<dyn Fn()>=std::rc::Rc::new(move ||{
                if pending.replace(true){return;}
                let (weak,count,size,update,pending)=(weak.clone(),count.clone(),size.clone(),update.clone(),pending.clone());
                // Adjustment changes also occur during allocation. Append after
                // layout, coalescing resize/scroll signals into one page check.
                gtk::glib::idle_add_local_once(move ||{
                    pending.set(false);let Some(scroll)=weak.upgrade().filter(|s|s.is_mapped())else{return;};let adj=scroll.vadjustment();
                    if adj.page_size()<=0.0||adj.upper()-adj.page_size()-adj.value()>160.0||count.get()>=size.get(){return;}
                    count.set(count.get()+96);update();
                });
            });
            let more_scroll=more.clone();scroll.vadjustment().connect_value_changed(move |_|more_scroll());
            let more_size=more.clone();scroll.vadjustment().connect_changed(move |_|more_size());
            scroll.connect_map(move |_|more());renders.push(render);
        }
        tabs.set_visible_child_name("unicode");let render:std::rc::Rc<dyn Fn()>=std::rc::Rc::new(move ||{for render in &renders{render();}});
        let picker=Self{catalog,server_page:server_page.unwrap(),status,render};(picker.render)();picker
    }
    pub fn show_error(&self,message:&str){self.status.set_text(message);self.status.set_visible(true);}
    pub fn update(&self,name:&str,custom:&[crate::models::Emoji],textures:&HashMap<Uuid,gtk::gdk::Texture>,animations:&HashMap<Uuid,crate::media::animation::Animation>){
        self.status.set_visible(false);self.status.set_text("");let title=if name.chars().count()>20{format!("{}…",name.chars().take(20).collect::<String>())}else{name.into()};self.server_page.set_title(&title);self.server_page.child().set_tooltip_text(Some(name));let mut data=self.catalog.borrow_mut();data.custom=custom.to_vec();data.textures=textures.clone();data.animations=animations.clone();drop(data);(self.render)();
    }
}
#[cfg(test)]fn picker(list:&gtk::Box,builtin:&[Choice],custom:&[crate::models::Emoji],textures:&HashMap<Uuid,gtk::gdk::Texture>,animations:&HashMap<Uuid,crate::media::animation::Animation>,choose:impl Fn(Choice)+'static){
    let picker=Picker::new(list,builtin,choose);picker.update("Servidor",custom,textures,animations);
    list.last_child().and_downcast::<gtk::Stack>().unwrap().set_visible_child_name("server");
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,pump,until};use base64::Engine;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let id=Uuid::new_v4();
    backend.lock().unwrap().emojis=vec![serde_json::json!({"id":id,"name":"dance","format":"GIF","image_blob":base64::engine::general_purpose::STANDARD.encode(crate::media::animation::fixture()),"created_at":"2026-10-03T00:00:00Z"})];
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),..Default::default()}).detach();
    chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));until(context,||chat.model().actions.animations.contains_key(&id)&&chat.model().actions.emojis_request.is_none());
    let animation=chat.model().actions.animations[&id].clone();
    let view=chat.model().profile_text("A :dance: emoji");let buffer=view.buffer();let inline=buffer.iter_at_offset(2).paintable().unwrap();assert_eq!(inline,animation.clone().upcast::<gtk::gdk::Paintable>());
    let column=gtk::Box::new(gtk::Orientation::Vertical,0);column.append(&view);let spacer=gtk::Box::new(gtk::Orientation::Vertical,0);spacer.set_height_request(1000);column.append(&spacer);
    let scroll=gtk::ScrolledWindow::builder().child(&column).build();let window=gtk::Window::builder().default_width(400).default_height(200).child(&scroll).build();window.present();until(context,||view.is_mapped());
    let first=inline.current_image();until(context,||inline.current_image()!=first);
    scroll.vadjustment().set_value(800.0);pump(context);let clipped=inline.current_image();for _ in 0..80{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}assert_eq!(inline.current_image(),clipped,"offscreen inline emojis must pause");
    scroll.vadjustment().set_value(0.0);until(context,||inline.current_image()!=clipped);
    window.set_child(None::<&gtk::Widget>);window.close();pump(context);let hidden=animation.current_image();for _ in 0..40{pump(context);std::thread::sleep(std::time::Duration::from_millis(5));}assert_eq!(animation.current_image(),hidden,"cached channel/profile emoji must not keep animating");
    let list=gtk::Box::new(gtk::Orientation::Vertical,0);{let model=chat.model();picker(&list,&model.unicode_emojis,&model.actions.emojis,&model.actions.textures,&model.actions.animations,|_|{});}
    let picker=gtk::Window::builder().default_width(400).default_height(400).child(&list).build();picker.present();until(context,||list.is_mapped());until(context,||animation.current_image()!=hidden);
    chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));until(context,||chat.model().actions.emojis_request.is_none());assert_eq!(chat.model().actions.animations[&id],animation,"catalog refresh reuses decoded frames");
    picker.set_child(None::<&gtk::Widget>);picker.close();backend.lock().unwrap().emojis.clear();chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));until(context,||chat.model().actions.animations.is_empty());
    drop(inline);drop(buffer);drop(view);drop(column);scroll.set_child(None::<&gtk::Widget>);drop(list);drop(picker);drop(window);drop(scroll);let weak=animation.downgrade();drop(animation);drop(chat);server.abort();until(context,||weak.upgrade().is_none());
}

#[cfg(test)]
pub(crate) fn exercise_resize(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,pump,until};use adw::prelude::*;
    let root=gtk::Box::new(gtk::Orientation::Vertical,8);
    let builtin:Vec<_>=(0..1024).map(|i|Choice{name:format!("unicode_{i:04}"),text:"😀".into(),id:None}).collect();
    let custom:Vec<crate::models::Emoji>=(0..512).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":format!("server_{i:04}"),"format":"PNG","created_at":"2026-10-15T12:00:00Z"})).unwrap()).collect();
    let chosen=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));let selected=chosen.clone();
    let picker=Picker::new(&root,&builtin,move |choice|selected.borrow_mut().push(choice.id));picker.update("Server",&custom,&HashMap::new(),&HashMap::new());
    let window=adw::Window::builder().default_width(400).default_height(420).content(&root).build();window.present();performance::settle(context);
    let tabs=descendants(root.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Stack>().ok()).unwrap();
    let search=descendants(root.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::SearchEntry>().ok()).unwrap();
    let active=||{let widgets=descendants(&tabs.visible_child().unwrap());(widgets.iter().find_map(|w|w.downcast_ref::<gtk::FlowBox>()).unwrap().clone(),widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone())};
    let geometry=|grid:&gtk::FlowBox,scroll:&gtk::ScrolledWindow|{
        let mut rows=Vec::<i32>::new();let mut first_row=0;let mut visible=0;
        let mut child=grid.first_child();
        while let Some(cell)=child{
            child=cell.next_sibling();let bounds=cell.compute_bounds(grid).unwrap();let y=bounds.y().round() as i32;
            if rows.last()!=Some(&y){rows.push(y);}if y==rows[0]{first_row+=1;}
            let bounds=cell.compute_bounds(scroll).unwrap();if bounds.y()>=0.0&&bounds.y()+bounds.height()<=scroll.height() as f32{visible+=1;}
        }
        (first_row,rows.get(1).map(|y|y-rows[0]).unwrap_or(0),visible)
    };
    let (grid,scroll)=active();let first=grid.first_child().unwrap();let small=geometry(&grid,&scroll);assert!(small.0>1&&small.1>0);
    window.set_default_size(400,760);performance::settle(context);let tall=geometry(&grid,&scroll);
    assert_eq!(tall.0,small.0);assert_eq!(tall.1,small.1,"growing vertically must preserve compact row spacing");assert!(tall.2>small.2,"a taller picker must show more emoji rows");
    window.set_default_size(1000,760);performance::settle(context);let wide=geometry(&grid,&scroll);
    assert!(wide.0>tall.0,"a wider picker must add columns");assert_eq!(wide.1,small.1);assert!(wide.2>96,"a large viewport must fill beyond the first lazy page");
    assert!(grid.observe_children().n_items()>96&&grid.observe_children().n_items()<1024,"resize fills the viewport without eagerly mounting the entire catalog");assert_eq!(grid.first_child().unwrap(),first,"resizing preserves existing cells");
    // Both shared picker tabs use the same responsive layout.
    tabs.set_visible_child_name("server");performance::settle(context);let (server,server_scroll)=active();let broad=geometry(&server,&server_scroll);assert!(broad.0>small.0&&broad.2>96);
    window.set_default_size(320,420);performance::settle(context);let narrow=geometry(&server,&server_scroll);assert!(narrow.0<broad.0);assert_eq!(narrow.1,broad.1);assert!(server_scroll.hadjustment().upper()<=server_scroll.hadjustment().page_size()+1.0);
    window.set_default_size(1000,760);search.set_text("server_0001");until(context,||server.observe_children().n_items()==1);performance::settle(context);
    let cell=server.first_child().unwrap();assert!(cell.height()<64,"one filtered emoji must not stretch to fill the window");assert!(server.height()<80,"unused height belongs below the grid");
    cell.downcast::<gtk::FlowBoxChild>().unwrap().child().and_downcast::<gtk::Button>().unwrap().emit_clicked();assert_eq!(*chosen.borrow(),vec![Some(custom[1].id)]);
    window.set_content(None::<&gtk::Widget>);window.close();drop(picker);pump(context);
}
