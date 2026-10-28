//! Explicit channel actions and leaner notification/search/call dialogs.
use super::*;
use adw::prelude::*;
use crate::ui::chat::actions::tests::{mock,descendants,find_button,pump,until};

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let now=chrono::Utc::now();let message=Uuid::new_v4();
    let other:Uuid="12345678-1234-4234-8234-123456789ac2".parse().unwrap();
    {let mut b=backend.lock().unwrap();b.admin_channels[1]["last_message"]=serde_json::json!({"id":message,"created_at":now});b.channel_messages.insert(other.to_string(),vec![serde_json::json!({"id":message,"channel_id":other,"content":"Unread elsewhere","created_at":now})]);}
    let user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().active_channel_id.is_some()&&main.model().background_started&&main.model().access_request.is_none());let selected=main.model().active_channel_id.unwrap();
    main.emit(MainWindowMsg::MarkChannelRead(selected));until(context,||main.model().notifications.manual_reads.is_empty()&&main.model().channels.iter().find(|c|c.id==selected).is_some_and(|c|!c.has_unread()));
    let count=||backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p==&format!("/channels/{other}/read")).count();
    let popup=||{
        let row=descendants(main.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("channel-{other}")).unwrap();
        let controllers=row.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>().filter(|g|g.button()==3)).unwrap();
        click.emit_by_name::<()>("pressed",&[&1i32,&20.0f64,&12.0f64]);pump(context);find_button(&row,"Marcar canal como lido").emit_clicked();
    };
    backend.lock().unwrap().fail_channel_read=true;popup();until(context,||count()==1&&main.model().notifications.manual_reads.is_empty());
    assert!(main.model().channels.iter().find(|c|c.id==other).unwrap().has_unread());assert_eq!(main.model().active_channel_id,Some(selected),"marking a channel read must not navigate to it");
    popup();until(context,||count()==2&&main.model().notifications.manual_reads.is_empty());assert!(!main.model().channels.iter().find(|c|c.id==other).unwrap().has_unread());assert_eq!(main.model().active_channel_id,Some(selected));
    let requests=backend.lock().unwrap().requests.clone();assert!(requests.iter().any(|(m,p,_)|m=="GET"&&p.contains(&format!("/channels/{other}/messages?"))&&p.contains("limit=1")));
    main.emit(MainWindowMsg::Search(search::SearchMsg::Open));pump(context);let search=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Pesquisar mensagens"))).unwrap();
    let controllers=search.observe_controllers();let keys=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Bubble)).unwrap();assert!(keys.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Escape,&0u32,&gtk::gdk::ModifierType::empty()]));pump(context);assert!(!search.is_visible());assert!(main.model().search.token.is_none());
    main.emit(MainWindowMsg::Notification(notifications::NoticeMsg::Open));pump(context);let inbox=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Notificações"))).unwrap();
    assert!(!descendants(inbox.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Atualizar notificações"))));inbox.close();
    main.emit(MainWindowMsg::Voice(voice::VoiceMsg::Settings));pump(context);let settings=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some("Configurar chamada"))).unwrap();
    let gain=descendants(settings.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Scale>().ok().filter(|w|w.widget_name()=="microphone-gain")).unwrap();assert_eq!(gain.value(),150.0);assert_eq!(gain.adjustment().upper(),300.0);gain.set_value(200.0);pump(context);assert_eq!(gain.value(),200.0);assert!(!backend.lock().unwrap().requests.iter().any(|(_,p,_)|p=="/voice/ice-servers"),"gain adjustment outside a call does not start capture");settings.close();
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
