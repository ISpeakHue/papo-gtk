//! Recycled search widgets; model records contain presentation text only.
use super::*;
#[derive(Clone)]pub(super) struct Row{pub id:Uuid,pub channel:Uuid,pub heading:String,pub text:String,pub display:String,pub revision:u64,pub renderer:std::rc::Rc<dyn Fn(&str)->gtk::TextView>}
impl PartialEq for Row{fn eq(&self,other:&Self)->bool{self.id==other.id&&self.channel==other.channel&&self.heading==other.heading&&self.text==other.text&&self.display==other.display&&self.revision==other.revision}}
pub(super) fn view(sender:&ComponentSender<MainWindowModel>)->(gtk::gio::ListStore,gtk::ListView){
    let store=gtk::gio::ListStore::new::<gtk::glib::BoxedAnyObject>();let factory=gtk::SignalListItemFactory::new();let input=sender.input_sender().clone();
    factory.connect_setup(move |_,item|{
        let item=item.downcast_ref::<gtk::ListItem>().unwrap();let button=gtk::Button::new();button.add_css_class("flat");button.add_css_class("card");button.set_margin_bottom(12);button.add_css_class("papo-search-result");button.set_hexpand(true);
        let body=gtk::Box::new(gtk::Orientation::Vertical,10);let heading=gtk::Label::new(None);heading.set_xalign(0.0);heading.set_ellipsize(gtk::pango::EllipsizeMode::End);heading.add_css_class("caption");heading.add_css_class("dim-label");body.append(&heading);
        let content=gtk::Box::new(gtk::Orientation::Vertical,0);body.append(&content);button.set_child(Some(&body));
        let weak=item.downgrade();let output=input.clone();button.connect_clicked(move |_|{if let Some(object)=weak.upgrade().and_then(|i|i.item()).and_downcast::<gtk::glib::BoxedAnyObject>(){let row=object.borrow::<Row>();let _=output.send(MainWindowMsg::Search(SearchMsg::Navigate(row.channel,row.id)));}});item.set_child(Some(&button));
        crate::performance::record("search_widgets_built",1);
    });
    factory.connect_bind(|_,item|{let item=item.downcast_ref::<gtk::ListItem>().unwrap();let object=item.item().and_downcast::<gtk::glib::BoxedAnyObject>().unwrap();let row=object.borrow::<Row>();let button=item.child().and_downcast::<gtk::Button>().unwrap();let body=button.child().unwrap();body.first_child().and_downcast::<gtk::Label>().unwrap().set_text(&row.heading);let content=body.last_child().and_downcast::<gtk::Box>().unwrap();while let Some(child)=content.first_child(){content.remove(&child);}let text=(row.renderer)(&row.text);text.set_can_target(false);text.set_focusable(false);content.append(&text);});
    factory.connect_unbind(|_,item|{let item=item.downcast_ref::<gtk::ListItem>().unwrap();if let Some(content)=item.child().and_downcast::<gtk::Button>().and_then(|b|b.child()).and_then(|b|b.last_child()).and_downcast::<gtk::Box>(){while let Some(child)=content.first_child(){content.remove(&child);}}});
    let list=gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))),Some(factory));list.set_single_click_activate(false);list.add_css_class("background");(store,list)
}
pub(super) fn update(store:&gtk::gio::ListStore,desired:Vec<Row>){
    let old:Vec<_>=(0..store.n_items()).map(|i|store.item(i).and_downcast::<gtk::glib::BoxedAnyObject>().unwrap()).collect();
    let indexed:HashMap<_,_>=old.iter().map(|o|(o.borrow::<Row>().id,o)).collect();
    let desired:Vec<_>=desired.into_iter().map(|r|indexed.get(&r.id).filter(|o|*o.borrow::<Row>()==r).map(|o|(*o).clone()).unwrap_or_else(||gtk::glib::BoxedAnyObject::new(r))).collect();
    let prefix=old.iter().zip(&desired).take_while(|(a,b)|a==b).count();let suffix=old[prefix..].iter().rev().zip(desired[prefix..].iter().rev()).take_while(|(a,b)|a==b).count();
    if prefix+suffix<old.len()||prefix+suffix<desired.len(){store.splice(prefix as u32,(old.len()-prefix-suffix) as u32,&desired[prefix..desired.len()-suffix]);}
}
