//! Moderation and append-only audit viewer, gated by manage_server.
use super::*;
use crate::models::{AuditFilter,AuditPage,RecoveryLink};
use crate::ui::chat::actions::window;
#[derive(Debug,Clone,Copy)]
pub enum Operation {Ban(bool),Recovery}
#[derive(Debug)]
pub enum ModerationMsg {Open,Close,TargetChanged,Confirm(Operation),Apply{view:Uuid,user:Uuid,operation:Operation},
    Saved{view:Uuid,user:Uuid,result:anyhow::Result<Option<RecoveryLink>>},Copy,
    Filter,More,Logs{view:Uuid,request:Uuid,append:bool,result:anyhow::Result<AuditPage>}}
#[derive(Default)]
pub(super) struct Moderation {view:Option<View>,confirmation:Option<gtk::Window>,jobs:Vec<tokio::task::JoinHandle<()>>,busy:bool,request:Option<Uuid>,filter:AuditFilter,cursor:Option<Uuid>}
struct View {token:Uuid,window:gtk::Window,body:gtk::Box,user:gtk::Entry,status:gtk::Label,link:gtk::Entry,expiry:gtk::Label,copy:gtk::Button,recovery:Option<RecoveryLink>,recovery_user:Option<Uuid>,filters:Vec<gtk::Entry>,ascending:gtk::CheckButton,logs:gtk::Box,more:gtk::Button}
impl Moderation {fn clear(&mut self){for j in self.jobs.drain(..){j.abort();}self.request=None;self.busy=false;self.cursor=None;if let Some(w)=self.confirmation.take(){w.close();}if let Some(v)=self.view.take(){v.link.set_text("");v.window.close();}}}
impl Drop for Moderation {fn drop(&mut self){self.clear();}}
fn button(body:&gtk::Box,text:&str,s:&ComponentSender<MainWindowModel>,f:impl Fn()->ModerationMsg+'static)->gtk::Button{let b=gtk::Button::with_label(text);let s=s.clone();b.connect_clicked(move |_|s.input(MainWindowMsg::Moderation(f())));body.append(&b);b}
fn entry(body:&gtk::Box,text:&str)->gtk::Entry{let e=gtk::Entry::new();e.set_placeholder_text(Some(text));body.append(&e);e}
impl MainWindowModel {
    pub(super) fn sync_moderation_access(&mut self){if !self.server_access.manage_server{self.moderation.clear();}}
    fn load_audit(&mut self,append:bool,s:ComponentSender<Self>){
        if !self.server_access.manage_server||self.moderation.request.is_some(){return;}
        let Some(v)=self.moderation.view.as_ref().filter(|v|v.window.is_visible())else{return;};
        let view=v.token;let request=Uuid::new_v4();self.moderation.request=Some(request);v.more.set_sensitive(false);v.status.set_text("Carregando auditoria…");
        let filter=self.moderation.filter.clone();let cursor=if append{self.moderation.cursor}else{None};let api=self.api_client.clone();
        self.moderation.jobs.push(tokio::spawn(async move{let result=api.audit_logs(&filter,cursor).await;s.input(MainWindowMsg::Moderation(ModerationMsg::Logs{view,request,append,result}));}));
    }
    pub(super) fn moderation_event(&mut self,msg:ModerationMsg,s:ComponentSender<Self>,root:&gtk::Box){
        self.moderation.jobs.retain(|j|!j.is_finished());
        if let ModerationMsg::Close=msg{self.moderation.clear();return;}
        if !self.server_access.manage_server{self.moderation.clear();return;}
        match msg {
            ModerationMsg::Open=>{
                if let Some(v)=&self.moderation.view{if v.window.is_visible(){v.window.present();return;}}
                self.moderation.clear();let token=Uuid::new_v4();let(w,body)=window(root,"Moderação e auditoria");w.set_default_size(650,750);
                let choices:Vec<_>=self.users.iter().map(|u|format!("{}{} · {}",u.display_name(),if u.banned{" (banido)"}else{""},u.id)).collect();
                let select=gtk::DropDown::from_strings(&choices.iter().map(String::as_str).collect::<Vec<_>>());body.append(&select);
                let user=entry(&body,"UUID do membro (também permite desbanir um membro ausente)");
                let ids:Vec<_>=self.users.iter().map(|u|u.id).collect();if let Some(id)=ids.first(){user.set_text(&id.to_string());}let e=user.clone();select.connect_selected_notify(move|d|{if let Some(id)=ids.get(d.selected() as usize){e.set_text(&id.to_string());}});
                let controls=gtk::Box::new(gtk::Orientation::Horizontal,6);body.append(&controls);
                button(&controls,"Banir membro",&s,||ModerationMsg::Confirm(Operation::Ban(true)));button(&controls,"Desbanir membro",&s,||ModerationMsg::Confirm(Operation::Ban(false)));button(&controls,"Gerar recuperação",&s,||ModerationMsg::Confirm(Operation::Recovery));
                let link=entry(&body,"Link de recuperação");link.set_editable(false);let expiry=gtk::Label::new(None);body.append(&expiry);let copy=button(&body,"Copiar link",&s,||ModerationMsg::Copy);copy.set_sensitive(false);
                body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));body.append(&gtk::Label::new(Some("Auditoria · somente leitura")));
                let filters:Vec<_>=["Ação (ex.: user.ban)","UUID do ator","Tipo de entidade (ex.: user)","Desde (RFC3339, ex.: 2026-10-01T00:00:00Z)","Até (RFC3339)"].iter().map(|l|entry(&body,l)).collect();
                let ascending=gtk::CheckButton::with_label("Mais antigos primeiro");body.append(&ascending);button(&body,"Aplicar filtros",&s,||ModerationMsg::Filter);
                let logs=gtk::Box::new(gtk::Orientation::Vertical,8);let scroll=gtk::ScrolledWindow::builder().vexpand(true).min_content_height(180).child(&logs).build();body.append(&scroll);
                let more=button(&body,"Mais registros",&s,||ModerationMsg::More);more.set_sensitive(false);let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);
                let l=link.clone();let s2=s.clone();w.connect_close_request(move |_|{l.set_text("");s2.input(MainWindowMsg::Moderation(ModerationMsg::Close));gtk::glib::Propagation::Proceed});
                let s2=s.clone();user.connect_changed(move |_|s2.input(MainWindowMsg::Moderation(ModerationMsg::TargetChanged)));
                self.moderation.view=Some(View{token,window:w.clone(),body,user,status,link,expiry,copy,recovery:None,recovery_user:None,filters,ascending,logs,more});self.moderation.filter=AuditFilter::default();w.present();self.load_audit(false,s);
            }
            ModerationMsg::Confirm(operation)=>{
                if self.moderation.busy{return;}let Some(v)=self.moderation.view.as_ref().filter(|v|v.window.is_visible())else{return;};
                let user=match v.user.text().trim().parse::<Uuid>(){Ok(id)=>id,Err(_)=>{v.status.set_text("Informe um UUID de membro válido.");return;}};
                if matches!(operation,Operation::Recovery)&&user==self.current_user.id{v.status.set_text("Use Segurança para alterar sua própria senha.");return;}
                if matches!(operation,Operation::Ban(true))&&self.server.as_ref().and_then(|s|s.owner_id)==Some(user){v.status.set_text("O dono do servidor não pode ser banido.");return;}
                let view=v.token;let(w,body)=window(root,"Confirmar moderação");
                let verb=match operation{Operation::Ban(true)=>"Banir encerra todas as sessões do membro.",Operation::Ban(false)=>"Desbanir permite ao membro entrar novamente.",Operation::Recovery=>"O novo link substitui a recuperação anterior. Compartilhe-o apenas com o membro escolhido."};
                let label=gtk::Label::new(Some(&format!("{verb}\nMembro: {user}")));label.set_wrap(true);body.append(&label);
                let cancel=gtk::Button::with_label("Cancelar");let c=w.clone();cancel.connect_clicked(move |_|c.close());body.append(&cancel);
                let c=w.clone();button(&body,"Confirmar moderação",&s,move||{c.close();ModerationMsg::Apply{view,user,operation}});if let Some(old)=self.moderation.confirmation.replace(w.clone()){old.close();}w.present();
            }
            ModerationMsg::Apply{view,user,operation}=>{
                if self.moderation.busy{return;}let Some(v)=self.moderation.view.as_mut().filter(|v|v.token==view&&v.window.is_visible())else{return;};
                if matches!(operation,Operation::Recovery){v.recovery=None;v.recovery_user=None;v.link.set_text("");v.copy.set_sensitive(false);v.expiry.set_text("");}
                self.moderation.busy=true;v.body.set_sensitive(false);v.status.set_text("Aplicando moderação…");let api=self.api_client.clone();let own=self.current_user.id;
                self.moderation.jobs.push(tokio::spawn(async move{let result=match operation{Operation::Ban(state)=>api.set_user_banned(user,state).await.map(|_|None),Operation::Recovery=>api.create_recovery_link(own,user).await.map(Some)};s.input(MainWindowMsg::Moderation(ModerationMsg::Saved{view,user,result}));}));
            }
            ModerationMsg::Saved{view,user,result}=>{
                self.moderation.busy=false;let Some(v)=self.moderation.view.as_mut().filter(|v|v.token==view&&v.window.is_visible())else{return;};v.body.set_sensitive(true);
                match result{Ok(link)=>{v.status.set_text("Operação concluída.");if let Some(link)=link{v.link.set_text(&link.reset_url);v.expiry.set_text(&format!("Recuperação de {user} · expira em {}",link.expires_at.to_rfc3339()));v.copy.set_sensitive(link.expires_at>chrono::Utc::now()&&v.user.text().trim().parse::<Uuid>().ok()==Some(user));v.recovery_user=Some(user);v.recovery=Some(link);}self.refresh_users(s.clone(),Some(user));if self.moderation.request.is_none(){self.load_audit(false,s);}},Err(e)=>{v.status.set_text(&e.to_string());s.input(MainWindowMsg::ActionError(e));}}
            }
            ModerationMsg::Copy=>{if let Some(v)=&self.moderation.view{if v.window.is_visible(){if let Some(link)=v.recovery.as_ref().filter(|l|l.expires_at>chrono::Utc::now()&&v.recovery_user==v.user.text().trim().parse::<Uuid>().ok()){v.window.clipboard().set_text(&link.reset_url);v.status.set_text("Link copiado. Compartilhe-o apenas com o membro correto.");}else{v.copy.set_sensitive(false);v.link.set_text("");v.status.set_text("O link expirou. Gere outro.");}}}}
            ModerationMsg::TargetChanged=>{if let Some(v)=self.moderation.view.as_mut(){if v.recovery.is_some()&&v.recovery_user!=v.user.text().trim().parse::<Uuid>().ok(){v.recovery=None;v.recovery_user=None;v.link.set_text("");v.expiry.set_text("");v.copy.set_sensitive(false);}}}
            ModerationMsg::Filter=>{
                let Some(v)=&self.moderation.view else{return;};
                let parse=||->anyhow::Result<AuditFilter>{let text:Vec<_>=v.filters.iter().map(|e|e.text().trim().to_owned()).collect();let f=AuditFilter{action:text[0].clone(),actor_id:if text[1].is_empty(){None}else{Some(text[1].parse().map_err(|_|anyhow::anyhow!("UUID do ator inválido."))?)},entity_type:text[2].clone(),since:if text[3].is_empty(){None}else{Some(text[3].parse().map_err(|_|anyhow::anyhow!("Data inicial inválida; use RFC3339."))?)},until:if text[4].is_empty(){None}else{Some(text[4].parse().map_err(|_|anyhow::anyhow!("Data final inválida; use RFC3339."))?)},ascending:v.ascending.is_active()};f.validate()?;Ok(f)};
                match parse(){Ok(f)=>{self.moderation.request=None;self.moderation.cursor=None;self.moderation.filter=f;self.load_audit(false,s);},Err(e)=>v.status.set_text(&e.to_string())}
            }
            ModerationMsg::More=>{if self.moderation.cursor.is_some()&&self.moderation.view.as_ref().is_some_and(|v|v.more.is_sensitive()){self.load_audit(true,s);}}
            ModerationMsg::Logs{view,request,append,result}=>{
                if self.moderation.request!=Some(request){return;}self.moderation.request=None;let Some(v)=self.moderation.view.as_ref().filter(|v|v.token==view&&v.window.is_visible())else{return;};
                match result{Ok(page)=>{if !append{while let Some(c)=v.logs.first_child(){v.logs.remove(&c);}}
                    self.moderation.cursor=page.logs.last().map(|e|e.id).or(if append{self.moderation.cursor}else{None});
                    for e in page.logs{let label=gtk::Label::new(Some(&format!("{} · {} · {} · {}\nAlvo: {}",e.created_at.to_rfc3339(),e.actor_username,e.action,e.entity_type,e.target_user_id.map(|id|id.to_string()).unwrap_or_else(||"—".into()))));label.set_wrap(true);label.set_xalign(0.0);label.set_selectable(true);v.logs.append(&label);}v.more.set_sensitive(page.has_more);v.status.set_text(if page.has_more{"Há mais registros."}else{"Todos os registros carregados."});},Err(e)=>{v.more.set_sensitive(append&&self.moderation.cursor.is_some());v.status.set_text(&e.to_string());s.input(MainWindowMsg::ActionError(e));}}
            }
            ModerationMsg::Close=>{}
        }
    }
}

#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext,backend:&std::sync::Arc<std::sync::Mutex<crate::ui::chat::actions::tests::Backend>>){
    use crate::ui::chat::actions::tests::{find_button,pump,until,descendants};
    let peer:Uuid="12345678-1234-4234-8234-123456789ac0".parse().unwrap();
    find_button(main.widget().upcast_ref(),"Moderação e auditoria").emit_clicked();until(context,||main.model().moderation.view.is_some()&&main.model().moderation.request.is_none());
    let w=main.model().moderation.view.as_ref().unwrap().window.clone();
    assert_eq!(main.model().moderation.view.as_ref().unwrap().logs.observe_children().n_items(),1);
    find_button(w.upcast_ref(),"Mais registros").emit_clicked();until(context,||main.model().moderation.request.is_none()&&!main.model().moderation.view.as_ref().unwrap().more.is_sensitive());assert_eq!(main.model().moderation.view.as_ref().unwrap().logs.observe_children().n_items(),2);
    assert!(!descendants(w.upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).any(|l|l.text().contains("never-display")));
    {let m=main.model();let v=m.moderation.view.as_ref().unwrap();v.filters[1].set_text("invalid");}find_button(w.upcast_ref(),"Aplicar filtros").emit_clicked();pump(context);assert!(main.model().moderation.view.as_ref().unwrap().status.text().contains("UUID"));
    {let m=main.model();let v=m.moderation.view.as_ref().unwrap();v.filters[1].set_text("");v.filters[0].set_text("user.ban");v.filters[3].set_text("2026-10-01T00:00:00Z");}backend.lock().unwrap().fail_audit=true;
    find_button(w.upcast_ref(),"Aplicar filtros").emit_clicked();until(context,||main.model().moderation.request.is_none()&&main.model().moderation.view.as_ref().unwrap().status.text().contains("Audit retry"));assert_eq!(main.model().moderation.view.as_ref().unwrap().filters[0].text(),"user.ban");
    find_button(w.upcast_ref(),"Aplicar filtros").emit_clicked();until(context,||main.model().moderation.request.is_none()&&main.model().moderation.view.as_ref().unwrap().more.is_sensitive());
    main.model().moderation.view.as_ref().unwrap().user.set_text(&peer.to_string());
    let count=||backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p.ends_with("/ban")).count();let before=count();find_button(w.upcast_ref(),"Banir membro").emit_clicked();pump(context);let dialog=main.model().moderation.confirmation.as_ref().unwrap().clone();find_button(dialog.upcast_ref(),"Cancelar").emit_clicked();pump(context);assert_eq!(count(),before);
    find_button(w.upcast_ref(),"Banir membro").emit_clicked();pump(context);find_button(main.model().moderation.confirmation.as_ref().unwrap().upcast_ref(),"Confirmar moderação").emit_clicked();until(context,||!main.model().moderation.busy&&main.model().moderation.view.as_ref().unwrap().status.text().contains("Moderation retry"));assert_eq!(main.model().moderation.view.as_ref().unwrap().user.text(),peer.to_string());
    find_button(w.upcast_ref(),"Banir membro").emit_clicked();pump(context);find_button(main.model().moderation.confirmation.as_ref().unwrap().upcast_ref(),"Confirmar moderação").emit_clicked();until(context,||main.model().users.iter().any(|u|u.id==peer&&u.banned));
    find_button(w.upcast_ref(),"Desbanir membro").emit_clicked();pump(context);find_button(main.model().moderation.confirmation.as_ref().unwrap().upcast_ref(),"Confirmar moderação").emit_clicked();until(context,||!main.model().moderation.busy&&main.model().users.iter().any(|u|u.id==peer&&!u.banned));
    find_button(w.upcast_ref(),"Gerar recuperação").emit_clicked();pump(context);find_button(main.model().moderation.confirmation.as_ref().unwrap().upcast_ref(),"Confirmar moderação").emit_clicked();until(context,||!main.model().moderation.busy&&main.model().moderation.view.as_ref().unwrap().recovery.is_some());
    let m=main.model();let v=m.moderation.view.as_ref().unwrap();assert!(v.copy.is_sensitive());assert!(v.expiry.text().contains("2099"));let link=v.recovery.as_ref().unwrap().reset_url.clone();drop(m);
    // Issued links use the already implemented unauthenticated recovery contract.
    let api=main.model().api_client.clone();context.block_on(api.recover_password(&link,"Password!9")).unwrap();
    assert!(backend.lock().unwrap().requests.iter().any(|(m,p,b)|m=="POST"&&p=="/auth/password_reset"&&b["token"]=="issued-token"));
    find_button(w.upcast_ref(),"Copiar link").emit_clicked();pump(context);assert!(main.model().moderation.view.as_ref().unwrap().status.text().contains("copiado"));
    main.model().moderation.view.as_ref().unwrap().user.set_text(&main.model().current_user.id.to_string());pump(context);assert!(main.model().moderation.view.as_ref().unwrap().link.text().is_empty());assert!(!main.model().moderation.view.as_ref().unwrap().copy.is_sensitive());
}
#[cfg(test)]
pub(crate) fn exercise_denied(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{find_button,pump};assert!(main.model().moderation.view.is_none());assert!(!find_button(main.widget().upcast_ref(),"Moderação e auditoria").get_visible());
    main.emit(MainWindowMsg::Moderation(ModerationMsg::Open));main.emit(MainWindowMsg::Moderation(ModerationMsg::Confirm(Operation::Ban(true))));main.emit(MainWindowMsg::Moderation(ModerationMsg::Filter));pump(context);assert!(main.model().moderation.view.is_none());
}
