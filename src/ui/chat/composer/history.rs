//! EN-US: GTK's text history omits paintables. Keep bounded rich draft snapshots.
//! PT-BR: O histórico de texto do GTK não inclui paintables; mantém snapshots limitados dos rascunhos com conteúdo visual.
use super::*;
#[derive(Clone)]
struct Snapshot {value:DisplayText,caret:i32,bound:i32}
#[derive(Clone,Copy,PartialEq,Eq)]
pub(super) enum Kind {Insert,Delete,Atomic}
#[derive(Default)]
pub(super) struct History {
    undo:std::collections::VecDeque<Snapshot>,redo:Vec<Snapshot>,
    depth:u32,recorded:bool,last:Option<(Kind,i32,Instant)>,
    actions:Option<(gtk::glib::WeakRef<gtk::gio::SimpleAction>,gtk::glib::WeakRef<gtk::gio::SimpleAction>)>,
}
impl History {
    fn update(&self){if let Some((undo,redo))=&self.actions{if let Some(action)=undo.upgrade(){action.set_enabled(!self.undo.is_empty());}if let Some(action)=redo.upgrade(){action.set_enabled(!self.redo.is_empty());}}}
    fn limit(&mut self){
        while self.undo.len()>128||self.undo.iter().chain(self.redo.iter()).map(|s|s.value.text.len()+s.value.spans.len()*std::mem::size_of::<Span>()+s.value.emojis.iter().map(|e|e.code.len()+std::mem::size_of::<mentions::InlineEmoji>()).sum::<usize>()).sum::<usize>()>512<<10{
            if self.undo.pop_front().is_none(){self.redo.clear();break;}
        }
    }
}
impl Composer {
    fn snapshot(&self,entry:&gtk::TextView)->Snapshot {
        let buffer=entry.buffer();Snapshot{value:self.value.borrow().clone(),caret:entry.position(),bound:buffer.iter_at_mark(&buffer.selection_bound()).offset()}
    }
    pub(super) fn remember(&self,entry:&gtk::TextView,kind:Kind,next:i32){
        if self.updating.get(){return;}
        let snapshot=self.snapshot(entry);let mut history=self.history.borrow_mut();
        if history.depth>0&&history.recorded{return;}
        history.recorded=true;
        let grouped=kind!=Kind::Atomic&&snapshot.caret==snapshot.bound&&history.last.is_some_and(|(old,caret,at)|old==kind&&caret==snapshot.caret&&at.elapsed().as_millis()<750);
        if !grouped{history.undo.push_back(snapshot);}
        history.last=Some((kind,next,Instant::now()));history.redo.clear();history.limit();history.update();
    }
    pub(super) fn clear_history(&self){
        let mut history=self.history.borrow_mut();history.undo.clear();history.redo.clear();history.last=None;history.update();
    }
    fn restore_history(&self,entry:&gtk::TextView,redo:bool){
        if !entry.is_editable()||!entry.is_sensitive()||self.is_composing(){return;}
        let current=self.snapshot(entry);let mut history=self.history.borrow_mut();
        let previous=if redo{history.redo.pop()}else{history.undo.pop_back()};
        let Some(previous)=previous else{return;};
        if redo{history.undo.push_back(current);}else{history.redo.push(current);}
        history.last=None;history.limit();history.update();drop(history);
        *self.value.borrow_mut()=previous.value;self.refresh_at(entry,previous.caret);
        entry.select_region(previous.bound,previous.caret);entry.buffer().emit_by_name::<()>("changed",&[]);
    }
    pub(super) fn connect_history(&self,entry:&gtk::TextView){
        let buffer=entry.buffer();buffer.set_enable_undo(false);
        let composer=self.clone();buffer.connect_begin_user_action(move |_|{if !composer.updating.get(){let mut history=composer.history.borrow_mut();if history.depth==0{history.recorded=false;}history.depth+=1;}});
        let composer=self.clone();buffer.connect_end_user_action(move |_|{if !composer.updating.get(){let mut history=composer.history.borrow_mut();history.depth=history.depth.saturating_sub(1);}});
        let group=gtk::gio::SimpleActionGroup::new();let undo=gtk::gio::SimpleAction::new("undo",None);let redo=gtk::gio::SimpleAction::new("redo",None);
        for (action,is_redo) in [(&undo,false),(&redo,true)]{let composer=self.clone();let weak=entry.downgrade();action.connect_activate(move |_,_|{if let Some(entry)=weak.upgrade(){composer.restore_history(&entry,is_redo);}});group.add_action(action);}
        self.history.borrow_mut().actions=Some((undo.downgrade(),redo.downgrade()));self.history.borrow().update();entry.insert_action_group("draft",Some(&group));
        let menu=gtk::gio::Menu::new();menu.append(Some("Desfazer"),Some("draft.undo"));menu.append(Some("Refazer"),Some("draft.redo"));entry.set_extra_menu(Some(&menu));
        let shortcuts=gtk::ShortcutController::new();shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
        use gtk::gdk::{Key,ModifierType as M};
        for (key,mods,redo) in [(Key::z,M::CONTROL_MASK,false),(Key::Z,M::CONTROL_MASK|M::SHIFT_MASK,true),(Key::z,M::CONTROL_MASK|M::SHIFT_MASK,true),(Key::y,M::CONTROL_MASK,true)]{
            let weak=entry.downgrade();let composer=self.clone();
            let action=gtk::CallbackAction::new(move |_,_|{
                if composer.is_composing(){return gtk::glib::Propagation::Proceed;}
                if let Some(entry)=weak.upgrade(){composer.restore_history(&entry,redo);}gtk::glib::Propagation::Stop
            });
            shortcuts.add_shortcut(gtk::Shortcut::new(Some(gtk::KeyvalTrigger::new(key,mods)),Some(action)));
        }entry.add_controller(shortcuts);
    }
}
