//! Server emoji frames are decoded on a worker and shared at their display size.
use super::{PreparedImage,animation::PreparedAnimation};
use base64::prelude::*;
use image::{AnimationDecoder,ImageDecoder};
use std::time::Duration;

#[derive(Debug)]
pub enum PreparedEmoji { Static(PreparedImage), Animated(PreparedAnimation) }
impl PreparedEmoji {
    pub fn bytes(&self)->usize {match self{Self::Static(image)=>image.pixels.len(),Self::Animated(animation)=>animation.bytes()}}
    pub fn into_static(self)->Self {match self{Self::Animated(animation)=>Self::Static(animation.frames.into_iter().next().unwrap().0),other=>other}}
}
pub fn prepare(blob:&str)->Option<PreparedEmoji>{
    if blob.len()>350_000{return None;}
    let bytes=BASE64_STANDARD.decode(blob).ok()?;
    let format=image::guess_format(&bytes).ok()?;
    let (w,h)=image::ImageReader::with_format(std::io::Cursor::new(&bytes),format).into_dimensions().ok()?;
    if w==0||h==0||w>512||h>512{return None;}
    let mut limits=image::Limits::default();limits.max_alloc=Some(8<<20);
    let frames=match format{
        image::ImageFormat::Gif=>{let mut decoder=image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes)).ok()?;decoder.set_limits(limits).ok()?;Some(decoder.into_frames())},
        image::ImageFormat::WebP=>{let mut decoder=image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(&bytes)).ok()?;decoder.set_limits(limits).ok()?;decoder.has_animation().then(||decoder.into_frames())},
        image::ImageFormat::Png=>{let mut decoder=image::codecs::png::PngDecoder::new(std::io::Cursor::new(&bytes)).ok()?;decoder.set_limits(limits).ok()?;if decoder.is_apng().ok()?{Some(decoder.apng().ok()?.into_frames())}else{None}},
        _=>None,
    };
    if let Some(frames)=frames{
        if let Some(animation)=collect(frames){if animation.frames.len()>1{return Some(PreparedEmoji::Animated(animation));}}
    }
    PreparedImage::decode_thumbnail(&bytes,32,32).map(PreparedEmoji::Static)
}
fn collect(frames:image::Frames<'_>)->Option<PreparedAnimation>{
    let mut prepared=vec![];let mut total=0;
    for (index,frame) in frames.enumerate(){
        if index>=240{return None;}let frame=frame.ok()?;let(n,d)=frame.delay().numer_denom_ms();
        let delay=Duration::from_millis((u64::from(n)/u64::from(d.max(1))).clamp(20,10_000));
        let pixels=image::DynamicImage::ImageRgba8(frame.into_buffer()).thumbnail(32,32).into_rgba8();
        total+=pixels.len();if total>1<<20{return None;}
        prepared.push((PreparedImage{width:pixels.width(),height:pixels.height(),pixels:pixels.into_raw()},delay));
    }
    (!prepared.is_empty()).then_some(PreparedAnimation{frames:prepared})
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn animated_server_emoji_keeps_distinct_frames_and_timing(){
        let blob=BASE64_STANDARD.encode(crate::media::animation::fixture());
        let PreparedEmoji::Animated(animation)=prepare(&blob).unwrap()else{panic!("GIF must remain animated")};
        assert_eq!(animation.frames.len(),2);assert_ne!(animation.frames[0].0.pixels,animation.frames[1].0.pixels);assert_eq!(animation.frames[0].1,Duration::from_millis(40));
        assert!(prepare("invalid").is_none());assert!(prepare(&"a".repeat(350_001)).is_none());
    }
    #[test]fn animated_png_and_webp_are_downsampled_without_losing_animation(){
        for bytes in [include_bytes!("../../tests/fixtures/animated-emoji.png").as_slice(),include_bytes!("../../tests/fixtures/animated-emoji.webp").as_slice()]{
            let PreparedEmoji::Animated(animation)=prepare(&BASE64_STANDARD.encode(bytes)).unwrap()else{panic!("animation was lost")};
            assert_eq!(animation.frames.len(),2);assert_eq!(animation.frames[0].0.width,32);assert_eq!(animation.bytes(),8192);
            assert_ne!(animation.frames[0].0.pixels,animation.frames[1].0.pixels);assert_eq!(animation.frames[1].1,Duration::from_millis(80));
        }
    }
    #[test]fn excessive_frame_count_falls_back_to_a_bounded_thumbnail(){
        let mut data=vec![];{let mut encoder=image::codecs::gif::GifEncoder::new(&mut data);for i in 0..241{encoder.encode_frame(image::Frame::new(image::RgbaImage::from_pixel(1,1,image::Rgba([i as u8,0,0,255])))).unwrap();}}
        let PreparedEmoji::Static(image)=prepare(&BASE64_STANDARD.encode(data)).unwrap()else{panic!("unbounded animation")};assert_eq!(image.pixels.len(),4);
    }
}
