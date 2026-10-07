use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct Emoji {
    pub id: Uuid,
    pub name: String,
    pub image_blob: Option<String>,
    pub format: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct EmojiList { pub emojis: Vec<Emoji>, pub has_more: bool }

#[derive(Debug, Serialize)]
pub struct CreateEmojiRequest { pub name: String, pub format: String, pub image_blob: String }

impl CreateEmojiRequest {
    pub fn from_image(name: &str, bytes: &[u8]) -> anyhow::Result<Self> {
        use base64::Engine;
        anyhow::ensure!(!name.trim().is_empty() && name.chars().count() <= 32, "O nome deve ter entre 1 e 32 caracteres.");
        anyhow::ensure!(!bytes.is_empty() && bytes.len() <= 256 * 1024, "A imagem deve ter até 256 KB.");
        let format = image::guess_format(bytes)?;
        let declared = match format {
            image::ImageFormat::Png => "PNG", image::ImageFormat::Jpeg => "JPEG",
            image::ImageFormat::Gif => "GIF", image::ImageFormat::WebP => "WEBP",
            _ => anyhow::bail!("Use PNG, JPEG, GIF ou WebP."),
        };
        // Validate dimensions before allocating a full decoded image.
        let reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        let (width, height) = reader.into_dimensions()?;
        anyhow::ensure!(width > 0 && height > 0 && width <= 512 && height <= 512, "A imagem deve ter no máximo 512 × 512 pixels.");
        Ok(Self { name: name.to_owned(), format: declared.into(), image_blob: base64::engine::general_purpose::STANDARD.encode(bytes) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn png(width: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(width, 1).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }
    #[test]
    fn emoji_upload_checks_decoded_size_dimensions_format_and_unicode_name() {
        use base64::Engine;
        let bytes = png(1); let name = "🐈".repeat(32);
        let request = CreateEmojiRequest::from_image(&name,&bytes).unwrap();
        assert_eq!(request.format,"PNG");
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(request.image_blob).unwrap(),bytes);
        for (name, bytes) in [("",png(1)),(" ",png(1)),("valid",vec![0; 256*1024+1]),("valid",png(513)),("valid",b"not an image".to_vec())] {
            assert!(CreateEmojiRequest::from_image(name,&bytes).is_err());
        }
        assert!(CreateEmojiRequest::from_image(&"🐈".repeat(33),&png(1)).is_err());
    }
}
