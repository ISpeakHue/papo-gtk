//! EN-US: Native desktop alerts are pending; the shared in-app inbox and unread badges remain active.
//! PT-BR: Alertas nativos do desktop estão pendentes; a caixa interna e os indicadores de não lidas continuam ativos.
use super::*;
#[derive(Default)]pub(super) struct Desktop;
impl Desktop {
    pub fn available(&self)->bool{false}
    pub fn initialize(&mut self,_sender:relm4::Sender<MainWindowMsg>){}
    pub fn send(&mut self,_notice:&state::Notice,_observed:Option<crate::models::MessageObservation>,_body:String,_sound:bool){}
    pub fn revoke(&mut self,_allowed:&HashSet<Uuid>,_readable:&HashSet<Uuid>){}
}
