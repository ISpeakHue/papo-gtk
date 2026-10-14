//! Give rich text its content width while retaining native, responsive wrapping.
//! GtkTextView reports no useful natural width to a left-aligned embed card.
use gtk::{glib,pango,prelude::*,subclass::prelude::*};

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct Flow {
        pub child:std::cell::OnceCell<gtk::TextView>,
        pub width:std::cell::Cell<i32>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for Flow {
        const NAME:&'static str="PapoRichTextFlow";
        type Type=super::Flow;
        type ParentType=gtk::Widget;
    }
    impl ObjectImpl for Flow {
        fn dispose(&self){if let Some(child)=self.child.get(){if child.parent().is_some(){child.unparent();}}}
    }
    impl WidgetImpl for Flow {
        fn request_mode(&self)->gtk::SizeRequestMode{gtk::SizeRequestMode::HeightForWidth}
        fn measure(&self,orientation:gtk::Orientation,for_size:i32)->(i32,i32,i32,i32){
            if orientation==gtk::Orientation::Horizontal{(0,self.width.get(),-1,-1)}
            else{self.child.get().map_or((0,0,-1,-1),|child|child.measure(orientation,for_size))}
        }
        fn size_allocate(&self,width:i32,height:i32,baseline:i32){if let Some(child)=self.child.get(){child.allocate(width,height,baseline,None);}}
        fn snapshot(&self,snapshot:&gtk::Snapshot){if let Some(child)=self.child.get(){self.obj().snapshot_child(child,snapshot);}}
    }
}
glib::wrapper!{pub struct Flow(ObjectSubclass<imp::Flow>) @extends gtk::Widget,@implements gtk::Accessible,gtk::Buildable,gtk::ConstraintTarget;}

fn preferred_width(view:&gtk::TextView)->i32{
    let buffer=view.buffer();let text=buffer.slice(&buffer.start_iter(),&buffer.end_iter(),true);
    let layout=view.create_pango_layout(Some(&text));let attributes=pango::AttrList::new();
    let mut start=buffer.start_iter();let mut byte=0usize;
    while start.offset()<buffer.char_count(){
        let mut end=start;if !end.forward_to_tag_toggle(None::<&gtk::TextTag>){end=buffer.end_iter();}
        let segment=buffer.slice(&start,&end,true);let limit=byte+segment.len();
        let add=|mut attr:pango::Attribute|{attr.set_start_index(byte as u32);attr.set_end_index(limit as u32);attributes.change(attr);};
        for tag in start.tags(){
            if tag.is_weight_set(){add(pango::AttrInt::new_weight(if tag.weight()>=700{pango::Weight::Bold}else{pango::Weight::Normal}).into());}
            if tag.is_style_set(){add(pango::AttrInt::new_style(tag.style()).into());}
            if tag.is_scale_set(){add(pango::AttrFloat::new_scale(tag.scale()).into());}
            if tag.is_family_set(){if let Some(family)=tag.family(){add(pango::AttrString::new_family(&family).into());}}
        }
        for (character,(at,ch)) in segment.char_indices().enumerate(){if ch=='\u{fffc}'{
            if let Some(image)=buffer.iter_at_offset(start.offset()+character as i32).paintable(){
                let rect=pango::Rectangle::new(0,0,image.intrinsic_width().max(1)*pango::SCALE,image.intrinsic_height().max(1)*pango::SCALE);
                let mut attr:pango::Attribute=pango::AttrShape::new(&rect,&rect).into();attr.set_start_index((byte+at) as u32);attr.set_end_index((byte+at+ch.len_utf8()) as u32);attributes.change(attr);
            }
        }}
        byte=limit;start=end;
    }
    layout.set_attributes(Some(&attributes));
    // A tiny rounding allowance avoids moving the last glyph to a new line.
    (layout.pixel_size().0+view.left_margin()+view.right_margin()+2).clamp(1,520)
}
pub(super) fn wrap(view:&gtk::TextView)->Flow{
    let flow:Flow=glib::Object::new();flow.imp().width.set(preferred_width(view));
    view.set_parent(&flow);let _=flow.imp().child.set(view.clone());flow.set_hexpand(true);flow.set_vexpand(false);flow
}
