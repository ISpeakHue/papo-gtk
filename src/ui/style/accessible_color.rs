//! EN-US: Role colors yield to high contrast; each live label owns one weak observer.
//! PT-BR: Cores de cargos cedem ao alto contraste; cada rótulo vivo tem um observador fraco.
use gtk::prelude::*;
use std::{cell::RefCell,collections::HashMap,rc::Rc};
type Palette=Rc<RefCell<Option<gtk::pango::AttrList>>>;
thread_local!{static LABELS:RefCell<HashMap<usize,Palette>>=RefCell::new(HashMap::new());}
fn apply(label:&gtk::Label,normal:&RefCell<Option<gtk::pango::AttrList>>,high:bool){
    let attrs=normal.borrow().as_ref().and_then(|attrs|attrs.copy());
    if high{if let Some(attrs)=&attrs{let _=attrs.filter(|attr|matches!(attr.type_(),gtk::pango::AttrType::Foreground|gtk::pango::AttrType::Background|gtk::pango::AttrType::ForegroundAlpha|gtk::pango::AttrType::BackgroundAlpha));}}
    label.set_attributes(attrs.as_ref());
}
pub(super) fn set(label:&gtk::Label,attrs:Option<gtk::pango::AttrList>){
    let key=label.as_ptr() as usize;
    let existing=LABELS.with(|all|all.borrow().get(&key).cloned());
    let normal=existing.unwrap_or_else(||{
        let normal=Rc::new(RefCell::new(None));let manager=adw::StyleManager::default();let weak=label.downgrade();let saved=normal.clone();
        let handler=manager.connect_high_contrast_notify(move |manager|if let Some(label)=weak.upgrade(){apply(&label,&saved,manager.is_high_contrast());});
        // EN-US: Remove entries on finalization, so recycled/dropped rows never grow this map.
        // PT-BR: Remove entradas na finalização para que linhas recicladas/removidas não aumentem o mapa.
        label.add_weak_ref_notify_local(move||{manager.disconnect(handler);let _=LABELS.try_with(|all|all.borrow_mut().remove(&key));});
        LABELS.with(|all|all.borrow_mut().insert(key,normal.clone()));normal
    });
    *normal.borrow_mut()=attrs;apply(label,&normal,adw::StyleManager::default().is_high_contrast());
}
