//! EN-US: Windows toast adapter; delivery policy remains in the shared notification inbox.
//! PT-BR: Adaptador de avisos Windows; a política de entrega permanece na caixa compartilhada.
use super::*;
use crate::platform::{activation::Account,notifications::{Backend,Target}};

#[derive(Default)]pub(super) struct Desktop {
    backend:Option<Backend>,owner:Option<Account>,ready:bool,attempted:bool,
    allowed:HashSet<Uuid>,readable:HashSet<Uuid>,
}
impl Desktop{
    pub fn account(&mut self,owner:Account){if self.owner.as_ref()!=Some(&owner){self.owner=Some(owner);self.backend=crate::platform::notifications::backend();self.ready=false;self.attempted=false;self.allowed.clear();self.readable.clear();}}
    pub fn available(&self)->bool{self.ready}
    pub fn initialize(&mut self,sender:relm4::Sender<MainWindowMsg>){
        if self.attempted{return;}self.attempted=true;
        let Some(backend)=self.backend.clone()else{return;};
        tokio::spawn(async move{let ready=backend.ready().await;let _=sender.send(MainWindowMsg::Notification(NoticeMsg::WindowsReady(ready)));});
    }
    pub fn ready(&mut self,ready:bool){self.ready=ready;if !ready{tracing::warn!("Windows desktop notifications unavailable; check this package's desktop registration");}}
    pub fn send(&mut self,notice:&state::Notice,_observed:Option<crate::models::MessageObservation>,body:String,sound:bool){
        let(Some(backend),Some(owner),Some(channel))=(&self.backend,&self.owner,notice.channel_id)else{return;};
        if self.ready{backend.show(owner.clone(),Target{notice:notice.id,channel,message:notice.message_id},body,sound);}
    }
    pub fn revoke(&mut self,allowed:&HashSet<Uuid>,readable:&HashSet<Uuid>){
        if self.allowed==*allowed&&self.readable==*readable{return;}
        if let(Some(backend),Some(owner))=(&self.backend,&self.owner){if backend.reconcile(owner.clone(),allowed.clone(),readable.clone()){self.allowed=allowed.clone();self.readable=readable.clone();}}
    }
}

#[cfg(test)]impl Desktop{
    pub(super) fn fixture(owner:Account,backend:Backend)->Self{Self{owner:Some(owner),backend:Some(backend),..Default::default()}}
}
