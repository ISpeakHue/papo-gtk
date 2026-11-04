//! EN-US: Window-wide wheel routing, native input and nested/dynamic scrolling regressions.
//! PT-BR: Regressões de rolagem por janela, entrada nativa e painéis aninhados/dinâmicos.
use super::*;
use std::{cell::Cell,time::{Duration,Instant}};
use crate::ui::chat::actions::tests::{pump,until};

#[test]fn wheel_routing_preserves_pixel_input_zoom_and_horizontal_intent(){
    use gtk::{gdk::{ScrollUnit as U,ModifierType as M},Orientation as O};
    assert_eq!(wheel_delta(U::Wheel,M::empty(),0.0,1.0),Some((O::Vertical,1.0)));
    assert_eq!(wheel_delta(U::Wheel,M::empty(),-1.0,0.0),Some((O::Horizontal,-1.0)));
    assert_eq!(wheel_delta(U::Wheel,M::SHIFT_MASK,0.0,2.0),Some((O::Horizontal,2.0)));
    assert!(wheel_delta(U::Surface,M::empty(),0.0,1.0).is_none());
    assert!(wheel_delta(U::Wheel,crate::platform::shortcuts::primary_modifier(),0.0,1.0).is_none());
    assert!(wheel_delta(U::Wheel,M::empty(),0.0,0.0).is_none());
    for value in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY]{assert!(wheel_delta(U::Wheel,M::empty(),0.0,value).is_none());}
}

fn spin(context:&gtk::glib::MainContext,ms:u64){
    let end=Instant::now()+Duration::from_millis(ms);
    while Instant::now()<end{pump(context);std::thread::sleep(Duration::from_millis(5));}
}

fn native_input(window:&gtk::Window)->bool{
    gtk::prelude::WidgetExt::display(window).type_().name()=="GdkX11Display" ||
        (cfg!(windows)&&std::env::var("PAPO_NATIVE_INPUT_TEST").as_deref()==Ok("1"))
}

fn input(widget:&impl IsA<gtk::Widget>,action:&str){
    let widget=widget.as_ref();let native=widget.native().unwrap();
    let point=widget.compute_point(&native.clone().upcast::<gtk::Widget>(),&gtk::graphene::Point::new(widget.width() as f32/2.0,widget.height() as f32/2.0)).unwrap();
    let (tx,ty)=native.surface_transform();
    let directory=std::env::var_os("PAPO_TEST_HELPERS_DIR").map(std::path::PathBuf::from).unwrap_or_else(||std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/helpers"));
    let python=crate::platform::runtime::python();let mut command=std::process::Command::new(python.as_std().get_program());
    #[cfg(windows)]{
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000).arg(directory.join("windows_input.py")).arg(std::process::id().to_string()).arg("Papo smooth scrolling test").arg("--pointer");
    }
    #[cfg(not(windows))]command.arg(directory.join("scroll_input.py")).arg("Papo smooth scrolling test");
    let status=command.arg(((point.x() as f64-tx).round() as i32).to_string()).arg(((point.y() as f64-ty).round() as i32).to_string()).arg(action).status().unwrap();
    assert!(status.success(),"native wheel helper failed");
}

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    crate::ui::style::install();
    let settings=gtk::Settings::default().unwrap();let previous=settings.is_gtk_enable_animations();settings.set_gtk_enable_animations(true);
    let content=gtk::Box::new(gtk::Orientation::Vertical,0);
    let inner_content=gtk::Box::new(gtk::Orientation::Vertical,0);
    for i in 0..40{inner_content.append(&gtk::Label::new(Some(&format!("Nested row {i}"))));}
    let inner=gtk::ScrolledWindow::builder().height_request(100).hscrollbar_policy(gtk::PolicyType::Never).child(&inner_content).build();content.append(&inner);
    let button=gtk::Button::with_label("Normal button action");content.append(&button);
    let clicks=Rc::new(Cell::new(0));let count=clicks.clone();button.connect_clicked(move |_|count.set(count.get()+1));
    for i in 0..60{content.append(&gtk::Label::new(Some(&format!("Dialog row {i}"))));}
    let outer=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&content).build();
    let body=gtk::Box::new(gtk::Orientation::Vertical,0);body.append(&outer);
    let window=gtk::Window::builder().title("Papo smooth scrolling test").default_width(500).default_height(400).child(&body).build();window.present();
    until(context,||inner.is_mapped()&&outer.vadjustment().upper()>outer.vadjustment().page_size());
    assert!(window.observe_controllers().iter::<gtk::glib::Object>().filter_map(Result::ok).filter_map(|c|c.downcast::<gtk::EventControllerLegacy>().ok()).any(|c|c.name().as_deref()==Some(WINDOW_CONTROLLER)),"new dialogs automatically receive the shared dispatcher");
    assert_eq!(destination(Some(inner_content.clone().upcast()),gtk::Orientation::Vertical,1.0).unwrap(),inner);

    // EN-US: Test the shared animation independently of OS event injection, including intermediate frames and accumulation.
    // PT-BR: Testa a animação compartilhada independentemente da injeção do sistema, incluindo quadros intermediários e acumulação.
    let states=WindowWheels::default();
    let adj=inner.vadjustment();let step=adj.page_size().powf(2.0/3.0);
    animate_window(&states,&inner,gtk::Orientation::Vertical,1.0);
    let animation=states.borrow()[&(inner.as_ptr() as usize)].vertical.borrow().as_ref().unwrap().clone();
    assert_eq!(animation.duration(),160);animation.set_duration(1000);
    until(context,||adj.value()>0.0);assert!(adj.value()<step);
    animate_window(&states,&inner,gtk::Orientation::Vertical,2.0);
    let next=states.borrow()[&(inner.as_ptr() as usize)].vertical.borrow().as_ref().unwrap().clone();
    assert!((next.value_to()-3.0*step).abs()<0.1);
    stop_window(&states);assert!(states.borrow().is_empty());assert_ne!(next.state(),adw::AnimationState::Playing);
    adj.set_value(0.0);

    if native_input(&window){
        let values=Rc::new(RefCell::new(Vec::new()));let captured=values.clone();adj.connect_value_changed(move |a|captured.borrow_mut().push(a.value()));
        input(&inner,"down");until(context,||(adj.value()-step).abs()<0.75);
        assert!(values.borrow().iter().any(|value|*value>0.0&&*value<step-0.5),"a real wheel event must traverse intermediate animation frames: {:?}, step={step}",values.borrow());
        assert_eq!(outer.vadjustment().value(),0.0,"nested scrolling must not move the outer dialog");
        spin(context,200);
        adj.set_value(adj.upper()-adj.page_size());input(&inner,"down");
        until(context,||outer.vadjustment().value()>1.0);spin(context,200);
        assert!((adj.value()-(adj.upper()-adj.page_size())).abs()<0.5,"the nested boundary remains bounded while its parent scrolls: value={}, max={}",adj.value(),adj.upper()-adj.page_size());

        // This pane is created after the window was mapped; no per-pane setup is required.
        let wide=gtk::Label::new(Some("Horizontal content"));wide.set_size_request(2000,40);
        let horizontal=gtk::ScrolledWindow::builder().height_request(70).vscrollbar_policy(gtk::PolicyType::Never).child(&wide).build();body.append(&horizontal);
        until(context,||horizontal.is_mapped()&&horizontal.hadjustment().upper()>horizontal.hadjustment().page_size());
        let h=horizontal.hadjustment();input(&horizontal,"right");until(context,||h.value()>0.0);spin(context,200);
        let before=h.value();input(&horizontal,"Shift_L+down");until(context,||h.value()>before+1.0);spin(context,200);
        let zooms=Rc::new(Cell::new(0));let count=zooms.clone();
        let zoom=gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        zoom.connect_scroll(move |c,_,_|{if crate::platform::shortcuts::primary_pressed(c.current_event_state()){count.set(count.get()+1);gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});horizontal.add_controller(zoom);
        let before=h.value();input(&horizontal,"Control_L+down");until(context,||zooms.get()==1);assert!((h.value()-before).abs()<0.5,"primary-wheel zoom must not pan the viewer");

        outer.vadjustment().set_value(0.0);spin(context,50);input(&button,"click");until(context,||clicks.get()==1);
        settings.set_gtk_enable_animations(false);
        let a=outer.vadjustment();let before=a.value();let target=(before+a.page_size().powf(2.0/3.0)).min(a.upper()-a.page_size());
        input(&button,"down");until(context,||(a.value()-target).abs()<0.75);
        settings.set_gtk_enable_animations(true);
        input(&button,"down");until(context,||a.value()>target+0.1);window.set_visible(false);
        let stopped=a.value();spin(context,250);assert!((a.value()-stopped).abs()<0.5,"closing a dialog must stop queued scroll frames");
    }
    window.set_child(None::<&gtk::Widget>);window.close();pump(context);settings.set_gtk_enable_animations(previous);
}

pub(crate) fn exercise_pane(window:&gtk::Window,scroll:&gtk::ScrolledWindow,context:&gtk::glib::MainContext){
    if !native_input(window){return;}
    let title=window.title();window.set_title(Some("Papo smooth scrolling test"));
    let settings=gtk::Settings::default().unwrap();let enabled=settings.is_gtk_enable_animations();settings.set_gtk_enable_animations(true);
    until(context,||scroll.is_mapped()&&scroll.vadjustment().upper()>scroll.vadjustment().page_size()+10.0);
    let adj=scroll.vadjustment();adj.set_value(0.0);let step=adj.page_size().powf(2.0/3.0);
    let values=Rc::new(RefCell::new(Vec::new()));let captured=values.clone();
    let handler=adj.connect_value_changed(move |a|captured.borrow_mut().push(a.value()));
    input(scroll,"down");until(context,||(adj.value()-step).abs()<0.75);
    assert!(values.borrow().iter().any(|value|*value>0.0&&*value<step-0.5),"{} must ease actual wheel input",title.as_deref().unwrap_or("dialog"));
    adj.disconnect(handler);adj.set_value(0.0);settings.set_gtk_enable_animations(enabled);window.set_title(title.as_deref());
}
