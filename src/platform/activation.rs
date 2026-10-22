//! EN-US: Opaque shell tickets carry no authority; only account-bound local metadata can select a destination.
//! PT-BR: Tickets opacos do shell não têm autoridade; só metadados locais vinculados à conta podem definir destinos.
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Activation { Present, Ticket(Uuid) }
pub fn parse_uri(value:&str)->Option<Uuid>{
    let text=value.strip_prefix("papo-notification://open/")?;let id=Uuid::parse_str(text).ok()?;
    (id.to_string()==text&&!id.is_nil()).then_some(id)
}
#[derive(Clone,PartialEq,Eq)]
pub struct Account {pub server:String,pub user:Uuid,pub session:Uuid}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Destination {pub channel:Uuid,pub message:Uuid}
struct Ticket {owner:Account,destination:Destination,expires:u64}
#[derive(Default)]
pub struct Router {account:Option<Account>,tickets:HashMap<Uuid,Ticket>}
impl Router {
    pub fn account(&mut self,account:Option<Account>){if self.account!=account{self.tickets.clear();self.account=account;}}
    pub fn issue(&mut self,destination:Destination,expires:u64)->Option<Uuid>{
        if self.tickets.len()>=64{return None;}
        let id=Uuid::new_v4();self.tickets.insert(id,Ticket{owner:self.account.clone()?,destination,expires});Some(id)
    }
    pub fn resolve(&mut self,id:Uuid,now:u64,readable:impl Fn(Uuid)->bool)->Option<Destination>{
        let ticket=self.tickets.remove(&id)?;
        (self.account.as_ref()==Some(&ticket.owner)&&ticket.expires>now&&readable(ticket.destination.channel)).then_some(ticket.destination)
    }
}
// EN-US: An offline CLI prototype exercises the same resolver used by the root window; no real session is touched.
// PT-BR: Um protótipo CLI offline testa o mesmo resolvedor da janela raiz; não acessa sessões reais.
pub fn check_policy()->anyhow::Result<()>{
    let owner=Account{server:"https://fixture.invalid/api".into(),user:Uuid::new_v4(),session:Uuid::new_v4()};
    let destination=Destination{channel:Uuid::new_v4(),message:Uuid::new_v4()};let mut router=Router::default();
    anyhow::ensure!(router.issue(destination,100).is_none(),"Unauthenticated destination issued");
    router.account(Some(owner.clone()));let id=router.issue(destination,100).unwrap();anyhow::ensure!(router.resolve(id,99,|_|true)==Some(destination),"Matching account rejected");
    for wrong in [Account{server:"https://other.invalid/api".into(),..owner.clone()},Account{user:Uuid::new_v4(),..owner.clone()},Account{session:Uuid::new_v4(),..owner.clone()}]{router.account(Some(owner.clone()));let id=router.issue(destination,100).unwrap();router.account(Some(wrong));anyhow::ensure!(router.resolve(id,0,|_|true).is_none(),"Cross-account activation accepted");}
    router.account(Some(owner));let id=router.issue(destination,100).unwrap();anyhow::ensure!(router.resolve(id,100,|_|true).is_none(),"Expired activation accepted");
    let id=router.issue(destination,100).unwrap();anyhow::ensure!(router.resolve(id,0,|_|false).is_none(),"Unreadable channel accepted");
    let id=router.issue(destination,100).unwrap();router.account(None);anyhow::ensure!(router.resolve(id,0,|_|true).is_none(),"Logged-out activation accepted");
    Ok(())
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn account_permission_logout_and_expiration_policy(){check_policy().unwrap();}
    #[test]fn cold_start_unknown_ticket_has_no_trusted_destination(){let id=Uuid::new_v4();let mut router=Router::default();assert_eq!(parse_uri(&format!("papo-notification://open/{id}")),Some(id));assert!(router.resolve(id,0,|_|true).is_none());}
    #[test]fn shell_uri_rejects_extra_data_and_noncanonical_identifiers(){
        let id=Uuid::new_v4();let uri=format!("papo-notification://open/{id}");assert_eq!(parse_uri(&uri),Some(id));
        for invalid in [format!("{uri}?token=secret"),format!("{uri}/other"),uri.to_uppercase(),"papo-notification://open/00000000-0000-0000-0000-000000000000".into(),"https://fixture.invalid".into()]{assert!(parse_uri(&invalid).is_none());}
        assert_eq!(Activation::Present,Activation::Present);assert_eq!(Activation::Ticket(id),Activation::Ticket(id));
    }
}
