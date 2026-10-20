//! EN-US: Query text filters the popup only; changing it never selects a member.
//! PT-BR: O texto da busca filtra apenas o popup; digitar não seleciona automaticamente um membro.
use super::*;
use std::{cell::RefCell,rc::Rc};
type Render=Rc<dyn Fn(&gtk::Box,&str)>;
pub(super) struct Choice {
    pub widget:gtk::MenuButton,
    ids:Vec<String>,
    pub labels:gtk::StringList,
    pub selection:gtk::SingleSelection,
    #[cfg(test)] pub filter:gtk::StringFilter,
    #[cfg(test)] pub list:gtk::ListView,
    pub avatars:Option<Rc<RefCell<HashMap<Uuid,gtk::gdk::Texture>>>>,
    render:Rc<RefCell<Render>>,
}

#[cfg(test)]
pub(super) fn exercise_scroll_reset(context:&gtk::glib::MainContext,input:relm4::Sender<MainWindowMsg>){
    use crate::ui::chat::{actions::tests::descendants,performance::settle};
    let users:Vec<UserSummary>=(0..100).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"username":format!("member{i:03}"),"nickname":format!("Member {i:03}"),"created_at":"2026-10-19T00:00:00Z"})).unwrap()).collect();
    let choice=Choice::members("Qualquer autor",&users,&HashMap::new(),input,Uuid::new_v4());choice.set_active(Some(8));
    let original:Vec<_>=(0..choice.labels.n_items()).map(|i|choice.labels.string(i).unwrap()).collect();let selected=choice.selection.selected();
    let window=adw::Window::builder().default_width(360).default_height(100).content(&choice.widget).build();window.present();settle(context);choice.widget.popup();settle(context);
    let popup=choice.widget.popover().unwrap();let widgets=descendants(popup.upcast_ref());
    let query=widgets.iter().find_map(|w|w.downcast_ref::<gtk::SearchEntry>()).unwrap();
    let scroll=widgets.iter().find_map(|w|w.downcast_ref::<gtk::ScrolledWindow>()).unwrap();let adjustment=scroll.vadjustment();
    for filter in ["member073","member018","no-such-member","member098"]{
        choice.list.scroll_to(70,gtk::ListScrollFlags::NONE,None);settle(context);assert!(adjustment.value()>300.0);
        query.set_text(filter);settle(context);
        assert_eq!(choice.list.model().unwrap().n_items(),u32::from(filter!="no-such-member"));
        query.set_text("");settle(context);
        let actual:Vec<_>=(0..choice.list.model().unwrap().n_items()).map(|i|choice.list.model().unwrap().item(i).and_downcast::<gtk::StringObject>().unwrap().string()).collect();
        assert_eq!(actual,original,"clearing preserves the source ordering");
        assert!(adjustment.value()<1.0,"clearing {filter} must reveal the start after layout, scroll={}",adjustment.value());
        assert_eq!(choice.selection.selected(),selected,"filtering must not change the committed author");
        assert!(gtk::prelude::RootExt::focus(&window).is_some_and(|w|w.is_ancestor(query)),"resetting the list must keep typing focus in its search field");
    }
    choice.widget.popdown();window.set_content(None::<&gtk::Widget>);window.close();settle(context);
}
impl Choice {
    pub fn new()->Self {
        let labels=gtk::StringList::new(&[]);
        let selection=gtk::SingleSelection::new(Some(labels.clone()));selection.set_autoselect(false);selection.set_can_unselect(true);
        let filter=gtk::StringFilter::new(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(),None::<gtk::Expression>,"string")));
        filter.set_match_mode(gtk::StringFilterMatchMode::Substring);filter.set_ignore_case(true);
        let filtered=gtk::FilterListModel::new(Some(labels.clone()),Some(filter.clone()));
        let popup_selection=gtk::SingleSelection::new(Some(filtered.clone()));popup_selection.set_autoselect(false);popup_selection.set_can_unselect(true);
        let render:Rc<RefCell<Render>>=Rc::new(RefCell::new(Rc::new(|body,title|{let label=gtk::Label::new(Some(title));label.set_xalign(0.0);body.append(&label);})));
        let factory=gtk::SignalListItemFactory::new();
        factory.connect_setup(|_,item|{item.downcast_ref::<gtk::ListItem>().unwrap().set_child(Some(&gtk::Box::new(gtk::Orientation::Horizontal,8)));});
        let rows=render.clone();factory.connect_bind(move |_,item|{
            let item=item.downcast_ref::<gtk::ListItem>().unwrap();let body=item.child().and_downcast::<gtk::Box>().unwrap();
            while let Some(child)=body.first_child(){body.remove(&child);}
            if let Some(value)=item.item().and_downcast::<gtk::StringObject>(){rows.borrow()(&body,&value.string());}
        });
        let list=gtk::ListView::new(Some(popup_selection),Some(factory));list.set_single_click_activate(true);
        let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).min_content_height(80).max_content_height(280).propagate_natural_height(true).child(&list).build();
        let query=gtk::SearchEntry::new();query.set_placeholder_text(Some("Filtrar opções"));query.set_search_delay(0);
        let body=gtk::Box::new(gtk::Orientation::Vertical,6);body.append(&query);body.append(&scroll);
        let popover=gtk::Popover::new();popover.set_child(Some(&body));
        let widget=gtk::MenuButton::new();widget.set_popover(Some(&popover));widget.set_hexpand(true);widget.set_always_show_arrow(true);
        let header=gtk::Box::new(gtk::Orientation::Horizontal,8);widget.set_child(Some(&header));
        let rows=render.clone();selection.connect_selected_item_notify(move |selection|{
            while let Some(child)=header.first_child(){header.remove(&child);}
            if let Some(value)=selection.selected_item().and_downcast::<gtk::StringObject>(){rows.borrow()(&header,&value.string());}
        });
        let filter_changed=filter.clone();let weak_list=list.downgrade();query.connect_changed(move |query|{
            filter_changed.set_search(Some(&query.text()));
            // Reset the list's anchor, not just today's adjustment value: GTK
            // otherwise restores the matching row's offset during allocation.
            if let Some(list)=weak_list.upgrade().filter(|l|l.model().is_some_and(|m|m.n_items()>0)){
                list.scroll_to(0,gtk::ListScrollFlags::NONE,None);
            }else{scroll.vadjustment().set_value(0.0);}
        });
        let weak_list=list.downgrade();query.connect_activate(move |_|{
            if let Some(list)=weak_list.upgrade().filter(|l|l.model().is_some_and(|m|m.n_items()>0)){list.emit_by_name::<()>("activate",&[&0u32]);}
        });
        let weak_query=query.downgrade();popover.connect_closed(move |_|{if let Some(query)=weak_query.upgrade(){query.set_text("");}});
        let weak_query=query.downgrade();popover.connect_map(move |_|{if let Some(query)=weak_query.upgrade(){query.grab_focus();}});
        let selected=selection.clone();let source=labels.clone();let weak=popover.downgrade();
        list.connect_activate(move |_,position|{
            let Some(item)=filtered.item(position)else{return;};
            if let Some(index)=(0..source.n_items()).find(|i|source.item(*i).as_ref()==Some(&item)){selected.set_selected(index);}
            if let Some(popover)=weak.upgrade(){popover.popdown();}
        });
        Self{widget,ids:vec![],labels,selection,#[cfg(test)] filter,#[cfg(test)] list,avatars:None,render}
    }
    pub fn append(&mut self,id:Option<&str>,label:&str){self.ids.push(id.unwrap_or("").into());self.labels.append(label);}
    pub fn set_active(&self,index:Option<u32>){self.selection.set_selected(index.unwrap_or(gtk::INVALID_LIST_POSITION));}
    pub fn id(&self)->&str{self.ids.get(self.selection.selected() as usize).map(String::as_str).unwrap_or("")}
    pub fn members(label:&str,users:&[UserSummary],avatars:&HashMap<Uuid,gtk::gdk::Texture>,input:relm4::Sender<MainWindowMsg>,view:Uuid)->Self {
        let mut choice=Self::new();choice.append(Some(""),label);
        let mut members=HashMap::new();
        for user in users.iter().filter(|u|!u.banned){let title=format!("{} (@{})",user.display_name(),user.username);choice.append(Some(&user.id.to_string()),&title);members.insert(title,user.clone());}
        let avatars=Rc::new(RefCell::new(avatars.clone()));choice.avatars=Some(avatars.clone());
        *choice.render.borrow_mut()=Rc::new(move |body,title|{
            if let Some(user)=members.get(title){
                let _=input.send(MainWindowMsg::Search(SearchMsg::MemberVisible{view,id:user.id}));
                let textures=avatars.borrow();let avatar=crate::media::member_avatar(user.id,textures.get(&user.id),24);avatar.set_text(Some(user.display_name()));body.append(&avatar);
            }
            let label=gtk::Label::new(Some(title));label.set_xalign(0.0);label.set_hexpand(true);label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if let Some(roles)=members.get(title).and_then(|u|u.roles.as_deref()){crate::ui::style::role_color(&label,roles);}body.append(&label);
        });choice.set_active(Some(0));choice
    }
}
