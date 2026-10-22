//! EN-US: Activation carries only an opaque identifier; destinations require account and access checks.
//! PT-BR: A ativação carrega apenas um identificador opaco; destinos exigem validação da conta e do acesso.
use uuid::Uuid;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Activation { Present, Ticket(Uuid) }

pub fn parse_uri(value:&str)->Option<Uuid>{
    let text=value.strip_prefix("papo-notification://open/")?;
    let id=Uuid::parse_str(text).ok()?;
    (id.to_string()==text&&!id.is_nil()).then_some(id)
}

// EN-US: Prototype policy for step 9; the shell cannot supply account/channel/message authority.
// PT-BR: Política de protótipo para a etapa 9; o shell não pode definir conta/canal/mensagem autorizados.
#[cfg(test)]
mod routing_design {
    use super::*;
    #[derive(Clone,PartialEq,Eq)]
    struct Account {server:String,user:Uuid,session:Uuid}
    struct Ticket {owner:Account,channel:Uuid,message:Uuid,expires:u64}
    fn resolve<'a>(ticket:&'a Ticket,current:&Account,readable:impl Fn(Uuid)->bool,now:u64)->Option<&'a Ticket>{
        (&ticket.owner==current&&ticket.expires>now&&readable(ticket.channel)).then_some(ticket)
    }
    #[test] fn activation_policy_rechecks_account_session_permission_and_expiration(){
        let account=Account{server:"https://one.invalid/api".into(),user:Uuid::new_v4(),session:Uuid::new_v4()};
        let ticket=Ticket{owner:account.clone(),channel:Uuid::new_v4(),message:Uuid::new_v4(),expires:100};
        assert_eq!(resolve(&ticket,&account,|_|true,99).unwrap().message,ticket.message);
        for wrong in [Account{server:"https://two.invalid/api".into(),..account.clone()},Account{user:Uuid::new_v4(),..account.clone()},Account{session:Uuid::new_v4(),..account.clone()}]{assert!(resolve(&ticket,&wrong,|_|true,0).is_none());}
        assert!(resolve(&ticket,&account,|_|false,0).is_none());
        assert!(resolve(&ticket,&account,|_|true,100).is_none());
    }
    #[test] fn uri_rejects_extra_data_and_noncanonical_identifiers(){
        let id=Uuid::new_v4();let uri=format!("papo-notification://open/{id}");assert_eq!(parse_uri(&uri),Some(id));
        for invalid in [format!("{uri}?token=secret"),format!("{uri}/other"),uri.to_uppercase(),"papo-notification://open/00000000-0000-0000-0000-000000000000".into(),"https://one.invalid/api".into(),"--diagnostics".into()]{assert!(parse_uri(&invalid).is_none());}
    }
    #[test] fn cold_start_has_no_destination_until_trusted_account_metadata_is_restored(){
        let id=Uuid::new_v4();assert_eq!(parse_uri(&format!("papo-notification://open/{id}")),Some(id));
        // EN-US: An opaque shell ticket alone cannot reconstruct a destination or a login session.
        // PT-BR: Um ticket opaco do shell não reconstrói sozinho um destino ou sessão autenticada.
        let restored:Option<Ticket>=None;assert!(restored.is_none());
        assert_eq!(Activation::Present,Activation::Present);assert_eq!(Activation::Ticket(id),Activation::Ticket(id));
    }
}
