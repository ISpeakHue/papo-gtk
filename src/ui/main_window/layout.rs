//! EN-US: Native adaptive navigation: server rail + channel list, chat, utility pane.
//! PT-BR: Navegação adaptativa nativa com lista de servidores, canais, chat e painel auxiliar.
use super::*;
use adw::prelude::*;

pub(super) struct Layout {
    pub widget:adw::BreakpointBin,
    pub navigation:adw::OverlaySplitView,
    pub members:adw::OverlaySplitView,
    header_settings:gtk::Settings,
    header_scale_handler:Option<gtk::glib::SignalHandlerId>,
}
impl Layout {
    pub fn new(sidebar:&gtk::Box,chat:&Controller<ChatModel>,members:&gtk::Box,s:&ComponentSender<MainWindowModel>)->Self {
        let center=gtk::Box::new(gtk::Orientation::Vertical,0);
        center.append(chat.widget());
        let utility=adw::OverlaySplitView::new();utility.set_content(Some(&center));utility.set_sidebar(Some(members));utility.set_sidebar_position(gtk::PackType::End);utility.set_min_sidebar_width(224.0);utility.set_max_sidebar_width(224.0);utility.set_sidebar_width_unit(adw::LengthUnit::Sp);utility.set_show_sidebar(true);
        let header=ChatModel::take_header(chat);
        let content=adw::ToolbarView::new();content.add_top_bar(&header);content.set_content(Some(&utility));
        let adaptive_content=adw::BreakpointBin::new();adaptive_content.set_size_request(320,240);adaptive_content.set_child(Some(&content));let (header_settings,header_scale_handler)=ChatModel::adapt_header(chat,&adaptive_content,&utility);
        let navigation=adw::OverlaySplitView::new();navigation.set_sidebar(Some(sidebar));navigation.set_content(Some(&adaptive_content));navigation.set_min_sidebar_width(320.0);navigation.set_max_sidebar_width(320.0);navigation.set_sidebar_width_unit(adw::LengthUnit::Sp);navigation.set_show_sidebar(true);
        let widget=adw::BreakpointBin::new();widget.set_child(Some(&navigation));widget.set_size_request(360,320);
        let medium=adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth,1100.0,adw::LengthUnit::Sp));
        medium.add_setters(&[(&utility,"collapsed",true),(&utility,"show-sidebar",false)]);widget.add_breakpoint(medium);
        let narrow=adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth,760.0,adw::LengthUnit::Sp));
        narrow.add_setters(&[(&navigation,"collapsed",true),(&navigation,"show-sidebar",false),(&utility,"collapsed",true),(&utility,"show-sidebar",false)]);widget.add_breakpoint(narrow);
        let s=s.clone();utility.connect_show_sidebar_notify(move |view|s.input(MainWindowMsg::MembersVisible(view.shows_sidebar())));
        Self{widget,navigation,members:utility,header_settings,header_scale_handler:Some(header_scale_handler)}
    }
    pub fn dismiss_navigation(&self){if self.navigation.is_collapsed(){self.navigation.set_show_sidebar(false);}}
}

impl Drop for Layout {
    fn drop(&mut self){if let Some(handler)=self.header_scale_handler.take(){self.header_settings.disconnect(handler);}}
}

pub(super) fn shortcuts(root:&gtk::Box,s:&ComponentSender<MainWindowModel>){
    let actions=gtk::gio::SimpleActionGroup::new();
    for (name,kind) in [("search",0),("preferences",1),("notifications",2),("navigation",3),("members",4),("channels",5),("direct",6)]{
        let action=gtk::gio::SimpleAction::new(name,None);let s=s.clone();action.connect_activate(move |_,_|s.input(match kind{
            0=>MainWindowMsg::Search(SearchMsg::Open),1=>MainWindowMsg::Account(AccountMsg::Preferences),2=>MainWindowMsg::Notification(NoticeMsg::Open),3=>MainWindowMsg::ToggleNavigation,4=>MainWindowMsg::ToggleUserList,5=>MainWindowMsg::SidebarMode(false),_=>MainWindowMsg::SidebarMode(true),
        }));actions.add_action(&action);
    }
    root.insert_action_group("papo",Some(&actions));
    let controller=gtk::ShortcutController::new();controller.set_scope(gtk::ShortcutScope::Managed);
    for (trigger,action) in [("<Control>k","search"),("<Control>f","search"),("<Control>comma","preferences"),("<Control><Shift>m","members"),("<Alt>1","channels"),("<Alt>2","direct"),("F9","navigation")]{
        controller.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(&trigger.replace("<Control>",&crate::platform::shortcuts::accelerator(""))),Some(gtk::NamedAction::new(&format!("papo.{action}")))));
    }
    root.add_controller(controller);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ui::chat::actions::tests::{pump,until,descendants,find_button};

    fn snapshot(window:&impl IsA<gtk::Window>,name:&str,context:&gtk::glib::MainContext){
        if std::env::var_os("PAPO_DESIGN_PREVIEW").is_none(){return;}
        let window=window.as_ref();
        let content=window.upcast_ref::<gtk::Widget>();
        let paintable=gtk::WidgetPaintable::new(Some(content));content.queue_draw();
        let node=std::cell::RefCell::new(None);
        until(context,||{let snapshot=gtk::Snapshot::new();paintable.snapshot(&snapshot,content.width() as f64,content.height() as f64);*node.borrow_mut()=snapshot.to_node();node.borrow().is_some()});
        let node=node.into_inner().unwrap();
        let texture=window.renderer().unwrap().render_texture(&node,Some(&gtk::graphene::Rect::new(0.0,0.0,content.width() as f32,content.height() as f32)));
        texture.save_to_png(format!("/tmp/papo-design-{name}.png")).unwrap();
    }
    fn settle(context:&gtk::glib::MainContext){for _ in 0..120{pump(context);std::thread::sleep(Duration::from_millis(5));}}
    pub(crate) fn preview(window:&impl IsA<gtk::Window>,name:&str,context:&gtk::glib::MainContext){if std::env::var_os("PAPO_DESIGN_PREVIEW").is_some(){settle(context);snapshot(window,name,context);}}

    fn assert_chat_center(main:&Controller<MainWindowModel>,title:&adw::WindowTitle,header:&adw::HeaderBar){
        let title=title.compute_bounds(header).unwrap();let chat=main.model().chat.widget().compute_bounds(header).unwrap();
        let offset=title.x()+title.width()/2.0-chat.x()-chat.width()/2.0;
        assert!(offset.abs()<=1.0,"title must follow the chat center, offset {offset}px");
    }

    pub(crate) fn exercise_header(main:&Controller<MainWindowModel>,window:&adw::ApplicationWindow,context:&gtk::glib::MainContext){
        until(context,||main.model().active_channel_id.is_some()&&main.model().background_started);
        let header=descendants(main.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::HeaderBar>().ok()).unwrap();
        let title=descendants(header.upcast_ref()).into_iter().find_map(|w|w.downcast::<adw::WindowTitle>().ok()).unwrap();
        assert!(header.shows_end_title_buttons());
        assert!(!descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.is::<adw::HeaderBar>()),"the conversation has no duplicate content toolbar");
        let channel=main.model().channels.iter().find(|c|Some(c.id)==main.model().active_channel_id).unwrap().clone();
        assert_eq!(title.title(),format!("# {}",channel.name));
        let mut edited=channel.clone();edited.name="Canal de teste".into();edited.topic=Some("Um tópico longo para conferir que o cabeçalho permanece compacto. ".repeat(12));
        main.model().chat.emit(ChatMsg::UpdateChannel(edited.clone()));until(context,||title.title()=="# Canal de teste");
        assert_eq!(title.subtitle(),edited.topic.as_deref().unwrap());
        for width in [1280,900,540,360]{
            window.set_default_size(width,760);settle(context);
            preview(window,&format!("gnome-{width}"),context);
            assert!(window.width()<=width+2,"requested {width}, actual {}, header minimum {:?}, app minimum {:?}",window.width(),header.measure(gtk::Orientation::Horizontal,-1),main.widget().measure(gtk::Orientation::Horizontal,-1));
            // Topic length must not move the channel title toward either
            // toolbar group. Check actual allocations, including compact mode.
            let mut previous_center:Option<f32>=None;
            for topic in [None,Some("Curto".to_owned()),edited.topic.clone()]{
                let mut updated=edited.clone();updated.topic=topic;
                main.model().chat.emit(ChatMsg::UpdateChannel(updated));settle(context);
                let bounds=title.compute_bounds(&header).unwrap();
                let center=bounds.x()+bounds.width()/2.0;
                assert!(bounds.width()>0.0,"the channel name remains visible");
                let chat=main.model().chat.widget().compute_bounds(&header).unwrap();
                let offset=center-(chat.x()+chat.width()/2.0);
                assert!(offset.abs()<=1.0,"title shifted {offset}px at width {width}, topic {:?}",title.subtitle());
                if let Some(previous)=previous_center{assert!((center-previous).abs()<=1.0,"topic length moved the title at width {width}");}
                previous_center=Some(center);
            }
            let model=main.model();
            let bounds=|w:&gtk::Widget|w.compute_bounds(window).unwrap();
            let pane=bounds(main.widget().upcast_ref());let bar=bounds(header.upcast_ref());let chat=bounds(model.chat.widget().upcast_ref());
            assert!((bar.y()-pane.y()).abs()<=1.0,"header starts at the top of the application");
            assert!(chat.y()>=bar.y()+bar.height()-1.0);
            assert!((bar.x()+bar.width()-pane.x()-pane.width()).abs()<=1.0,"header spans the member pane to the window controls");
            for tooltip in ["Fixadas","Buscar mensagens (Ctrl+K)","Notificações","Lista de Membros"]{
                let button=find_button(header.upcast_ref(),tooltip);
                let limit=720.0+if !model.layout.members.is_collapsed()&&model.layout.members.shows_sidebar(){224.0}else{0.0};
                assert_eq!(button.is_mapped(),header.width() as f64>limit||tooltip=="Notificações");
                if !button.is_mapped(){continue;}
                let button=bounds(button.upcast_ref());assert!(button.x()>=bar.x()&&button.x()+button.width()<=bar.x()+bar.width()+1.0);
            }
            if width>760{
                let sidebar=bounds(model.sidebar.widget().upcast_ref());assert!((sidebar.y()-pane.y()).abs()<=1.0);assert!((sidebar.height()-pane.height()).abs()<=1.0);
            }else{assert!(model.layout.navigation.is_collapsed());assert!(!model.layout.navigation.shows_sidebar());}
            assert_eq!(model.layout.members.is_collapsed(),width<=1100);
            drop(model);
        }
        let menu=descendants(header.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::MenuButton>().ok()).unwrap();menu.popup();settle(context);
        for label in ["Mensagens fixadas","Pesquisar mensagens","Lista de membros"]{assert!(find_button(header.upcast_ref(),label).is_mapped());}
        find_button(header.upcast_ref(),"Lista de membros").emit_clicked();pump(context);assert!(main.model().layout.members.shows_sidebar());settle(context);assert_chat_center(main,&title,&header);main.model().layout.members.set_show_sidebar(false);pump(context);menu.popdown();
        find_button(header.upcast_ref(),"Canais e conversas (F9)").emit_clicked();pump(context);assert!(main.model().layout.navigation.shows_sidebar());
        main.model().layout.dismiss_navigation();pump(context);
        window.set_default_size(1280,760);settle(context);
        let members=main.model().layout.members.shows_sidebar();find_button(header.upcast_ref(),"Lista de Membros").emit_clicked();pump(context);assert_eq!(main.model().layout.members.shows_sidebar(),!members);
        settle(context);assert_chat_center(main,&title,&header);
        find_button(header.upcast_ref(),"Lista de Membros").emit_clicked();pump(context);settle(context);assert_chat_center(main,&title,&header);
        for visible in [false,true]{
            main.model().layout.navigation.set_show_sidebar(visible);settle(context);assert_chat_center(main,&title,&header);
        }
        for (tooltip,name) in [("Buscar mensagens (Ctrl+K)","Pesquisar mensagens"),("Notificações","Notificações")]{
            find_button(header.upcast_ref(),tooltip).emit_clicked();pump(context);
            let popup=gtk::Window::list_toplevels().into_iter().find_map(|w|w.downcast::<gtk::Window>().ok().filter(|w|w.title().as_deref()==Some(name))).unwrap();assert!(popup.is_visible());popup.close();pump(context);
        }
        main.model().chat.emit(ChatMsg::UpdateChannel(channel));pump(context);
    }

    pub(crate) fn exercise_refresh_budget(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext,backend:&std::sync::Arc<std::sync::Mutex<crate::ui::chat::actions::tests::Backend>>){
        until(context,||main.model().access_request.is_none());
        let baseline=backend.lock().unwrap().requests.len();
        for _ in 0..8 { main.emit(MainWindowMsg::RefreshAccess); }
        until(context,||main.model().access_request.is_none()&&backend.lock().unwrap().requests[baseline..].iter().filter(|(_,path,_)|path=="/channels").count()>=2);
        let backend=backend.lock().unwrap();let calls=&backend.requests[baseline..];
        assert_eq!(calls.iter().filter(|(_,path,_)|path=="/channels").count(),2,"a burst of refresh triggers needs one active snapshot and one follow-up");
        assert!(!calls.iter().any(|(_,path,_)|path.ends_with("/permissions")));
    }

    pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
        until(context,||main.model().users.len()>=2);
        let model=main.model();let selected=model.active_channel_id.unwrap();
        let saved_channels=model.channels.clone();let saved_direct=model.sidebar.model().direct.clone();let users=model.users.clone();let avatars=model.avatars.textures.clone();
        let mut channels=saved_channels.clone();
        for (kind,name,position) in [("category","Comunidade",-1),("text","desenvolvimento",2),("text","galeria",3),("category","Salas de voz",4),("voice","Papo de fim de tarde",5)]{
            channels.push(serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":name,"type":kind,"position":position,"created_at":"2026-10-07T12:00:00Z"})).unwrap());
        }
        channels.sort_by_key(|c|c.position.unwrap_or(0));model.sidebar.emit(SidebarMsg::SetChannels(channels));
        let mut added=vec![];
        let author=users.first().unwrap().id;let peer=users.iter().find(|u|u.id!=author).unwrap().id;
        for (who,text,time) in [(author,"Bom dia, pessoal! O papo já está com câmera e compartilhamento de tela. 🎉","2026-10-07T12:00:00Z"),(author,"Vamos testar a chamada na sala de voz depois do almoço?","2026-10-07T12:01:00Z"),(peer,"Bora! Gostei de poder compartilhar só uma janela.","2026-10-07T12:02:00Z"),(author,"Também organizei os canais. As novidades e ideias para o projeto ficam por aqui.","2026-10-07T12:03:00Z")]{
            let id=Uuid::new_v4();added.push(id);model.chat.emit(ChatMsg::AddMessage(serde_json::from_value(serde_json::json!({"id":id,"channel_id":selected,"author_id":who,"content":text,"created_at":time,"reactions":[{"unicode":"🎉","count":3},{"unicode":"❤️","count":2}]})).unwrap()));
        }
        let peers:Vec<_>=users.iter().map(|u|serde_json::from_value(serde_json::json!({"user_id":u.id,"status":"online"})).unwrap()).collect();model.user_list.emit(UserListMsg::PresenceSync(peers));
        drop(model);pump(context);
        let view=adw::ToolbarView::new();let header=adw::HeaderBar::new();header.set_title_widget(Some(&adw::WindowTitle::new("Papo","Comunidade")));view.add_top_bar(&header);view.set_content(Some(main.widget()));
        let window=adw::Window::builder().default_width(1280).default_height(760).content(&view).build();window.present();
        until(context,||window.width()>1100);settle(context);
        assert!(!main.model().layout.navigation.is_collapsed());assert!(!main.model().layout.members.is_collapsed());
        let style=adw::StyleManager::default();let scheme=style.color_scheme();
        style.set_color_scheme(adw::ColorScheme::ForceDark);settle(context);snapshot(&window,"dark",context);
        for (id,visible) in [(added[0],true),(added[1],false),(added[2],true)]{
            let row=descendants(main.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("message-{id}")).unwrap();
            let header=descendants(&row).into_iter().find(|w|w.has_css_class("papo-message-header")).unwrap();assert_eq!(header.get_visible(),visible,"consecutive messages group only for the same author");
        }
        // Keyboard focus exposes exactly the same actions as pointer hover.
        let row=descendants(main.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==format!("message-{}",added[0])).unwrap();
        let accepted=row.grab_focus();settle(context);let action=descendants(&row).into_iter().find_map(|w|w.downcast::<gtk::Revealer>().ok()).unwrap();assert!(action.reveals_child(),"focus accepted={accepted}, has_focus={}, flags={:?}, root_focus={:?}, revealed={}, mapped={}, parent={}, current_row={}",row.has_focus(),row.state_flags(),gtk::prelude::RootExt::focus(&window),action.reveals_child(),row.is_mapped(),row.parent().is_some(),descendants(main.widget().upcast_ref()).into_iter().find(|w|w.widget_name()==row.widget_name()).is_some_and(|w|w==row));
        let menu=descendants(&row).into_iter().find_map(|w|w.downcast::<gtk::MenuButton>().ok()).unwrap();menu.popup();settle(context);assert!(find_button(&row,"Quem reagiu").is_visible());menu.popdown();
        // Real actions still work from the compact toolbar.
        main.widget().activate_action("papo.direct",None).unwrap();pump(context);assert!(main.model().sidebar.model().direct_mode);
        main.widget().activate_action("papo.channels",None).unwrap();pump(context);assert!(!main.model().sidebar.model().direct_mode);
        main.widget().activate_action("papo.members",None).unwrap();pump(context);assert!(!main.model().layout.members.shows_sidebar());
        main.widget().activate_action("papo.members",None).unwrap();pump(context);assert!(main.model().layout.members.shows_sidebar());
        style.set_color_scheme(adw::ColorScheme::ForceLight);settle(context);snapshot(&window,"light",context);
        window.set_default_size(900,760);until(context,||window.width()<=1100);settle(context);assert!(!main.model().layout.navigation.is_collapsed());assert!(main.model().layout.members.is_collapsed());assert!(!main.model().layout.members.shows_sidebar());
        window.set_default_size(540,760);until(context,||window.width()<=760);settle(context);assert!(main.model().layout.navigation.is_collapsed());assert!(!main.model().layout.navigation.shows_sidebar());
        main.widget().activate_action("papo.navigation",None).unwrap();settle(context);assert!(main.model().layout.navigation.shows_sidebar());snapshot(&window,"narrow-navigation",context);
        main.emit(MainWindowMsg::ChannelSelected(saved_channels.iter().find(|c|c.id==selected).unwrap().clone()));settle(context);assert!(!main.model().layout.navigation.shows_sidebar());
        style.set_color_scheme(adw::ColorScheme::ForceDark);settle(context);snapshot(&window,"narrow",context);
        // Opening media controls does not force a wider chat window.
        let panel=main.model().voice.panel.clone();panel.set_visible(true);main.widget().activate_action("papo.navigation",None).unwrap();settle(context);assert!(panel.is_mapped());assert!(window.width()<=760);assert!(panel.measure(gtk::Orientation::Horizontal,-1).0<=360);snapshot(&window,"narrow-call",context);panel.set_visible(false);main.widget().activate_action("papo.navigation",None).unwrap();settle(context);
        window.set_default_size(1280,760);until(context,||window.width()>1100);settle(context);assert!(!main.model().layout.navigation.is_collapsed());assert!(!main.model().layout.members.is_collapsed());
        // Use the ListBox signal GTK emits for clicks, rather than direct component messages.
        if let Some(other)=saved_channels.iter().find(|c|c.id!=selected&&matches!(c.channel_type,None|Some(crate::models::ChannelType::Text))){
            let activate=|id|{let row=descendants(main.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ListBoxRow>().ok().filter(|r|r.widget_name()==format!("channel-{id}"))).unwrap();row.parent().and_downcast::<gtk::ListBox>().unwrap().emit_by_name::<()>("row-activated",&[&row]);};
            activate(other.id);until(context,||main.model().active_channel_id==Some(other.id));activate(selected);until(context,||main.model().active_channel_id==Some(selected)&&main.model().chat.model().active_channel.as_ref().is_some_and(|c|c.id()==selected));settle(context);
        }
        style.set_color_scheme(scheme);view.set_content(None::<&gtk::Widget>);window.close();
        let model=main.model();model.sidebar.emit(SidebarMsg::SetChannels(saved_channels));model.sidebar.emit(SidebarMsg::SetDirect(saved_direct));model.chat.emit(ChatMsg::SetAvatars(avatars));
        for id in added{model.chat.emit(ChatMsg::ApplyChange(Change::Delete(id)));}drop(model);pump(context);
    }
}
