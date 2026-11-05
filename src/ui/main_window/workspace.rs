//! EN-US: Publish account-local rail state; hidden accounts never acknowledge visible reads.
//! PT-BR: Publica estado local da conta para a barra; contas ocultas não confirmam leituras visuais.
use super::*;
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Summary {pub name:String,pub icon_revision:Uuid,pub direct:bool,pub direct_count:usize,pub count:usize,pub unread:bool,pub connected:bool,pub conversation:Option<Uuid>,pub calling:bool}
#[derive(Default)]pub(super) struct State{pub last:Option<Summary>,last_at:Option<Instant>,pub(super) hosted:bool,pub(super) active:bool,pub(super) direct:bool,pub(super) preferred:Option<Uuid>}
impl MainWindowModel {
    pub(crate) fn workspace_icon(&self)->Option<gtk::gdk::Paintable>{self.sidebar.model().server_image.clone()}
    pub(super) fn workspace_visibility(&mut self,active:bool,restore:Option<Uuid>,root:&gtk::Box){
        self.workspace.hosted=true;self.workspace.active=active;if active{self.apply_config(root);}if restore.is_some(){self.workspace.preferred=restore;}
        self.sidebar.emit(SidebarMsg::Hosted(true));self.layout.navigation.set_min_sidebar_width(256.0);self.layout.navigation.set_max_sidebar_width(256.0);
        self.chat.emit(ChatMsg::WorkspaceVisible(active));
    }
    pub(super) fn workspace_restore(&mut self,root:&gtk::Box,sender:&ComponentSender<Self>){
        let Some(id)=self.workspace.preferred else{return;};
        if let Some(channel)=self.channels.iter().find(|c|c.id==id&&c.supports_messages()).cloned(){self.workspace.preferred=None;self.select_channel(channel,sender.clone());}
        else if self.direct.items.iter().any(|d|d.id==id){self.workspace.preferred=None;self.direct_event(DirectMsg::Select(id,None),sender.clone(),root);}
        else if self.connection.state.background_started&&self.connection.state.access_valid&&self.direct.request.is_none(){self.workspace.preferred=None;}
    }
    pub(super) fn publish_workspace(&mut self,sender:&ComponentSender<Self>){
        if !self.workspace.hosted||self.workspace.last_at.is_some_and(|last|last.elapsed()<Duration::from_millis(250)){return;}
        self.workspace.last_at=Some(Instant::now());
        let count=self.notifications.inbox.unread(&self.notification_channels());
        let summary=Summary{direct:self.workspace.direct,direct_count:self.direct.items.iter().fold(0usize,|count,d|count.saturating_add(d.unread_count as usize)),icon_revision:self.sidebar.model().server_icon_revision,name:self.server.as_ref().map(|s|s.name.chars().take(256).collect()).unwrap_or_else(||self.connection.account.server().to_string()),count,
            unread:count>0||self.channels.iter().any(|c|c.has_unread())||self.direct.items.iter().any(|d|d.unread_count>0),
            connected:self.connection.state.socket_ready,conversation:self.active_channel_id,calling:self.voice.has_call()};
        if self.workspace.last.as_ref()!=Some(&summary){self.workspace.last=Some(summary.clone());let _=sender.output(MainWindowOutput::Workspace(summary));}
    }
}
