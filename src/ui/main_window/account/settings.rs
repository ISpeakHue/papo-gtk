//! EN-US: Shared account settings shell. Pages mount once and retain their draft widgets.
//! PT-BR: Janela compartilhada de configurações; cada aba é montada uma vez e preserva seus campos de rascunho.
use super::*;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Tab {Profile,Preferences,Security}
impl Tab {
    fn name(self)->&'static str{match self{Self::Profile=>"profile",Self::Preferences=>"preferences",Self::Security=>"security"}}
}

pub(super) struct View {pub window:gtk::Window,allow_close:std::rc::Rc<std::cell::Cell<bool>>,prompt:std::cell::Cell<bool>,tabs:gtk::Stack,pages:Vec<(Tab,gtk::Box)>,requested:std::cell::RefCell<std::collections::HashSet<Tab>>}
impl View{pub(super) fn force_close(&self){self.allow_close.set(true);self.window.close();}}
impl MainWindowModel {
    pub(in crate::ui::main_window) fn settings_mount(&self,tab:Tab)->Option<(gtk::Window,gtk::Box)>{
        let view=self.account.settings.as_ref().filter(|v|v.window.is_visible())?;
        let page=&view.pages.iter().find(|(key,_)|*key==tab)?.1;
        Some((view.window.clone(),page.clone()))
    }
    pub(in crate::ui::main_window) fn clear_settings_tab(&self,tab:Tab,window:&gtk::Window){
        if let Some(view)=self.account.settings.as_ref().filter(|v|&v.window==window){
            view.requested.borrow_mut().remove(&tab);
            if let Some((_,page))=view.pages.iter().find(|(key,_)|*key==tab){while let Some(child)=page.first_child(){page.remove(&child);}}
        }
    }
    pub(in crate::ui::main_window) fn settings_form(&self,tab:Tab)->Option<(gtk::Window,gtk::Box)>{
        let (window,page)=self.settings_mount(tab)?;
        let body=gtk::Box::new(gtk::Orientation::Vertical,10);body.set_valign(gtk::Align::Start);
        body.set_margin_top(28);body.set_margin_bottom(28);body.set_margin_start(32);body.set_margin_end(32);
        let clamp=adw::Clamp::new();clamp.set_maximum_size(768);clamp.set_tightening_threshold(640);clamp.set_child(Some(&body));
        let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();page.append(&scroll);
        Some((window,body))
    }
    pub(super) fn open_settings(&mut self,root:&gtk::Box,sender:&ComponentSender<Self>){
        if let Some(view)=self.account.settings.as_ref().filter(|v|v.window.is_visible()){view.window.present();return;}
        let window:gtk::Window=adw::Window::builder().title("Configurações da conta").default_width(760).default_height(800).modal(true).build().upcast();
        window.set_widget_name("account-settings-window");
        window.add_css_class("papo-account-settings");
        window.set_size_request(320,400);
        if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
        let toolbar=adw::ToolbarView::new();let header=adw::HeaderBar::new();header.set_title_widget(Some(&adw::WindowTitle::new("Configurações da conta","")));toolbar.add_top_bar(&header);
        let body=gtk::Box::new(gtk::Orientation::Vertical,0);
        let tabs=gtk::Stack::new();tabs.set_widget_name("account-settings-tabs");tabs.set_vexpand(true);tabs.set_hhomogeneous(false);tabs.set_vhomogeneous(false);tabs.set_transition_type(gtk::StackTransitionType::None);
        let mut pages=Vec::new();
        for (tab,title) in [(Tab::Profile,"Meu perfil"),(Tab::Preferences,"Preferências"),(Tab::Security,"Segurança")]{
            let page=gtk::Box::new(gtk::Orientation::Vertical,0);page.set_vexpand(true);tabs.add_titled(&page,Some(tab.name()),title);pages.push((tab,page));
        }
        let navigation=gtk::Box::new(gtk::Orientation::Horizontal,0);navigation.add_css_class("papo-settings-navigation");
        let switcher=gtk::Box::new(gtk::Orientation::Horizontal,8);switcher.set_halign(gtk::Align::Center);switcher.set_hexpand(true);navigation.append(&switcher);
        let compact=adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 520sp").unwrap());
        let mut buttons:Vec<(Tab,gtk::ToggleButton)>=Vec::new();
        for (tab,title,icon) in [(Tab::Profile,"Meu perfil","papo-avatar-default-symbolic"),(Tab::Preferences,"Preferências","papo-preferences-system-symbolic"),(Tab::Security,"Segurança","papo-security-high-symbolic")]{
            let button=gtk::ToggleButton::new();button.add_css_class("flat");button.add_css_class("papo-settings-tab");button.set_widget_name(&format!("account-tab-{}",tab.name()));button.set_tooltip_text(Some(title));button.update_property(&[gtk::accessible::Property::Label(title)]);
            if let Some((_,first))=buttons.first(){button.set_group(Some(first));}
            let content=gtk::Box::new(gtk::Orientation::Horizontal,8);content.set_halign(gtk::Align::Center);let image=gtk::Image::from_icon_name(icon);image.set_pixel_size(18);content.append(&image);let label=gtk::Label::new(Some(title));content.append(&label);button.set_child(Some(&content));
            compact.add_setter(&label,"visible",Some(&false.to_value()));
            let weak=tabs.downgrade();button.connect_toggled(move |button|{if button.is_active(){if let Some(tabs)=weak.upgrade(){if tabs.visible_child_name().as_deref()!=Some(tab.name()){tabs.set_visible_child_name(tab.name());}}}});
            switcher.append(&button);buttons.push((tab,button));
        }
        buttons[0].1.set_active(true);
        tabs.connect_visible_child_name_notify(move |tabs|{for (tab,button) in &buttons{button.set_active(tabs.visible_child_name().as_deref()==Some(tab.name()));}});
        toolbar.add_top_bar(&navigation);body.append(&tabs);toolbar.set_content(Some(&body));let native=window.downcast_ref::<adw::Window>().unwrap();native.add_breakpoint(compact);native.set_content(Some(&toolbar));
        let input=sender.input_sender().clone();tabs.connect_visible_child_name_notify(move |tabs|{
            let tab=match tabs.visible_child_name().as_deref(){Some("preferences")=>Tab::Preferences,Some("security")=>Tab::Security,_=>Tab::Profile};
            let _=input.send(MainWindowMsg::Account(AccountMsg::SettingsTab(tab)));
        });
        let allow_close=std::rc::Rc::new(std::cell::Cell::new(false));let allowed=allow_close.clone();let input=sender.input_sender().clone();
        window.connect_close_request(move |_|{if allowed.get(){gtk::glib::Propagation::Proceed}else{let _=input.send(MainWindowMsg::Account(AccountMsg::CloseSettings));gtk::glib::Propagation::Stop}});
        self.account.close_after_save=false;
        self.account.settings=Some(View{window:window.clone(),allow_close,prompt:Default::default(),tabs,pages,requested:Default::default()});window.present();self.select_settings(Tab::Profile,sender);
    }
    pub(super) fn select_settings(&self,tab:Tab,sender:&ComponentSender<Self>){
        let Some(view)=self.account.settings.as_ref().filter(|v|v.window.is_visible())else{return;};
        // A newer click may already have changed the stack while this event
        // was queued. Never select an older tab again from its notification.
        if view.tabs.visible_child_name().as_deref()!=Some(tab.name()){return;}
        if view.pages.iter().find(|(key,_)|*key==tab).is_some_and(|(_,page)|page.first_child().is_some()){return;}
        // Coalesce tab events while the page builder is queued.
        if !view.requested.borrow_mut().insert(tab){return;}
        let msg=match tab{Tab::Profile=>MainWindowMsg::Account(AccountMsg::SettingsProfile),Tab::Preferences=>MainWindowMsg::Account(AccountMsg::SettingsPreferences),Tab::Security=>MainWindowMsg::Security(SecurityMsg::Open{embedded:true})};
        sender.input(msg);
    }
}

#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants,find_button};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    // Synthetic profile artwork exercises real image fetching and later removal.
    {
        use base64::Engine;
        let mut banner=std::io::Cursor::new(Vec::new());
        let pixels=image::ImageBuffer::from_fn(800,160,|x,y|image::Rgba([20+(x/8) as u8,35+(y/4) as u8,110+(x/8) as u8,255]));
        image::DynamicImage::ImageRgba8(pixels).write_to(&mut banner,image::ImageFormat::Png).unwrap();
        let mut avatar=std::io::Cursor::new(Vec::new());
        let pixels=image::ImageBuffer::from_fn(96,96,|x,y|image::Rgba(if (x==30||x==66)&&(28..40).contains(&y)||(y==65&&(25..72).contains(&x)){[25,35,65,255]}else{[255,207,90,255]}));
        image::DynamicImage::ImageRgba8(pixels).write_to(&mut avatar,image::ImageFormat::Png).unwrap();
        let mut b=backend.lock().unwrap();b.attachment_thumbnails.insert("/media/profile-banner".into(),banner.into_inner());
        b.profile["banner_media"]=serde_json::json!("profile-banner");b.profile["avatar_blob"]=serde_json::json!(base64::engine::general_purpose::STANDARD.encode(avatar.into_inner()));
        b.profile["roles"]=serde_json::json!([{"id":Uuid::new_v4(),"name":"Member","color":"#33b875"}]);
    }
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1100).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().background_started&&main.model().access_request.is_none()&&main.model().active_channel_id.is_some());
    let gear=find_button(main.widget().upcast_ref(),"Configurações da conta");gear.emit_clicked();
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.embedded&&v.fields.is_some()));
    let(window,tabs)={let model=main.model();let view=model.account.settings.as_ref().unwrap();(view.window.clone(),view.tabs.clone())};
    assert_eq!(tabs.pages().n_items(),3);assert_eq!(tabs.visible_child_name().as_deref(),Some("profile"));
    assert_eq!(main.model().account.profile.as_ref().unwrap().window,window);
    assert!(main.model().account.preferences.is_none(),"inactive pages are lazy");
    let requests=||backend.lock().unwrap().requests.clone();
    assert!(!requests().iter().any(|(_,path,_)|path=="/auth/connected_devices"));
    let (save,scroll)={let model=main.model();let v=model.account.profile.as_ref().unwrap();(v.save.as_ref().unwrap().clone(),v.body.ancestor(gtk::ScrolledWindow::static_type()).unwrap().downcast::<gtk::ScrolledWindow>().unwrap())};
    until(context,||save.height()>0&&scroll.vadjustment().upper()>scroll.vadjustment().page_size());
    wait_profile_layout(&main,&window,context);
    main.model().account.profile.as_ref().unwrap().hero.as_ref().unwrap().check_layout(&window,true);
    let y=save.compute_bounds(&window).unwrap().y();assert!(y>window.height() as f32-90.0);
    scroll.vadjustment().set_value(scroll.vadjustment().upper()-scroll.vadjustment().page_size());
    until(context,||main.model().account.profile.as_ref().unwrap().hero.as_ref().unwrap().widget.compute_bounds(&window).unwrap().y()<0.0);
    assert!((save.compute_bounds(&window).unwrap().y()-y).abs()<1.0,"scrolling keeps Save in the bottom bar");scroll.vadjustment().set_value(0.0);
    main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.set_text("Tabbed nickname");
    find_button(window.upcast_ref(),"Preferências").emit_clicked();until(context,||main.model().account.preferences.as_ref().is_some_and(|v|v.embedded));
    assert_eq!(main.model().account.preferences.as_ref().unwrap().window,window);
    until(context,||main.model().account.preferences.as_ref().unwrap().save.width()>0);
    {let model=main.model();let save=&model.account.preferences.as_ref().unwrap().save;let bounds=save.compute_bounds(&window).unwrap();assert!(bounds.y()+bounds.height()<=window.height() as f32,"the default preferences window must show the save action without scrolling");}
    if std::env::var_os("PAPO_DESIGN_PREVIEW").is_some(){
        let style=adw::StyleManager::default();let previous=style.color_scheme();
        for (scheme,name) in [(adw::ColorScheme::ForceDark,"account-settings-preferences"),(adw::ColorScheme::ForceLight,"account-settings-preferences-light")]{style.set_color_scheme(scheme);crate::ui::main_window::layout::tests::preview(&window,name,context);}
        style.set_color_scheme(previous);
    }
    main.model().account.preferences.as_ref().unwrap().theme.set_selected(2);main.model().account.preferences.as_ref().unwrap().motion.set_selected(2);
    {let model=main.model();let v=model.account.preferences.as_ref().unwrap();for (toggle,value) in v.toggles.iter().zip([false,true,true,false,false,true]){toggle.set_active(value);}}
    find_button(window.upcast_ref(),"Segurança").emit_clicked();until(context,||find_passwords(&window).len()==2&&descendants(window.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Encerrar sessão"))));
    let passwords=find_passwords(&window);passwords[0].set_text("Draft password");
    let baseline=requests();
    for tab in ["profile","preferences","security","profile"]{tabs.set_visible_child_name(tab);pump(context);}
    for tab in ["preferences","security","profile"]{tabs.set_visible_child_name(tab);}pump(context);
    assert_eq!(tabs.visible_child_name().as_deref(),Some("profile"),"queued changes must not switch back to an older tab");
    assert_eq!(main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.text(),"Tabbed nickname");
    assert_eq!(main.model().account.preferences.as_ref().unwrap().theme.selected(),2);assert_eq!(passwords[0].text(),"Draft password");
    assert_eq!(requests().iter().filter(|(method,_,_)|method=="GET").count(),baseline.iter().filter(|(method,_,_)|method=="GET").count(),"tab switches must reuse mounted forms and fetched sessions");
    gear.emit_clicked();pump(context);assert_eq!(main.model().account.settings.as_ref().unwrap().window,window);
    if std::env::var_os("PAPO_DESIGN_PREVIEW").is_some(){
        let style=adw::StyleManager::default();let previous=style.color_scheme();
        for (scheme,name) in [(adw::ColorScheme::ForceDark,"account-settings-profile"),(adw::ColorScheme::ForceLight,"account-settings-profile-light")]{style.set_color_scheme(scheme);crate::ui::main_window::layout::tests::preview(&window,name,context);}
        style.set_color_scheme(previous);
    }
    // Accepted profile writes retain the mounted form, preserving the window,
    // the currently selected tab and an unsaved preferences draft.
    let old=main.model().account.profile.as_ref().unwrap().token;
    main.emit(MainWindowMsg::Account(AccountMsg::Profile(main.model().current_user.id)));
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.embedded&&!v.loading));
    assert!(main.model().account.parked_profile.as_ref().is_some_and(|v|v.token==old),"preview preserves the mounted editor and unsaved draft");
    main.model().account.profile.as_ref().unwrap().window.close();until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.embedded&&v.token==old));
    assert_eq!(main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.text(),"Tabbed nickname");
    find_button(window.upcast_ref(),"Salvar perfil").emit_clicked();until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Profile failed")));
    find_button(window.upcast_ref(),"Salvar perfil").emit_clicked();tabs.set_visible_child_name("preferences");
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.token==old&&v.fields.is_some()&&!v.pending&&v.reconcile.is_none()&&v.error.text()=="Salvo."));
    assert!(window.is_visible());assert_eq!(main.model().account.profile.as_ref().unwrap().window,window);assert_eq!(tabs.visible_child_name().as_deref(),Some("preferences"));
    assert_eq!(main.model().current_user.nickname.as_deref(),Some("Tabbed nickname"));assert_eq!(main.model().account.preferences.as_ref().unwrap().theme.selected(),2);
    find_button(window.upcast_ref(),"Salvar preferências").emit_clicked();until(context,||main.model().account.preferences.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Settings failed")));
    assert_eq!(main.model().account.config.theme,Some(Theme::System));
    find_button(window.upcast_ref(),"Salvar preferências").emit_clicked();until(context,||main.model().account.config.theme==Some(Theme::Dark));assert_eq!(crate::media::animation::motion(),crate::config::Motion::Reduced);
    {let model=main.model();let config=&model.account.config;let d=config.display.as_ref().unwrap();let n=config.notifications.as_ref().unwrap();assert_eq!([d.show_timestamps,d.show_avatars,n.enabled,n.message_preview,n.sound,n.mentions],[Some(false),Some(true),Some(true),Some(false),Some(false),Some(true)],"each native switch saves its corresponding setting");}
    let (token,draft)={let model=main.model();let v=model.account.profile.as_ref().unwrap();(v.token,profile_values(v.fields.as_ref().unwrap()))};
    let generation=Uuid::new_v4();main.state().get_mut().model.account.profile.as_mut().unwrap().reconcile=Some(generation);
    main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().description.set_text("Draft entered during reconciliation");
    let mut accepted:UserProfile=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();accepted.nickname=Some("Normalized nickname".into());
    main.emit(MainWindowMsg::Account(AccountMsg::ProfileAccepted{token,generation,draft,result:Ok((accepted,None,None))}));pump(context);
    {let model=main.model();let fields=model.account.profile.as_ref().unwrap().fields.as_ref().unwrap();assert_eq!(fields.nickname.text(),"Normalized nickname");assert_eq!(profile_values(fields)[2],"Draft entered during reconciliation");assert!(model.account.profile_dirty());}
    tabs.set_visible_child_name("profile");pump(context);
    find_button(window.upcast_ref(),"Remover banner").emit_clicked();
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.pending&&v.reconcile.is_none()&&v.error.text()=="Salvo.")&&backend.lock().unwrap().profile["banner_media"].is_null());
    until(context,||main.model().account.profile.as_ref().unwrap().hero.as_ref().unwrap().widget.height()<144);
    wait_profile_layout(&main,&window,context);
    {let model=main.model();let v=model.account.profile.as_ref().unwrap();v.hero.as_ref().unwrap().check_layout(&window,false);assert_eq!(v.token,token);assert_eq!(profile_values(v.fields.as_ref().unwrap())[2],"Draft entered during reconciliation","image changes preserve the mounted profile draft");}
    tabs.set_visible_child_name("security");pump(context);super::super::security::exercise_controls(&main,context);
    passwords[0].set_text("Clear on close");passwords[1].set_text("Clear on close");
    window.set_default_size(390,650);until(context,||window.width()<=420);
    tabs.set_visible_child_name("profile");crate::ui::main_window::layout::tests::preview(&window,"account-settings-profile-narrow",context);
    wait_profile_layout(&main,&window,context);
    {let model=main.model();let v=model.account.profile.as_ref().unwrap();v.hero.as_ref().unwrap().check_layout(&window,false);let bounds=v.save.as_ref().unwrap().compute_bounds(&window).unwrap();assert!(bounds.y()>window.height() as f32-90.0&&bounds.y()+bounds.height()<=window.height() as f32,"the narrow profile keeps Save visible");}
    tabs.set_visible_child_name("preferences");
    until(context,||main.model().account.preferences.as_ref().unwrap().toggles.iter().all(|row|row.compute_bounds(&window).is_some_and(|b|b.width()>0.0&&b.x()>=0.0&&b.x()+b.width()<=window.width() as f32)));
    crate::ui::main_window::layout::tests::preview(&window,"account-settings-preferences-narrow",context);
    {let model=main.model();let save=&model.account.preferences.as_ref().unwrap().save;let bounds=save.compute_bounds(&window).unwrap();assert!(bounds.y()>=0.0&&bounds.y()+bounds.height()<=window.height() as f32,"the narrow preferences form keeps Save visible");}
    tabs.set_visible_child_name("security");crate::ui::main_window::layout::tests::preview(&window,"account-settings-narrow",context);
    until(context,||passwords.iter().all(|entry|entry.compute_bounds(&window).is_some_and(|b|b.width()>0.0&&b.x()>=0.0&&b.x()+b.width()<=window.width() as f32)));
    for entry in &passwords{let bounds=entry.compute_bounds(&window).unwrap();assert!(bounds.x()>=0.0&&bounds.x()+bounds.width()<=window.width() as f32,"password fields must fit the narrow settings window");assert!(entry.shows_peek_icon());}
    window.close();pump(context);let native=window.downcast_ref::<adw::Window>().unwrap();until(context,||native.visible_dialog().is_some());
    find_button(native.visible_dialog().unwrap().upcast_ref(),"Descartar").emit_clicked();until(context,||!window.is_visible());assert!(passwords.iter().all(|p|p.text().is_empty()));
    gear.emit_clicked();until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.window!=window&&v.fields.is_some()));
    let reopened=main.model().account.settings.as_ref().unwrap().window.clone();assert!(reopened.is_visible());
    assert_eq!(main.model().account.settings.as_ref().unwrap().tabs.visible_child_name().as_deref(),Some("profile"));
    assert_eq!(descendants(reopened.upcast_ref()).iter().filter(|w|w.is::<gtk::PasswordEntry>()).count(),0,"closed forms and secrets must not be remounted");
    main.model().account.settings.as_ref().unwrap().tabs.set_visible_child_name("security");
    until(context,||find_passwords(&reopened).len()==2&&descendants(reopened.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Encerrar sessão"))));
    assert!(find_passwords(&reopened).iter().all(|p|p.text().is_empty()));assert!(!window.is_visible(),"opening security must not resurrect the closed settings window");
    let tabs=main.model().account.settings.as_ref().unwrap().tabs.clone();tabs.set_visible_child_name("profile");pump(context);
    let nickname=main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.clone();nickname.set_text("Keep my draft");pump(context);
    assert!(find_button(reopened.upcast_ref(),"Salvar perfil").is_sensitive());
    reopened.close();let native=reopened.downcast_ref::<adw::Window>().unwrap();until(context,||native.visible_dialog().is_some());assert!(reopened.is_visible());
    find_button(native.visible_dialog().unwrap().upcast_ref(),"Cancelar").emit_clicked();until(context,||native.visible_dialog().is_none());assert_eq!(nickname.text(),"Keep my draft");assert!(reopened.is_visible());
    reopened.close();until(context,||native.visible_dialog().is_some());find_button(native.visible_dialog().unwrap().upcast_ref(),"Descartar").emit_clicked();until(context,||!reopened.is_visible());
    gear.emit_clicked();until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.window!=reopened&&v.fields.is_some()));
    let final_window=main.model().account.settings.as_ref().unwrap().window.clone();let final_native=final_window.downcast_ref::<adw::Window>().unwrap();
    assert_ne!(main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.text(),"Keep my draft");
    main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.set_text("Save before closing");pump(context);final_window.close();until(context,||final_native.visible_dialog().is_some());find_button(final_native.visible_dialog().unwrap().upcast_ref(),"Salvar").emit_clicked();
    until(context,||!final_window.is_visible());assert_eq!(main.model().current_user.nickname.as_deref(),Some("Save before closing"));
    find_button(main.widget().upcast_ref(),"Meu perfil").emit_clicked();
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.loading&&!v.embedded));
    {let model=main.model();let preview=model.account.profile.as_ref().unwrap();assert!(preview.fields.is_none()&&preview.save.is_none(),"footer opens a read-only own profile, editing stays in settings");
    assert!(!descendants(preview.window.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.label().as_deref()==Some("Mensagem direta"))),"never offer DM to self");
    preview.window.close();}
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
#[cfg(test)]
fn find_passwords(window:&gtk::Window)->Vec<gtk::PasswordEntry>{
    use crate::ui::chat::actions::tests::descendants;
    let tabs=descendants(window.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Stack>().ok().filter(|w|w.widget_name()=="account-settings-tabs")).unwrap();
    // Inactive GTK Stack pages remain owned even when outside the drawn hierarchy.
    descendants(&tabs.child_by_name("security").unwrap()).into_iter().filter_map(|w|w.downcast::<gtk::PasswordEntry>().ok()).collect()
}

pub(super) fn profile_values(f:&ProfileFields)->Vec<String>{vec![f.nickname.text().into(),f.status.text().into(),f.description.text(&f.description.start_iter(),&f.description.end_iter(),false).into(),f.typing.text().into(),f.manual.selected().to_string()]}
pub(super) fn preferences_values(v:&PreferencesView)->Vec<String>{std::iter::once(v.theme.selected().to_string()).chain([v.font.selected().to_string(),v.density.selected().to_string(),v.motion.selected().to_string()]).chain(v.toggles.iter().map(|c|c.is_active().to_string())).collect()}
pub(super) fn watch_profile(f:&ProfileFields,sender:&ComponentSender<MainWindowModel>){
    for entry in [&f.nickname,&f.status,&f.typing]{let s=sender.clone();entry.connect_changed(move |_|s.input(MainWindowMsg::Account(AccountMsg::Dirty)));}
    let s=sender.clone();f.description.connect_changed(move |_|s.input(MainWindowMsg::Account(AccountMsg::Dirty)));
    let s=sender.clone();f.manual.connect_selected_notify(move |_|s.input(MainWindowMsg::Account(AccountMsg::Dirty)));
}
pub(super) fn watch_preferences(v:&PreferencesView,sender:&ComponentSender<MainWindowModel>){
    for dropdown in [&v.theme,&v.font,&v.density,&v.motion]{let s=sender.clone();dropdown.connect_selected_notify(move |_|s.input(MainWindowMsg::Account(AccountMsg::Dirty)));}
    for toggle in &v.toggles{let s=sender.clone();toggle.connect_active_notify(move |_|s.input(MainWindowMsg::Account(AccountMsg::Dirty)));}
}
impl Account {
    fn profile_dirty(&self)->bool{self.profile.as_ref().filter(|v|v.embedded).or(self.parked_profile.as_ref()).is_some_and(|v|v.embedded&&v.fields.as_ref().is_some_and(|f|profile_values(f)!=v.baseline))}
    fn preferences_dirty(&self)->bool{self.preferences.as_ref().is_some_and(|v|v.embedded&&preferences_values(v)!=v.baseline)}
    fn pending(&self)->bool{self.mutation.is_some()||self.settings_pending||self.profile.as_ref().is_some_and(|v|v.reconcile.is_some())||self.parked_profile.as_ref().is_some_and(|v|v.reconcile.is_some())}
}
impl MainWindowModel {
    pub(super) fn open_settings_at(&mut self,tab:Tab,root:&gtk::Box,sender:&ComponentSender<Self>){
        self.restore_profile_editor();
        self.open_settings(root,sender);
        if let Some(v)=&self.account.settings{v.tabs.set_visible_child_name(tab.name());}
        self.select_settings(tab,sender);
    }
    pub(super) fn settings_dirty(&self){
        if let Some(v)=&self.account.profile{if let Some(save)=&v.save{save.set_sensitive(!v.pending&&v.reconcile.is_none()&&self.account.mutation.is_none()&&v.fields.as_ref().is_some_and(|f|profile_values(f)!=v.baseline));}}
        if let Some(v)=&self.account.preferences{v.save.set_sensitive(!v.pending&&preferences_values(v)!=v.baseline);}
    }
    pub(super) fn request_settings_close(&mut self,sender:&ComponentSender<Self>){
        self.restore_profile_editor();
        let Some(v)=self.account.settings.as_ref().filter(|v|v.window.is_visible())else{return;};
        if self.account.pending(){return;}
        if !self.account.profile_dirty()&&!self.account.preferences_dirty(){v.allow_close.set(true);v.window.close();return;}
        if v.prompt.replace(true){return;}
        let dialog=adw::AlertDialog::builder().heading("Salvar alterações?").body("Há alterações no perfil ou nas preferências que ainda não foram salvas.").build();
        dialog.add_responses(&[("cancel","Cancelar"),("discard","Descartar"),("save","Salvar")]);dialog.set_close_response("cancel");dialog.set_default_response(Some("save"));dialog.set_response_appearance("discard",adw::ResponseAppearance::Destructive);dialog.set_response_appearance("save",adw::ResponseAppearance::Suggested);
        let s=sender.clone();dialog.connect_response(None,move |_,response|s.input(MainWindowMsg::Account(AccountMsg::CloseChoice(response.into()))));dialog.present(Some(&v.window));
    }
    pub(super) fn settings_close_choice(&mut self,choice:&str,sender:&ComponentSender<Self>){
        let Some(v)=self.account.settings.as_ref().filter(|v|v.window.is_visible())else{return;};v.prompt.set(false);
        match choice{
            "discard"=>{v.allow_close.set(true);v.window.close();},
            "save"=>{
                self.account.close_after_save=true;
                if self.account.profile_dirty(){if let Some(v)=&self.account.profile{sender.input(MainWindowMsg::Account(AccountMsg::SaveProfile(v.token)));}}
                if self.account.preferences_dirty(){if let Some(v)=&self.account.preferences{sender.input(MainWindowMsg::Account(AccountMsg::SavePreferences(v.token)));}}
            },_=>{}
        }
    }
    pub(super) fn finish_settings_close(&mut self){
        if self.account.close_after_save&&!self.account.pending()&&!self.account.profile_dirty()&&!self.account.preferences_dirty(){
            self.account.close_after_save=false;if let Some(v)=&self.account.settings{v.allow_close.set(true);v.window.close();}
        }
    }
}

#[cfg(test)]
fn wait_profile_layout(main:&Controller<MainWindowModel>,window:&gtk::Window,context:&gtk::glib::MainContext){
    crate::ui::chat::actions::tests::until(context,||main.model().account.profile.as_ref().unwrap().hero.as_ref().unwrap().widget.compute_bounds(window).is_some_and(|b|b.width()>0.0&&b.x()>=0.0&&b.x()+b.width()<=window.width() as f32));
}
