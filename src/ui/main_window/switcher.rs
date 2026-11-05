//! EN-US: Local, permission-filtered navigation; no message-search request is needed.
//! PT-BR: Navegação local filtrada por permissões, sem pesquisar mensagens no servidor.
use super::*;
use adw::prelude::*;
use std::collections::VecDeque;
#[derive(Debug)]pub enum SwitchMsg {Open,Filter,Move(i32),Choose(Uuid),Close(Uuid)}
#[derive(Clone)]struct Entry {id:Uuid,name:String,detail:String,direct:bool,unread:bool}
#[derive(Default)]pub(super) struct Switcher {view:Option<View>,recent:VecDeque<Uuid>}
struct View {token:Uuid,window:gtk::Window,query:gtk::SearchEntry,list:gtk::ListBox,scroll:gtk::ScrolledWindow,empty:gtk::Label,ids:Vec<Uuid>,focus:Option<gtk::glib::WeakRef<gtk::Widget>>}
impl Drop for Switcher{fn drop(&mut self){if let Some(view)=self.view.take(){view.window.close();}}}
fn rank(mut entries:Vec<Entry>,query:&str,recent:&VecDeque<Uuid>)->Vec<Entry>{
    let query=query.trim().trim_start_matches(['#','@']).to_lowercase();let words:Vec<_>=query.split_whitespace().collect();
    entries.retain(|e|{let text=format!("{} {}",e.name,e.detail).to_lowercase();words.iter().all(|w|text.contains(w))});
    entries.sort_by_cached_key(|e|{let name=e.name.to_lowercase();(if query.is_empty()||name==query{0}else if name.starts_with(&query){1}else{2},!e.unread,recent.iter().position(|id|*id==e.id).unwrap_or(usize::MAX),name,e.id)});
    entries.truncate(100);entries
}
impl MainWindowModel {
    pub(super) fn switcher_open(&self)->bool{self.switcher.view.is_some()}
    pub(super) fn remember_conversation(&mut self,id:Uuid){self.switcher.recent.retain(|old|*old!=id);self.switcher.recent.push_front(id);self.switcher.recent.truncate(50);}
    fn switch_entries(&self)->Vec<Entry>{
        let mut items:Vec<_>=self.channels.iter().filter(|c|c.supports_messages()&&self.access.get(&c.id).is_some_and(|a|a.read)).map(|c|Entry{id:c.id,name:c.name.clone(),detail:self.server.as_ref().map(|s|s.name.clone()).unwrap_or_default(),direct:false,unread:c.has_unread()}).collect();
        items.extend(self.direct.items.iter().map(|d|Entry{id:d.id,name:d.user.display_name().into(),detail:format!("@{}",d.user.username),direct:true,unread:d.unread_count>0}));items
    }
    pub(super) fn switch_event(&mut self,msg:SwitchMsg,root:&gtk::Box,sender:ComponentSender<Self>){match msg{
        SwitchMsg::Open=>{
            if let Some(v)=&self.switcher.view{v.window.present();v.query.grab_focus();return;}
            let token=Uuid::new_v4();let parent=root.root().and_downcast::<gtk::Window>();let focus=parent.as_ref().and_then(|w|GtkWindowExt::focus(w)).map(|w|w.downgrade());
            let shell=adw::Window::builder().title(crate::i18n::tr("Alternar conversa")).default_width(540).default_height(480).modal(true).build();let window:gtk::Window=shell.clone().upcast();if let Some(p)=parent{window.set_transient_for(Some(&p));}
            let toolbar=adw::ToolbarView::new();let header=adw::HeaderBar::new();header.set_title_widget(Some(&gtk::Label::new(Some(crate::i18n::tr("Alternar conversa")))));toolbar.add_top_bar(&header);
            let body=gtk::Box::new(gtk::Orientation::Vertical,8);body.set_margin_start(12);body.set_margin_end(12);body.set_margin_top(12);body.set_margin_bottom(12);
            let query=gtk::SearchEntry::new();query.set_placeholder_text(Some(crate::i18n::tr("Pesquisar canais e mensagens diretas")));query.set_widget_name("conversation-switcher-query");body.append(&query);
            let list=gtk::ListBox::new();list.set_selection_mode(gtk::SelectionMode::Single);list.add_css_class("boxed-list");list.set_activate_on_single_click(true);
            let scroll=gtk::ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).child(&list).build();body.append(&scroll);
            let empty=gtk::Label::new(Some(crate::i18n::tr("Nenhuma conversa encontrada")));empty.set_wrap(true);body.append(&empty);toolbar.set_content(Some(&body));shell.set_content(Some(&toolbar));
            let input=sender.input_sender().clone();query.connect_search_changed(move |_|{let _=input.send(MainWindowMsg::Switcher(SwitchMsg::Filter));});
            let input=sender.input_sender().clone();list.connect_row_activated(move|_,row|{if let Ok(id)=row.widget_name().parse(){let _=input.send(MainWindowMsg::Switcher(SwitchMsg::Choose(id)));}});
            let input=sender.input_sender().clone();let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);keys.set_name(Some("conversation-switcher-keys"));let rows=list.clone();let weak=window.downgrade();
            keys.connect_key_pressed(move|_,key,_,mods|{if !mods.is_empty(){return gtk::glib::Propagation::Proceed;}match key{
                gtk::gdk::Key::Escape=>{if let Some(w)=weak.upgrade(){w.close();}},
                gtk::gdk::Key::Down=>{let _=input.send(MainWindowMsg::Switcher(SwitchMsg::Move(1)));},gtk::gdk::Key::Up=>{let _=input.send(MainWindowMsg::Switcher(SwitchMsg::Move(-1)));},
                gtk::gdk::Key::Return|gtk::gdk::Key::KP_Enter=>{if let Some(row)=rows.selected_row(){if let Ok(id)=row.widget_name().parse(){let _=input.send(MainWindowMsg::Switcher(SwitchMsg::Choose(id)));}}},_=>return gtk::glib::Propagation::Proceed}gtk::glib::Propagation::Stop});window.add_controller(keys);
            let input=sender.input_sender().clone();window.connect_close_request(move |_|{let _=input.send(MainWindowMsg::Switcher(SwitchMsg::Close(token)));gtk::glib::Propagation::Proceed});
            self.switcher.view=Some(View{token,window:window.clone(),query:query.clone(),list,scroll,empty,ids:vec![],focus});self.switch_event(SwitchMsg::Filter,root,sender);window.present();query.grab_focus();
        },
        SwitchMsg::Filter=>{let query=self.switcher.view.as_ref().map(|v|v.query.text().to_string()).unwrap_or_default();let entries=rank(self.switch_entries(),&query,&self.switcher.recent);if let Some(v)=&mut self.switcher.view{
            while let Some(child)=v.list.first_child(){v.list.remove(&child);}v.ids.clear();
            for e in entries{let row=gtk::ListBoxRow::new();row.set_widget_name(&e.id.to_string());let line=gtk::Box::new(gtk::Orientation::Horizontal,12);line.set_margin_start(12);line.set_margin_end(12);line.set_margin_top(10);line.set_margin_bottom(10);
                line.append(&gtk::Image::from_icon_name(if e.direct{"papo-avatar-default-symbolic"}else{"papo-chat-bubbles-text-symbolic"}));let text=gtk::Box::new(gtk::Orientation::Vertical,2);text.set_hexpand(true);let name=gtk::Label::new(Some(&e.name));name.set_xalign(0.0);name.set_ellipsize(gtk::pango::EllipsizeMode::End);text.append(&name);let detail=gtk::Label::new(Some(&e.detail));detail.set_xalign(0.0);detail.set_ellipsize(gtk::pango::EllipsizeMode::End);detail.add_css_class("caption");detail.add_css_class("dim-label");text.append(&detail);line.append(&text);if e.unread{let badge=gtk::Label::new(Some(crate::i18n::tr("Não lidas")));badge.add_css_class("accent");line.append(&badge);}row.set_child(Some(&line));v.list.append(&row);v.ids.push(e.id);
            }v.empty.set_visible(v.ids.is_empty());v.list.select_row(v.list.row_at_index(0).as_ref());
        }},
        SwitchMsg::Move(delta)=>{if let Some(v)=&self.switcher.view{let index=v.list.selected_row().map_or(0,|r|r.index());let next=(index+delta).clamp(0,v.ids.len().saturating_sub(1) as i32);if let Some(row)=v.list.row_at_index(next){v.list.select_row(Some(&row));if let Some(bounds)=row.compute_bounds(&v.list){let adj=v.scroll.vadjustment();let top=f64::from(bounds.y());let bottom=top+f64::from(bounds.height());if top<adj.value(){adj.set_value(top);}else if bottom>adj.value()+adj.page_size(){adj.set_value(bottom-adj.page_size());}}}}},
        SwitchMsg::Choose(id)=>{if !self.switch_entries().iter().any(|e|e.id==id){self.switch_event(SwitchMsg::Filter,root,sender);return;}if let Some(v)=self.switcher.view.take(){v.window.close();}self.layout.dismiss_navigation();
            if let Some(channel)=self.channels.iter().find(|c|c.id==id).cloned(){self.select_channel(channel,sender);}else{self.direct_event(DirectMsg::Select(id,None),sender,root);}self.chat.emit(ChatMsg::FocusComposer);
        },
        SwitchMsg::Close(token)=>{if self.switcher.view.as_ref().is_some_and(|v|v.token==token){if let Some(v)=self.switcher.view.take(){v.window.close();if let Some(w)=v.focus.and_then(|w|w.upgrade()).filter(|w|w.is_mapped()&&w.is_sensitive()){w.grab_focus();}else{self.chat.emit(ChatMsg::FocusComposer);}}}},
    }}
}
#[cfg(test)]mod tests{use super::*;#[test]fn ranking_filters_and_prioritizes_names_unread_and_recent(){let ids:[Uuid;3]=std::array::from_fn(|_|Uuid::new_v4());let make=|i,name:&str,unread|Entry{id:ids[i],name:name.into(),detail:"@person".into(),direct:false,unread};let entries=vec![make(0,"general",false),make(1,"game",true),make(2,"garden",false)];let recent=VecDeque::from([ids[2]]);assert_eq!(rank(entries.clone(),"ga",&recent).iter().map(|e|e.id).collect::<Vec<_>>(),vec![ids[1],ids[2]]);assert_eq!(rank(entries.clone(),"#general",&recent)[0].id,ids[0]);assert_eq!(rank(entries.clone(),"@person",&recent).len(),3);assert!(rank(entries,"missing",&recent).is_empty());}}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until};
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());let own=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();let peer=Uuid::new_v4();let dm=Uuid::new_v4();
    backend.lock().unwrap().dms=vec![serde_json::json!({"id":dm,"created_at":"2026-11-04T12:00:00Z","unread_count":2,"user":{"id":peer,"username":"switch-peer","nickname":"Amélia","created_at":"2026-11-04T12:00:00Z"}})];
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:own,api_client:api}).detach();let window=adw::Window::builder().default_width(1100).default_height(780).content(main.widget()).build();window.present();main.emit(MainWindowMsg::StartLoading);until(context,||main.model().channels.len()>=2&&main.model().direct.items.iter().any(|d|d.id==dm));
    let field=crate::ui::chat::composer::field(main.model().chat.widget().upcast_ref());field.grab_focus();field.set_text("Draft stays in its channel");pump(context);let previous=main.model().active_channel_id.unwrap();
    main.widget().activate_action("papo.switcher",None).unwrap();until(context,||main.model().switcher.view.is_some());let popup=main.model().switcher.view.as_ref().unwrap().window.clone();
    let query=main.model().switcher.view.as_ref().unwrap().query.clone();query.set_text("amé");until(context,||main.model().switcher.view.as_ref().unwrap().ids==vec![dm]);
    let keys=popup.observe_controllers();let key=(0..keys.n_items()).find_map(|i|keys.item(i).and_downcast::<gtk::EventControllerKey>().filter(|k|k.name().as_deref()==Some("conversation-switcher-keys"))).unwrap();let press=|k|key.emit_by_name::<bool>("key-pressed",&[&k,&0u32,&gtk::gdk::ModifierType::empty()]);
    press(gtk::gdk::Key::Down);pump(context);assert!(query.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));press(gtk::gdk::Key::Return);until(context,||main.model().active_channel_id==Some(dm));assert!(!popup.is_visible());assert!(field.text().is_empty());
    main.emit(MainWindowMsg::Switcher(SwitchMsg::Open));until(context,||main.model().switcher.view.is_some());let popup=main.model().switcher.view.as_ref().unwrap().window.clone();let query=main.model().switcher.view.as_ref().unwrap().query.clone();query.set_text("does-not-exist");until(context,||main.model().switcher.view.as_ref().unwrap().ids.is_empty());popup.close();until(context,||main.model().switcher.view.is_none());until(context,||field.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN));
    main.emit(MainWindowMsg::Switcher(SwitchMsg::Open));until(context,||main.model().switcher.view.is_some());{main.state().get_mut().model.access.remove(&previous);}main.emit(MainWindowMsg::Switcher(SwitchMsg::Choose(previous)));pump(context);assert_eq!(main.model().active_channel_id,Some(dm));assert!(!main.model().switcher.view.as_ref().unwrap().ids.contains(&previous));main.model().switcher.view.as_ref().unwrap().window.close();pump(context);
    main.widget().activate_action("papo.search",None).unwrap();until(context,||gtk::Window::list_toplevels().iter().filter_map(|w|w.downcast_ref::<gtk::Window>()).any(|w|w.title().as_deref()==Some("Pesquisar mensagens")&&w.is_visible()));
    window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}
