//! Klipy's public website lookup returns renditions for a share-link slug.
//! It is separate from the partner API: no app key, API session or cookies.
use reqwest::Url;

const METADATA_LIMIT:usize=256<<10;
fn lookup(link:&Url)->Option<(Url,&str)>{
    if !matches!(link.host_str(),Some("klipy.com"|"www.klipy.com")){return None;}
    // Reuse the share-link validator before putting a slug in a request path.
    super::page(link.as_str())?;
    let slug=link.path_segments()?.filter(|part|!part.is_empty()).nth(1)?;
    Some((Url::parse(&format!("https://klipy.com/api/proxy/web/gifs/{slug}")).ok()?,slug))
}
fn rendition(json:&serde_json::Value,slug:&str)->Option<(Url,usize)>{
    let item=&json["data"];
    if json["result"].as_bool()!=Some(true)||item["slug"].as_str()!=Some(slug)||item["type"].as_str()!=Some("gif"){return None;}
    // Prefer an actual GIF. A video rendition uses the same isolated, silent
    // animation decoder as bot GIFs, never a GTK video player.
    for (format,limit,maximum) in [("gif",4usize<<20,2048),("mp4",8<<20,4096),("webm",8<<20,4096)]{
        for tier in ["md","sm","hd","xs"]{
            let media=&item["file"][tier][format];
            if media["size"].as_u64().is_none_or(|n|n==0||n>limit as u64){continue;}
            if ["width","height"].iter().any(|name|media[name].as_u64().is_none_or(|n|n==0||n>maximum)){continue;}
            let url=media["url"].as_str().and_then(|value|if format=="gif"{super::media_url(value)}else{super::video_url(value)});
            if let Some(url)=url.filter(|url|matches!(url.host_str(),Some("static.klipy.com"|"static1.klipy.com"|"static2.klipy.com"))){return Some((url,limit));}
        }
    }None
}
async fn fetch_with<F,Fut>(link:&Url,download:F)->anyhow::Result<Vec<u8>>
where F:Fn(Url,usize)->Fut,Fut:std::future::Future<Output=anyhow::Result<Vec<u8>>>{
    let (url,slug)=lookup(link).ok_or_else(||anyhow::anyhow!("Link Klipy inválido"))?;
    let metadata=download(url,METADATA_LIMIT).await?;
    anyhow::ensure!(metadata.len()<=METADATA_LIMIT,"Metadados Klipy maiores que o limite");
    let json:serde_json::Value=serde_json::from_slice(&metadata)?;
    let (url,limit)=rendition(&json,slug).ok_or_else(||anyhow::anyhow!("O Klipy não disponibilizou uma animação para este link"))?;
    let bytes=download(url,limit).await?;
    anyhow::ensure!(bytes.len()<=limit&&(bytes.starts_with(b"GIF8")||super::is_video(&bytes)),"Animação Klipy inválida");
    Ok(bytes)
}
pub(super) async fn fetch(link:&Url)->anyhow::Result<Vec<u8>>{fetch_with(link,super::bytes).await}

#[cfg(test)]mod tests{
    use super::*;
    use serde_json::json;
    fn item()->serde_json::Value{json!({"result":true,"data":{"slug":"halloween-scary-22","type":"gif","file":{
        "md":{"gif":{"url":"https://static2.klipy.com/example.gif?delivery=original&token=unchanged","size":1000,"width":480,"height":480}},
        "sm":{"gif":{"url":"https://static.klipy.com/small.gif","size":100,"width":220,"height":220}},
        "hd":{"mp4":{"url":"https://static1.klipy.com/video.mp4","size":100,"width":480,"height":480}}
    }}})}
    #[tokio::test]
    #[ignore="requires public Klipy access; no account or credentials"]
    async fn public_share_links_return_decodable_animations(){
        for slug in ["halloween-scary-22","happycadogt-bang","hop-on-google-docs-1"]{
            let url=Url::parse(&format!("https://klipy.com/gifs/{slug}")).unwrap();
            let bytes=super::super::fetch(url.as_str(),None).await.expect("public Klipy lookup must succeed");
            let prepared=crate::media::animation::PreparedAnimation::decode(&bytes).expect("Klipy GIF rendition must decode");
            assert!(prepared.frames.len()>1,"a still image cannot replace the GIF");
            eprintln!("Klipy {slug}: {} bytes, {} frames",bytes.len(),prepared.frames.len());
        }
    }
    #[test]fn exact_slug_and_safe_animated_renditions_are_required(){
        assert_eq!(lookup(&Url::parse("https://klipy.com//gifs//halloween-scary-22/").unwrap()).unwrap().0.path(),"/api/proxy/web/gifs/halloween-scary-22");
        let mut data=item();let(url,limit)=rendition(&data,"halloween-scary-22").unwrap();assert_eq!(limit,4<<20);assert_eq!(url.query(),Some("delivery=original&token=unchanged"));
        assert!(rendition(&data,"another-gif").is_none());
        data["data"]["file"]["md"]["gif"]["size"]=json!(5<<20);assert_eq!(rendition(&data,"halloween-scary-22").unwrap().0.path(),"/small.gif");
        data["data"]["file"]["sm"]["gif"]["width"]=json!(100000);assert_eq!(rendition(&data,"halloween-scary-22").unwrap().0.path(),"/video.mp4");
        data["data"]["file"]["hd"]["mp4"]["url"]=json!("https://static1.klipy.com.evil/video.mp4");assert!(rendition(&data,"halloween-scary-22").is_none());
        for value in ["https://user:pass@static.klipy.com/a.gif","http://static.klipy.com/a.gif","https://static.klipy.com:8080/a.gif","https://media.tenor.com/a.gif","https://static.klipy.com/static.webp"]{
            let mut data=item();data["data"]["file"]=json!({"md":{"gif":{"url":value,"size":1,"width":1,"height":1}}});assert!(rendition(&data,"halloween-scary-22").is_none(),"{value}");
        }
        let mut data=item();data["result"]=json!(false);assert!(rendition(&data,"halloween-scary-22").is_none());
        data["result"]=json!(true);data["data"]["type"]=json!("clip");assert!(rendition(&data,"halloween-scary-22").is_none());
    }
    #[tokio::test]async fn pasted_link_uses_one_lookup_and_one_media_get_without_fetching_the_page(){
        let calls=std::sync::Mutex::new(vec![]);let data=item();
        let bytes=fetch_with(&Url::parse("https://www.klipy.com/gifs/halloween-scary-22?share=1").unwrap(),|url,limit|{
            calls.lock().unwrap().push((url.clone(),limit));let data=data.clone();async move{
                if url.path().starts_with("/api/proxy/"){Ok(serde_json::to_vec(&data).unwrap())}else{Ok(crate::media::animation::fixture())}
            }
        }).await.unwrap();
        assert!(crate::media::animation::PreparedAnimation::decode(&bytes).unwrap().frames.len()>1);
        let calls=calls.lock().unwrap();assert_eq!(calls.len(),2);assert_eq!(calls[0].0.as_str(),"https://klipy.com/api/proxy/web/gifs/halloween-scary-22");assert_eq!(calls[0].1,METADATA_LIMIT);assert_eq!(calls[1].0.host_str(),Some("static2.klipy.com"));
    }
    #[tokio::test]async fn wrong_or_missing_item_never_downloads_unrelated_media(){
        let calls=std::sync::atomic::AtomicUsize::new(0);
        assert!(fetch_with(&Url::parse("https://klipy.com/gifs/missing").unwrap(),|_,_|{
            calls.fetch_add(1,std::sync::atomic::Ordering::Relaxed);async{Ok(serde_json::to_vec(&item()).unwrap())}
        }).await.is_err());assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed),1);
        for value in ["http://klipy.com/gifs/a","https://klipy.com:8080/gifs/a","https://user@klipy.com/gifs/a","https://klipy.com.evil/gifs/a","https://klipy.com/gifs/a/b"]{assert!(lookup(&Url::parse(value).unwrap()).is_none());}
    }
}
