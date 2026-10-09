//! Move the existing player into a fullscreen window without duplicating it.
use super::*;
use std::cell::RefCell;

pub(crate) struct Fullscreen {
    slot:gtk::Box,player:gtk::Box,frame:crate::media::frame::Frame,
    media:gtk::MediaStream,button:gtk::Button,placeholder:gtk::Box,
    window:RefCell<Option<gtk::Window>>,stopped:Cell<bool>,
}
impl Fullscreen {
    pub fn new(slot:&gtk::Box,player:&gtk::Box,frame:&crate::media::frame::Frame,media:&impl IsA<gtk::MediaStream>,controls:&gtk::Box)->Rc<Self>{
        let button=gtk::Button::from_icon_name("view-fullscreen-symbolic");button.add_css_class("flat");button.set_tooltip_text(Some("Tela cheia"));button.update_property(&[gtk::accessible::Property::Label("Tela cheia")]);controls.append(&button);
        let placeholder=gtk::Box::new(gtk::Orientation::Vertical,8);placeholder.set_widget_name("fullscreen-video-placeholder");placeholder.append(&gtk::Label::new(Some("Reproduzindo em tela cheia")));
        let back=gtk::Button::with_label("Voltar ao chat");back.set_halign(gtk::Align::Start);placeholder.append(&back);
        let state=Rc::new(Self{slot:slot.clone(),player:player.clone(),frame:frame.clone(),media:media.as_ref().clone(),button:button.clone(),placeholder,window:RefCell::new(None),stopped:Cell::new(false)});
        let weak=Rc::downgrade(&state);button.connect_clicked(move |_|{if let Some(state)=weak.upgrade(){if state.is_open(){state.close();}else{state.open();}}});
        let weak=Rc::downgrade(&state);back.connect_clicked(move |_|{if let Some(state)=weak.upgrade(){state.close();}});state
    }
    pub fn is_open(&self)->bool{self.window.borrow().is_some()}
    fn open(self:&Rc<Self>){
        if self.stopped.get(){return;}
        let current=self.window.borrow().clone();if let Some(window)=current{window.present();return;}
        let Some(parent)=self.slot.root().and_downcast::<gtk::Window>() else{return;};
        let window=gtk::Window::builder().title("Vídeo em tela cheia").default_width(1000).default_height(700).transient_for(&parent).destroy_with_parent(true).build();
        window.set_application(parent.application().as_ref());
        let playing=self.media.is_playing();self.placeholder.set_height_request(self.player.height().max(1));
        self.slot.remove(&self.player);self.slot.append(&self.placeholder);
        self.frame.set_halign(gtk::Align::Fill);self.frame.set_valign(gtk::Align::Fill);self.frame.set_hexpand(true);self.frame.set_vexpand(true);
        window.set_child(Some(&self.player));self.button.set_icon_name("view-restore-symbolic");self.button.set_tooltip_text(Some("Sair da tela cheia"));self.button.update_property(&[gtk::accessible::Property::Label("Sair da tela cheia")]);
        let weak=Rc::downgrade(self);window.connect_close_request(move |_|{if let Some(state)=weak.upgrade(){state.restore();}gtk::glib::Propagation::Proceed});
        let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);let weak=window.downgrade();keys.connect_key_pressed(move |_,key,_,_|{if matches!(key,gtk::gdk::Key::Escape|gtk::gdk::Key::F11){if let Some(window)=weak.upgrade(){window.close();}gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});window.add_controller(keys);
        *self.window.borrow_mut()=Some(window.clone());window.fullscreen();window.present();if playing{self.media.play();}self.button.grab_focus();
    }
    fn restore(&self){
        let window=self.window.borrow_mut().take();let Some(window)=window else{return;};
        let playing=self.media.is_playing();window.set_child(None::<&gtk::Widget>);
        self.frame.set_halign(gtk::Align::Start);self.frame.set_valign(gtk::Align::Start);self.frame.set_hexpand(false);self.frame.set_vexpand(false);
        if self.placeholder.parent().is_some(){self.slot.remove(&self.placeholder);}self.slot.append(&self.player);
        self.button.set_icon_name("view-fullscreen-symbolic");self.button.set_tooltip_text(Some("Tela cheia"));self.button.update_property(&[gtk::accessible::Property::Label("Tela cheia")]);
        if !self.stopped.get(){if playing{self.media.play();}if let Some(parent)=self.slot.root().and_downcast::<gtk::Window>(){parent.present();}self.button.grab_focus();}
    }
    pub fn close(&self){let window=self.window.borrow().clone();if let Some(window)=window{window.close();}}
    pub fn shutdown(&self){self.stopped.set(true);self.close();}
}
impl Drop for Fullscreen {
    fn drop(&mut self){if let Some(window)=self.window.get_mut().take(){window.set_child(None::<&gtk::Widget>);window.close();}}
}
