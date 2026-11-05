//! EN-US: Timed overrides never overwrite a channel's permanent server mode.
//! PT-BR: Pausas temporárias nunca substituem o modo permanente do canal no servidor.
use super::*;
use adw::prelude::*;
pub(super) struct Controls {pub window:gtk::Window,body:gtk::Box,scope:gtk::DropDown,ids:Vec<Option<Uuid>>,status:gtk::Label}
impl MainWindowModel {
    pub(super) fn notification_controls(&mut self,msg:&NoticeMsg,root:&gtk::Box,sender:&ComponentSender<Self>)->bool {
        match msg {
            NoticeMsg::Controls(channel)=>{
                if let Some(v)=self.notifications.controls.take(){v.window.close();}
                let (window,body)=crate::ui::chat::actions::window(root,crate::i18n::tr("Sons e pausas de notificações"));
                let info=gtk::Label::new(Some(crate::i18n::tr("Neste dispositivo e conta. Pausas e Ocupado silenciam avisos de mensagens, incluindo menções e DMs. O histórico e os não lidos são preservados. Sons de entrada e saída de chamadas são independentes.")));info.set_wrap(true);info.set_xalign(0.0);body.append(&info);
                let channels=self.notification_channels();let mut ids=vec![None];ids.extend(channels.iter().map(|c|Some(c.id)));
                let mut names=vec![crate::i18n::tr("Todas as notificações").to_string()];names.extend(channels.into_iter().map(|c|c.name));
                let scope=gtk::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());scope.set_selected(ids.iter().position(|id|id==channel).unwrap_or(0) as u32);scope.set_enable_search(true);scope.update_property(&[gtk::accessible::Property::Label(crate::i18n::tr("Canal ou todas as notificações"))]);body.append(&scope);
                let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);
                let durations=gtk::DropDown::from_strings(&[crate::i18n::tr("Retomar agora"),crate::i18n::tr("15 minutos"),crate::i18n::tr("1 hora"),crate::i18n::tr("8 horas"),crate::i18n::tr("24 horas")]);durations.update_property(&[gtk::accessible::Property::Label(crate::i18n::tr("Duração da pausa"))]);durations.set_selected(1);body.append(&durations);
                let apply=gtk::Button::with_label(crate::i18n::tr("Aplicar pausa"));let choices=ids.clone();let choice=scope.clone();let s=sender.clone();apply.connect_clicked(move |_|{if let Some(scope)=choices.get(choice.selected() as usize){s.input(MainWindowMsg::Notification(NoticeMsg::SetPolicy{scope:*scope,minutes:Some([0,15,60,480,1440][durations.selected().min(4) as usize]),sounds:None}));}});body.append(&apply);
                let legacy=self.connection.config.notifications.as_ref().and_then(|n|n.sound).unwrap_or(true);
                let sounds=adw::PreferencesGroup::new();body.append(&sounds);
                let message=adw::SwitchRow::new();message.set_title(crate::i18n::tr("Som das mensagens (neste dispositivo)"));message.set_active(self.notifications.policy.message_sound(legacy));sounds.add(&message);
                let call=adw::SwitchRow::new();call.set_title(crate::i18n::tr("Som das chamadas (neste dispositivo)"));call.set_active(self.notifications.policy.call_sound(legacy));sounds.add(&call);
                let save=gtk::Button::with_label(crate::i18n::tr("Salvar sons"));let s=sender.clone();save.connect_clicked(move |_|s.input(MainWindowMsg::Notification(NoticeMsg::SetPolicy{scope:None,minutes:None,sounds:Some((message.is_active(),call.is_active()))})));body.append(&save);
                let s=sender.clone();scope.connect_selected_notify(move |_|s.input(MainWindowMsg::Notification(NoticeMsg::PolicyScopeChanged)));
                window.set_child(None::<&gtk::Widget>);let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&body).build();scroll.set_widget_name("notification-controls-scroll");window.set_child(Some(&scroll));window.set_default_size(560,620);
                self.notifications.controls=Some(Controls{window:window.clone(),body,scope,ids,status});self.render_notification_policy();window.present();
            },
            NoticeMsg::PolicyScopeChanged=>self.render_notification_policy(),
            NoticeMsg::SetPolicy{scope,minutes,sounds}=>{
                if self.notifications.policy_saving{return true;}
                if scope.is_some_and(|id|!self.notification_channels().iter().any(|c|c.id==id)){return true;}
                let mut policy=self.notifications.policy.clone();
                if let Some(minutes)=minutes {if ![0,15,60,480,1440].contains(minutes){return true;}let until=(*minutes>0).then(||chrono::Utc::now()+chrono::Duration::minutes(i64::from(*minutes)));if let Some(id)=scope{if let Some(until)=until{policy.channels.insert(*id,until);}else{policy.channels.remove(id);}}else{policy.snooze=until;}}
                if let Some((messages,calls))=sounds{policy.messages=Some(*messages);policy.calls=Some(*calls);}
                self.notifications.policy_saving=true;if let Some(v)=&self.notifications.controls{v.body.set_sensitive(false);v.status.set_text(crate::i18n::tr("Salvando…"));}
                let owner=self.connection.account.clone();let s=sender.clone();self.notifications.jobs.push(tokio::spawn(async move{let copy=policy.clone();let result=tokio::task::spawn_blocking(move||copy.save(&owner)).await.map_err(anyhow::Error::from).and_then(|r|r);s.input(MainWindowMsg::Notification(NoticeMsg::PolicySaved{policy,result}));}));
            },
            NoticeMsg::PolicySaved{policy,result}=>{
                self.notifications.policy_saving=false;if let Some(v)=&self.notifications.controls{v.body.set_sensitive(true);}
                match result{Ok(())=>{self.deliver_notices(sender.clone());self.notifications.policy=policy.clone();self.render_notification_policy();self.deliver_notices(sender.clone());},Err(e)=>{if let Some(v)=&self.notifications.controls{v.status.set_text(&crate::i18n::fmt("Falha ao salvar: {e}",&[("e",e.to_string())]));}}}
            },_=>return false
        }true
    }
    fn render_notification_policy(&self){if let Some(v)=&self.notifications.controls{
        v.body.set_sensitive(!self.notifications.policy_saving);
        let scope=v.ids.get(v.scope.selected() as usize).copied().flatten();let until=scope.and_then(|id|self.notifications.policy.channels.get(&id).copied()).or(if scope.is_none(){self.notifications.policy.snooze}else{None});
        v.status.set_text(&if let Some(until)=until.filter(|until|*until>chrono::Utc::now()){crate::i18n::fmt("Pausado até {time}",&[("time",until.with_timezone(&chrono::Local).format(crate::i18n::datetime_pattern()).to_string())])}else{crate::i18n::tr("Sem pausa temporária. As regras permanentes continuam valendo.").into()});
    }}
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until};
    let (api,_backend,server)=tokio::runtime::Handle::current().block_on(mock());let own=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:own,api_client:api}).detach();let window=adw::Window::builder().default_width(1000).default_height(720).content(main.widget()).build();window.present();main.emit(MainWindowMsg::StartLoading);
    until(context,||main.model().channels.len()>=2&&main.model().connection.state.background_started);
    let ch=main.model().active_channel_id.unwrap();let permanent=main.model().channels.iter().find(|c|c.id==ch).unwrap().notification_settings.clone();let config=main.model().connection.config.clone();
    let message:Message=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":ch,"author_id":Uuid::new_v4(),"content":"synthetic mention","created_at":chrono::Utc::now()})).unwrap();let notice=Uuid::new_v4();
    {let mut state=main.state().get_mut();state.model.notifications.inbox.observe(message.clone());state.model.notifications.inbox.event(NotificationEvent{id:notice,message_id:message.id,user_id:message.author_id,message_content:"synthetic mention".into()});}
    main.emit(MainWindowMsg::Notification(NoticeMsg::Controls(Some(ch))));until(context,||main.model().notifications.controls.is_some());
    main.emit(MainWindowMsg::Notification(NoticeMsg::SetPolicy{scope:Some(ch),minutes:Some(15),sounds:None}));until(context,||main.model().notifications.policy.channels.contains_key(&ch)&&!main.model().notifications.policy_saving);
    assert_eq!(main.model().channels.iter().find(|c|c.id==ch).unwrap().notification_settings,permanent);assert_eq!(main.model().connection.config,config);
    assert!(main.model().notifications.inbox.rows.contains_key(&notice));assert!(!main.model().notifications.inbox.rows[&notice].read);assert!(!main.model().notifications.inbox.rows[&notice].pending_desktop,"muted alerts cannot replay after expiration");
    let policy=policy::Policy::load(&main.model().connection.account);assert!(!policy.allows(Some(ch),false,chrono::Utc::now()));assert!(policy.allows(Some(ch),false,chrono::Utc::now()+chrono::Duration::minutes(16)));
    let other=crate::account::AccountId::new(&main.model().connection.api,Uuid::new_v4());assert_eq!(policy::Policy::load(&other),policy::Policy::default());
    main.emit(MainWindowMsg::Notification(NoticeMsg::SetPolicy{scope:None,minutes:Some(60),sounds:Some((false,true))}));until(context,||main.model().notifications.policy.snooze.is_some()&&!main.model().notifications.policy_saving);assert!(!main.model().notifications.policy.message_sound(true));assert!(main.model().notifications.policy.call_sound(false));assert!(!main.model().notifications.policy.allows(None,false,chrono::Utc::now()));
    main.emit(MainWindowMsg::Notification(NoticeMsg::SetPolicy{scope:Some(ch),minutes:Some(0),sounds:None}));until(context,||!main.model().notifications.policy.channels.contains_key(&ch)&&!main.model().notifications.policy_saving);assert!(main.model().notifications.policy.snooze.is_some());
    main.emit(MainWindowMsg::Notification(NoticeMsg::SetPolicy{scope:None,minutes:Some(0),sounds:None}));until(context,||main.model().notifications.policy.snooze.is_none()&&!main.model().notifications.policy_saving);assert!(policy::Policy::load(&main.model().connection.account).allows(Some(ch),false,chrono::Utc::now()));assert!(!main.model().notifications.policy.allows(None,true,chrono::Utc::now()));
    main.model().notifications.controls.as_ref().unwrap().window.close();window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}
