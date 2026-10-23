//! Media utilities — Base64 decoding, GDK texture generation, and file size formatting.
//! EN-US: Manages image processing, bounded memory decoding, thumbnail downscaling, and GDK texture generation for UI display.
//! PT-BR: Utilitários de mídia — decodificação em memória limitada, miniaturas, geração de texturas GDK e manipulação de avatares/anexos.

use gtk::gdk::Texture;
use gtk::glib::Bytes;
use gtk::prelude::*;

pub mod avatars;
pub mod sound;
pub mod runtime;
pub mod preview;
pub mod inline;
pub mod frame;
pub mod animation;
pub mod emoji;
pub mod giphy;
pub mod gif_links;
pub mod poster;
pub mod container;
mod sandbox;

/// Bound encoded input, dimensions and decoded cache size before creating a texture.
/// EN-US: Decodes image bytes into a GDK texture, constraining bounds to a default 512x512 box.
/// PT-BR: Decodifica bytes de imagem em uma textura GDK, restringindo as dimensões a uma caixa máxima de 512x512.
pub fn bounded_texture(bytes: &[u8]) -> Option<Texture> {
    bounded_texture_size(bytes,512,512)
}

/// EN-US: Prepared raw RGBA image data ready for texture upload on the UI thread.
/// PT-BR: Dados brutos de imagem RGBA prontos para upload como textura na thread de interface.
#[derive(Debug)]
pub struct PreparedImage{pub width:u32,pub height:u32,pub pixels:Vec<u8>}
impl PreparedImage{
    /// Original attachments are decoded only for an open viewer. Keep the
    /// encoded file on disk and bound source pixels and the displayed texture.
    /// EN-US: Decodes full-resolution image for the modal lightbox viewer with strict memory bounds (192MB max alloc).
    /// PT-BR: Decodifica imagem em resolução completa para o visualizador modal com limites estritos de alocação (máx 192MB).
    pub fn decode_viewer(path:&std::path::Path)->Option<Self>{
        let reader=image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
        let(w,h)=reader.into_dimensions().ok()?;
        if w==0||h==0||w>16384||h>16384||u64::from(w)*u64::from(h)>32_000_000{return None;}
        let mut reader=image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
        let mut limits=image::Limits::default();limits.max_alloc=Some(192<<20);reader.limits(limits);
        let image=reader.decode().ok()?;
        let image=if w>4096||h>4096{image.resize(4096,4096,image::imageops::FilterType::Lanczos3)}else{image};
        let pixels=image.into_rgba8();Some(Self{width:pixels.width(),height:pixels.height(),pixels:pixels.into_raw()})
    }

    /// EN-US: Decodes image bytes upscaling or downscaling to target dimensions.
    /// PT-BR: Decodifica bytes de imagem ajustando (ampliando ou reduzindo) para as dimensões alvo.
    pub fn decode(bytes:&[u8],width:u32,height:u32)->Option<Self>{Self::decode_impl(bytes,width,height,true)}

    /// Chat frames handle enlargement at presentation time. Other native image
    /// widgets retain their established decoding size (notably avatars).
    /// EN-US: Decodes image bytes into a thumbnail without enlarging smaller originals.
    /// PT-BR: Decodifica bytes de imagem em miniatura sem ampliar originais que já sejam menores.
    pub fn decode_thumbnail(bytes:&[u8],width:u32,height:u32)->Option<Self>{Self::decode_impl(bytes,width,height,false)}

    // EN-US: Internal decoding implementation with bounds checking (max 4MB file, max 4096x4096px).
    // PT-BR: Implementação interna de decodificação com checagem de limites (arquivo máx 4MB, dimensões máx 4096x4096px).
    fn decode_impl(bytes:&[u8],width:u32,height:u32,enlarge:bool)->Option<Self>{
        if bytes.len()>4<<20{return None;}
        let format=image::guess_format(bytes).ok()?;let(w,h)=image::ImageReader::with_format(std::io::Cursor::new(bytes),format).into_dimensions().ok()?;
        if w==0||h==0||w>4096||h>4096||u64::from(w)*u64::from(h)>16_777_216{return None;}
        let (width,height)=if enlarge{(width,height)}else{(width.min(w),height.min(h))};
        let pixels=image::load_from_memory_with_format(bytes,format).ok()?.thumbnail(width,height).to_rgba8();let(w,h)=pixels.dimensions();Some(Self{width:w,height:h,pixels:pixels.into_raw()})
    }

    /// EN-US: Convert raw RGBA pixels into a `gdk::Texture` (`gdk::MemoryTexture`).
    /// PT-BR: Converte pixels brutos RGBA em uma textura `gdk::Texture` (`gdk::MemoryTexture`).
    pub fn texture(self)->Texture{use gtk::prelude::*;gtk::gdk::MemoryTexture::new(self.width as i32,self.height as i32,gtk::gdk::MemoryFormat::R8g8b8a8,&Bytes::from_owned(self.pixels),self.width as usize*4).upcast()}
}

/// EN-US: Helper to decode and directly create a bounded texture in a single step.
/// PT-BR: Auxiliar para decodificar e criar diretamente uma textura limitada em um único passo.
pub fn bounded_texture_size(bytes:&[u8],width:u32,height:u32)->Option<Texture>{PreparedImage::decode(bytes,width,height).map(PreparedImage::texture)}

/// Reuse decoded textures when rebuilding rows, with a fallback for missing avatars.
/// EN-US: Construct an `adw::Avatar` widget with an optional texture and fallback icon.
/// PT-BR: Constrói um widget `adw::Avatar` com textura opcional e ícone padrão de fallback.
pub fn avatar_image(texture: Option<&Texture>, size: i32) -> adw::Avatar {
    let image = adw::Avatar::new(size, None, false);
    image.set_icon_name(Some("papo-avatar-default-symbolic"));
    set_avatar(&image, texture);
    image
}

/// Stable avatar widgets allow asynchronous batches to update only the image.
/// EN-US: Create an avatar widget labeled with user ID for targeted in-place updates.
/// PT-BR: Cria um widget de avatar nomeado com o ID do usuário para atualizações direcionadas sem reconstruir o layout.
pub fn member_avatar(id:uuid::Uuid,texture:Option<&Texture>,size:i32)->adw::Avatar{
    let avatar=avatar_image(texture,size);avatar.set_widget_name(&format!("avatar-{id}"));avatar
}
pub fn update_member_avatars(root:&gtk::Widget,old:&std::collections::HashMap<uuid::Uuid,Texture>,new:&std::collections::HashMap<uuid::Uuid,Texture>){
    use gtk::prelude::*;
    let changed:std::collections::HashSet<_>=old.keys().chain(new.keys()).copied().filter(|id|old.get(id)!=new.get(id)).collect();
    if changed.is_empty(){return;}
    fn visit(widget:&gtk::Widget,changed:&std::collections::HashSet<uuid::Uuid>,textures:&std::collections::HashMap<uuid::Uuid,Texture>){
        if let Some(avatar)=widget.downcast_ref::<adw::Avatar>(){
            if let Some(id)=avatar.widget_name().strip_prefix("avatar-").and_then(|id|uuid::Uuid::parse_str(id).ok()).filter(|id|changed.contains(id)){set_avatar(avatar,textures.get(&id));}
            return;
        }
        let mut child=widget.first_child();while let Some(w)=child{visit(&w,changed,textures);child=w.next_sibling();}
    }
    visit(root,&changed,new);
}

pub fn set_avatar(image: &adw::Avatar, texture: Option<&Texture>) {
    image.set_custom_image(texture);
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

}

#[cfg(test)]
mod prepared_image_tests {
    #[test]
    fn supported_still_formats_decode_to_shared_rgba_thumbnails(){
        for format in [image::ImageFormat::Png,image::ImageFormat::Jpeg,image::ImageFormat::WebP,image::ImageFormat::Gif]{
            let mut bytes=std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(32,16).write_to(&mut bytes,format).unwrap();
            let image=super::PreparedImage::decode_thumbnail(bytes.get_ref(),680,460).unwrap();
            assert_eq!((image.width,image.height,image.pixels.len()),(32,16,32*16*4),"{format:?}");
        }
    }
    #[test]
    fn viewer_preserves_native_detail_and_bounds_large_or_invalid_sources(){
        let path=std::env::temp_dir().join(format!("papo-viewer-test-{}",uuid::Uuid::new_v4()));
        let file=crate::api::features::TemporaryFile(path);
        for (width,height,expected) in [(1800,900,Some((1800,900))),(5000,2,Some((4096,2))),(17000,1,None)] {
            let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(width,height).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
            std::fs::write(&file.0,bytes.into_inner()).unwrap();
            assert_eq!(super::PreparedImage::decode_viewer(&file.0).map(|i|(i.width,i.height)),expected);
        }
        std::fs::write(&file.0,b"not an image").unwrap();assert!(super::PreparedImage::decode_viewer(&file.0).is_none());
    }
    #[test]
    fn decoding_preserves_small_sources_and_bounds_large_images(){
        for (width,height,expected) in [(32,32,(32,32)),(1024,256,(680,170)),(800,1200,(307,460))]{
            let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(width,height).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
            let image=super::PreparedImage::decode_thumbnail(&bytes.into_inner(),680,460).unwrap();
            assert_eq!((image.width,image.height),expected);assert_eq!(image.pixels.len(),(image.width*image.height*4) as usize);
        }
    }
}
