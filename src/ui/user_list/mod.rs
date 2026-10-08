//! Online users and member list component.

use gtk::pango;
use gtk::prelude::*;
use relm4::prelude::*;
use uuid::Uuid;

use crate::models::UserSummary;
use crate::media::avatar_image;
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
}

#[derive(Debug)]
pub enum UserListMsg {
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
    type Output = Uuid;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_width_request: 224,
            add_css_class: "papo-members",

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
        let model = UserListModel {
            users: init.users,
            presence: HashMap::new(),
            avatars: HashMap::new(),
            show_avatars: true,
        };

        let widgets = view_output!();
        rebuild_members_list(&widgets.members_list, &model.users, &model.presence, &model.avatars, model.show_avatars, &sender);

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
            UserListMsg::OpenProfile(id) => {let _=sender.output(id);}
            UserListMsg::SetConfig(config) => {self.show_avatars=config.display.as_ref().and_then(|d|d.show_avatars).unwrap_or(true);rebuild_members_list(&widgets.members_list,&self.users,&self.presence,&self.avatars,self.show_avatars,&sender);}
            UserListMsg::SetAvatars(avatars) => {
                self.avatars = avatars;
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &sender);
            }
            UserListMsg::SetUsers(users) => {
                self.users = users;
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &sender);
            }
            UserListMsg::PresenceSync(entries) => {
                self.presence = entries.into_iter().map(|entry| (entry.user_id, entry)).collect();
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &sender);
            }
            UserListMsg::UpdatePresence(entry) => {
                self.presence.insert(entry.user_id, entry);
                rebuild_members_list(&widgets.members_list, &self.users, &self.presence, &self.avatars, self.show_avatars, &sender);
            }
        }
        self.update_view(widgets, sender);
    }
}

fn rebuild_members_list(list_box: &gtk::ListBox, users: &[UserSummary],
    presence: &HashMap<Uuid, PresenceEntry>, avatars: &HashMap<Uuid, gtk::gdk::Texture>, show_avatars:bool, _sender:&ComponentSender<UserListModel>) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    // Sort: online first, then alphabetically by display name
    let mut sorted_users = users.to_vec();
    sorted_users.sort_by(|a, b| {
        let a_online = presence.get(&a.id).is_some_and(PresenceEntry::online);
        let b_online = presence.get(&b.id).is_some_and(PresenceEntry::online);
        b_online.cmp(&a_online).then_with(|| a.display_name().cmp(b.display_name()))
    });

    let online=sorted_users.iter().filter(|u|presence.get(&u.id).is_some_and(PresenceEntry::online)).count();
    let mut group=None;
    for user in sorted_users {
        let is_online = presence.get(&user.id).is_some_and(PresenceEntry::online);
        if group!=Some(is_online){group=Some(is_online);let heading=gtk::Label::new(Some(&format!("{} — {}",if is_online{"ONLINE"}else{"OFFLINE"},if is_online{online}else{users.len()-online})));heading.add_css_class("caption-heading");heading.add_css_class("dim-label");heading.add_css_class("papo-section-heading");heading.set_xalign(0.0);let row=gtk::ListBoxRow::new();row.set_activatable(false);row.set_selectable(false);row.set_child(Some(&heading));list_box.append(&row);}


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
        let avatar = avatar_image(avatars.get(&user.id), 24);
        avatar.set_text(Some(user.display_name()));
        if !is_online {
            avatar.add_css_class("dim-label");
        }
        overlay.set_child(Some(&avatar));

        if is_online {
            let status_icon_name = match presence.get(&user.id).map(|entry| entry.status) {
                Some(PresenceStatus::Away) => "user-idle-symbolic",
                Some(PresenceStatus::Busy) => "user-busy-symbolic",
                _ => "user-available-symbolic",
            };
            let status_img = gtk::Image::from_icon_name(status_icon_name);
            status_img.set_pixel_size(10);status_img.add_css_class("papo-status-dot");
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
        list_box.append(&row);
    }
}
