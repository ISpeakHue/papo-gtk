//! Scroll only for explicit navigation or while following the newest messages.
use super::*;
use adw::prelude::*;
use std::{cell::{Cell,RefCell},rc::Rc};

#[derive(Clone)]
pub(super) enum Position { Bottom, Message(Uuid), Unread(Uuid), Anchor{ id:Option<Uuid>,offset:f64,value:f64 } }
pub(super) struct Viewport {
    following:Rc<Cell<bool>>,pending:Rc<RefCell<Option<Position>>>,generation:Rc<Cell<u64>>,
    wheel:Rc<RefCell<Option<adw::TimedAnimation>>>,
    last_motion:Rc<Cell<Option<Instant>>>,idle_media:Rc<Cell<bool>>,
    opening:Rc<Cell<bool>>,opening_ready:Rc<Cell<bool>>,opening_callback:Rc<Cell<bool>>,
    reflow:Rc<RefCell<Option<Position>>>,media_layout:Rc<Cell<bool>>,
}
impl Default for Viewport {fn default()->Self{Self{following:Rc::new(Cell::new(true)),pending:Default::default(),generation:Default::default(),wheel:Default::default(),last_motion:Default::default(),idle_media:Default::default(),opening:Default::default(),opening_ready:Default::default(),opening_callback:Default::default(),reflow:Default::default(),media_layout:Default::default()}}}
impl Drop for Viewport{fn drop(&mut self){self.idle_media.set(false);self.media_layout.set(false);self.reflow.borrow_mut().take();stop_wheel(&self.wheel);self.last_motion.set(None);}}
impl Viewport {
    pub fn following(&self)->bool{self.following.get()}
    pub fn scrolling(&self)->bool{scrolling(&self.last_motion,&self.wheel)}
    pub fn begin_open(&self,scroll:&gtk::ScrolledWindow){
        stop_wheel(&self.wheel);self.last_motion.set(None);
        self.pending.borrow_mut().take();self.generation.set(self.generation.get().wrapping_add(1));
        self.reflow.borrow_mut().take();
        self.following.set(true);self.opening.set(true);self.opening_ready.set(false);self.opening_callback.set(false);
        // Opacity preserves mapping and allocation. Hiding the widget would
        // prevent TextView height-for-width layout from settling at all.
        scroll.set_opacity(0.0);
    }
    pub fn open_ready(&self,ready:bool)->bool{
        self.opening.get()&&self.opening_ready.replace(ready)!=ready&&ready
    }
    pub fn cancel_open(&self,scroll:&gtk::ScrolledWindow){
        if self.opening.replace(false){self.pending.borrow_mut().take();self.generation.set(self.generation.get().wrapping_add(1));}
        self.opening_ready.set(false);self.opening_callback.set(false);scroll.set_opacity(1.0);
    }
    pub fn media_when_idle(&self,sender:&ComponentSender<ChatModel>){
        if self.idle_media.replace(true){return;}
        let pending=self.idle_media.clone();let motion=self.last_motion.clone();let wheel=self.wheel.clone();let input=sender.input_sender().clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(120),move ||{
            if !pending.get(){return gtk::glib::ControlFlow::Break;}
            if scrolling(&motion,&wheel){return gtk::glib::ControlFlow::Continue;}
            pending.set(false);let _=input.send(ChatMsg::MediaViewport);gtk::glib::ControlFlow::Break
        });
    }
    pub fn media_after_layout(&self,sender:&ComponentSender<ChatModel>){
        if self.media_layout.replace(true){return;}
        let pending=self.media_layout.clone();let input=sender.input_sender().clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(80),move ||{if pending.replace(false){let _=input.send(ChatMsg::MediaViewport);}});
    }
    pub fn connect(&self,scroll:&gtk::ScrolledWindow,list:&gtk::ListBox,sender:&ComponentSender<ChatModel>){
        if let Some(view)=scroll.child().and_downcast::<gtk::Viewport>(){view.set_scroll_to_focus(false);}
        // ListBox also scrolls its cursor independently of GtkViewport.
        list.set_adjustment(None::<&gtk::Adjustment>);
        // Tick callbacks run before layout. Restoring only on a later tick
        // exposes one frame at the wrong position when a row changes height.
        // Apply the anchor after GTK allocation, before that frame is painted.
        let layout_hook:Rc<RefCell<Option<(gtk::gdk::FrameClock,gtk::glib::SignalHandlerId)>>>=Default::default();
        let hook=layout_hook.clone();let rows=list.downgrade();let pending=self.pending.clone();let reflow=self.reflow.clone();let wheel=self.wheel.clone();
        scroll.connect_map(move |scroll|{
            if hook.borrow().is_some(){return;}
            let Some(clock)=scroll.frame_clock()else{return;};let weak=scroll.downgrade();let rows=rows.clone();let pending=pending.clone();let reflow=reflow.clone();let wheel=wheel.clone();
            let id=clock.connect_local("layout",true,move |_|{
                let(Some(scroll),Some(list))=(weak.upgrade(),rows.upgrade())else{return None;};let adj=scroll.vadjustment();
                let correction=reflow.borrow_mut().take();
                if let Some(Position::Anchor{id:Some(id),offset,value})=correction{
                    if let Some(bounds)=find_row(&list,id).and_then(|row|row.compute_bounds(&list)){
                        let shift=f64::from(bounds.y())-offset-value;
                        if shift.abs()>0.5{let max=(adj.upper()-adj.page_size()).max(adj.lower());
                            if let Some(animation)=wheel.borrow().as_ref().filter(|a|a.state()==adw::AnimationState::Playing){animation.set_value_from((animation.value_from()+shift).clamp(adj.lower(),max));animation.set_value_to((animation.value_to()+shift).clamp(adj.lower(),max));}
                            adj.set_value((adj.value()+shift).clamp(adj.lower(),max));
                        }
                    }
                }
                if let Some(position)=pending.borrow().as_ref(){set_position(&adj,&list,position,false);}
                None
            });*hook.borrow_mut()=Some((clock,id));
        });
        scroll.connect_unmap(move |_|{if let Some((clock,id))=layout_hook.borrow_mut().take(){clock.disconnect(id);}});
        let pending=self.pending.clone();let following=self.following.clone();let s=sender.clone();let media_pending=Rc::new(Cell::new(false));let extent=Rc::new(Cell::new((scroll.vadjustment().upper(),scroll.vadjustment().page_size())));let seen=extent.clone();
        let motion=self.last_motion.clone();let opening=self.opening.clone();scroll.vadjustment().connect_value_changed(move |adj|{
            if !media_pending.replace(true){let s=s.clone();let scheduled=media_pending.clone();gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(16),move ||{scheduled.set(false);let _=s.input_sender().send(ChatMsg::MediaViewport);});}
            let previous=seen.replace((adj.upper(),adj.page_size()));
            // Adjustment.configure can emit value-changed while resizing the
            // viewport. That is not a reader scrolling away from the bottom.
            if opening.get()||pending.borrow().is_some()||previous!=(adj.upper(),adj.page_size()){return;}
            motion.set(Some(Instant::now()));
            let at_bottom=(adj.upper()-adj.page_size()-adj.value()).max(0.0)<=24.0;
            if following.replace(at_bottom)!=at_bottom||adj.value()<=120.0{let _=s.input_sender().send(ChatMsg::ViewportChanged(!at_bottom));}
        });
        // Layout can finish long after a send or media update (especially for
        // TextView height-for-width). Follow the actual extent, not a frame count.
        let pending=self.pending.clone();let following=self.following.clone();let weak=scroll.downgrade();
        scroll.vadjustment().connect_changed(move |adj|{
            if following.get() && pending.borrow().as_ref().is_none_or(|p|matches!(p,Position::Bottom)){
                adj.set_value((adj.upper()-adj.page_size()).max(adj.lower()));
                if let Some(scroll)=weak.upgrade(){scroll.queue_draw();}
            }
        });
        let controller=gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let pending=self.pending.clone();let generation=self.generation.clone();let wheel=self.wheel.clone();let weak=scroll.downgrade();let input=sender.input_sender().clone();
        let motion=self.last_motion.clone();controller.connect_scroll(move |controller,_,dy|{let _=input.send(ChatMsg::CancelInitialRead);
            motion.set(Some(Instant::now()));
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
                if let Some(bounds)=row.compute_bounds(list){if f64::from(bounds.y())<=value+scroll.vadjustment().page_size()&&f64::from(bounds.y()+bounds.height())>value{return Position::Anchor{id:Some(id),offset:f64::from(bounds.y())-value,value};}}
            }child=row.next_sibling();
        }Position::Anchor{id:None,offset:0.0,value}
    }
    pub fn restore(&self,scroll:&gtk::ScrolledWindow,list:&gtk::ListBox,position:Position,sender:&ComponentSender<ChatModel>){
        // A virtualization/pagination reflow is not navigation. Compensate only
        // its geometry shift, adding it to the current position and animation
        // endpoints so an ongoing wheel gesture keeps its remaining travel.
        if self.scrolling(){if matches!(position,Position::Anchor{id:Some(_),..}){
            *self.reflow.borrow_mut()=Some(position);list.queue_resize();scroll.queue_draw();return;
        }}
        self.reflow.borrow_mut().take();
        stop_wheel(&self.wheel);
        if matches!(position,Position::Unread(_)|Position::Message(_)){self.following.set(false);}
        // Media completions update the shared target without repeatedly
        // restarting the opening frame budget or adding more frame callbacks.
        let reusing=self.opening.get()&&self.opening_callback.get();
        let generation=if reusing{self.generation.get()}else{self.generation.get().wrapping_add(1)};
        self.generation.set(generation);*self.pending.borrow_mut()=Some(position);
        // Explicit latest navigation must take effect even if an unchanged,
        // temporarily occluded history has no new allocation frame yet.
        if matches!(*self.pending.borrow(),Some(Position::Bottom)){
            let adj=scroll.vadjustment();adj.set_value((adj.upper()-adj.page_size()).max(adj.lower()));
            self.following.set(true);if !self.opening.get(){sender.input(ChatMsg::ViewportChanged(false));}
        }
        list.queue_resize();scroll.queue_draw();
        if reusing{return;}
        if self.opening.get(){self.opening_callback.set(true);}
        let pending=self.pending.clone();let current=self.generation.clone();let following=self.following.clone();let list=list.downgrade();let s=sender.clone();let frames=Cell::new(0);
        let opening=self.opening.clone();let ready=self.opening_ready.clone();let active=self.opening_callback.clone();let geometry=Cell::new(None);let stable=Cell::new(0);let reported_bottom=Cell::new(None);
        scroll.add_tick_callback(move |scroll,_|{
            if current.get()!=generation{return gtk::glib::ControlFlow::Break;}
            frames.set(frames.get()+1);if frames.get()<2{return gtk::glib::ControlFlow::Continue;}
            let Some(position)=pending.borrow().clone()else{return gtk::glib::ControlFlow::Break;};
            let Some(list)=list.upgrade()else{pending.borrow_mut().take();return gtk::glib::ControlFlow::Break;};let adj=scroll.vadjustment();
            set_position(&adj,&list,&position,true);
            // Draw using the post-allocation scroll transform. A pre-layout
            // invalidation can otherwise leave the newest row clipped until
            // the next input event, especially with height-for-width text.
            list.queue_draw();scroll.queue_draw();
            if let Some(row)=list.last_child(){row.queue_draw();}
            // TextView may finish height-for-width layout on a later frame.
            // Keep the same anchor through that allocation; real input cancels it.
            if opening.get(){
                // Do not present an intermediate page while finding the unread
                // boundary. The next accepted history response restarts this.
                if !ready.get(){active.set(false);return gtk::glib::ControlFlow::Break;}
                let bottom=(adj.upper()-adj.page_size()-adj.value()).max(0.0)<=24.0;
                if reported_bottom.replace(Some(bottom))!=Some(bottom){let _=s.input_sender().send(ChatMsg::ViewportChanged(!bottom));stable.set(0);}
                let extent=(list.width(),list.height(),adj.upper(),adj.page_size(),adj.value());
                stable.set(if geometry.replace(Some(extent))==Some(extent){stable.get()+1}else{0});
                // Validate actual layout rather than revealing at an arbitrary
                // frame number. A bound keeps continuous media updates from
                // delaying presentation indefinitely; downloads are not awaited.
                if (adj.page_size()<=0.0||stable.get()<2)&&frames.get()<12{return gtk::glib::ControlFlow::Continue;}
                opening.set(false);active.set(false);scroll.set_opacity(1.0);
            }else if frames.get()<4{return gtk::glib::ControlFlow::Continue;}
            pending.borrow_mut().take();let bottom=(adj.upper()-adj.page_size()-adj.value()).max(0.0)<=24.0;following.set(bottom);let _=s.input_sender().send(ChatMsg::ViewportChanged(!bottom));gtk::glib::ControlFlow::Break
        });
    }
}

fn find_row(list:&gtk::ListBox,id:Uuid)->Option<gtk::Widget>{
    let name=format!("message-{id}");let mut row=list.first_child();while let Some(w)=row{if w.widget_name()==name{return Some(w);}row=w.next_sibling();}None
}
fn set_position(adj:&gtk::Adjustment,list:&gtk::ListBox,position:&Position,focus:bool){
    let value=match position{
        Position::Bottom=>adj.upper()-adj.page_size(),
        Position::Message(id)|Position::Unread(id)=>find_row(list,*id).and_then(|row|{if focus&&matches!(position,Position::Message(_)){row.grab_focus();}row.compute_bounds(list)}).map_or(adj.value(),|r|f64::from(r.y())-24.0),
        Position::Anchor{id,offset,value}=>id.and_then(|id|find_row(list,id)).and_then(|row|row.compute_bounds(list)).map_or(*value,|r|f64::from(r.y())-offset),
    };
    adj.set_value(value.clamp(adj.lower(),(adj.upper()-adj.page_size()).max(adj.lower())));
}

fn stop_wheel(wheel:&RefCell<Option<adw::TimedAnimation>>){if let Some(animation)=wheel.borrow_mut().take(){animation.pause();}}
fn scrolling(motion:&Cell<Option<Instant>>,wheel:&RefCell<Option<adw::TimedAnimation>>)->bool{
    motion.get().is_some_and(|at|at.elapsed()<std::time::Duration::from_millis(120))||wheel.borrow().as_ref().is_some_and(|a|a.state()==adw::AnimationState::Playing)
}
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

#[cfg(test)]pub(crate) fn exercise_media_scroll(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,pump,until};use performance::settle;
    let created=chrono::Utc::now();let channel=Uuid::new_v4();let user=Uuid::new_v4();let key=Uuid::new_v4();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"scrolling images","created_at":created})).unwrap();
    let mut messages:Vec<Message>=(0..100).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Scroll message {i}"),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect();
    messages[48].attachments=Some(vec![serde_json::from_value(serde_json::json!({"id":key,"mime_type":"image/png","original_file_name":"later.png","size_bytes":500,"created_at":created})).unwrap()]);
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(900).default_height(650).content(chat.widget()).build();window.present();settle(context);
    gtk::Settings::default().unwrap().set_gtk_enable_animations(true);
    let all=descendants(chat.widget().upcast_ref());let scroll=all.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();let list=all.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|w|w.has_css_class("papo-chat-history"))).unwrap().clone();let adj=scroll.vadjustment();
    let row=chat.model().rendered[&format!("message-{}",messages[48].id)].1.clone();adj.set_value(f64::from(row.compute_bounds(&list).unwrap().y())-100.0);settle(context);
    let media=find_media_box(row.upcast_ref()).unwrap();let placeholder=media.first_child().unwrap();
    // Keep responses deterministic: the pending fixture completion below
    // supplies the image; an API client lets us observe hydration work itself.
    chat.state().get_mut().model.actions.api=Some(crate::api::ApiClient::new("http://127.0.0.1:9").unwrap());
    let passes=chat.model().render_passes;let hydrations=chat.model().media_hydrations;let from=adj.value();
    {let state=chat.model();state.viewport.last_motion.set(Some(Instant::now()));animate_wheel(&scroll,&state.viewport.wheel,-6.0);}
    let animation=chat.model().viewport.wheel.borrow().as_ref().unwrap().clone();let destination=animation.value_to();assert!(destination<from-100.0);assert_eq!(animation.state(),adw::AnimationState::Playing);
    let epoch=chat.model().transfers.epoch;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(key,token);
    chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,message:messages[48].id,key,token,result:Some(crate::media::PreparedImage{width:640,height:160,pixels:vec![255;640*160*4]})}));
    let start=Instant::now();while start.elapsed()<std::time::Duration::from_millis(70){pump(context);std::thread::sleep(std::time::Duration::from_millis(3));}
    assert!(chat.model().transfers.textures.contains_key(&key));assert!(chat.model().media_render_pending.is_some());assert_eq!(chat.model().render_passes,passes,"decoding during scrolling must not rebuild media");assert_eq!(chat.model().media_hydrations,hydrations,"scroll ticks must not rescan/hydrate the history");assert_eq!(media.first_child().unwrap(),placeholder);assert_eq!(animation.state(),adw::AnimationState::Playing,"image completion must not cancel wheel motion");assert!(adj.value()<from-20.0,"scroll frames keep advancing while the picture waits");
    // Structural updates also preserve the same running animation. Reactions
    // do not touch the pending image row, so its placeholder still waits.
    chat.emit(ChatMsg::ApplyChange(Change::Reaction(messages[30].id,crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:1})));pump(context);assert_eq!(animation.state(),adw::AnimationState::Playing);assert_eq!(media.first_child().unwrap(),placeholder,"unrelated live changes must not flush deferred pictures");
    until(context,||animation.state()==adw::AnimationState::Finished);let anchor=chat.model().viewport.capture(&scroll,&list);let end=adj.value();assert!((end-animation.value_to()).abs()<3.0,"the gesture reaches its destination after compensating any layout shift: {end} vs {}",animation.value_to());
    until(context,||chat.model().media_render_pending.is_none());settle(context);assert!(descendants(media.upcast_ref()).iter().any(|w|w.is::<gtk::Picture>()),"the deferred image appears automatically after scrolling pauses");
    if let Position::Anchor{id:Some(id),offset,..}=anchor{let row=chat.model().rendered[&format!("message-{id}")].1.clone();assert!((f64::from(row.compute_bounds(&list).unwrap().y())-adj.value()-offset).abs()<3.0,"idle image layout preserves the reading anchor");}
    // Pixel scrolling and scrollbar drags change the adjustment without the
    // custom wheel animation. Keep deferring throughout that continuous motion.
    let displayed=media.first_child().unwrap();let passes=chat.model().render_passes;let hydrations=chat.model().media_hydrations;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(key,token);adj.set_value(adj.value()-10.0);
    chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,message:messages[48].id,key,token,result:Some(crate::media::PreparedImage{width:320,height:80,pixels:vec![180;320*80*4]})}));
    for _ in 0..6{adj.set_value(adj.value()-10.0);let start=Instant::now();while start.elapsed()<std::time::Duration::from_millis(30){pump(context);std::thread::sleep(std::time::Duration::from_millis(3));}assert_eq!(media.first_child().unwrap(),displayed);}
    assert_eq!(chat.model().render_passes,passes);assert_eq!(chat.model().media_hydrations,hydrations);until(context,||chat.model().media_render_pending.is_none());settle(context);assert_ne!(media.first_child().unwrap(),displayed,"a continuous pixel gesture also flushes its image after pausing");
    // Explicit navigation still takes priority over scrolling and old results.
    {let state=chat.model();state.viewport.last_motion.set(Some(Instant::now()));animate_wheel(&scroll,&state.viewport.wheel,-2.0);}let running=chat.model().viewport.wheel.borrow().as_ref().unwrap().clone();chat.emit(ChatMsg::Latest);pump(context);assert_ne!(running.state(),adw::AnimationState::Playing);settle(context);assert!(chat.model().viewport.following());
    adj.set_value(adj.value()-10.0);let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(key,token);chat.emit(ChatMsg::Transfer(TransferMsg::ImageReady{epoch,message:messages[48].id,key,token,result:Some(crate::media::PreparedImage{width:32,height:32,pixels:vec![100;32*32*4]})}));pump(context);assert!(chat.model().media_render_pending.is_some());chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});settle(context);assert!(chat.model().transfers.textures.is_empty());assert!(!descendants(list.upcast_ref()).iter().any(|w|w.is::<gtk::Picture>()),"an idle flush cannot restore revoked images");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}
