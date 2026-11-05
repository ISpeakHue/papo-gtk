//! EN-US: Bounded media discovery using existing search filters, never a history scan.
//! PT-BR: Descoberta limitada por filtros de busca existentes, sem varrer o histórico.
use super::*;
use futures_util::{stream,StreamExt,TryStreamExt};
#[derive(Debug,Clone,Default)]pub struct MediaCursor{at:[Option<MessageCursor>;2],done:[bool;2],pending:[Vec<SearchResult>;2]}
impl MediaCursor{pub fn has_more(&self)->bool{self.pending.iter().any(|p|!p.is_empty())||self.done.iter().any(|done|!*done)}}
#[derive(Debug)]pub struct MediaPage{pub messages:Vec<Message>,pub cursor:MediaCursor}
impl ApiClient{
    pub async fn media_history(&self,channel:Uuid,mut cursor:MediaCursor)->Result<MediaPage>{
        {
            let fetch=|source:usize|{let cursor=&cursor;async move{
                if cursor.done[source]||!cursor.pending[source].is_empty(){return Ok(None);}
                let request=SearchRequest{channel_id:channel.to_string(),order:"desc".into(),contains_attachment:Some(source==0),has:if source==1{"link".into()}else{String::new()},..Default::default()};
                let page=self.search(&request,cursor.at[source]).await?;
                anyhow::ensure!(page.results.len()<=100&&page.results.iter().all(|m|m.channel_id==channel&&cursor.at[source].is_none_or(|c|(m.created_at,m.id)<(c.created_at,c.id))),"Invalid media search page");
                anyhow::ensure!(!page.has_more||!page.results.is_empty(),"Media search made no progress");Ok::<_,anyhow::Error>(Some(page))
            }};
            let (attachments,links)=tokio::try_join!(fetch(0),fetch(1))?;
            for(source,page)in [attachments,links].into_iter().enumerate(){if let Some(page)=page{cursor.done[source]=!page.has_more;cursor.at[source]=page.results.iter().min_by_key(|m|(m.created_at,m.id)).map(|m|MessageCursor{created_at:m.created_at,id:m.id});cursor.pending[source]=page.results;cursor.pending[source].sort_by_key(|m|(m.created_at,m.id));cursor.pending[source].dedup_by_key(|m|m.id);}}
        }
        let mut targets=Vec::new();for _ in 0..8{let Some(source)=(0..2).filter(|i|!cursor.pending[*i].is_empty()).max_by_key(|i|cursor.pending[*i].last().map(|m|(m.created_at,m.id)))else{break;};targets.push(cursor.pending[source].pop().unwrap());if cursor.pending[source].is_empty()&&!cursor.done[source]{break;}}
        let mut messages:Vec<_>=stream::iter(targets).map(|target|async move{
            // EN-US: An inclusive one-message lookup resolves only this media-bearing search hit.
            // PT-BR: Uma consulta inclusiva de uma mensagem resolve apenas este resultado com mídia.
            let at=if let Some(id)=target.id.as_u128().checked_add(1){MessageCursor{created_at:target.created_at,id:Uuid::from_u128(id)}}else{MessageCursor{created_at:target.created_at.checked_add_signed(chrono::Duration::microseconds(1)).context("Media cursor overflow")?,id:Uuid::nil()}};
            let page=self.message_page(channel,Some(at),1).await?;
            anyhow::ensure!(page.messages.len()<=1&&page.messages.iter().all(|m|m.channel_id==channel&&m.attachments.iter().flatten().count()+m.embeds.iter().flatten().count()<=64),"Invalid media lookup");
            Ok::<_,anyhow::Error>(page.messages.into_iter().find(|m|m.id==target.id))
        }).buffer_unordered(4).try_collect::<Vec<_>>().await?.into_iter().flatten().collect();
        messages.sort_by_key(|m|std::cmp::Reverse((m.created_at,m.id)));Ok(MediaPage{messages,cursor})
    }
}

#[cfg(test)]mod tests{
    use super::*;
    use crate::ui::chat::actions::tests::mock;
    #[tokio::test]async fn merges_filtered_pages_with_tied_timestamps_and_only_exact_lookups(){
        let(api,backend,server)=mock().await;let channel=Uuid::new_v4();let at=chrono::Utc::now();
        let messages:Vec<_>=(1..=36u128).map(|i|serde_json::json!({"id":Uuid::from_u128(i),"channel_id":channel,"created_at":at,"content":if i%3==1{"https://example.test/image"}else{"ordinary text"},"attachments":if i%3==0{vec![serde_json::json!({"id":Uuid::new_v4(),"mime_type":"image/png","original_file_name":"photo.png","size_bytes":1,"created_at":at})]}else{vec![]}})).collect();
        {let mut state=backend.lock().unwrap();state.media_search=true;state.paged_messages=messages;}
        let mut cursor=MediaCursor::default();let mut found=Vec::new();for _ in 0..8{let page=api.media_history(channel,cursor).await.unwrap();assert!(page.messages.len()<=8);found.extend(page.messages.iter().map(|m|m.id));cursor=page.cursor;if !cursor.has_more(){break;}}
        let expected:Vec<_>=(1..=36u128).rev().filter(|i|i%3!=2).map(Uuid::from_u128).collect();assert_eq!(found,expected);
        let state=backend.lock().unwrap();let search:Vec<_>=state.requests.iter().filter(|(_,p,_)|p.starts_with("/search")).collect();assert!(search.iter().all(|(_,_,b)|b["channel_id"]==channel.to_string()));assert!(search.iter().any(|(_,_,b)|b["has"]=="link"&&b["contains_attachment"]==false));
        let history:Vec<_>=state.requests.iter().filter(|(m,p,_)|m=="GET"&&p.contains("/messages")).collect();assert_eq!(history.len(),24);assert!(history.iter().all(|(_,p,_)|reqwest::Url::parse(&format!("http://local{p}")).unwrap().query_pairs().any(|(k,v)|k=="limit"&&v=="1")));drop(state);server.abort();
    }
    #[tokio::test]async fn rejects_a_cross_channel_search_before_resolving_any_history(){
        let(api,backend,server)=mock().await;let channel=Uuid::new_v4();backend.lock().unwrap().search_results=Some(vec![serde_json::json!({"type":"message","id":Uuid::new_v4(),"channel_id":Uuid::new_v4(),"channel_name":"Other","created_at":chrono::Utc::now()})]);assert!(api.media_history(channel,MediaCursor::default()).await.is_err());assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p.contains("/messages")));server.abort();
    }
}
