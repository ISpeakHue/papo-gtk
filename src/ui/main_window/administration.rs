//! Server setup and permission-gated role/channel administration.
use super::*;
use crate::models::*;
use crate::ui::chat::actions::window;
#[derive(Debug)]
pub enum AdminMsg {
    Open, Reload, EditRole(Option<Uuid>), EditChannel(Option<Uuid>), Member,
    SaveRole, SaveServer, SaveChannel, MoveChannel, SaveOverride, RemoveOverride,
    Assign(bool), DeleteRole(Uuid,bool), DeleteChannel(Uuid,bool),
    Icon(bool), IconLoaded{token:Uuid,result:anyhow::Result<Vec<u8>>},
    Loaded{token:Uuid,result:anyhow::Result<(Vec<Role>,Vec<Channel>,Vec<UserSummary>)>},
    Saved{token:Uuid,result:anyhow::Result<Outcome>},
}
#[derive(Debug)]
pub enum Outcome { Saved, RoleSaved(Role), ServerSaved{active:bool,server:Server}, CreatedServer{warning:Option<String>}, ChannelSaved(Channel), CreatedChannel(Channel) }
#[derive(Default)]
pub(super) struct Administration {
    view:Option<AdminView>,dialogs:Vec<gtk::Window>,jobs:Vec<tokio::task::JoinHandle<()>>,pending:Option<Uuid>,
    load:Option<Uuid>,roles:Vec<Role>,channels:Vec<Channel>,users:Vec<UserSummary>,
    pub reopen:bool,
}
struct Select {widget:gtk::DropDown,ids:Vec<Uuid>}
impl Select {
    fn new(items:impl Iterator<Item=(Uuid,String)>)->Self{let items:Vec<_>=items.collect();let labels:Vec<_>=items.iter().map(|(_,s)|s.as_str()).collect();Self{widget:gtk::DropDown::from_strings(&labels),ids:items.iter().map(|(id,_)|*id).collect()}}
    fn id(&self)->Option<Uuid>{self.ids.get(self.widget.selected() as usize).copied()}
}
struct AdminView {
    token:Uuid,window:gtk::Window,status:gtk::Label,stack:gtk::Stack,
    server_body:gtk::Box,role_body:gtk::Box,channel_body:gtk::Box,member_body:gtk::Box,
    role_list:gtk::Box,channel_list:gtk::Box,role_form:gtk::Box,channel_form:gtk::Box,member_form:gtk::Box,
    server:Option<ServerForm>,role:Option<RoleForm>,channel:Option<ChannelForm>,member:Option<MemberForm>,
}
struct ServerForm {name:gtk::Entry,public:gtk::CheckButton,password:gtk::PasswordEntry,change_password:gtk::CheckButton,
    baseline:Option<Server>,icon:Option<(String,String)>,preview:gtk::Image,creating:bool}
struct RoleForm {id:Option<Uuid>,name:gtk::Entry,color:gtk::Entry,checks:Vec<gtk::CheckButton>}
struct ChannelForm {id:Option<Uuid>,name:gtk::Entry,topic:gtk::Entry,kind:gtk::DropDown,position:gtk::SpinButton,old_position:i32,role:Select,checks:Vec<gtk::CheckButton>}
struct MemberForm {user:Select,role:Select,summary:gtk::Label}
impl Drop for Administration {fn drop(&mut self){for j in self.jobs.drain(..){j.abort();}for w in self.dialogs.drain(..){w.close();}if let Some(v)=self.view.take(){v.window.close();}}}
fn entry(body:&gtk::Box,label:&str,value:&str)->gtk::Entry{let l=gtk::Label::new(Some(label));l.set_xalign(0.0);body.append(&l);let e=gtk::Entry::new();e.set_text(value);body.append(&e);e}
fn checks(body:&gtk::Box,values:&[(&str,bool)])->Vec<gtk::CheckButton>{values.iter().map(|(label,value)|{let c=gtk::CheckButton::with_label(label);c.set_active(*value);body.append(&c);c}).collect()}
fn button(body:&gtk::Box,label:&str,sender:&ComponentSender<MainWindowModel>,f:impl Fn()->AdminMsg+'static){let b=gtk::Button::with_label(label);let s=sender.clone();b.connect_clicked(move |_|s.input(MainWindowMsg::Administration(f())));body.append(&b);}
fn clear(body:&gtk::Box){while let Some(c)=body.first_child(){body.remove(&c);}}
fn valid_name(name:&str)->anyhow::Result<()>{anyhow::ensure!(!name.trim().is_empty()&&name.chars().count()<=32,"Use um nome de 1 a 32 caracteres.");Ok(())}
fn role_request(f:&RoleForm)->anyhow::Result<CreateRoleRequest>{
    let name=f.name.text().trim().to_owned();valid_name(&name)?;let color=f.color.text().trim().to_owned();
    anyhow::ensure!(color.is_empty()||color.len()==7&&color.starts_with('#')&&color[1..].bytes().all(|c|c.is_ascii_hexdigit()),"Use uma cor #RRGGBB ou deixe vazio.");
    let b:Vec<_>=f.checks.iter().map(|c|Some(c.is_active())).collect();
    Ok(CreateRoleRequest{name,color:(!color.is_empty()).then_some(color),permissions:Some(RolePermissions{manage_server:b[0],manage_channels:b[1],manage_roles:b[2],ban_members:b[3],pin_message:b[4],everyone_message:b[5],send_attachment:b[6]})})
}
fn server_request(f:&ServerForm)->anyhow::Result<ServerWrite>{
    let name=f.name.text().trim().to_owned();valid_name(&name)?;
    let mut r=ServerWrite::default();
    if f.creating||f.baseline.as_ref().is_some_and(|s|s.name!=name){r.name=Some(name);}
    let public=f.public.is_active();if f.creating||f.baseline.as_ref().is_some_and(|s|s.public.unwrap_or(true)!=public){r.public=Some(public);}
    if f.change_password.is_active(){r.password=Some(f.password.text().to_string());}
    if !public&&(f.creating||f.baseline.as_ref().is_some_and(|s|s.public.unwrap_or(true))){anyhow::ensure!(r.password.as_ref().is_some_and(|s|!s.is_empty()),"Defina uma senha para criar ou tornar o servidor privado.");}
    if let Some((blob,format))=&f.icon{r.icon_blob=Some(blob.clone());r.icon_format=Some(format.clone());}
    Ok(r)
}
fn override_request(f:&ChannelForm)->ChannelPermissions{let b:Vec<_>=f.checks.iter().map(|c|Some(c.is_active())).collect();ChannelPermissions{read_channel:b[0],send_messages:b[1],delete_messages:b[2],connect_voice:b[3]}}
impl MainWindowModel {
    pub(super) fn admin_member_summary(&mut self){
        if let Some(f)=self.administration.view.as_ref().and_then(|v|v.member.as_ref()){
            if let Some(u)=f.user.id().and_then(|id|self.users.iter().find(|u|u.id==id)){f.summary.set_text(&format!("Cargos atuais: {}",u.roles.iter().flatten().map(|r|r.name.as_str()).collect::<Vec<_>>().join(", ")));}
        }
    }
    pub(super) fn admin_refresh_assignments(&mut self,sender:ComponentSender<Self>){if self.admin_visible()&&self.administration.pending.is_none(){self.admin_reload(sender);}}
    pub(super) fn admin_refresh_lists(&mut self,sender:&ComponentSender<Self>){
        if self.admin_visible()&&self.administration.load.is_none()&&self.administration.pending.is_none(){self.administration.roles=self.roles.clone();self.administration.channels=self.managed_channels.clone();self.administration.users=self.users.clone();self.admin_lists(sender);if let Some(f)=self.administration.view.as_mut().and_then(|v|v.channel.as_mut()){if let Some(c)=f.id.and_then(|id|self.managed_channels.iter().find(|c|c.id==id)){f.old_position=c.position.unwrap_or(f.old_position);}}}
    }

    pub(super) fn admin_visible(&self)->bool{self.administration.view.as_ref().is_some_and(|v|v.window.is_visible())}
    pub(super) fn sync_admin_access(&self){
        if let Some(v)=&self.administration.view{
            let idle=self.administration.pending.is_none();
            v.server_body.set_sensitive(idle&&(self.setup||self.server_access.manage_server));
            v.role_body.set_sensitive(idle&&self.server_access.manage_roles);
            v.member_body.set_sensitive(idle&&self.server_access.manage_roles);
            v.channel_body.set_sensitive(idle&&self.server_access.manage_channels);
        }
    }
    fn admin_error(&self,message:&str){if let Some(v)=&self.administration.view{v.status.set_text(message);}}
    fn admin_reload(&mut self,sender:ComponentSender<Self>){
        let token=Uuid::new_v4();self.administration.load=Some(token);let api=self.api_client.clone();
        self.administration.jobs.retain(|j|!j.is_finished());
        self.administration.jobs.push(tokio::spawn(async move{let result=async{Ok((api.list_roles().await?,api.list_channels().await?,api.list_all_users().await?))}.await;sender.input(MainWindowMsg::Administration(AdminMsg::Loaded{token,result}));}));
    }
    fn admin_lists(&mut self,sender:&ComponentSender<Self>){
        let Some(v)=&self.administration.view else{return;};clear(&v.role_list);clear(&v.channel_list);
        for r in &self.administration.roles{let row=gtk::Box::new(gtk::Orientation::Horizontal,4);let id=r.id;button(&row,&format!("{} {}",r.name,r.color.as_deref().unwrap_or("")),sender,move ||AdminMsg::EditRole(Some(id)));button(&row,"Excluir cargo",sender,move ||AdminMsg::DeleteRole(id,false));v.role_list.append(&row);}
        let mut channels=self.administration.channels.clone();channels.sort_by_key(|c|c.position.unwrap_or(0));
        for c in channels{let row=gtk::Box::new(gtk::Orientation::Horizontal,4);let id=c.id;button(&row,&format!("{} · {} ({:?})",c.position.unwrap_or(0),c.name,c.channel_type),sender,move ||AdminMsg::EditChannel(Some(id)));button(&row,"Excluir canal",sender,move ||AdminMsg::DeleteChannel(id,false));v.channel_list.append(&row);}
        self.sync_admin_access();
    }
    fn admin_mutation(&mut self,sender:ComponentSender<Self>,future:impl std::future::Future<Output=anyhow::Result<Outcome>>+Send+'static){
        if self.administration.pending.is_some(){return;}let Some(v)=&self.administration.view else{return;};let token=v.token;
        self.administration.pending=Some(token);self.administration.load=None;self.admin_error("Salvando…");self.sync_admin_access();
        self.administration.jobs.push(tokio::spawn(async move{let result=future.await;sender.input(MainWindowMsg::Administration(AdminMsg::Saved{token,result}));}));
    }
    pub(super) fn admin_event(&mut self,msg:AdminMsg,sender:ComponentSender<Self>,root:&gtk::Box){
        match msg {
            AdminMsg::Open=>{
                if !self.setup&&!self.server_access.manage_server&&!self.server_access.manage_roles&&!self.server_access.manage_channels{return;}
                if self.administration.pending.is_some(){self.admin_error("Aguarde a operação atual.");return;}
                if let Some(v)=self.administration.view.take(){v.window.close();}
                self.administration.roles=self.roles.clone();self.administration.channels=self.managed_channels.clone();self.administration.users=self.users.clone();
                let (window,body)=window(root,if self.setup{"Criar servidor"}else{"Administração"});window.set_default_size(650,700);
                let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);
                let stack=gtk::Stack::new();let switcher=gtk::StackSwitcher::new();switcher.set_stack(Some(&stack));body.append(&switcher);
                let pages:Vec<_>=["Servidor","Cargos","Canais","Membros"].iter().map(|title|{let page=gtk::Box::new(gtk::Orientation::Vertical,8);let scroll=gtk::ScrolledWindow::builder().vexpand(true).child(&page).build();stack.add_titled(&scroll,Some(title),title);page}).collect();body.append(&stack);
                button(&body,"Atualizar administração",&sender,||AdminMsg::Reload);
                let role_list=gtk::Box::new(gtk::Orientation::Vertical,4);let role_form=gtk::Box::new(gtk::Orientation::Vertical,4);button(&pages[1],"Novo cargo",&sender,||AdminMsg::EditRole(None));pages[1].append(&role_list);pages[1].append(&role_form);
                let channel_list=gtk::Box::new(gtk::Orientation::Vertical,4);let channel_form=gtk::Box::new(gtk::Orientation::Vertical,4);button(&pages[2],"Novo canal",&sender,||AdminMsg::EditChannel(None));pages[2].append(&channel_list);pages[2].append(&channel_form);
                let member_form=gtk::Box::new(gtk::Orientation::Vertical,4);button(&pages[3],"Escolher membro e cargo",&sender,||AdminMsg::Member);pages[3].append(&member_form);
                let name=entry(&pages[0],"Nome do servidor",self.server.as_ref().map(|s|s.name.as_str()).unwrap_or(""));
                let public=gtk::CheckButton::with_label("Servidor público");public.set_active(self.server.as_ref().and_then(|s|s.public).unwrap_or(true));pages[0].append(&public);
                let change_password=gtk::CheckButton::with_label("Definir / alterar senha do servidor");pages[0].append(&change_password);
                let password=gtk::PasswordEntry::new();password.set_show_peek_icon(true);pages[0].append(&password);
                let notice=gtk::Label::new(Some("Alterar ou remover a senha encerra todas as sessões, incluindo a sua. Um servidor privado exige senha."));notice.set_wrap(true);pages[0].append(&notice);
                let preview=gtk::Image::new();preview.set_pixel_size(64);if let Some(t)=self.server.as_ref().and_then(|s|s.icon_blob.as_deref()).and_then(crate::media::texture_from_base64){preview.set_paintable(Some(&t));}pages[0].append(&preview);
                button(&pages[0],"Escolher ícone",&sender,||AdminMsg::Icon(false));button(&pages[0],"Remover ícone",&sender,||AdminMsg::Icon(true));
                button(&pages[0],if self.setup{"Criar servidor e canal inicial"}else{"Salvar servidor"},&sender,||AdminMsg::SaveServer);
                let server=Some(ServerForm{name,public,password,change_password,baseline:self.server.clone(),icon:None,preview,creating:self.setup});
                window.present();self.administration.view=Some(AdminView{token:Uuid::new_v4(),window,status,stack,server_body:pages[0].clone(),role_body:pages[1].clone(),channel_body:pages[2].clone(),member_body:pages[3].clone(),role_list,channel_list,role_form,channel_form,member_form,server,role:None,channel:None,member:None});
                self.admin_lists(&sender);if !self.setup{self.admin_reload(sender);}
            }
            AdminMsg::Reload=>{self.refresh_channels(sender.clone());self.refresh_users(sender.clone(),None);self.admin_reload(sender);}
            AdminMsg::Loaded{token,result}=>{
                if self.administration.load!=Some(token){return;}self.administration.load=None;
                match result{Ok((roles,channels,users))=>{self.administration.roles=roles;self.administration.channels=channels;self.administration.users=users;self.admin_lists(&sender);},Err(e)=>{self.admin_error(&e.to_string());sender.input(MainWindowMsg::ActionError(e));}}
            }
            AdminMsg::EditRole(id)=>{
                if !self.server_access.manage_roles||self.administration.pending.is_some(){return;}
                let role=id.and_then(|id|self.administration.roles.iter().find(|r|r.id==id));if id.is_some()&&role.is_none(){return;}
                let Some(v)=self.administration.view.as_mut()else{return;};clear(&v.role_form);v.stack.set_visible_child_name("Cargos");
                let name=entry(&v.role_form,"Nome do cargo",role.map(|r|r.name.as_str()).unwrap_or(""));let color=entry(&v.role_form,"Cor #RRGGBB (vazio remove)",role.and_then(|r|r.color.as_deref()).unwrap_or(""));
                let p=role.map(|r|&r.permissions);let values=[("Gerenciar servidor",p.and_then(|p|p.manage_server)),("Gerenciar canais",p.and_then(|p|p.manage_channels)),("Gerenciar cargos",p.and_then(|p|p.manage_roles)),("Banir membros (o backend exige gerenciar servidor)",p.and_then(|p|p.ban_members)),("Fixar mensagens",p.and_then(|p|p.pin_message)),("Mencionar todos",p.and_then(|p|p.everyone_message)),("Enviar anexos",p.and_then(|p|p.send_attachment))];
                let checks=checks(&v.role_form,&values.iter().map(|(l,b)|(*l,b.unwrap_or(false))).collect::<Vec<_>>());button(&v.role_form,"Salvar cargo",&sender,||AdminMsg::SaveRole);v.role=Some(RoleForm{id,name,color,checks});
            }
            AdminMsg::SaveRole=>{
                if !self.server_access.manage_roles||self.administration.pending.is_some(){return;}let Some(f)=self.administration.view.as_ref().and_then(|v|v.role.as_ref())else{return;};
                let request=match role_request(f){Ok(r)=>r,Err(e)=>{self.admin_error(&e.to_string());return;}};let id=f.id;let api=self.api_client.clone();
                self.admin_mutation(sender,async move{let role=if let Some(id)=id{api.update_role(id,&UpdateRoleRequest{name:request.name,color:request.color,permissions:request.permissions}).await?}else{api.create_role(&request).await?};Ok(Outcome::RoleSaved(role))});
            }
            AdminMsg::Member=>{
                if !self.server_access.manage_roles||self.administration.pending.is_some(){return;}let Some(v)=self.administration.view.as_mut()else{return;};clear(&v.member_form);v.stack.set_visible_child_name("Membros");
                let user=Select::new(self.administration.users.iter().map(|u|(u.id,u.display_name().to_owned())));let role=Select::new(self.administration.roles.iter().map(|r|(r.id,r.name.clone())));
                v.member_form.append(&gtk::Label::new(Some("Membro")));v.member_form.append(&user.widget);v.member_form.append(&gtk::Label::new(Some("Cargo")));v.member_form.append(&role.widget);
                let summary=gtk::Label::new(None);summary.set_wrap(true);v.member_form.append(&summary);
                let users=self.administration.users.clone();let ids=user.ids.clone();let label=summary.clone();user.widget.connect_selected_notify(move |w|{let u=ids.get(w.selected() as usize).and_then(|id|users.iter().find(|u|u.id==*id));label.set_text(&format!("Cargos atuais: {}",u.map(|u|u.roles.iter().flatten().map(|r|r.name.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default()));});
                button(&v.member_form,"Atribuir cargo",&sender,||AdminMsg::Assign(true));button(&v.member_form,"Remover cargo do membro",&sender,||AdminMsg::Assign(false));v.member=Some(MemberForm{user,role,summary});
                if let Some(f)=&v.member{if let Some(u)=f.user.id().and_then(|id|self.administration.users.iter().find(|u|u.id==id)){f.summary.set_text(&format!("Cargos atuais: {}",u.roles.iter().flatten().map(|r|r.name.as_str()).collect::<Vec<_>>().join(", ")));}}
            }
            AdminMsg::Assign(assign)=>{
                if !self.server_access.manage_roles||self.administration.pending.is_some(){return;}let Some(f)=self.administration.view.as_ref().and_then(|v|v.member.as_ref())else{return;};let(Some(user),Some(role))=(f.user.id(),f.role.id())else{self.admin_error("Selecione um membro e um cargo.");return;};let api=self.api_client.clone();
                self.admin_mutation(sender,async move{if assign{api.assign_role(user,role).await?;}else{api.remove_role(user,role).await?;}Ok(Outcome::Saved)});
            }
            AdminMsg::EditChannel(id)=>{
                if !self.server_access.manage_channels||self.administration.pending.is_some(){return;}
                let channel=id.and_then(|id|self.administration.channels.iter().find(|c|c.id==id));if id.is_some()&&channel.is_none(){return;}
                let Some(v)=self.administration.view.as_mut()else{return;};clear(&v.channel_form);v.stack.set_visible_child_name("Canais");
                let name=entry(&v.channel_form,"Nome do canal",channel.map(|c|c.name.as_str()).unwrap_or(""));let topic=entry(&v.channel_form,"Tópico (até 512 caracteres; categoria sem tópico)",channel.and_then(|c|c.topic.as_deref()).unwrap_or(""));
                let kind=gtk::DropDown::from_strings(&["Texto","Categoria","Voz"]);kind.set_selected(match channel.and_then(|c|c.channel_type.as_ref()){Some(ChannelType::Category)=>1,Some(ChannelType::Voice)=>2,_=>0});kind.set_sensitive(id.is_none());v.channel_form.append(&kind);
                let old_position=channel.and_then(|c|c.position).unwrap_or(1);let position=gtk::SpinButton::with_range(1.0,self.administration.channels.len().max(1) as f64,1.0);position.set_value(old_position as f64);v.channel_form.append(&gtk::Label::new(Some("Posição na lista (1 até o número de canais)")));v.channel_form.append(&position);
                button(&v.channel_form,"Salvar canal",&sender,||AdminMsg::SaveChannel);
                let role=Select::new(self.administration.roles.iter().map(|r|(r.id,r.name.clone())));
                let checks=checks(&v.channel_form,&[("Ler canal",false),("Enviar mensagens",false),("Excluir mensagens de outros",false),("Conectar à voz",false)]);
                if id.is_some(){button(&v.channel_form,"Mover canal",&sender,||AdminMsg::MoveChannel);v.channel_form.append(&gtk::Label::new(Some("Permissões do cargo selecionado (substituição completa)")));v.channel_form.append(&role.widget);
                    let permissions=channel.and_then(|c|c.permissions.clone()).unwrap_or_default();let ids=role.ids.clone();let buttons=checks.clone();role.widget.connect_selected_notify(move |w|{let p=ids.get(w.selected() as usize).and_then(|id|permissions.iter().find(|p|p.role_id==*id));let values=p.map(|p|[p.permissions.read_channel,p.permissions.send_messages,p.permissions.delete_messages,p.permissions.connect_voice]).unwrap_or([None;4]);for(c,b)in buttons.iter().zip(values){c.set_active(b.unwrap_or(false));}});
                    if let Some(p)=role.id().and_then(|id|channel.and_then(|c|c.permissions.as_ref()).and_then(|p|p.iter().find(|p|p.role_id==id))){for(c,b)in checks.iter().zip([p.permissions.read_channel,p.permissions.send_messages,p.permissions.delete_messages,p.permissions.connect_voice]){c.set_active(b.unwrap_or(false));}}
                    button(&v.channel_form,"Salvar permissões do cargo",&sender,||AdminMsg::SaveOverride);button(&v.channel_form,"Remover permissões do cargo",&sender,||AdminMsg::RemoveOverride);
                }
                v.channel=Some(ChannelForm{id,name,topic,kind,position,old_position,role,checks});
            }
            AdminMsg::SaveChannel=>{
                if !self.server_access.manage_channels||self.administration.pending.is_some(){return;}let Some(f)=self.administration.view.as_ref().and_then(|v|v.channel.as_ref())else{return;};let name=f.name.text().trim().to_owned();if let Err(e)=valid_name(&name){self.admin_error(&e.to_string());return;}
                let topic=f.topic.text().to_string();if topic.chars().count()>512||f.kind.selected()==1&&!topic.is_empty(){self.admin_error("Use até 512 caracteres no tópico; categorias não têm tópico.");return;}
                let id=f.id;let kind=match f.kind.selected(){1=>ChannelType::Category,2=>ChannelType::Voice,_=>ChannelType::Text};let api=self.api_client.clone();
                self.admin_mutation(sender,async move{if let Some(id)=id{Ok(Outcome::ChannelSaved(api.update_channel(id,&UpdateChannelRequest{name,topic:Some(topic)}).await?))}else{Ok(Outcome::CreatedChannel(api.create_channel(&CreateChannelRequest{name,topic:Some(topic),channel_type:Some(kind)}).await?))}});
            }
            AdminMsg::MoveChannel=>{
                if !self.server_access.manage_channels||self.administration.pending.is_some(){return;}let Some(f)=self.administration.view.as_ref().and_then(|v|v.channel.as_ref())else{return;};let Some(id)=f.id else{return;};let request=ChangeChannelPositionRequest{old_position:f.old_position,new_position:f.position.value_as_int()};let api=self.api_client.clone();self.admin_mutation(sender,async move{Ok(Outcome::ChannelSaved(api.move_channel(id,&request).await?))});
            }
            AdminMsg::SaveOverride|AdminMsg::RemoveOverride=>{
                if !self.server_access.manage_channels||self.administration.pending.is_some(){return;}let Some(f)=self.administration.view.as_ref().and_then(|v|v.channel.as_ref())else{return;};let(Some(channel),Some(role))=(f.id,f.role.id())else{self.admin_error("Selecione um canal existente e um cargo.");return;};let remove=matches!(msg,AdminMsg::RemoveOverride);let request=override_request(f);let api=self.api_client.clone();self.admin_mutation(sender,async move{if remove{api.remove_override(channel,role).await?;}else{api.set_override(channel,role,&request).await?;}Ok(Outcome::Saved)});
            }
            AdminMsg::DeleteRole(id,confirmed)|AdminMsg::DeleteChannel(id,confirmed)=>{
                let role=matches!(msg,AdminMsg::DeleteRole(..));if self.administration.pending.is_some()||if role{!self.server_access.manage_roles}else{!self.server_access.manage_channels}{return;}
                if !confirmed{let (w,b)=window(root,if role{"Excluir cargo?"}else{"Excluir canal e seu histórico?"});let warning=gtk::Label::new(Some(if role{"A exclusão remove este cargo de todos os membros e pode mudar o acesso aos canais."}else{"A exclusão remove o canal e seu histórico de mensagens."}));warning.set_wrap(true);b.append(&warning);let s=sender.clone();let confirm=gtk::Button::with_label("Confirmar exclusão");let dialog=w.clone();confirm.connect_clicked(move |_|{dialog.close();s.input(MainWindowMsg::Administration(if role{AdminMsg::DeleteRole(id,true)}else{AdminMsg::DeleteChannel(id,true)}));});b.append(&confirm);let cancel=gtk::Button::with_label("Cancelar");let dialog=w.clone();cancel.connect_clicked(move |_|dialog.close());b.append(&cancel);w.present();self.administration.dialogs.retain(|w|w.is_visible());self.administration.dialogs.push(w);return;}
                let api=self.api_client.clone();self.admin_mutation(sender,async move{if role{api.delete_role(id).await?;}else{api.delete_channel(id).await?;}Ok(Outcome::Saved)});
            }
            AdminMsg::SaveServer=>{
                if !self.setup&&!self.server_access.manage_server||self.administration.pending.is_some(){return;}let Some(f)=self.administration.view.as_ref().and_then(|v|v.server.as_ref())else{return;};let creating=f.creating;let request=match server_request(f){Ok(r)=>r,Err(e)=>{self.admin_error(&e.to_string());return;}};let api=self.api_client.clone();
                self.admin_mutation(sender,async move{
                    if creating{
                        api.create_server(&request).await?;
                        let warning=api.create_channel(&CreateChannelRequest{name:"geral".into(),channel_type:Some(ChannelType::Text),topic:None}).await.err().map(|e|format!("Servidor criado. Crie o canal inicial na aba Canais: {e}"));
                        Ok(Outcome::CreatedServer{warning})
                    }
                    else{let server=api.patch_server(&request).await?;let active=match api.whoami().await{Ok(_)=>true,Err(e)if crate::api::is_session_error(&e)=>false,Err(e)=>return Err(e)};Ok(Outcome::ServerSaved{active,server})}
                });
            }
            AdminMsg::Saved{token,result}=>{
                if self.administration.pending!=Some(token){return;}self.administration.pending=None;self.sync_admin_access();
                match result {
                    Ok(outcome)=>{
                        if let Some(f)=self.administration.view.as_ref().and_then(|v|v.server.as_ref()){f.password.set_text("");}
                        if matches!(outcome,Outcome::ServerSaved{active:false,..}){self.voice_disconnected();let _=sender.output(MainWindowOutput::SessionExpired);return;}
                        match outcome {
                            Outcome::CreatedServer{warning}=>{self.administration.reopen=true;if let Some(v)=self.administration.view.take(){v.window.close();}if let Some(warning)=warning{self.chat.emit(ChatMsg::OperationError(warning));}},
                            Outcome::ServerSaved{server,..}=>{if let Some(f)=self.administration.view.as_mut().and_then(|v|v.server.as_mut()){f.baseline=Some(server);f.icon=None;f.change_password.set_active(false);}},
                            Outcome::RoleSaved(role)=>{if let Some(f)=self.administration.view.as_mut().and_then(|v|v.role.as_mut()){f.id=Some(role.id);}},
                            Outcome::ChannelSaved(c)|Outcome::CreatedChannel(c)=>{if let Some(f)=self.administration.view.as_mut().and_then(|v|v.channel.as_mut()){f.id=Some(c.id);f.old_position=c.position.unwrap_or(1);f.kind.set_sensitive(false);} },
                            _=>{},
                        }
                        self.admin_error("Salvo. Atualizando permissões e membros…");self.refresh_channels(sender.clone());self.refresh_users(sender.clone(),None);self.admin_reload(sender);
                    }
                    Err(e)=>{self.admin_error(&e.to_string());sender.input(MainWindowMsg::ActionError(e));}
                }
            }
            AdminMsg::Icon(remove)=>{
                if !self.setup&&!self.server_access.manage_server||self.administration.pending.is_some(){return;}let Some(v)=self.administration.view.as_mut()else{return;};let Some(f)=v.server.as_mut()else{return;};
                if remove{f.icon=Some((String::new(),String::new()));f.preview.clear();return;}
                let token=v.token;let parent=v.window.clone();let s=sender.clone();
                gtk::glib::spawn_future_local(async move {
                    let dialog=gtk::FileDialog::builder().title("Ícone do servidor (até 2 MiB)").build();
                    if let Ok(file)=dialog.open_future(Some(&parent)).await {
                        if let Some(path)=file.path(){tokio::spawn(async move {
                            let result=async {
                                anyhow::ensure!(tokio::fs::metadata(&path).await?.len()<=2<<20,"Ícone de até 2 MiB");
                                Ok(tokio::fs::read(path).await?)
                            }.await;
                            s.input(MainWindowMsg::Administration(AdminMsg::IconLoaded{token,result}));
                        });}
                    }
                });
            }
            AdminMsg::IconLoaded{token,result}=>{
                if self.administration.pending.is_some(){return;}let Some(v)=self.administration.view.as_mut().filter(|v|v.token==token)else{return;};let Some(f)=v.server.as_mut()else{return;};
                match result{Err(e)=>v.status.set_text(&e.to_string()),Ok(bytes)=>{
                    use base64::Engine;
                    let format=match image::guess_format(&bytes){Ok(image::ImageFormat::Png)=>"PNG",Ok(image::ImageFormat::Jpeg)=>"JPEG",Ok(image::ImageFormat::Gif)=>"GIF",Ok(image::ImageFormat::WebP)=>"WEBP",_=>{v.status.set_text("Escolha PNG, JPEG, GIF ou WebP.");return;}};
                    if bytes.len()>2<<20{v.status.set_text("Ícone de até 2 MiB");return;}let Some(texture)=crate::media::bounded_texture(&bytes)else{v.status.set_text("Imagem inválida ou dimensões excessivas.");return;};f.preview.set_paintable(Some(&texture));f.icon=Some((base64::engine::general_purpose::STANDARD.encode(bytes),format.into()));
                }}
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{find_button,pump,until};
    main.emit(MainWindowMsg::Administration(AdminMsg::Open));until(context,||main.model().administration.load.is_none()&&main.model().administration.view.is_some());
    let window=main.model().administration.view.as_ref().unwrap().window.clone();
    find_button(window.upcast_ref(),"Novo cargo").emit_clicked();pump(context);
    let v=main.model();let f=v.administration.view.as_ref().unwrap().role.as_ref().unwrap();f.name.set_text("Moderator");f.color.set_text("#AB12CD");f.checks[1].set_active(true);drop(v);
    find_button(window.upcast_ref(),"Salvar cargo").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().administration.view.as_ref().unwrap().status.text().contains("Role failed"));
    assert_eq!(main.model().administration.view.as_ref().unwrap().role.as_ref().unwrap().name.text(),"Moderator");
    find_button(window.upcast_ref(),"Salvar cargo").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().administration.load.is_none()&&main.model().roles.len()==1);
    let role=main.model().roles[0].id;assert_eq!(main.model().roles[0].color.as_deref(),Some("#AB12CD"));
    main.emit(MainWindowMsg::Administration(AdminMsg::EditRole(Some(role))));pump(context);main.model().administration.view.as_ref().unwrap().role.as_ref().unwrap().name.set_text("Editor");main.emit(MainWindowMsg::Administration(AdminMsg::SaveRole));until(context,||main.model().administration.pending.is_none()&&main.model().administration.load.is_none()&&main.model().roles[0].name=="Editor");
    main.emit(MainWindowMsg::Administration(AdminMsg::Member));pump(context);find_button(window.upcast_ref(),"Atribuir cargo").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().current_user.roles.as_ref().is_some_and(|r|!r.is_empty()));find_button(window.upcast_ref(),"Remover cargo do membro").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().current_user.roles.as_ref().is_some_and(|r|r.is_empty()));
    main.emit(MainWindowMsg::Administration(AdminMsg::EditChannel(None)));pump(context);let v=main.model();let f=v.administration.view.as_ref().unwrap().channel.as_ref().unwrap();f.name.set_text("new-text");f.topic.set_text("Topic");drop(v);find_button(window.upcast_ref(),"Salvar canal").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().administration.load.is_none()&&main.model().channels.iter().any(|c|c.name=="new-text"));
    let c=main.model().channels.iter().find(|c|c.name=="new-text").unwrap().clone();main.emit(MainWindowMsg::Administration(AdminMsg::EditChannel(Some(c.id))));pump(context);main.model().administration.view.as_ref().unwrap().channel.as_ref().unwrap().position.set_value(1.0);find_button(window.upcast_ref(),"Mover canal").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().channels.iter().any(|c|c.name=="new-text"&&c.position==Some(1)));
    main.emit(MainWindowMsg::Administration(AdminMsg::EditChannel(Some(c.id))));pump(context);main.model().administration.view.as_ref().unwrap().channel.as_ref().unwrap().checks[0].set_active(true);find_button(window.upcast_ref(),"Salvar permissões do cargo").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none());find_button(window.upcast_ref(),"Remover permissões do cargo").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none());
    main.emit(MainWindowMsg::ChannelSelected(c.clone()));pump(context);main.emit(MainWindowMsg::Administration(AdminMsg::DeleteChannel(c.id,true)));until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&!main.model().channels.iter().any(|ch|ch.id==c.id));assert_ne!(main.model().active_channel_id,Some(c.id));
    for (name,kind) in [("Category",1),("Voice",2)]{main.emit(MainWindowMsg::Administration(AdminMsg::EditChannel(None)));pump(context);let v=main.model();let f=v.administration.view.as_ref().unwrap().channel.as_ref().unwrap();f.name.set_text(name);f.kind.set_selected(kind);drop(v);main.emit(MainWindowMsg::Administration(AdminMsg::SaveChannel));until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().administration.load.is_none()&&main.model().channels.iter().any(|c|c.name==name));let id=main.model().channels.iter().find(|c|c.name==name).unwrap().id;main.emit(MainWindowMsg::Administration(AdminMsg::DeleteChannel(id,true)));until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none());}
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(1,1).write_to(&mut bytes,image::ImageFormat::Png).unwrap();let token=main.model().administration.view.as_ref().unwrap().token;
    main.emit(MainWindowMsg::Administration(AdminMsg::IconLoaded{token,result:Ok(bytes.into_inner())}));pump(context);
    main.model().administration.view.as_ref().unwrap().server.as_ref().unwrap().name.set_text("Renamed server");find_button(window.upcast_ref(),"Salvar servidor").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().administration.view.as_ref().unwrap().status.text().contains("Server failed"));assert_eq!(main.model().administration.view.as_ref().unwrap().server.as_ref().unwrap().name.text(),"Renamed server");find_button(window.upcast_ref(),"Salvar servidor").emit_clicked();until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().server.as_ref().is_some_and(|s|s.name=="Renamed server"));
    assert!(main.model().server.as_ref().unwrap().icon_blob.is_some());
    assert!(crate::ui::chat::actions::tests::descendants(main.model().sidebar.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Image>()).any(|w|w.paintable().is_some()));
    main.emit(MainWindowMsg::Administration(AdminMsg::DeleteRole(role,true)));until(context,||main.model().administration.pending.is_none()&&main.model().access_request.is_none()&&main.model().roles.is_empty());window.close();
}
#[cfg(test)]
pub(crate) fn exercise_setup_and_password(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,find_button};use std::sync::{Arc,Mutex};
    let (api,backend,task)=gtk::glib::MainContext::default().block_on(mock());backend.lock().unwrap().server_missing=true;backend.lock().unwrap().admin_channels.clear();
    let user=gtk::glib::MainContext::default().block_on(api.whoami()).unwrap();let outputs=Arc::new(Mutex::new(Vec::new()));let capture=outputs.clone();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).connect_receiver(move|_,o|capture.lock().unwrap().push(o));
    until(context,||main.model().setup&&main.model().administration.view.is_some());let window=main.model().administration.view.as_ref().unwrap().window.clone();main.model().administration.view.as_ref().unwrap().server.as_ref().unwrap().name.set_text("Fresh server");find_button(window.upcast_ref(),"Criar servidor e canal inicial").emit_clicked();until(context,||!main.model().setup&&main.model().active_channel_id.is_some()&&main.model().administration.view.is_some());
    assert!(outputs.lock().unwrap().is_empty());main.emit(MainWindowMsg::Administration(AdminMsg::Open));pump(context);let v=main.model();let f=v.administration.view.as_ref().unwrap().server.as_ref().unwrap();f.public.set_active(false);f.change_password.set_active(true);f.password.set_text("Secret123!");drop(v);main.emit(MainWindowMsg::Administration(AdminMsg::SaveServer));until(context,||main.model().administration.pending.is_none()); // first failure retains secrets
    assert_eq!(main.model().administration.view.as_ref().unwrap().server.as_ref().unwrap().password.text(),"Secret123!");main.emit(MainWindowMsg::Administration(AdminMsg::SaveServer));until(context,||outputs.lock().unwrap().iter().any(|o|matches!(o,MainWindowOutput::SessionExpired)));
    assert_eq!(main.model().administration.view.as_ref().unwrap().server.as_ref().unwrap().password.text(),"");
    let requests=&backend.lock().unwrap().requests;let writes:Vec<_>=requests.iter().filter(|(m,p,_)|m=="PATCH"&&p=="/server").collect();assert_eq!(writes.last().unwrap().2,serde_json::json!({"public":false,"password":"Secret123!"}));task.abort();
}
#[cfg(test)]
pub(crate) fn exercise_denied(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{descendants,pump};
    assert!(!main.model().server_access.manage_roles);assert!(!main.model().server_access.manage_channels);assert!(!main.model().server_access.manage_server);
    assert!(!descendants(main.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<gtk::Button>()).any(|b|b.label().as_deref()==Some("Administração")&&b.get_visible()));
    main.emit(MainWindowMsg::Administration(AdminMsg::Open));main.emit(MainWindowMsg::Administration(AdminMsg::SaveRole));main.emit(MainWindowMsg::Administration(AdminMsg::SaveServer));main.emit(MainWindowMsg::Administration(AdminMsg::SaveChannel));main.emit(MainWindowMsg::Administration(AdminMsg::Assign(true)));pump(context);
    assert!(main.model().administration.pending.is_none());assert!(!main.model().admin_visible());
}
