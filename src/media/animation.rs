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
        let mut frames:Vec<(PreparedImage,Duration)>=vec![];let mut total=0usize;let mut target=(420.min(w),300.min(h));
        for (index,frame) in decoder.into_frames().enumerate(){if index>=300{return None;}let frame=frame.ok()?;let(n,d)=frame.delay().numer_denom_ms();
            let delay=Duration::from_millis((u64::from(n)/u64::from(d.max(1))).clamp(20,10_000));
            let pixels=image::DynamicImage::ImageRgba8(frame.into_buffer()).thumbnail(target.0,target.1).into_rgba8();
            total+=pixels.len();
            frames.push((PreparedImage{width:pixels.width(),height:pixels.height(),pixels:pixels.into_raw()},delay));
            // Preserve the whole animation by reducing preview resolution
            // instead of rejecting a normal GIF when decoded frames fill RAM.
            while total>48<<20 {
                target=((target.0*3/4).max(1),(target.1*3/4).max(1));total=0;
                for (frame,_) in &mut frames {
                    let rgba=image::RgbaImage::from_raw(frame.width,frame.height,std::mem::take(&mut frame.pixels))?;
                    let resized=image::DynamicImage::ImageRgba8(rgba).thumbnail(target.0,target.1).into_rgba8();
                    frame.width=resized.width();frame.height=resized.height();frame.pixels=resized.into_raw();total+=frame.pixels.len();
                }
            }
        }
        if frames.is_empty(){None}else{Some(Self{frames})}
    }
    pub async fn video(path:&std::path::Path)->anyhow::Result<Self>{
        let mut command=crate::media::runtime::python();
        command.args(["-u","-c",include_str!("gif_video.py")]).arg(path).env("GST_DEBUG","0").kill_on_drop(true);
        let output=tokio::time::timeout(Duration::from_secs(20),command.output()).await??;
        anyhow::ensure!(output.status.success(),"Não foi possível decodificar o vídeo do GIF: {}",String::from_utf8_lossy(&output.stderr).trim());
        Self::video_frames(&output.stdout)
    }
    fn video_frames(data:&[u8])->anyhow::Result<Self>{
        anyhow::ensure!(data.len()>=16&&data.len()<=((48<<20)+4096)&&&data[..8]==b"PAPOGIF1","Quadros de GIF inválidos");
        let width=u32::from_le_bytes(data[8..12].try_into()?);let height=u32::from_le_bytes(data[12..16].try_into()?);
        anyhow::ensure!(width>0&&width<=420&&height>0&&height<=300,"Geometria do GIF inválida");
        let size=width as usize*height as usize*4;let mut remaining=&data[16..];let mut frames=vec![];let mut total=0;
        while !remaining.is_empty(){
            anyhow::ensure!(frames.len()<602&&remaining.len()>=size+4,"Quadro de GIF incompleto");
            let delay=u32::from_le_bytes(remaining[..4].try_into()?);anyhow::ensure!((20..=10000).contains(&delay),"Intervalo do GIF inválido");
            total+=size;anyhow::ensure!(total<=48<<20,"GIF maior que o limite de memória");
            frames.push((PreparedImage{width,height,pixels:remaining[4..4+size].to_vec()},Duration::from_millis(u64::from(delay))));remaining=&remaining[size+4..];
        }
        anyhow::ensure!(!frames.is_empty(),"Nenhum quadro de GIF disponível");Ok(Self{frames})
    }
    pub fn bytes(&self)->usize{self.frames.iter().map(|(f,_)|f.pixels.len()).sum()}
    pub fn paintable(self)->Animation{
        let animation:Animation=glib::Object::new();let frames=self.frames.into_iter().map(|(frame,delay)|(frame.texture(),delay)).collect::<Vec<_>>();
        let _=animation.imp().frames.set(frames);animation
    }
}
mod imp{
    use super::*;
    #[derive(Default)]pub struct Animation{pub frames:OnceCell<Vec<(gdk::Texture,Duration)>>,pub index:Cell<usize>,pub timer:RefCell<Option<glib::SourceId>>,pub consumers:RefCell<Vec<(glib::WeakRef<gtk::Widget>,Option<i32>)>>}
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
    pub fn animated(&self)->bool{self.imp().frames.get().unwrap().len()>1}
    pub fn first(&self)->gdk::Texture{self.imp().frames.get().unwrap()[0].0.clone()}
    pub fn picture(&self)->gtk::Picture{
        let picture=gtk::Picture::for_paintable(self);picture.set_can_shrink(true);picture.set_content_fit(gtk::ContentFit::Contain);
        self.watch(&picture,None);picture
    }
    /// Inline text supplies a character offset so an offscreen line does not
    /// animate just because another part of its TextView is visible.
    pub fn watch(&self,widget:&impl IsA<gtk::Widget>,offset:Option<i32>){
        let widget=widget.as_ref();
        let already_watched={let mut consumers=self.imp().consumers.borrow_mut();consumers.retain(|(w,_)|w.upgrade().is_some());
            let existing=consumers.iter().any(|(w,_)|w.upgrade().as_ref()==Some(widget));consumers.push((widget.downgrade(),offset));existing};
        if already_watched{return;}
        let weak=self.downgrade();widget.connect_map(move |_|{if let Some(animation)=weak.upgrade(){schedule(&animation);}});
        let weak=self.downgrade();widget.connect_unmap(move |_|{if let Some(animation)=weak.upgrade(){
            if !animation.has_mapped_consumer(){if let Some(timer)=animation.imp().timer.borrow_mut().take(){timer.remove();}}
        }});
        if widget.is_mapped(){schedule(self);}
    }
    fn has_mapped_consumer(&self)->bool{self.consumers().iter().any(|(w,_)|w.is_mapped())}
    fn consumers(&self)->Vec<(gtk::Widget,Option<i32>)>{
        let mut consumers=self.imp().consumers.borrow_mut();consumers.retain(|(w,_)|w.upgrade().is_some());consumers.iter().filter_map(|(w,offset)|w.upgrade().map(|w|(w,*offset))).collect()
    }
    fn on_screen(&self)->bool{
        self.consumers().iter().any(|(widget,offset)|{
            if !widget.is_mapped(){return false;}
            // Reject clipped rows before asking TextView to validate line layout.
            let bounds=gtk::graphene::Rect::new(0.0,0.0,widget.width() as f32,widget.height() as f32);
            if !in_scroll_viewports(widget,&bounds){return false;}
            let local=if let (Some(offset),Some(view))=(offset,widget.downcast_ref::<gtk::TextView>()){
                let rect=view.iter_location(&view.buffer().iter_at_offset(*offset));
                let (x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x(),rect.y());
                gtk::graphene::Rect::new(x as f32,y as f32,rect.width().max(1) as f32,rect.height().max(1) as f32)
            }else{gtk::graphene::Rect::new(0.0,0.0,widget.width() as f32,widget.height() as f32)};
            in_scroll_viewports(widget,&local)
        })
    }
}
fn in_scroll_viewports(widget:&gtk::Widget,local:&gtk::graphene::Rect)->bool{
    let mut parent=widget.parent();
    while let Some(ancestor)=parent{
        if let Some(scroll)=ancestor.downcast_ref::<gtk::ScrolledWindow>(){
            let Some(transform)=widget.compute_transform(scroll)else{return false;};let r=transform.transform_bounds(local);
            if r.y()+r.height()<=0.0||r.y()>=scroll.height() as f32||r.x()+r.width()<=0.0||r.x()>=scroll.width() as f32{return false;}
        }
        parent=ancestor.parent();
    }true
}
fn schedule(animation:&Animation){
    let frames=animation.imp().frames.get().unwrap();if frames.len()<2||animation.imp().timer.borrow().is_some()||!animation.has_mapped_consumer(){return;}
    // Cached or clipped GIFs must not continually invalidate the entire chat.
    // Poll clipped, mapped consumers slowly so scrolling them into view resumes.
    let delay=if animation.on_screen(){frames[animation.imp().index.get()].1}else{Duration::from_millis(250)};let weak=animation.downgrade();
    *animation.imp().timer.borrow_mut()=Some(glib::timeout_add_local_once(delay,move ||{if let Some(animation)=weak.upgrade(){
        animation.imp().timer.borrow_mut().take();if animation.on_screen(){let count=animation.imp().frames.get().unwrap().len();animation.imp().index.set((animation.imp().index.get()+1)%count);animation.invalidate_contents();}schedule(&animation);
    }}));
}
#[cfg(test)]mod tests{
    use super::*;
    pub(crate) fn fixture()->Vec<u8>{let mut out=vec![];{let mut encoder=image::codecs::gif::GifEncoder::new(&mut out);encoder.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();for color in [[255,0,0,255],[0,255,0,255]]{encoder.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(8,8,image::Rgba(color)),0,0,image::Delay::from_numer_denom_ms(40,1))).unwrap();}}out}
    #[test]fn isolated_video_frames_validate_geometry_budget_and_complete_frames(){
        let mut data=b"PAPOGIF1".to_vec();data.extend_from_slice(&2u32.to_le_bytes());data.extend_from_slice(&2u32.to_le_bytes());data.extend_from_slice(&100u32.to_le_bytes());data.extend_from_slice(&[255;16]);
        let animation=PreparedAnimation::video_frames(&data).unwrap();assert_eq!(animation.frames.len(),1);assert_eq!(animation.bytes(),16);assert_eq!(animation.frames[0].1,Duration::from_millis(100));
        assert!(PreparedAnimation::video_frames(&data[..data.len()-1]).is_err());data[8..12].copy_from_slice(&9999u32.to_le_bytes());assert!(PreparedAnimation::video_frames(&data).is_err());
    }
    #[test]fn supplied_giphy_sample_decodes_when_provided(){if let Ok(path)=std::env::var("PAPO_GIPHY_SAMPLE"){let data=std::fs::read(path).unwrap();let animation=PreparedAnimation::decode(&data).expect("public Giphy sample must decode");assert!(animation.frames.len()>1);println!("Giphy sample: {} frames, {} decoded bytes",animation.frames.len(),animation.bytes());}}
    #[test]fn gifs_keep_frames_timing_and_enforce_input_limits(){let gif=PreparedAnimation::decode(&fixture()).unwrap();assert_eq!(gif.frames.len(),2);assert_eq!(gif.frames[0].1,Duration::from_millis(40));assert_ne!(gif.frames[0].0.pixels,gif.frames[1].0.pixels);assert_eq!(gif.bytes(),512);assert!(PreparedAnimation::decode(&vec![0; (4<<20)+1]).is_none());assert!(PreparedAnimation::decode(b"invalid").is_none());}
    #[test]fn accepts_backend_gif_thumbnails_with_three_hundred_frames(){
        let mut bytes=Vec::new();{
            let mut encoder=image::codecs::gif::GifEncoder::new(&mut bytes);
            for i in 0..300{encoder.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(2,2,image::Rgba([i as u8,100,200,255])),0,0,image::Delay::from_numer_denom_ms(40,1))).unwrap();}
        }
        let decoded=PreparedAnimation::decode(&bytes).expect("the backend permits 300 thumbnail frames");assert_eq!(decoded.frames.len(),300);assert_eq!(decoded.bytes(),4800);
    }
    #[test]fn long_gifs_reduce_resolution_without_dropping_frames_or_exceeding_memory_budget(){
        let mut bytes=Vec::new();{
            let mut encoder=image::codecs::gif::GifEncoder::new(&mut bytes);
            for i in 0..100{encoder.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(420,300,image::Rgba([i as u8,100,200,255])),0,0,image::Delay::from_numer_denom_ms(40,1))).unwrap();}
        }
        let decoded=PreparedAnimation::decode(&bytes).unwrap();assert_eq!(decoded.frames.len(),100);assert!(decoded.bytes()<=48<<20);
        let first=&decoded.frames[0].0;assert!(first.width<420);
        assert!(decoded.frames.iter().all(|(frame,delay)|frame.width==first.width&&frame.height==first.height&&*delay==Duration::from_millis(40)));
        assert_ne!(first.pixels,decoded.frames.last().unwrap().0.pixels);
    }
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
