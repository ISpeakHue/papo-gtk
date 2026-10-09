//! Observe backend audio updates, not just GtkMediaStream's cached properties.
use super::*;
use gtk::{glib,subclass::prelude::*};
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Probe {pub audio:Cell<(bool,f64)>}
    #[glib::object_subclass]
    impl ObjectSubclass for Probe {
        const NAME:&'static str="PapoAudioUpdateProbe";
        type Type=super::Probe;
        type ParentType=gtk::MediaStream;
    }
    impl ObjectImpl for Probe {}
    impl MediaStreamImpl for Probe {
        fn play(&self)->bool{true}
        fn pause(&self){}
        fn update_audio(&self,muted:bool,volume:f64){self.audio.set((muted,volume));}
    }
}
glib::wrapper!{pub struct Probe(ObjectSubclass<imp::Probe>) @extends gtk::MediaStream,@implements gtk::gdk::Paintable;}
pub(super) fn exercise(){
    use crate::ui::chat::actions::tests::{descendants,find_button};
    // GTK reports defaults of unmuted/1.0 while the output backend is silent.
    // Reapplying those same values does not invoke update_audio.
    let stream:Probe=glib::Object::new();assert_eq!(stream.imp().audio.get(),(false,0.0));
    let video=gtk::Video::for_media_stream(Some(&stream));let _controls=super::controls(&video,&stream);
    assert_eq!(stream.imp().audio.get(),(false,1.0),"initial sound must reach the backend without touching the slider");
    stream.imp().audio.set((false,0.0));stream.stream_prepared(true,true,true,2_000_000);
    assert_eq!(stream.imp().audio.get(),(false,1.0),"preparation must synchronize native output even when GTK's properties already have their default values");
    // Choices made before asynchronous preparation must win over defaults.
    let stream:Probe=glib::Object::new();let video=gtk::Video::for_media_stream(Some(&stream));let controls=super::controls(&video,&stream);
    let slider=descendants(controls.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Scale>().ok().filter(|s|s.tooltip_text().as_deref()==Some("Volume do vídeo"))).unwrap();
    slider.set_value(0.37);find_button(controls.upcast_ref(),"Silenciar vídeo").emit_clicked();
    stream.stream_prepared(true,true,true,2_000_000);assert_eq!(stream.imp().audio.get(),(true,0.37));assert!(stream.is_muted());assert_eq!(stream.volume(),0.37);
    // A repeated prepare cannot undo an explicit mute.
    stream.stream_unprepared();stream.stream_prepared(true,true,true,2_000_000);assert_eq!(stream.imp().audio.get(),(true,0.37));
    drop(controls);
}

pub(super) fn fullscreen(context:&glib::MainContext){
    use crate::ui::chat::{actions::tests::{descendants,find_button,until},performance::settle};
    let stream:Probe=glib::Object::new();let video=gtk::Video::for_media_stream(Some(&stream));let controls=super::controls(&video,&stream);
    stream.stream_prepared(true,true,true,60_000_000);stream.update(12_000_000);stream.set_volume(0.37);stream.set_muted(true);
    let slot=gtk::Box::new(gtk::Orientation::Vertical,0);let player=gtk::Box::new(gtk::Orientation::Vertical,6);slot.append(&player);
    let frame=crate::media::frame::frame(&video,640,360);player.append(&frame);player.append(&controls);
    let state=super::Fullscreen::new(&slot,&player,&frame,&stream,&controls);
    let parent=gtk::Window::builder().default_width(700).default_height(500).child(&slot).build();parent.present();settle(context);
    let toggle=find_button(slot.upcast_ref(),"Tela cheia");
    for (key,playing) in [(gtk::gdk::Key::Escape,false),(gtk::gdk::Key::F11,true)]{
        if playing{stream.play();}else{stream.pause();}toggle.emit_clicked();settle(context);
        let window=video.root().and_downcast::<gtk::Window>().unwrap();assert_ne!(window,parent);assert!(state.is_open());
        assert_eq!(video.media_stream().unwrap(),stream.clone().upcast::<gtk::MediaStream>());assert_eq!(video.width(),window.width());assert!(video.width()>640&&video.height()>360,"fullscreen surface expands: {}x{}",video.width(),video.height());
        assert_eq!(stream.timestamp(),12_000_000);assert_eq!(stream.volume(),0.37);assert!(stream.is_muted());assert_eq!(stream.is_playing(),playing,"opening fullscreen preserves play/pause");
        assert!(descendants(slot.upcast_ref()).iter().any(|w|w.widget_name()=="fullscreen-video-placeholder"));
        let controllers=window.observe_controllers();let keys=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
        assert!(keys.emit_by_name::<bool>("key-pressed",&[&key,&0u32,&gtk::gdk::ModifierType::empty()]));until(context,||!window.is_visible());settle(context);
        assert!(!state.is_open());assert_eq!(video.root().and_downcast::<gtk::Window>().unwrap(),parent);assert_eq!(stream.timestamp(),12_000_000);assert_eq!(stream.volume(),0.37);assert!(stream.is_muted());assert_eq!(stream.is_playing(),playing,"exiting fullscreen preserves play/pause");
    }
    toggle.emit_clicked();settle(context);let window=video.root().and_downcast::<gtk::Window>().unwrap();find_button(window.upcast_ref(),"Sair da tela cheia").emit_clicked();settle(context);assert!(!window.is_visible()&&!state.is_open());
    toggle.emit_clicked();settle(context);let window=video.root().and_downcast::<gtk::Window>().unwrap();state.shutdown();settle(context);assert!(!window.is_visible()&&!state.is_open());toggle.emit_clicked();assert!(!state.is_open(),"disposed playback cannot reopen fullscreen");
    parent.set_child(None::<&gtk::Widget>);parent.close();
}
