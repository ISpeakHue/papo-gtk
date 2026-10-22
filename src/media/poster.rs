//! EN-US: Extract one silent, small video frame outside GTK's playback lifecycle.
//! PT-BR: Extrai um quadro pequeno e silencioso do vídeo fora do ciclo de vida do reprodutor GTK.
use std::{path::Path,time::Duration};

pub async fn extract(path:&Path)->anyhow::Result<Vec<u8>> {
    let checked=path.to_owned();tokio::task::spawn_blocking(move||super::container::validate(&checked)).await??;
    super::sandbox::prepare().await?;
    let mut command=super::sandbox::command(path,include_str!("poster.py"))?;
    let output=tokio::time::timeout(Duration::from_secs(9),command.output()).await??;
    anyhow::ensure!(output.status.success(),"Não foi possível gerar a prévia do vídeo");
    tokio::task::spawn_blocking(move ||encode(&output.stdout)).await?
}
fn encode(data:&[u8])->anyhow::Result<Vec<u8>> {
    anyhow::ensure!(data.len()>=16&&&data[..8]==b"PAPOPOS1","Quadro de vídeo inválido");
    let width=u32::from_le_bytes(data[8..12].try_into()?);let height=u32::from_le_bytes(data[12..16].try_into()?);
    anyhow::ensure!((1..=320).contains(&width)&&(1..=180).contains(&height)&&data.len()==16+width as usize*height as usize*4,"Geometria da prévia inválida");
    let pixels=image::RgbaImage::from_raw(width,height,data[16..].to_vec()).unwrap();
    let mut encoded=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(pixels).write_to(&mut encoded,image::ImageFormat::Png)?;Ok(encoded.into_inner())
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn validates_bounded_poster_protocol_and_encodes_png(){
        let mut data=b"PAPOPOS1".to_vec();data.extend_from_slice(&2u32.to_le_bytes());data.extend_from_slice(&2u32.to_le_bytes());data.extend_from_slice(&[200;16]);
        let png=encode(&data).unwrap();let image=super::super::PreparedImage::decode_thumbnail(&png,320,180).unwrap();assert_eq!((image.width,image.height),(2,2));
        assert!(encode(&data[..data.len()-1]).is_err());data[8..12].copy_from_slice(&9999u32.to_le_bytes());assert!(encode(&data).is_err());
    }
}
