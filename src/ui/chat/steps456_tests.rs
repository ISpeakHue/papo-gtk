//! EN-US: Local language, restart recovery and delayed-send workflows never use real accounts.
//! PT-BR: Fluxos locais de idioma, recuperação e envio atrasado não usam contas reais.
use super::*;
use actions::tests::{mock,pump,until,descendants,find_button,USER,CHANNEL,DATE};
use adw::prelude::*;

fn target(id:Uuid)->Channel{serde_json::from_value(serde_json::json!({"id":id,"name":"Mensagem","created_at":DATE})).unwrap()}
fn message(channel:Uuid)->Message{serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":DATE,"content":"Reply target"})).unwrap()}

pub(crate) fn drafts(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let user=USER.parse().unwrap();let channel:Uuid=CHANNEL.parse().unwrap();let reply=message(channel);
    let owner=crate::account::AccountId::new(&api,user);let path=crate::drafts::path(&owner).unwrap();
    let file_root=path.parent().unwrap().join("fixture-files");std::fs::create_dir_all(&file_root).unwrap();
    let keep=file_root.join("keep.txt");let gone=file_root.join("gone.txt");std::fs::write(&keep,b"keep").unwrap();std::fs::write(&gone,b"gone").unwrap();
    let files=tokio::runtime::Handle::current().block_on(async{vec![crate::api::features::UploadFile::inspect(keep.clone()).await.unwrap(),crate::api::features::UploadFile::inspect(gone.clone()).await.unwrap()]});
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api.clone()),user_id:Some(user),active_channel:Some(target(channel)),messages:vec![reply.clone()],..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(900).default_height(700).content(chat.widget()).build();window.present();
    until(context,||!chat.model().persistence.loading);
    {chat.state().get_mut().model.draft.files=files;}
    let text=format!("á é ç\n@mention(<@{user}>) unfinished");
    chat.emit(ChatMsg::SetReply(Some(reply)));chat.emit(ChatMsg::InputChanged(text.clone()));
    until(context,||crate::drafts::load_at(&path,&owner).is_ok_and(|s|s.drafts.get(&channel).is_some_and(|d|d.text==text&&d.reply.is_some())));
    assert!(chat.model().draft_channels.contains(&channel));
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);pump(context);std::fs::remove_file(&gone).unwrap();
    backend.lock().unwrap().channel_messages.insert(channel.to_string(),vec![]);
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),user_id:Some(user),active_channel:Some(target(channel)),messages:vec![message(channel)],..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(900).default_height(700).content(chat.widget()).build();window.present();
    until(context,||!chat.model().persistence.loading&&chat.model().draft.text==text&&!chat.model().draft.reply_unresolved);
    assert_eq!(chat.model().draft.files.len(),1);assert_eq!(chat.model().draft.files[0].path,keep);
    assert!(chat.model().draft.recovery_notice.as_ref().unwrap().contains("1 anexo"));
    assert!(chat.model().draft.reply.is_none());assert!(chat.model().draft.reply_error.as_ref().unwrap().contains("removida"));
    let field=composer::field(chat.widget().upcast_ref());assert!(field.text().starts_with("á é ç\n"));
    // EN-US: Replay a delayed restore across GTK's programmatic changed signal, then a genuine edit.
    // PT-BR: Reproduz restauração atrasada durante changed programático do GTK e depois uma edição real.
    let next=Uuid::new_v4();{let mut state=chat.state().get_mut();state.model.persistence.loading=true;state.model.persistence.dirty.clear();}
    chat.emit(ChatMsg::SetChannel(target(next)));pump(context);
    let restored=||{let mut draft=Draft::default();draft.text="Recovered after switching".into();persistence::Loaded{drafts:HashMap::from([(next,draft)]),pending:Default::default(),snapshot:crate::drafts::Snapshot::empty(owner.clone())}};
    chat.emit(ChatMsg::DraftsLoaded{epoch:chat.model().persistence.epoch,result:Ok(restored())});until(context,||field.text()=="Recovered after switching");
    {let mut state=chat.state().get_mut();state.model.persistence.loading=true;state.model.persistence.dirty.clear();}
    field.set_text("Typed before restore");pump(context);chat.emit(ChatMsg::DraftsLoaded{epoch:chat.model().persistence.epoch,result:Ok(restored())});pump(context);assert_eq!(field.text(),"Typed before restore","real typing wins over a late disk snapshot");
    let reply=message(next);let id=reply.id;chat.emit(ChatMsg::SetReply(Some(reply)));chat.emit(ChatMsg::DeleteMessage(id));pump(context);assert!(chat.model().draft.reply.is_none());assert_eq!(field.text(),"Typed before restore");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}

pub(crate) fn outgoing(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let user=USER.parse().unwrap();let channel=CHANNEL.parse().unwrap();
    backend.lock().unwrap().upload_delay=std::time::Duration::from_millis(600);
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),user_id:Some(user),active_channel:Some(target(channel)),messages:vec![message(channel)],..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(900).default_height(700).content(chat.widget()).build();window.present();until(context,||!chat.model().persistence.loading);
    let field=composer::field(chat.widget().upcast_ref());field.set_text("first send");pump(context);chat.emit(ChatMsg::SendClicked);
    until(context,||chat.model().outbox.active.is_some()&&field.text().is_empty());
    assert!(field.is_editable());field.set_text("second send");pump(context);chat.emit(ChatMsg::SendClicked);
    until(context,||chat.model().outbox.entries.len()==2&&field.text().is_empty());field.set_text("third independent draft");pump(context);
    let other=Uuid::new_v4();chat.emit(ChatMsg::SetChannel(target(other)));pump(context);field.set_text("other-channel draft");pump(context);
    until(context,||chat.model().outbox.active.is_none()&&chat.model().outbox.entries.len()==1);
    assert_eq!(field.text(),"other-channel draft");
    chat.emit(ChatMsg::SetChannel(target(channel)));until(context,||field.text()=="third independent draft");
    let failed=chat.model().outbox.entries[0].id;assert!(matches!(chat.model().outbox.entries[0].status,outbox::Status::Failed{uncertain:true,..}));
    let calls=||backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="POST"&&p=="/messages").count();assert_eq!(calls(),2);
    performance::settle(context);assert_eq!(calls(),2,"an uncertain send must not retry itself");
    chat.emit(ChatMsg::RetryPending(failed));pump(context);find_button(window.upcast_ref(),"Cancelar").emit_clicked();pump(context);assert_eq!(calls(),2);
    chat.emit(ChatMsg::RetryPending(failed));pump(context);find_button(window.upcast_ref(),"Reenviar").emit_clicked();
    until(context,||chat.model().outbox.entries.is_empty());assert_eq!(calls(),3);assert_eq!(field.text(),"third independent draft");
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);
}

pub(crate) fn translations(context:&gtk::glib::MainContext,root:&impl IsA<gtk::Widget>){
    assert_eq!(crate::i18n::current(),crate::i18n::Language::English);
    assert!(descendants(root.as_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|label|label.text()=="Sign in"));
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    backend.lock().unwrap().profile["nickname"]=serde_json::json!("Salvar");
    let user=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    let main=crate::ui::main_window::MainWindowModel::builder().launch(crate::ui::main_window::MainWindowInit{current_user:user,api_client:api.clone()}).detach();
    let window=adw::Window::builder().default_width(1100).default_height(780).content(main.widget()).build();window.present();main.emit(crate::ui::main_window::MainWindowMsg::StartLoading);
    until(context,||main.model().active_channel_id.is_some()&&main.model().connection.state.background_started);
    assert!(descendants(main.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text()=="Salvar"),"a user's nickname must never be translated");
    let direct=descendants(main.widget().upcast_ref()).into_iter().find(|w|w.widget_name()=="direct-navigation").unwrap();assert_eq!(direct.tooltip_text().as_deref(),Some("Direct messages (Alt+2)"));
    use crate::ui::main_window::{MainWindowMsg,account::AccountMsg};
    main.emit(MainWindowMsg::Account(AccountMsg::Preferences));pump(context);
    let settings=gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.title().as_deref()==Some("Account settings")).unwrap();
    let language=descendants(settings.upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::ComboRow>().ok().filter(|r|r.title()=="Language (on this device)")).unwrap();
    assert_eq!(language.selected(),0);language.set_selected(2);pump(context);
    let _=tokio::runtime::Handle::current().block_on(api.save_settings(&crate::models::UserConfig::default()));
    find_button(settings.upcast_ref(),"Save preferences").emit_clicked();until(context,||crate::config::AppConfig::load().language==crate::i18n::Language::English);
    assert_eq!(crate::i18n::plural("messages_count",1),"1 message");
    assert_eq!(crate::i18n::plural("messages_count",2),"2 messages");
    assert_eq!(chrono::NaiveDate::from_ymd_opt(2026,11,4).unwrap().format(crate::i18n::date_pattern()).to_string(),"11/04/2026");
    settings.close();window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}
