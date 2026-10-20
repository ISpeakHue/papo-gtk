//! EN-US: Shared limits cover chunked responses and error paths as well as JSON success.
//! PT-BR: Limites compartilhados para respostas recebidas em partes, erros e respostas JSON bem-sucedidas.
use super::*;
use futures_util::StreamExt;
pub(super) const ERROR_LIMIT:usize=64<<10;
fn budget(url:&Url)->usize {
    let path=url.path();
    if path.ends_with("/profile_batch"){160<<20}
    else if path.ends_with("/messages")||path.ends_with("/pinned"){32<<20}
    else if path.ends_with("/emojis"){24<<20}
    else{8<<20}
}
pub(super) async fn read(response:reqwest::Response,limit:usize)->Result<Vec<u8>>{
    anyhow::ensure!(response.content_length().is_none_or(|n|n<=limit as u64),"Resposta da API excede o limite de tamanho.");
    // EN-US: Enforce the budget while streaming too; Content-Length can be absent or inaccurate.
    // PT-BR: Aplica o limite também durante a leitura; Content-Length pode estar ausente ou incorreto.
    let mut bytes=Vec::new();let mut chunks=response.bytes_stream();
    while let Some(chunk)=chunks.next().await{let chunk=chunk?;anyhow::ensure!(chunk.len()<=limit.saturating_sub(bytes.len()),"Resposta da API excede o limite de tamanho.");bytes.extend_from_slice(&chunk);}
    Ok(bytes)
}
pub(super) async fn json_bytes(response:reqwest::Response)->Result<Vec<u8>>{let limit=budget(response.url());read(response,limit).await}
/// EN-US: Apply the same structural limits to HTTP and socket data before GTK renders it.
/// PT-BR: Aplica os mesmos limites estruturais aos dados HTTP e de socket antes da renderização pelo GTK.
pub(crate) fn validate(value:&serde_json::Value)->Result<()> {
    let mut queue=vec![("",value)];let mut nodes=0usize;
    while let Some((key,value))=queue.pop(){
        nodes+=1;anyhow::ensure!(nodes<=100_000,"Resposta da API contém dados demais.");
        match value{
            serde_json::Value::String(text)=>{
                let limit=match key{"content"|"description"|"message_content"|"title"|"value"=>32<<10,"url"|"avatar_url"|"embed_url"=>4096,"image_data"|"author_image_data"|"image_blob"|"icon_blob"|"avatar_blob"|"banner_blob"=>8<<20,_=>1<<20};
                anyhow::ensure!(text.len()<=limit,"Texto da API excede o limite de tamanho.");
            },
            serde_json::Value::Array(items)=>{
                let limit=match key{"messages"=>100,"profiles"=>50,"attachments"=>10,"embeds"=>10,"fields"=>25,"emojis"=>500,_=>4096};
                anyhow::ensure!(items.len()<=limit,"Lista da API excede o limite de itens.");
                queue.extend(items.iter().map(|v|(key,v)));
            },
            serde_json::Value::Object(items)=>{anyhow::ensure!(items.len()<=256,"Objeto da API contém campos demais.");queue.extend(items.iter().map(|(k,v)|(k.as_str(),v)));},_=>{},
        }
    }Ok(())
}
pub(super) fn decode<T:serde::de::DeserializeOwned>(bytes:&[u8])->Result<T>{let value:serde_json::Value=serde_json::from_slice(bytes)?;validate(&value)?;Ok(serde_json::from_value(value)?)}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn rejects_oversized_rendered_text_and_collections(){
        assert!(validate(&serde_json::json!({"content":"x".repeat((32<<10)+1)})).is_err());
        assert!(validate(&serde_json::json!({"messages":vec![serde_json::Value::Null;101]})).is_err());
        assert!(validate(&serde_json::json!({"content":"á".repeat(8192),"embeds":[]})).is_ok());
    }
}
