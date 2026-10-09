//! https://specifications.freedesktop.org/notification/latest/protocol.html
use super::*;
use gtk::{gio,glib};
use glib::variant::ToVariant;
use std::{cell::{RefCell,Cell},rc::Rc};
pub(super) struct Desktop {
    proxy:Option<gio::DBusProxy>, pub initializing:bool, attempted:bool,
    targets:Rc<RefCell<HashMap<u32,Uuid>>>, allowed:Rc<RefCell<std::collections::HashSet<Uuid>>>, alive:Rc<Cell<bool>>,
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
        let Ok(proxy)=result else{return;};let targets=self.targets.clone();
        proxy.connect_local("g-signal",false,move |values|{
            let name=values[2].get::<String>().ok()?;let args=values[3].get::<glib::Variant>().ok()?;
            if name=="ActionInvoked"{if let Some((id,action))=args.get::<(u32,String)>(){if action=="default"{if let Some(notice)=targets.borrow().get(&id).copied(){let _=sender.send(MainWindowMsg::Notification(NoticeMsg::OpenMessage(notice)));}}}}
            else if name=="NotificationClosed"{if let Some((id,_))=args.get::<(u32,u32)>(){targets.borrow_mut().remove(&id);}}
            None
        });self.proxy=Some(proxy);
    }
    pub fn send(&mut self,id:Uuid,body:String,sound:bool){
        self.jobs.retain(|(_,job)|!job.source().is_destroyed());
        let Some(proxy)=self.proxy.clone()else{return;};let targets=self.targets.clone();let allowed=self.allowed.clone();let alive=self.alive.clone();
        let mut hints=HashMap::new();hints.insert("desktop-entry".to_owned(),crate::app::APP_ID.to_variant());hints.insert("suppress-sound".to_owned(),(!sound).to_variant());if sound{hints.insert("sound-name".to_owned(),"message-new-instant".to_variant());}
        let escaped=glib::markup_escape_text(&body).to_string();
        let params=("Papo",0u32,crate::app::APP_ID,"Nova mensagem no Papo",escaped,vec!["default","Abrir"],hints,-1i32).to_variant();
        self.jobs.push((false,glib::spawn_future_local(async move{
            if !alive.get()||!allowed.borrow().contains(&id){return;}
            if let Ok(result)=proxy.call_future("Notify",Some(&params),gio::DBusCallFlags::NONE,5000).await{
                if let Some((remote,))=result.get::<(u32,)>(){
                    if alive.get()&&allowed.borrow().contains(&id){targets.borrow_mut().insert(remote,id);}
                    else{let _=proxy.call_future("CloseNotification",Some(&(remote,).to_variant()),gio::DBusCallFlags::NONE,3000).await;}
                }
            }
        })));
    }
    pub fn revoke(&mut self,allowed:&std::collections::HashSet<Uuid>){
        *self.allowed.borrow_mut()=allowed.clone();
        let Some(proxy)=&self.proxy else{return;};let revoked:Vec<_>=self.targets.borrow().iter().filter(|(_,id)|!allowed.contains(id)).map(|(id,_)|*id).collect();
        for id in revoked {self.targets.borrow_mut().remove(&id);proxy.call("CloseNotification",Some(&(id,).to_variant()),gio::DBusCallFlags::NONE,3000,None::<&gio::Cancellable>,|_|{});}
    }
}
impl Drop for Desktop {fn drop(&mut self){self.alive.set(false);for (initialization,job) in self.jobs.drain(..){if initialization{job.abort();}}self.revoke(&Default::default());}}

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
    <signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal></interface></node>"#;
    let info=gio::DBusNodeInfo::for_xml(xml).unwrap().lookup_interface("org.freedesktop.Notifications").unwrap();
    let calls:Rc<RefCell<Vec<(String,glib::Variant)>>>=Default::default();let captured=calls.clone();
    let registration=service.register_object("/org/freedesktop/Notifications",&info).method_call(move|_,_,_,_,method,params,invocation|{captured.borrow_mut().push((method.to_owned(),params));if method=="Notify"{invocation.return_value(Some(&(7u32,).to_variant()));}else{invocation.return_value(None);}}).build().unwrap();
    let proxy=context.block_on(gio::DBusProxy::new_future(&client,gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,Some(&info),service.unique_name().as_deref(),"/org/freedesktop/Notifications","org.freedesktop.Notifications")).unwrap();
    let mut desktop=Desktop::default();desktop.ready(Ok(proxy),main.sender().clone());desktop.revoke(&[notice].into_iter().collect());desktop.send(notice,"<b>private & content</b>".into(),false);
    until(context,||desktop.targets.borrow().contains_key(&7));
    let(_,params)=calls.borrow()[0].clone();assert_eq!(params.type_().as_str(),"(susssasa{sv}i)");assert_eq!(params.child_value(4).get::<String>().unwrap(),"&lt;b&gt;private &amp; content&lt;/b&gt;");let hints=params.child_value(6).get::<HashMap<String,glib::Variant>>().unwrap();assert_eq!(hints["suppress-sound"].get::<bool>(),Some(true));assert_eq!(params.child_value(5).get::<Vec<String>>().unwrap(),["default","Abrir"]);
    let target=main.model().notifications.inbox.rows[&notice].message_id;
    main.emit(MainWindowMsg::Navigate{channel_id:main.model().notifications.inbox.rows[&notice].channel_id.unwrap(),message_id:"12345678-1234-4234-8234-123456789abe".parse().unwrap()});pump(context);
    service.emit_signal(None,"/org/freedesktop/Notifications","org.freedesktop.Notifications","ActionInvoked",Some(&(7u32,"default").to_variant())).unwrap();
    until(context,||crate::ui::chat::actions::tests::descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("message-{target}")&&w.has_css_class("papo-message-highlight")));
    pump(context);desktop.revoke(&Default::default());until(context,||calls.borrow().iter().any(|(method,_)|method=="CloseNotification"));
    drop(desktop);service.unregister_object(registration).unwrap();let _=client.close_sync(None::<&gio::Cancellable>);let _=service.close_sync(None::<&gio::Cancellable>);drop(bus);
}
