//! Mapped regressions for delayed allocation, channel loading and media bursts.
use super::*;
use actions::tests::{descendants, pump};
use adw::prelude::*;

pub(crate) fn exercise_cache(context:&gtk::glib::MainContext){super::transfers::exercise_cache_scroll(context);}
pub(crate) fn exercise_media_scroll(context:&gtk::glib::MainContext){super::viewport::exercise_media_scroll(context);}

pub(crate) fn exercise_smooth_updates(context:&gtk::glib::MainContext){
    use std::{cell::RefCell,rc::Rc};
    let created=chrono::Utc::now();let channel=Uuid::new_v4();let user=Uuid::new_v4();let image=Uuid::new_v4();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"live layout","created_at":created})).unwrap();
    let mut messages:Vec<Message>=(0..110).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Message {i}"),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect();
    messages[15].attachments=Some(vec![serde_json::from_value(serde_json::json!({"id":image,"mime_type":"image/png","original_file_name":"later.png","size_bytes":128,"created_at":created})).unwrap()]);
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(640).content(chat.widget()).build();window.present();settle(context);
    let widgets=descendants(chat.widget().upcast_ref());let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();let list=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|w|w.has_css_class("papo-chat-history"))).unwrap().clone();let adj=scroll.vadjustment();
    let reading=chat.model().rendered[&format!("message-{}",messages[70].id)].1.clone();adj.set_value(f64::from(reading.compute_bounds(&list).unwrap().y())-120.0);settle(context);assert!(!chat.model().viewport.following());
    let offset=f64::from(reading.compute_bounds(&list).unwrap().y())-adj.value();let samples=Rc::new(RefCell::new(Vec::new()));let seen=samples.clone();let weak=scroll.downgrade();let row=reading.downgrade();let rows=list.downgrade();let clock=scroll.frame_clock().unwrap();
    let observer=clock.connect_after_paint(move |_|{if let(Some(scroll),Some(row),Some(list))=(weak.upgrade(),row.upgrade(),rows.upgrade()){if let Some(bounds)=row.compute_bounds(&list){seen.borrow_mut().push(f64::from(bounds.y())-scroll.vadjustment().value());}}});
    let epoch=chat.model().transfers.epoch;let token=Uuid::new_v4();chat.state().get_mut().model.transfers.image_tokens.insert(image,token);
    chat.emit(ChatMsg::Transfer(transfers::TransferMsg::ImageReady{epoch,message:messages[15].id,key:image,token,result:Some(crate::media::PreparedImage{width:640,height:160,pixels:vec![255;640*160*4]})}));settle(context);
    assert!(!samples.borrow().is_empty());for value in samples.borrow().iter(){assert!((value-offset).abs()<2.0,"image reflow must preserve the reading position in every painted frame: {value} vs {offset}");}
    clock.disconnect(observer);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

pub(crate) fn exercise_opening(context:&gtk::glib::MainContext){
    use actions::tests::until;
    use std::{cell::RefCell,rc::Rc};
    let created=chrono::Utc::now();let channel=Uuid::new_v4();let user=Uuid::new_v4();
    let mut target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"opening layout","created_at":created})).unwrap();
    let messages:Vec<Message>=(0..70).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Message {i}: {}", "Wrapped text with several words and accents áéí. ".repeat(6)),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target.clone()),messages:messages.clone(),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(750).default_height(620).content(chat.widget()).build();window.present();
    let widgets=descendants(chat.widget().upcast_ref());let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();let list=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|w|w.has_css_class("papo-chat-history"))).unwrap().clone();
    assert_eq!(scroll.opacity(),0.0,"initial history must be measured before it is painted");
    until(context,||scroll.is_mapped());
    let expected=Rc::new(std::cell::Cell::new(None::<Uuid>));let samples=Rc::new(RefCell::new(Vec::new()));
    let weak=scroll.downgrade();let rows=list.downgrade();let destination=expected.clone();let seen=samples.clone();
    let clock=scroll.frame_clock().unwrap();let observer=clock.connect_after_paint(move |_|{
        let (Some(scroll),Some(list))=(weak.upgrade(),rows.upgrade())else{return;};
        if scroll.opacity()<1.0{return;}
        let adj=scroll.vadjustment();let max=(adj.upper()-adj.page_size()).max(0.0);
        let wanted=destination.get().map_or(max,|id|{
            let mut row=list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){return w.compute_bounds(&list).map_or(f64::NAN,|r|(f64::from(r.y())-24.0).clamp(0.0,max));}row=w.next_sibling();}f64::NAN
        });seen.borrow_mut().push((adj.value(),wanted));
    });
    let assert_painted=||{let frames=samples.borrow();assert!(!frames.is_empty(),"history must become visible without waiting for media");for (value,wanted) in frames.iter(){assert!((value-wanted).abs()<2.0,"a painted opening frame is misplaced: {value} vs {wanted}");}};
    settle(context);assert_eq!(scroll.opacity(),1.0);assert_painted();
    // Switch away from an old reading position to a narrower, wrapped history.
    scroll.vadjustment().set_value(250.0);settle(context);samples.borrow_mut().clear();
    chat.emit(ChatMsg::SetChannel(target.clone()));pump(context);assert_eq!(scroll.opacity(),0.0);
    window.set_default_size(520,620);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});
    // Updates arriving every frame must not restart opening indefinitely.
    let updates=Rc::new(std::cell::Cell::new(0));let count=updates.clone();let input=chat.sender().clone();let last=messages[69].id;
    let first_visible=Rc::new(std::cell::Cell::new(None));let first=first_visible.clone();let count_at_paint=updates.clone();let weak=scroll.downgrade();
    let presentation=clock.connect_after_paint(move |_|{if first.get().is_none()&&weak.upgrade().is_some_and(|s|s.opacity()==1.0){first.set(Some(count_at_paint.get()));}});
    let burst=list.add_tick_callback(move |_,_|{let n=count.get()+1;count.set(n);let _=input.send(ChatMsg::ApplyChange(state::Change::Reaction(last,crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:n})));if n<64{gtk::glib::ControlFlow::Continue}else{gtk::glib::ControlFlow::Break}});
    // Main-loop pumping can process later updates before returning to the test.
    // Measure the first painted presentation rather than its later observation.
    until(context,||first_visible.get().is_some());let first=first_visible.get().unwrap();assert!(first<24,"ongoing completions must not hold the new channel concealed: first visible at update {first}");if updates.get()<64{burst.remove();}clock.disconnect(presentation);settle(context);assert_painted();
    // The unread boundary is outside the first page: do not paint that
    // intermediate destination while its older page is still in flight.
    samples.borrow_mut().clear();target.last_read_message=Some(messages[19].id);target.last_message=Some(serde_json::from_value(serde_json::json!({"id":messages[69].id,"created_at":messages[69].created_at})).unwrap());expected.set(Some(messages[20].id));
    chat.emit(ChatMsg::SetChannel(target.clone()));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages[35..].to_vec(),has_more:true})});settle(context);
    assert_eq!(scroll.opacity(),0.0);assert!(samples.borrow().is_empty());assert!(chat.model().older_requested);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:Some(MessageCursor::from(&messages[35]))});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:channel,messages:messages[..35].to_vec(),has_more:false})});settle(context);assert_painted();assert!(chat.model().viewing_old);
    // Rapid switches invalidate the old reveal; a stale response cannot show
    // that channel, and an empty final channel still becomes visible.
    samples.borrow_mut().clear();expected.set(None);chat.emit(ChatMsg::SetChannel(target.clone()));let stale=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:stale,cursor:None});
    let mut other=target.clone();other.id=Uuid::new_v4();other.last_read_message=None;other.last_message=None;chat.emit(ChatMsg::SetChannel(other.clone()));chat.emit(ChatMsg::HistoryLoaded{request_id:stale,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});pump(context);assert_eq!(scroll.opacity(),0.0);assert!(chat.model().history.messages.is_empty());
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:other.id,messages:vec![],has_more:false})});settle(context);assert_eq!(scroll.opacity(),1.0);assert_eq!(list.first_child().unwrap().widget_name(),"empty-history");assert_painted();
    // Errors and explicit reader input must never leave the history concealed.
    chat.emit(ChatMsg::SetChannel(target.clone()));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Err("fixture failure".into())});settle(context);assert_eq!(scroll.opacity(),1.0);
    chat.emit(ChatMsg::SetChannel(target));pump(context);assert_eq!(scroll.opacity(),0.0);chat.emit(ChatMsg::CancelInitialRead);pump(context);assert_eq!(scroll.opacity(),1.0);
    clock.disconnect(observer);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

pub(crate) fn exercise_startup(context:&gtk::glib::MainContext){
    use crate::ui::user_list::{UserListInit,UserListModel,UserListMsg};
    let created=chrono::Utc::now();let channel=Uuid::new_v4();
    let users:Vec<UserSummary>=(0..512).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"username":format!("startup-{i:04}"),"created_at":created})).unwrap()).collect();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"startup","created_at":created})).unwrap();
    let messages:Vec<Message>=(0..200).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":users[i%users.len()].id,"content":format!("Startup message {i}"),"created_at":created+chrono::Duration::seconds(i as i64)})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),messages:messages.clone(),..Default::default()}).detach();
    let members=UserListModel::builder().launch(UserListInit{users:users.clone()}).detach();chat.emit(ChatMsg::SetAccess{user_id:users[0].id,access:crate::models::Access::direct()});chat.emit(ChatMsg::SetUsers(users.clone()));
    let content=gtk::Box::new(gtk::Orientation::Horizontal,0);content.append(chat.widget());content.append(members.widget());
    let window=adw::Window::builder().default_width(1000).default_height(700).content(&content).build();window.present();settle(context);
    let rows=chat.model().rendered.clone();let renders=chat.model().render_passes;
    let member_rows:HashMap<_,_>=descendants(members.widget().upcast_ref()).into_iter().filter(|w|w.widget_name().starts_with("member-")).map(|w|(w.widget_name().to_string(),w)).collect();
    FORMATTED_ROWS.with(|n|n.set(0));let start=std::time::Instant::now();let mut textures=HashMap::new();
    for chunk in users.chunks(16){
        for user in chunk{textures.insert(user.id,crate::media::PreparedImage{width:8,height:8,pixels:vec![255;256]}.texture());}
        chat.emit(ChatMsg::SetAvatars(textures.clone()));members.emit(UserListMsg::SetAvatars(textures.clone()));pump(context);
    }
    let elapsed=start.elapsed();assert_eq!(FORMATTED_ROWS.with(|n|n.get()),0,"avatar batches must not format message rows");assert_eq!(chat.model().render_passes,renders,"avatar-only updates must not run the history renderer");assert_eq!(chat.model().rendered,rows);
    let after:HashMap<_,_>=descendants(members.widget().upcast_ref()).into_iter().filter(|w|w.widget_name().starts_with("member-")).map(|w|(w.widget_name().to_string(),w)).collect();assert_eq!(after,member_rows,"startup images must preserve member row identity");
    for root in [chat.widget(),members.widget()]{
        let avatars:Vec<_>=descendants(root.upcast_ref()).into_iter().filter_map(|w|w.downcast::<adw::Avatar>().ok()).collect();
        assert!(!avatars.is_empty());assert!(avatars.iter().all(|a|a.custom_image().is_some()),"visible images must still appear");
    }
    // Removal and repeated snapshots must retain rows too; newly created rows
    // read the current cache rather than relying on the earlier notifications.
    textures.remove(&users[0].id);chat.emit(ChatMsg::SetAvatars(textures.clone()));members.emit(UserListMsg::SetAvatars(textures.clone()));members.emit(UserListMsg::SetUsers(users.clone()));chat.emit(ChatMsg::Latest);pump(context);assert_eq!(chat.model().rendered,rows);
    for root in [chat.widget(),members.widget()]{let avatar=descendants(root.upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::Avatar>().ok().filter(|a|a.widget_name()==format!("avatar-{}",users[0].id))).unwrap();assert!(avatar.custom_image().is_none());}
    let mut extra=messages[0].clone();extra.id=Uuid::new_v4();extra.author_id=Some(users[250].id);extra.created_at=created+chrono::Duration::seconds(201);chat.emit(ChatMsg::AddMessage(extra.clone()));pump(context);let row=chat.model().rendered[&format!("message-{}",extra.id)].1.clone();assert!(descendants(row.upcast_ref()).into_iter().filter_map(|w|w.downcast::<adw::Avatar>().ok()).any(|a|a.custom_image().as_ref()==Some(textures[&users[250].id].upcast_ref())));
    eprintln!("startup avatar benchmark: members=512, message_rows=200, batches=32, avatar_dispatch_ms={}, message_formats=0, history_renders=0, replaced_member_rows=0",elapsed.as_millis());
    window.set_content(None::<&gtk::Widget>);window.close();
}

pub(crate) fn settle(context: &gtk::glib::MainContext) {
    for _ in 0..70 {
        pump(context);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    let channel = Uuid::new_v4();
    let user = Uuid::new_v4();
    let start = chrono::Utc::now();
    let target: Channel = serde_json::from_value(serde_json::json!({
        "id": channel, "name": "layout", "created_at": start
    })).unwrap();
    let chat = ChatModel::builder().launch(ChatInit::default()).detach();
    chat.emit(ChatMsg::SetAccess { user_id: user, access: crate::models::Access::resolve(user, Some(user), &[], &[], &[]) });
    chat.emit(ChatMsg::SetChannel(target.clone()));
    let window = adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();
    window.present();
    settle(context);
    let widgets = descendants(chat.widget().upcast_ref());
    let list = widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|l|l.has_css_class("papo-chat-history"))).unwrap().clone();
    let scroll = widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();
    assert!(list.first_child().is_none(), "channel loading must not flash the empty-chat prompt");
    let mut unrelated = target.clone(); unrelated.id = Uuid::new_v4();
    chat.emit(ChatMsg::UpdateChannel(unrelated));
    pump(context);
    assert_eq!(chat.model().active_channel.as_ref().unwrap().id(), channel);

    let messages: Vec<Message> = (0..70).map(|i|serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "channel_id": channel, "author_id": user,
        "created_at": start + chrono::Duration::seconds(i), "content": format!("Message {i}")
    })).unwrap()).collect();
    let request = Uuid::new_v4();
    chat.emit(ChatMsg::BeginHistory { request_id: request, cursor: None });
    chat.emit(ChatMsg::HistoryLoaded { request_id: request, append: false, result: Ok(MessageListResponse { channel_id: channel, messages: messages.clone(), has_more: false }) });
    settle(context);
    let mut sent = messages.last().unwrap().clone();
    sent.id = Uuid::new_v4(); sent.created_at += chrono::Duration::seconds(1);
    sent.content = Some("New outgoing message\nwith another line".into());
    let pending = { let mut state=chat.state().get_mut(); state.model.draft.text=sent.content.clone().unwrap(); state.model.draft.begin().unwrap() };
    chat.emit(ChatMsg::SendFinished { request_id: pending, channel_id: channel, result: Ok(sent.clone()) });
    settle(context);
    let adj = scroll.vadjustment();
    let row = chat.model().rendered[&format!("message-{}", sent.id)].1.clone();
    let assert_latest = || {
        let bounds = row.compute_bounds(&list).unwrap();
        assert!(f64::from(bounds.y() + bounds.height()) <= adj.value() + adj.page_size() + 2.0,
            "new outgoing row must be visible without manual scrolling");
        assert!((adj.upper()-adj.page_size()-adj.value()).abs()<2.0);
    };
    assert_latest();
    // Simulate height-for-width/media allocation finishing after the old
    // four-frame restoration window, without emitting any chat update.
    let first = chat.model().rendered[&format!("message-{}", messages[0].id)].1.clone();
    first.set_height_request(first.height()+220);
    settle(context);
    assert_latest();
    adj.set_value(240.0); settle(context);
    let reading = adj.value();
    row.set_height_request(row.height()+180); settle(context);
    assert!((adj.value()-reading).abs()<2.0, "late layout must not pull a reader to the bottom");
    assert!(!chat.model().viewport.following());

    chat.emit(ChatMsg::SetChannel(target)); settle(context);
    assert!(list.first_child().is_none());
    let request = Uuid::new_v4();
    chat.emit(ChatMsg::BeginHistory { request_id: request, cursor: None });
    chat.emit(ChatMsg::HistoryLoaded { request_id: request, append: false, result: Ok(MessageListResponse { channel_id: channel, messages: vec![], has_more: false }) });
    settle(context);
    let empty = list.first_child().unwrap();
    assert_eq!(empty.widget_name(), "empty-history");
    chat.emit(ChatMsg::Latest); settle(context);
    assert_eq!(list.first_child().as_ref(), Some(&empty), "unchanged empty state retains its widget");
    window.set_content(None::<&gtk::Widget>); window.close();
    super::transfers::exercise_performance(context);
    super::transfers::exercise_cache_scroll(context);
}

pub(crate) fn exercise_corrections(context:&gtk::glib::MainContext){
    use actions::tests::until;
    let channel=Uuid::new_v4();let user=Uuid::new_v4();let start=chrono::Utc::now();
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"bounded history","created_at":start})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::resolve(user,Some(user),&[],&[],&[])});
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();settle(context);
    let widgets=descendants(chat.widget().upcast_ref());let list=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|l|l.has_css_class("papo-chat-history"))).unwrap().clone();let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();
    let emojis:Vec<crate::models::Emoji>=(0..500).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":format!("e{i}"),"format":"PNG","created_at":start})).unwrap()).collect();
    {let mut state=chat.state().get_mut();state.model.actions.emoji_names=emojis.iter().map(|e|(e.name.clone(),e.id)).collect();state.model.actions.emojis=emojis;}
    for count in [100,1000,5000]{
        let mut messages:Vec<Message>=(0..count).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Message {i} :e0:"),"created_at":start+chrono::Duration::seconds(i)})).unwrap()).collect();
        let image=Uuid::new_v4();messages.last_mut().unwrap().attachments=Some(vec![serde_json::from_value(serde_json::json!({"id":image,"mime_type":"image/png","original_file_name":"photo.png","size_bytes":128,"created_at":start})).unwrap()]);
        let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});settle(context);
        assert!(chat.model().rendered.len()<=window::ROW_LIMIT+5);assert!(scroll.vadjustment().upper()-scroll.vadjustment().page_size()-scroll.vadjustment().value()<2.0);
        let stable=chat.model().rendered[&format!("message-{}",messages[count as usize-20].id)].1.clone();
        FORMATTED_ROWS.with(|n|n.set(0));let started=std::time::Instant::now();chat.emit(ChatMsg::ApplyChange(state::Change::Reaction(messages.last().unwrap().id,crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:1})));pump(context);
        let reaction_time=started.elapsed();let formatted=FORMATTED_ROWS.with(|n|n.get());assert!(formatted<=3,"a reaction must not format all {count} messages: {formatted}");assert_eq!(chat.model().rendered[&format!("message-{}",messages[count as usize-20].id)].1,stable);
        let token=Uuid::new_v4();let epoch=chat.model().transfers.epoch;chat.state().get_mut().model.transfers.image_tokens.insert(image,token);FORMATTED_ROWS.with(|n|n.set(0));
        chat.emit(ChatMsg::Transfer(transfers::TransferMsg::ImageReady{epoch,message:messages.last().unwrap().id,key:image,token,result:Some(crate::media::PreparedImage{width:2,height:2,pixels:vec![255;16]})}));until(context,||chat.model().transfers.textures.contains_key(&image)&&chat.model().media_render_pending.is_none());assert!(FORMATTED_ROWS.with(|n|n.get())<=3);
        let mut new=messages.last().unwrap().clone();new.id=Uuid::new_v4();new.created_at+=chrono::Duration::seconds(1);new.attachments=None;FORMATTED_ROWS.with(|n|n.set(0));chat.emit(ChatMsg::AddMessage(new));pump(context);assert!(FORMATTED_ROWS.with(|n|n.get())<=3);
        let memory=std::fs::read_to_string("/proc/self/status").ok().and_then(|s|s.lines().find(|l|l.starts_with("VmRSS:")).map(str::to_owned)).unwrap_or_default();
        eprintln!("chat benchmark: messages={count}, widgets={}, reaction_format_rows={formatted}, reaction_update_us={}, {memory}",chat.model().rendered.len(),reaction_time.as_micros());
        if count==5000{
            // Media-only viewport updates must stay independent of cached
            // history length. The already cached picture needs no HTTP fixture.
            chat.state().get_mut().model.actions.api=Some(crate::api::ApiClient::new("http://127.0.0.1:9").unwrap());chat.emit(ChatMsg::MediaViewport);settle(context);
            let metadata=chat.model().transfers.metadata_visits;let candidates=chat.model().transfers.candidate_visits;
            for _ in 0..100{chat.emit(ChatMsg::MediaViewport);}pump(context);
            assert_eq!(chat.model().transfers.metadata_visits,metadata,"unchanged scrolling must not renormalize full history");assert!(chat.model().transfers.candidate_visits-candidates<=100*window::ROW_LIMIT,"media discovery must remain bounded to materialized rows");
            chat.emit(ChatMsg::ApplyChange(Change::Reaction(messages[4999].id,crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:2})));settle(context);assert_eq!(chat.model().transfers.metadata_visits,metadata,"reactions do not invalidate embed metadata");
            let mut plain=messages[4999].clone();plain.id=Uuid::new_v4();plain.created_at+=chrono::Duration::seconds(2);plain.attachments=None;chat.emit(ChatMsg::AddMessage(plain));settle(context);assert_eq!(chat.model().transfers.metadata_visits,metadata,"plain live messages do not invalidate embed metadata");
            let embed:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","title":"Fresh metadata","created_at":start})).unwrap();
            chat.emit(ChatMsg::ApplyChange(Change::Embeds(messages[4999].id,vec![embed.clone()])));settle(context);assert!(chat.model().transfers.metadata_visits>metadata);let row=chat.model().rendered[&format!("message-{}",messages[4999].id)].1.clone();assert!(descendants(row.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::Label>().ok()).any(|l|l.text()=="Fresh metadata"),"metadata changes must still reach the rendered card");
            chat.state().get_mut().model.actions.api=None;
            chat.emit(ChatMsg::Action(ActionMsg::Navigate(messages[15].id)));settle(context);assert!(chat.model().rendered.contains_key(&format!("message-{}",messages[15].id)));assert!(chat.model().rendered.len()<=window::ROW_LIMIT+5);
            let anchor=chat.model().viewport.capture(&scroll,&list);let value=scroll.vadjustment().value();let request=Uuid::new_v4();chat.emit(ChatMsg::BeginSnapshot(request));
            let mut changed=messages.clone();changed[4999].content=Some("updated while offline".into());changed.remove(100);
            chat.state().get_mut().model.reconcile_request=Some(request);chat.emit(ChatMsg::Reconciled{request,result:Ok(MessageListResponse{channel_id:channel,messages:changed,has_more:false})});settle(context);
            assert!(chat.model().rendered.contains_key(&format!("message-{}",messages[15].id)));assert!(!chat.model().history.messages.iter().any(|m|m.id==messages[100].id));assert!((scroll.vadjustment().value()-value).abs()<2.0,"reconnect must preserve the older reading anchor");
            if let Position::Anchor{id:Some(id),..}=anchor{assert!(chat.model().rendered.contains_key(&format!("message-{id}")));}
            // Dragging the scrollbar into a spacer must materialize that region.
            scroll.vadjustment().set_value(scroll.vadjustment().upper()/2.0);settle(context);assert!(chat.model().render_window.range.start>1000&&chat.model().render_window.range.start<4000);assert!(chat.model().rendered.len()<=window::ROW_LIMIT+5);
            chat.emit(ChatMsg::Latest);settle(context);assert_eq!(chat.model().render_window.range.end,chat.model().history.messages.len());
        }
    }
    window.set_content(None::<&gtk::Widget>);window.close();
}
