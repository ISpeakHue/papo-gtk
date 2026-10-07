//! Give thumbnails a larger presentation size without allocating enlarged pixels.
use gtk::{gdk,glib};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Preview {pub texture:std::cell::OnceCell<gdk::Texture>,pub size:std::cell::Cell<(i32,i32)>}
    #[glib::object_subclass]
    impl ObjectSubclass for Preview {
        const NAME:&'static str="PapoChatPreview";
        type Type=super::Preview;
        type Interfaces=(gdk::Paintable,);
    }
    impl ObjectImpl for Preview {}
    impl PaintableImpl for Preview {
        fn current_image(&self)->gdk::Paintable{self.obj().clone().upcast()}
        fn flags(&self)->gdk::PaintableFlags{gdk::PaintableFlags::SIZE|gdk::PaintableFlags::CONTENTS}
        fn intrinsic_width(&self)->i32{self.size.get().0}
        fn intrinsic_height(&self)->i32{self.size.get().1}
        fn intrinsic_aspect_ratio(&self)->f64{let(w,h)=self.size.get();if h==0{0.0}else{f64::from(w)/f64::from(h)}}
        fn snapshot(&self,snapshot:&gdk::Snapshot,width:f64,height:f64){if let Some(texture)=self.texture.get(){texture.snapshot(snapshot,width,height);}}
    }
}
glib::wrapper!{pub struct Preview(ObjectSubclass<imp::Preview>) @implements gdk::Paintable;}
pub fn picture(texture:&gdk::Texture)->gtk::Picture {
    let preview:Preview=glib::Object::new();let ratio=f64::from(texture.width())/f64::from(texture.height());
    let width=720.0f64.min(480.0*ratio);let height=width/ratio;
    preview.imp().size.set((width.round().max(1.0) as i32,height.round().max(1.0) as i32));let _=preview.imp().texture.set(texture.clone());
    let picture=gtk::Picture::for_paintable(&preview);picture.set_can_shrink(true);picture.set_content_fit(gtk::ContentFit::Contain);picture
}
