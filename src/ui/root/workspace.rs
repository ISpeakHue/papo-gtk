//! EN-US: Each account keeps its controller, cookie jar, socket and drafts while switching pages.
//! PT-BR: Cada conta mantém controlador, cookies, conexão e rascunhos ao trocar de página.
use super::*;
use crate::{account::AccountId,workspace::{Directory,Saved,LIVE_LIMIT}};
use crate::ui::main_window::{MainWindowMsg,MainWindowOutput};
use std::collections::HashMap;
pub(super) struct RailItem{button:gtk::ToggleButton,badge:gtk::Label,avatar:adw::Avatar}
pub(super) struct Workspace {
    pub login_scope:uuid::Uuid,
    pub directory:Directory,pub pages:gtk::Stack,pub rail:gtk::Box,
    pub inactive:HashMap<AccountId,Controller<MainWindowModel>>,
    pub generations:HashMap<AccountId,uuid::Uuid>,
    summaries:HashMap<AccountId,crate::ui::main_window::workspace::Summary>,direct:gtk::ToggleButton,direct_badge:gtk::Label,items:HashMap<AccountId,RailItem>,buttons:gtk::Box,status:gtk::Label,
    pending:HashMap<AccountId,(uuid::Uuid,tokio::task::JoinHandle<()>)>,requested:Option<AccountId>,writer:Option<crate::workspace::Writer>,
    media:Option<crate::media::policy::Preferences>,removal:Option<adw::AlertDialog>,pub error:Option<String>,
}
impl Drop for Workspace{fn drop(&mut self){for(_,(_,job))in self.pending.drain(){job.abort();}if let Some(d)=self.removal.take(){d.force_close();}}}
impl Workspace{
    pub fn new(sender:&ComponentSender<RootModel>)->Self{
        let path=Directory::path();let loaded=path.as_ref().map_err(|e|e.to_string()).and_then(|p|Directory::load(p).map_err(|e|e.to_string()));
        let (directory,error)=match loaded{Ok(d)=>(d,None),Err(_)=>(Directory::default(),Some(crate::i18n::tr("Não foi possível abrir a lista de contas salvas. O arquivo foi preservado.").into()))};
        let writer=if error.is_none(){path.ok().map(|p|{let s=sender.clone();crate::workspace::Writer::new(p,move|result|s.input(RootMsg::DirectorySaved(result.map_err(|_|crate::i18n::tr("Não foi possível salvar a lista de contas.").into()))))})}else{None};
        let pages=gtk::Stack::new();pages.set_hhomogeneous(false);pages.set_vhomogeneous(false);pages.set_transition_type(gtk::StackTransitionType::None);
        let rail=gtk::Box::new(gtk::Orientation::Vertical,6);rail.set_width_request(52);rail.add_css_class("papo-rail");rail.set_widget_name("account-rail");
        let direct=gtk::ToggleButton::new();direct.set_widget_name("direct-navigation");direct.set_tooltip_text(Some(crate::i18n::tr("Mensagens diretas (Alt+2)")));direct.update_property(&[gtk::accessible::Property::Label(crate::i18n::tr("Mensagens diretas (Alt+2)"))]);
        let overlay=gtk::Overlay::new();overlay.set_child(Some(&gtk::Image::from_icon_name("papo-chat-message-new-symbolic")));let direct_badge=gtk::Label::new(None);direct_badge.add_css_class("papo-direct-indicator");direct_badge.set_widget_name("direct-unread-indicator");direct_badge.set_halign(gtk::Align::End);direct_badge.set_valign(gtk::Align::Start);direct_badge.set_can_target(false);overlay.add_overlay(&direct_badge);direct.set_child(Some(&overlay));let s=sender.clone();direct.connect_clicked(move |_|s.input(RootMsg::DirectNavigation));rail.append(&direct);rail.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let buttons=gtk::Box::new(gtk::Orientation::Vertical,6);let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&buttons).build();rail.append(&scroll);
        let add=gtk::Button::with_label("+");add.set_widget_name("add-account");add.set_tooltip_text(Some(crate::i18n::tr("Adicionar servidor ou conta")));add.update_property(&[gtk::accessible::Property::Label(crate::i18n::tr("Adicionar servidor ou conta"))]);let s=sender.clone();add.connect_clicked(move |_|s.input(RootMsg::AddAccount));rail.append(&add);
        let status=gtk::Label::new(None);status.set_wrap(true);status.set_max_width_chars(10);status.add_css_class("caption");rail.append(&status);
        Self{login_scope:uuid::Uuid::nil(),directory,pages,rail,inactive:HashMap::new(),generations:HashMap::new(),summaries:HashMap::new(),direct,direct_badge,items:HashMap::new(),buttons,status,pending:HashMap::new(),requested:None,writer,media:None,removal:None,error}
    }
    fn save(&self){if let Some(writer)=&self.writer{writer.submit(self.directory.clone());}}
}
impl RootModel{
    fn active_id(&self)->Option<AccountId>{self.main_controller.as_ref().map(|c|c.model().connection.account.clone())}
    fn account_controller(&self,id:&AccountId)->Option<&Controller<MainWindowModel>>{self.main_controller.as_ref().filter(|c|c.model().connection.account==*id).or_else(||self.workspace.inactive.get(id))}
    fn source_current(&self,id:&AccountId,session:uuid::Uuid)->bool{self.workspace.generations.get(id)==Some(&session)&&self.account_controller(id).is_some()}
    fn rail(&mut self,sender:&ComponentSender<Self>){
        let selected=self.active_id().filter(|_|self.view==View::Main);
        let wanted:std::collections::HashSet<_>=self.workspace.directory.accounts.iter().map(|a|a.id.clone()).collect();
        self.workspace.items.retain(|id,item|{if wanted.contains(id){true}else{self.workspace.buttons.remove(&item.button);false}});
        for a in &self.workspace.directory.accounts{
            if !self.workspace.items.contains_key(&a.id){
                let button=gtk::ToggleButton::new();button.add_css_class("papo-account-button");button.set_widget_name(&format!("account-{}",a.id.namespace()));
                let overlay=gtk::Overlay::new();let avatar=adw::Avatar::new(36,None,true);overlay.set_child(Some(&avatar));
                let badge=gtk::Label::new(None);badge.add_css_class("papo-direct-indicator");badge.set_halign(gtk::Align::End);badge.set_valign(gtk::Align::Start);badge.set_can_target(false);overlay.add_overlay(&badge);button.set_child(Some(&overlay));
                let id=a.id.clone();let s=sender.clone();button.connect_clicked(move |_|s.input(RootMsg::SelectAccount(id.clone())));
                let open:std::rc::Rc<dyn Fn()>=std::rc::Rc::new({let weak=button.downgrade();let id=a.id.clone();let s=sender.clone();move||{let Some(b)=weak.upgrade()else{return;};let p=gtk::Popover::new();p.set_parent(&b);let remove=gtk::Button::with_label(crate::i18n::tr("Remover conta da lista"));let input=s.clone();let id=id.clone();let popup=p.downgrade();remove.connect_clicked(move |_|{if let Some(p)=popup.upgrade(){p.popdown();}input.input(RootMsg::RemoveAccount(id.clone()));});p.set_child(Some(&remove));p.connect_closed(|p|p.unparent());p.popup();}});
                let gesture=gtk::GestureClick::new();gesture.set_button(3);let show=open.clone();gesture.connect_pressed(move |g,_,_,_|{g.set_state(gtk::EventSequenceState::Claimed);show();});button.add_controller(gesture);
                let keys=gtk::EventControllerKey::new();keys.connect_key_pressed(move |_,key,_,mods|{if key==gtk::gdk::Key::Menu||key==gtk::gdk::Key::F10&&crate::platform::shortcuts::modifiers(mods)==gtk::gdk::ModifierType::SHIFT_MASK{open();gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});button.add_controller(keys);
                self.workspace.buttons.append(&button);self.workspace.items.insert(a.id.clone(),RailItem{button,badge,avatar});
            }
            let icon=self.account_controller(&a.id).and_then(|c|c.model().workspace_icon());
            let item=&self.workspace.items[&a.id];item.avatar.set_custom_image(icon.as_ref());let summary=self.workspace.summaries.get(&a.id);let pending=self.workspace.pending.contains_key(&a.id);let live=self.account_controller(&a.id).is_some();
            let state=if pending{crate::i18n::tr("Conectando…")}else if summary.is_some_and(|s|s.calling){crate::i18n::tr("Em chamada")}else if summary.is_some_and(|s|s.connected){crate::i18n::tr("Conectado")}else if live{crate::i18n::tr("Reconectando…")}else{crate::i18n::tr("Entrar")};
            let title=crate::i18n::fmt("{name}\n@{user} · {server}\n{status}",&[("name",a.name.clone()),("user",a.username.clone()),("server",a.id.server().into()),("status",state.into())]);
            item.button.set_tooltip_text(Some(&title));item.button.update_property(&[gtk::accessible::Property::Label(&title)]);item.button.set_active(selected.as_ref()==Some(&a.id));item.avatar.set_text(Some(&a.name));
            let count=summary.map_or(0,|s|s.count);let unread=summary.is_some_and(|s|s.unread);item.badge.set_visible(unread);item.badge.set_text(&if count==0{"•".into()}else if count>99{"99+".into()}else{count.to_string()});
            if unread{item.button.add_css_class("papo-account-unread");}else{item.button.remove_css_class("papo-account-unread");}
            if live&&!summary.is_some_and(|s|s.connected){item.button.add_css_class("papo-account-offline");}else{item.button.remove_css_class("papo-account-offline");}
            if summary.is_some_and(|s|s.calling){item.button.add_css_class("papo-account-call");}else{item.button.remove_css_class("papo-account-call");}
        }
        let summary=selected.as_ref().and_then(|id|self.workspace.summaries.get(id));self.workspace.direct.set_sensitive(self.main_controller.is_some());self.workspace.direct.set_active(summary.is_some_and(|s|s.direct));let count=summary.map_or(0,|s|s.direct_count);self.workspace.direct_badge.set_visible(count>0);self.workspace.direct_badge.set_text(&if count>99{"99+".into()}else{count.to_string()});
        self.workspace.status.set_text(if self.workspace.error.is_some(){"!"}else{""});self.workspace.status.set_tooltip_text(self.workspace.error.as_deref());self.workspace.status.update_property(&[gtk::accessible::Property::Label(self.workspace.error.as_deref().unwrap_or(""))]);self.workspace.status.set_visible(self.workspace.error.is_some());
    }
    fn activate_account(&mut self,id:&AccountId){
        if self.view==View::Login{self.workspace.login_scope=uuid::Uuid::new_v4();self.login_controller.emit(LoginMsg::Prepare{scope:self.workspace.login_scope,server:None,user:None,remember:false});}
        if self.active_id().as_ref()!=Some(id){let Some(next)=self.workspace.inactive.remove(id)else{return;};
            if let Some(previous)=self.main_controller.take(){if let Some(saved)=self.workspace.directory.accounts.iter_mut().find(|a|a.id==previous.model().connection.account){saved.conversation=previous.model().active_channel_id.or(saved.conversation);}previous.emit(MainWindowMsg::Workspace{active:false,restore:None});let old=previous.model().connection.account.clone();self.workspace.inactive.insert(old,previous);}self.main_controller=Some(next);
        }
        let c=self.main_controller.as_ref().unwrap();c.emit(MainWindowMsg::Workspace{active:true,restore:None});self.current_user=Some(c.model().current_user.clone());self.api_client=Some(c.model().connection.api.clone());self.workspace.pages.set_visible_child_name(&id.namespace());self.view=View::Main;
        self.workspace.directory.selected=Some(id.clone());self.workspace.requested=Some(id.clone());self.workspace.save();
    }
    pub(super) fn attach_account(&mut self,whoami:WhoamiResponse,client:ApiClient,remember:bool,activate:bool,sender:&ComponentSender<Self>){
        let id=AccountId::new(&client,whoami.id);let existing=self.account_controller(&id).is_some();
        let live=self.workspace.inactive.len()+usize::from(self.main_controller.is_some());
        if !existing&&live>=LIVE_LIMIT{self.workspace.error=Some(crate::i18n::tr("Limite de oito contas conectadas. Desconecte uma conta antes de adicionar outra.").into());return;}
        let saved=self.workspace.directory.accounts.iter().find(|a|a.id==id).cloned();let restore=saved.as_ref().and_then(|a|a.conversation);
        let account=Saved{id:id.clone(),username:whoami.username.clone(),name:saved.as_ref().map_or_else(||client.base_url().into(),|a|a.name.clone()),remember,conversation:restore};
        if self.workspace.directory.upsert(account).is_err(){self.workspace.error=Some(crate::i18n::tr("A lista de contas salvas está cheia.").into());return;}
        let old=if self.active_id().as_ref()==Some(&id){self.main_controller.take()}else{self.workspace.inactive.remove(&id)};if let Some(old)=old{self.workspace.pages.remove(old.widget());}
        let previous_scheme=adw::StyleManager::default().color_scheme();
        let owner=id.clone();let session=uuid::Uuid::new_v4();self.workspace.generations.insert(id.clone(),session);let controller=MainWindowModel::builder().launch(MainWindowInit{current_user:whoami,api_client:client}).forward(sender.input_sender(),move|output|RootMsg::AccountOutput{id:owner.clone(),session,output});
        if !activate{adw::StyleManager::default().set_color_scheme(previous_scheme);}
        controller.emit(MainWindowMsg::Workspace{active:activate,restore});self.workspace.pages.add_named(controller.widget(),Some(&id.namespace()));
        #[cfg(windows)]if let Some(ticket)=self.native_activation{controller.emit(MainWindowMsg::Notification(crate::ui::main_window::notifications::NoticeMsg::NativeTicket(ticket)));}
        self.workspace.inactive.insert(id.clone(),controller);if self.workspace.writer.is_some(){self.workspace.error=None;}
        if activate{self.activate_account(&id);}self.workspace.save();self.rail(sender);
    }
    fn resume_account(&mut self,id:AccountId,sender:&ComponentSender<Self>){
        if self.workspace.pending.contains_key(&id){return;}
        let Some(saved)=self.workspace.directory.accounts.iter().find(|a|a.id==id).cloned()else{return;};
        if self.workspace.inactive.len()+usize::from(self.main_controller.is_some())+self.workspace.pending.len()>=LIVE_LIMIT{self.workspace.error=Some(crate::i18n::tr("Limite de oito contas conectadas. Desconecte uma conta antes de adicionar outra.").into());return;}
        let ticket=uuid::Uuid::new_v4();let s=sender.clone();let owner=id.clone();let job=tokio::spawn(async move{
            static RESUMES:tokio::sync::Semaphore=tokio::sync::Semaphore::const_new(2);let _permit=RESUMES.acquire().await;
            let result=async{let api=ApiClient::new(owner.server())?;match crate::session::resume(&api,&saved.username).await?{Some(profile)if profile.id==owner.user()=>Ok(Some((profile,api))),Some(_)=>anyhow::bail!("saved identity mismatch"),None=>Ok(None)}}.await;
            s.input(RootMsg::AccountResumed{id:owner,ticket,result});
        });self.workspace.pending.insert(id,(ticket,job));
    }
    fn prepare_login(&mut self,saved:Option<&Saved>){self.workspace.login_scope=uuid::Uuid::new_v4();self.login_controller.emit(LoginMsg::Prepare{scope:self.workspace.login_scope,server:saved.map(|a|a.id.server().to_owned()),user:saved.map(|a|a.username.clone()),remember:saved.is_some_and(|a|a.remember)});if let Some(c)=&self.main_controller{c.emit(MainWindowMsg::Workspace{active:false,restore:None});}self.view=View::Login;}
    fn end_account(&mut self,id:&AccountId,expired:bool){
        let active=self.active_id().as_ref()==Some(id);let old=if active{self.main_controller.take()}else{self.workspace.inactive.remove(id)};
        if let Some(c)=old{self.workspace.pages.remove(c.widget());let api=c.model().connection.api.clone();let user=c.model().current_user.username.clone();
            #[cfg(windows)]crate::platform::notifications::forget(crate::platform::activation::Account{server:api.base_url().into(),user:id.user(),session:api.session_owner()});
            tokio::spawn(async move{let _=crate::session::forget(&api,&user).await;if !expired{let _=api.logout().await;}});
        }
        self.workspace.generations.remove(id);self.workspace.summaries.remove(id);if let Some(a)=self.workspace.directory.accounts.iter_mut().find(|a|&a.id==id){a.remember=false;}
        if active{self.current_user=None;self.api_client=None;let fallback=self.workspace.directory.accounts.iter().find(|a|self.workspace.inactive.contains_key(&a.id)).map(|a|a.id.clone());if let Some(next)=fallback{self.activate_account(&next);}else{let saved=self.workspace.directory.accounts.iter().find(|a|&a.id==id).cloned();self.prepare_login(saved.as_ref());if expired{self.login_controller.emit(LoginMsg::AuthFailed(crate::i18n::tr("Sua sessão terminou. Entre novamente.").into()));}}}
        self.workspace.save();
    }
    pub(super) fn workspace_message(&mut self,message:RootMsg,sender:&ComponentSender<Self>,root:&adw::ApplicationWindow){match message{
        RootMsg::WorkspaceStart=>{self.workspace.requested=self.workspace.directory.selected.clone();let saved=self.workspace.directory.accounts.iter().find(|a|Some(&a.id)==self.workspace.directory.selected.as_ref()).cloned();if !self.workspace.directory.accounts.is_empty(){self.prepare_login(saved.as_ref());}let mut ids:Vec<_>=self.workspace.directory.accounts.iter().filter(|a|a.remember).map(|a|a.id.clone()).collect();ids.sort_by_key(|id|Some(id)!=self.workspace.directory.selected.as_ref());for id in ids.into_iter().take(LIVE_LIMIT){self.resume_account(id,sender);}self.rail(sender);},
        RootMsg::DirectNavigation=>{if let Some(id)=self.active_id(){if self.view==View::Login{self.activate_account(&id);}let next=!self.workspace.summaries.get(&id).is_some_and(|s|s.direct);if let Some(summary)=self.workspace.summaries.get_mut(&id){summary.direct=next;}if let Some(c)=self.account_controller(&id){c.emit(MainWindowMsg::SidebarMode(next));}self.rail(sender);}},
        RootMsg::AddAccount=>{self.workspace.requested=None;self.prepare_login(None);self.rail(sender);},
        RootMsg::CancelAccountLogin=>{if let Some(id)=self.active_id(){self.workspace.login_scope=uuid::Uuid::new_v4();self.login_controller.emit(LoginMsg::Prepare{scope:self.workspace.login_scope,server:None,user:None,remember:false});self.activate_account(&id);}self.rail(sender);},
        RootMsg::SelectAccount(id)=>{self.workspace.requested=Some(id.clone());if self.account_controller(&id).is_some(){self.activate_account(&id);}else if let Some(saved)=self.workspace.directory.accounts.iter().find(|a|a.id==id).cloned(){if saved.remember{self.resume_account(id,sender);}else{self.prepare_login(Some(&saved));}}self.rail(sender);},
        RootMsg::AccountResumed{id,ticket,result}=>{if !self.workspace.pending.get(&id).is_some_and(|(t,_)|*t==ticket){return;}self.workspace.pending.remove(&id);let activate=self.workspace.requested.as_ref()==Some(&id)||self.workspace.requested.is_none()&&self.main_controller.is_none();match result{Ok(Some((profile,api)))=>self.attach_account(profile,api,true,activate,sender),Ok(None)=>{if let Some(a)=self.workspace.directory.accounts.iter_mut().find(|a|a.id==id){a.remember=false;}self.workspace.save();if activate{let saved=self.workspace.directory.accounts.iter().find(|a|a.id==id).cloned();self.prepare_login(saved.as_ref());}},Err(_)=>{self.workspace.error=Some(crate::i18n::tr("Não foi possível conectar esta conta. As outras conexões continuam disponíveis.").into());if activate{let saved=self.workspace.directory.accounts.iter().find(|a|a.id==id).cloned();self.prepare_login(saved.as_ref());}}}self.rail(sender);},
        RootMsg::AccountOutput{id,session,output}=>{if !self.source_current(&id,session){return;}match output{
            MainWindowOutput::Logout=>self.end_account(&id,false),MainWindowOutput::SessionExpired=>self.end_account(&id,true),
            #[cfg(any(target_os="linux",windows,test))]
            MainWindowOutput::Activate=>self.activate_account(&id),
            MainWindowOutput::DeviceMedia(media)=>{if self.workspace.media!=Some(media){self.workspace.media=Some(media);for c in self.main_controller.iter().chain(self.workspace.inactive.values()){c.emit(MainWindowMsg::MediaPreferences(media));}}},
            MainWindowOutput::Workspace(summary)=>{let mut changed=false;if let Some(a)=self.workspace.directory.accounts.iter_mut().find(|a|a.id==id){let name: String=summary.name.chars().take(256).collect();changed=a.name!=name||a.conversation!=summary.conversation;a.name=name;if summary.conversation.is_some(){a.conversation=summary.conversation;}}self.workspace.summaries.insert(id,summary);if changed{self.workspace.save();}},
        }self.rail(sender);},
        RootMsg::RemoveAccount(id)=>{let Some(account)=self.workspace.directory.accounts.iter().find(|a|a.id==id)else{return;};let body=crate::i18n::fmt("A conta @{user} em {server} será desconectada neste dispositivo. As outras contas não serão afetadas.",&[("user",account.username.clone()),("server",account.name.clone())]);let dialog=adw::AlertDialog::builder().heading(crate::i18n::tr("Remover conta da lista?")).body(body).build();dialog.add_responses(&[("cancel",crate::i18n::tr("Cancelar")),("remove",crate::i18n::tr("Remover"))]);dialog.set_response_appearance("remove",adw::ResponseAppearance::Destructive);dialog.set_close_response("cancel");let s=sender.clone();dialog.connect_response(None,move|_,response|if response=="remove"{s.input(RootMsg::RemoveAccountConfirmed(id.clone()));});dialog.present(Some(root));self.workspace.removal=Some(dialog);},
        RootMsg::RemoveAccountConfirmed(id)=>{if let Some((_,job))=self.workspace.pending.remove(&id){job.abort();}self.end_account(&id,false);if let Some(a)=self.workspace.directory.accounts.iter().find(|a|a.id==id).cloned(){let shared=self.workspace.directory.accounts.iter().any(|other|other.id!=id&&other.id.server()==a.id.server()&&other.username==a.username);if !shared{tokio::spawn(async move{if let Ok(api)=ApiClient::new(a.id.server()){let _=crate::session::forget_account(&api,&a.username).await;}});}}self.workspace.directory.remove(&id);if self.workspace.requested.as_ref()==Some(&id){self.workspace.requested=self.active_id();}self.workspace.save();self.rail(sender);},
        RootMsg::DirectorySaved(result)=>{if let Err(error)=result{self.workspace.error=Some(error);self.rail(sender);}},
        _=>unreachable!()
    }}
}

#[cfg(test)]mod tests;
#[cfg(test)]pub(super) use tests::{exercise,accounts_restart};
