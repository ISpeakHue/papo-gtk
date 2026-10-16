//! Native grouped settings rows matching the account preferences mockup.
use super::*;

pub(super) fn group(body:&gtk::Box,title:&str)->adw::PreferencesGroup {
    let group=adw::PreferencesGroup::new();group.set_title(title);body.append(&group);group
}

pub(super) fn choice(group:&adw::PreferencesGroup,title:&str,values:&[&str],selected:u32)->adw::ComboRow {
    let row=adw::ComboRow::new();
    row.set_title(title);row.set_title_lines(0);row.set_use_subtitle(false);
    row.set_model(Some(&gtk::StringList::new(values)));row.set_selected(selected);
    group.add(&row);row
}
