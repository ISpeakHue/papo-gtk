//! EN-US: Restore before saving; edits made during disk loading always win over old snapshots.
//! PT-BR: Restaura antes de salvar; edições feitas durante a leitura sempre vencem snapshots antigos.
use super::*;
use std::collections::{HashSet,VecDeque};

#[derive(Default)]pub(super) struct Persistence {
    pub epoch:Uuid,pub loading:bool,pub dirty:HashSet<Uuid>,pub notice:Option<String>,
    writer:Option<crate::drafts::Writer>,task:Option<tokio::task::JoinHandle<()>>,pub reply_task:Option<tokio::task::JoinHandle<()>>,
    last:Option<crate::drafts::Snapshot>,
}
impl Drop for Persistence{fn drop(&mut self){if let Some(task)=self.task.take(){task.abort();}if let Some(task)=self.reply_task.take(){task.abort();}}}
pub struct Loaded {pub drafts:HashMap<Uuid,Draft>,pub pending:VecDeque<outbox::Pending>,pub snapshot:crate::drafts::Snapshot}
impl std::fmt::Debug for Loaded{fn fmt(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.debug_struct("LoadedDrafts").field("drafts",&self.drafts.len()).field("pending",&self.pending.len()).finish()}}
impl ChatModel {
    pub(super) fn remove_deleted_reply(&mut self,id:Uuid){
        fn clear(draft:&mut Draft,id:Uuid){if draft.reply.as_ref().is_some_and(|r|r.id==id){draft.reply=None;draft.reply_unresolved=false;draft.reply_error=Some(crate::i18n::tr("A mensagem respondida foi removida. O texto do rascunho foi preservado.").into());}}
        if self.draft.reply.as_ref().is_some_and(|r|r.id==id){if let Some(task)=self.persistence.reply_task.take(){task.abort();}}
        clear(&mut self.draft,id);for draft in self.session.drafts.values_mut(){clear(draft,id);}
        // Queued replies must not silently turn into unrelated standalone messages.
        for entry in &mut self.outbox.entries{if entry.draft.reply.as_ref().is_some_and(|r|r.id==id)&&entry.status==outbox::Status::Queued{
            clear(&mut entry.draft,id);entry.status=outbox::Status::Failed{message:crate::i18n::tr("A mensagem respondida foi removida. O texto do rascunho foi preservado.").into(),uncertain:false};
        }}
    }

    pub(super) fn start_persistence(&mut self,sender:&ComponentSender<Self>){
        if self.actions.pinned_view||self.persistence.loading||self.persistence.writer.is_some(){return;}
        #[cfg(test)]if std::env::var_os("PAPO_TEST_CONFIG_DIR").is_none(){return;}
        let Some(owner)=self.session.owner.clone()else{return;};let Ok(path)=crate::drafts::path(&owner)else{return;};
        self.persistence.epoch=Uuid::new_v4();let epoch=self.persistence.epoch;self.persistence.loading=true;
        let output=sender.input_sender().clone();
        self.persistence.task=Some(tokio::spawn(async move{
            let result=async{
                let snapshot=tokio::task::spawn_blocking(move||crate::drafts::restore_at(&path,&owner)).await??;
                let mut drafts=HashMap::new();for (channel,saved) in snapshot.drafts.clone(){drafts.insert(channel,saved.restore().await);}
                let pending=outbox::Outbox::restore(snapshot.pending.clone()).await;
                Ok::<_,anyhow::Error>(Loaded{drafts,pending,snapshot})
            }.await.map_err(|_|crate::i18n::tr("Não foi possível restaurar os rascunhos. O arquivo salvo foi preservado.").to_owned());
            let _=output.send(ChatMsg::DraftsLoaded{epoch,result});
        }));
    }
    pub(super) fn drafts_loaded(&mut self,epoch:Uuid,result:Result<Loaded,String>,entry:&gtk::TextView,sender:&ComponentSender<Self>){
        if self.persistence.epoch!=epoch{return;}
        self.persistence.loading=false;self.persistence.task=None;
        match result{
            Ok(loaded)=>{
                let account=self.session.owner.clone().expect("account-bound restore");
                self.persistence.last=Some(loaded.snapshot);
                for (channel,draft) in loaded.drafts{
                    if self.persistence.dirty.contains(&channel){continue;}
                    if self.active_channel.as_ref().is_some_and(|c|c.id()==channel){self.draft=draft;self.composer.set_wire(entry,&self.draft.text,&self.users_map);}
                    else{self.session.drafts.insert(channel,draft);}
                }
                self.outbox.entries.extend(loaded.pending);
                if let Ok(path)=crate::drafts::path(&account){let output=sender.input_sender().clone();self.persistence.writer=Some(crate::drafts::Writer::new(path,move|result|{let _=output.send(ChatMsg::DraftsSaved{epoch,result});}));}
            },
            Err(message)=>self.persistence.notice=Some(message),
        }
        sender.input(ChatMsg::ResolveDraftReply);
    }
    pub(super) fn save_drafts(&mut self,sender:&ComponentSender<Self>){
        let mut channels:HashSet<_>=self.session.drafts.iter().filter(|(_,d)|!d.is_empty()).map(|(id,_)|*id).collect();
        if !self.draft.is_empty(){if let Some(channel)=&self.active_channel{channels.insert(channel.id());}}
        if channels!=self.draft_channels{self.draft_channels=channels.clone();let _=sender.output(ChatOutput::DraftChannels(channels));}
        if self.persistence.loading||self.actions.pinned_view{return;}
        let (Some(owner),Some(writer))=(self.session.owner.clone(),self.persistence.writer.as_ref())else{return;};
        let mut snapshot=crate::drafts::Snapshot::empty(owner);
        snapshot.drafts=self.session.drafts.iter().filter_map(|(id,draft)|{let saved=crate::drafts::SavedDraft::capture(draft);(!saved.is_empty()).then_some((*id,saved))}).collect();
        if let Some(channel)=&self.active_channel{let saved=crate::drafts::SavedDraft::capture(&self.draft);if saved.is_empty(){snapshot.drafts.remove(&channel.id());}else{snapshot.drafts.insert(channel.id(),saved);}}
        snapshot.pending=self.outbox.saved();
        if self.persistence.last.as_ref()!=Some(&snapshot){writer.queue(snapshot.clone());self.persistence.last=Some(snapshot);}
    }
    pub(super) fn resolve_reply(&mut self,sender:&ComponentSender<Self>){
        let (Some(api),Some(reply))=(self.actions.api.clone(),self.draft.reply.clone())else{return;};
        if !self.draft.reply_unresolved||!self.access.read||self.persistence.reply_task.as_ref().is_some_and(|t|!t.is_finished()){return;}
        let epoch=self.persistence.epoch;let output=sender.input_sender().clone();
        self.persistence.reply_task=Some(tokio::spawn(async move{
            let result=api.moderation_messages(reply.channel_id,&reply).await.map(|page|page.messages.into_iter().find(|m|m.id==reply.id));
            let _=output.send(ChatMsg::ReplyRestored{epoch,channel:reply.channel_id,id:reply.id,result:result.map_err(|_|crate::i18n::tr("Não foi possível restaurar a resposta. Remova a referência ou tente novamente.").to_owned())});
        }));
    }
}
