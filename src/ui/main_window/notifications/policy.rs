//! EN-US: Device/account-local alert overrides; inbox and server preferences remain authoritative.
//! PT-BR: Ajustes de avisos locais por dispositivo/conta; caixa e preferências do servidor são preservadas.
use chrono::{DateTime,Utc};
use serde::{Serialize,Deserialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug,Clone,Default,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct Policy {
    pub snooze:Option<DateTime<Utc>>,
    pub channels:BTreeMap<Uuid,DateTime<Utc>>,
    pub messages:Option<bool>,
    pub calls:Option<bool>,
}
impl Policy {
    pub fn allows(&self,channel:Option<Uuid>,busy:bool,now:DateTime<Utc>)->bool {
        !busy&&!self.snooze.is_some_and(|until|until>now)
            &&!channel.and_then(|id|self.channels.get(&id)).is_some_and(|until|*until>now)
    }
    pub fn message_sound(&self,legacy:bool)->bool{self.messages.unwrap_or(legacy)}
    pub fn call_sound(&self,legacy:bool)->bool{self.calls.unwrap_or(legacy)}
    fn path()->anyhow::Result<std::path::PathBuf>{Ok(crate::config::AppConfig::path()?.with_file_name("notification-overrides.json"))}
    fn read(path:&std::path::Path)->anyhow::Result<BTreeMap<String,Self>> {
        match std::fs::File::open(path){
            Ok(file)=>{use std::io::Read;let mut bytes=vec![];file.take((1<<20)+1).read_to_end(&mut bytes)?;anyhow::ensure!(bytes.len()<=1<<20,"notification preferences exceed limit");Ok(serde_json::from_slice(&bytes)?)},
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(BTreeMap::new()),Err(e)=>Err(e.into())
        }
    }
    pub fn load(owner:&crate::account::AccountId)->Self {
        #[cfg(test)]if std::env::var_os("PAPO_TEST_CONFIG_DIR").is_none(){return Self::default();}
        Self::path().and_then(|p|Self::read(&p)).ok().and_then(|mut all|all.remove(&owner.namespace())).unwrap_or_default()
    }
    pub fn save(&self,owner:&crate::account::AccountId)->anyhow::Result<()> {
        static WRITER:std::sync::Mutex<()>=std::sync::Mutex::new(());
        let _guard=WRITER.lock().map_err(|_|anyhow::anyhow!("notification preferences lock"))?;
        let path=Self::path()?;let mut all=Self::read(&path)?;let mut policy=self.clone();let now=Utc::now();
        policy.channels.retain(|_,until|*until>now);if policy.snooze.is_some_and(|until|until<=now){policy.snooze=None;}
        all.insert(owner.namespace(),policy);let bytes=serde_json::to_vec(&all)?;anyhow::ensure!(bytes.len()<=1<<20,"notification preferences exceed limit");
        if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?;}crate::platform::files::atomic_write(&path,&bytes)
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn timed_overrides_expire_without_changing_permanent_settings(){let now=Utc::now();let ch=Uuid::new_v4();let mut p=Policy::default();p.channels.insert(ch,now+chrono::Duration::minutes(15));assert!(!p.allows(Some(ch),false,now));assert!(p.allows(Some(Uuid::new_v4()),false,now));assert!(p.allows(Some(ch),false,now+chrono::Duration::minutes(15)));p.snooze=Some(now+chrono::Duration::hours(1));assert!(!p.allows(None,false,now));assert!(!p.allows(None,true,now+chrono::Duration::hours(2)));assert!(p.allows(None,false,now+chrono::Duration::hours(2)));}
    #[test]fn sounds_are_independent_and_migrate_the_legacy_default(){let mut p=Policy::default();assert!(!p.message_sound(false));assert!(!p.call_sound(false));p.messages=Some(false);p.calls=Some(true);assert!(!p.message_sound(true));assert!(p.call_sound(false));let back:Policy=serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();assert_eq!(back,p);}
}
