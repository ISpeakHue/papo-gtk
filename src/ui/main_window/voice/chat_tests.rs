//! Voice chat reuses the real message UI without opening audio capture.
use super::*;
use crate::ui::chat::actions::tests::{mock,pump,until,descendants};
use serde_json::json;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let room:Uuid=Uuid::new_v4();let read_only=Uuid::new_v4();let connect_only=Uuid::new_v4();let role=Uuid::new_v4();
    let first=Uuid::new_v4();let readonly_message=Uuid::new_v4();
    {
        let mut b=backend.lock().unwrap();b.server_value["owner_id"]=serde_json::Value::Null;
        b.profile["roles"]=json!([{"id":role,"name":"Member"}]);b.fail_upload=false;
        b.admin_roles=vec![json!({"id":role,"name":"Member","permissions":{"send_attachment":true},"created_at":"2026-10-03T12:00:00Z"})];
        for (id,name,read,send,connect) in [(room,"Call chat",true,true,true),(read_only,"Read only chat",true,false,false),(connect_only,"Connect only",false,false,true)]{
            b.admin_channels.push(json!({"id":id,"name":name,"type":"voice","position":10,"created_at":"2026-10-03T12:00:00Z","permissions":[{"role_id":role,"role_name":"Member","permissions":{"read_channel":read,"send_messages":send,"connect_voice":connect}}]}));
        }
        for (id,message,content) in [(room,first,"Room history"),(read_only,readonly_message,"Read only history")]{
            let mut m=b.message.clone();m["id"]=json!(message);m["channel_id"]=json!(id);m["content"]=json!(content);b.channel_messages.insert(id.to_string(),vec![m]);
        }
    }
    let own=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:own,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().background_started&&main.model().access_request.is_none()&&main.model().voice_allowed(connect_only));
    if let Some(listener)=main.state().get_mut().model.ws_listener.take(){listener.abort();}
    let(commands,mut receiver)=tokio::sync::mpsc::channel(128);main.state().get_mut().model.ws_tx=commands;
    main.emit(MainWindowMsg::Voice(VoiceMsg::TestEngine));main.emit(MainWindowMsg::WsReceived(WsEvent::ConnectionReady(Uuid::new_v4())));pump(context);
    let text=main.model().channels.iter().find(|c|c.channel_type==Some(ChannelType::Text)).unwrap().clone();
    let chat_button=|id|descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.widget_name()==format!("voice-chat-{id}")));
    let visible_message=|id|descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("message-{id}"));
    let composer=||descendants(main.model().chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::TextView>().ok().filter(|w|w.widget_name()=="message-composer")).unwrap();
    until(context,||chat_button(room).is_some()&&chat_button(read_only).is_some());assert!(chat_button(connect_only).is_none());
    chat_button(room).unwrap().emit_clicked();until(context,||main.model().active_channel_id==Some(room)&&visible_message(first));
    assert!(composer().is_sensitive());assert!(main.model().voice.call.is_none());
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p=="/voice/ice-servers"),"opening text chat must not request microphone access or join voice");
    composer().buffer().set_text("Voice sent fixture");pump(context);main.model().chat.emit(ChatMsg::SendClicked);
    let sent:Uuid="12345678-1234-4234-8234-123456789ac1".parse().unwrap();until(context,||visible_message(sent));
    assert!(backend.lock().unwrap().requests.iter().any(|(m,p,b)|m=="POST"&&p=="/messages"&&b.as_str().is_some_and(|b|b.contains(&room.to_string())&&b.contains("Voice sent fixture"))));
    let mut live:crate::models::Message=serde_json::from_value(backend.lock().unwrap().message.clone()).unwrap();live.id=Uuid::new_v4();live.channel_id=room;live.content=Some("Live call chat".into());
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(live.clone())));until(context,||visible_message(live.id));
    // Selecting/joining voice leaves the current message target intact.
    main.emit(MainWindowMsg::Voice(VoiceMsg::Select(room)));main.emit(MainWindowMsg::Voice(VoiceMsg::Join));until(context,||main.model().voice.call.as_ref().is_some_and(|c|c.phase==Phase::Joining));
    let own=main.model().current_user.id;main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Joined{channel_id:room,members:vec![VoiceMember{user_id:own,muted:true,camera_on:false,screen_sharing:false}],active_speakers:vec![]})));until(context,||main.model().voice.call.as_ref().is_some_and(|c|c.phase==Phase::Negotiating));
    let call=main.model().voice.call.as_ref().unwrap().id;
    chat_button(read_only).unwrap().emit_clicked();until(context,||main.model().active_channel_id==Some(read_only)&&visible_message(readonly_message));
    assert!(!composer().is_sensitive());assert_eq!(main.model().voice.call.as_ref().unwrap().id,call);
    let posts=backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="POST"&&p=="/messages").count();main.model().chat.emit(ChatMsg::SendClicked);pump(context);assert_eq!(backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="POST"&&p=="/messages").count(),posts);
    let restricted=main.model().managed_channels.iter().find(|c|c.id==connect_only).unwrap().clone();main.emit(MainWindowMsg::ChannelSelected(restricted));pump(context);assert_eq!(main.model().active_channel_id,Some(read_only));
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p.starts_with(&format!("/channels/{connect_only}/messages"))));
    // Search/notification jumps can open a voice channel without joining it.
    main.emit(MainWindowMsg::ChannelSelected(text.clone()));until(context,||main.model().active_channel_id==Some(text.id));
    main.emit(MainWindowMsg::Navigate{channel_id:room,message_id:first});until(context,||main.model().active_channel_id==Some(room)&&descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("message-{first}")&&w.has_css_class("papo-message-highlight")));
    assert_eq!(main.model().voice.call.as_ref().unwrap().id,call);
    // Revoking text access hides chat without ending an allowed voice call.
    {let mut b=backend.lock().unwrap();let c=b.admin_channels.iter_mut().find(|c|c["id"]==room.to_string()).unwrap();c["permissions"][0]["permissions"]["read_channel"]=json!(false);}
    main.emit(MainWindowMsg::RefreshAccess);until(context,||main.model().access_request.is_none()&&!main.model().access[&room].read&&main.model().active_channel_id!=Some(room));
    until(context,||chat_button(room).is_none());assert_eq!(main.model().voice.call.as_ref().unwrap().id,call);
    // Revoking voice only ends audio; restoring read access keeps chat usable.
    {let mut b=backend.lock().unwrap();let c=b.admin_channels.iter_mut().find(|c|c["id"]==room.to_string()).unwrap();c["permissions"][0]["permissions"]["read_channel"]=json!(true);c["permissions"][0]["permissions"]["connect_voice"]=json!(false);}
    main.emit(MainWindowMsg::RefreshAccess);until(context,||main.model().access_request.is_none()&&main.model().voice.call.is_none()&&!main.model().voice_allowed(room));until(context,||chat_button(room).is_some());
    chat_button(room).unwrap().emit_clicked();until(context,||main.model().active_channel_id==Some(room)&&visible_message(first));
    assert!(composer().is_sensitive());assert!(main.model().voice.call.is_none());
    while let Ok(command)=receiver.try_recv(){if let ws::WsCommand::Voice{text,..}=command{assert!(!text.contains("voice_offer"),"the test engine never starts physical capture");}}
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
