//! Mapped wheel regressions for both ordinary and recycled member lists.
use super::*;
use adw::prelude::*;
use crate::ui::chat::actions::tests::{pump,until};

pub(super) fn exercise(
    context:&gtk::glib::MainContext,
    list:&Controller<UserListModel>,
    window:&gtk::Window,
    scroll:&gtk::ScrolledWindow,
) {
    let settings=gtk::Settings::default().unwrap();
    let animations=settings.is_gtk_enable_animations();
    settings.set_gtk_enable_animations(true);
    let adj=scroll.vadjustment();adj.set_value(0.0);
    let controllers=scroll.observe_controllers();
    let wheel=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerScroll>()).unwrap();
    assert_eq!(wheel.propagation_phase(),gtk::PropagationPhase::Capture);
    let step=adj.page_size().max(1.0).powf(2.0/3.0);
    let handled:bool=wheel.emit_by_name("scroll",&[&0.0f64,&1.0f64]);
    assert!(handled,"wheel events must not also take GTK's immediate scroll path");
    let animation=list.model().wheel.borrow().as_ref().unwrap().clone();
    assert_eq!(animation.duration(),160,"member and chat wheel timing must match");
    assert_eq!(animation.easing(),adw::Easing::EaseOutCubic);
    assert_eq!(adj.value(),0.0,"wheel input starts an animation instead of jumping immediately");
    // Keep the mapped software-rendering fixture alive across several frames.
    animation.set_duration(1000);
    until(context,||adj.value()>0.0);
    assert!(adj.value()<step,"wheel scrolling must expose intermediate positions");
    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&2.0f64]);
    assert_ne!(animation.state(),adw::AnimationState::Playing);
    let next=list.model().wheel.borrow().as_ref().unwrap().clone();
    assert!((next.value_to()-3.0*step).abs()<0.1,"consecutive wheel steps accumulate their destination");
    until(context,||next.state()==adw::AnimationState::Finished);
    // GtkListView aligns its adjustment to whole pixels after allocation.
    assert!((adj.value()-next.value_to()).abs()<1.0,"{} members: wheel ended at {}, target {}",list.model().users.len(),adj.value(),next.value_to());

    // A primary click (including a scrollbar drag) interrupts the wheel without
    // claiming the gesture or letting a later animation frame undo the drag.
    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&2.0f64]);
    let running=list.model().wheel.borrow().as_ref().unwrap().clone();
    let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>()).unwrap();
    click.emit_by_name::<()>("pressed",&[&1i32,&0.0f64,&0.0f64]);
    assert_ne!(running.state(),adw::AnimationState::Playing);
    assert!(list.model().wheel.borrow().is_none());
    adj.set_value(step);pump(context);assert!((adj.value()-step).abs()<1.0);

    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&2.0f64]);
    let running=list.model().wheel.borrow().as_ref().unwrap().clone();
    window.set_visible(false);pump(context);
    assert_ne!(running.state(),adw::AnimationState::Playing,"a hidden member pane must release its animation");
    assert!(list.model().wheel.borrow().is_none());
    window.present();until(context,||scroll.is_mapped());

    settings.set_gtk_enable_animations(false);
    adj.set_value(adj.upper()-adj.page_size()-10.0);
    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&1.0f64]);
    let animation=list.model().wheel.borrow().as_ref().unwrap().clone();
    until(context,||animation.state()==adw::AnimationState::Finished);
    assert!((adj.value()-(adj.upper()-adj.page_size())).abs()<1.0,"reduced motion jumps directly to the bounded destination");
    adj.set_value(0.0);
    let _:bool=wheel.emit_by_name("scroll",&[&0.0f64,&-1.0f64]);
    assert_eq!(adj.value(),0.0,"the top boundary cannot overscroll");
    settings.set_gtk_enable_animations(animations);
}
