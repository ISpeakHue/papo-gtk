//! Media utilities — Base64 decoding, GDK texture generation, and file size formatting.

use base64::prelude::*;
use gtk::gdk::Texture;
use gtk::glib::Bytes;
use tracing::warn;

pub mod avatars;
pub mod sound;
pub mod preview;
pub mod frame;
pub mod animation;
pub mod giphy;

/// Bound encoded input, dimensions and decoded cache size before creating a texture.
pub fn bounded_texture(bytes: &[u8]) -> Option<Texture> {
    bounded_texture_size(bytes,512,512)
}
#[derive(Debug)]
pub struct PreparedImage{pub width:u32,pub height:u32,pub pixels:Vec<u8>}
impl PreparedImage{
    pub fn decode(bytes:&[u8],width:u32,height:u32)->Option<Self>{Self::decode_impl(bytes,width,height,true)}
    /// Chat frames handle enlargement at presentation time. Other native image
    /// widgets retain their established decoding size (notably avatars).
    pub fn decode_thumbnail(bytes:&[u8],width:u32,height:u32)->Option<Self>{Self::decode_impl(bytes,width,height,false)}
    fn decode_impl(bytes:&[u8],width:u32,height:u32,enlarge:bool)->Option<Self>{
        if bytes.len()>4<<20{return None;}
        let format=image::guess_format(bytes).ok()?;let(w,h)=image::ImageReader::with_format(std::io::Cursor::new(bytes),format).into_dimensions().ok()?;
        if w==0||h==0||w>4096||h>4096||u64::from(w)*u64::from(h)>16_777_216{return None;}
        let (width,height)=if enlarge{(width,height)}else{(width.min(w),height.min(h))};
        let pixels=image::load_from_memory_with_format(bytes,format).ok()?.thumbnail(width,height).to_rgba8();let(w,h)=pixels.dimensions();Some(Self{width:w,height:h,pixels:pixels.into_raw()})
    }
    pub fn texture(self)->Texture{use gtk::prelude::*;gtk::gdk::MemoryTexture::new(self.width as i32,self.height as i32,gtk::gdk::MemoryFormat::R8g8b8a8,&Bytes::from_owned(self.pixels),self.width as usize*4).upcast()}
}
pub fn bounded_texture_size(bytes:&[u8],width:u32,height:u32)->Option<Texture>{PreparedImage::decode(bytes,width,height).map(PreparedImage::texture)}

/// Compact static emoji textures. Bound decoding and discard large source blobs.
pub fn emoji_texture(b64: &str) -> Option<Texture> {
    prepare_emoji(b64).map(PreparedImage::texture)
}
pub fn prepare_emoji(b64:&str)->Option<PreparedImage>{
    if b64.len()>350_000{return None;}
    let data=BASE64_STANDARD.decode(b64).ok()?;
    let format=image::guess_format(&data).ok()?;
    let(w,h)=image::ImageReader::with_format(std::io::Cursor::new(&data),format).into_dimensions().ok()?;
    if w==0||h==0||w>512||h>512{return None;}
    PreparedImage::decode_thumbnail(&data,32,32)
}

/// Reuse decoded textures when rebuilding rows, with a fallback for missing avatars.
pub fn avatar_image(texture: Option<&Texture>, size: i32) -> adw::Avatar {
    let image = adw::Avatar::new(size, None, false);
    image.set_icon_name(Some("avatar-default-symbolic"));
    set_avatar(&image, texture);
    image
}

pub fn set_avatar(image: &adw::Avatar, texture: Option<&Texture>) {
    image.set_custom_image(texture);
}

/// Decodes base64 image data and creates a `gdk::Texture`.
pub fn texture_from_base64(b64: &str) -> Option<Texture> {
    // Strip data URL scheme if present (e.g. "data:image/png;base64,...")
    let clean_b64 = if let Some(idx) = b64.find(',') {
        &b64[idx + 1..]
    } else {
        b64
    };

    match BASE64_STANDARD.decode(clean_b64.trim()) {
        Ok(bytes) => texture_from_bytes(&bytes),
        Err(e) => {
            warn!("Failed to decode base64 image: {e}");
            None
        }
    }
}

/// Creates a `gdk::Texture` from raw image bytes.
pub fn texture_from_bytes(bytes: &[u8]) -> Option<Texture> {
    let glib_bytes = Bytes::from(bytes);
    match Texture::from_bytes(&glib_bytes) {
        Ok(texture) => Some(texture),
        Err(e) => {
            // GDK guarantees PNG/JPEG/TIFF support, while the backend also
            // accepts GIF and WebP avatars. Decode their first frame in Rust.
            use gtk::prelude::*;
            match image::load_from_memory(bytes) {
                Ok(image) => {
                    let image = image.to_rgba8();
                    let width = i32::try_from(image.width()).ok()?;
                    let height = i32::try_from(image.height()).ok()?;
                    let stride = image.width() as usize * 4;
                    let bytes = Bytes::from_owned(image.into_raw());
                    Some(gtk::gdk::MemoryTexture::new(width, height,
                        gtk::gdk::MemoryFormat::R8g8b8a8, &bytes, stride).upcast())
                }
                Err(decode_error) => {
                    warn!("Failed to decode image: {decode_error} (GDK: {e})");
                    None
                }
            }
        }
    }
}

/// Formats a byte size into a human-readable string (e.g. "1.2 MB").
pub fn format_file_size(bytes: i64) -> String {
    let bytes = bytes.max(0);
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size_units() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(-100), "0 B");
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_file_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_file_size(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn test_texture_from_invalid_base64() {
        assert!(texture_from_base64("!!!invalid-base-64!!!").is_none());
    }
}

#[cfg(test)]
mod prepared_image_tests {
    #[test]
    fn decoding_preserves_small_sources_and_bounds_large_images(){
        for (width,height,expected) in [(32,32,(32,32)),(1024,256,(680,170)),(800,1200,(307,460))]{
            let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(width,height).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
            let image=super::PreparedImage::decode_thumbnail(&bytes.into_inner(),680,460).unwrap();
            assert_eq!((image.width,image.height),expected);assert_eq!(image.pixels.len(),(image.width*image.height*4) as usize);
        }
    }
}
