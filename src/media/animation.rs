//! Bounded GIF decoding off the UI thread; frame timers live with the paintable.
//! EN: Decodes animated GIFs safely off the UI thread with strict memory budgets, viewport-aware playback, and system motion preferences.
//! PT-BR: Decodificação limitada de GIFs fora da thread de interface; temporizadores de quadros vinculados ao paintable, com respeito ao orçamento de memória, visibilidade na tela e preferências de movimento do sistema.

use super::PreparedImage;
use gtk::{gdk,glib,prelude::*,subclass::prelude::*};
use std::{cell::{Cell,RefCell,OnceCell},time::Duration};

/// EN: In-memory representation of decoded animation frames and their respective display delays.
/// PT-BR: Representação em memória dos quadros decodificados da animação e seus respectivos tempos de espera (delays).
#[derive(Debug)]
pub struct PreparedAnimation{pub frames:Vec<(PreparedImage,Duration)>}
impl PreparedAnimation{
    /// Publish a bounded first frame before preparing the remaining animation.
    /// EN: Fast-path decode of the first frame (max 420x300) to display an immediate placeholder while remaining frames decode.
    /// PT-BR: Decodificação rápida do primeiro quadro (máx 420x300) para exibir uma prévia imediata enquanto os demais quadros são processados.
    pub fn first(bytes:&[u8])->Option<PreparedImage>{
        use image::{AnimationDecoder,ImageDecoder};
        // EN: Reject files larger than 5MB. / PT-BR: Rejeita arquivos maiores que 5MB.
        if bytes.len()>5<<20{return None;}
        let mut decoder=image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
        let(w,h)=decoder.dimensions();if w==0||h==0||w>2048||h>2048{return None;}
        let mut limits=image::Limits::default();limits.max_alloc=Some(32<<20);decoder.set_limits(limits).ok()?;
        let frame=decoder.into_frames().next()?.ok()?;
        let pixels=image::DynamicImage::ImageRgba8(frame.into_buffer()).thumbnail(420.min(w),300.min(h)).into_rgba8();
        Some(PreparedImage{width:pixels.width(),height:pixels.height(),pixels:pixels.into_raw()})
    }

    /// EN: Decodes all frames up to a limit of 300 frames. If memory exceeds 48MB, dynamically downscales all frames to fit in RAM.
    /// PT-BR: Decodifica todos os quadros até o limite de 300 quadros. Se a memória exceder 48MB, reduz dinamicamente a resolução de todos os quadros para caber na RAM.
    pub fn decode(bytes:&[u8])->Option<Self>{
        use image::{AnimationDecoder,ImageDecoder};
        if bytes.len()>5<<20{return None;}
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
            // EN: Dynamic resolution step-down if decoded uncompressed frames exceed 48MB.
            // PT-BR: Redução dinâmica de resolução caso os quadros não comprimidos excedam 48MB.
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
        let checked=path.to_owned();tokio::task::spawn_blocking(move||super::container::validate(&checked)).await??;
        let mut command=super::sandbox::command(path,include_str!("gif_video.py"))?;
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
        let _=animation.imp().frames.set(frames);ANIMATIONS.with(|all|all.borrow_mut().push(animation.downgrade()));animation
    }
}
mod imp{
    use super::*;
    #[derive(Default)]pub struct Animation{pub frames:OnceCell<Vec<(gdk::Texture,Duration)>>,pub index:Cell<usize>,pub due:Cell<Option<std::time::Instant>>,pub consumers:RefCell<Vec<(glib::WeakRef<gtk::Widget>,Option<i32>)>>}
    #[glib::object_subclass]impl ObjectSubclass for Animation{const NAME:&'static str="PapoGifAnimation";type Type=super::Animation;type Interfaces=(gdk::Paintable,);}
    impl ObjectImpl for Animation{}
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
            let existing=consumers.iter().any(|(w,_)|w.upgrade().as_ref()==Some(widget));if !consumers.iter().any(|(w,o)|w.upgrade().as_ref()==Some(widget)&&*o==offset){consumers.push((widget.downgrade(),offset));}existing};
        if already_watched{return;}
        let weak=self.downgrade();widget.connect_map(move |widget|{let mut parent=widget.parent();while let Some(ancestor)=parent{if let Some(scroll)=ancestor.downcast_ref::<gtk::ScrolledWindow>(){watch_scroll(scroll);}parent=ancestor.parent();}if let Some(animation)=weak.upgrade(){schedule(&animation);}});
        let weak=self.downgrade();widget.connect_unmap(move |_|{if let Some(animation)=weak.upgrade(){
            if !animation.has_mapped_consumer(){animation.imp().due.set(None);}wake();
        }});
        let mut parent=widget.parent();while let Some(ancestor)=parent{if let Some(scroll)=ancestor.downcast_ref::<gtk::ScrolledWindow>(){watch_scroll(scroll);}parent=ancestor.parent();}
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
thread_local!{
    static ANIMATIONS:RefCell<Vec<glib::WeakRef<Animation>>>=const{RefCell::new(Vec::new())};
    static TIMER:RefCell<Option<glib::SourceId>>=const{RefCell::new(None)};
    static SCROLLS:RefCell<Vec<glib::WeakRef<gtk::Adjustment>>>=const{RefCell::new(Vec::new())};
    static MOTION:Cell<crate::config::Motion>=const{Cell::new(crate::config::Motion::System)};
    static SYSTEM_WATCHED:Cell<bool>=const{Cell::new(false)};
    static WAKE_PENDING:Cell<bool>=const{Cell::new(false)};
}
fn allowed()->bool{MOTION.with(|mode|match mode.get(){crate::config::Motion::Always=>true,crate::config::Motion::Reduced=>false,crate::config::Motion::System=>gtk::Settings::default().is_none_or(|s|s.is_gtk_enable_animations())})}
pub fn set_motion(motion:crate::config::Motion){MOTION.with(|mode|mode.set(motion));wake();}
pub fn motion()->crate::config::Motion{MOTION.with(Cell::get)}
fn animations()->Vec<Animation>{ANIMATIONS.with(|all|{let mut all=all.borrow_mut();all.retain(|a|a.upgrade().is_some());all.iter().filter_map(glib::WeakRef::upgrade).collect()})}
fn watch_scroll(scroll:&gtk::ScrolledWindow){
    let adjustment=scroll.vadjustment();let new=SCROLLS.with(|all|{let mut all=all.borrow_mut();all.retain(|a|a.upgrade().is_some());if all.iter().any(|a|a.upgrade().as_ref()==Some(&adjustment)){false}else{all.push(adjustment.downgrade());true}});
    if new{
        // Adjustment signals run during allocation, before the child transform
        // is updated. Check visibility on the next allocated frame as well.
        let pending=std::rc::Rc::new(Cell::new(false));let weak=scroll.downgrade();
        let check:std::rc::Rc<dyn Fn()>=std::rc::Rc::new(move ||{wake();if pending.replace(true){return;}if let Some(scroll)=weak.upgrade(){let pending=pending.clone();let first=Cell::new(true);scroll.add_tick_callback(move |_,_|{if first.replace(false){return glib::ControlFlow::Continue;}pending.set(false);wake();glib::ControlFlow::Break});}else{pending.set(false);}});
        let changed=check.clone();adjustment.connect_value_changed(move |_|changed());adjustment.connect_changed(move |_|check());
    }
}
fn wake(){if WAKE_PENDING.with(|p|p.replace(true)){return;}glib::idle_add_local_once(||{WAKE_PENDING.with(|p|p.set(false));reschedule(false);});}
fn schedule(_animation:&Animation){
    SYSTEM_WATCHED.with(|watched|{if !watched.replace(true){if let Some(settings)=gtk::Settings::default(){settings.connect_gtk_enable_animations_notify(|_|wake());}}});wake();
}
fn reschedule(advance:bool){
    TIMER.with(|timer|{if let Some(timer)=timer.borrow_mut().take(){timer.remove();}});
    let now=std::time::Instant::now();let enabled=allowed();let mut next=None::<std::time::Instant>;
    for animation in animations(){let imp=animation.imp();let frames=imp.frames.get().unwrap();
        if !enabled{imp.due.set(None);if imp.index.replace(0)!=0{animation.invalidate_contents();}continue;}
        if frames.len()<2||!animation.on_screen(){imp.due.set(None);continue;}
        let mut due=imp.due.get().unwrap_or(now+frames[imp.index.get()].1);
        if advance&&due<=now{imp.index.set((imp.index.get()+1)%frames.len());animation.invalidate_contents();crate::performance::record("animation_frames",1);due=now+frames[imp.index.get()].1;}
        imp.due.set(Some(due));next=Some(next.map_or(due,|previous|previous.min(due)));
    }
    if let Some(next)=next{TIMER.with(|timer|*timer.borrow_mut()=Some(glib::timeout_add_local_once(next.saturating_duration_since(now).max(Duration::from_millis(1)),||{TIMER.with(|timer|timer.borrow_mut().take());reschedule(true);})) );}
}
#[cfg(test)]mod tests{
    use super::*;
    pub(crate) fn fixture()->Vec<u8>{let mut out=vec![];{let mut encoder=image::codecs::gif::GifEncoder::new(&mut out);encoder.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();for color in [[255,0,0,255],[0,255,0,255]]{encoder.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(8,8,image::Rgba(color)),0,0,image::Delay::from_numer_denom_ms(40,1))).unwrap();}}out}
    #[test]fn progressive_first_frame_preserves_pixels_and_rejects_oversized_inputs(){
        let bytes=fixture();let first=PreparedAnimation::first(&bytes).unwrap();let full=PreparedAnimation::decode(&bytes).unwrap();
        assert_eq!((first.width,first.height),(full.frames[0].0.width,full.frames[0].0.height));assert_eq!(first.pixels,full.frames[0].0.pixels);
        assert!(PreparedAnimation::first(b"invalid").is_none());assert!(PreparedAnimation::first(&vec![0;(5<<20)+1]).is_none());
        let mut oversized=bytes;oversized[6..8].copy_from_slice(&2049u16.to_le_bytes());assert!(PreparedAnimation::first(&oversized).is_none());
    }
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
        let previous=motion();set_motion(crate::config::Motion::Always);
        let animation=PreparedAnimation::decode(&fixture()).unwrap().paintable();assert!(animation.imp().due.get().is_none(),"cached GIFs need no timer");
        let picture=animation.picture();picture.set_size_request(80,80);let column=gtk::Box::new(gtk::Orientation::Vertical,0);column.append(&picture);let spacer=gtk::Box::new(gtk::Orientation::Vertical,0);spacer.set_height_request(1000);column.append(&spacer);
        let scroll=gtk::ScrolledWindow::new();scroll.set_child(Some(&column));let window=gtk::Window::builder().default_width(300).default_height(200).child(&scroll).build();window.present();until(context,||picture.is_mapped());
        let first=animation.current_image();until(context,||animation.current_image()!=first);
        scroll.vadjustment().set_value(800.0);pump(context);let offscreen=animation.current_image();for _ in 0..80{pump(context);std::thread::sleep(Duration::from_millis(5));}assert_eq!(animation.current_image(),offscreen,"offscreen GIFs must not advance frames");
        scroll.vadjustment().set_value(0.0);until(context,||animation.current_image()!=offscreen);
        set_motion(crate::config::Motion::Reduced);pump(context);assert_eq!(animation.current_image(),animation.first().upcast::<gdk::Paintable>());assert!(animation.imp().due.get().is_none());
        for _ in 0..20{pump(context);std::thread::sleep(Duration::from_millis(5));}assert_eq!(animation.current_image(),animation.first().upcast::<gdk::Paintable>());
        set_motion(crate::config::Motion::Always);until(context,||animation.current_image()!=animation.first().upcast::<gdk::Paintable>());
        picture.set_visible(false);pump(context);assert!(animation.imp().due.get().is_none(),"unmapped GIFs stop their timer");
        window.set_child(None::<&gtk::Widget>);window.close();scroll.set_child(None::<&gtk::Widget>);drop(column);drop(picture);let weak=animation.downgrade();drop(animation);assert!(weak.upgrade().is_none(),"timer must not retain an invisible GIF");set_motion(previous);pump(context);
    }
}
#[cfg(test)]pub(crate) use tests::{exercise,fixture};
