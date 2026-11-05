//! EN-US: Repeatable local performance scenarios use synthetic content and isolated accounts.
//! PT-BR: Cenários locais reproduzíveis usam conteúdo sintético e contas isoladas.
use super::*;
use actions::tests::{descendants, mock, pump, until};
use adw::prelude::*;
use std::time::Instant;

pub(crate) fn measurement(name:&'static str, started:Instant, before:Option<u64>, extra:serde_json::Value){
    eprintln!("Papo baseline: {}",serde_json::json!({"scenario":name,"elapsed_us":started.elapsed().as_micros(),
        "resident_before_bytes":before,"resident_after_bytes":crate::performance::resident_bytes(),"details":extra}));
}

pub(crate) fn run(context:&gtk::glib::MainContext){
    channels(context);
    let start=Instant::now();let memory=crate::performance::resident_bytes();
    performance::exercise_scroll_frames(context);
    measurement("scrolling",start,memory,serde_json::json!({"messages":1000,"input":"synthetic wheel signals","duration_seconds":3}));
    let start=Instant::now();let memory=crate::performance::resident_bytes();
    performance::exercise_corrections(context);
    measurement("long_history",start,memory,serde_json::json!({"message_counts":[100,1000,5000],"scope":"bounded rows, reaction updates, navigation and reconciliation"}));
    let start=Instant::now();let memory=crate::performance::resident_bytes();
    super::transfers::exercise_residency_baseline(context);
    measurement("media_residency",start,memory,serde_json::json!({"channels":8,"previews_per_channel":30,"preview_size":[640,360],"compressed_bytes_per_preview":1048576}));
}

fn channels(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let a=Uuid::new_v4();let b=Uuid::new_v4();
    let at="2026-11-04T12:00:00Z";
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"Baseline","created_at":at})).unwrap();
    let mut png=std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(320,180).write_to(&mut png,image::ImageFormat::Png).unwrap();
    let keys:Vec<_>=(0..8).map(|_|Uuid::new_v4()).collect();
    let messages:Vec<serde_json::Value>=(0..30).map(|i|serde_json::json!({"id":Uuid::new_v4(),"channel_id":a,"author_id":user,
        "created_at":format!("2026-11-04T12:00:{i:02}Z"),"content":format!("Synthetic message {i}: **bold** and accents á é ç"),
        "attachments":if i>=22 {vec![serde_json::json!({"id":keys[i-22],"mime_type":"image/png","original_file_name":"fixture.png","moderation_status":"clean","size_bytes":png.get_ref().len(),"created_at":at})]}else{vec![]}})).collect();
    {let mut state=backend.lock().unwrap();state.channel_messages.insert(a.to_string(),messages);state.channel_messages.insert(b.to_string(),vec![]);
        for key in &keys{state.attachment_thumbnails.insert(format!("/attachments/{key}/thumbnail"),png.get_ref().clone());}}
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),user_id:Some(user),..Default::default()}).detach();
    let window=adw::Window::builder().default_width(1000).default_height(720).content(chat.widget()).build();window.present();
    let before=crate::performance::resident_bytes();let start=Instant::now();
    chat.emit(ChatMsg::SelectTarget{target:ConversationTarget::Channel(channel(a)),user_id:user,access:crate::models::Access::direct()});
    chat.emit(ChatMsg::RequestHistory{channel_id:a,cursor:None});
    until(context,||chat.model().history_ready&&!chat.model().viewport.covered());
    measurement("cold_channel_text",start,before,serde_json::json!({"messages":30}));
    until(context,||keys.iter().all(|key|chat.model().transfers.resident.contains(a,*key))&&chat.model().media_render_pending.is_none());
    measurement("cold_channel_media",start,before,serde_json::json!({"images":8,"size":[320,180]}));
    let media_calls=||backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path.starts_with("/attachments/")).count();
    let cold_requests=media_calls();assert_eq!(cold_requests,8);
    let counts=crate::performance::snapshot();
    for _ in 0..3{
        chat.emit(ChatMsg::SelectTarget{target:ConversationTarget::Channel(channel(b)),user_id:user,access:crate::models::Access::direct()});
        until(context,||chat.model().active_channel.as_ref().is_some_and(|c|c.id()==b));
        let start=Instant::now();let memory=crate::performance::resident_bytes();
        chat.emit(ChatMsg::SelectTarget{target:ConversationTarget::Channel(channel(a)),user_id:user,access:crate::models::Access::direct()});
        chat.emit(ChatMsg::RequestHistory{channel_id:a,cursor:None});
        until(context,||chat.model().history_ready&&!chat.model().viewport.covered()&&keys.iter().all(|key|chat.model().transfers.textures.contains_key(key)));
        measurement("warm_channel",start,memory,serde_json::json!({"additional_media_requests":media_calls()-cold_requests,"messages":30}));
    }
    assert_eq!(media_calls(),cold_requests,"warm returns must reuse the opening media batch");
    eprintln!("Papo baseline: {}",serde_json::json!({"scenario":"media_requests","details":{"cold":cold_requests,"warm":media_calls()-cold_requests,"metrics_at_cold":counts}}));
    let widgets=descendants(chat.widget().upcast_ref());assert!(widgets.iter().any(|w|w.is::<gtk::ListBox>()));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}
