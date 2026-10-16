//! Zoom changes the native picture's allocation, never decodes another copy.
use super::*;
use std::{cell::Cell,rc::Rc};

struct State {
    picture:gtk::glib::WeakRef<gtk::Picture>,scroll:gtk::glib::WeakRef<gtk::ScrolledWindow>,
    percentage:gtk::glib::WeakRef<gtk::Label>,fullscreen:Cell<bool>,fit:Cell<bool>,scale:Cell<f64>,dimensions:Cell<(i32,i32)>,generation:Cell<u64>,
}
impl State {
    fn fitted_scale(&self)->f64{
        let Some(scroll)=self.scroll.upgrade()else{return 1.0;};let(w,h)=self.dimensions.get();
        (f64::from(scroll.width().max(1))/f64::from(w.max(1))).min(f64::from(scroll.height().max(1))/f64::from(h.max(1)))
    }
    fn scale(&self)->f64{if self.fit.get(){self.fitted_scale()}else{self.scale.get()}}
    fn refresh(&self){
        let Some(picture)=self.picture.upgrade()else{return;};let Some(image)=picture.paintable()else{return;};
        let dimensions=(image.intrinsic_width().max(1),image.intrinsic_height().max(1));let old=self.dimensions.replace(dimensions);
        // A better-quality version arriving must not change the visible size
        // of an image the user is already inspecting at a chosen zoom.
        if !self.fit.get()&&old.0>0&&old!=dimensions{self.scale.set(self.scale.get()*f64::from(old.0)/f64::from(dimensions.0));}
        if self.fit.get(){picture.set_size_request(-1,-1);}else{
            let(w,h)=dimensions;picture.set_size_request((f64::from(w)*self.scale.get()).round().clamp(1.0,32768.0) as i32,(f64::from(h)*self.scale.get()).round().clamp(1.0,32768.0) as i32);
        }
        picture.set_hexpand(self.fit.get());picture.set_vexpand(self.fit.get());
        picture.set_halign(if self.fit.get(){gtk::Align::Fill}else{gtk::Align::Center});picture.set_valign(if self.fit.get(){gtk::Align::Fill}else{gtk::Align::Center});
        if let Some(label)=self.percentage.upgrade(){label.set_text(&format!("{:.0}%",self.scale()*100.0));}
        picture.set_cursor_from_name(Some(if self.fit.get(){"default"}else{"grab"}));
    }
    fn zoom(self:&Rc<Self>,scale:f64){
        if !self.fullscreen.get()||!scale.is_finite(){return;}
        let(Some(scroll),Some(picture))=(self.scroll.upgrade(),self.picture.upgrade())else{return;};
        let h=scroll.hadjustment();let v=scroll.vadjustment();let(w,h_image)=self.dimensions.get();let old=self.scale();
        let center=|a:&gtk::Adjustment,size:f64|((a.value()+a.page_size()/2.0-(a.page_size()-size).max(0.0)/2.0)/size.max(1.0)).clamp(0.0,1.0);
        let center=(center(&h,f64::from(w)*old),center(&v,f64::from(h_image)*old));
        self.fit.set(false);self.scale.set(scale.clamp(0.05,8.0));self.refresh();
        let generation=self.generation.get().wrapping_add(1);self.generation.set(generation);
        let weak=Rc::downgrade(self);let frames=Cell::new(0);
        picture.add_tick_callback(move |picture,_|{
            let Some(state)=weak.upgrade()else{return gtk::glib::ControlFlow::Break;};
            if state.generation.get()!=generation{return gtk::glib::ControlFlow::Break;}
            frames.set(frames.get()+1);if (picture.width()!=picture.width_request()||picture.height()!=picture.height_request())&&frames.get()<10{return gtk::glib::ControlFlow::Continue;}
            if let Some(scroll)=state.scroll.upgrade(){let h=scroll.hadjustment();let v=scroll.vadjustment();h.set_value((center.0*f64::from(picture.width())-h.page_size()/2.0).clamp(h.lower(),(h.upper()-h.page_size()).max(h.lower())));v.set_value((center.1*f64::from(picture.height())-v.page_size()/2.0).clamp(v.lower(),(v.upper()-v.page_size()).max(v.lower())));}
            gtk::glib::ControlFlow::Break
        });
    }
    fn fit(&self){self.generation.set(self.generation.get().wrapping_add(1));self.fit.set(true);self.refresh();}
}
fn button(icon:&str,label:&str)->gtk::Button{
    let button=gtk::Button::from_icon_name(icon);button.add_css_class("flat");button.set_tooltip_text(Some(label));button.update_property(&[gtk::accessible::Property::Label(label)]);button
}
fn fullscreen(window:&gtk::Window){if window.is_fullscreen(){window.unfullscreen();}else{window.fullscreen();}}

pub(in crate::ui::chat::transfers) fn install(window:&gtk::Window,picture:&gtk::Picture){
    picture.set_widget_name("image-viewer-picture");picture.set_can_shrink(true);picture.set_content_fit(gtk::ContentFit::Contain);picture.set_halign(gtk::Align::Center);picture.set_valign(gtk::Align::Center);
    let scroll=gtk::ScrolledWindow::new();scroll.set_widget_name("image-viewer-scroll");scroll.set_hexpand(true);scroll.set_vexpand(true);scroll.set_child(Some(picture));
    let controls=gtk::Box::new(gtk::Orientation::Horizontal,6);controls.set_halign(gtk::Align::Center);
    let minus=button("zoom-out-symbolic","Diminuir zoom");let plus=button("zoom-in-symbolic","Aumentar zoom");let fit=button("zoom-fit-best-symbolic","Ajustar à janela");let original=button("zoom-original-symbolic","Tamanho original (100%)");let full=button("view-fullscreen-symbolic","Tela cheia");
    let percentage=gtk::Label::new(None);percentage.set_width_chars(5);percentage.add_css_class("numeric");percentage.set_widget_name("image-viewer-zoom");
    let zoom_controls=gtk::Box::new(gtk::Orientation::Horizontal,6);zoom_controls.set_widget_name("image-viewer-zoom-controls");zoom_controls.set_visible(false);controls.append(&zoom_controls);
    for item in [minus.clone().upcast::<gtk::Widget>(),percentage.clone().upcast(),plus.clone().upcast(),fit.clone().upcast(),original.clone().upcast()]{zoom_controls.append(&item);}controls.append(&full);
    let header=adw::HeaderBar::new();header.set_centering_policy(adw::CenteringPolicy::Strict);
    let title=adw::WindowTitle::new(window.title().as_deref().unwrap_or("Imagem"),"");
    window.bind_property("title",&title,"title").sync_create().build();
    header.set_title_widget(Some(&title));header.pack_end(&controls);
    let root=gtk::Box::new(gtk::Orientation::Vertical,0);root.append(&header);root.append(&scroll);window.set_titlebar(Some(&gtk::Box::new(gtk::Orientation::Vertical,0)));window.set_child(Some(&root));window.set_destroy_with_parent(true);
    let state=Rc::new(State{picture:picture.downgrade(),scroll:scroll.downgrade(),percentage:percentage.downgrade(),fullscreen:Cell::new(false),fit:Cell::new(true),scale:Cell::new(1.0),dimensions:Cell::new((0,0)),generation:Cell::new(0)});
    // The root owns state; all widget callbacks use weak refs, so closing the
    // viewer releases its texture even if somebody still holds the window.
    let owner=state.clone();root.connect_map(move |_|owner.refresh());
    for (button,factor) in [(minus,0.8),(plus,1.25)]{let weak=Rc::downgrade(&state);button.connect_clicked(move |_|{if let Some(state)=weak.upgrade(){state.zoom(state.scale()*factor);}});}
    let weak=Rc::downgrade(&state);fit.connect_clicked(move |_|{if let Some(state)=weak.upgrade(){state.fit();}});
    let weak=Rc::downgrade(&state);original.connect_clicked(move |_|{if let Some(state)=weak.upgrade(){state.zoom(1.0);}});
    let weak=window.downgrade();full.connect_clicked(move |_|{if let Some(window)=weak.upgrade(){fullscreen(&window);}});
    let weak=full.downgrade();let group=zoom_controls.downgrade();let zoom=Rc::downgrade(&state);window.connect_fullscreened_notify(move |window|{
        let fullscreen=window.is_fullscreen();if let Some(group)=group.upgrade(){group.set_visible(fullscreen);}
        if let Some(state)=zoom.upgrade(){state.fullscreen.set(fullscreen);if !fullscreen{state.fit();}}
        if let Some(button)=weak.upgrade(){let label=if fullscreen{"Sair da tela cheia"}else{"Tela cheia"};button.set_icon_name(if fullscreen{"view-restore-symbolic"}else{"view-fullscreen-symbolic"});button.set_tooltip_text(Some(label));button.update_property(&[gtk::accessible::Property::Label(label)]);}
    });
    let weak=Rc::downgrade(&state);picture.connect_paintable_notify(move |_|{if let Some(state)=weak.upgrade(){state.refresh();}});
    let observer=gtk::WidgetPaintable::new(Some(&scroll));let weak=Rc::downgrade(&state);observer.connect_invalidate_size(move |_|{if let Some(state)=weak.upgrade(){state.refresh();}});scroll.connect_map(move |scroll|observer.set_widget(Some(scroll)));
    let wheel=gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);let weak=Rc::downgrade(&state);wheel.connect_scroll(move |event,_,dy|{if !event.current_event_state().contains(gtk::gdk::ModifierType::CONTROL_MASK){return gtk::glib::Propagation::Proceed;}if let Some(state)=weak.upgrade().filter(|s|s.fullscreen.get()){state.zoom(state.scale()*(-dy*0.15).exp());gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});scroll.add_controller(wheel);
    let pinch=gtk::GestureZoom::new();let base=Rc::new(Cell::new(1.0));let start=base.clone();let weak=Rc::downgrade(&state);pinch.connect_begin(move |_,_|{if let Some(state)=weak.upgrade(){start.set(state.scale());}});let weak=Rc::downgrade(&state);pinch.connect_scale_changed(move |_,factor|{if let Some(state)=weak.upgrade(){state.zoom(base.get()*factor);}});picture.add_controller(pinch);
    let drag=gtk::GestureDrag::new();drag.set_button(1);let origin=Rc::new(Cell::new((0.0,0.0)));let start=origin.clone();let weak=scroll.downgrade();drag.connect_drag_begin(move |_,_,_|{if let Some(scroll)=weak.upgrade(){start.set((scroll.hadjustment().value(),scroll.vadjustment().value()));}});let weak=scroll.downgrade();drag.connect_drag_update(move |gesture,x,y|{if let Some(scroll)=weak.upgrade(){let h=scroll.hadjustment();let v=scroll.vadjustment();if h.upper()>h.page_size()||v.upper()>v.page_size(){gesture.set_state(gtk::EventSequenceState::Claimed);let(a,b)=origin.get();h.set_value((a-x).clamp(h.lower(),(h.upper()-h.page_size()).max(h.lower())));v.set_value((b-y).clamp(v.lower(),(v.upper()-v.page_size()).max(v.lower())));}}});picture.add_controller(drag);
    let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);let weak=window.downgrade();let zoom=Rc::downgrade(&state);keys.connect_key_pressed(move |_,key,_,modifiers|{
        let Some(window)=weak.upgrade()else{return gtk::glib::Propagation::Proceed;};
        match key {
            gtk::gdk::Key::Escape=>{if window.is_fullscreen(){window.unfullscreen();}else{window.close();}},
            gtk::gdk::Key::F11=>fullscreen(&window),
            gtk::gdk::Key::plus|gtk::gdk::Key::KP_Add|gtk::gdk::Key::equal if window.is_fullscreen()=>{if let Some(state)=zoom.upgrade(){state.zoom(state.scale()*1.25);}},
            gtk::gdk::Key::minus|gtk::gdk::Key::KP_Subtract if window.is_fullscreen()=>{if let Some(state)=zoom.upgrade(){state.zoom(state.scale()*0.8);}},
            gtk::gdk::Key::_0 if window.is_fullscreen()&&modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)=>{if let Some(state)=zoom.upgrade(){state.fit();}},
            _=>return gtk::glib::Propagation::Proceed,
        }gtk::glib::Propagation::Stop
    });window.add_controller(keys);
    let weak=picture.downgrade();window.connect_close_request(move |window|{if let Some(picture)=weak.upgrade(){picture.set_paintable(None::<&gtk::gdk::Paintable>);}window.set_child(None::<&gtk::Widget>);gtk::glib::Propagation::Proceed});
    state.refresh();
}

pub(in crate::ui::chat::transfers) fn picture(window:&gtk::Window)->Option<gtk::Picture>{
    let root=window.child()?;let mut child=root.first_child();
    let scroll=loop{let widget=child?;child=widget.next_sibling();
        if widget.widget_name()=="image-viewer-scroll"{break widget.downcast::<gtk::ScrolledWindow>().ok()?;}
    };
    let child=scroll.child()?;child.clone().downcast::<gtk::Picture>().ok().or_else(||child.first_child()?.downcast::<gtk::Picture>().ok())
}

#[cfg(test)]pub(super) fn exercise_gif(context:&gtk::glib::MainContext){
    use actions::tests::{find_button,pump,until};
    let animation=crate::media::animation::PreparedAnimation::decode(&crate::media::animation::fixture()).unwrap().paintable();let weak=animation.downgrade();
    let picture=animation.picture();let window=gtk::Window::builder().title("Animated image test").default_width(900).default_height(680).build();install(&window,&picture);dismiss_image(&window,&picture);window.present();until(context,||picture.is_mapped());
    let first=animation.current_image();until(context,||animation.current_image()!=first);
    let zoom=actions::tests::descendants(window.upcast_ref()).into_iter().find(|w|w.widget_name()=="image-viewer-zoom-controls").unwrap();assert!(!zoom.is_visible());
    if std::env::var_os("PAPO_NATIVE_FULLSCREEN_TEST").is_some(){window.fullscreen();until(context,||window.is_fullscreen());assert!(zoom.is_visible());find_button(window.upcast_ref(),"Tamanho original (100%)").emit_clicked();window.unfullscreen();until(context,||!window.is_fullscreen());assert!(!zoom.is_visible());}
    assert_eq!(picture.paintable().unwrap(),animation.clone().upcast::<gtk::gdk::Paintable>());
    find_button(window.upcast_ref(),"Ajustar à janela").emit_clicked();performance::settle(context);
    let controllers=picture.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>()).unwrap();
    click.emit_by_name::<()>("released",&[&1i32,&f64::from(picture.width()/2),&f64::from(picture.height()/2)]);assert!(window.is_visible(),"clicking the GIF keeps the viewer open");
    drop(first);drop(animation);click.emit_by_name::<()>("released",&[&1i32,&1.0f64,&1.0f64]);pump(context);assert!(!window.is_visible()&&picture.paintable().is_none());assert!(weak.upgrade().is_none(),"closing a retained viewer releases GIF frames and animation timers");
}

#[cfg(test)]pub(super) fn exercise(window:&gtk::Window,context:&gtk::glib::MainContext){
    use actions::tests::{descendants,find_button,pump,until};
    let picture=picture(window).unwrap();let image=picture.paintable().unwrap();let width=image.intrinsic_width();let height=image.intrinsic_height();let texture=image.downgrade();
    let scroll=descendants(window.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok().filter(|s|s.widget_name()=="image-viewer-scroll")).unwrap();
    let zoom=descendants(window.upcast_ref()).into_iter().find(|w|w.widget_name()=="image-viewer-zoom-controls").unwrap();assert!(!zoom.is_visible());
    let keys=window.observe_controllers();let key=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    assert!(!key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::plus,&0u32,&gtk::gdk::ModifierType::empty()]));assert_eq!(picture.width_request(),-1,"windowed images stay fitted");
    if std::env::var_os("PAPO_NATIVE_FULLSCREEN_TEST").is_none(){return;}
    find_button(window.upcast_ref(),"Tela cheia").emit_clicked();until(context,||window.is_fullscreen());assert!(zoom.is_visible());
    find_button(window.upcast_ref(),"Tamanho original (100%)").emit_clicked();until(context,||picture.width_request()==width&&picture.width()==width);assert_eq!(picture.height_request(),height);
    assert!(scroll.hadjustment().upper()>scroll.hadjustment().page_size(),"large originals must be scrollable at 100%");
    find_button(window.upcast_ref(),"Aumentar zoom").emit_clicked();until(context,||picture.width_request()>width&&picture.width()==picture.width_request());
    let enlarged=picture.width_request();find_button(window.upcast_ref(),"Diminuir zoom").emit_clicked();until(context,||picture.width_request()<enlarged);assert_eq!(picture.width_request(),width);
    // Native drag changes the viewport instead of dismissing a zoomed image.
    performance::settle(context);let h=scroll.hadjustment();h.set_value((h.upper()-h.page_size())/2.0);let before=h.value();
    let controllers=picture.observe_controllers();let drag=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureDrag>()).unwrap();drag.emit_by_name::<()>("drag-begin",&[&0.0f64,&0.0f64]);drag.emit_by_name::<()>("drag-update",&[&-80.0f64,&0.0f64]);assert!(h.value()>before&&window.is_visible());
    let keys=window.observe_controllers();let key=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>()).unwrap();
    assert!(key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::plus,&0u32,&gtk::gdk::ModifierType::empty()]));pump(context);assert!(picture.width_request()>width);
    assert!(key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::_0,&0u32,&gtk::gdk::ModifierType::CONTROL_MASK]));until(context,||picture.width_request()==-1);performance::settle(context);
    assert!(scroll.hadjustment().upper()<=scroll.hadjustment().page_size()+1.0&&scroll.vadjustment().upper()<=scroll.vadjustment().page_size()+1.0,"fit resets zoom without horizontal or vertical overflow");
    key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Escape,&0u32,&gtk::gdk::ModifierType::empty()]);until(context,||!window.is_fullscreen());assert!(!zoom.is_visible());assert!(window.is_visible(),"Escape exits fullscreen before closing the image");assert_eq!(picture.width_request(),-1);
    key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::F11,&0u32,&gtk::gdk::ModifierType::empty()]);until(context,||window.is_fullscreen());assert!(zoom.is_visible());find_button(window.upcast_ref(),"Sair da tela cheia").emit_clicked();until(context,||!window.is_fullscreen());assert!(!zoom.is_visible());
    assert_eq!(picture.paintable().unwrap(),image,"zoom and fullscreen must reuse the same paintable");assert!(texture.upgrade().is_some());
    crate::ui::main_window::layout::tests::preview(window,"image-zoom-controls",context);
}
