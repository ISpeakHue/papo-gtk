//! Bounded GIF decoding off the UI thread; frame timers live with the paintable.
use super::PreparedImage;
use gtk::{gdk,glib,prelude::*,subclass::prelude::*};
use std::{cell::{Cell,RefCell,OnceCell},time::Duration};
#[derive(Debug)]
pub struct PreparedAnimation{pub frames:Vec<(PreparedImage,Duration)>}
impl PreparedAnimation{
    pub fn decode(bytes:&[u8])->Option<Self>{
        use image::{AnimationDecoder,ImageDecoder};
        if bytes.len()>4<<20{return None;}
        let mut decoder=image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
        let(w,h)=decoder.dimensions();if w==0||h==0||w>2048||h>2048{return None;}
        let mut limits=image::Limits::default();limits.max_alloc=Some(32<<20);decoder.set_limits(limits).ok()?;
        let mut frames=vec![];let mut total=0usize;
        for (index,frame) in decoder.into_frames().enumerate(){if index>=240{return None;}let frame=frame.ok()?;let(n,d)=frame.delay().numer_denom_ms();
            let delay=Duration::from_millis((u64::from(n)/u64::from(d.max(1))).clamp(20,10_000));
            let pixels=image::DynamicImage::ImageRgba8(frame.into_buffer()).thumbnail(420.min(w),300.min(h)).into_rgba8();
            total+=pixels.len();if total>48<<20{return None;}
            frames.push((PreparedImage{width:pixels.width(),height:pixels.height(),pixels:pixels.into_raw()},delay));
        }
        if frames.is_empty(){None}else{Some(Self{frames})}
    }
    pub fn bytes(&self)->usize{self.frames.iter().map(|(f,_)|f.pixels.len()).sum()}
    pub fn paintable(self)->Animation{
        let animation:Animation=glib::Object::new();let frames=self.frames.into_iter().map(|(frame,delay)|(frame.texture(),delay)).collect::<Vec<_>>();
        let _=animation.imp().frames.set(frames);animation
    }
}
mod imp{
    use super::*;
    #[derive(Default)]pub struct Animation{pub frames:OnceCell<Vec<(gdk::Texture,Duration)>>,pub index:Cell<usize>,pub timer:RefCell<Option<glib::SourceId>>,pub pictures:RefCell<Vec<glib::WeakRef<gtk::Picture>>>}
    #[glib::object_subclass]impl ObjectSubclass for Animation{const NAME:&'static str="PapoGifAnimation";type Type=super::Animation;type Interfaces=(gdk::Paintable,);}
    impl ObjectImpl for Animation{fn dispose(&self){if let Some(timer)=self.timer.borrow_mut().take(){timer.remove();}}}
    impl PaintableImpl for Animation{
        fn flags(&self)->gdk::PaintableFlags{gdk::PaintableFlags::SIZE}
        fn current_image(&self)->gdk::Paintable{self.frames.get().unwrap()[self.index.get()].0.clone().upcast()}
        fn intrinsic_width(&self)->i32{self.frames.get().unwrap()[0].0.width()}
        fn intrinsic_height(&self)->i32{self.frames.get().unwrap()[0].0.height()}
        fn intrinsic_aspect_ratio(&self)->f64{f64::from(self.intrinsic_width())/f64::from(self.intrinsic_height())}
        fn snapshot(&self,snapshot:&gdk::Snapshot,width:f64,height:f64){self.frames.get().unwrap()[self.index.get()].0.snapshot(snapshot,width,height);}
    }
}
glib::wrapper!{pub struct Animation(ObjectSubclass<imp::Animation>) @implements gdk::Paintable;}
impl Animation{
    pub fn first(&self)->gdk::Texture{self.imp().frames.get().unwrap()[0].0.clone()}
    pub fn picture(&self)->gtk::Picture{
        let picture=gtk::Picture::for_paintable(self);picture.set_can_shrink(true);picture.set_content_fit(gtk::ContentFit::Contain);
        self.imp().pictures.borrow_mut().push(picture.downgrade());
        let weak=self.downgrade();picture.connect_map(move |_|{if let Some(animation)=weak.upgrade(){schedule(&animation);}});
        let weak=self.downgrade();picture.connect_unmap(move |_|{if let Some(animation)=weak.upgrade(){
            if !animation.has_mapped_picture(){if let Some(timer)=animation.imp().timer.borrow_mut().take(){timer.remove();}}
        }});picture
    }
    fn has_mapped_picture(&self)->bool{self.pictures().iter().any(|p|p.is_mapped())}
    fn pictures(&self)->Vec<gtk::Picture>{
        let mut pictures=self.imp().pictures.borrow_mut();pictures.retain(|p|p.upgrade().is_some());pictures.iter().filter_map(|p|p.upgrade()).collect()
    }
    fn on_screen(&self)->bool{
        self.pictures().iter().any(|picture|{
            if !picture.is_mapped(){return false;}
            let mut parent=picture.parent();
            while let Some(widget)=parent{
                if let Some(scroll)=widget.downcast_ref::<gtk::ScrolledWindow>(){
                    if picture.compute_bounds(scroll).is_none_or(|r|r.y()+r.height()<=0.0||r.y()>=scroll.height() as f32||r.x()+r.width()<=0.0||r.x()>=scroll.width() as f32){return false;}
                }
                parent=widget.parent();
            }true
        })
    }
}
fn schedule(animation:&Animation){
    let frames=animation.imp().frames.get().unwrap();if frames.len()<2||animation.imp().timer.borrow().is_some()||!animation.has_mapped_picture(){return;}
    // Cached or clipped GIFs must not continually invalidate the entire chat.
    // Poll clipped, mapped pictures slowly so scrolling them into view resumes.
    let delay=if animation.on_screen(){frames[animation.imp().index.get()].1}else{Duration::from_millis(250)};let weak=animation.downgrade();
    *animation.imp().timer.borrow_mut()=Some(glib::timeout_add_local_once(delay,move ||{if let Some(animation)=weak.upgrade(){
        animation.imp().timer.borrow_mut().take();if animation.on_screen(){let count=animation.imp().frames.get().unwrap().len();animation.imp().index.set((animation.imp().index.get()+1)%count);animation.invalidate_contents();}schedule(&animation);
    }}));
}
#[cfg(test)]mod tests{
    use super::*;
    pub(crate) fn fixture()->Vec<u8>{let mut out=vec![];{let mut encoder=image::codecs::gif::GifEncoder::new(&mut out);encoder.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();for color in [[255,0,0,255],[0,255,0,255]]{encoder.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(8,8,image::Rgba(color)),0,0,image::Delay::from_numer_denom_ms(40,1))).unwrap();}}out}
    #[test]fn supplied_giphy_sample_decodes_when_provided(){if let Ok(path)=std::env::var("PAPO_GIPHY_SAMPLE"){let data=std::fs::read(path).unwrap();let animation=PreparedAnimation::decode(&data).expect("public Giphy sample must decode");assert!(animation.frames.len()>1);println!("Giphy sample: {} frames, {} decoded bytes",animation.frames.len(),animation.bytes());}}
    #[test]fn gifs_keep_frames_timing_and_enforce_input_limits(){let gif=PreparedAnimation::decode(&fixture()).unwrap();assert_eq!(gif.frames.len(),2);assert_eq!(gif.frames[0].1,Duration::from_millis(40));assert_ne!(gif.frames[0].0.pixels,gif.frames[1].0.pixels);assert_eq!(gif.bytes(),512);assert!(PreparedAnimation::decode(&vec![0; (4<<20)+1]).is_none());assert!(PreparedAnimation::decode(b"invalid").is_none());}
    pub(crate) fn exercise(context:&glib::MainContext){
        use crate::ui::chat::actions::tests::{until,pump};
        let animation=PreparedAnimation::decode(&fixture()).unwrap().paintable();assert!(animation.imp().timer.borrow().is_none(),"cached GIFs need no timer");
        let picture=animation.picture();picture.set_size_request(80,80);let column=gtk::Box::new(gtk::Orientation::Vertical,0);column.append(&picture);let spacer=gtk::Box::new(gtk::Orientation::Vertical,0);spacer.set_height_request(1000);column.append(&spacer);
        let scroll=gtk::ScrolledWindow::new();scroll.set_child(Some(&column));let window=gtk::Window::builder().default_width(300).default_height(200).child(&scroll).build();window.present();until(context,||picture.is_mapped());
        let first=animation.current_image();until(context,||animation.current_image()!=first);
        scroll.vadjustment().set_value(800.0);pump(context);let offscreen=animation.current_image();for _ in 0..80{pump(context);std::thread::sleep(Duration::from_millis(5));}assert_eq!(animation.current_image(),offscreen,"offscreen GIFs must not advance frames");
        scroll.vadjustment().set_value(0.0);until(context,||animation.current_image()!=offscreen);
        picture.set_visible(false);pump(context);assert!(animation.imp().timer.borrow().is_none(),"unmapped GIFs stop their timer");
        window.set_child(None::<&gtk::Widget>);window.close();scroll.set_child(None::<&gtk::Widget>);drop(column);drop(picture);let weak=animation.downgrade();drop(animation);assert!(weak.upgrade().is_none(),"timer must not retain an invisible GIF");
    }
}
#[cfg(test)]pub(crate) use tests::{exercise,fixture};
