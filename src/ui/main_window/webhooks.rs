//! EN-US: Permission-gated management. Secrets are held only in the current dialog.
//! PT-BR: Gestão de webhooks conforme as permissões; segredos existem apenas no diálogo atual.
use super::*;
use crate::models::*;
use crate::ui::chat::actions::window;
use adw::prelude::*;

#[derive(Debug)]
pub enum WebhookMsg {
    Open, Close(Uuid), Reload, Create, Edit(Uuid), Save, Avatar(bool),
    AvatarLoaded{view:Uuid,edit:Uuid,result:anyhow::Result<Vec<u8>>},
    Revoke(Uuid), ConfirmRevoke{view:Uuid,id:Uuid}, DismissRevoke(Uuid),
    Loaded{view:Uuid,result:anyhow::Result<Vec<Webhook>>},
    Saved{view:Uuid,result:anyhow::Result<Saved>},
}
#[derive(Debug)]
pub enum Saved { Created(CreatedWebhook), Updated(Webhook), Revoked(Uuid) }
#[derive(Default)]
pub(super) struct Webhooks { view:Option<View>,job:Option<tokio::task::JoinHandle<()>>,dialog:Option<adw::AlertDialog> }
struct View {
    token:Uuid,window:gtk::Window,controls:gtk::Box,status:gtk::Label,list:gtk::Box,items:Vec<Webhook>,
    name:gtk::Entry,channel:gtk::DropDown,channel_ids:Vec<Uuid>,
    endpoint:gtk::Entry,secret:gtk::PasswordEntry,credential:gtk::Box,
    form:gtk::Box,edit:Option<Uuid>,nickname:gtk::Entry,avatar:Option<(String,String)>,
}
impl Drop for Webhooks {fn drop(&mut self){if let Some(job)=self.job.take(){job.abort();}if let Some(d)=self.dialog.take(){d.force_close();}if let Some(v)=self.view.take(){v.secret.set_text("");v.window.close();}}}
fn entry(body:&gtk::Box,label:&str)->gtk::Entry{let l=gtk::Label::new(Some(label));l.set_xalign(0.0);body.append(&l);let e=gtk::Entry::new();body.append(&e);e}
fn button(body:&gtk::Box,label:&str,s:&ComponentSender<MainWindowModel>,msg:impl Fn()->WebhookMsg+'static){let b=gtk::Button::with_label(label);let output=s.input_sender().clone();b.connect_clicked(move |_|{let _=output.send(MainWindowMsg::Webhook(msg()));});body.append(&b);}
impl MainWindowModel {
    pub(super) fn sync_webhook_access(&mut self){
        if !self.server_access.manage_webhooks{self.webhooks=Default::default();}
    }
    fn webhook_status(&self,text:&str){if let Some(v)=&self.webhooks.view{v.status.set_text(text);}}
    fn webhook_job(&mut self,s:ComponentSender<Self>,future:impl std::future::Future<Output=anyhow::Result<Saved>>+Send+'static){
        let Some(v)=&self.webhooks.view else{return;};if self.webhooks.job.is_some(){return;}
        let view=v.token;v.controls.set_sensitive(false);v.status.set_text("Salvando…");
        let output=s.input_sender().clone();
        self.webhooks.job=Some(tokio::spawn(async move{let result=future.await;let _=output.send(MainWindowMsg::Webhook(WebhookMsg::Saved{view,result}));}));
    }
    fn webhook_rows(&self,s:&ComponentSender<Self>){
        let Some(v)=&self.webhooks.view else{return;};while let Some(child)=v.list.first_child(){v.list.remove(&child);}
        if v.items.is_empty(){v.list.append(&gtk::Label::new(Some("Nenhum webhook ativo.")));}
        for hook in &v.items {
            let row=gtk::Box::new(gtk::Orientation::Vertical,4);row.add_css_class("card");row.set_margin_top(4);row.set_margin_bottom(4);
            let channel=self.managed_channels.iter().find(|c|c.id==hook.channel_id).map(|c|c.name.clone()).unwrap_or_else(||hook.channel_id.to_string());
            let label=gtk::Label::new(Some(&format!("{} · #{}",hook.display_name.as_deref().unwrap_or(&hook.name),channel)));label.set_xalign(0.0);label.set_ellipsize(gtk::pango::EllipsizeMode::End);row.append(&label);
            let actions=gtk::Box::new(gtk::Orientation::Horizontal,6);let id=hook.id;
            button(&actions,"Editar identidade",s,move ||WebhookMsg::Edit(id));button(&actions,"Revogar webhook",s,move ||WebhookMsg::Revoke(id));row.append(&actions);v.list.append(&row);
        }
    }
    pub(super) fn webhook_event(&mut self,msg:WebhookMsg,s:ComponentSender<Self>,root:&gtk::Box){
        if let WebhookMsg::Close(token)=msg{if self.webhooks.view.as_ref().is_some_and(|v|v.token==token){self.webhooks=Default::default();}return;}
        if !self.server_access.manage_webhooks{self.sync_webhook_access();return;}
        match msg {
            WebhookMsg::Open=>{
                if let Some(v)=&self.webhooks.view{v.window.present();return;}
                let (window,body)=window(root,"Webhooks");window.set_default_size(540,620);
                let token=Uuid::new_v4();let output=s.input_sender().clone();window.connect_close_request(move |_|{let _=output.send(MainWindowMsg::Webhook(WebhookMsg::Close(token)));gtk::glib::Propagation::Proceed});
                let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);
                let scroll=gtk::ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();let controls=gtk::Box::new(gtk::Orientation::Vertical,10);scroll.set_child(Some(&controls));body.append(&scroll);
                let name=entry(&controls,"Nome do novo webhook (até 80 caracteres)");name.set_max_length(80);
                let channels:Vec<_>=self.managed_channels.iter().filter(|c|matches!(c.channel_type,Some(ChannelType::Text|ChannelType::Voice))).collect();
                let names:Vec<_>=channels.iter().map(|c|c.name.as_str()).collect();let channel=gtk::DropDown::from_strings(&names);controls.append(&channel);
                button(&controls,"Criar webhook",&s,||WebhookMsg::Create);
                let credential=gtk::Box::new(gtk::Orientation::Vertical,6);credential.set_visible(false);
                let warning=gtk::Label::new(Some("Copie o token agora: ele só é mostrado uma vez. A integração deve enviá-lo no cabeçalho Authorization: Bearer <token>."));warning.set_wrap(true);credential.append(&warning);
                let endpoint=entry(&credential,"Endereço de envio");endpoint.set_editable(false);
                let secret=gtk::PasswordEntry::new();secret.set_editable(false);secret.set_tooltip_text(Some("Token do webhook"));secret.set_show_peek_icon(true);credential.append(&secret);
                for (label,widget) in [("Copiar endereço",endpoint.clone().upcast::<gtk::Widget>()),("Copiar token",secret.clone().upcast::<gtk::Widget>())]{let copy=gtk::Button::with_label(label);let field=widget;let weak=window.downgrade();copy.connect_clicked(move |_|{if let Some(w)=weak.upgrade(){let text=if let Some(e)=field.downcast_ref::<gtk::Entry>(){e.text()}else{field.downcast_ref::<gtk::PasswordEntry>().unwrap().text()};w.clipboard().set_text(&text);}});credential.append(&copy);}
                controls.append(&credential);controls.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                let list=gtk::Box::new(gtk::Orientation::Vertical,6);controls.append(&list);
                let form=gtk::Box::new(gtk::Orientation::Vertical,6);let nickname=entry(&form,"Apelido (vazio usa o nome original)");nickname.set_max_length(100);
                button(&form,"Escolher avatar",&s,||WebhookMsg::Avatar(false));button(&form,"Remover avatar",&s,||WebhookMsg::Avatar(true));button(&form,"Salvar identidade",&s,||WebhookMsg::Save);form.set_visible(false);controls.append(&form);
                button(&controls,"Atualizar webhooks",&s,||WebhookMsg::Reload);
                self.webhooks.view=Some(View{token,window:window.clone(),controls,status,list,items:Vec::new(),name,channel,channel_ids:channels.iter().map(|c|c.id).collect(),endpoint,secret,credential,form,edit:None,nickname,avatar:None});
                window.present();s.input(MainWindowMsg::Webhook(WebhookMsg::Reload));
            }
            WebhookMsg::Reload=>{
                if self.webhooks.job.is_some(){return;}let Some(v)=&self.webhooks.view else{return;};let view=v.token;v.controls.set_sensitive(false);v.status.set_text("Carregando webhooks…");let api=self.api_client.clone();
                let output=s.input_sender().clone();
                self.webhooks.job=Some(tokio::spawn(async move{let result=api.list_webhooks().await;let _=output.send(MainWindowMsg::Webhook(WebhookMsg::Loaded{view,result}));}));
            }
            WebhookMsg::Loaded{view,result}=>{
                let Some(v)=self.webhooks.view.as_mut().filter(|v|v.token==view)else{return;};self.webhooks.job=None;v.controls.set_sensitive(true);
                match result{Ok(items)=>{v.items=items;v.status.set_text("");self.webhook_rows(&s);},Err(e)=>{self.webhook_status(&e.to_string());s.input(MainWindowMsg::ActionError(e));}}
            }
            WebhookMsg::Create=>{
                let Some(v)=&self.webhooks.view else{return;};let Some(channel)=v.channel_ids.get(v.channel.selected() as usize).copied()else{self.webhook_status("Selecione um canal de texto ou voz.");return;};
                let name=v.name.text().to_string();let api=self.api_client.clone();self.webhook_job(s,async move{Ok(Saved::Created(api.create_webhook(channel,&name).await?))});
            }
            WebhookMsg::Edit(id)=>{
                if self.webhooks.job.is_some(){return;}let Some(v)=&mut self.webhooks.view else{return;};let Some(hook)=v.items.iter().find(|h|h.id==id)else{return;};v.edit=Some(id);v.avatar=None;v.nickname.set_text(hook.display_name.as_deref().unwrap_or(""));v.form.set_visible(true);v.status.set_text(&format!("Identidade de {}",hook.name));
            }
            WebhookMsg::Save=>{
                let Some(v)=&self.webhooks.view else{return;};let Some(id)=v.edit else{return;};let mut identity=WebhookIdentity{display_name:Some(v.nickname.text().trim().to_owned()),..Default::default()};if let Some((blob,format))=&v.avatar{identity.avatar=Some(blob.clone());identity.avatar_format=Some(format.clone());}
                let api=self.api_client.clone();self.webhook_job(s,async move{Ok(Saved::Updated(api.update_webhook(id,&identity).await?))});
            }
            WebhookMsg::Avatar(remove)=>{
                if self.webhooks.job.is_some(){return;}let Some(v)=&mut self.webhooks.view else{return;};let Some(edit)=v.edit else{return;};
                if remove{v.avatar=Some((String::new(),String::new()));v.status.set_text("O avatar será removido ao salvar.");return;}
                let view=v.token;let parent=v.window.clone();let output=s.input_sender().clone();
                gtk::glib::spawn_future_local(async move{let dialog=gtk::FileDialog::builder().title("Avatar (até 512 × 512, 2 MiB)").build();if let Ok(file)=dialog.open_future(Some(&parent)).await{if let Some(path)=file.path(){tokio::spawn(async move{let result=async{anyhow::ensure!(tokio::fs::metadata(&path).await?.len()<=2<<20,"Avatar de até 2 MiB.");Ok(tokio::fs::read(path).await?)}.await;let _=output.send(MainWindowMsg::Webhook(WebhookMsg::AvatarLoaded{view,edit,result}));});}}});
            }
            WebhookMsg::AvatarLoaded{view,edit,result}=>{
                let Some(v)=self.webhooks.view.as_mut().filter(|v|v.token==view&&v.edit==Some(edit))else{return;};if self.webhooks.job.is_some(){return;}
                match result.and_then(|bytes|{use base64::Engine;let format=crate::api::features::validate_profile_image(&bytes,false)?.to_owned();Ok((base64::engine::general_purpose::STANDARD.encode(bytes),format))}){Ok(avatar)=>{v.avatar=Some(avatar);v.status.set_text("Avatar escolhido. Salve a identidade para aplicar.");},Err(e)=>v.status.set_text(&e.to_string())}
            }
            WebhookMsg::Revoke(id)=>{
                if self.webhooks.job.is_some()||self.webhooks.dialog.is_some(){return;}let Some(v)=&self.webhooks.view else{return;};if !v.items.iter().any(|h|h.id==id){return;}
                let dialog=adw::AlertDialog::new(Some("Revogar webhook?"),Some("A integração perderá o acesso imediatamente. As mensagens existentes serão preservadas."));dialog.add_responses(&[("cancel","Cancelar"),("revoke","Revogar")]);dialog.set_response_appearance("revoke",adw::ResponseAppearance::Destructive);dialog.set_default_response(Some("cancel"));dialog.set_close_response("cancel");let view=v.token;let output=s.input_sender().clone();dialog.connect_response(None,move |_,response|{let msg=if response=="revoke"{WebhookMsg::ConfirmRevoke{view,id}}else{WebhookMsg::DismissRevoke(view)};let _=output.send(MainWindowMsg::Webhook(msg));});dialog.present(Some(&v.window));self.webhooks.dialog=Some(dialog);
            }
            WebhookMsg::DismissRevoke(view)=>{if self.webhooks.view.as_ref().is_some_and(|v|v.token==view){self.webhooks.dialog=None;}},
            WebhookMsg::ConfirmRevoke{view,id}=>{
                if !self.webhooks.view.as_ref().is_some_and(|v|v.token==view&&v.items.iter().any(|h|h.id==id)){return;}self.webhooks.dialog=None;let api=self.api_client.clone();self.webhook_job(s,async move{api.revoke_webhook(id).await?;Ok(Saved::Revoked(id))});
            }
            WebhookMsg::Saved{view,result}=>{
                let Some(v)=self.webhooks.view.as_mut().filter(|v|v.token==view)else{return;};self.webhooks.job=None;v.controls.set_sensitive(true);
                match result {
                    Ok(saved)=>{match saved{
                        Saved::Created(created)=>{v.endpoint.set_text(&self.api_client.webhook_endpoint(created.webhook.id));v.secret.set_text(&created.token);v.credential.set_visible(true);v.name.set_text("");v.items.insert(0,created.webhook);},
                        Saved::Updated(hook)=>{self.chat.emit(ChatMsg::Transfer(crate::ui::chat::transfers::TransferMsg::InvalidateWebhookAvatar(hook.id)));if let Some(item)=v.items.iter_mut().find(|h|h.id==hook.id){*item=hook;}v.avatar=None;},
                        Saved::Revoked(id)=>{v.items.retain(|h|h.id!=id);if v.edit==Some(id){v.edit=None;v.form.set_visible(false);}if v.endpoint.text()==self.api_client.webhook_endpoint(id){v.secret.set_text("");v.credential.set_visible(false);}},
                    }v.status.set_text("Salvo.");self.webhook_rows(&s);},
                    Err(e)=>{v.status.set_text(&e.to_string());s.input(MainWindowMsg::ActionError(e));}
                }
            }
            WebhookMsg::Close(_)=>{},
        }
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants,find_button};
    use serde_json::json;
    use base64::Engine;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let role=Uuid::new_v4();
    {let mut b=backend.lock().unwrap();b.server_value["owner_id"]=serde_json::Value::Null;b.profile["roles"]=json!([{"id":role,"name":"Integrations"}]);b.admin_roles=vec![json!({"id":role,"name":"Integrations","created_at":"2026-10-14T20:00:00Z","permissions":{"manage_webhooks":true}})];}
    let user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().background_started&&main.model().active_channel_id.is_some()&&main.model().access_request.is_none());
    assert!(main.model().server_access.manage_webhooks&&!main.model().server_access.manage_server&&!main.model().server_access.manage_channels);
    assert!(!backend.lock().unwrap().requests.iter().any(|(_,path,_)|path=="/webhooks"),"webhooks are loaded on demand");
    backend.lock().unwrap().fail_webhooks=true;
    main.emit(MainWindowMsg::Webhook(WebhookMsg::Open));until(context,||main.model().webhooks.view.is_some()&&main.model().webhooks.job.is_none());
    assert!(main.model().webhooks.view.as_ref().unwrap().status.text().contains("Permission denied"));backend.lock().unwrap().fail_webhooks=false;
    main.emit(MainWindowMsg::Webhook(WebhookMsg::Reload));until(context,||main.model().webhooks.job.is_none());
    let window=main.model().webhooks.view.as_ref().unwrap().window.clone();
    main.model().webhooks.view.as_ref().unwrap().name.set_text("Discord bridge");
    find_button(window.upcast_ref(),"Criar webhook").emit_clicked();find_button(window.upcast_ref(),"Criar webhook").emit_clicked();
    until(context,||main.model().webhooks.job.is_none()&&main.model().webhooks.view.as_ref().is_some_and(|v|v.items.len()==1));
    let id={let m=main.model();let v=m.webhooks.view.as_ref().unwrap();assert!(v.credential.is_visible());assert_eq!(v.secret.text(),"TEST_WEBHOOK_TOKEN");assert!(v.endpoint.text().ends_with(&format!("/webhooks/{}/messages",v.items[0].id)));v.items[0].id};
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="POST"&&path=="/webhooks").count(),1);
    main.emit(MainWindowMsg::Webhook(WebhookMsg::Edit(id)));pump(context);
    let token=main.model().webhooks.view.as_ref().unwrap().token;
    main.model().webhooks.view.as_ref().unwrap().nickname.set_text("Discord sender");
    let bytes=base64::engine::general_purpose::STANDARD.decode(crate::ui::chat::actions::tests::tiny_png_base64()).unwrap();
    main.emit(MainWindowMsg::Webhook(WebhookMsg::AvatarLoaded{view:token,edit:id,result:Ok(bytes.clone())}));pump(context);
    find_button(window.upcast_ref(),"Salvar identidade").emit_clicked();until(context,||main.model().webhooks.job.is_none()&&main.model().webhooks.view.as_ref().unwrap().items[0].display_name.as_deref()==Some("Discord sender"));
    let request=backend.lock().unwrap().requests.iter().find(|(method,path,_)|method=="PATCH"&&path==&format!("/webhooks/{id}")).unwrap().2.clone();assert_eq!(request["avatar_format"],"PNG");assert!(!request["avatar"].as_str().unwrap().is_empty());
    // New machine messages retain their actual sender, avatar and live edits.
    let channel=main.model().active_channel_id.unwrap();let avatar_path=format!("/webhooks/{id}/avatar");let stored=json!({"id":Uuid::new_v4(),"channel_id":channel,"author_id":"","webhook_id":id,"external_author":{"source":"discord","id":"987654321012345678","name":"Remote sender","avatar_url":avatar_path},"content":"Webhook live message","created_at":chrono::Utc::now()});let message:Message=serde_json::from_value(stored.clone()).unwrap();
    backend.lock().unwrap().channel_messages.insert(channel.to_string(),vec![stored.clone()]);
    backend.lock().unwrap().attachment_thumbnails.insert(avatar_path.clone(),bytes);
    main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(message.clone())));
    let avatar_loaded=||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).any(|a|a.text().as_deref()==Some("Remote sender")&&a.custom_image().is_some());
    until(context,avatar_loaded);
    let labels=||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Label>()).map(|l|l.text().to_string()).collect::<Vec<_>>();
    assert!(labels().iter().any(|l|l=="Remote sender"));assert!(labels().iter().any(|l|l=="APP"));
    main.emit(MainWindowMsg::WsReceived(WsEvent::MessageEdit{id:message.id,channel_id:Some(channel),content:"Webhook edited".into(),edited_at:Some(chrono::Utc::now())}));until(context,||descendants(main.model().chat.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::TextView>()).any(|v|{let b=v.buffer();b.text(&b.start_iter(),&b.end_iter(),true).contains("Webhook edited")}));
    assert!(avatar_loaded());
    // Reusing this sender in another message must hit the avatar cache.
    let mut next=message.clone();next.id=Uuid::new_v4();next.created_at+=chrono::Duration::seconds(1);main.emit(MainWindowMsg::WsReceived(WsEvent::NewMessage(next)));pump(context);
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p==&avatar_path).count(),1);
    // Returning to a channel reuses the tiny decoded avatar without HTTP/decode work.
    let original=main.model().channels.iter().find(|c|c.id==channel).unwrap().clone();let other=main.model().channels.iter().find(|c|c.id!=channel).unwrap().clone();let other_id=other.id;
    main.emit(MainWindowMsg::ChannelSelected(other));until(context,||main.model().active_channel_id==Some(other_id));
    main.emit(MainWindowMsg::ChannelSelected(original));until(context,avatar_loaded);
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="GET"&&p==&avatar_path).count(),1);
    main.emit(MainWindowMsg::Webhook(WebhookMsg::Revoke(id)));pump(context);assert!(main.model().webhooks.dialog.is_some());main.emit(MainWindowMsg::Webhook(WebhookMsg::ConfirmRevoke{view:Uuid::new_v4(),id}));pump(context);assert!(main.model().webhooks.dialog.is_some(),"stale confirmations must not dismiss the current dialog");main.model().webhooks.dialog.as_ref().unwrap().emit_by_name::<()>("response",&[&"cancel"]);pump(context);assert!(main.model().webhooks.dialog.is_none());assert_eq!(backend.lock().unwrap().webhooks.len(),1);
    main.emit(MainWindowMsg::Webhook(WebhookMsg::Revoke(id)));pump(context);main.model().webhooks.dialog.as_ref().unwrap().emit_by_name::<()>("response",&[&"revoke"]);until(context,||main.model().webhooks.job.is_none()&&backend.lock().unwrap().webhooks.is_empty());
    {let m=main.model();let v=m.webhooks.view.as_ref().unwrap();assert!(v.secret.text().is_empty());assert!(!v.credential.is_visible());}
    // Losing the independent permission closes the manager and clears secrets.
    backend.lock().unwrap().admin_roles[0]["permissions"]["manage_webhooks"]=json!(false);main.emit(MainWindowMsg::RefreshAccess);
    until(context,||main.model().access_request.is_none()&&!main.model().server_access.manage_webhooks&&main.model().webhooks.view.is_none());
    let count=backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p=="/webhooks").count();main.emit(MainWindowMsg::Webhook(WebhookMsg::Open));pump(context);assert!(main.model().webhooks.view.is_none());assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p=="/webhooks").count(),count);
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
