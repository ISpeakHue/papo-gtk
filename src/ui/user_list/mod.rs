//! Online users and member list component.

use gtk::pango;
use gtk::prelude::*;
use relm4::prelude::*;
use uuid::Uuid;

use crate::models::{UserSummary, UserStatus};
use crate::ws::{PresenceEntry, PresenceStatus};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct UserListInit {
    pub users: Vec<UserSummary>,
}

#[derive(Debug)]
pub struct UserListModel {
    pub users: Vec<UserSummary>,
    pub presence: HashMap<Uuid, PresenceEntry>,
    avatars: HashMap<Uuid, gtk::gdk::Texture>,
    show_avatars: bool,
    rows:HashMap<String,(String,gtk::ListBoxRow)>,
    status: Option<UserStatus>,
    status_pending: bool,
    status_error: String,
}

#[derive(Debug)]
pub enum UserListOutput {
    OpenProfile(Uuid),
    ChangeStatus(Option<UserStatus>),
}

#[derive(Debug)]
pub enum UserListMsg {
    SelectStatus(Option<UserStatus>),
    SetStatus(Option<UserStatus>),
    StatusPending(bool),
    StatusError(String),
    SetConfig(crate::models::UserConfig),
    OpenProfile(Uuid),
    SetUsers(Vec<UserSummary>),
    SetAvatars(HashMap<Uuid, gtk::gdk::Texture>),
    PresenceSync(Vec<PresenceEntry>),
    UpdatePresence(PresenceEntry),
}

#[relm4::component(pub)]
impl Component for UserListModel {
    type Init = UserListInit;
    type Input = UserListMsg;
    type Output = UserListOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_width_request: 224,
            add_css_class: "papo-members",

            #[name="status_button"]
            gtk::MenuButton {
                set_margin_top: 12,
                set_margin_bottom: 4,
                set_margin_start: 12,
                set_margin_end: 12,
                set_tooltip_text: Some("Alterar meu status"),
                #[watch]
                set_sensitive: !model.status_pending,
                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_spacing: 8,
                    gtk::Image {
                        set_pixel_size: 12,
                        #[watch]
                        set_resource: Some(crate::ui::status::resource(crate::ui::status::manual(model.status.as_ref()))),
                    },
                    gtk::Label {
                        set_hexpand: true,
                        set_xalign: 0.0,
                        #[watch]
                        set_label: status_label(model.status.as_ref()),
                    },
                    gtk::Image { set_icon_name: Some("pan-down-symbolic"), set_pixel_size: 12 },
                },
            },
            gtk::Label {
                #[watch]
                set_label: &model.status_error,
                #[watch]
                set_visible: !model.status_error.is_empty(),
                set_wrap: true,
                set_max_width_chars: 24,
                set_margin_start: 12,
                set_margin_end: 12,
                add_css_class: "error",
            },

            // Top title
            gtk::Box {
                add_css_class:"papo-members-heading",

                gtk::Label {
                    #[watch]
                    set_text: &format!("Membros · {}", model.users.len()),
                    add_css_class: "heading",
                    set_xalign: 0.0,
                },
            },

            gtk::Separator {
                set_orientation: gtk::Orientation::Horizontal,
            },

            // Members List
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                set_vexpand: true,

                #[name = "members_list"]
                gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    add_css_class: "navigation-sidebar",
                    connect_row_activated[sender] => move |_,row| {
                        if let Some(id)=row.widget_name().strip_prefix("member-").and_then(|id|Uuid::parse_str(id).ok()){sender.input(UserListMsg::OpenProfile(id));}
                    },
                },
            },
        }
    }

    fn init(init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let mut model = UserListModel {
            users: init.users,
            presence: HashMap::new(),
            avatars: HashMap::new(),
            show_avatars: true,rows:HashMap::new(),
            status: None, status_pending: false, status_error: String::new(),
        };

        let widgets = view_output!();
        widgets.status_button.update_property(&[gtk::accessible::Property::Label("Alterar meu status")]);
        let popover=gtk::Popover::new();
        let options=gtk::Box::new(gtk::Orientation::Vertical,2);
        for status in [None,Some(UserStatus::Busy),Some(UserStatus::Away)] {
            let button=gtk::Button::new();button.add_css_class("flat");
            button.set_tooltip_text(Some(status_label(status.as_ref())));
            button.update_property(&[gtk::accessible::Property::Label(status_label(status.as_ref()))]);
            let row=gtk::Box::new(gtk::Orientation::Horizontal,10);
            row.append(&crate::ui::status::image(crate::ui::status::manual(status.as_ref())));
            let label=gtk::Label::new(Some(status_label(status.as_ref())));label.set_xalign(0.0);label.set_hexpand(true);row.append(&label);
            button.set_child(Some(&row));let input=sender.input_sender().clone();
            button.connect_clicked(move |_|{let _=input.send(UserListMsg::SelectStatus(status.clone()));});
            options.append(&button);
        }
        popover.set_child(Some(&options));widgets.status_button.set_popover(Some(&popover));
        rebuild_members_list(&widgets.members_list, &model.users, &model.presence, &model.avatars, model.show_avatars, &mut model.rows, &sender);

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            UserListMsg::OpenProfile(id) => {let _=sender.output(UserListOutput::OpenProfile(id));}
            UserListMsg::SelectStatus(status) => {
                widgets.status_button.popdown();
                if !self.status_pending&&status!=self.status {
                    // Disable immediately, before the parent starts the write.
                    self.status_pending=true;self.status_error.clear();
                    let _=sender.output(UserListOutput::ChangeStatus(status));
                }
            }
            UserListMsg::SetStatus(status) => self.status=status,
            UserListMsg::StatusPending(pending) => {self.status_pending=pending;if pending{self.status_error.clear();}}
            UserListMsg::StatusError(error) => self.status_error=error,
            UserListMsg::SetConfig(config) => {self.show_avatars=config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true);rebuild_members_list(&widgets.members_list,&self.users,&self.presence,&self.avatars,self.show_avatars,&mut self.rows,&sender);}
            UserListMsg::SetAvatars(avatars) => {
                crate::media::update_member_avatars(widgets.members_list.upcast_ref(),&self.avatars,&avatars);
                self.avatars = avatars;
            }
            UserListMsg::SetUsers(users) => {
                self.users = users;
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &mut self.rows, &sender);
            }
            UserListMsg::PresenceSync(entries) => {
                self.presence = entries.into_iter().map(|entry| (entry.user_id, entry)).collect();
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &mut self.rows, &sender);
            }
            UserListMsg::UpdatePresence(entry) => {
                if self.presence.get(&entry.user_id)==Some(&entry){return;}
                let id=entry.user_id;
                let group_changed=member_group(self.presence.get(&id).map(|p|p.status))!=member_group(Some(entry.status));
                let only_status=self.presence.get(&id).is_some_and(|old|old.online()==entry.online()&&old.nickname==entry.nickname&&old.status_message==entry.status_message);
                self.presence.insert(id, entry);
                if only_status{
                    if let Some((signature,row))=self.rows.get_mut(&format!("member-{id}")){
                        set_status_icon(row.upcast_ref(),self.presence[&id].status);
                        if let Some(user)=self.users.iter().find(|u|u.id==id){*signature=member_signature(user,&self.presence,&self.avatars,self.show_avatars);}
                    }
                    if group_changed{rebuild_members_list(&widgets.members_list,&self.users,&self.presence,&self.avatars,self.show_avatars,&mut self.rows,&sender);}
                    self.update_view(widgets,sender);return;
                }
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &mut self.rows, &sender);
            }
        }
        self.update_view(widgets, sender);
    }
}

fn status_label(status:Option<&UserStatus>)->&'static str {
    match status {None=>"Online",Some(UserStatus::Busy)=>"Ocupado",Some(UserStatus::Away)=>"Ausente"}
}

const MEMBER_GROUPS: [&str;4] = ["ONLINE","OCUPADOS","AUSENTES","OFFLINE"];

fn rebuild_members_list(list_box: &gtk::ListBox, users: &[UserSummary],
    presence: &HashMap<Uuid, PresenceEntry>, avatars: &HashMap<Uuid, gtk::gdk::Texture>, show_avatars:bool, cache:&mut HashMap<String,(String,gtk::ListBoxRow)>, _sender:&ComponentSender<UserListModel>) {
    let mut ordered=Vec::new();

    let mut sorted_users:Vec<_> = users.iter().collect();
    sorted_users.sort_by(|a,b|member_group(presence.get(&a.id).map(|p|p.status)).cmp(&member_group(presence.get(&b.id).map(|p|p.status)))
        .then_with(||member_name(a,presence).cmp(member_name(b,presence))).then_with(||a.id.cmp(&b.id)));
    let mut counts=[0usize;MEMBER_GROUPS.len()];for user in users{counts[member_group(presence.get(&user.id).map(|p|p.status))]+=1;}
    let mut group=None;
    for user in sorted_users {
        let category=member_group(presence.get(&user.id).map(|p|p.status));
        let is_online=presence.get(&user.id).is_some_and(PresenceEntry::online);
        if group!=Some(category){
            group=Some(category);let key=format!("group-{category}");
            let text=format!("{} — {}",MEMBER_GROUPS[category],counts[category]);
            let row=cache.entry(key.clone()).or_insert_with(||{let heading=gtk::Label::new(None);heading.add_css_class("caption-heading");heading.add_css_class("dim-label");heading.add_css_class("papo-section-heading");heading.set_xalign(0.0);let row=gtk::ListBoxRow::new();row.set_widget_name(&key);row.set_activatable(false);row.set_selectable(false);row.set_child(Some(&heading));(String::new(),row)});
            if row.0!=text{row.1.child().and_downcast::<gtk::Label>().unwrap().set_text(&text);row.0=text;}
            ordered.push(row.1.clone());
        }
        let key=format!("member-{}",user.id);
        let signature=member_signature(user,presence,avatars,show_avatars);
        if let Some((_,row))=cache.get(&key).filter(|(old,_)|*old==signature){ordered.push(row.clone());continue;}



        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);
        row.set_widget_name(&format!("member-{}",user.id));
        row.set_selectable(false);

        let box_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        box_row.set_margin_top(6);
        box_row.set_margin_bottom(6);
        box_row.set_margin_start(12);
        box_row.set_margin_end(12);

        // Avatar + Status indicator
        let overlay = gtk::Overlay::new();
        let avatar = crate::media::member_avatar(user.id,avatars.get(&user.id),24);
        avatar.set_text(Some(user.display_name()));
        if !is_online {
            avatar.add_css_class("dim-label");
        }
        overlay.set_child(Some(&avatar));

        {
            let status_img = crate::ui::status::image(presence.get(&user.id).map(|entry|entry.status).unwrap_or(PresenceStatus::Offline));
            status_img.set_halign(gtk::Align::End);
            status_img.set_valign(gtk::Align::End);
            overlay.add_overlay(&status_img);
        }

        overlay.set_visible(show_avatars);
        box_row.append(&overlay);

        // Name and typing/status phrase
        let name_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        name_box.set_valign(gtk::Align::Center);name_box.set_hexpand(true);

        let name_label = gtk::Label::new(Some(presence.get(&user.id).and_then(|entry| entry.nickname.as_deref().filter(|name|!name.trim().is_empty())).unwrap_or(user.display_name())));
        name_label.set_xalign(0.0);
        if let Some(roles)=user.roles.as_deref(){crate::ui::style::role_color(&name_label,roles);}
        name_label.set_ellipsize(pango::EllipsizeMode::End);
        if !is_online {
            name_label.add_css_class("dim-label");
        }
        name_box.append(&name_label);

        if let Some(phrase) = presence.get(&user.id).and_then(|entry| entry.status_message.as_deref()).or(user.status_message.as_deref()) {
            let typing_label = gtk::Label::new(Some(phrase));
            typing_label.set_xalign(0.0);typing_label.set_ellipsize(pango::EllipsizeMode::End);typing_label.set_tooltip_text(Some(phrase));
            typing_label.add_css_class("caption");
            typing_label.add_css_class("dim-label");
            name_box.append(&typing_label);
        }

        box_row.append(&name_box);
        row.set_child(Some(&box_row));
        cache.insert(key,(signature,row.clone()));ordered.push(row);
    }
    let keep:std::collections::HashSet<_>=ordered.iter().cloned().collect();
    cache.retain(|_,(_,row)|keep.contains(row));
    let mut child=list_box.first_child();while let Some(w)=child{child=w.next_sibling();if !w.downcast_ref::<gtk::ListBoxRow>().is_some_and(|r|keep.contains(r)){list_box.remove(&w);}}
    for (index,row) in ordered.iter().enumerate(){if list_box.row_at_index(index as i32).as_ref()!=Some(row){if row.parent().is_some(){list_box.remove(row);}list_box.insert(row,index as i32);}}
}

fn member_group(status:Option<PresenceStatus>)->usize {
    match status {
        Some(PresenceStatus::Online)=>0,
        Some(PresenceStatus::Busy)=>1,
        Some(PresenceStatus::Away)=>2,
        Some(PresenceStatus::Offline)|None=>3,
    }
}

fn member_name<'a>(user:&'a UserSummary,presence:&'a HashMap<Uuid,PresenceEntry>)->&'a str{
    presence.get(&user.id).and_then(|p|p.nickname.as_deref().filter(|n|!n.trim().is_empty())).unwrap_or(user.display_name())
}
fn member_signature(user:&UserSummary,presence:&HashMap<Uuid,PresenceEntry>,_avatars:&HashMap<Uuid,gtk::gdk::Texture>,show:bool)->String{
    let p=presence.get(&user.id);
    format!("{:?}|{:?}|{:?}|{:?}|{}",user,p.map(|p|p.status),p.and_then(|p|p.nickname.as_deref()),p.and_then(|p|p.status_message.as_deref()),show)
}
fn set_status_icon(widget:&gtk::Widget,status:PresenceStatus){
    if widget.has_css_class("papo-status-dot"){if let Some(image)=widget.downcast_ref::<gtk::Image>(){crate::ui::status::set(image,status);}}
    let mut child=widget.first_child();while let Some(w)=child{child=w.next_sibling();set_status_icon(&w,status);}
}

#[cfg(test)]pub(crate) fn exercise_corrections(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{descendants,pump};
    let users:Vec<UserSummary>=(0..500).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"username":format!("member-{i:04}"),"created_at":"2026-10-08T12:00:00Z"})).unwrap()).collect();
    let list=UserListModel::builder().launch(UserListInit{users:users.clone()}).detach();
    let entry=|id,status|PresenceEntry{user_id:id,status,status_message:None,typing:None,nickname:None,user_voice:vec![]};
    list.emit(UserListMsg::PresenceSync(users.iter().map(|u|entry(u.id,PresenceStatus::Online)).collect()));pump(context);
    list.emit(UserListMsg::UpdatePresence(entry(users[0].id,PresenceStatus::Away)));pump(context);
    assert!(descendants(list.widget().upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="AUSENTES — 1")));assert!(descendants(list.widget().upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()=="ONLINE — 499")));
    let headings=||descendants(list.widget().upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::ListBoxRow>().ok().filter(|r|r.widget_name().starts_with("group-"))).map(|r|r.child().and_downcast::<gtk::Label>().unwrap().text().to_string()).collect::<Vec<_>>();
    list.emit(UserListMsg::UpdatePresence(entry(users[1].id,PresenceStatus::Busy)));pump(context);
    assert_eq!(headings(),["ONLINE — 498","OCUPADOS — 1","AUSENTES — 1"],"busy members need their own counted section");
    let rows:HashMap<_,_>=descendants(list.widget().upcast_ref()).into_iter().filter(|w|w.widget_name().starts_with("member-")).map(|w|(w.widget_name().to_string(),w)).collect();
    let affected=rows[&format!("member-{}",users[0].id)].clone();
    let name=descendants(&affected).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|l|l.text()=="member-0000")).unwrap();
    assert!(!name.has_css_class("dim-label"),"away members are still connected");
    for i in 0..200{list.emit(UserListMsg::UpdatePresence(entry(users[0].id,if i%2==0{PresenceStatus::Away}else{PresenceStatus::Busy})));}pump(context);
    let after:HashMap<_,_>=descendants(list.widget().upcast_ref()).into_iter().filter(|w|w.widget_name().starts_with("member-")).map(|w|(w.widget_name().to_string(),w)).collect();assert_eq!(rows,after);assert_eq!(after[&format!("member-{}",users[0].id)],affected);
    let badge=descendants(affected.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Image>().ok().filter(|w|w.has_css_class("papo-status-dot"))).unwrap();assert_eq!(badge.tooltip_text().as_deref(),Some("Ocupado"));assert!(badge.paintable().is_some());
    assert_eq!(headings(),["ONLINE — 498","OCUPADOS — 2"],"empty sections must disappear after a status change");
    list.emit(UserListMsg::UpdatePresence(entry(users[0].id,PresenceStatus::Offline)));pump(context);
    let after:HashMap<_,_>=descendants(list.widget().upcast_ref()).into_iter().filter(|w|w.widget_name().starts_with("member-")).map(|w|(w.widget_name().to_string(),w)).collect();for user in &users[1..]{assert_eq!(rows[&format!("member-{}",user.id)],after[&format!("member-{}",user.id)]);}
    assert_eq!(headings(),["ONLINE — 498","OCUPADOS — 1","OFFLINE — 1"]);
    let name=descendants(&after[&format!("member-{}",users[0].id)]).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|l|l.text()=="member-0000")).unwrap();
    assert!(name.has_css_class("dim-label"),"offline members remain dimmed");
    list.emit(UserListMsg::PresenceSync(vec![entry(users[0].id,PresenceStatus::Online),entry(users[1].id,PresenceStatus::Busy),entry(users[2].id,PresenceStatus::Away),entry(users[3].id,PresenceStatus::Offline)]));pump(context);
    assert_eq!(headings(),["ONLINE — 1","OCUPADOS — 1","AUSENTES — 1","OFFLINE — 497"],"full syncs must keep four sections and count unknown presence as offline");
}
