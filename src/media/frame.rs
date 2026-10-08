//! Responsive media geometry independent of a thumbnail's pixel dimensions.
use gtk::{glib,prelude::*,subclass::prelude::*};
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Frame {
        pub child:std::cell::OnceCell<gtk::Widget>,
        pub size:std::cell::Cell<(i32,i32)>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Frame {
        const NAME:&'static str="PapoMediaFrame";
        type Type=super::Frame;
        type ParentType=gtk::Widget;
    }
    impl ObjectImpl for Frame {
        fn dispose(&self){if let Some(child)=self.child.get(){if child.parent().is_some(){child.unparent();}}}
    }
    impl WidgetImpl for Frame {
        fn request_mode(&self)->gtk::SizeRequestMode{gtk::SizeRequestMode::HeightForWidth}
        fn measure(&self,orientation:gtk::Orientation,for_size:i32)->(i32,i32,i32,i32){
            let (width,height)=self.size.get();
            if orientation==gtk::Orientation::Horizontal{(0,width,-1,-1)}else{
                let available=if for_size<0{width}else{for_size.min(width).max(0)};
                let scaled=(f64::from(available)*f64::from(height)/f64::from(width.max(1))).round() as i32;
                (if for_size<0{0}else{scaled},scaled,-1,-1)
            }
        }
        fn size_allocate(&self,width:i32,height:i32,baseline:i32){if let Some(child)=self.child.get(){child.allocate(width,height,baseline,None);}}
        fn snapshot(&self,snapshot:&gtk::Snapshot){if let Some(child)=self.child.get(){self.obj().snapshot_child(child,snapshot);}}
    }
}
glib::wrapper!{pub struct Frame(ObjectSubclass<imp::Frame>) @extends gtk::Widget,@implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget;}
pub fn frame(child:&impl IsA<gtk::Widget>,width:i32,height:i32)->Frame {
    let frame:Frame=glib::Object::new();frame.imp().size.set((width.max(1),height.max(1)));
    let child=child.as_ref().clone();child.set_parent(&frame);let _=frame.imp().child.set(child);
    frame.set_halign(gtk::Align::Start);frame.set_valign(gtk::Align::Start);frame.set_overflow(gtk::Overflow::Hidden);frame.add_css_class("papo-media-frame");frame
}
pub fn image_frame(child:&impl IsA<gtk::Widget>,texture:&gtk::gdk::Texture,embedded:bool)->Frame {
    let ratio=f64::from(texture.width())/f64::from(texture.height().max(1));
    let width=if embedded{420.0f64.min(300.0*ratio)}else{640.0f64.min(480.0*ratio)};
    frame(child,width.round().max(1.0) as i32,(width/ratio).round().max(1.0) as i32)
}
