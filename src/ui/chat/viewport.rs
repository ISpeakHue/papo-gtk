//! Scroll only for explicit navigation or while following the newest messages.
use super::*;
use adw::prelude::*;
use std::{cell::{Cell,RefCell},rc::Rc};

#[derive(Clone)]
pub(super) enum Position { Bottom, Message(Uuid), Anchor{ id:Option<Uuid>,offset:f64,value:f64 } }
pub(super) struct Viewport {
    following:Rc<Cell<bool>>,pending:Rc<RefCell<Option<Position>>>,generation:Rc<Cell<u64>>,
    wheel:Rc<RefCell<Option<adw::TimedAnimation>>>,
}
impl Default for Viewport {fn default()->Self{Self{following:Rc::new(Cell::new(true)),pending:Default::default(),generation:Default::default(),wheel:Default::default()}}}
impl Viewport {
    pub fn following(&self)->bool{self.following.get()}
    pub fn connect(&self,scroll:&gtk::ScrolledWindow,list:&gtk::ListBox,sender:&ComponentSender<ChatModel>){
        if let Some(view)=scroll.child().and_downcast::<gtk::Viewport>(){view.set_scroll_to_focus(false);}
        // ListBox also scrolls its cursor independently of GtkViewport.
        list.set_adjustment(None::<&gtk::Adjustment>);
        let pending=self.pending.clone();let following=self.following.clone();let s=sender.clone();
        scroll.vadjustment().connect_value_changed(move |adj|{
            if pending.borrow().is_some(){return;}
            let at_bottom=(adj.upper()-adj.page_size()-adj.value()).max(0.0)<=24.0;
            following.set(at_bottom);s.input(ChatMsg::ViewportChanged(!at_bottom));
        });
        let controller=gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pending=self.pending.clone();let generation=self.generation.clone();let wheel=self.wheel.clone();let weak=scroll.downgrade();
        controller.connect_scroll(move |controller,_,dy|{
            pending.borrow_mut().take();generation.set(generation.get().wrapping_add(1));
            let Some(scroll)=weak.upgrade()else{return gtk::glib::Propagation::Proceed;};
            if controller.unit()!=gtk::gdk::ScrollUnit::Wheel{stop_wheel(&wheel);return gtk::glib::Propagation::Proceed;}
            animate_wheel(&scroll,&wheel,dy);gtk::glib::Propagation::Stop
        });scroll.add_controller(controller);
        let click=gtk::GestureClick::new();click.set_button(1);click.set_propagation_phase(gtk::PropagationPhase::Capture);let pending=self.pending.clone();let generation=self.generation.clone();
        let wheel=self.wheel.clone();click.connect_pressed(move |_,_,_,_|{pending.borrow_mut().take();generation.set(generation.get().wrapping_add(1));stop_wheel(&wheel);});scroll.add_controller(click);
        // GTK's automatic focus scrolling also runs during layout changes. Only
        // reveal a focused row when the user explicitly navigates with keys.
        let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pending=self.pending.clone();let generation=self.generation.clone();let weak=scroll.downgrade();let wheel=self.wheel.clone();
        keys.connect_key_pressed(move |_,key,_,_|{
            let reveal=matches!(key,gtk::gdk::Key::Tab|gtk::gdk::Key::ISO_Left_Tab|gtk::gdk::Key::Up|gtk::gdk::Key::Down|gtk::gdk::Key::Home|gtk::gdk::Key::End);
            if reveal||matches!(key,gtk::gdk::Key::Page_Up|gtk::gdk::Key::Page_Down){
                pending.borrow_mut().take();generation.set(generation.get().wrapping_add(1));
                stop_wheel(&wheel);
                if reveal{if let Some(scroll)=weak.upgrade(){reveal_keyboard_focus(&scroll,generation.clone(),generation.get());}}
            }
            gtk::glib::Propagation::Proceed
        });scroll.add_controller(keys);
    }
    pub fn capture(&self,scroll:&gtk::ScrolledWindow,list:&gtk::ListBox)->Position{
        if let Some(position)=self.pending.borrow().clone(){return position;}
        let value=scroll.vadjustment().value();let mut child=list.first_child();
        while let Some(row)=child{
            if let Some(id)=row.widget_name().strip_prefix("message-").and_then(|id|Uuid::parse_str(id).ok()){
                if let Some(bounds)=row.compute_bounds(list){if f64::from(bounds.y()+bounds.height())>value{return Position::Anchor{id:Some(id),offset:f64::from(bounds.y())-value,value};}}
            }child=row.next_sibling();
        }Position::Anchor{id:None,offset:0.0,value}
    }
    pub fn restore(&self,scroll:&gtk::ScrolledWindow,list:&gtk::ListBox,position:Position,sender:&ComponentSender<ChatModel>){
        stop_wheel(&self.wheel);
        let generation=self.generation.get().wrapping_add(1);self.generation.set(generation);*self.pending.borrow_mut()=Some(position);
        let pending=self.pending.clone();let current=self.generation.clone();let following=self.following.clone();let list=list.downgrade();let s=sender.clone();let frames=Cell::new(0);
        scroll.add_tick_callback(move |scroll,_|{
            if current.get()!=generation{return gtk::glib::ControlFlow::Break;}
            frames.set(frames.get()+1);if frames.get()<2{return gtk::glib::ControlFlow::Continue;}
            let Some(position)=pending.borrow().clone()else{return gtk::glib::ControlFlow::Break;};
            let Some(list)=list.upgrade()else{pending.borrow_mut().take();return gtk::glib::ControlFlow::Break;};let adj=scroll.vadjustment();
            let find=|id:Uuid|{let mut row=list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){return Some(w);}row=w.next_sibling();}None};
            let value=match position{
                Position::Bottom=>adj.upper()-adj.page_size(),
                Position::Message(id)=>find(id).and_then(|row|{row.grab_focus();row.compute_bounds(&list)}).map_or(adj.value(),|r|f64::from(r.y())-24.0),
                Position::Anchor{id,offset,value}=>id.and_then(find).and_then(|row|row.compute_bounds(&list)).map_or(value,|r|f64::from(r.y())-offset),
            };
            adj.set_value(value.max(adj.lower()).min((adj.upper()-adj.page_size()).max(adj.lower())));
            // TextView may finish height-for-width layout on a later frame.
            // Keep the same anchor through that allocation; real input cancels it.
            if frames.get()<4{return gtk::glib::ControlFlow::Continue;}
            pending.borrow_mut().take();let bottom=(adj.upper()-adj.page_size()-adj.value()).max(0.0)<=24.0;following.set(bottom);s.input(ChatMsg::ViewportChanged(!bottom));gtk::glib::ControlFlow::Break
        });
    }
}

fn stop_wheel(wheel:&RefCell<Option<adw::TimedAnimation>>){if let Some(animation)=wheel.borrow_mut().take(){animation.pause();}}
fn animate_wheel(scroll:&gtk::ScrolledWindow,wheel:&RefCell<Option<adw::TimedAnimation>>,dy:f64){
    if !dy.is_finite()||dy==0.0{return;}
    let adj=scroll.vadjustment();let from=adj.value();
    let previous=wheel.borrow().as_ref().filter(|a|a.state()==adw::AnimationState::Playing).map_or(from,|a|a.value_to());
    let to=(previous+dy*adj.page_size().max(1.0).powf(2.0/3.0)).clamp(adj.lower(),(adj.upper()-adj.page_size()).max(adj.lower()));
    stop_wheel(wheel);let target=adw::CallbackAnimationTarget::new(move |value|adj.set_value(value));
    let animation=adw::TimedAnimation::new(scroll,from,to,160,target);animation.set_easing(adw::Easing::EaseOutCubic);animation.play();*wheel.borrow_mut()=Some(animation);
}

fn reveal_keyboard_focus(scroll:&gtk::ScrolledWindow,generation:Rc<Cell<u64>>,expected:u64){
    let frames=Cell::new(0);
    scroll.add_tick_callback(move |scroll,_|{
        if generation.get()!=expected{return gtk::glib::ControlFlow::Break;}
        frames.set(frames.get()+1);if frames.get()<2{return gtk::glib::ControlFlow::Continue;}
        let mut focus=scroll.root().and_then(|root|root.focus());
        while let Some(widget)=focus{
            if let Some(row)=widget.downcast_ref::<gtk::ListBoxRow>(){
                if row.is_ancestor(scroll){if let Some(list)=row.parent(){if let Some(bounds)=row.compute_bounds(&list){
                    let adj=scroll.vadjustment();let top=f64::from(bounds.y());let bottom=top+f64::from(bounds.height());
                    if top<adj.value(){adj.set_value(top);}else if bottom>adj.value()+adj.page_size(){adj.set_value(bottom-adj.page_size());}
                }}}break;
            }focus=widget.parent();
        }gtk::glib::ControlFlow::Break
    });
}
