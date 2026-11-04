//! EN-US: Shared mouse-wheel easing; pixel/touchpad scrolling stays native to GTK.
//! PT-BR: Suavização compartilhada para a roda do mouse; a rolagem em pixels e por touchpad continua nativa do GTK.
use adw::prelude::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[cfg(test)] mod tests;
#[cfg(test)] pub(crate) use tests::exercise;
#[cfg(test)] pub(crate) use tests::exercise_pane;
pub(super) const VERTICAL_CONTROLLER:&str="papo-smooth-scroll-vertical";
const WINDOW_CONTROLLER:&str="papo-window-smooth-scroll";

pub(super) type Wheel = Rc<RefCell<Option<adw::TimedAnimation>>>;

pub(super) fn stop_wheel(wheel: &RefCell<Option<adw::TimedAnimation>>) {
    if let Some(animation) = wheel.borrow_mut().take() { animation.pause(); }
}

pub(super) fn animate_wheel(
    scroll: &gtk::ScrolledWindow,
    wheel: &RefCell<Option<adw::TimedAnimation>>,
    dy: f64,
) {
    animate_axis(scroll,wheel,gtk::Orientation::Vertical,dy);
}

fn animate_axis(scroll:&gtk::ScrolledWindow,wheel:&RefCell<Option<adw::TimedAnimation>>,axis:gtk::Orientation,dy:f64){
    if !dy.is_finite() || dy == 0.0 { return; }
    let adj = adjustment(scroll,axis);
    let from = adj.value();
    let previous = wheel.borrow().as_ref()
        .filter(|a| a.state() == adw::AnimationState::Playing)
        .map_or(from, |a| a.value_to());
    let to = (previous + dy * adj.page_size().max(1.0).powf(2.0 / 3.0))
        .clamp(adj.lower(), (adj.upper() - adj.page_size()).max(adj.lower()));
    stop_wheel(wheel);
    let target = adw::CallbackAnimationTarget::new(move |value| adj.set_value(value));
    let animation = adw::TimedAnimation::new(scroll, from, to, 160, target);
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.play();
    *wheel.borrow_mut() = Some(animation);
}

pub(super) fn connect(scroll: &gtk::ScrolledWindow, wheel: &Wheel) {
    let controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    controller.set_name(Some(VERTICAL_CONTROLLER));
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = scroll.downgrade();
    let state = wheel.clone();
    controller.connect_scroll(move |controller, _, dy| {
        if controller.unit() != gtk::gdk::ScrollUnit::Wheel {
            stop_wheel(&state);
            return gtk::glib::Propagation::Proceed;
        }
        let Some(scroll) = weak.upgrade() else { return gtk::glib::Propagation::Proceed; };
        animate_wheel(&scroll, &state, dy);
        gtk::glib::Propagation::Stop
    });
    scroll.add_controller(controller);

    // Do not claim clicks or navigation keys: row activation, keyboard focus
    // and scrollbar dragging continue through their standard GTK handlers.
    let click = gtk::GestureClick::new();
    click.set_button(1);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let state = wheel.clone();
    click.connect_pressed(move |_, _, _, _| stop_wheel(&state));
    scroll.add_controller(click);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let state = wheel.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if matches!(key, gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab
            | gtk::gdk::Key::Up | gtk::gdk::Key::Down | gtk::gdk::Key::Home
            | gtk::gdk::Key::End | gtk::gdk::Key::Page_Up | gtk::gdk::Key::Page_Down
            | gtk::gdk::Key::space) {
            stop_wheel(&state);
        }
        gtk::glib::Propagation::Proceed
    });
    scroll.add_controller(keys);
    let state = wheel.clone();
    scroll.connect_unmap(move |_| stop_wheel(&state));
}

struct ScrollState {
    widget:gtk::glib::WeakRef<gtk::ScrolledWindow>,
    vertical:Wheel,
    horizontal:Wheel,
    unmap:Option<gtk::glib::SignalHandlerId>,
}
impl Drop for ScrollState{
    fn drop(&mut self){
        stop_wheel(&self.vertical);stop_wheel(&self.horizontal);
        if let (Some(widget),Some(handler))=(self.widget.upgrade(),self.unmap.take()){widget.disconnect(handler);}
    }
}
type WindowWheels=Rc<RefCell<HashMap<usize,ScrollState>>>;

fn adjustment(scroll:&gtk::ScrolledWindow,axis:gtk::Orientation)->gtk::Adjustment{
    if axis==gtk::Orientation::Vertical{scroll.vadjustment()}else{scroll.hadjustment()}
}

fn wheel_delta(unit:gtk::gdk::ScrollUnit,modifiers:gtk::gdk::ModifierType,dx:f64,dy:f64)->Option<(gtk::Orientation,f64)>{
    if unit!=gtk::gdk::ScrollUnit::Wheel || crate::platform::shortcuts::primary_pressed(modifiers) || !dx.is_finite() || !dy.is_finite(){return None;}
    if modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK)&&dx==0.0&&dy!=0.0{return Some((gtk::Orientation::Horizontal,dy));}
    if dx.abs()>dy.abs(){Some((gtk::Orientation::Horizontal,dx))}else if dy!=0.0{Some((gtk::Orientation::Vertical,dy))}else{None}
}

fn can_scroll(scroll:&gtk::ScrolledWindow,axis:gtk::Orientation,delta:f64)->bool{
    let policy=if axis==gtk::Orientation::Vertical{scroll.vscrollbar_policy()}else{scroll.hscrollbar_policy()};
    if policy==gtk::PolicyType::Never{return false;}
    let adj=adjustment(scroll,axis);
    if delta>0.0{adj.value()<(adj.upper()-adj.page_size()).max(adj.lower())-0.5}else{adj.value()>adj.lower()+0.5}
}

fn destination(mut picked:Option<gtk::Widget>,axis:gtk::Orientation,delta:f64)->Option<gtk::ScrolledWindow>{
    while let Some(widget)=picked{
        // EN-US: Native value controls keep their own wheel gestures; nested panes yield at their boundaries.
        // PT-BR: Controles de valor mantêm seus gestos nativos; painéis aninhados cedem a rolagem nos limites.
        if widget.is::<gtk::Scale>()||widget.is::<gtk::SpinButton>()||widget.is::<gtk::DropDown>(){return None;}
        if let Some(scroll)=widget.downcast_ref::<gtk::ScrolledWindow>().filter(|s|can_scroll(s,axis,delta)){return Some(scroll.clone());}
        picked=widget.parent();
    }
    None
}

fn custom_vertical(scroll:&gtk::ScrolledWindow,axis:gtk::Orientation)->bool{
    axis==gtk::Orientation::Vertical && scroll.observe_controllers().iter::<gtk::glib::Object>()
        .filter_map(Result::ok).filter_map(|c|c.downcast::<gtk::EventControllerScroll>().ok())
        .any(|c|c.name().as_deref()==Some(VERTICAL_CONTROLLER))
}

fn stop_window(states:&WindowWheels){
    let removed=std::mem::take(&mut *states.borrow_mut());
    for state in removed.values(){stop_wheel(&state.vertical);stop_wheel(&state.horizontal);}
}

fn animate_window(states:&WindowWheels,scroll:&gtk::ScrolledWindow,axis:gtk::Orientation,delta:f64){
    let key=scroll.as_ptr() as usize;
    let weak=Rc::downgrade(states);
    let wheel={
        let mut states=states.borrow_mut();
        states.retain(|_,state|state.widget.upgrade().is_some());
        let state=states.entry(key).or_insert_with(||{
            let unmap=scroll.connect_unmap(move |_|{
                if let Some(states)=weak.upgrade(){
                    let removed=states.borrow_mut().remove(&key);
                    drop(removed);
                }
            });
            ScrollState{widget:scroll.downgrade(),vertical:Default::default(),horizontal:Default::default(),unmap:Some(unmap)}
        });
        if axis==gtk::Orientation::Vertical{state.vertical.clone()}else{state.horizontal.clone()}
    };
    animate_axis(scroll,&wheel,axis,delta);
}

fn observe_window(window:&gtk::Window){
    let states=WindowWheels::default();
    // EN-US: Keep the original wheel modifiers/direction; positionless GDK events use the pointer's own surface below.
    // PT-BR: Preserva modificadores/direção da roda; eventos GDK sem posição usam a superfície do próprio ponteiro abaixo.
    let controller=gtk::EventControllerLegacy::new();
    controller.set_name(Some(WINDOW_CONTROLLER));
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    controller.set_propagation_limit(gtk::PropagationLimit::None);
    let wheels=states.clone();
    controller.connect_event(move |_,event|{
        let Some(wheel)=event.downcast_ref::<gtk::gdk::ScrollEvent>()else{return gtk::glib::Propagation::Proceed;};
        #[cfg(test)]if std::env::var_os("PAPO_SCROLL_DIAGNOSTICS").is_some(){eprintln!("Wheel metadata: direction={:?}, unit={:?}, delta={:?}, position={:?}, modifiers={:?}",wheel.direction(),wheel.unit(),wheel.deltas(),event.position(),event.modifier_state());}
        let (dx,dy)=match wheel.direction(){
            gtk::gdk::ScrollDirection::Up=>(0.0,-1.0),gtk::gdk::ScrollDirection::Down=>(0.0,1.0),
            gtk::gdk::ScrollDirection::Left=>(-1.0,0.0),gtk::gdk::ScrollDirection::Right=>(1.0,0.0),
            gtk::gdk::ScrollDirection::Smooth=>wheel.deltas(),_=>return gtk::glib::Propagation::Proceed,
        };
        let Some((axis,delta))=wheel_delta(wheel.unit(),event.modifier_state(),dx,dy)else{
            stop_window(&wheels);return gtk::glib::Propagation::Proceed;
        };
        let location=event.position().zip(event.surface()).map(|((x,y),s)|(s,x,y)).or_else(||{
            let device=event.seat().and_then(|s|s.pointer()).or_else(||event.device())?;
            let (surface,x,y)=device.surface_at_position();
            #[cfg(test)]if std::env::var_os("PAPO_SCROLL_DIAGNOSTICS").is_some(){eprintln!("Wheel pointer fallback: device={}, surface={}, position=({x},{y})",device.type_().name(),surface.is_some());}
            Some((surface?,x,y))
        });
        let Some((surface,x,y))=location else{return gtk::glib::Propagation::Proceed;};
        let Some(native)=gtk::Native::for_surface(&surface)else{return gtk::glib::Propagation::Proceed;};
        let (tx,ty)=native.surface_transform();
        let picked=native.upcast::<gtk::Widget>().pick(x+tx,y+ty,gtk::PickFlags::DEFAULT);
        #[cfg(test)]if std::env::var_os("PAPO_SCROLL_DIAGNOSTICS").is_some(){eprintln!("Wheel hit: position=({x},{y}), transform=({tx},{ty}), widget={:?}",picked.as_ref().map(|w|w.type_().name()));}
        let Some(scroll)=destination(picked,axis,delta)else{return gtk::glib::Propagation::Proceed;};
        // EN-US: Chat anchoring and member recycling already own a compatible wheel controller; never animate twice.
        // PT-BR: A ancoragem do chat e a lista de membros já têm um controlador compatível; nunca anima duas vezes.
        if custom_vertical(&scroll,axis){return gtk::glib::Propagation::Proceed;}
        animate_window(&wheels,&scroll,axis,delta);
        gtk::glib::Propagation::Stop
    });
    window.add_controller(controller);
    let click=gtk::GestureClick::new();click.set_button(0);click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let wheels=states.clone();click.connect_pressed(move |_,_,_,_|stop_window(&wheels));window.add_controller(click);
    let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let wheels=states.clone();keys.connect_key_pressed(move |_,key,_,_|{
        if matches!(key,gtk::gdk::Key::Tab|gtk::gdk::Key::ISO_Left_Tab|gtk::gdk::Key::Up|gtk::gdk::Key::Down|gtk::gdk::Key::Home|gtk::gdk::Key::End|gtk::gdk::Key::Page_Up|gtk::gdk::Key::Page_Down|gtk::gdk::Key::space){stop_window(&wheels);}
        gtk::glib::Propagation::Proceed
    });window.add_controller(keys);
    window.connect_unmap(move |_|stop_window(&states));
}

// EN-US: One lazy dispatcher per window covers current/future dialogs and popup content without watching every row.
// PT-BR: Um despachante sob demanda por janela cobre diálogos e popups presentes/futuros sem observar cada linha.
pub(super) fn install(){
    for widget in gtk::Window::list_toplevels(){if let Ok(window)=widget.downcast::<gtk::Window>(){observe_window(&window);}}
    gtk::Window::toplevels().connect_items_changed(|list,position,_,added|{
        for index in position..position+added{if let Some(window)=list.item(index).and_downcast::<gtk::Window>(){observe_window(&window);}}
    });
}
