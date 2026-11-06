//! EN-US: Bounded sequential sends keep the composer independent of network completion.
//! PT-BR: Envios sequenciais limitados mantêm o compositor independente da conclusão da rede.
use super::*;
use std::collections::VecDeque;

#[derive(Debug,Clone)]
pub struct SendFailure {pub message:String,pub uncertain:bool}
impl From<String> for SendFailure{fn from(message:String)->Self{Self{message,uncertain:true}}}
impl From<&str> for SendFailure{fn from(message:&str)->Self{message.to_owned().into()}}
#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) enum Status {Queued,Uploading{bytes:u64},Failed{message:String,uncertain:bool}}
pub(crate) struct Pending {pub id:Uuid,pub attempt:Uuid,pub channel:Uuid,pub created_at:chrono::DateTime<chrono::Utc>,pub queued_at:Instant,pub draft:Draft,pub status:Status,pub attempted:bool}
#[derive(Default)]
pub(super) struct Outbox {pub entries:VecDeque<Pending>,pub active:Option<Uuid>,pub access:HashMap<Uuid,crate::models::Access>}
pub(super) struct Job {pub attempt:Uuid,pub request:crate::models::CreateMessageRequest,pub files:Vec<crate::api::features::UploadFile>}
impl Outbox {
    pub fn enqueue(&mut self,channel:Uuid,draft:Draft)->Uuid{
        let id=Uuid::new_v4();self.entries.push_back(Pending{id,attempt:Uuid::new_v4(),channel,created_at:chrono::Utc::now(),queued_at:Instant::now(),draft,status:Status::Queued,attempted:false});id
    }
    pub fn capacity(&self)->bool{self.entries.len()<20}
    pub fn next(&mut self)->Option<Job>{
        if self.active.is_some(){return None;}
        let index=loop{
            let index=self.entries.iter().position(|e|e.status==Status::Queued)?;
            let entry=&self.entries[index];
            if self.access.get(&entry.channel).is_some_and(|a|a.send&&a.read&&(entry.draft.files.is_empty()||a.attachments)){break index;}
            self.entries[index].status=Status::Failed{message:crate::i18n::tr("Permissão de envio indisponível. Confira o acesso à conversa antes de reenviar.").into(),uncertain:false};
        };
        let entry=&mut self.entries[index];
        entry.attempt=Uuid::new_v4();entry.attempted=true;entry.status=Status::Uploading{bytes:0};self.active=Some(entry.attempt);
        Some(Job{attempt:entry.attempt,request:crate::models::CreateMessageRequest{channel_id:entry.channel,content:Some(entry.draft.text.trim().into()),reply_to:entry.draft.reply.as_ref().map(|r|r.id),embeds:entry.draft.embeds.clone()},files:entry.draft.files.clone()})
    }
    pub fn progress(&mut self,attempt:Uuid,bytes:u64){if self.active==Some(attempt){if let Some(entry)=self.entries.iter_mut().find(|e|e.attempt==attempt){entry.status=Status::Uploading{bytes};}}}
    pub fn finish(&mut self,attempt:Uuid,result:Result<(),SendFailure>)->bool{
        if self.active!=Some(attempt){return false;}
        let Some(index)=self.entries.iter().position(|e|e.attempt==attempt)else{return false;};self.active=None;
        match result{Ok(())=>{self.entries.remove(index);},Err(e)=>self.entries[index].status=Status::Failed{message:e.message,uncertain:e.uncertain}}
        true
    }
    pub fn cancel(&mut self,id:Uuid)->Option<Uuid>{
        let index=self.entries.iter().position(|e|e.id==id)?;
        if matches!(self.entries[index].status,Status::Uploading{..}){
            let attempt=self.entries[index].attempt;self.active=None;
            self.entries[index].status=Status::Failed{message:crate::i18n::tr("Envio cancelado. O servidor pode ter recebido a mensagem.").into(),uncertain:true};Some(attempt)
        }else{self.entries.remove(index);None}
    }
    pub fn retry(&mut self,id:Uuid){if let Some(entry)=self.entries.iter_mut().find(|e|e.id==id&&matches!(e.status,Status::Failed{..})){
        if !entry.draft.reply_unresolved&&entry.draft.recovery_notice.is_none(){entry.status=Status::Queued;}
    }}
    pub fn saved(&self)->Vec<crate::drafts::SavedPending>{self.entries.iter().map(|p|crate::drafts::SavedPending{id:p.id,channel:p.channel,created_at:p.created_at,draft:crate::drafts::SavedDraft::capture(&p.draft),attempted:p.attempted}).collect()}
    pub async fn restore(rows:Vec<crate::drafts::SavedPending>)->VecDeque<Pending>{
        let mut entries=VecDeque::new();
        for row in rows{let draft=row.draft.restore().await;entries.push_back(Pending{id:row.id,attempt:Uuid::new_v4(),channel:row.channel,created_at:row.created_at,queued_at:Instant::now(),draft,
            status:Status::Failed{message:crate::i18n::tr("Envio recuperado. Confira o histórico antes de reenviar.").into(),uncertain:true},attempted:row.attempted});}
        entries
    }
}

impl ChatModel {
    pub(super) fn confirm_pending_retry(&self,id:Uuid,root:&gtk::Box,sender:&ComponentSender<Self>){
        let Some(entry)=self.outbox.entries.iter().find(|e|e.id==id)else{return;};
        if !matches!(entry.status,Status::Failed{uncertain:true,..}){sender.input(ChatMsg::RetryPendingConfirmed(id));return;}
        let dialog=adw::AlertDialog::builder().heading(crate::i18n::tr("Reenviar mensagem?"))
            .body(crate::i18n::tr("O servidor pode ter recebido o envio anterior. Confira o histórico para evitar uma mensagem duplicada."))
            .default_response("cancel").close_response("cancel").build();
        dialog.add_response("cancel",crate::i18n::tr("Cancelar"));dialog.add_response("retry",crate::i18n::tr("Reenviar"));
        let output=sender.input_sender().clone();dialog.connect_response(None,move|_,response|{if response=="retry"{let _=output.send(ChatMsg::RetryPendingConfirmed(id));}});
        use adw::prelude::*;dialog.present(root.root().and_downcast::<gtk::Window>().as_ref());
    }
}

#[derive(Default)]pub(super) struct Rows {entries:HashMap<Uuid,Row>}
struct Row {root:gtk::Box,feedback:gtk::Box,status:gtk::Label,progress:gtk::ProgressBar,retry:gtk::Button,edit:gtk::Button,cancel:gtk::Button}
impl Rows {
    pub fn render(&mut self,container:&gtk::Box,outbox:&Outbox,channel:Option<Uuid>,users:&HashMap<Uuid,UserSummary>,actions:&Actions,user:Option<Uuid>,avatars:&HashMap<Uuid,gtk::gdk::Texture>,config:&crate::models::UserConfig,sender:&ComponentSender<ChatModel>){
        self.entries.retain(|id,row|{let keep=outbox.entries.iter().any(|e|e.id==*id&&Some(e.channel)==channel);if !keep&&row.root.parent().is_some(){container.remove(&row.root);}keep});
        for entry in outbox.entries.iter().filter(|e|Some(e.channel)==channel){
            let row=self.entries.entry(entry.id).or_insert_with(||{
                let root=gtk::Box::new(gtk::Orientation::Horizontal,12);root.set_widget_name(&format!("pending-{}",entry.id));root.add_css_class("papo-pending-message");
                root.set_margin_start(16);root.set_margin_end(16);root.set_margin_top(12);root.set_margin_bottom(6);
                let author=user.and_then(|id|users.get(&id));let name=author.map_or(crate::i18n::tr("Você"),|u|u.display_name());
                let avatar=avatar_image(user.and_then(|id|avatars.get(&id)),36);avatar.set_text(Some(name));avatar.set_valign(gtk::Align::Start);avatar.set_visible(config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true));root.append(&avatar);
                let body=gtk::Box::new(gtk::Orientation::Vertical,4);body.set_hexpand(true);root.append(&body);
                let header=gtk::Box::new(gtk::Orientation::Horizontal,8);let author_label=gtk::Label::new(Some(name));author_label.add_css_class("heading");if let Some(roles)=author.and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&author_label,roles);}header.append(&author_label);
                if config.display.as_ref().and_then(|d|d.show_timestamps).unwrap_or(true){let time=gtk::Label::new(Some(&entry.created_at.with_timezone(&chrono::Local).format(crate::i18n::time_pattern()).to_string()));time.add_css_class("caption");time.add_css_class("dim-label");header.append(&time);}body.append(&header);
                body.append(&super::text::widget(&entry.draft.text,users,actions));
                if !entry.draft.embeds.is_empty(){let label=gtk::Label::new(Some(&crate::i18n::plural("custom_embed_count",entry.draft.embeds.len())));label.set_xalign(0.0);label.add_css_class("caption");body.append(&label);}
                for file in &entry.draft.files{let label=gtk::Label::new(Some(&file.name));label.set_xalign(0.0);label.set_ellipsize(pango::EllipsizeMode::Middle);label.add_css_class("caption");body.append(&label);}
                let feedback=gtk::Box::new(gtk::Orientation::Vertical,4);feedback.set_widget_name("pending-feedback");body.append(&feedback);
                let status=gtk::Label::new(None);status.set_xalign(0.0);status.set_wrap(true);status.add_css_class("caption");feedback.append(&status);
                let progress=gtk::ProgressBar::new();feedback.append(&progress);
                let buttons=gtk::Box::new(gtk::Orientation::Horizontal,6);feedback.append(&buttons);
                let retry=gtk::Button::with_label(crate::i18n::tr("Tentar novamente"));let edit=gtk::Button::with_label(crate::i18n::tr("Editar antes de reenviar"));let cancel=gtk::Button::with_label(crate::i18n::tr("Cancelar envio"));
                for button in [&retry,&edit,&cancel]{button.add_css_class("flat");buttons.append(button);}
                let id=entry.id;let input=sender.input_sender().clone();retry.connect_clicked(move |_|{let _=input.send(ChatMsg::RetryPending(id));});
                let input=sender.input_sender().clone();edit.connect_clicked(move |_|{let _=input.send(ChatMsg::EditPending(id));});
                let input=sender.input_sender().clone();cancel.connect_clicked(move |_|{let _=input.send(ChatMsg::CancelPending(id));});
                container.append(&root);Row{root,feedback,status,progress,retry,edit,cancel}
            });
            let total=entry.draft.files.iter().map(|f|f.size).sum::<u64>();
            let (text,uploading,failed)=match &entry.status{
                Status::Queued=>(crate::i18n::tr("Na fila de envio").to_owned(),false,false),
                Status::Uploading{bytes}=>{row.progress.set_fraction(if total==0{0.0}else{(*bytes as f64/total as f64).min(1.0)});(if total>0{crate::i18n::fmt("Enviando: {sent} / {total}",&[("sent",format_file_size(*bytes as i64)),("total",format_file_size(total as i64))])}else{crate::i18n::tr("Enviando mensagem…").into()},true,false)},
                Status::Failed{message,..}=>(format!("{}{}",message,entry.draft.recovery_notice.as_ref().map(|n|format!(" {n}")).unwrap_or_default()),false,true),
            };
            row.status.set_text(&text);row.progress.set_visible(uploading&&total>0);row.retry.set_visible(failed&&entry.draft.recovery_notice.is_none()&&!entry.draft.reply_unresolved);
            row.feedback.set_visible(failed||entry.queued_at.elapsed()>=std::time::Duration::from_millis(500));
            row.edit.set_visible(failed);row.cancel.set_label(crate::i18n::tr(if failed{"Descartar"}else{"Cancelar envio"}));
            if failed{row.status.add_css_class("error");}else{row.status.remove_css_class("error");}
        }
        container.set_visible(!self.entries.is_empty());
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn independent_composer_order_and_late_completion_after_cancel(){
        let channel=Uuid::new_v4();let mut queue=Outbox::default();queue.access.insert(channel,crate::models::Access::direct());
        let mut first=Draft::default();first.text="first".into();let id=queue.enqueue(channel,first);
        let job=queue.next().unwrap();let mut second=Draft::default();second.text="second".into();queue.enqueue(channel,second);
        assert!(queue.next().is_none());assert_eq!(queue.cancel(id),Some(job.attempt));
        assert!(!queue.finish(job.attempt,Ok(())));assert_eq!(queue.entries.len(),2);
        let next=queue.next().unwrap();assert_eq!(next.request.content.as_deref(),Some("second"));
        assert!(queue.finish(next.attempt,Ok(())));assert_eq!(queue.entries.len(),1);
    }
    #[tokio::test]async fn recovered_uploads_keep_files_embeds_and_never_start_themselves(){
        let path=std::env::temp_dir().join(format!("papo-outbox-{}.txt",Uuid::new_v4()));std::fs::write(&path,b"attachment").unwrap();
        let file=crate::api::features::UploadFile::inspect(path.clone()).await.unwrap();
        let channel=Uuid::new_v4();let mut queue=Outbox::default();queue.access.insert(channel,crate::models::Access::direct());
        let mut draft=Draft::default();draft.text="with attachment".into();draft.files.push(file);draft.embeds.push(crate::models::EmbedInput{title:"Custom embed".into(),..Default::default()});
        let id=queue.enqueue(channel,draft);
        let queued=Outbox::restore(queue.saved()).await;assert!(matches!(queued[0].status,Status::Failed{uncertain:true,..}),"an older debounced queued snapshot cannot prove no later POST was attempted");
        queue.next().unwrap();
        let entries=Outbox::restore(queue.saved()).await;let mut restored=Outbox{entries,..Default::default()};restored.access.insert(channel,crate::models::Access::direct());
        assert!(restored.next().is_none());assert_eq!(restored.entries[0].draft.files.len(),1);assert_eq!(restored.entries[0].draft.embeds[0].title,"Custom embed");
        restored.retry(id);assert_eq!(restored.next().unwrap().files.len(),1);
        std::fs::remove_file(&path).unwrap();let mut missing=Outbox{entries:Outbox::restore(queue.saved()).await,..Default::default()};missing.access.insert(channel,crate::models::Access::direct());missing.retry(id);assert!(missing.next().is_none());assert!(missing.entries[0].draft.recovery_notice.is_some());
        let recovered=crate::drafts::SavedDraft::capture(&missing.entries[0].draft).restore().await;assert!(recovered.recovery_notice.is_some(),"missing-file warning must survive another restart");
    }
    #[test]fn revoked_access_blocks_queued_messages_without_sending(){
        let channel=Uuid::new_v4();let mut queue=Outbox::default();let mut draft=Draft::default();draft.text="unsent".into();queue.enqueue(channel,draft);
        assert!(queue.next().is_none());assert!(matches!(queue.entries[0].status,Status::Failed{uncertain:false,..}));
        queue.access.insert(channel,crate::models::Access::direct());assert!(queue.next().is_none(),"restored permission must not auto-retry a failed message");
    }
    #[test]fn failed_requests_are_never_automatically_replayed(){
        let channel=Uuid::new_v4();let mut queue=Outbox::default();queue.access.insert(channel,crate::models::Access::direct());
        let mut draft=Draft::default();draft.text="uncertain".into();let id=queue.enqueue(channel,draft);let first=queue.next().unwrap();
        assert!(queue.finish(first.attempt,Err("lost response".into())));assert!(queue.next().is_none());
        queue.retry(id);let retried=queue.next().unwrap();assert_ne!(retried.attempt,first.attempt);
        assert!(!queue.finish(first.attempt,Ok(())));assert_eq!(queue.active,Some(retried.attempt));
    }
}
