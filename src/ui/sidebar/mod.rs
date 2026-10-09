//! Channel sidebar component — displays server header, channels list, and user footer.

use gtk::pango;
use gtk::prelude::*;
use relm4::prelude::*;
use uuid::Uuid;

use crate::models::{Channel, ChannelType, DirectConversation, Access, Server, UserStatus, WhoamiResponse};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VoiceParticipant{pub id:Uuid,pub name:String,pub muted:bool,pub speaking:bool,pub camera:bool,pub screen:bool}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct VoiceRoom{pub id:Uuid,pub members:Vec<VoiceParticipant>}

#[derive(Debug, Clone)]
pub struct SidebarInit {
    pub current_user: WhoamiResponse,
    pub server: Option<Server>,
    pub channels: Vec<Channel>,
}

#[derive(Debug)]
pub struct SidebarModel {
    pub current_user: WhoamiResponse,
    pub server: Option<Server>,
    pub channels: Vec<Channel>,
    voice_rooms:Vec<VoiceRoom>,
    direct_rows:std::collections::HashMap<Uuid,(String,gtk::Box)>,
    channel_rows:std::collections::HashMap<String,(String,gtk::ListBoxRow)>,
    pub unread_notifications: usize,
    pub more_notifications:bool,
    pub selected_channel_id: Option<Uuid>,
    pub direct: Vec<DirectConversation>,
    pub access: Access,
    pub setup: bool,
    show_avatars:bool,
    pub direct_mode: bool,
    pub avatars: std::collections::HashMap<Uuid,gtk::gdk::Texture>,
    pub presence: std::collections::HashMap<Uuid,crate::ws::PresenceStatus>,
    icon_request:Option<Uuid>,
    icon_job:Option<tokio::task::JoinHandle<()>>,
}
impl Drop for SidebarModel{fn drop(&mut self){if let Some(job)=self.icon_job.take(){job.abort();}}}

#[derive(Debug)]
pub enum SidebarMsg {
    VoiceDock(gtk::Box),VoiceRooms(Vec<VoiceRoom>),OpenProfile(Uuid),WatchVoice{user:Uuid,kind:&'static str},
    SetServer(Server),
    IconReady{request:Uuid,image:Option<crate::media::PreparedImage>},
    ShowChannels, ShowDirect,
    SetManagement { access: Access, setup: bool },
    SetDirect(Vec<DirectConversation>),
    DirectSelect(Uuid), HideDirect(Uuid), Blocks, Administration, Moderation,
    PeerAvatars(std::collections::HashMap<Uuid,gtk::gdk::Texture>),
    Presence(std::collections::HashMap<Uuid,crate::ws::PresenceStatus>),
    SetCurrentUser(WhoamiResponse),
    SetConfig(crate::models::UserConfig),
    Profile, Preferences, Security, Search, Notifications,
    UnreadNotifications{count:usize,more:bool},
    SetAvatar(Option<gtk::gdk::Texture>),
    SetChannels(Vec<Channel>),
    SelectChannel(Uuid),
    VoiceSelect(Uuid),
    SetSelection(Uuid),
    LogoutClicked,
}

#[derive(Debug)]
pub enum SidebarOutput {
    WatchVoice{user:Uuid,kind:&'static str},
    DirectSelect(Uuid), HideDirect(Uuid), Blocks, Administration, Moderation,
    ChannelSelected(Channel),
    VoiceSelect(Uuid),
    Logout,
    Profile(Uuid), Preferences, Security, Search, Notifications,
}

#[relm4::component(pub)]
impl Component for SidebarModel {
    type Init = SidebarInit;
    type Input = SidebarMsg;
    type Output = SidebarOutput;
    type CommandOutput = (Uuid,Option<crate::media::PreparedImage>);

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_width_request: 320,
            add_css_class: "papo-navigation",
            gtk::Box {
                set_orientation:gtk::Orientation::Horizontal,set_vexpand:true,
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_width_request: 64,
                add_css_class: "papo-rail",
                gtk::ToggleButton {
                    set_icon_name: "chat-message-new-symbolic",
                    set_tooltip_text: Some("Mensagens diretas (Alt+2)"),
                    #[watch] set_active: model.direct_mode,
                    connect_clicked => SidebarMsg::ShowDirect,
                },
                gtk::Separator {},
                gtk::ToggleButton {
                    #[watch] set_tooltip_text: Some(model.server.as_ref().map(|s|s.name.as_str()).unwrap_or("Canais do servidor (Alt+1)")),
                    #[watch] set_active: !model.direct_mode,
                    connect_clicked => SidebarMsg::ShowChannels,
                    #[name="server_icon"]
                    gtk::Image {set_icon_name:Some("network-server-symbolic"),set_pixel_size:32},
                },
                gtk::Box {set_vexpand:true},

            },
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_hexpand:true,
                add_css_class: "papo-channel-panel",
                gtk::MenuButton {
                    add_css_class:"flat",add_css_class:"papo-server-heading",
                    set_tooltip_text:Some("Opções do servidor"),
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_spacing:8,
                        gtk::Label {
                            #[watch] set_text: if model.direct_mode{"Mensagens"}else{model.server.as_ref().map(|s|s.name.as_str()).unwrap_or("Papo")},
                            add_css_class:"heading",set_hexpand:true,set_xalign:0.0,
                            set_ellipsize:pango::EllipsizeMode::End,
                        },
                        gtk::Image {set_icon_name:Some("pan-down-symbolic")},
                    },
                    #[wrap(Some)]
                    set_popover = &gtk::Popover {
                        add_css_class:"papo-menu",
                        gtk::Box {
                            set_orientation:gtk::Orientation::Vertical,set_spacing:2,
                            gtk::Button {
                                add_css_class:"flat",
                                #[watch] set_label:if model.setup{"Criar servidor"}else{"Administração"},
                                #[watch] set_visible:model.setup||model.access.manage_server||model.access.manage_roles||model.access.manage_channels,
                                connect_clicked => SidebarMsg::Administration,
                            },
                            gtk::Button {set_label:"Moderação e auditoria",add_css_class:"flat",#[watch] set_visible:model.access.manage_server,connect_clicked => SidebarMsg::Moderation},
                            gtk::Button {set_label:"Usuários bloqueados",add_css_class:"flat",connect_clicked => SidebarMsg::Blocks},
                        },
                    },
                },
                gtk::Button {
                    set_label:"Pesquisar",set_tooltip_text:Some("Pesquisar mensagens (Ctrl+K)"),
                    add_css_class:"flat",add_css_class:"papo-navigation-search",
                    connect_clicked => SidebarMsg::Search,
                },
                gtk::Label {
                    #[watch] set_text:if model.direct_mode{"MENSAGENS DIRETAS"}else{"CANAIS"},
                    set_xalign:0.0,add_css_class:"caption-heading",add_css_class:"dim-label",add_css_class:"papo-section-heading",
                },
                gtk::Stack {
                    set_vexpand:true,set_transition_type:gtk::StackTransitionType::Crossfade,
                    add_named[Some("channels")] = &gtk::ScrolledWindow {
                        set_hscrollbar_policy:gtk::PolicyType::Never,
                        #[name="channels_list"]
                        gtk::ListBox {set_selection_mode:gtk::SelectionMode::Single,add_css_class:"navigation-sidebar",
                            connect_row_activated[sender] => move |_,row| {
                                if let Some(id)=row.widget_name().strip_prefix("channel-").and_then(|id|Uuid::parse_str(id).ok()){
                                    sender.input(if row.has_css_class("papo-voice-channel"){SidebarMsg::VoiceSelect(id)}else{SidebarMsg::SelectChannel(id)});
                                }
                            },
                        },
                    },
                    add_named[Some("direct")] = &gtk::ScrolledWindow {
                        set_hscrollbar_policy:gtk::PolicyType::Never,
                        #[name="direct_list"]
                        gtk::Box {set_orientation:gtk::Orientation::Vertical,set_spacing:2},
                    },
                    #[watch] set_visible_child_name:if model.direct_mode{"direct"}else{"channels"},
                },
                gtk::Label {
                    set_text:"Foi detectada reutilização de uma sessão anterior. Revise suas sessões e altere a senha em Segurança.",
                    set_wrap:true,add_css_class:"warning",set_margin_start:12,set_margin_end:12,
                    #[watch] set_visible:model.current_user.connection_violation.unwrap_or(false),
                },
            },
            },
            #[name="voice_slot"] gtk::Box {set_orientation:gtk::Orientation::Vertical},
                gtk::Box {
                    set_spacing:4,add_css_class:"papo-user-footer",
                    gtk::Button {
                        set_has_frame:false,set_hexpand:true,set_tooltip_text:Some("Meu perfil"),
                        connect_clicked => SidebarMsg::Profile,
                        gtk::Box {
                            set_spacing:8,
                            gtk::Overlay {
                                #[name="avatar_widget"]
                                adw::Avatar {set_icon_name:Some("avatar-default-symbolic"),set_size:32,set_show_initials:false,#[watch] set_text:Some(model.current_user.display_name())},
                                add_overlay = &gtk::Image {
                                    #[watch] set_icon_name:Some(match model.current_user.status {Some(UserStatus::Away)=>"user-idle-symbolic",Some(UserStatus::Busy)=>"user-busy-symbolic",_=>"user-available-symbolic"}),
                                    set_pixel_size:10,set_halign:gtk::Align::End,set_valign:gtk::Align::End,add_css_class:"papo-status-dot",
                                },
                            },
                            gtk::Box {
                                set_orientation:gtk::Orientation::Vertical,set_valign:gtk::Align::Center,set_hexpand:true,
                                gtk::Label {
                                    #[watch] set_text:model.current_user.display_name(),
                                    set_xalign:0.0,add_css_class:"heading",set_ellipsize:pango::EllipsizeMode::End,
                                },
                                gtk::Label {
                                    #[watch] set_text:match model.current_user.status {Some(UserStatus::Away)=>"Ausente",Some(UserStatus::Busy)=>"Ocupado",_=>"Disponível"},
                                    set_xalign:0.0,add_css_class:"caption",add_css_class:"dim-label",
                                },
                            },
                        },
                    },
                    gtk::MenuButton {
                        set_icon_name:"emblem-system-symbolic",set_has_frame:false,set_tooltip_text:Some("Configurações da conta"),
                        #[wrap(Some)]
                        set_popover = &gtk::Popover {
                            add_css_class:"papo-menu",
                            gtk::Box {
                                set_orientation:gtk::Orientation::Vertical,set_spacing:2,
                                gtk::Button {set_label:"Meu perfil",add_css_class:"flat",connect_clicked => SidebarMsg::Profile},
                                gtk::Button {set_label:"Preferências",add_css_class:"flat",connect_clicked => SidebarMsg::Preferences},
                                gtk::Button {set_label:"Segurança",add_css_class:"flat",connect_clicked => SidebarMsg::Security},
                                gtk::Separator {},
                                gtk::Button {set_label:"Desconectar",add_css_class:"flat",connect_clicked => SidebarMsg::LogoutClicked},
                            },
                        },
                    },
                },
        }
    }

    fn init(init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let first_channel_id = init.channels.iter().find(|channel| matches!(channel.channel_type, None | Some(ChannelType::Text))).map(|c| c.id);

        let mut model = SidebarModel {
            current_user: init.current_user,
            server: init.server,
            channels: init.channels,voice_rooms:vec![],channel_rows:Default::default(),direct_rows:Default::default(),
            selected_channel_id: first_channel_id,
            unread_notifications: 0,
            more_notifications:false,
            direct:Vec::new(), access:Access::default(),setup:false,show_avatars:true,direct_mode:false,avatars:Default::default(),presence:Default::default(),
            icon_request:None,icon_job:None,
        };

        let widgets = view_output!();
        crate::ui::style::close_popovers_on_action(&root);

        // MainWindow's bounded background avatar cache publishes the user's
        // picture. Decoding it here duplicates that work on GTK's main thread.
        model.load_icon(&sender);

        rebuild_channel_list(&widgets.channels_list, &model.channels, model.selected_channel_id, &model.voice_rooms,&model.avatars,model.show_avatars,&mut model.channel_rows,&sender);

        ComponentParts { model, widgets }
    }

    fn update_cmd_with_view(&mut self,widgets:&mut Self::Widgets,(request,image):Self::CommandOutput,sender:ComponentSender<Self>,root:&Self::Root){
        self.update_with_view(widgets,SidebarMsg::IconReady{request,image},sender,root);
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            SidebarMsg::VoiceDock(panel)=>{if panel.parent().is_none(){widgets.voice_slot.append(&panel);}},
            SidebarMsg::VoiceRooms(rooms)=>{if self.voice_rooms!=rooms{self.voice_rooms=rooms;rebuild_channel_list(&widgets.channels_list,&self.channels,self.selected_channel_id,&self.voice_rooms,&self.avatars,self.show_avatars,&mut self.channel_rows,&sender);}},
            SidebarMsg::OpenProfile(id)=>{let _=sender.output(SidebarOutput::Profile(id));},
            SidebarMsg::WatchVoice{user,kind}=>{let _=sender.output(SidebarOutput::WatchVoice{user,kind});},
            SidebarMsg::ShowChannels=>self.direct_mode=false,
            SidebarMsg::ShowDirect=>self.direct_mode=true,
            SidebarMsg::SetManagement{access,setup}=>{self.access=access;self.setup=setup;},
            SidebarMsg::Moderation=>{if self.access.manage_server{let _=sender.output(SidebarOutput::Moderation);}},
            SidebarMsg::Administration=>{let _=sender.output(SidebarOutput::Administration);},
            SidebarMsg::Blocks=>{let _=sender.output(SidebarOutput::Blocks);},
            SidebarMsg::DirectSelect(id)=>{self.direct_mode=true;let _=sender.output(SidebarOutput::DirectSelect(id));},
            SidebarMsg::HideDirect(id)=>{let _=sender.output(SidebarOutput::HideDirect(id));},
            SidebarMsg::SetDirect(dms)=>{self.direct=dms;if !self.direct.iter().any(|d|Some(d.id)==self.selected_channel_id)&&!self.channels.iter().any(|c|Some(c.id)==self.selected_channel_id){self.selected_channel_id=None;}self.rebuild_direct(&widgets.direct_list,&sender);},
            SidebarMsg::PeerAvatars(avatars)=>{crate::media::update_member_avatars(widgets.direct_list.upcast_ref(),&self.avatars,&avatars);crate::media::update_member_avatars(widgets.channels_list.upcast_ref(),&self.avatars,&avatars);self.avatars=avatars;},
            SidebarMsg::Presence(presence)=>{if self.presence==presence{return;}self.presence=presence;self.rebuild_direct(&widgets.direct_list,&sender);},
            SidebarMsg::SetCurrentUser(user) => self.current_user=user,
            SidebarMsg::SetConfig(config) => {self.show_avatars=config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true);widgets.avatar_widget.set_visible(self.show_avatars);self.rebuild_direct(&widgets.direct_list,&sender);rebuild_channel_list(&widgets.channels_list,&self.channels,self.selected_channel_id,&self.voice_rooms,&self.avatars,self.show_avatars,&mut self.channel_rows,&sender);},
            SidebarMsg::Profile => {let _=sender.output(SidebarOutput::Profile(self.current_user.id));}
            SidebarMsg::UnreadNotifications{count,more}=>{self.unread_notifications=count;self.more_notifications=more;},
            SidebarMsg::Search => {let _=sender.output(SidebarOutput::Search);}
            SidebarMsg::Notifications => {let _=sender.output(SidebarOutput::Notifications);}
            SidebarMsg::Security => {let _=sender.output(SidebarOutput::Security);}
            SidebarMsg::Preferences => {let _=sender.output(SidebarOutput::Preferences);}
            SidebarMsg::SetAvatar(texture) => crate::media::set_avatar(&widgets.avatar_widget, texture.as_ref()),
            SidebarMsg::SetServer(server) => {
                let changed=self.server.as_ref().is_none_or(|old|old.id!=server.id||old.icon_blob!=server.icon_blob);
                self.server = Some(server);
                if changed{widgets.server_icon.set_icon_name(Some("network-server-symbolic"));self.load_icon(&sender);}
            }
            SidebarMsg::IconReady{request,image}=>{if self.icon_request==Some(request){self.icon_request=None;self.icon_job=None;if let Some(image)=image{widgets.server_icon.set_paintable(Some(&image.texture()));}else{widgets.server_icon.set_icon_name(Some("network-server-symbolic"));}}},
            SidebarMsg::SetChannels(channels) => {
                if !channels.iter().any(|channel| Some(channel.id) == self.selected_channel_id) && !self.direct.iter().any(|d|Some(d.id)==self.selected_channel_id) {
                    self.selected_channel_id = channels.iter().find(|channel| matches!(channel.channel_type, None | Some(ChannelType::Text))).map(|channel| channel.id);
                }
                self.channels = channels;
                rebuild_channel_list(&widgets.channels_list, &self.channels, self.selected_channel_id, &self.voice_rooms,&self.avatars,self.show_avatars,&mut self.channel_rows,&sender);
            }
            SidebarMsg::SetSelection(id)=>{self.direct_mode=self.direct.iter().any(|d|d.id==id);self.selected_channel_id=Some(id);self.rebuild_direct(&widgets.direct_list,&sender);rebuild_channel_list(&widgets.channels_list,&self.channels,self.selected_channel_id,&self.voice_rooms,&self.avatars,self.show_avatars,&mut self.channel_rows,&sender);}
            SidebarMsg::VoiceSelect(id)=>{if self.channels.iter().any(|c|c.id==id&&c.channel_type==Some(ChannelType::Voice)){let _=sender.output(SidebarOutput::VoiceSelect(id));}},
            SidebarMsg::SelectChannel(id) => {
                self.direct_mode=false;
                self.selected_channel_id = Some(id);
                if let Some(channel) = self.channels.iter().find(|c| c.id == id).cloned() {
                    let _ = sender.output(SidebarOutput::ChannelSelected(channel));
                }
                rebuild_channel_list(&widgets.channels_list, &self.channels, self.selected_channel_id, &self.voice_rooms,&self.avatars,self.show_avatars,&mut self.channel_rows,&sender);
                self.rebuild_direct(&widgets.direct_list,&sender);
            }
            SidebarMsg::LogoutClicked => {
                let _ = sender.output(SidebarOutput::Logout);
            }
        }
        self.update_view(widgets, sender);
    }
}

fn rebuild_channel_list(
    list_box: &gtk::ListBox,
    channels: &[Channel],
    selected_id: Option<Uuid>,
    rooms:&[VoiceRoom],avatars:&std::collections::HashMap<Uuid,gtk::gdk::Texture>,
    show_avatars:bool,cache:&mut std::collections::HashMap<String,(String,gtk::ListBoxRow)>,
    sender: &ComponentSender<SidebarModel>,
) {
    let mut desired=Vec::new();

    for channel in channels {
        let is_category = matches!(channel.channel_type, Some(ChannelType::Category));
        let is_voice = matches!(channel.channel_type, Some(ChannelType::Voice));

        let key=format!("channel-{}",channel.id);let signature=format!("{}|{:?}|{}",channel.name,channel.channel_type,channel.has_unread());
        if let Some((old_signature,row))=cache.get(&key).filter(|(old,_)|*old==signature){
            desired.push((old_signature.clone(),row.clone()));
        } else {
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(&format!("channel-{}",channel.id));
        if is_voice{row.add_css_class("papo-voice-channel");}
        row.set_activatable(!is_category);
        row.set_selectable(!is_category && !is_voice);
        if channel.has_unread() {row.add_css_class("papo-channel-unread");}
        if is_voice { row.set_tooltip_text(Some("Abrir controles de voz")); }

        let box_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        box_row.set_margin_top(6);
        box_row.set_margin_bottom(6);
        box_row.set_margin_start(12);
        box_row.set_margin_end(12);

        if is_category {
            row.add_css_class("papo-category");
            let label = gtk::Label::new(Some(&channel.name.to_uppercase()));
            label.add_css_class("caption-heading");
            label.add_css_class("dim-label");
            label.set_xalign(0.0);
            label.set_hexpand(true);
            box_row.append(&label);
        } else {
            let icon_name = if is_voice {
                "audio-volume-high-symbolic"
            } else {
                "chat-bubbles-text-symbolic"
            };

            if is_voice {
                let icon = gtk::Image::from_icon_name(icon_name);icon.set_pixel_size(16);icon.add_css_class("dim-label");box_row.append(&icon);
            } else {
                let hash=gtk::Label::new(Some("#"));hash.add_css_class("papo-channel-symbol");hash.add_css_class("dim-label");hash.set_width_request(16);box_row.append(&hash);
            }

            let label = gtk::Label::new(Some(&channel.name));
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.set_ellipsize(pango::EllipsizeMode::End);
            box_row.append(&label);

            if channel.has_unread() {
                let badge = gtk::Label::new(Some("●"));
                badge.set_widget_name(&format!("unread-{}",channel.id));
                badge.set_tooltip_text(Some("Novas mensagens"));
                badge.set_width_request(8);
                badge.set_height_request(8);
                badge.set_valign(gtk::Align::Center);
                badge.add_css_class("accent");
                box_row.append(&badge);
            }
        }

        row.set_child(Some(&box_row));

        desired.push((signature,row));
        }
        if is_voice{if let Some(room)=rooms.iter().find(|r|r.id==channel.id){for member in &room.members{
            let key=format!("voice-member-{}-{}",channel.id,member.id);
            let mut signature_member=member.clone();signature_member.speaking=false;
            let signature=format!("{:?}-{}",signature_member,show_avatars);
            if let Some((_,old))=cache.get(&key).filter(|(old,_)|*old==signature){
                update_voice_speaker(old.upcast_ref(),member.speaking&&!member.muted,show_avatars);
                desired.push((signature,old.clone()));continue;
            }
            let row=gtk::ListBoxRow::new();row.set_widget_name(&format!("voice-member-{}-{}",channel.id,member.id));row.set_activatable(false);row.set_selectable(false);row.add_css_class("papo-voice-member");
            let line=gtk::Box::new(gtk::Orientation::Horizontal,4);let open=gtk::Button::new();open.add_css_class("flat");open.set_hexpand(true);open.set_tooltip_text(Some("Abrir perfil"));let content=gtk::Box::new(gtk::Orientation::Horizontal,6);
            let avatar=crate::media::member_avatar(member.id,avatars.get(&member.id),24);avatar.set_text(Some(&member.name));let avatar_box=gtk::Overlay::new();avatar_box.set_child(Some(&avatar));avatar_box.set_visible(show_avatars);let speaking=gtk::Image::from_icon_name("microphone-sensitivity-high-symbolic");speaking.set_pixel_size(10);speaking.set_halign(gtk::Align::End);speaking.set_valign(gtk::Align::End);speaking.add_css_class("papo-voice-speaking-badge");speaking.set_widget_name("voice-speaking-icon");speaking.set_tooltip_text(Some("Falando"));speaking.set_visible(member.speaking&&!member.muted);avatar_box.add_overlay(&speaking);content.append(&avatar_box);let fallback=gtk::Image::from_icon_name("microphone-sensitivity-high-symbolic");fallback.set_widget_name("voice-speaking-fallback");fallback.set_pixel_size(16);fallback.set_tooltip_text(Some("Falando"));fallback.add_css_class("papo-voice-speaking");fallback.set_visible(member.speaking&&!member.muted);content.append(&fallback);let name=gtk::Label::new(Some(&member.name));name.set_xalign(0.0);name.set_hexpand(true);name.set_ellipsize(pango::EllipsizeMode::End);name.add_css_class("caption");name.set_widget_name("voice-member-name");if member.speaking&&!member.muted{name.add_css_class("papo-voice-speaking");}content.append(&name);if member.muted{content.append(&gtk::Image::from_icon_name("microphone-disabled-symbolic"));}open.set_child(Some(&content));let s=sender.clone();let id=member.id;open.connect_clicked(move |_|s.input(SidebarMsg::OpenProfile(id)));line.append(&open);
            for (active,kind,icon,label) in [(member.camera,"video","camera-video-symbolic","Ver câmera"),(member.screen,"screen","video-display-symbolic","Ver tela")]{if active{let watch=gtk::Button::from_icon_name(icon);watch.add_css_class("flat");watch.set_tooltip_text(Some(label));let s=sender.clone();watch.connect_clicked(move |_|s.input(SidebarMsg::WatchVoice{user:id,kind}));line.append(&watch);}}
            row.set_child(Some(&line));
            desired.push((signature,row));
        }}}

    }
    let mut next=std::collections::HashMap::new();let mut rows=Vec::new();
    for (signature,built) in desired{
        let key=built.widget_name().to_string();
        let row=cache.remove(&key).filter(|(old,_)|*old==signature).map_or(built,|(_,row)|row);
        next.insert(key,(signature,row.clone()));rows.push(row);
    }
    let keep:std::collections::HashSet<_>=rows.iter().cloned().collect();let mut child=list_box.first_child();while let Some(w)=child{child=w.next_sibling();if !w.downcast_ref::<gtk::ListBoxRow>().is_some_and(|r|keep.contains(r)){list_box.remove(&w);}}
    for (index,row) in rows.iter().enumerate(){if list_box.row_at_index(index as i32).as_ref()!=Some(row){if row.parent().is_some(){list_box.remove(row);}list_box.insert(row,index as i32);}}
    list_box.unselect_all();if let Some(id)=selected_id{if let Some((_,row))=next.get(&format!("channel-{id}")){if row.is_selectable(){list_box.select_row(Some(row));}}}
    *cache=next;
}

fn update_voice_speaker(widget:&gtk::Widget,speaking:bool,show_avatars:bool){
    match widget.widget_name().as_str(){
        "voice-member-name"=>{if speaking{widget.add_css_class("papo-voice-speaking");}else{widget.remove_css_class("papo-voice-speaking");}},
        "voice-speaking-icon"=>widget.set_visible(speaking),"voice-speaking-fallback"=>widget.set_visible(speaking),_=>{},
    }
    let mut child=widget.first_child();while let Some(w)=child{update_voice_speaker(&w,speaking,show_avatars);child=w.next_sibling();}
}

impl SidebarModel {
    fn load_icon(&mut self,sender:&ComponentSender<Self>){
        if let Some(job)=self.icon_job.take(){job.abort();}self.icon_request=None;
        let Some(blob)=self.server.as_ref().and_then(|s|s.icon_blob.clone()).filter(|b|!b.is_empty()) else{return;};
        let request=Uuid::new_v4();self.icon_request=Some(request);let output=sender.command_sender().clone();
        self.icon_job=Some(tokio::spawn(async move{let image=crate::media::avatars::prepare_blob(Some(blob),64).await;let _=output.send((request,image));}));
    }
    fn rebuild_direct(&mut self,list:&gtk::Box,sender:&ComponentSender<Self>){
        let mut ordered=vec![];
        if self.direct.is_empty(){let empty=gtk::Label::new(Some("O seu papo começa aqui.
Abra o perfil de um membro para enviar uma mensagem."));empty.set_wrap(true);empty.set_margin_start(16);empty.set_margin_end(16);empty.set_margin_top(20);empty.add_css_class("dim-label");ordered.push(empty.upcast::<gtk::Widget>());}
        for dm in &self.direct {
            let signature=format!("{}|{}|{:?}|{}|{}",dm.user.display_name(),dm.unread_count,self.presence.get(&dm.user.id),self.selected_channel_id==Some(dm.id),self.show_avatars);
            if let Some((_,row))=self.direct_rows.get(&dm.id).filter(|(old,_)|*old==signature){ordered.push(row.clone().upcast());continue;}

            let row=gtk::Box::new(gtk::Orientation::Horizontal,2);row.add_css_class("papo-direct-row");
            let open=gtk::Button::new();open.add_css_class("flat");
            if self.selected_channel_id==Some(dm.id){open.add_css_class("selected");}
            open.set_hexpand(true);open.set_widget_name(&format!("dm-{}",dm.id));open.set_tooltip_text(Some(dm.user.display_name()));
            let content=gtk::Box::new(gtk::Orientation::Horizontal,8);
            let (icon,status)=match self.presence.get(&dm.user.id){Some(crate::ws::PresenceStatus::Online)=>("user-available-symbolic","Disponível"),Some(crate::ws::PresenceStatus::Away)=>("user-idle-symbolic","Ausente"),Some(crate::ws::PresenceStatus::Busy)=>("user-busy-symbolic","Ocupado"),_=>("user-offline-symbolic","Offline")};
            let presence=gtk::Image::from_icon_name(icon);presence.set_pixel_size(10);presence.set_tooltip_text(Some(status));presence.add_css_class("papo-status-dot");
            if self.show_avatars{let avatar=gtk::Overlay::new();let image=crate::media::member_avatar(dm.user.id,self.avatars.get(&dm.user.id),28);image.set_text(Some(dm.user.display_name()));avatar.set_child(Some(&image));presence.set_halign(gtk::Align::End);presence.set_valign(gtk::Align::End);avatar.add_overlay(&presence);content.append(&avatar);}else{content.append(&presence);}
            let label=gtk::Label::new(Some(dm.user.display_name()));label.set_xalign(0.0);label.set_ellipsize(pango::EllipsizeMode::End);label.set_hexpand(true);content.append(&label);
            if dm.unread_count>0{let badge=gtk::Label::new(Some(&dm.unread_count.to_string()));badge.add_css_class("papo-notification-count");content.append(&badge);}
            open.set_child(Some(&content));let s=sender.clone();let id=dm.id;open.connect_clicked(move |_|s.input(SidebarMsg::DirectSelect(id)));row.append(&open);
            let hide=gtk::Button::from_icon_name("window-close-symbolic");hide.add_css_class("flat");hide.set_tooltip_text(Some("Ocultar conversa; o histórico é preservado"));let s=sender.clone();hide.connect_clicked(move |_|s.input(SidebarMsg::HideDirect(id)));row.append(&hide);self.direct_rows.insert(id,(signature,row.clone()));ordered.push(row.upcast());
        }
        self.direct_rows.retain(|id,_|self.direct.iter().any(|d|d.id==*id));
        let keep:std::collections::HashSet<_>=ordered.iter().cloned().collect();
        let mut child=list.first_child();while let Some(w)=child{child=w.next_sibling();if !keep.contains(&w){list.remove(&w);}}
        let mut previous:Option<&gtk::Widget>=None;for row in &ordered{if row.parent().is_none(){list.insert_child_after(row,previous);}else{list.reorder_child_after(row,previous);}previous=Some(row);}
    }
}

#[cfg(test)]pub(crate) fn exercise_startup(context:&gtk::glib::MainContext){
    use adw::prelude::*;
    use crate::ui::chat::actions::tests::{descendants,pump,until};
    use crate::ui::chat::performance::settle;
    use base64::Engine;
    let created=chrono::Utc::now();let user=Uuid::new_v4();let server_id=Uuid::new_v4();let voice=Uuid::new_v4();
    let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2048,2048).write_to(&mut bytes,image::ImageFormat::Png).unwrap();let blob=base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
    let who:WhoamiResponse=serde_json::from_value(serde_json::json!({"id":user,"username":"startup","created_at":created,"avatar_blob":blob})).unwrap();
    let server:Server=serde_json::from_value(serde_json::json!({"id":server_id,"name":"startup","created_at":created,"icon_blob":blob})).unwrap();
    let channel:Channel=serde_json::from_value(serde_json::json!({"id":voice,"name":"voice","type":"voice","created_at":created})).unwrap();
    // Hold both decoder permits: GTK must be able to present and respond while
    // the icon is queued, and duplicate refreshes must retain the same request.
    let permit=crate::media::avatars::hold_decoders();
    let sidebar=SidebarModel::builder().launch(SidebarInit{current_user:who,server:Some(server.clone()),channels:vec![channel]}).detach();
    let window=adw::Window::builder().default_width(320).default_height(700).content(sidebar.widget()).build();window.present();settle(context);
    let request=sidebar.model().icon_request.unwrap();let icon=descendants(sidebar.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Image>().ok().filter(|i|i.pixel_size()==32&&i.icon_name().as_deref()==Some("network-server-symbolic"))).unwrap();assert!(icon.is_mapped());
    let mut rename=server.clone();rename.name="renamed".into();sidebar.emit(SidebarMsg::SetServer(rename));pump(context);assert_eq!(sidebar.model().icon_request,Some(request));
    let mut replacement=server.clone();replacement.icon_blob=Some(format!("data:image/png;base64,{blob}"));sidebar.emit(SidebarMsg::SetServer(replacement));pump(context);let current=sidebar.model().icon_request.unwrap();assert_ne!(current,request);
    sidebar.emit(SidebarMsg::IconReady{request,image:Some(crate::media::PreparedImage{width:1,height:1,pixels:vec![255;4]})});pump(context);assert_eq!(icon.icon_name().as_deref(),Some("network-server-symbolic"),"old results cannot replace the new server icon");
    drop(permit);until(context,||sidebar.model().icon_request.is_none());let texture=icon.paintable().and_downcast::<gtk::gdk::Texture>().unwrap();assert!(texture.width()<=64&&texture.height()<=64,"a 2048-pixel source must not remain a full-size texture");
    sidebar.emit(SidebarMsg::SetServer(server.clone()));pump(context);let pending=sidebar.model().icon_request.unwrap();let mut removal=server;removal.icon_blob=None;sidebar.emit(SidebarMsg::SetServer(removal));pump(context);sidebar.emit(SidebarMsg::IconReady{request:pending,image:Some(crate::media::PreparedImage{width:1,height:1,pixels:vec![0;4]})});pump(context);assert_eq!(icon.icon_name().as_deref(),Some("network-server-symbolic"));
    let peers:Vec<Uuid>=(0..8).map(|_|Uuid::new_v4()).collect();let dms=peers.iter().enumerate().map(|(i,id)|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"user":{"id":id,"username":format!("peer-{i}"),"created_at":created},"created_at":created,"unread_count":0})).unwrap()).collect();
    sidebar.emit(SidebarMsg::SetDirect(dms));sidebar.emit(SidebarMsg::VoiceRooms(vec![VoiceRoom{id:voice,members:peers.iter().enumerate().map(|(i,id)|VoiceParticipant{id:*id,name:format!("peer-{i}"),muted:false,speaking:false,camera:false,screen:false}).collect()}]));pump(context);
    let before:Vec<_>=descendants(sidebar.widget().upcast_ref()).into_iter().filter(|w|(w.is::<gtk::Button>()&&w.widget_name().starts_with("dm-"))||(w.is::<gtk::ListBoxRow>()&&w.widget_name().starts_with("voice-member-"))).collect();assert_eq!(before.len(),16);
    let textures=peers.iter().map(|id|(*id,crate::media::PreparedImage{width:8,height:8,pixels:vec![255;256]}.texture())).collect();sidebar.emit(SidebarMsg::PeerAvatars(textures));pump(context);
    let after:Vec<_>=descendants(sidebar.widget().upcast_ref()).into_iter().filter(|w|(w.is::<gtk::Button>()&&w.widget_name().starts_with("dm-"))||(w.is::<gtk::ListBoxRow>()&&w.widget_name().starts_with("voice-member-"))).collect();assert_eq!(before,after,"avatar batches preserve DM and voice rows");
    for row in &after{assert!(descendants(row).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).all(|a|a.custom_image().is_some()));}
    sidebar.emit(SidebarMsg::PeerAvatars(Default::default()));pump(context);for row in &after{assert!(descendants(row).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).all(|a|a.custom_image().is_none()));}
    sidebar.emit(SidebarMsg::Presence(Default::default()));pump(context);assert!(before.iter().all(|w|w.parent().is_some()));
    window.set_content(None::<&gtk::Widget>);window.close();
}
