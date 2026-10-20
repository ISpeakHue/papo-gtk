//! EN-US: Desktop notifications using the freedesktop.org protocol.
//! PT-BR: Notificações do desktop integradas pelo protocolo freedesktop.org.
//! https://specifications.freedesktop.org/notification/latest/protocol.html
use super::*;
use gtk::{gio,glib};
use glib::variant::ToVariant;
use std::{cell::{RefCell,Cell},rc::Rc};
#[derive(Clone)]
struct Target {notice:Uuid,channel:Uuid,message:Uuid,observed:Option<crate::models::MessageObservation>,token:Option<String>,closed:bool,at:std::time::Instant}
type Targets=Rc<RefCell<HashMap<u32,Target>>>;
// Some servers close a notification before emitting its default action. Keep
// its destination briefly, including when focus/read synchronization closes it.
fn expire(targets:&Targets,id:u32){
    let Some(at)=targets.borrow_mut().get_mut(&id).map(|t|{t.closed=true;t.at})else{return;};
    let targets=Rc::downgrade(targets);glib::timeout_add_local_once(std::time::Duration::from_secs(5),move ||{
        if let Some(targets)=targets.upgrade(){let mut targets=targets.borrow_mut();if targets.get(&id).is_some_and(|t|t.closed&&t.at==at){targets.remove(&id);}}
    });
}
pub(super) struct Desktop {
    proxy:Option<gio::DBusProxy>, pub initializing:bool, attempted:bool,
    targets:Targets, allowed:Rc<RefCell<HashSet<Uuid>>>, alive:Rc<Cell<bool>>,
    pub jobs:Vec<(bool,glib::JoinHandle<()>)>,
}
impl Default for Desktop {fn default()->Self{Self{proxy:None,initializing:false,attempted:false,targets:Default::default(),allowed:Default::default(),alive:Rc::new(Cell::new(true)),jobs:vec![]}}}
impl Desktop {
    pub fn available(&self)->bool{self.proxy.is_some()}
    pub fn initialize(&mut self,sender:relm4::Sender<MainWindowMsg>){
        if self.proxy.is_some()||self.initializing||self.attempted{return;}self.initializing=true;self.attempted=true;
        self.jobs.push((true,glib::spawn_future_local(async move{
            let result=gio::DBusProxy::for_bus_future(gio::BusType::Session,gio::DBusProxyFlags::DO_NOT_AUTO_START,None,"org.freedesktop.Notifications","/org/freedesktop/Notifications","org.freedesktop.Notifications").await;
            let _=sender.send(MainWindowMsg::Notification(NoticeMsg::DesktopReady(result)));
        })));
    }
    pub fn ready(&mut self,result:Result<gio::DBusProxy,glib::Error>,sender:relm4::Sender<MainWindowMsg>){
        self.initializing=false;
        let Ok(proxy)=result else{return;};let targets=self.targets.clone();let alive=self.alive.clone();
        proxy.connect_local("g-signal",false,move |values|{
            if !alive.get(){return None;}
            let name=values[2].get::<String>().ok()?;let args=values[3].get::<glib::Variant>().ok()?;
            if name=="ActivationToken"{if let Some((id,token))=args.get::<(u32,String)>(){if token.len()<=4096{if let Some(target)=targets.borrow_mut().get_mut(&id){target.token=Some(token);}}}}
            else if name=="ActionInvoked"{if let Some((id,action))=args.get::<(u32,String)>(){if action=="default"{
                if let Some(target)=targets.borrow_mut().remove(&id){let _=sender.send(MainWindowMsg::Notification(NoticeMsg::DesktopAction{notice:target.notice,channel:target.channel,message:target.message,observed:target.observed,token:target.token}));}
            }}}
            else if name=="NotificationClosed"{if let Some((id,_))=args.get::<(u32,u32)>(){expire(&targets,id);}}
            None
        });self.proxy=Some(proxy);
    }
    pub fn send(&mut self,notice:&state::Notice,observed:Option<crate::models::MessageObservation>,body:String,sound:bool){
        self.jobs.retain(|(_,job)|!job.source().is_destroyed());
        let Some(channel)=notice.channel_id else{return;};let id=notice.id;
        let target=Target{notice:id,channel,message:notice.message_id,observed,token:None,closed:false,at:std::time::Instant::now()};
        let Some(proxy)=self.proxy.clone()else{return;};let targets=self.targets.clone();let allowed=self.allowed.clone();let alive=self.alive.clone();
        let mut hints=HashMap::new();hints.insert("desktop-entry".to_owned(),crate::app::APP_ID.to_variant());hints.insert("suppress-sound".to_owned(),(!sound).to_variant());if sound{hints.insert("sound-name".to_owned(),"message-new-instant".to_variant());}
        let escaped=glib::markup_escape_text(&body).to_string();
        let params=("Papo",0u32,crate::app::APP_ID,"Nova mensagem no Papo",escaped,vec!["default","Abrir"],hints,-1i32).to_variant();
        self.jobs.push((false,glib::spawn_future_local(async move{
            if !alive.get()||!allowed.borrow().contains(&id){return;}
            if let Ok(result)=proxy.call_future("Notify",Some(&params),gio::DBusCallFlags::NONE,5000).await{
                if let Some((remote,))=result.get::<(u32,)>(){
                    if alive.get()&&allowed.borrow().contains(&id){
                        let mut targets=targets.borrow_mut();
                        if targets.len()>=500{if let Some(old)=targets.iter().min_by_key(|(_,t)|t.at).map(|(id,_)|*id){targets.remove(&old);}}
                        targets.insert(remote,target);
                    }else{let _=proxy.call_future("CloseNotification",Some(&(remote,).to_variant()),gio::DBusCallFlags::NONE,3000).await;}
                }
            }
        })));
    }
    pub fn revoke(&mut self,allowed:&HashSet<Uuid>,readable:&HashSet<Uuid>){
        *self.allowed.borrow_mut()=allowed.clone();
        let Some(proxy)=&self.proxy else{return;};let revoked:Vec<_>=self.targets.borrow().iter().filter(|(_,t)|!t.closed&&(!allowed.contains(&t.notice)||!readable.contains(&t.channel))).map(|(id,_)|*id).collect();
        for id in revoked {
            let permitted=self.targets.borrow().get(&id).is_some_and(|t|readable.contains(&t.channel));
            if permitted{expire(&self.targets,id);}else{self.targets.borrow_mut().remove(&id);}
            proxy.call("CloseNotification",Some(&(id,).to_variant()),gio::DBusCallFlags::NONE,3000,None::<&gio::Cancellable>,|_|{});
        }
        self.targets.borrow_mut().retain(|_,t|readable.contains(&t.channel));
    }
}
impl Drop for Desktop {fn drop(&mut self){self.alive.set(false);for (initialization,job) in self.jobs.drain(..){if initialization{job.abort();}}self.revoke(&Default::default(),&Default::default());}}

#[cfg(test)]
pub(super) fn exercise(main:&Controller<MainWindowModel>,context:&glib::MainContext,notice:Uuid){
    use crate::ui::chat::actions::tests::{until,pump};
    use std::io::BufRead;
    // A private bus isolates the test from the user's desktop notification service.
    struct Bus(std::process::Child);impl Drop for Bus{fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();}}
    let mut bus=Bus(std::process::Command::new("dbus-daemon").args(["--session","--nofork","--print-address=1"]).stdout(std::process::Stdio::piped()).spawn().unwrap());
    let mut address=String::new();std::io::BufReader::new(bus.0.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let flags=gio::DBusConnectionFlags::AUTHENTICATION_CLIENT|gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION;
    let service=gio::DBusConnection::for_address_sync(address.trim(),flags,None,None::<&gio::Cancellable>).unwrap();
    let client=gio::DBusConnection::for_address_sync(address.trim(),flags,None,None::<&gio::Cancellable>).unwrap();
    let xml=r#"<node><interface name="org.freedesktop.Notifications">
    <method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
    <method name="CloseNotification"><arg type="u" direction="in"/></method>
    <signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
    <signal name="ActivationToken"><arg type="u"/><arg type="s"/></signal>
    <signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal></interface></node>"#;
    let info=gio::DBusNodeInfo::for_xml(xml).unwrap().lookup_interface("org.freedesktop.Notifications").unwrap();
    let calls:Rc<RefCell<Vec<(String,glib::Variant)>>>=Default::default();let captured=calls.clone();
    let registration=service.register_object("/org/freedesktop/Notifications",&info).method_call(move|_,_,_,_,method,params,invocation|{captured.borrow_mut().push((method.to_owned(),params));if method=="Notify"{invocation.return_value(Some(&(7u32,).to_variant()));}else{invocation.return_value(None);}}).build().unwrap();
    let proxy=context.block_on(gio::DBusProxy::new_future(&client,gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,Some(&info),service.unique_name().as_deref(),"/org/freedesktop/Notifications","org.freedesktop.Notifications")).unwrap();
    let mut desktop=Desktop::default();desktop.ready(Ok(proxy),main.sender().clone());desktop.revoke(&[notice].into_iter().collect(),&[main.model().notifications.inbox.rows[&notice].channel_id.unwrap()].into_iter().collect());desktop.send(&main.model().notifications.inbox.rows[&notice],None,"<b>private & content</b>".into(),false);
    until(context,||desktop.targets.borrow().contains_key(&7));
    let(_,params)=calls.borrow()[0].clone();assert_eq!(params.type_().as_str(),"(susssasa{sv}i)");assert_eq!(params.child_value(4).get::<String>().unwrap(),"&lt;b&gt;private &amp; content&lt;/b&gt;");let hints=params.child_value(6).get::<HashMap<String,glib::Variant>>().unwrap();assert_eq!(hints["suppress-sound"].get::<bool>(),Some(true));assert_eq!(params.child_value(5).get::<Vec<String>>().unwrap(),["default","Abrir"]);
    let row=main.model().notifications.inbox.rows[&notice].clone();let channel=row.channel_id.unwrap();let target=row.message_id;
    // Notification paging must not discard the desktop activation destination.
    main.state().get_mut().model.notifications.inbox.rows.remove(&notice);
    service.emit_signal(None,"/org/freedesktop/Notifications","org.freedesktop.Notifications","ActivationToken",Some(&(7u32,"private-test-token").to_variant())).unwrap();
    until(context,||desktop.targets.borrow().get(&7).is_some_and(|t|t.token.as_deref()==Some("private-test-token")));
    service.emit_signal(None,"/org/freedesktop/Notifications","org.freedesktop.Notifications","NotificationClosed",Some(&(7u32,2u32).to_variant())).unwrap();
    until(context,||desktop.targets.borrow().get(&7).is_some_and(|t|t.closed));
    main.emit(MainWindowMsg::Navigate{channel_id:channel,message_id:"12345678-1234-4234-8234-123456789abe".parse().unwrap()});pump(context);
    service.emit_signal(None,"/org/freedesktop/Notifications","org.freedesktop.Notifications","ActionInvoked",Some(&(7u32,"default").to_variant())).unwrap();
    until(context,||crate::ui::chat::actions::tests::descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("message-{target}")&&w.has_css_class("papo-message-highlight")));
    assert!(!desktop.targets.borrow().contains_key(&7),"a click activates only once");
    main.state().get_mut().model.notifications.inbox.rows.insert(notice,row.clone());
    desktop.send(&row,None,"read-close race".into(),false);until(context,||desktop.targets.borrow().contains_key(&7));
    desktop.revoke(&Default::default(),&[channel].into_iter().collect());until(context,||calls.borrow().iter().any(|(method,_)|method=="CloseNotification"));
    assert!(desktop.targets.borrow().get(&7).is_some_and(|t|t.closed),"read synchronization leaves a short grace period for an already queued click");
    desktop.revoke(&Default::default(),&Default::default());assert!(desktop.targets.borrow().is_empty(),"access revocation removes destinations immediately");
    drop(desktop);service.unregister_object(registration).unwrap();let _=client.close_sync(None::<&gio::Cancellable>);let _=service.close_sync(None::<&gio::Cancellable>);drop(bus);
}
