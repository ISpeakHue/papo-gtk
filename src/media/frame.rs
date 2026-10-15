//! Responsive media geometry independent of a thumbnail's pixel dimensions.
use gtk::{glib,prelude::*,subclass::prelude::*};
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Frame {
        pub child:std::cell::OnceCell<gtk::Widget>,
        pub size:std::cell::Cell<(i32,i32)>,
        pub first_snapshot:std::cell::RefCell<Option<Box<dyn FnOnce()>>>,
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
        fn size_allocate(&self,width:i32,height:i32,baseline:i32){if let Some(child)=self.child.get().filter(|child|child.should_layout()){
            // A remembered tiny GIF can be smaller than its retry button's
            // padding/label. Clip that loading control, rather than allocating
            // negative label space or inflating the media's natural dimensions.
            let child_width=width.max(child.measure(gtk::Orientation::Horizontal,-1).0);
            let child_height=height.max(child.measure(gtk::Orientation::Vertical,child_width).0);
            child.allocate(child_width,child_height,baseline,None);
        }}
        fn snapshot(&self,snapshot:&gtk::Snapshot){
            if let Some(first)=self.first_snapshot.borrow_mut().take(){first();}
            if let Some(child)=self.child.get(){self.obj().snapshot_child(child,snapshot);}
        }
    }
}
glib::wrapper!{pub struct Frame(ObjectSubclass<imp::Frame>) @extends gtk::Widget,@implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget;}
impl Frame {
    pub fn on_first_snapshot(&self,callback:impl FnOnce()+'static){*self.imp().first_snapshot.borrow_mut()=Some(Box::new(callback));}
}
pub fn frame(child:&impl IsA<gtk::Widget>,width:i32,height:i32)->Frame {
    let frame:Frame=glib::Object::new();frame.imp().size.set((width.max(1),height.max(1)));
    let child=child.as_ref().clone();child.set_parent(&frame);let _=frame.imp().child.set(child);
    frame.set_halign(gtk::Align::Start);frame.set_valign(gtk::Align::Start);frame.set_overflow(gtk::Overflow::Hidden);frame.add_css_class("papo-media-frame");frame
}
/// Bound presentation without enlarging stickers, tiny GIFs or source pixels.
pub fn fit_size(width:i32,height:i32,embedded:bool)->(i32,i32){
    let(width,height)=(width.max(1),height.max(1));
    let(max_width,max_height)=if embedded{(420.0,300.0)}else{(640.0,480.0)};
    let scale=1.0f64.min(max_width/f64::from(width)).min(max_height/f64::from(height));
    ((f64::from(width)*scale).round().max(1.0) as i32,(f64::from(height)*scale).round().max(1.0) as i32)
}
#[cfg(test)]mod tests{
    #[test]fn media_sizes_preserve_small_sources_and_aspect(){
        for embedded in [false,true]{assert_eq!(super::fit_size(32,16,embedded),(32,16));assert_eq!(super::fit_size(160,160,embedded),(160,160));}
        assert_eq!(super::fit_size(1280,320,false),(640,160));assert_eq!(super::fit_size(1280,320,true),(420,105));
        assert_eq!(super::fit_size(160,640,false),(120,480));assert_eq!(super::fit_size(0,0,true),(1,1));
    }
}
