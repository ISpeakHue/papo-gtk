//! The editor shares the emoji catalog and completion rules with the composer.
use super::*;
pub(super) struct Completion {
    pub panel:gtk::ScrolledWindow,
    choices:gtk::Box,
    dismissed:Option<(String,i32)>,
    signature:String,
    pub picker:Option<(gtk::Window,emoji::Picker)>,
}
impl Completion {
    pub fn new(text:&gtk::TextView,outer:&gtk::Box)->Self {
        let choices=gtk::Box::new(gtk::Orientation::Vertical,2);
        let panel=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).max_content_height(200).propagate_natural_height(true).child(&choices).visible(false).build();
        panel.set_widget_name("edit-emoji-suggestions");panel.add_css_class("papo-mention-suggestions");
        outer.append(&panel);input::shortcuts::completion(&panel,text);
        Self{panel,choices,dismissed:None,signature:String::new(),picker:None}
    }
    pub fn dismiss(&mut self,text:&gtk::TextView,composer:&composer::Composer){self.dismissed=Some((composer.wire_text(),text.position()));self.panel.set_visible(false);}
}
impl Drop for Completion{fn drop(&mut self){if let Some((window,_))=self.picker.take(){window.close();}}}
impl ChatModel {
    pub(super) fn refresh_edit_emojis(&mut self){
        let Some(edit)=self.actions.edit.as_mut()else{return;};
        let composer=&edit.mentions.composer;
        let allowed=!edit.pending&&self.user_id.is_some_and(|u|self.access.can_edit(u,edit.message.author_id));
        let displayed=edit.text.text();
        let query=emoji::completion(&displayed,edit.text.position());
        if !allowed||composer.is_composing()||query.is_none()||edit.emojis.dismissed.as_ref()==Some(&(composer.wire_text(),edit.text.position())){edit.emojis.panel.set_visible(false);return;}
        let choices=emoji::choices(&self.unicode_emojis,&self.actions.emojis,query.unwrap().2);
        let signature=format!("{:?}",choices.iter().take(30).map(|c|(&c.name,c.id,c.id.and_then(|id|self.actions.textures.get(&id)).map(|t|t.as_ptr()))).collect::<Vec<_>>());
        if signature!=edit.emojis.signature {
            edit.emojis.signature=signature;clear(&edit.emojis.choices);
            for choice in choices.iter().take(30){
                let button=gtk::Button::new();button.add_css_class("flat");let row=gtk::Box::new(gtk::Orientation::Horizontal,8);
                if let Some(image)=choice.id.and_then(|id|self.actions.animations.get(&id).map(|a|{let image=gtk::Image::from_paintable(Some(a));image.set_pixel_size(24);a.watch(&image,None);image}).or_else(||self.actions.textures.get(&id).map(|t|{let image=gtk::Image::from_paintable(Some(t));image.set_pixel_size(24);image}))){row.append(&image);}
                else{row.append(&gtk::Label::new(Some(if choice.id.is_none(){&choice.text}else{"◌"})));}
                let name=gtk::Label::new(Some(&format!(":{}:",choice.name)));name.set_xalign(0.0);name.set_hexpand(true);row.append(&name);button.set_child(Some(&row));
                let token=edit.token;let value=choice.text.clone();let input=edit.mentions.input.clone();button.connect_clicked(move |_|{let _=input.send(ChatMsg::Action(ActionMsg::EditEmojiSelected(token,value.clone(),true)));});edit.emojis.choices.append(&button);
            }
            if choices.is_empty(){edit.emojis.choices.append(&label("Nenhum emoji encontrado"));}
        }
        edit.emojis.panel.set_vscrollbar_policy(if edit.emojis.choices.measure(gtk::Orientation::Vertical,-1).1>200{gtk::PolicyType::Automatic}else{gtk::PolicyType::External});
        edit.emojis.panel.set_visible(true);
    }
}
