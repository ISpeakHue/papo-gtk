//! Independent per-member generations and full-snapshot reconciliation.
use std::collections::{HashMap,HashSet};
use uuid::Uuid;
use crate::models::UserSummary;
#[derive(Default)]
pub(super) struct Members {
    version:u64,
    accepted:HashMap<Uuid,u64>,
    pending:HashMap<Uuid,Uuid>,
    pub queued:HashSet<Uuid>,
    pub debounce:bool,
    full:Option<(Uuid,u64)>,
    pub full_again:bool,
    pub jobs:Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Members{fn drop(&mut self){for job in &self.jobs{job.abort();}}}
impl Members {
    pub fn is_pending(&self,id:Uuid)->bool{self.pending.contains_key(&id)||self.queued.contains(&id)}
    pub fn version(&self)->u64{self.version}
    pub fn changed_after(&self,id:Uuid,version:u64)->bool{self.accepted.get(&id).is_some_and(|v|*v>version)}
    pub fn begin_full(&mut self)->Option<(Uuid,u64)>{
        if self.full.is_some(){self.full_again=true;return None;}
        let token=Uuid::new_v4();let version=self.version;self.full=Some((token,version));Some((token,version))
    }
    pub fn invalidate(&mut self,id:Uuid){self.pending.remove(&id);self.version+=1;self.accepted.insert(id,self.version);}
    pub fn begin(&mut self,ids:&[Uuid])->Uuid {
        let token=Uuid::new_v4();for id in ids{self.pending.insert(*id,token);}token
    }
    pub fn finish(&mut self,token:Uuid,full:bool,ids:&[Uuid],result:Option<Vec<UserSummary>>,users:&mut Vec<UserSummary>)->bool {
        let old=users.clone();
        if full {
            let Some((request,version))=self.full else{return false;};if request!=token{return false;}
            self.full=None;
            if let Some(mut incoming)=result {
                for user in users.iter(){if self.accepted.get(&user.id).is_some_and(|v|*v>version)||self.pending.contains_key(&user.id)||self.queued.contains(&user.id){
                    if let Some(item)=incoming.iter_mut().find(|u|u.id==user.id){*item=user.clone();}else{incoming.push(user.clone());}
                }}
                *users=incoming;let ids:HashSet<_>=users.iter().map(|u|u.id).collect();self.accepted.retain(|id,_|ids.contains(id));
            }
        } else {
            let mut incoming:HashMap<_,_>=result.unwrap_or_default().into_iter().map(|u|(u.id,u)).collect();
            for id in ids{if self.pending.get(id)==Some(&token){self.pending.remove(id);if let Some(user)=incoming.remove(id){
                self.version+=1;self.accepted.insert(*id,self.version);
                if let Some(existing)=users.iter_mut().find(|u|u.id==*id){*existing=user;}else{users.push(user);}
            }}}
        }
        *users!=old
    }
}
#[cfg(test)]mod tests{
    use super::*;
    fn user(id:Uuid,name:&str)->UserSummary{serde_json::from_value(serde_json::json!({"id":id,"username":name,"created_at":"2026-10-08T12:00:00Z"})).unwrap()}
    #[test]fn snapshot_and_reversed_member_responses_preserve_independent_updates(){
        let a=Uuid::new_v4();let b=Uuid::new_v4();let mut users=vec![user(a,"old")];let mut m=Members::default();
        let (full,_)=m.begin_full().unwrap();let old=m.begin(&[a]);let new=m.begin(&[a]);let other=m.begin(&[b]);
        assert!(m.finish(new,false,&[a],Some(vec![user(a,"new")]),&mut users));
        assert!(!m.finish(old,false,&[a],Some(vec![user(a,"stale")]),&mut users));
        assert!(m.finish(other,false,&[b],Some(vec![user(b,"other")]),&mut users));
        m.finish(full,true,&[],Some(vec![user(a,"snapshot")]),&mut users);
        assert_eq!(users.iter().find(|u|u.id==a).unwrap().username,"new");assert!(users.iter().any(|u|u.id==b));
        let (fresh,_)=m.begin_full().unwrap();assert!(m.finish(fresh,true,&[],Some(vec![user(a,"fresh")]),&mut users));assert_eq!(users[0].username,"fresh");
    }
}
