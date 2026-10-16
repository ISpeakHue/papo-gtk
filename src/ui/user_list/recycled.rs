//! Large rosters retain lightweight records and recycle their visible widgets.
use super::*;
#[derive(Clone,PartialEq)]struct Row{key:String,id:Option<Uuid>,name:String,phrase:String,status:PresenceStatus,roles:Vec<crate::models::RoleSummary>,avatar:Option<gtk::gdk::Texture>,show:bool}
pub(super) struct List{store:gtk::gio::ListStore,pub widget:gtk::ListView}
impl List{
    pub fn new(sender:&ComponentSender<UserListModel>)->Self{
        let store=gtk::gio::ListStore::new::<gtk::glib::BoxedAnyObject>();let factory=gtk::SignalListItemFactory::new();let input=sender.input_sender().clone();
        factory.connect_setup(move |_,item|{
            let item=item.downcast_ref::<gtk::ListItem>().unwrap();let button=gtk::Button::new();button.add_css_class("flat");button.set_hexpand(true);
            let body=gtk::Box::new(gtk::Orientation::Horizontal,10);let overlay=gtk::Overlay::new();let avatar=adw::Avatar::new(24,None,true);overlay.set_child(Some(&avatar));let status=crate::ui::status::image(PresenceStatus::Offline);status.set_halign(gtk::Align::End);status.set_valign(gtk::Align::End);overlay.add_overlay(&status);body.append(&overlay);
            let names=gtk::Box::new(gtk::Orientation::Vertical,2);names.set_hexpand(true);let name=gtk::Label::new(None);name.set_xalign(0.0);name.set_ellipsize(pango::EllipsizeMode::End);names.append(&name);let phrase=gtk::Label::new(None);phrase.set_xalign(0.0);phrase.set_ellipsize(pango::EllipsizeMode::End);phrase.add_css_class("caption");phrase.add_css_class("dim-label");names.append(&phrase);body.append(&names);button.set_child(Some(&body));
            let weak=item.downgrade();let input=input.clone();button.connect_clicked(move |_|{if let Some(object)=weak.upgrade().and_then(|i|i.item()).and_downcast::<gtk::glib::BoxedAnyObject>(){if let Some(id)=object.borrow::<Row>().id{let _=input.send(UserListMsg::OpenProfile(id));}}});item.set_child(Some(&button));crate::performance::record("member_widgets_built",1);
        });
        factory.connect_bind(|_,item|{
            let item=item.downcast_ref::<gtk::ListItem>().unwrap();let object=item.item().and_downcast::<gtk::glib::BoxedAnyObject>().unwrap();let row=object.borrow::<Row>();let button=item.child().and_downcast::<gtk::Button>().unwrap();button.set_widget_name(&row.key);button.set_can_target(row.id.is_some());button.set_focusable(row.id.is_some());
            let body=button.child().unwrap();let overlay=body.first_child().and_downcast::<gtk::Overlay>().unwrap();let avatar=overlay.child().and_downcast::<adw::Avatar>().unwrap();avatar.set_text(Some(&row.name));avatar.set_custom_image(row.avatar.as_ref());overlay.set_visible(row.id.is_some()&&row.show);
            let mut child=overlay.first_child();while let Some(w)=child{child=w.next_sibling();if let Some(image)=w.downcast_ref::<gtk::Image>(){crate::ui::status::set(image,row.status);}}
            let names=body.last_child().unwrap();let name=names.first_child().and_downcast::<gtk::Label>().unwrap();name.set_text(&row.name);name.set_attributes(None);name.remove_css_class("papo-role-name");name.remove_css_class("caption-heading");name.remove_css_class("papo-section-heading");name.remove_css_class("dim-label");avatar.remove_css_class("dim-label");
            if row.id.is_none(){name.add_css_class("caption-heading");name.add_css_class("dim-label");name.add_css_class("papo-section-heading");}else{crate::ui::style::role_color(&name,&row.roles);if row.status==PresenceStatus::Offline{name.add_css_class("dim-label");avatar.add_css_class("dim-label");}}
            let phrase=names.last_child().and_downcast::<gtk::Label>().unwrap();phrase.set_text(&row.phrase);phrase.set_tooltip_text(Some(&row.phrase));phrase.set_visible(!row.phrase.is_empty());
        });
        let widget=gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))),Some(factory));widget.add_css_class("navigation-sidebar");Self{store,widget}
    }
    pub fn update(&self,model:&UserListModel){
        let mut users:Vec<_>=model.users.iter().collect();users.sort_by(|a,b|member_group(model.presence.get(&a.id).map(|p|p.status)).cmp(&member_group(model.presence.get(&b.id).map(|p|p.status))).then_with(||member_name(a,&model.presence).cmp(member_name(b,&model.presence))).then_with(||a.id.cmp(&b.id)));
        let mut counts=[0usize;4];for user in &users{counts[member_group(model.presence.get(&user.id).map(|p|p.status))]+=1;}
        let mut desired=vec![];let mut previous=None;
        for user in users{let status=model.presence.get(&user.id).map(|p|p.status).unwrap_or(PresenceStatus::Offline);let group=member_group(Some(status));if previous!=Some(group){previous=Some(group);desired.push(Row{key:format!("group-{group}"),id:None,name:format!("{} — {}",MEMBER_GROUPS[group],counts[group]),phrase:String::new(),status,roles:vec![],avatar:None,show:false});}
            desired.push(Row{key:format!("member-{}",user.id),id:Some(user.id),name:member_name(user,&model.presence).into(),phrase:model.presence.get(&user.id).and_then(|p|p.status_message.as_deref()).or(user.status_message.as_deref()).unwrap_or("").into(),status,roles:user.roles.clone().unwrap_or_default(),avatar:model.avatars.get(&user.id).cloned(),show:model.show_avatars});}
        let old:Vec<_>=(0..self.store.n_items()).map(|i|self.store.item(i).and_downcast::<gtk::glib::BoxedAnyObject>().unwrap()).collect();let indexed:HashMap<_,_>=old.iter().map(|o|(o.borrow::<Row>().key.clone(),o)).collect();
        let desired:Vec<_>=desired.into_iter().map(|r|indexed.get(&r.key).filter(|o|*o.borrow::<Row>()==r).map(|o|(*o).clone()).unwrap_or_else(||gtk::glib::BoxedAnyObject::new(r))).collect();
        let prefix=old.iter().zip(&desired).take_while(|(a,b)|a==b).count();let suffix=old[prefix..].iter().rev().zip(desired[prefix..].iter().rev()).take_while(|(a,b)|a==b).count();if prefix+suffix<old.len()||prefix+suffix<desired.len(){self.store.splice(prefix as u32,(old.len()-prefix-suffix) as u32,&desired[prefix..desired.len()-suffix]);}
    }
    pub fn visible(&self,scroll:&gtk::ScrolledWindow)->Vec<Uuid>{
        // ListView may keep measured, unallocated factory children around.
        // Picking the painted viewport avoids treating their stale bounds as
        // visible. Add five adjacent model records as overscan on each side.
        let mut seen=std::collections::HashSet::new();
        for y in (0..scroll.height()).step_by(12){
            let mut picked=scroll.pick((scroll.width()/2) as f64,y as f64,gtk::PickFlags::DEFAULT);
            while let Some(widget)=picked{if let Some(id)=widget.widget_name().strip_prefix("member-").and_then(|s|Uuid::parse_str(s).ok()){seen.insert(id);break;}picked=widget.parent();}
        }
        let visible:Vec<_>=(0..self.store.n_items()).filter_map(|i|{let object=self.store.item(i).and_downcast::<gtk::glib::BoxedAnyObject>()?;let row=object.borrow::<Row>();row.id.filter(|id|seen.contains(id)).map(|_|i)}).collect();
        let(Some(first),Some(last))=(visible.first(),visible.last())else{return vec![];};
        ((*first).saturating_sub(5)..(*last+6).min(self.store.n_items())).filter_map(|i|self.store.item(i).and_downcast::<gtk::glib::BoxedAnyObject>().and_then(|o|o.borrow::<Row>().id)).collect()
    }
}
