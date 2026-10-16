//! Local socket recovery and cached-draft regression.
use super::*;
use adw::prelude::*;
use crate::ui::chat::actions::tests::{mock,pump,until};
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().background_started&&main.model().access_request.is_none()&&!main.model().history_recovering);
    {let mut state=main.state().get_mut();state.model.startup_on_socket=true;state.model.history_recovering=true;}
    main.emit(MainWindowMsg::WsReceived(WsEvent::Reconnected));pump(context);assert!(main.model().history_recovering,"the first socket handshake cannot claim history is already reconciled");
    main.emit(MainWindowMsg::HistoryObserved{channel_id:main.model().active_channel_id.unwrap(),messages:vec![],latest:true});pump(context);
    let composer=crate::ui::chat::composer::field(main.model().chat.widget().upcast_ref());composer.set_text("Keep my unsent draft");pump(context);
    let messages=||crate::ui::chat::actions::tests::descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.widget_name().strip_prefix("message-").and_then(|id|Uuid::parse_str(id).ok())).collect::<Vec<_>>();let previous=messages();
    main.emit(MainWindowMsg::WsReceived(WsEvent::Disconnected));pump(context);assert!(main.model().connection_status.as_ref().unwrap().contains("Reconectando"));
    assert_eq!(messages(),previous);assert_eq!(composer.text(),"Keep my unsent draft");
    let token=Uuid::new_v4();main.state().get_mut().model.access_request=Some(token);
    main.emit(MainWindowMsg::AccessLoaded{request_id:token,result:Err(anyhow::anyhow!("temporary offline"))});pump(context);assert_eq!(messages(),previous);assert_eq!(composer.text(),"Keep my unsent draft");
    let channel=main.model().active_channel_id.unwrap();assert!(main.model().access[&channel].read,"a transient request failure preserves the last authorized cached view");
    direct::exercise_transient(&main,context);
    let before=backend.lock().unwrap().requests.len();
    main.emit(MainWindowMsg::WsReceived(WsEvent::Reconnected));
    until(context,||main.model().socket_ready&&main.model().access_request.is_none()&&!main.model().history_recovering&&main.model().connection_status.is_none());
    assert_eq!(composer.text(),"Keep my unsent draft");
    let ready_requests=backend.lock().unwrap().requests.len()-before;
    until(context,||backend.lock().unwrap().requests[before..].iter().any(|(m,p,_)|m=="GET"&&p.split('?').next().is_some_and(|route|route.ends_with("/notifications"))));
    let requests=backend.lock().unwrap().requests.clone();let new=&requests[before..];
    let history=new.iter().position(|(m,p,_)|m=="GET"&&p.contains("/messages"));let notices=new.iter().position(|(m,p,_)|m=="GET"&&p.split('?').next().is_some_and(|route|route.ends_with("/notifications")));assert!(history.is_some()&&notices>history,"background inbox refresh follows selected-chat reconciliation");assert!(!new.iter().any(|(m,p,_)|m=="POST"&&p.contains("/messages")),"recovery never replays an unsent or ambiguous mutation");
    assert!(new.iter().filter(|(_,p,_)|p=="/users/profile_batch").count()<=2,"recovery revalidates visible avatars only");
    host.set_visible(false);pump(context);assert!(main.model().refresh_interval(main.widget())>=Duration::from_secs(90));
    eprintln!("recovery benchmark: requests_until_ready={}, avatar_batches={}",ready_requests,new.iter().filter(|(_,p,_)|p=="/users/profile_batch").count());
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
