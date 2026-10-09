//! Explicit mute and hover-volume controls for user-started inline playback.
use gtk::prelude::*;
use std::{cell::Cell,rc::Rc};
mod fullscreen;
pub(super) use fullscreen::Fullscreen;
fn initialize_audio(media:&gtk::MediaStream){
    // GTK skips setters whose value matches its cached default, so repeating
    // volume=1/muted=false can leave the native output silent. Force an audio
    // update before playback and when asynchronous preparation finishes.
    media.set_volume(0.0);media.set_muted(false);media.set_volume(1.0);
}
pub(super) fn controls(video:&gtk::Video,media:&impl IsA<gtk::MediaStream>)->gtk::Box{
    let media=media.as_ref();
    // Detach native MediaControls, whose volume binding also maps mute to
    // zero volume. Hiding its button alone leaves that binding active.
    fn detach_controls(widget:&gtk::Widget){
        if let Some(controls)=widget.downcast_ref::<gtk::MediaControls>(){controls.set_media_stream(None::<&gtk::MediaStream>);controls.set_visible(false);return;}
        let mut child=widget.first_child();while let Some(w)=child{detach_controls(&w);child=w.next_sibling();}
    }
    detach_controls(video.upcast_ref());video.connect_map(|video|detach_controls(video.upcast_ref()));
    let bar=gtk::Box::new(gtk::Orientation::Horizontal,6);bar.set_hexpand(true);
    let play=gtk::Button::from_icon_name("media-playback-pause-symbolic");play.set_tooltip_text(Some("Reproduzir ou pausar vídeo"));play.add_css_class("flat");let weak=media.downgrade();play.connect_clicked(move |_|{if let Some(media)=weak.upgrade(){if media.is_playing(){media.pause();}else{if media.is_ended(){media.seek(0);}media.play();}}});bar.append(&play);
    let seek=gtk::Scale::with_range(gtk::Orientation::Horizontal,0.0,1.0,0.1);seek.set_hexpand(true);seek.set_draw_value(false);seek.set_tooltip_text(Some("Posição do vídeo"));bar.append(&seek);
    let time=gtk::Label::new(Some("0:00 / 0:00"));time.add_css_class("caption");bar.append(&time);
    let updating=Rc::new(Cell::new(false));let busy=updating.clone();let weak=media.downgrade();seek.connect_value_changed(move |seek|{if !busy.get(){if let Some(media)=weak.upgrade(){if media.is_seekable(){media.seek((seek.value()*1_000_000.0) as i64);}}}});
    let weak_seek=seek.downgrade();let weak_time=time.downgrade();let weak_play=play.downgrade();media.connect_notify_local(None,move |media,property|{
        if !matches!(property.name(),"timestamp"|"duration"|"playing"|"prepared"|"seekable"){return;}
        updating.set(true);if let Some(seek)=weak_seek.upgrade(){seek.set_range(0.0,(media.duration() as f64/1_000_000.0).max(1.0));seek.set_value(media.timestamp() as f64/1_000_000.0);seek.set_sensitive(media.is_seekable());}updating.set(false);
        if let Some(time)=weak_time.upgrade(){let seconds=media.timestamp()/1_000_000;let duration=media.duration()/1_000_000;time.set_text(&format!("{}:{:02} / {}:{:02}",seconds/60,seconds%60,duration/60,duration%60));}
        if let Some(play)=weak_play.upgrade(){play.set_icon_name(if media.is_playing(){"media-playback-pause-symbolic"}else{"media-playback-start-symbolic"});}
    });
    let row=gtk::Box::new(gtk::Orientation::Horizontal,4);row.set_widget_name("papo-video-volume");row.set_halign(gtk::Align::Start);
    let mute=gtk::Button::from_icon_name("audio-volume-high-symbolic");mute.set_tooltip_text(Some("Silenciar vídeo"));mute.add_css_class("flat");row.append(&mute);
    let slider=gtk::Scale::with_range(gtk::Orientation::Horizontal,0.0,1.0,0.01);slider.set_draw_value(false);slider.set_width_request(120);slider.set_value(1.0);slider.set_tooltip_text(Some("Volume do vídeo"));
    let reveal=gtk::Revealer::new();reveal.set_transition_type(gtk::RevealerTransitionType::SlideRight);reveal.set_child(Some(&slider));row.append(&reveal);
    let touched=Rc::new(Cell::new(false));let last_volume=Rc::new(Cell::new(1.0));let last=last_volume.clone();let changed=touched.clone();let weak=media.downgrade();mute.connect_clicked(move |_|{
        changed.set(true);if let Some(media)=weak.upgrade(){if media.is_muted()||media.volume()==0.0{if media.volume()==0.0{media.set_volume(last.get());}media.set_muted(false);}else{last.set(media.volume());media.set_muted(true);}}
    });
    let syncing=Rc::new(Cell::new(false));let last=last_volume.clone();let updating=syncing.clone();let changed=touched.clone();let weak=media.downgrade();slider.connect_value_changed(move |slider|{if updating.get(){return;}changed.set(true);if slider.value()>0.0{last.set(slider.value());}if let Some(media)=weak.upgrade(){media.set_volume(slider.value());media.set_muted(slider.value()==0.0);}});
    let weak_mute=mute.downgrade();let weak_slider=slider.downgrade();media.connect_notify_local(None,move |media,property|{
        if !matches!(property.name(),"volume"|"muted"){return;}
        if let Some(mute)=weak_mute.upgrade(){mute.set_icon_name(if media.is_muted()||media.volume()==0.0{"audio-volume-muted-symbolic"}else{"audio-volume-high-symbolic"});mute.set_tooltip_text(Some(if media.is_muted(){"Ativar som do vídeo"}else{"Silenciar vídeo"}));}
        if let Some(slider)=weak_slider.upgrade(){syncing.set(true);slider.set_value(media.volume());syncing.set(false);}
    });
    let weak=reveal.downgrade();let motion=gtk::EventControllerMotion::new();motion.connect_enter(move |_,_,_|{if let Some(reveal)=weak.upgrade(){reveal.set_reveal_child(true);}});
    let weak=reveal.downgrade();let weak_row=row.downgrade();motion.connect_leave(move |_|{if let (Some(reveal),Some(row))=(weak.upgrade(),weak_row.upgrade()){if !row.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN){reveal.set_reveal_child(false);}}});row.add_controller(motion);
    let focus=gtk::EventControllerFocus::new();let weak=reveal.downgrade();focus.connect_enter(move |_|{if let Some(reveal)=weak.upgrade(){reveal.set_reveal_child(true);}});let weak=reveal.downgrade();focus.connect_leave(move |_|{if let Some(reveal)=weak.upgrade(){reveal.set_reveal_child(false);}});row.add_controller(focus);
    // Set these after GtkVideo has attached the stream, and once preparation
    // completes. An explicit user mute/volume choice always wins thereafter.
    initialize_audio(media);
    media.connect_prepared_notify(move |media|{if media.is_prepared()&&!touched.get(){initialize_audio(media);}});
    bar.append(&row);bar
}

#[cfg(test)]mod tests;
#[cfg(test)]pub(crate) fn exercise_initial_audio(){tests::exercise();}
#[cfg(test)]pub(crate) fn exercise_fullscreen(context:&gtk::glib::MainContext){tests::fullscreen(context);}

#[cfg(test)]pub(super) fn exercise(root:&gtk::Box,media:&gtk::MediaFile,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{descendants,pump};
    let widgets=descendants(root.upcast_ref());let row=widgets.iter().find(|w|w.widget_name()=="papo-video-volume").unwrap();
    let button=descendants(row).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok()).unwrap();let reveal=descendants(row).into_iter().find_map(|w|w.downcast::<gtk::Revealer>().ok()).unwrap();
    assert!(!reveal.reveals_child());button.emit_clicked();pump(context);assert!(media.is_muted());assert!(!reveal.reveals_child(),"click toggles mute without opening a popup");button.emit_clicked();assert!(!media.is_muted());
    let controllers=row.observe_controllers();let motion=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerMotion>()).unwrap();motion.emit_by_name::<()>("enter",&[&0.0f64,&0.0f64]);assert!(reveal.reveals_child());
    let slider=descendants(row).into_iter().find_map(|w|w.downcast::<gtk::Scale>().ok()).unwrap();slider.set_value(0.3);assert!((media.volume()-0.3).abs()<0.001);button.emit_clicked();button.emit_clicked();assert!((media.volume()-0.3).abs()<0.001&&!media.is_muted(),"volume={}, muted={}",media.volume(),media.is_muted());
    slider.set_value(0.0);assert!(media.is_muted());button.emit_clicked();assert!(!media.is_muted()&&(media.volume()-0.3).abs()<0.001,"unmuting zero volume restores the last audible level");
    motion.emit_by_name::<()>("leave",&[]);assert!(!reveal.reveals_child());button.emit_clicked();assert!(media.is_muted());
}
