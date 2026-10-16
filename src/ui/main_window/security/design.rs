//! Account security layout; request and confirmation handling stays in the parent.
use super::*;

pub(super) fn heading(text:&str)->gtk::Label {
    let label=gtk::Label::new(Some(text));label.set_xalign(0.0);label.add_css_class("title-4");label
}

pub(super) fn password_fields()->(gtk::Grid,gtk::PasswordEntry,gtk::PasswordEntry) {
    let grid=gtk::Grid::new();grid.set_column_homogeneous(true);grid.set_column_spacing(20);grid.set_row_spacing(12);
    grid.add_css_class("card");grid.add_css_class("papo-security-passwords");
    let password=gtk::PasswordEntry::new();let confirmation=gtk::PasswordEntry::new();
    for (row,title,entry) in [(0,"Nova senha",&password),(1,"Repetir a nova senha",&confirmation)] {
        let label=gtk::Label::new(Some(title));label.set_xalign(0.0);label.set_wrap(true);label.set_hexpand(true);label.set_width_chars(1);label.set_mnemonic_widget(Some(entry));
        entry.set_show_peek_icon(true);entry.set_width_chars(6);entry.set_hexpand(true);entry.set_valign(gtk::Align::Center);
        entry.update_property(&[gtk::accessible::Property::Label(title)]);
        grid.attach(&label,0,row,1,1);grid.attach(entry,1,row,1,1);
    }
    (grid,password,confirmation)
}

pub(super) fn session(device:&Connection,current:bool)->(gtk::Box,gtk::Button) {
    let card=gtk::Box::new(gtk::Orientation::Horizontal,16);card.add_css_class("card");card.add_css_class("papo-security-session");
    let details=gtk::Box::new(gtk::Orientation::Vertical,2);details.set_hexpand(true);details.set_valign(gtk::Align::Center);card.append(&details);
    let title=gtk::Label::new(Some(if current{"Sessão atual"}else{"Sessão"}));title.set_xalign(0.0);details.append(&title);
    let id=gtk::Label::new(Some(&format!("ID: {}",device.id)));id.set_xalign(0.0);id.set_selectable(true);id.set_ellipsize(gtk::pango::EllipsizeMode::Middle);id.set_width_chars(1);id.set_tooltip_text(Some(&device.id.to_string()));details.append(&id);
    let dates=gtk::Label::new(Some(&format!("Criada: {} · Expira: {}",device.created_at.format("%d/%m/%Y %H:%M"),device.expires_at.format("%d/%m/%Y %H:%M"))));dates.set_wrap(true);dates.set_xalign(0.0);dates.set_width_chars(1);dates.add_css_class("dim-label");dates.add_css_class("caption");details.append(&dates);
    let end=gtk::Button::with_label("Encerrar sessão");end.add_css_class("destructive-action");end.set_valign(gtk::Align::Center);card.append(&end);
    (card,end)
}
