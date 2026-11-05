//! EN-US: Account identity protects retained drafts/history even when server UUIDs coincide.
//! PT-BR: A identidade da conta protege rascunhos/histórico mesmo quando UUIDs dos servidores coincidem.
use super::*;
use actions::tests::{mock, pump};
use adw::prelude::*;

pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let (api_a,_,server_a)=tokio::runtime::Handle::current().block_on(mock());
    let (api_b,_,server_b)=tokio::runtime::Handle::current().block_on(mock());
    let user=Uuid::new_v4();let channel=Uuid::new_v4();let other=Uuid::new_v4();
    let target=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"Account scope","created_at":"2026-11-04T12:00:00Z"})).unwrap();
    let message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"content":"First server only","created_at":"2026-11-04T12:00:00Z"})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api_a.clone()),user_id:Some(user),active_channel:Some(target(channel)),messages:vec![message],..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(650).content(chat.widget()).build();window.present();performance::settle(context);
    chat.emit(ChatMsg::InputChanged("Draft on first server".into()));
    chat.emit(ChatMsg::SetChannel(target(other)));pump(context);
    assert_eq!(chat.model().session.drafts[&channel].text,"Draft on first server");
    chat.emit(ChatMsg::SetChannel(target(channel)));pump(context);
    assert_eq!(chat.model().draft.text,"Draft on first server");
    assert_eq!(chat.model().history.messages.len(),1);
    let stale=Uuid::new_v4();chat.emit(ChatMsg::BeginSnapshot(stale));pump(context);
    {let mut state=chat.state().get_mut();state.model.actions.api=Some(api_b.clone());}
    chat.emit(ChatMsg::SetAccess{user_id:user,access:Default::default()});pump(context);
    assert_eq!(chat.model().session.owner,Some(crate::account::AccountId::new(&api_b,user)));
    assert!(chat.model().draft.text.is_empty()&&chat.model().session.drafts.is_empty());
    assert!(chat.model().history.messages.is_empty());
    assert!(chat.model().transfers.textures.is_empty()&&chat.model().actions.emoji_names.is_empty());
    assert!(chat.model().session.conversations.entries_empty());
    // EN-US: A late response from the previous account cannot complete the new draft or history request.
    // PT-BR: Uma resposta atrasada da conta anterior não pode concluir o novo rascunho/histórico.
    chat.emit(ChatMsg::HistoryLoaded{request_id:stale,append:false,result:Ok(MessageListResponse{channel_id:channel,messages:vec![],has_more:false})});pump(context);
    assert!(chat.model().history.messages.is_empty());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server_a.abort();server_b.abort();pump(context);
}
