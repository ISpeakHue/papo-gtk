//! EN-US: Recycled search widgets with shared, session-cached webhook avatars.
//! PT-BR: Linhas de busca reutilizáveis com avatares de webhook compartilhados no cache da sessão.
use super::*;
#[derive(Clone)]pub(super) struct Row{pub external:Option<crate::models::ExternalAuthor>,pub avatar:Option<gtk::gdk::Texture>,pub id:Uuid,pub channel:Uuid,pub heading:String,pub text:String,pub display:String,pub revision:u64,pub renderer:std::rc::Rc<dyn Fn(&str)->gtk::TextView>}
impl PartialEq for Row{fn eq(&self,other:&Self)->bool{self.external==other.external&&self.avatar==other.avatar&&self.id==other.id&&self.channel==other.channel&&self.heading==other.heading&&self.text==other.text&&self.display==other.display&&self.revision==other.revision}}
pub(super) fn view(sender:&ComponentSender<MainWindowModel>,view:Uuid)->(gtk::gio::ListStore,gtk::ListView){
    let store=gtk::gio::ListStore::new::<gtk::glib::BoxedAnyObject>();let factory=gtk::SignalListItemFactory::new();let input=sender.input_sender().clone();let visible=input.clone();
    factory.connect_setup(move |_,item|{
        let item=item.downcast_ref::<gtk::ListItem>().unwrap();let button=gtk::Button::new();button.add_css_class("flat");button.add_css_class("card");button.set_margin_bottom(12);button.add_css_class("papo-search-result");button.set_hexpand(true);
        let body=gtk::Box::new(gtk::Orientation::Vertical,10);let header=gtk::Box::new(gtk::Orientation::Horizontal,8);let heading=gtk::Label::new(None);heading.set_widget_name("search-heading");heading.set_xalign(0.0);heading.set_hexpand(true);heading.set_ellipsize(gtk::pango::EllipsizeMode::End);heading.add_css_class("caption");heading.add_css_class("dim-label");header.append(&heading);body.append(&header);
        let content=gtk::Box::new(gtk::Orientation::Vertical,0);body.append(&content);button.set_child(Some(&body));
        let weak=item.downgrade();let output=input.clone();button.connect_clicked(move |_|{if let Some(object)=weak.upgrade().and_then(|i|i.item()).and_downcast::<gtk::glib::BoxedAnyObject>(){let row=object.borrow::<Row>();let _=output.send(MainWindowMsg::Search(SearchMsg::Navigate(row.channel,row.id)));}});item.set_child(Some(&button));
        crate::performance::record("search_widgets_built",1);
    });
    factory.connect_bind(move |_,item|{
        let item=item.downcast_ref::<gtk::ListItem>().unwrap();let object=item.item().and_downcast::<gtk::glib::BoxedAnyObject>().unwrap();let row=object.borrow::<Row>();let button=item.child().and_downcast::<gtk::Button>().unwrap();let body=button.child().unwrap();let header=body.first_child().and_downcast::<gtk::Box>().unwrap();
        let mut child=header.first_child();let mut heading=None;
        while let Some(widget)=child{child=widget.next_sibling();if widget.widget_name()=="search-heading"{heading=widget.downcast::<gtk::Label>().ok();}}
        heading.unwrap().set_text(&row.heading);
        if let Some(author)=&row.external{
            // Native result rows need no avatar widget tree. Construct it only
            // for external identities, retaining row recycling for long searches.
            let avatar=header.first_child().and_downcast::<adw::Avatar>().unwrap_or_else(||{let avatar=crate::media::avatar_image(None,28);header.prepend(&avatar);avatar});
            avatar.set_text(Some(&author.name));avatar.set_custom_image(row.avatar.as_ref());
            if header.last_child().unwrap().widget_name()!="search-app-badge"{let badge=gtk::Label::new(Some("APP"));badge.set_widget_name("search-app-badge");badge.add_css_class("accent");badge.add_css_class("caption");header.append(&badge);}
            let _=visible.send(MainWindowMsg::Search(SearchMsg::ExternalVisible{view,channel:row.channel,author:author.clone()}));
        }else{
            let mut child=header.first_child();while let Some(widget)=child{child=widget.next_sibling();if widget.widget_name()!="search-heading"{header.remove(&widget);}}
        }
        let content=body.last_child().and_downcast::<gtk::Box>().unwrap();while let Some(child)=content.first_child(){content.remove(&child);}let text=(row.renderer)(&row.text);text.set_can_target(false);text.set_focusable(false);content.append(&text);
    });
    factory.connect_unbind(|_,item|{let item=item.downcast_ref::<gtk::ListItem>().unwrap();if let Some(avatar)=item.child().and_downcast::<gtk::Button>().and_then(|b|b.child()).and_then(|b|b.first_child()).and_then(|h|h.first_child()).and_downcast::<adw::Avatar>(){avatar.set_custom_image(None::<&gtk::gdk::Paintable>);}if let Some(content)=item.child().and_downcast::<gtk::Button>().and_then(|b|b.child()).and_then(|b|b.last_child()).and_downcast::<gtk::Box>(){while let Some(child)=content.first_child(){content.remove(&child);}}});
    let list=gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))),Some(factory));list.set_single_click_activate(false);list.add_css_class("background");(store,list)
}
pub(super) fn update(store:&gtk::gio::ListStore,desired:Vec<Row>){
    let old:Vec<_>=(0..store.n_items()).map(|i|store.item(i).and_downcast::<gtk::glib::BoxedAnyObject>().unwrap()).collect();
    let indexed:HashMap<_,_>=old.iter().map(|o|(o.borrow::<Row>().id,o)).collect();
    let desired:Vec<_>=desired.into_iter().map(|r|indexed.get(&r.id).filter(|o|*o.borrow::<Row>()==r).map(|o|(*o).clone()).unwrap_or_else(||gtk::glib::BoxedAnyObject::new(r))).collect();
    let prefix=old.iter().zip(&desired).take_while(|(a,b)|a==b).count();let suffix=old[prefix..].iter().rev().zip(desired[prefix..].iter().rev()).take_while(|(a,b)|a==b).count();
    if prefix+suffix<old.len()||prefix+suffix<desired.len(){store.splice(prefix as u32,(old.len()-prefix-suffix) as u32,&desired[prefix..desired.len()-suffix]);}
}
