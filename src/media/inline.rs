//! Give shared emoji frames text-sized geometry without decoding another copy.
use gtk::{gdk,glib,prelude::*,subclass::prelude::*};
mod imp {
    use super::*;
    #[derive(Default)]pub struct Inline {pub source:std::cell::OnceCell<gdk::Paintable>,pub size:std::cell::Cell<(i32,i32)>,pub signal:std::cell::RefCell<Option<glib::SignalHandlerId>>}
    #[glib::object_subclass]impl ObjectSubclass for Inline{const NAME:&'static str="PapoInlinePaintable";type Type=super::Inline;type Interfaces=(gdk::Paintable,);}
    impl ObjectImpl for Inline{fn dispose(&self){if let Some(id)=self.signal.borrow_mut().take(){self.source.get().unwrap().disconnect(id);}}}
    impl PaintableImpl for Inline{
        fn current_image(&self)->gdk::Paintable{let p=self.obj();super::sized(&self.source.get().unwrap().current_image(),p.intrinsic_width(),p.intrinsic_height()).upcast()}
        fn flags(&self)->gdk::PaintableFlags{gdk::PaintableFlags::SIZE|(self.source.get().unwrap().flags()&gdk::PaintableFlags::CONTENTS)}
        fn intrinsic_width(&self)->i32{self.size.get().0}
        fn intrinsic_height(&self)->i32{self.size.get().1}
        fn intrinsic_aspect_ratio(&self)->f64{let(w,h)=self.size.get();f64::from(w)/f64::from(h)}
        fn snapshot(&self,snapshot:&gdk::Snapshot,width:f64,height:f64){self.source.get().unwrap().snapshot(snapshot,width,height);}
    }
}
glib::wrapper!{pub struct Inline(ObjectSubclass<imp::Inline>) @implements gdk::Paintable;}
fn sized(source:&gdk::Paintable,width:i32,height:i32)->Inline{
    let p:Inline=glib::Object::new();p.imp().size.set((width,height));p.imp().source.set(source.clone()).unwrap();
    if !source.flags().contains(gdk::PaintableFlags::CONTENTS){let weak=p.downgrade();let id=source.connect_invalidate_contents(move |_|{if let Some(p)=weak.upgrade(){p.invalidate_contents();}});p.imp().signal.replace(Some(id));}p
}
pub fn fit(source:&gdk::Paintable,size:i32)->gdk::Paintable{
    let(w,h)=(source.intrinsic_width().max(1),source.intrinsic_height().max(1));let scale=f64::from(size.max(1))/f64::from(w.max(h));
    sized(source,(f64::from(w)*scale).round().max(1.0) as i32,(f64::from(h)*scale).round().max(1.0) as i32).upcast()
}
