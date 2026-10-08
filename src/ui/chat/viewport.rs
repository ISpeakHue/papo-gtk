//! Scroll only for explicit navigation or while following the newest messages.
use super::*;
use adw::prelude::*;
use std::{cell::{Cell,RefCell},rc::Rc};

#[derive(Clone)]
pub(super) enum Position { Bottom, Message(Uuid), Unread(Uuid), Anchor{ id:Option<Uuid>,offset:f64,value:f64 } }
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
        let pending=self.pending.clone();let generation=self.generation.clone();let wheel=self.wheel.clone();let weak=scroll.downgrade();let input=sender.input_sender().clone();
        controller.connect_scroll(move |controller,_,dy|{let _=input.send(ChatMsg::CancelInitialRead);
            pending.borrow_mut().take();generation.set(generation.get().wrapping_add(1));
            let Some(scroll)=weak.upgrade()else{return gtk::glib::Propagation::Proceed;};
            if controller.unit()!=gtk::gdk::ScrollUnit::Wheel{stop_wheel(&wheel);return gtk::glib::Propagation::Proceed;}
            animate_wheel(&scroll,&wheel,dy);gtk::glib::Propagation::Stop
        });scroll.add_controller(controller);
        let click=gtk::GestureClick::new();click.set_button(1);click.set_propagation_phase(gtk::PropagationPhase::Capture);let pending=self.pending.clone();let generation=self.generation.clone();
        let input=sender.input_sender().clone();let wheel=self.wheel.clone();click.connect_pressed(move |_,_,_,_|{let _=input.send(ChatMsg::CancelInitialRead);pending.borrow_mut().take();generation.set(generation.get().wrapping_add(1));stop_wheel(&wheel);});scroll.add_controller(click);
        // GTK's automatic focus scrolling also runs during layout changes. Only
        // reveal a focused row when the user explicitly navigates with keys.
        let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pending=self.pending.clone();let generation=self.generation.clone();let weak=scroll.downgrade();let wheel=self.wheel.clone();let input=sender.input_sender().clone();
        keys.connect_key_pressed(move |_,key,_,_|{
            let reveal=matches!(key,gtk::gdk::Key::Tab|gtk::gdk::Key::ISO_Left_Tab|gtk::gdk::Key::Up|gtk::gdk::Key::Down|gtk::gdk::Key::Home|gtk::gdk::Key::End);
            if reveal||matches!(key,gtk::gdk::Key::Page_Up|gtk::gdk::Key::Page_Down){let _=input.send(ChatMsg::CancelInitialRead);
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
        if matches!(position,Position::Unread(_)|Position::Message(_)){self.following.set(false);}
        let generation=self.generation.get().wrapping_add(1);self.generation.set(generation);*self.pending.borrow_mut()=Some(position);
        // Explicit latest navigation must take effect even if an unchanged,
        // temporarily occluded history has no new allocation frame yet.
        if matches!(*self.pending.borrow(),Some(Position::Bottom)){
            let adj=scroll.vadjustment();adj.set_value((adj.upper()-adj.page_size()).max(adj.lower()));
            self.following.set(true);sender.input(ChatMsg::ViewportChanged(false));
        }
        scroll.queue_draw();
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
                Position::Unread(id)=>find(id).and_then(|row|row.compute_bounds(&list)).map_or(adj.value(),|r|f64::from(r.y())-24.0),
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

/// Snapshot the read cursor before observing history can advance it.
#[derive(Clone)]
pub(super) struct ReadBoundary{message:Option<Uuid>,at:Option<chrono::DateTime<chrono::Utc>>,unread:bool,cursor:Option<MessageCursor>,pages:usize}
impl ReadBoundary{
    pub fn new(target:&ConversationTarget)->Self{match target{
        ConversationTarget::Channel(c)=>Self{message:c.last_read_message,at:c.last_read_at,unread:c.has_unread(),cursor:None,pages:0},
        ConversationTarget::Direct(d)=>Self{message:d.last_read_message,at:d.last_read_at,unread:d.unread_count>0||d.last_message.as_ref().is_some_and(|m|Some(m.id)!=d.last_read_message),cursor:None,pages:0},
    }}
    pub fn older(&mut self,messages:&[Message],has_more:bool)->Option<MessageCursor>{
        if !self.unread||!has_more||self.pages>=20||(self.message.is_none()&&self.at.is_none()){return None;}
        if self.message.is_some_and(|id|messages.iter().any(|m|m.id==id)){return None;}
        let cursor=messages.first().map(MessageCursor::from)?;
        if self.at.is_some_and(|at|cursor.created_at<=at)||self.cursor.is_some_and(|old|(cursor.created_at,cursor.id)>=(old.created_at,old.id)){return None;}
        self.cursor=Some(cursor);self.pages+=1;Some(cursor)
    }
    pub fn position(&self,messages:&[Message])->Position{
        if !self.unread{return Position::Bottom;}
        let next=self.message.and_then(|id|messages.iter().position(|m|m.id==id)).map(|index|messages.get(index+1))
            .unwrap_or_else(||messages.iter().find(|m|self.at.is_none_or(|at|m.created_at>at)));
        next.map_or(Position::Bottom,|m|Position::Unread(m.id))
    }
}
#[cfg(test)]mod read_tests{
    use super::*;
    #[test]fn unread_search_is_serial_and_stops_on_cursor_or_bounds(){
        let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"created_at":"2026-10-08T12:00:00Z"})).unwrap();
        let mut boundary=ReadBoundary{message:Some(Uuid::new_v4()),at:None,unread:true,cursor:None,pages:0};assert!(boundary.older(&[message.clone()],true).is_some());assert!(boundary.older(&[message.clone()],true).is_none(),"identical pages cannot loop");boundary.cursor=None;boundary.message=Some(message.id);assert!(boundary.older(&[message.clone()],true).is_none());boundary.message=Some(Uuid::new_v4());boundary.pages=20;assert!(boundary.older(&[message],true).is_none());
    }
    #[test]fn unread_cursor_wins_over_read_operation_time(){
        let a:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"created_at":"2026-10-08T12:00:00Z"})).unwrap();let mut b=a.clone();b.id=Uuid::new_v4();b.created_at+=chrono::Duration::seconds(1);
        let boundary=ReadBoundary{message:Some(a.id),at:Some(b.created_at+chrono::Duration::seconds(5)),unread:true,cursor:None,pages:0};
        assert!(matches!(boundary.position(&[a.clone(),b.clone()]),Position::Unread(id) if id==b.id));
        assert!(matches!(ReadBoundary{unread:false,..boundary}.position(&[a,b]),Position::Bottom));
    }
}
