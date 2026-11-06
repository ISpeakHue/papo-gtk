//! EN-US: Unread indicators follow surviving messages, including snapshots already in flight.
//! PT-BR: Indicadores de não lidas acompanham mensagens restantes, incluindo consultas já em andamento.
use super::*;
use adw::prelude::*;
use crate::ui::chat::actions::tests::{mock,descendants,pump,until};

pub(super) fn exercise(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let channel:Uuid="12345678-1234-4234-8234-123456789ac2".parse().unwrap();
    let time=chrono::DateTime::parse_from_rfc3339("2026-10-03T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let message=|second|serde_json::from_value::<Message>(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":Uuid::nil(),"content":"Synthetic message","created_at":time+chrono::Duration::seconds(second)})).unwrap();
    let first=message(0);let deleted=message(1);
    let summary=|m:&Message|serde_json::json!({"id":m.id,"created_at":m.created_at,"author_id":m.author_id,"content":m.content});
    {let mut b=backend.lock().unwrap();b.admin_channels[1]["last_message"]=summary(&first);b.admin_channels[1]["last_read_message"]=serde_json::json!(first.id);}
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    let window=adw::Window::builder().default_width(1100).default_height(700).content(main.widget()).build();window.present();
    until(context,||main.model().active_channel_id.is_some()&&main.model().connection.state.background_started&&main.model().access_request.is_none());
    let active=main.model().active_channel_id;
    let unread=||main.model().channels.iter().find(|c|c.id==channel).unwrap().has_unread();
    let dot=||descendants(main.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("unread-{channel}"));
    assert!(!unread()&&!dot());
    // EN-US: Hold a synthetic permission refresh open across create/delete to reproduce the stale-snapshot race.
    // PT-BR: Mantém uma consulta sintética aberta durante criação/exclusão para reproduzir a resposta antiga.
    let request=Uuid::new_v4();let version=main.model().notifications.journal.version;
    main.state().get_mut().model.access_request=Some(request);
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(deleted.clone())));pump(context);assert!(unread()&&dot());
    let snapshot={let m=main.model();AccessSnapshot{user:m.current_user.clone(),server:m.server.clone(),roles:m.roles.clone(),channels:m.channels.clone(),access:m.access.clone(),server_access:m.server_access.clone(),read_version:version,member_version:m.members.version()}};
    main.emit(MainWindowMsg::WsReceived(WsEvent::MessageDelete{id:deleted.id,channel_id:channel}));pump(context);
    assert!(!unread()&&!dot(),"deleting the only unread message clears its dot before the refresh returns");
    assert_eq!(main.model().channels.iter().find(|c|c.id==channel).unwrap().last_read_message,Some(first.id));
    main.emit(MainWindowMsg::AccessLoaded{request_id:request,result:Ok(snapshot)});
    until(context,||main.model().access_request.is_none());assert!(!unread()&&!dot(),"an older snapshot must not restore the deleted head");
    let remaining=message(2);let latest=message(3);
    {backend.lock().unwrap().admin_channels[1]["last_message"]=summary(&remaining);}
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(remaining.clone())));main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(latest.clone())));pump(context);assert!(dot());
    main.emit(MainWindowMsg::WsReceived(WsEvent::MessageDelete{id:latest.id,channel_id:channel}));pump(context);
    assert!(unread()&&dot(),"deleting the newest message must retain an earlier unread message");
    until(context,||main.model().access_request.is_none());assert!(unread()&&dot());
    {backend.lock().unwrap().admin_channels[1]["last_message"]=summary(&first);}
    main.emit(MainWindowMsg::WsReceived(WsEvent::MessageDelete{id:remaining.id,channel_id:channel}));
    until(context,||main.model().access_request.is_none()&&!unread()&&!dot());
    assert_eq!(main.model().active_channel_id,active,"deletions must not navigate to the inactive channel");
    let requests=backend.lock().unwrap().requests.clone();
    assert!(!requests.iter().any(|(method,path,_)|method=="PUT"&&path==&format!("/channels/{channel}/read")),"deletion never marks surviving messages read");
    assert!(!requests.iter().any(|(_,path,_)|path.starts_with(&format!("/channels/{channel}/messages"))),"unread reconciliation needs no inactive history downloads");
    window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}
