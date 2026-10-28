//! Native regressions for the October 19 usability report; synthetic data only.
use super::*;
use adw::prelude::*;
use actions::tests::{mock,pump,until,descendants,find_button};

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let (mut latest,old)={let b=backend.lock().unwrap();(serde_json::from_value::<Message>(b.message.clone()).unwrap(),serde_json::from_value::<Message>(b.old.clone()).unwrap())};
    let user=latest.author_id.unwrap();let channel=latest.channel_id;let emoji=Uuid::new_v4();
    let long=format!("😄 :dance: {} end", "wide preview ".repeat(35));latest.content=Some(long.clone());
    let target:Channel=serde_json::from_value(serde_json::json!({"id":channel,"name":"Polish","created_at":latest.created_at})).unwrap();
    let mut reply=latest.clone();reply.id=Uuid::new_v4();reply.reply_to=Some(latest.id);reply.content=Some("Reply".into());reply.created_at+=chrono::Duration::seconds(1);
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api.clone()),active_channel:Some(target),messages:vec![latest.clone(),reply.clone()],user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let texture=crate::media::PreparedImage{width:64,height:32,pixels:vec![255;64*32*4]}.texture();
    {let mut state=chat.state().get_mut();state.model.actions.emoji_names.insert("dance".into(),emoji);state.model.actions.textures.insert(emoji,texture.clone());}
    let host=adw::Window::builder().default_width(960).default_height(680).content(chat.widget()).build();host.present();
    chat.emit(ChatMsg::SetReply(Some(latest.clone())));performance::settle(context);
    let previews:Vec<_>=descendants(host.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::TextView>().ok()).filter(|v|v.widget_name()=="reply-preview-text").collect();
    assert_eq!(previews.len(),2,"composer and message reference both use rich previews");
    for preview in &previews{let b=preview.buffer();assert!(b.slice(&b.start_iter(),&b.end_iter(),true).ends_with(" end"),"available width, not a character cutoff, controls clipping");let mut iter=b.start_iter();let mut paints=0;loop{paints+=usize::from(iter.paintable().is_some());if let Some(p)=iter.paintable(){let metrics=preview.pango_context().metrics(None,None);let height=(metrics.ascent()+metrics.descent()+pango::SCALE-1)/pango::SCALE;assert!(p.intrinsic_height()<=height);let next=iter.clone();let rect=preview.iter_location(&next);assert!(rect.height()<=preview.parent().unwrap().height(),"emoji and adjacent glyphs fit the single-line preview");}if !iter.forward_char(){break;}}assert_eq!(paints,1);assert!(b.text(&b.start_iter(),&b.end_iter(),false).contains("😄"));}
    assert_eq!(texture.height(),32,"compact previews must not resize the shared chat texture");
    let before=previews[0].parent().unwrap().width();crate::ui::chat::actions::tests::resize_window(&host,600,680);until(context,||host.width()<700);assert!(previews[0].parent().unwrap().width()<before);
    // Recently viewed images outside the opening batch survive a visit elsewhere.
    let mut resident=transfers::resident::Cache::default();let key=Uuid::new_v4();let owners=std::collections::HashSet::from([latest.id]);
    resident.select(channel,&HashMap::new());resident.insert(channel,key,&owners,&texture);
    resident.select(channel,&HashMap::from([(Uuid::new_v4(),owners.clone())]));
    assert_eq!(resident.get(channel,key).unwrap().as_ptr(),texture.as_ptr());
    resident.select(Uuid::new_v4(),&HashMap::new());assert!(resident.get(channel,key).is_some());
    resident.apply(&Change::Delete(latest.id));assert!(!resident.contains(channel,key));
    // A dated jump excludes the recent head; returning restores the true head.
    let count=backend.lock().unwrap().requests.len();chat.emit(ChatMsg::Action(ActionMsg::NavigateAt(MessageCursor::from(&old))));
    until(context,||chat.model().actions.highlight==Some(old.id));assert!(chat.model().history_detached);
    // EN-US: Count the two navigation pages, excluding an independent one-message read reconciliation.
    // PT-BR: Conta as duas páginas da navegação, excluindo a reconciliação independente de uma mensagem lida.
    let requests=backend.lock().unwrap().requests[count..].to_vec();
    let pages:Vec<_>=requests.iter().filter_map(|(_,p,_)|reqwest::Url::parse(&format!("http://fixture.invalid{p}")).ok())
        .filter(|url|url.path().ends_with("/messages")&&url.query_pairs().any(|(key,value)|key=="limit"&&value=="30")&&url.query_pairs().any(|(key,_)|key=="since")).collect();
    assert_eq!(pages.len(),2);
    assert!(pages.iter().any(|url|url.query_pairs().any(|(key,value)|key=="order"&&value=="desc")));
    assert!(pages.iter().any(|url|url.query_pairs().any(|(key,value)|key=="order"&&value=="asc")));
    assert!(!chat.model().history.messages.iter().any(|m|m.id==latest.id),"historical pages must not create an artificial gap to today's messages");
    chat.emit(ChatMsg::Latest);until(context,||!chat.model().history_detached&&chat.model().history.messages.iter().any(|m|m.id==latest.id));
    chat.emit(ChatMsg::Action(ActionMsg::NavigateAt(MessageCursor::from(&old))));until(context,||chat.model().actions.highlight==Some(old.id));
    let mut sent=latest.clone();sent.id=Uuid::new_v4();sent.content=Some("New message".into());sent.created_at+=chrono::Duration::seconds(2);{let mut b=backend.lock().unwrap();b.message["id"]=serde_json::json!(sent.id);b.message["content"]=serde_json::json!(sent.content);b.message["created_at"]=serde_json::json!(sent.created_at);}
    chat.emit(ChatMsg::InputChanged("New message".into()));pump(context);let request=chat.state().get_mut().model.draft.begin().unwrap();
    chat.emit(ChatMsg::SendFinished{request_id:request,channel_id:channel,result:Ok(sent.clone())});
    until(context,||!chat.model().history_detached&&chat.model().history.messages.iter().any(|m|m.id==sent.id));
    assert!(!chat.model().history.messages.iter().any(|m|m.id==old.id),"sending from a dated page returns to the head instead of joining disjoint ranges");
    // Direct messages never offer or insert member/everyone completion.
    let dm=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"user":{"id":Uuid::new_v4(),"username":"peer","created_at":latest.created_at},"created_at":latest.created_at,"unread_count":0})).unwrap();
    chat.emit(ChatMsg::SelectTarget{target:crate::models::ConversationTarget::Direct(dm),user_id:user,access:crate::models::Access::direct()});pump(context);
    let composer=composer::field(chat.widget().upcast_ref());composer.set_text("@al");composer.set_position(-1);pump(context);
    chat.emit(ChatMsg::OpenMentions);chat.emit(ChatMsg::MentionSelected(Some(user)));pump(context);
    assert!(!chat.model().mention_visible);assert_eq!(composer.text(),"@al");assert!(!find_button(chat.widget().upcast_ref(),"Mencionar membro (digite @ para filtrar)").is_visible());
    // Channel notification counts follow the same numeric badge convention as DMs.
    use crate::ui::sidebar::{SidebarModel,SidebarInit,SidebarMsg};
    let own=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    let c=serde_json::from_value(serde_json::json!({"id":channel,"name":"Badge","created_at":latest.created_at,"last_message":{"id":latest.id,"created_at":latest.created_at}})).unwrap();
    let sidebar=SidebarModel::builder().launch(SidebarInit{current_user:own,server:None,channels:vec![c]}).detach();
    sidebar.emit(SidebarMsg::ChannelCounts(HashMap::from([(channel,3)])));pump(context);
    let badge=||descendants(sidebar.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|w|w.widget_name()==format!("notifications-{channel}")));
    assert_eq!(badge().unwrap().text(),"3");assert!(descendants(sidebar.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("unread-{channel}")),"a notification count must not replace the separate unread dot");sidebar.emit(SidebarMsg::ChannelCounts(HashMap::new()));pump(context);assert!(badge().is_none());drop(sidebar);
    // Shared Escape handling closes transient windows, never their root.
    let child=adw::Window::builder().title("Escape probe").transient_for(&host).build();child.present();pump(context);
    let controllers=child.observe_controllers();let keys=(0..controllers.n_items()).filter_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>()).collect::<Vec<_>>();
    assert!(!keys.is_empty());for key in keys{key.emit_by_name::<bool>("key-pressed",&[&gtk::gdk::Key::Escape,&0u32,&gtk::gdk::ModifierType::empty()]);}
    pump(context);assert!(!child.is_visible()&&host.is_visible());
    host.set_content(None::<&gtk::Widget>);host.close();drop(chat);server.abort();pump(context);
}
