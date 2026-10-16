use super::*;
use crate::ui::chat::actions::tests::{pump,until,descendants,find_button};
use crate::models::AudioRoute;
const ROOM:&str="12345678-1234-4234-8234-123456789ad0";

pub(crate) fn exercise_controls(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::mock;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let first:Uuid=ROOM.parse().unwrap();let second=Uuid::new_v4();
    {
        let mut b=backend.lock().unwrap();
        for (id,name,position) in [(first,"Voice one",10),(second,"Voice two",11)]{
            b.admin_channels.push(serde_json::json!({"id":id,"name":name,"type":"voice","position":position,"created_at":"2026-10-03T12:00:00Z"}));
        }
    }
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    let window=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();window.present();
    until(context,||main.model().voice_allowed(second)&&main.model().active_channel_id.is_some()&&main.model().access_request.is_none());
    // Drive socket transitions explicitly; no live server or audio capture is used.
    if let Some(listener)=main.state().get_mut().model.ws_listener.take(){listener.abort();}
    let (commands,_receiver)=tokio::sync::mpsc::channel(64);
    main.state().get_mut().model.ws_tx=commands;
    let text=main.model().active_channel_id;
    let row=|id|descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ListBoxRow>().ok().filter(|r|r.widget_name()==format!("channel-{id}"))).unwrap();
    let select=|id|{let row=row(id);row.parent().and_downcast::<gtk::ListBox>().unwrap().emit_by_name::<()>("row-activated",&[&row]);};
    let ready=||{main.emit(MainWindowMsg::WsReceived(WsEvent::ConnectionReady(Uuid::new_v4())));pump(context);};
    ready();select(first);until(context,||row(first).has_css_class("papo-voice-selected")&&main.model().voice.join.is_sensitive());
    assert!(main.model().voice.call.is_none());assert!(!main.model().voice.status.is_visible());assert!(main.model().voice.status.text().is_empty());
    let text_row=row(text.unwrap());assert!(text_row.is_selected(),"opening voice controls preserves the selected text channel");
    main.emit(MainWindowMsg::WsReceived(WsEvent::Disconnected));pump(context);
    assert!(main.model().voice.status.text().is_empty(),"an idle socket disconnect is not an interrupted call");assert!(!main.model().voice.status.is_visible());assert!(!main.model().voice.join.is_sensitive());
    ready();assert!(main.model().voice.join.is_sensitive());assert!(main.model().voice.status.text().is_empty());
    select(second);until(context,||row(second).has_css_class("papo-voice-selected")&&!row(first).has_css_class("papo-voice-selected"));
    assert_eq!(main.model().active_channel_id,text);
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,path,_)|path=="/voice/ice-servers"),"selecting a room never joins a call");
    backend.lock().unwrap().admin_channels.iter_mut().find(|c|c["id"]==second.to_string()).unwrap()["name"]=serde_json::json!("Renamed voice");
    main.emit(MainWindowMsg::RefreshAccess);
    until(context,||main.model().managed_channels.iter().any(|c|c.id==second&&c.name=="Renamed voice")&&main.model().access_request.is_none());
    until(context,||row(second).has_css_class("papo-voice-selected"));assert!(text_row.is_selected());
    let style=adw::StyleManager::default();let scheme=style.color_scheme();style.set_color_scheme(adw::ColorScheme::ForceDark);
    crate::ui::main_window::layout::tests::preview(&window,"voice-selection",context);style.set_color_scheme(scheme);
    super::video::exercise_configuration(&main,context);
    find_button(main.model().voice.panel.upcast_ref(),"Fechar voz").emit_clicked();until(context,||!main.model().voice.panel.is_visible()&&!row(second).has_css_class("papo-voice-selected"));
    select(first);until(context,||main.model().voice.join.is_sensitive()&&row(first).has_css_class("papo-voice-selected"));assert!(main.model().voice.status.text().is_empty());
    main.model().voice.join.emit_clicked();until(context,||main.model().voice.call.as_ref().is_some_and(|c|c.phase==Phase::Joining));
    main.emit(MainWindowMsg::WsReceived(WsEvent::Disconnected));pump(context);
    assert!(main.model().voice.call.is_none());assert!(main.model().voice.status.is_visible());assert!(main.model().voice.status.text().contains("Conexão encerrada"),"an interrupted join must still report its error");
    ready();assert!(main.model().voice.status.text().contains("Conexão encerrada"),"reconnecting must preserve a real call interruption");
    select(second);until(context,||row(second).has_css_class("papo-voice-selected"));assert!(main.model().voice.status.text().is_empty(),"choosing a different room clears the old room's error");
    backend.lock().unwrap().admin_channels.retain(|c|c["id"]!=second.to_string());main.emit(MainWindowMsg::RefreshAccess);
    until(context,||main.model().voice.selected.is_none()&&!main.model().voice.panel.is_visible());
    assert_eq!(main.model().active_channel_id,text);
    assert!(!descendants(main.model().sidebar.widget().upcast_ref()).iter().any(|w|w.has_css_class("papo-voice-selected")));
    window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}

fn join(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext)->Uuid{
    main.emit(MainWindowMsg::WsReceived(WsEvent::ConnectionReady(Uuid::new_v4())));
    let row=descendants(main.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ListBoxRow>().ok().filter(|r|r.widget_name()==format!("channel-{ROOM}"))).expect("voice room must be in native navigation");
    row.parent().and_downcast::<gtk::ListBox>().unwrap().emit_by_name::<()>("row-activated",&[&row]);pump(context);assert!(main.model().voice.panel.get_visible());main.model().voice.join.emit_clicked();
    until(context,||main.model().voice.call.as_ref().is_some_and(|c|c.phase==Phase::Joining));
    let user=main.model().current_user.id;let id=main.model().voice.call.as_ref().unwrap().id;
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Joined{channel_id:ROOM.parse().unwrap(),members:vec![VoiceMember{user_id:user,muted:true,camera_on:false,screen_sharing:false}],active_speakers:vec![]})));
    until(context,||main.model().voice.call.as_ref().is_some_and(|c|c.phase==Phase::Negotiating));id
}
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext,backend:&std::sync::Arc<std::sync::Mutex<crate::ui::chat::actions::tests::Backend>>){
    use serde_json::json;
    let room:Uuid=ROOM.parse().unwrap();let peer:Uuid="12345678-1234-4234-8234-123456789ac0".parse().unwrap();let text=main.model().active_channel_id;
    backend.lock().unwrap().admin_channels.push(json!({"id":ROOM,"name":"Native voice","type":"voice","position":10,"created_at":"2026-10-03T12:00:00Z"}));
    main.emit(MainWindowMsg::RefreshAccess);until(context,||main.model().managed_channels.iter().any(|c|c.id==room));
    main.emit(MainWindowMsg::WsReceived(WsEvent::PresenceUpdate(ws::PresenceEntry{user_id:peer,status:ws::PresenceStatus::Online,status_message:None,typing:None,nickname:None,user_voice:vec![room]})));pump(context);
    let peer_row=descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("voice-member-{room}-{peer}")).expect("room participants appear before joining");
    assert!(main.model().voice.call.is_none());assert!(!main.model().voice.settings.is_visible());
    main.emit(MainWindowMsg::Voice(VoiceMsg::TestEngine));pump(context);let call=join(main,context);
    assert!(main.model().voice.mute.is_active());main.model().voice.mute.set_active(false);pump(context);assert!(!main.model().voice.call.as_ref().unwrap().muted);
    let cues=main.model().voice.cue.played;main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event:EngineEvent::Connection{state:"connected".into()}}));pump(context);assert_eq!(main.model().voice.call.as_ref().unwrap().phase,Phase::Connected);
    main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event:EngineEvent::Connection{state:"connected".into()}}));pump(context);assert_eq!(main.model().voice.cue.played,cues,"disabled sounds and repeated connection events must not play cues");
    main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event:EngineEvent::Speaking{active:true}}));pump(context);
    let own=main.model().current_user.id;let self_row=descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("voice-member-{room}-{own}")).unwrap();
    assert!(descendants(&self_row).iter().any(|w|w.widget_name()=="voice-speaking-fallback"&&w.is_visible()),"local capture must light the user's microphone before any server speaker event");
    main.model().voice.mute.set_active(true);pump(context);assert!(!main.model().voice.local_speaking);let self_row=descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("voice-member-{room}-{own}")).unwrap();assert!(!descendants(&self_row).iter().any(|w|w.widget_name()=="voice-speaking-fallback"&&w.is_visible()));
    main.model().voice.mute.set_active(false);pump(context);
    let member=VoiceMember{user_id:peer,muted:false,camera_on:false,screen_sharing:false};main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::State{channel_id:room,member})));
    pump(context);let state_row=descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==peer_row.widget_name()).unwrap();
    let channel_row=descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("channel-{room}")).unwrap();
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Speakers{channel_id:room,user_ids:vec![peer]})));
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Routes{channel_id:room,routes:vec![AudioRoute{track_id:"papo-audio-0".into(),user_id:peer}]})));pump(context);
    assert!(descendants(main.model().sidebar.widget().upcast_ref()).iter().any(|w|w.widget_name()=="voice-speaking-fallback"&&w.is_visible()),"speaking needs a microphone indicator even when avatars are disabled");
    assert_eq!(main.model().voice.routes["papo-audio-0"],peer);assert!(descendants(main.model().sidebar.widget().upcast_ref()).iter().any(|w|w.has_css_class("papo-voice-speaking")));
    assert_eq!(descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==state_row.widget_name()).unwrap(),state_row,"speaking events preserve participant buttons");
    assert_eq!(descendants(main.model().sidebar.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==channel_row.widget_name()).unwrap(),channel_row,"speaking events preserve channel clicks");
    let sidebar=main.model().sidebar.widget().clone();
    let mut config=main.model().account.config.clone();config.display.as_mut().unwrap().show_avatars=Some(true);main.model().sidebar.emit(SidebarMsg::SetConfig(config));pump(context);
    assert!(descendants(sidebar.upcast_ref()).iter().any(|w|w.widget_name()=="voice-speaking-fallback"&&w.is_visible()),"the name-side speaking indicator is visible with avatars too");
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::State{channel_id:room,member:VoiceMember{user_id:peer,muted:true,camera_on:false,screen_sharing:false}})));pump(context);
    let row=descendants(sidebar.upcast_ref()).into_iter().find(|w|w.widget_name()==state_row.widget_name()).unwrap();assert!(!descendants(&row).iter().any(|w|w.has_css_class("papo-voice-speaking")&&w.is_visible()));
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::State{channel_id:room,member:VoiceMember{user_id:peer,muted:false,camera_on:false,screen_sharing:false}})));pump(context);
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Speakers{channel_id:room,user_ids:vec![]})));pump(context);let row=descendants(sidebar.upcast_ref()).into_iter().find(|w|w.widget_name()==state_row.widget_name()).unwrap();assert!(!descendants(&row).iter().any(|w|w.widget_name()=="voice-speaking-fallback"&&w.is_visible()));
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Speakers{channel_id:room,user_ids:vec![peer]})));pump(context);
    let window=adw::Window::builder().default_width(1280).default_height(760).content(main.widget()).build();window.present();until(context,||window.is_mapped());
    crate::ui::main_window::layout::tests::preview(&window,"sidebar-call",context);
    let sidebar=main.model().sidebar.widget().clone();let all=descendants(sidebar.upcast_ref());let footer=all.iter().find(|w|w.has_css_class("papo-user-footer")).unwrap();let rail=all.iter().find(|w|w.has_css_class("papo-rail")).unwrap();let footer_rect=footer.compute_bounds(&sidebar).unwrap();assert!(footer_rect.width()>=sidebar.width() as f32-1.0);let rail_rect=rail.compute_bounds(&sidebar).unwrap();assert!(rail_rect.y()+rail_rect.height()<=footer_rect.y()+1.0);
    assert!(!descendants(main.model().voice.panel.upcast_ref()).iter().any(|w|w.is::<gtk::DropDown>()));main.model().voice.settings_button.emit_clicked();until(context,||main.model().voice.settings.is_mapped());let settings=main.model().voice.settings.clone();crate::ui::main_window::layout::tests::preview(&settings,"call-settings",context);settings.close();pump(context);assert!(!main.model().voice.settings.is_visible());
    window.set_content(None::<&gtk::Widget>);window.close();
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Routes{channel_id:room,routes:vec![]})));pump(context);assert!(main.model().voice.routes.is_empty());
    super::video::exercise(main,context,call,peer);
    main.model().voice.leave.emit_clicked();pump(context);assert!(main.model().voice.call.is_none());main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event:EngineEvent::Connection{state:"connected".into()}}));pump(context);assert!(main.model().voice.call.is_none());assert_eq!(main.model().active_channel_id,text);
    main.emit(MainWindowMsg::Account(super::super::account::AccountMsg::Preferences));pump(context);
    let prefs=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.title().as_deref()==Some("Configurações da conta")).unwrap();
    let sound=descendants(prefs.upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::SwitchRow>().ok().filter(|c|c.title().to_lowercase().contains("som"))).unwrap();sound.set_active(true);
    find_button(prefs.upcast_ref(),"Salvar preferências").emit_clicked();until(context,||main.model().account.config.notifications.as_ref().and_then(|n|n.sound)==Some(true));prefs.close();pump(context);
    let cues=main.model().voice.cue.played;
    let next=join(main,context);assert_eq!(main.model().voice.cue.played,cues+1,"room entry plays the enabled cue before ICE connects");
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Joined{channel_id:room,members:vec![],active_speakers:vec![]})));main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call:next,event:EngineEvent::Connection{state:"connected".into()}}));pump(context);assert_eq!(main.model().voice.cue.played,cues+1,"duplicate join/connection notifications must not replay the cue");
    main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event:EngineEvent::Error{message:"old worker".into()}}));pump(context);assert_eq!(main.model().voice.call.as_ref().unwrap().id,next);
    main.emit(MainWindowMsg::WsReceived(WsEvent::Disconnected));pump(context);assert_eq!(main.model().voice.cue.played,cues+2,"leaving a joined call plays a separate departure cue once");main.emit(MainWindowMsg::Voice(VoiceMsg::Leave));pump(context);assert_eq!(main.model().voice.cue.played,cues+2,"duplicate leave does not play another cue");assert!(main.model().voice.call.is_none());assert!(main.model().voice.connection.is_none());assert!(!main.model().voice.join.is_sensitive());
    for code in ["voice-room-full","voice-forbidden","voice-already-in-room","voice-invalid-sdp","voice-room-closed"]{join(main,context);main.emit(MainWindowMsg::WsReceived(WsEvent::Error{message:"voice failure".into(),code:Some(code.into())}));pump(context);assert!(main.model().voice.call.is_none(),"{code} must release audio");}
    let call=join(main,context);main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event:EngineEvent::Error{message:"Device disconnected".into()}}));pump(context);assert!(main.model().voice.call.is_none());
    join(main,context);backend.lock().unwrap().admin_channels.retain(|c|c["id"]!=ROOM);main.emit(MainWindowMsg::RefreshAccess);until(context,||main.model().voice.call.is_none()&&!main.model().managed_channels.iter().any(|c|c.id==room));
    // Restore the room and leave a live call for the revocation check.
    {let mut b=backend.lock().unwrap();b.server_value["owner_id"]=serde_json::Value::Null;
        b.profile["roles"]=json!([{"id":peer,"name":"Voice only"}]);
        b.admin_roles=vec![json!({"id":peer,"name":"Voice only","permissions":{"ban_members":true},"created_at":"2026-10-03T12:00:00Z"})];
        b.admin_channels.push(json!({"id":ROOM,"name":"Connect only","type":"voice","position":10,"created_at":"2026-10-03T12:00:00Z","permissions":[{"role_id":peer,"role_name":"Voice only","permissions":{"read_channel":false,"connect_voice":true}}]}));}
    main.emit(MainWindowMsg::RefreshAccess);until(context,||main.model().access.get(&room).is_some_and(|a|a.voice&&!a.read));
    assert!(!main.model().server_access.manage_server);assert!(!find_button(main.widget().upcast_ref(),"Moderação e auditoria").get_visible());
    assert!(!main.model().channels.iter().any(|c|c.id==room));
    assert!(descendants(main.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="Connect only"));
    join(main,context);
    // Parent smoke test revokes current roles next: leave a live call to verify
    // immediate cleanup before the asynchronous permission snapshot returns.
}
pub(crate) fn exercise_denied(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    assert!(main.model().voice.call.is_none());main.emit(MainWindowMsg::Voice(VoiceMsg::Select(ROOM.parse().unwrap())));main.emit(MainWindowMsg::Voice(VoiceMsg::Join));pump(context);assert!(main.model().voice.call.is_none());
}
