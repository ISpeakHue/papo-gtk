//! Form-based custom embed authoring, shared by drafts and message edits.
use super::*;
use crate::models::*;
use std::{rc::Rc,cell::RefCell};

#[derive(Clone)]
pub(super) struct Editors {pub root:gtk::Box,items:Rc<RefCell<Vec<(gtk::Expander,Form)>>>}
struct Form {
    inputs:HashMap<&'static str,gtk::Entry>,description:gtk::TextBuffer,
    fields:Rc<RefCell<Vec<Field>>>,required_thumbnail:bool,required_image:bool,
}
struct Field {root:gtk::Box,name:gtk::Entry,value:gtk::TextBuffer,inline:gtk::CheckButton}
fn entry(body:&gtk::Box,label:&str,value:&str)->gtk::Entry{
    let l=gtk::Label::new(Some(label));l.set_xalign(0.0);body.append(&l);
    let e=gtk::Entry::new();e.set_text(value);e.set_hexpand(true);e.update_property(&[gtk::accessible::Property::Label(label)]);body.append(&e);e
}
fn text(body:&gtk::Box,label:&str,value:&str)->gtk::TextBuffer{
    let l=gtk::Label::new(Some(label));l.set_xalign(0.0);body.append(&l);let t=gtk::TextView::new();t.set_wrap_mode(gtk::WrapMode::WordChar);t.buffer().set_text(value);t.update_property(&[gtk::accessible::Property::Label(label)]);
    body.append(&gtk::ScrolledWindow::builder().min_content_height(70).child(&t).build());t.buffer()
}
fn buffer(b:&gtk::TextBuffer)->String{b.text(&b.start_iter(),&b.end_iter(),false).to_string()}
impl Editors {
    pub fn new(inputs:Vec<EmbedInput>)->Self{
        let root=gtk::Box::new(gtk::Orientation::Vertical,8);let list=gtk::Box::new(gtk::Orientation::Vertical,8);root.append(&list);
        let items=Rc::new(RefCell::new(Vec::new()));let add=gtk::Button::with_label("Adicionar embed");root.append(&add);
        for input in inputs{append(&items,&list,&add,input);}
        let weak=Rc::downgrade(&items);let weak_list=list.downgrade();add.connect_clicked(move |add|{if let (Some(items),Some(list))=(weak.upgrade(),weak_list.upgrade()){if items.borrow().len()<10{append(&items,&list,add,EmbedInput::default());}}});
        Self{root,items}
    }
    pub fn read(&self)->anyhow::Result<Vec<EmbedInput>>{
        let inputs=self.items.borrow().iter().map(|(_,form)|form.read()).collect::<anyhow::Result<Vec<_>>>()?;
        crate::models::validate_embeds(&inputs)?;Ok(inputs)
    }
}
fn append(items:&Rc<RefCell<Vec<(gtk::Expander,Form)>>>,list:&gtk::Box,add:&gtk::Button,input:EmbedInput){
    let panel=gtk::Expander::new(Some("Embed personalizado"));panel.set_expanded(items.borrow().is_empty());let body=gtk::Box::new(gtk::Orientation::Vertical,6);panel.set_child(Some(&body));
    let mut inputs=HashMap::new();
    let title=entry(&body,"Título",&input.title);inputs.insert("title",title.clone());
    let weak=panel.downgrade();title.connect_changed(move |title|{if let Some(panel)=weak.upgrade(){let text=title.text();panel.set_label(Some(if text.is_empty(){"Embed personalizado"}else{text.as_str()}));}});
    let description=text(&body,"Descrição",&input.description);
    for (key,label,value) in [
        ("url","Link",input.url.as_str()),("color","Cor (#RRGGBB)",input.color.as_str()),("site","Nome do site",input.site_name.as_str()),
        ("author","Autor",input.author.as_ref().map_or("",|a|a.name.as_str())),("author_url","Link do autor",input.author.as_ref().map_or("",|a|a.url.as_str())),
        ("footer","Rodapé",input.footer.as_ref().map_or("",|f|f.text.as_str())),
        ("thumbnail","URL HTTPS da miniatura",input.thumbnail.as_ref().map_or("",|m|m.url.as_str())),
        ("image","URL HTTPS da imagem",input.image.as_ref().map_or("",|m|m.url.as_str())),
        ("video","URL HTTPS do vídeo",input.video.as_ref().map_or("",|m|m.url.as_str())),
        ("video_mime","Formato do vídeo (video/mp4, video/webm ou video/ogg)",input.video.as_ref().map_or("",|m|m.mime_type.as_str())),
    ]{inputs.insert(key,entry(&body,label,value));}
    let required_thumbnail=input.thumbnail.as_ref().is_some_and(|m|m.url.is_empty());let required_image=input.image.as_ref().is_some_and(|m|m.url.is_empty());
    if required_thumbnail||required_image{let l=gtk::Label::new(Some("Para manter a mídia salva neste embed, informe novamente sua URL. O servidor não devolve a URL original."));l.set_wrap(true);l.add_css_class("warning");body.append(&l);}
    let fields=Rc::new(RefCell::new(Vec::new()));let field_list=gtk::Box::new(gtk::Orientation::Vertical,8);body.append(&field_list);
    let add_field=gtk::Button::with_label("Adicionar campo");body.append(&add_field);for field in input.fields{append_field(&fields,&field_list,&add_field,field);}
    let weak=Rc::downgrade(&fields);let weak_list=field_list.downgrade();add_field.connect_clicked(move |button|{if let (Some(fields),Some(list))=(weak.upgrade(),weak_list.upgrade()){if fields.borrow().len()<25{append_field(&fields,&list,button,EmbedFieldInput::default());}}});
    let remove=gtk::Button::with_label("Remover embed");body.append(&remove);let weak=Rc::downgrade(items);let weak_list=list.downgrade();let weak_panel=panel.downgrade();let weak_add=add.downgrade();
    remove.connect_clicked(move |_|{if let (Some(items),Some(list),Some(panel))=(weak.upgrade(),weak_list.upgrade(),weak_panel.upgrade()){items.borrow_mut().retain(|(p,_)|*p!=panel);list.remove(&panel);if let Some(add)=weak_add.upgrade(){add.set_sensitive(true);}}});
    items.borrow_mut().push((panel.clone(),Form{inputs,description,fields,required_thumbnail,required_image}));list.append(&panel);add.set_sensitive(items.borrow().len()<10);
}
fn append_field(fields:&Rc<RefCell<Vec<Field>>>,list:&gtk::Box,add:&gtk::Button,input:EmbedFieldInput){
    let root=gtk::Box::new(gtk::Orientation::Vertical,4);root.add_css_class("card");let name=entry(&root,"Nome do campo",&input.name);let value=text(&root,"Valor do campo",&input.value);
    let inline=gtk::CheckButton::with_label("Exibir ao lado de outros campos");inline.set_active(input.inline);root.append(&inline);let remove=gtk::Button::with_label("Remover campo");root.append(&remove);
    let weak=Rc::downgrade(fields);let weak_list=list.downgrade();let weak_row=root.downgrade();let weak_add=add.downgrade();remove.connect_clicked(move |_|{if let (Some(fields),Some(list),Some(row))=(weak.upgrade(),weak_list.upgrade(),weak_row.upgrade()){fields.borrow_mut().retain(|f|f.root!=row);list.remove(&row);if let Some(add)=weak_add.upgrade(){add.set_sensitive(true);}}});
    list.append(&root);fields.borrow_mut().push(Field{root,name,value,inline});add.set_sensitive(fields.borrow().len()<25);
}
impl Form {
    fn read(&self)->anyhow::Result<EmbedInput>{
        let value=|key|self.inputs[key].text().to_string();
        let thumbnail=value("thumbnail");let image=value("image");
        anyhow::ensure!(!self.required_thumbnail||!thumbnail.trim().is_empty(),"Informe a URL da miniatura salva ou remova este embed; a edição não pode apagar sua mídia silenciosamente.");
        anyhow::ensure!(!self.required_image||!image.trim().is_empty(),"Informe a URL da imagem salva ou remova este embed; a edição não pode apagar sua mídia silenciosamente.");
        let media=|url:String,mime:String|(!url.trim().is_empty()).then_some(EmbedMediaInput{url,mime_type:mime});
        let author=value("author");let author_url=value("author_url");let footer=value("footer");
        Ok(EmbedInput{title:value("title"),description:buffer(&self.description),url:value("url"),color:value("color"),site_name:value("site"),
            author:(!author.is_empty()||!author_url.is_empty()).then_some(EmbedAuthorInput{name:author,url:author_url}),footer:(!footer.is_empty()).then_some(EmbedFooterInput{text:footer}),
            thumbnail:media(thumbnail,String::new()),image:media(image,String::new()),video:media(value("video"),value("video_mime")),
            fields:self.fields.borrow().iter().map(|f|EmbedFieldInput{name:f.name.text().to_string(),value:buffer(&f.value),inline:f.inline.is_active()}).collect()})
    }
}
