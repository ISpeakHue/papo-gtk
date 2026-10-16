use super::*;
use actions::tests::{mock,pump,until};
use adw::prelude::*;

pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    budgets();
    let (api,backend,server) = tokio::runtime::Handle::current().block_on(mock());
    let channel = Uuid::new_v4(); let user = Uuid::new_v4(); let now = chrono::Utc::now();
    let keys: Vec<_> = (0..40).map(|_|Uuid::new_v4()).collect();
    let messages: Vec<Message> = keys.iter().enumerate().map(|(i,key)|serde_json::from_value(serde_json::json!({
        "id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Photo {i}"),"created_at":now+chrono::Duration::seconds(i as i64),
        "attachments":[{"id":key,"mime_type":if i==10{"image/gif"}else{"image/png"},"original_file_name":"image.png","size_bytes":128,"created_at":now}]
    })).unwrap()).collect();
    let mut output=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(64,64).write_to(&mut output,image::ImageFormat::Png).unwrap();let png=output.into_inner();
    { let mut state = backend.lock().unwrap(); for (index,key) in keys.iter().enumerate() { if index==10{state.attachment_thumbnails.insert(format!("/attachments/{key}"),crate::media::animation::fixture());}else{state.attachment_thumbnails.insert(format!("/attachments/{key}/thumbnail"),png.clone());} } }
    let target: Channel = serde_json::from_value(serde_json::json!({"id":channel,"name":"Recent previews","created_at":now})).unwrap();
    let chat = ChatModel::builder().launch(ChatInit { api:Some(api),active_channel:Some(target.clone()),messages:messages.clone(),user_id:Some(user),..Default::default() }).detach();
    chat.emit(ChatMsg::SetAccess { user_id:user,access:crate::models::Access::direct() });
    let window = adw::Window::builder().default_width(850).default_height(700).content(chat.widget()).build(); window.present();
    let start = std::time::Instant::now();
    loop {
        pump(context);
        if keys[10..].iter().all(|key|chat.model().transfers.resident.contains(channel,*key)) { break; }
        assert!(start.elapsed()<std::time::Duration::from_secs(10),"the complete opening media batch must prefetch without scrolling");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    performance::settle(context);
    assert!(chat.model().transfers.static_previews.contains(&keys[10])&&!chat.model().transfers.animations.contains_key(&keys[10]),"offscreen GIF prefetch prepares only its first frame");
    let latest = chat.model().transfers.textures[&keys[39]].downgrade();
    let pointer = chat.model().transfers.textures[&keys[39]].as_ptr();
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[20].id))); performance::settle(context);
    assert!(latest.upgrade().is_some(),"scrolling nineteen messages up retains the latest preview");
    chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[10].id)));
    until(context,||chat.model().transfers.animations.contains_key(&keys[10]));
    chat.emit(ChatMsg::Latest); performance::settle(context);
    assert_eq!(chat.model().transfers.textures[&keys[39]].as_ptr(),pointer);
    let mut other = target.clone(); other.id = Uuid::new_v4();
    chat.emit(ChatMsg::SetChannel(other)); pump(context);
    assert!(latest.upgrade().is_some(),"switching channels keeps recent previews resident");
    // Reopening must restore actual existing textures before any decoder runs.
    let held = tokio::runtime::Handle::current().block_on(chat.model().transfers.decode_limiter.clone().acquire_many_owned(2)).unwrap();
    chat.emit(ChatMsg::SetChannel(target));
    until(context,||chat.model().transfers.textures.contains_key(&keys[39]));
    assert_eq!(chat.model().transfers.textures[&keys[39]].as_ptr(),pointer);
    for (index,key) in keys.iter().enumerate().skip(10) {
        let endpoint=if index==10{format!("/attachments/{key}")}else{format!("/attachments/{key}/thumbnail")};
        assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path==&endpoint).count(),1,"each recent image is downloaded once");
    }
    drop(held);
    chat.emit(ChatMsg::DeleteMessage(messages[39].id)); pump(context);
    assert!(!chat.model().transfers.resident.contains(channel,keys[39]));
    chat.emit(ChatMsg::SetAccess { user_id:user,access:Default::default() }); pump(context);
    assert!(keys.iter().all(|key|!chat.model().transfers.resident.contains(channel,*key)&&!chat.model().transfers.encoded.contains(*key)));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}

fn budgets() {
    let mut cache = resident::Cache::default();
    let channel = Uuid::new_v4(); let message = Uuid::new_v4();
    let texture = crate::media::PreparedImage { width:512,height:512,pixels:vec![255;512*512*4] }.texture();
    let messages = HashSet::from([message]);
    cache.select(channel,&HashMap::new());
    let first = Uuid::new_v4(); cache.insert(channel,first,&messages,&texture);
    for _ in 0..128 { cache.insert(channel,Uuid::new_v4(),&messages,&texture); }
    assert!(!cache.contains(channel,first),"the resident byte budget evicts least-recent previews");
    let key = Uuid::new_v4();let second = Uuid::new_v4();
    cache.insert(channel,key,&HashSet::from([message,second]),&texture);
    cache.apply(&Change::Delete(message));assert!(cache.contains(channel,key),"a shared preview retains its other owner");
    cache.apply(&Change::Delete(second));assert!(!cache.contains(channel,key));
    cache.insert(channel,key,&messages,&texture);cache.apply(&Change::Moderation(message,key,"sensitive".into()));assert!(!cache.contains(channel,key));
    cache.insert(channel,key,&messages,&texture);cache.retain_channels(&[]);assert!(!cache.contains(channel,key));
    cache.select(channel,&HashMap::new());cache.insert(channel,key,&messages,&texture);
    for _ in 0..8 { cache.select(Uuid::new_v4(),&HashMap::new()); }
    assert!(!cache.contains(channel,key),"at most eight recently selected channels remain resident");
}
