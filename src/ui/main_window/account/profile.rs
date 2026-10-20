//! EN-US: The editable profile header keeps image updates separate from the mounted form.
//! PT-BR: O cabeçalho editável do perfil atualiza imagens sem remontar o formulário.
use super::*;

pub(super) struct Hero {
    pub widget: gtk::Overlay,
    pub avatar: adw::Avatar,
    pub name: gtk::Label,
    banner: gtk::Picture,
    identity: gtk::Box,
}

impl Hero {
    pub fn new(profile: &UserProfile, avatar: Option<&gtk::gdk::Texture>, banner: Option<&gtk::gdk::Texture>) -> Self {
        let widget = gtk::Overlay::new();
        let base = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let picture = gtk::Picture::new();
        picture.set_height_request(144);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        picture.set_overflow(gtk::Overflow::Hidden);
        picture.add_css_class("papo-profile-banner");
        base.append(&picture);
        widget.set_child(Some(&base));

        let identity = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        base.append(&identity);
        let avatar = crate::media::avatar_image(avatar, 96);
        avatar.set_text(Some(profile.display_name()));
        avatar.set_valign(gtk::Align::Start);
        avatar.set_halign(gtk::Align::Start);
        avatar.add_css_class("papo-profile-avatar");
        widget.add_overlay(&avatar);
        let names = gtk::Box::new(gtk::Orientation::Vertical, 2);
        names.set_hexpand(true);
        names.set_valign(gtk::Align::Center);
        let name = gtk::Label::new(Some(profile.display_name()));
        name.add_css_class("title-2");
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        if let Some(roles) = profile.roles.as_deref() {
            crate::ui::style::role_color(&name, roles);
        }
        names.append(&name);
        let username = gtk::Label::new(Some(&format!("@{}", profile.username)));
        username.add_css_class("dim-label");
        username.set_xalign(0.0);
        username.set_ellipsize(gtk::pango::EllipsizeMode::End);
        names.append(&username);
        let badges = gtk::FlowBox::new();
        badges.add_css_class("papo-profile-roles");
        badges.set_selection_mode(gtk::SelectionMode::None);
        badges.set_min_children_per_line(1);
        badges.set_max_children_per_line(8);
        badges.set_column_spacing(4);
        badges.set_row_spacing(4);
        badges.set_halign(gtk::Align::Start);
        for role in profile.roles.iter().flatten() {
            let badge = gtk::Label::new(Some(&role.name));
            badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
            crate::ui::style::role_color(&badge, std::slice::from_ref(role));
            badges.insert(&badge, -1);
        }
        if badges.first_child().is_some() {
            names.append(&badges);
        }
        identity.append(&names);
        // Names and wrapped roles use normal box layout below the banner.
        // Only the avatar overlaps it, so extra roles never cover the image.
        widget.set_measure_overlay(&avatar, true);
        let hero = Self { widget, avatar, name, banner: picture, identity };
        hero.set_banner(banner);
        hero
    }

    pub fn set_banner(&self, texture: Option<&gtk::gdk::Texture>) {
        self.banner.set_paintable(texture);
        self.banner.set_visible(texture.is_some());
        self.identity.set_height_request(if texture.is_some() { 76 } else { 104 });
        self.identity.set_margin_start(if texture.is_some() { 132 } else { 116 });
        self.identity.set_margin_end(if texture.is_some() { 16 } else { 0 });
        self.avatar.set_margin_start(if texture.is_some() { 16 } else { 0 });
        self.avatar.set_margin_top(if texture.is_some() { 112 } else { 0 });
    }

    #[cfg(test)]
    pub(super) fn check_layout(&self, window: &gtk::Window, with_banner: bool) {
        let bounds = self.widget.compute_bounds(window).unwrap();
        assert!(bounds.x() >= 0.0 && bounds.x() + bounds.width() <= window.width() as f32,"hero bounds={bounds:?}, window={}x{}",window.width(),window.height());
        assert_eq!(self.banner.is_visible(), with_banner);
        let avatar = self.avatar.compute_bounds(window).unwrap();
        if with_banner {
            let banner = self.banner.compute_bounds(window).unwrap();
            let bottom = banner.y() + banner.height();
            assert!(avatar.y() < bottom && avatar.y() + avatar.height() > bottom, "the avatar overlaps the lower edge of the banner");
            assert!(self.name.compute_bounds(window).unwrap().y() >= bottom, "the name stays below the banner");
        } else {
            assert!(bounds.height() < 144.0, "removing the banner leaves no empty banner space");
        }
    }
}
