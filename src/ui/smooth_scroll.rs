//! Shared mouse-wheel easing; pixel/touchpad scrolling stays native to GTK.
use adw::prelude::*;
use std::{cell::RefCell, rc::Rc};

pub(super) type Wheel = Rc<RefCell<Option<adw::TimedAnimation>>>;

pub(super) fn stop_wheel(wheel: &RefCell<Option<adw::TimedAnimation>>) {
    if let Some(animation) = wheel.borrow_mut().take() { animation.pause(); }
}

pub(super) fn animate_wheel(
    scroll: &gtk::ScrolledWindow,
    wheel: &RefCell<Option<adw::TimedAnimation>>,
    dy: f64,
) {
    if !dy.is_finite() || dy == 0.0 { return; }
    let adj = scroll.vadjustment();
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
