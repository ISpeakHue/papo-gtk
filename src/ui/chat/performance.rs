//! Mapped regressions for delayed allocation, channel loading and media bursts.
use super::*;
use actions::tests::{descendants, pump};
use adw::prelude::*;

pub(crate) fn exercise_cache(context:&gtk::glib::MainContext){super::transfers::exercise_cache_scroll(context);}
pub(crate) fn exercise_media_scroll(context:&gtk::glib::MainContext){super::viewport::exercise_media_scroll(context);}

pub(crate) fn exercise_channel_cache(context:&gtk::glib::MainContext) {
    use actions::tests::until;
    use serde_json::json;
    use std::sync::{Arc,Mutex,atomic::{AtomicU16,Ordering}};
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    let created=chrono::Utc::now(); let a=Uuid::new_v4(); let b=Uuid::new_v4(); let user=Uuid::new_v4();
    let channel=|id,name|serde_json::from_value::<Channel>(json!({"id":id,"name":name,"created_at":created})).unwrap();
    let original=json!({"id":Uuid::new_v4(),"channel_id":a,"author_id":user,"content":"cached message","created_at":created});
    let removed=json!({"id":Uuid::new_v4(),"channel_id":a,"author_id":user,"content":"deleted while away","created_at":created+chrono::Duration::seconds(1)});
    let peer=json!({"id":Uuid::new_v4(),"channel_id":b,"author_id":user,"content":"other channel","created_at":created});
    let status=Arc::new(AtomicU16::new(200)); let requests=Arc::new(Mutex::new(Vec::<String>::new()));
    let mut edited=original.clone(); edited["content"]=json!("updated message"); edited["edited_at"]=json!(created+chrono::Duration::seconds(2));
    let fresh=json!({"channel_id":a,"messages":[edited],"has_more":false});
    let state=status.clone(); let counts=requests.clone();
    let (api,server)=tokio::runtime::Handle::current().block_on(async move {
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api=crate::api::ApiClient::new(&format!("http://{}",listener.local_addr().unwrap())).unwrap();
        let task=tokio::spawn(async move { loop {
            let Ok((mut stream,_))=listener.accept().await else{break;}; let fresh=fresh.clone(); let counts=counts.clone(); let state=state.clone();
            tokio::spawn(async move {
                let mut bytes=vec![]; loop { let mut buf=[0;2048]; let n=stream.read(&mut buf).await.unwrap(); if n==0{return;} bytes.extend_from_slice(&buf[..n]); if bytes.windows(4).any(|w|w==b"\r\n\r\n"){break;} }
                let request=String::from_utf8(bytes).unwrap(); let path=request.split_whitespace().nth(1).unwrap().to_owned(); counts.lock().unwrap().push(path.clone());
                let history=path.contains("/messages?"); let status=if history{state.load(Ordering::SeqCst)}else{200};
                if history { tokio::time::sleep(std::time::Duration::from_millis(700)).await; }
                let body=if status!=200{json!({"detail":"unavailable"})}else if history{fresh}else if path.starts_with("/emojis"){json!({"emojis":[],"has_more":false})}else{json!({"messages":[],"has_more":false})}.to_string();
                let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                let _=stream.write_all(header.as_bytes()).await; let _=stream.write_all(body.as_bytes()).await;
            });
        }}); (api,task)
    });
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a,"cached")),messages:vec![serde_json::from_value(original.clone()).unwrap(),serde_json::from_value(removed).unwrap()],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(640).content(chat.widget()).build(); window.present(); settle(context);
    chat.emit(ChatMsg::SetChannel(channel(b,"other"))); let request=Uuid::new_v4(); chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None}); chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:b,messages:vec![serde_json::from_value(peer).unwrap()],has_more:false})}); settle(context);
    chat.state().get_mut().model.actions.api=Some(api);
    chat.emit(ChatMsg::SetChannel(channel(a,"cached"))); chat.emit(ChatMsg::RequestHistory{channel_id:a,cursor:None}); pump(context);
    assert!(chat.model().history_ready); assert_eq!(chat.model().history.messages.len(),2,"cached history is available before the network responds");
    until(context,||!chat.model().viewport.covered());
    assert_eq!(chat.model().history.messages.len(),2,"opening must not wait for the slow refresh");
    until(context,||chat.model().history.messages.len()==1 && chat.model().history.messages[0].content.as_deref()==Some("updated message"));
    assert_eq!(requests.lock().unwrap().iter().filter(|p|p.contains("/messages?")).count(),1,"one-page cached history requires one refresh, no duplicate load");
    // Live deletion updates the inactive snapshot before it can be shown.
    chat.emit(ChatMsg::SetChannel(channel(b,"other"))); pump(context); assert!(chat.model().history_ready);
    let id=original["id"].as_str().unwrap().parse().unwrap(); let passes=chat.model().render_passes;
    chat.emit(ChatMsg::CacheChange(Change::Edit(id,"live cached edit".into(),Some(created+chrono::Duration::seconds(3))))); pump(context);
    assert_eq!(chat.model().render_passes,passes,"inactive live updates must not rebuild the active conversation");
    chat.emit(ChatMsg::SetChannel(channel(a,"cached"))); pump(context);
    assert!(chat.model().history_ready); assert_eq!(chat.model().history.messages[0].content.as_deref(),Some("live cached edit"));
    chat.emit(ChatMsg::SetChannel(channel(b,"other"))); pump(context);
    chat.emit(ChatMsg::ApplyChange(Change::Delete(original["id"].as_str().unwrap().parse().unwrap()))); pump(context);
    chat.emit(ChatMsg::SetChannel(channel(a,"cached"))); pump(context); assert!(chat.model().history_ready); assert!(chat.model().history.messages.is_empty(),"cached history must not flash a live-deleted message");
    chat.emit(ChatMsg::SetChannel(channel(b,"other"))); pump(context); assert!(chat.model().history_ready);
    chat.emit(ChatMsg::HistoryFailure{request_id:Uuid::new_v4(),error:crate::api::ApiError{status:reqwest::StatusCode::FORBIDDEN,code:None,message:"old channel".into()}.into()}); pump(context);
    assert!(chat.model().access.read,"a previous-channel failure must not revoke the newly selected chat");
    status.store(500,Ordering::SeqCst); chat.emit(ChatMsg::RequestHistory{channel_id:b,cursor:None}); pump(context);
    until(context,||chat.model().reconcile_request.is_none()); assert_eq!(chat.model().history.messages.len(),1,"a failed refresh keeps readable cached content");
    status.store(403,Ordering::SeqCst); chat.emit(ChatMsg::RequestHistory{channel_id:b,cursor:None});
    until(context,||!chat.model().access.read); assert!(chat.model().history.messages.is_empty(),"denied access immediately removes retained content");
    window.set_content(None::<&gtk::Widget>); window.close(); drop(chat); pump(context); server.abort();
}

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
    chat.emit(ChatMsg::Transfer(transfers::TransferMsg::ImageReady{epoch,key:image,token,result:Some(crate::media::PreparedImage{width:640,height:160,pixels:vec![255;640*160*4]})}));settle(context);
    assert!(!samples.borrow().is_empty());for value in samples.borrow().iter(){assert!((value-offset).abs()<2.0,"image reflow must preserve the reading position in every painted frame: {value} vs {offset}");}
    clock.disconnect(observer);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
}

pub(crate) fn exercise_opening(context:&gtk::glib::MainContext){
    use actions::tests::until;
    use std::{cell::RefCell,rc::Rc};
    let style=adw::StyleManager::default();let previous=style.color_scheme();style.set_color_scheme(adw::ColorScheme::ForceLight);
    let created=chrono::Utc::now();let channel=Uuid::new_v4();let user=Uuid::new_v4();
    let mut target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"opening layout","created_at":created})).unwrap();
    let messages:Vec<Message>=(0..70).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":user,"content":format!("Message {i}: {}", "Wrapped text with several words and accents áéí. ".repeat(6)),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target.clone()),messages:messages.clone(),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(750).default_height(620).content(chat.widget()).build();window.present();
    let widgets=descendants(chat.widget().upcast_ref());let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();let list=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|w|w.has_css_class("papo-chat-history"))).unwrap().clone();
    assert!(chat.model().viewport.covered(),"initial history must be measured before it is painted");
    assert_eq!(scroll.opacity(),1.0,"opening must not make the chat surface transparent");
    until(context,||scroll.is_mapped());
    let expected=Rc::new(std::cell::Cell::new(None::<Uuid>));let samples=Rc::new(RefCell::new(Vec::new()));
    let cover=descendants(chat.widget().upcast_ref()).into_iter().find(|w|w.widget_name()=="history-cover").unwrap();
    let concealed=cover.downgrade();
    let weak=scroll.downgrade();let rows=list.downgrade();let destination=expected.clone();let seen=samples.clone();
    let covered_frames=Rc::new(std::cell::Cell::new(0usize)); let opaque=covered_frames.clone();
    let cover_failure=Rc::new(RefCell::new(None));let failure=cover_failure.clone();
    let surface=scroll.parent().unwrap(); let renderer=window.renderer().unwrap();
    let clock=scroll.frame_clock().unwrap();let observer=clock.connect_after_paint(move |_|{
        let (Some(scroll),Some(list))=(weak.upgrade(),rows.upgrade())else{return;};
        if concealed.upgrade().is_some_and(|w|w.is_visible()){
            // Inspect actual rendered pixels, not just a visibility property:
            // the loading surface must completely cover old text and images.
            let snapshot=gtk::Snapshot::new(); surface.parent().unwrap().snapshot_child(&surface,&snapshot);
            let node=snapshot.to_node().expect("opening produces an opaque background");
            let viewport=surface.compute_bounds(&surface.parent().unwrap()).unwrap();
            let texture=renderer.render_texture(&node,Some(&viewport));
            let mut pixels=vec![0;texture.width() as usize*texture.height() as usize*4]; texture.download(&mut pixels,texture.width() as usize*4);
            if (pixels[3]!=255||!pixels.chunks_exact(4).all(|pixel|pixel==&pixels[..4]))&&failure.borrow().is_none(){
                *failure.borrow_mut()=Some(format!("opening cover must be opaque and uniform; first pixel {:?}, viewport {:?}",&pixels[..4],viewport));
            }
            opaque.set(opaque.get()+1); return;
        }
        let adj=scroll.vadjustment();let max=(adj.upper()-adj.page_size()).max(0.0);
        let wanted=destination.get().map_or(max,|id|{
            let mut row=list.first_child();while let Some(w)=row{if w.widget_name()==format!("message-{id}"){return w.compute_bounds(&list).map_or(f64::NAN,|r|(f64::from(r.y())-24.0).clamp(0.0,max));}row=w.next_sibling();}f64::NAN
        });seen.borrow_mut().push((adj.value(),wanted));
    });
    let painted_check=std::cell::Cell::new(0);
    let assert_painted=||{painted_check.set(painted_check.get()+1);assert!(cover_failure.borrow().is_none(),"{:?}",cover_failure.borrow());let frames=samples.borrow();assert!(!frames.is_empty(),"history must become visible without waiting for media");for (value,wanted) in frames.iter(){assert!((value-wanted).abs()<2.0,"opening check {} is misplaced: {value} vs {wanted}",painted_check.get());}};
    settle(context);assert!(!chat.model().viewport.covered());assert_painted();
    // Switch away from an old reading position to a narrower, wrapped history.
    style.set_color_scheme(adw::ColorScheme::ForceDark);settle(context);
    scroll.vadjustment().set_value(250.0);settle(context);samples.borrow_mut().clear();
    let old_input=chat.model().viewport.input_epoch(); let covered_before=covered_frames.get();
    chat.emit(ChatMsg::SetChannel(target.clone()));chat.emit(ChatMsg::CancelInitialRead(old_input));pump(context);assert!(chat.model().viewport.covered(),"queued input from the old channel cannot reveal the new one");
    until(context,||covered_frames.get()>covered_before);
    crate::ui::chat::actions::tests::resize_window(&window,520,620);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});
    // Updates arriving every frame must not restart opening indefinitely.
    let updates=Rc::new(std::cell::Cell::new(0));let count=updates.clone();let input=chat.sender().clone();let last=messages[69].id;
    let first_visible=Rc::new(std::cell::Cell::new(None));let first=first_visible.clone();let count_at_paint=updates.clone();let concealed=cover.downgrade();
    let presentation=clock.connect_after_paint(move |_|{if first.get().is_none()&&concealed.upgrade().is_some_and(|w|!w.is_visible()){first.set(Some(count_at_paint.get()));}});
    let burst=list.add_tick_callback(move |_,_|{let n=count.get()+1;count.set(n);let _=input.send(ChatMsg::ApplyChange(state::Change::Reaction(last,crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:n})));if n<64{gtk::glib::ControlFlow::Continue}else{gtk::glib::ControlFlow::Break}});
    // Main-loop pumping can process later updates before returning to the test.
    // Measure the first painted presentation rather than its later observation.
    until(context,||first_visible.get().is_some());let first=first_visible.get().unwrap();assert!(first<24,"ongoing completions must not hold the new channel concealed: first visible at update {first}");if updates.get()<64{burst.remove();}clock.disconnect(presentation);settle(context);assert_painted();
    // Simulate a media allocation completing in the final update phase,
    // after the opening tick but before GTK allocates and paints that frame.
    samples.borrow_mut().clear();target.last_read_message=Some(messages[19].id);target.last_message=Some(serde_json::from_value(serde_json::json!({"id":messages[69].id,"created_at":messages[69].created_at})).unwrap());expected.set(Some(messages[20].id));
    chat.emit(ChatMsg::SetChannel(target.clone()));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});pump(context);
    let preceding=chat.model().rendered[&format!("message-{}",messages[10].id)].1.clone();let changed=Rc::new(std::cell::Cell::new(false));let applied=changed.clone();let reveal=chat.model().viewport.reveal_pending();let gated=cover.downgrade();
    let allocation=clock.connect_local("update",true,move |_|{if !applied.get()&&(reveal()||gated.upgrade().is_some_and(|w|!w.is_visible())){preceding.set_height_request(600);applied.set(true);}None});
    settle(context);clock.disconnect(allocation);assert!(changed.get());assert_painted();
    // The unread boundary is outside the first page: do not paint that
    // intermediate destination while its older page is still in flight.
    samples.borrow_mut().clear();target.last_read_message=Some(messages[19].id);target.last_message=Some(serde_json::from_value(serde_json::json!({"id":messages[69].id,"created_at":messages[69].created_at})).unwrap());expected.set(Some(messages[20].id));
    chat.emit(ChatMsg::SetChannel(target.clone()));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages[35..].to_vec(),has_more:true})});settle(context);
    assert!(chat.model().viewport.covered());assert!(samples.borrow().is_empty());assert!(chat.model().older_requested);
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:Some(MessageCursor::from(&messages[35]))});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:true,result:Ok(MessageListResponse{channel_id:channel,messages:messages[..35].to_vec(),has_more:false})});settle(context);assert_painted();assert!(chat.model().viewing_old);
    // Rapid switches invalidate the old reveal; a stale response cannot show
    // that channel, and an empty final channel still becomes visible.
    samples.borrow_mut().clear();expected.set(None);chat.emit(ChatMsg::SetChannel(target.clone()));let stale=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:stale,cursor:None});
    let mut other=target.clone();other.id=Uuid::new_v4();other.last_read_message=None;other.last_message=None;chat.emit(ChatMsg::SetChannel(other.clone()));chat.emit(ChatMsg::HistoryLoaded{request_id:stale,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:messages.clone(),has_more:false})});pump(context);assert!(chat.model().viewport.covered());assert!(chat.model().history.messages.is_empty());
    let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:other.id,messages:vec![],has_more:false})});settle(context);assert!(!chat.model().viewport.covered());assert_eq!(list.first_child().unwrap().widget_name(),"empty-history");assert_painted();
    // Errors and explicit reader input must never leave the history concealed.
    chat.emit(ChatMsg::SetChannel(target.clone()));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Err("fixture failure".into())});settle(context);assert!(!chat.model().viewport.covered());
    chat.emit(ChatMsg::SetChannel(target));pump(context);assert!(chat.model().viewport.covered());chat.emit(ChatMsg::CancelInitialRead(chat.model().viewport.input_epoch()));pump(context);assert!(!chat.model().viewport.covered());
    clock.disconnect(observer);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);style.set_color_scheme(previous);pump(context);
    exercise_displayed_channels(context);
    exercise_displayed_position(context);
}

fn exercise_displayed_channels(context:&gtk::glib::MainContext){
    use actions::tests::until;
    use std::{cell::{Cell,RefCell},rc::Rc};
    let created=chrono::Utc::now();let user=Uuid::new_v4();let a=Uuid::new_v4();let b=Uuid::new_v4();
    let channel=|id,name|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":name,"created_at":created})).unwrap();
    let messages=|id| (0..50).map(|i|serde_json::from_value::<Message>(serde_json::json!({"id":Uuid::new_v4(),"channel_id":id,"author_id":user,"content":format!("{id}: message {i}"),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect::<Vec<_>>();
    let first=messages(a);let second=messages(b);
    let css=gtk::CssProvider::new();let rules=first.iter().map(|m|format!("#message-{}{{background:#ff0000;}}",m.id)).chain(second.iter().map(|m|format!("#message-{}{{background:#00ff00;}}",m.id))).collect::<String>();css.load_from_string(&rules);
    gtk::style_context_add_provider_for_display(&gtk::gdk::Display::default().unwrap(),&css,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION+1);
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a,"first")),messages:first.clone(),user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(780).default_height(620).content(chat.widget()).build();window.present();until(context,||window.is_mapped());
    let Some(display)=super::display_frames::DisplayFrames::new(&window)else{window.set_content(None::<&gtk::Widget>);window.close();drop(chat);gtk::style_context_remove_provider_for_display(&gtk::gdk::Display::default().unwrap(),&css);return;};let display=Rc::new(display);
    let scroll=descendants(chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok()).unwrap();settle(context);
    assert!(display.contains(&window,scroll.upcast_ref(),0xff0000),"pixel regression must observe the original channel on the display");
    let forbidden=Rc::new(Cell::new(0xff0000));let wrong=Rc::new(RefCell::new(None));let frames=Rc::new(Cell::new(0));let advanced=Rc::new(Cell::new(false));
    let capture=display.clone();let previous=forbidden.clone();let failure=wrong.clone();let painted=frames.clone();let surface=window.clone();let area=scroll.clone();let clock=scroll.frame_clock().unwrap();let presented=advanced.clone();
    let observer=clock.connect_after_paint(move |_|{painted.set(painted.get()+1);let stale=capture.contains(&surface,area.upcast_ref(),previous.get());if !stale{presented.set(true);}else if presented.get(){*failure.borrow_mut()=Some(previous.get());}});
    // Observe covered frames, the first reveal, and subsequent media/layout
    // frames. Alternate cached and uncached selections repeatedly.
    for (target,history,old,new) in [(channel(b,"second"),second.clone(),0xff0000,0x00ff00),(channel(a,"first"),first.clone(),0x00ff00,0xff0000),(channel(b,"second"),second.clone(),0xff0000,0x00ff00)]{
        // GPU presentation can complete after after-paint; the previous final
        // buffer may remain until the cover/new channel reaches the screen.
        // Once that happens, the old channel must never appear again.
        forbidden.set(old);advanced.set(false);let before=frames.get();let viewport=scroll.child().unwrap();let passes=chat.model().render_passes;
        chat.emit(ChatMsg::SelectTarget{target:ConversationTarget::Channel(target.clone()),user_id:user,access:crate::models::Access{send:new==0xff0000,..crate::models::Access::direct()}});pump(context);
        assert_eq!(chat.model().render_passes,passes+1,"selection and permissions must render together without redrawing the old chat");
        assert_ne!(scroll.child().unwrap(),viewport,"channel switches must discard the previous viewport render node");
        until(context,||frames.get()>before);
        let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:target.id,messages:history,has_more:false})});settle(context);
        assert!(wrong.borrow().is_none(),"a displayed frame still contains the previous channel: {:?}",wrong.borrow());
        until(context,||display.contains(&window,scroll.upcast_ref(),new));
        assert!(wrong.borrow().is_none(),"the old channel cannot reappear while GPU presentation finishes");
    }
    clock.disconnect(observer);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
    gtk::style_context_remove_provider_for_display(&gtk::gdk::Display::default().unwrap(),&css);
}

// Distinguish the upper and lower history: a correct adjustment alone does
// not prove that GtkViewport painted the matching scroll transform.
fn exercise_displayed_position(context:&gtk::glib::MainContext){
    use actions::tests::until;
    use std::{cell::{Cell,RefCell},rc::Rc};
    let created=chrono::Utc::now();let user=Uuid::new_v4();let id=Uuid::new_v4();
    let mut target:Channel=serde_json::from_value(serde_json::json!({"id":id,"name":"position pixels","created_at":created})).unwrap();
    let messages:Vec<Message>=(0..70).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":id,"author_id":user,"content":format!("Message {i}"),"created_at":created+chrono::Duration::seconds(i)})).unwrap()).collect();
    let css=gtk::CssProvider::new();css.load_from_string(&messages.iter().enumerate().map(|(i,m)|format!("#message-{}{{background:{};}}",m.id,if i<11{"#ff0000"}else{"#00ff00"})).collect::<String>());
    gtk::style_context_add_provider_for_display(&gtk::gdk::Display::default().unwrap(),&css,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION+1);
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(target.clone()),messages:messages.clone(),user_id:Some(user),..Default::default()}).detach();chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(780).default_height(620).content(chat.widget()).build();window.present();until(context,||window.is_mapped());
    let widgets=descendants(chat.widget().upcast_ref());let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap().clone();let list=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ListBox>().filter(|w|w.has_css_class("papo-chat-history"))).unwrap().clone();
    let display=super::display_frames::DisplayFrames::new(&window);settle(context);
    let wrong=Rc::new(RefCell::new(Vec::new()));let errors=wrong.clone();let surface=window.clone();let area=scroll.clone();let rows=list.clone();
    let cover=widgets.iter().find(|w|w.widget_name()=="history-cover").unwrap().clone();let gate=cover.clone();
    let clock=scroll.frame_clock().unwrap();let observer=clock.connect_after_paint(move |_|{
        if gate.is_visible(){return;}
        // Bounds include the actual viewport transform, not just the requested
        // Adjustment value. Sampling must not force a fresh GTK snapshot.
        let origin=rows.compute_bounds(&area.child().unwrap()).unwrap().y();let requested=area.vadjustment().value();
        if (f64::from(origin)+requested).abs()>2.0{errors.borrow_mut().push(format!("stale transform: origin={origin}, adjustment={requested}"));}
        if display.as_ref().is_some_and(|d|d.contains(&surface,area.upcast_ref(),0xff0000)){errors.borrow_mut().push("upper history appeared on the display".into());}
    });
    for unread in [false,true]{
        target.last_read_message=Some(messages[if unread{19}else{69}].id);target.last_message=Some(serde_json::from_value(serde_json::json!({"id":messages[69].id,"created_at":messages[69].created_at})).unwrap());
        chat.emit(ChatMsg::SetChannel(target.clone()));let request=Uuid::new_v4();chat.emit(ChatMsg::BeginHistory{request_id:request,cursor:None});chat.emit(ChatMsg::HistoryLoaded{request_id:request,append:false,result:Ok(MessageListResponse{channel_id:id,messages:messages.clone(),has_more:false})});pump(context);
        let preceding=chat.model().rendered[&format!("message-{}",messages[10].id)].1.clone();let applied=Rc::new(Cell::new(false));let changed=applied.clone();let reveal=chat.model().viewport.reveal_pending();let gate=cover.clone();
        let allocation=clock.connect_local("update",true,move |_|{if !changed.get()&&(reveal()||!gate.is_visible()){preceding.set_height_request(2000);changed.set(true);}None});
        settle(context);clock.disconnect(allocation);assert!(applied.get());assert!(!chat.model().viewport.covered());assert!(wrong.borrow().is_empty(),"{:?}",wrong.borrow());
        if !unread{
            // A download can finish immediately after opening, too. Following
            // the bottom must update the transform before the next paint.
            chat.model().rendered[&format!("message-{}",messages[10].id)].1.set_height_request(4000);
            settle(context);assert!(wrong.borrow().is_empty(),"late visible allocation: {:?}",wrong.borrow());
        }
    }
    clock.disconnect(observer);window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);
    gtk::style_context_remove_provider_for_display(&gtk::gdk::Display::default().unwrap(),&css);
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
        chat.emit(ChatMsg::Transfer(transfers::TransferMsg::ImageReady{epoch,key:image,token,result:Some(crate::media::PreparedImage{width:2,height:2,pixels:vec![255;16]})}));until(context,||chat.model().transfers.textures.contains_key(&image)&&chat.model().media_render_pending.is_none());assert!(FORMATTED_ROWS.with(|n|n.get())<=3);
        let mut new=messages.last().unwrap().clone();new.id=Uuid::new_v4();new.created_at+=chrono::Duration::seconds(1);new.attachments=None;FORMATTED_ROWS.with(|n|n.set(0));chat.emit(ChatMsg::AddMessage(new));pump(context);assert!(FORMATTED_ROWS.with(|n|n.get())<=3);
        let memory=std::fs::read_to_string("/proc/self/status").ok().and_then(|s|s.lines().find(|l|l.starts_with("VmRSS:")).map(str::to_owned)).unwrap_or_default();
        eprintln!("chat benchmark: messages={count}, widgets={}, reaction_format_rows={formatted}, reaction_update_us={}, {memory}",chat.model().rendered.len(),reaction_time.as_micros());
        if count==5000{
            // Media-only viewport updates must stay independent of cached
            // history length. The already cached picture needs no HTTP fixture.
            chat.state().get_mut().model.actions.api=Some(crate::api::ApiClient::new("http://127.0.0.1:9").unwrap());chat.emit(ChatMsg::MediaViewport);settle(context);
            let metadata=chat.model().transfers.metadata_visits;let candidates=chat.model().transfers.candidate_visits;
            for _ in 0..100{chat.emit(ChatMsg::MediaViewport);}pump(context);
            assert_eq!(chat.model().transfers.metadata_visits,metadata,"unchanged scrolling must not renormalize full history");assert!(chat.model().transfers.candidate_visits-candidates<=100*(window::ROW_LIMIT+crate::models::INITIAL_MESSAGE_LIMIT),"media discovery stays bounded to mounted rows and the opening batch");
            chat.emit(ChatMsg::ApplyChange(Change::Reaction(messages[4999].id,crate::models::MessageReactionSummary{emoji_id:None,unicode:Some("❤️".into()),count:2})));settle(context);assert_eq!(chat.model().transfers.metadata_visits,metadata,"reactions do not invalidate embed metadata");
            let mut plain=messages[4999].clone();plain.id=Uuid::new_v4();plain.created_at+=chrono::Duration::seconds(2);plain.attachments=None;chat.emit(ChatMsg::AddMessage(plain));settle(context);assert!(chat.model().transfers.metadata_visits-metadata<=navigation::HISTORY_LIMIT,"trimming a live message visits only the bounded retained history");
            let metadata=chat.model().transfers.metadata_visits;
            let embed:Embed=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"source_type":"custom","fetch_method":"manual","title":"Fresh metadata","created_at":start})).unwrap();
            chat.emit(ChatMsg::ApplyChange(Change::Embeds(messages[4999].id,vec![embed.clone()])));settle(context);assert!(chat.model().transfers.metadata_visits>metadata);let row=chat.model().rendered[&format!("message-{}",messages[4999].id)].1.clone();assert!(descendants(row.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::Label>().ok()).any(|l|l.text()=="Fresh metadata"),"metadata changes must still reach the rendered card");
            chat.state().get_mut().model.actions.api=None;
            let older=chat.model().history.messages[15].id;chat.emit(ChatMsg::Action(ActionMsg::Navigate(older)));settle(context);assert!(chat.model().rendered.contains_key(&format!("message-{older}")));assert!(chat.model().rendered.len()<=window::ROW_LIMIT+5);
            let anchor=chat.model().viewport.capture(&scroll,&list);let request=Uuid::new_v4();chat.emit(ChatMsg::BeginSnapshot(request));
            let mut changed=messages.clone();changed[4999].content=Some("updated while offline".into());changed.remove(100);
            chat.state().get_mut().model.reconcile_request=Some(request);chat.emit(ChatMsg::Reconciled{request,result:Ok(MessageListResponse{channel_id:channel,messages:changed,has_more:false})});settle(context);
            assert!(chat.model().rendered.contains_key(&format!("message-{older}")));assert!(!chat.model().history.messages.iter().any(|m|m.id==messages[100].id));
            if let Position::Anchor{id:Some(id),offset,..}=anchor{let row=chat.model().rendered[&format!("message-{id}")].1.clone();let bounds=row.compute_bounds(&list).unwrap();assert!((f64::from(bounds.y())-scroll.vadjustment().value()-offset).abs()<2.0,"reconnect preserves the visible anchor when retained spacers change");}
            // Dragging the scrollbar into a spacer must materialize that region.
            scroll.vadjustment().set_value(scroll.vadjustment().upper()/2.0);settle(context);assert!(chat.model().history.messages.len()<=navigation::HISTORY_LIMIT);assert!(chat.model().render_window.range.start>0&&chat.model().render_window.range.start<chat.model().history.messages.len());assert!(chat.model().rendered.len()<=window::ROW_LIMIT+5);
            chat.emit(ChatMsg::Latest);settle(context);assert_eq!(chat.model().render_window.range.end,chat.model().history.messages.len());
        }
    }
    window.set_content(None::<&gtk::Widget>);window.close();
}
