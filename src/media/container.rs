//! EN-US: Permit only self-contained MP4/QuickTime and WebM before native playback.
//! PT-BR: Permite apenas MP4/QuickTime autocontido e WebM antes da reprodução nativa.
use std::{fs::File,io::{Read,Seek,SeekFrom},path::Path};
use anyhow::{Result,ensure,bail};
// EN-US: Inspect the actual container before native playback; a MIME label or suffix is only a UI hint.
// PT-BR: Inspeciona o contêiner real antes da reprodução nativa; MIME e extensão são apenas pistas para a interface.
pub fn validate(path:&Path)->Result<()> {
    let mut file=File::open(path)?;let length=file.metadata()?.len();ensure!((12..=256<<20).contains(&length),"Mídia fora do limite de tamanho.");
    let mut head=[0u8;12];file.read_exact(&mut head)?;
    if head.starts_with(&[0x1a,0x45,0xdf,0xa3]){webm(&mut file,length)?;return Ok(());}
    ensure!(matches!(&head[4..8],b"ftyp"|b"moov"|b"mdat"|b"free"|b"wide"),"Formato de mídia não permitido. Baixe o arquivo para abri-lo externamente.");
    let mut count=0;let mut media=false;boxes(&mut file,0,length,0,&mut count,&mut media)?;ensure!(media,"Contêiner de vídeo vazio.");Ok(())
}
fn boxes(file:&mut File,mut at:u64,end:u64,depth:u32,count:&mut usize,media:&mut bool)->Result<()> {
    ensure!(depth<=16,"Contêiner de vídeo complexo demais.");
    while at<end{
        *count+=1;ensure!(*count<=100_000&&end-at>=8,"Contêiner de vídeo inválido.");file.seek(SeekFrom::Start(at))?;let mut head=[0;8];file.read_exact(&mut head)?;
        let mut size=u32::from_be_bytes(head[..4].try_into()?) as u64;let mut header=8;
        if size==1{let mut ext=[0;8];file.read_exact(&mut ext)?;size=u64::from_be_bytes(ext);header=16;}else if size==0{size=end-at;}
        ensure!(size>=header&&size<=end-at,"Contêiner de vídeo incompleto.");let start=at+header;let next=at+size;
        match &head[4..]{
            b"mdat"=>*media=true,
            b"rmra"|b"rmda"|b"rdrf"|b"urn "=>bail!("Referências externas de mídia bloqueadas."),
            b"url "=>{ensure!(size==header+4,"Referência externa de mídia bloqueada.");let mut flags=[0;4];file.read_exact(&mut flags)?;ensure!(u32::from_be_bytes(flags)==1,"Referência externa de mídia bloqueada.");},
            b"dref"=>{
                ensure!(size>=header+8,"Referência de mídia inválida.");let mut full=[0;8];file.read_exact(&mut full)?;let entries=u32::from_be_bytes(full[4..].try_into()?) as u64;
                ensure!(full[..4]==[0;4]&&entries<=32&&size==header+8+entries*12,"Referências de mídia inválidas.");
                for _ in 0..entries{let mut entry=[0;12];file.read_exact(&mut entry)?;ensure!(entry[..4]==12u32.to_be_bytes()&&&entry[4..8]==b"url "&&entry[8..]==[0,0,0,1],"Referências externas de mídia bloqueadas.");}
            },
            b"moov"|b"trak"|b"mdia"|b"minf"|b"dinf"|b"mvex"|b"moof"|b"traf"=>boxes(file,start,next,depth+1,count,media)?,
            _=>{},
        }at=next;
    }Ok(())
}
fn vint(file:&mut File,id:bool)->Result<(u64,u64)>{
    let mut first=[0];file.read_exact(&mut first)?;let width=first[0].leading_zeros() as u64+1;ensure!(width<=8,"EBML inválido.");
    let mut value=if id{first[0] as u64}else{(first[0]&((1u16<<(8-width))-1) as u8) as u64};
    for _ in 1..width{let mut byte=[0];file.read_exact(&mut byte)?;value=(value<<8)|byte[0] as u64;}Ok((value,width))
}
fn webm(file:&mut File,length:u64)->Result<()>{
    file.seek(SeekFrom::Start(4))?;let (size,width)=vint(file,false)?;ensure!(size<=4096&&4+width+size<length,"Cabeçalho WebM inválido.");let end=4+width+size;let mut found=false;
    while file.stream_position()?<end{let (id,_)=vint(file,true)?;let(size,_)=vint(file,false)?;let at=file.stream_position()?;ensure!(at<=end&&size<=end-at,"Cabeçalho WebM inválido.");
        if id==0x4282{ensure!(size==4,"Apenas WebM autocontido é permitido.");let mut kind=[0;4];file.read_exact(&mut kind)?;ensure!(&kind==b"webm","Tipo de mídia não permitido.");found=true;}
        file.seek(SeekFrom::Start(at+size))?;
    }
    ensure!(found,"Tipo WebM ausente.");let(id,_)=vint(file,true)?;ensure!(id==0x18538067,"Segmento WebM inválido.");Ok(())
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn permits_fixtures_and_rejects_playlists_and_external_mp4_references(){
        for path in ["tests/fixtures/chat-h264.mp4","tests/fixtures/chat-video.webm","tests/fixtures/chat-video.mov"]{validate(Path::new(path)).unwrap();}
        let file=crate::api::features::TemporaryFile(std::env::temp_dir().join(format!("papo-container-{}",uuid::Uuid::new_v4())));
        std::fs::write(&file.0,b"#EXTM3U\nhttp://127.0.0.1/private\n").unwrap();assert!(validate(&file.0).is_err());
        let mut bytes=Vec::new();for (kind,data) in [(b"ftyp",b"isom0000".as_slice()),(b"url ",b"\0\0\0\0http://host".as_slice()),(b"mdat",b"data".as_slice())]{bytes.extend_from_slice(&(data.len() as u32+8).to_be_bytes());bytes.extend_from_slice(kind);bytes.extend_from_slice(data);}
        std::fs::write(&file.0,bytes).unwrap();assert!(validate(&file.0).is_err());
    }
}
