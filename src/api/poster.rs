//! Bounded, authenticated ranges for a silent video poster, never a full background download.
use super::*;
use super::requests::PacedRequest;
use futures_util::StreamExt;
use tokio::io::{AsyncSeekExt,AsyncWriteExt};

const PREFIX:u64=2<<20;
const TAIL:u64=256<<10;
const MAX_VIDEO:u64=256<<20;

impl ApiClient {
    async fn poster_range(&self,path:&str,start:u64,end:u64)->Result<(Vec<u8>,u64)> {
        let response=Self::check_response(self.inner.get(self.url(path))
            .header(reqwest::header::RANGE,format!("bytes={start}-{end}"))
            .timeout(Duration::from_secs(8)).send_paced(&self.requests).await?,"carregar prévia do vídeo").await?;
        let maximum=end-start+1;
        let total=if response.status()==StatusCode::PARTIAL_CONTENT {
            let value=response.headers().get(reqwest::header::CONTENT_RANGE).context("Resposta sem Content-Range")?.to_str()?;
            let (range,total)=value.strip_prefix("bytes ").context("Intervalo de vídeo inválido")?.split_once('/').context("Intervalo de vídeo inválido")?;
            let (first,last)=range.split_once('-').context("Intervalo de vídeo inválido")?;
            let (first,last,total):(u64,u64,u64)=(first.parse()?,last.parse()?,total.parse()?);
            anyhow::ensure!(first==start&&last>=first&&last<=end&&last<total&&total<=MAX_VIDEO,"Intervalo de vídeo fora dos limites");
            Some((total,last-first+1))
        }else {
            // A server ignoring Range is safe only for a genuinely small file.
            anyhow::ensure!(response.status()==StatusCode::OK&&start==0,"Servidor não suporta prévia parcial");None
        };
        anyhow::ensure!(response.content_length().is_none_or(|n|n<=maximum),"Servidor retornou vídeo inteiro para uma prévia");
        let mut stream=response.bytes_stream();let mut bytes=Vec::new();
        while let Some(chunk)=stream.next().await{let chunk=chunk?;anyhow::ensure!(bytes.len() as u64+chunk.len() as u64<=maximum,"Prévia do vídeo excede o limite");bytes.extend_from_slice(&chunk);}
        anyhow::ensure!(!bytes.is_empty(),"Vídeo vazio");
        if let Some((total,length))=total{anyhow::ensure!(bytes.len() as u64==length,"Intervalo de vídeo incompleto");Ok((bytes,total))}else{let size=bytes.len() as u64;Ok((bytes,size))}
    }

    pub(crate) async fn video_preview_file(&self,path:&str)->Result<features::TemporaryFile> {
        let (prefix,total)=self.poster_range(path,0,PREFIX-1).await?;
        let name=std::env::temp_dir().join(format!("papo-poster-{}",Uuid::new_v4()));
        let mut options=tokio::fs::OpenOptions::new();options.write(true).create_new(true);
        #[cfg(unix)]options.mode(0o600);
        let mut output=options.open(&name).await?;let file=features::TemporaryFile(name);
        // Sparse gaps permit MP4 demuxers to read metadata at the end without
        // transferring or allocating the intervening video payload.
        output.set_len(total).await?;output.write_all(&prefix).await?;
        if total>prefix.len() as u64 {
            let start=(total.saturating_sub(TAIL)).max(prefix.len() as u64);
            let (tail,length)=self.poster_range(path,start,total-1).await?;
            anyhow::ensure!(length==total,"Leitura parcial de outro vídeo");
            output.seek(std::io::SeekFrom::Start(start)).await?;output.write_all(&tail).await?;
        }
        output.flush().await?;Ok(file)
    }
}

#[cfg(test)]mod tests {
    use super::*;
    use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener};
    #[tokio::test]async fn reads_only_prefix_and_tail_and_preserves_sparse_offsets(){
        let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let api=ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
        let server=tokio::spawn(async move{
            let total=3<<20;let mut ranges=Vec::new();
            for (start,end) in [(0,PREFIX-1),(total-TAIL,total-1)] {
                let (mut socket,_)=listener.accept().await.unwrap();let mut request=Vec::new();
                loop{let mut part=[0;2048];let n=socket.read(&mut part).await.unwrap();request.extend_from_slice(&part[..n]);if request.windows(4).any(|w|w==b"\r\n\r\n"){break;}}
                let text=String::from_utf8(request).unwrap();let header=text.lines().find(|s|s.to_ascii_lowercase().starts_with("range:")).unwrap().to_ascii_lowercase();ranges.push(header);
                let response=format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",end-start+1);
                socket.write_all(response.as_bytes()).await.unwrap();socket.write_all(&vec![0xab;(end-start+1) as usize]).await.unwrap();
            }ranges
        });
        let file=api.video_preview_file("/attachments/video").await.unwrap();let data=tokio::fs::read(&file.0).await.unwrap();
        assert_eq!(data.len(),3<<20);assert_eq!(data[0],0xab);assert_eq!(data[PREFIX as usize+1],0);assert_eq!(*data.last().unwrap(),0xab);
        #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(&file.0).unwrap().permissions().mode()&0o777,0o600);}
        let requests=server.await.unwrap();assert!(requests[0].contains("bytes=0-2097151"));assert!(requests[1].contains("bytes=2883584-3145727"));
        let path=file.0.clone();drop(file);assert!(!path.exists());
    }
    #[tokio::test]async fn rejects_ignored_or_invalid_ranges_before_downloading_the_body(){
        for headers in ["200 OK\r\nContent-Length: 9000000","206 Partial Content\r\nContent-Length: 1\r\nContent-Range: bytes 1-1/10","206 Partial Content\r\nContent-Length: 1\r\nContent-Range: bytes 0-0/999999999"] {
            let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let api=ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();let headers=headers.to_string();
            let server=tokio::spawn(async move{let(mut socket,_)=listener.accept().await.unwrap();let mut request=[0;4096];let _=socket.read(&mut request).await;let _=socket.write_all(format!("HTTP/1.1 {headers}\r\nConnection: close\r\n\r\n").as_bytes()).await;});
            assert!(api.video_preview_file("/attachments/video").await.is_err());server.await.unwrap();
        }
    }
}
