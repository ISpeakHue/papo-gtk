//! EN-US: Profile and preferences dialogs; accepted writes are the source of local state.
//! PT-BR: Diálogos de perfil e preferências; o estado local acompanha as gravações aceitas pelo servidor.
use super::*;
use adw::prelude::*;
use crate::models::*;
use crate::ui::chat::actions::window;
pub(crate) mod settings;
mod preferences;
mod profile;
use settings::{profile_values,preferences_values,watch_profile,watch_preferences};
#[derive(Debug)]
pub enum AccountMsg {
    Dirty, CloseSettings, CloseChoice(String), ProfileAccepted{token:Uuid,generation:Uuid,draft:Vec<String>,result:anyhow::Result<(UserProfile,Option<Vec<u8>>,Option<crate::media::PreparedImage>)>},
    Settings, SettingsTab(settings::Tab), SettingsProfile, SettingsPreferences,
    ChangeStatus(Option<UserStatus>),
    StatusSaved { token:Uuid, status:Option<UserStatus>, result:anyhow::Result<()> },
    EmojisChanged, DescriptionChanged(Uuid), FitProfile(Uuid),
    PreviewClosed(Uuid), Profile(Uuid), RefreshProfile(Uuid), ProfileRefreshReady{token:Uuid,id:Uuid}, Preferences, ChannelPreferences, ProfileLoaded {
        token: Uuid, result: anyhow::Result<(UserProfile, Option<Vec<u8>>)>
    }, ProfilePrepared{token:Uuid,result:anyhow::Result<(UserProfile,Option<Vec<u8>>,Option<crate::media::PreparedImage>)>}, SaveProfile(Uuid), Image {
        token: Uuid, banner: bool, remove: bool
    }, ImageSelected {
        token: Uuid, banner: bool, result: anyhow::Result<Vec<u8>>
    }, ProfileSaved {
        token: Uuid, result: anyhow::Result<()>
    }, OwnUpdated {
        token: Uuid, result: anyhow::Result<WhoamiResponse>
    }, SavePreferences(Uuid), PreferencesSaved {
        token: Uuid, motion:crate::config::Motion, result: anyhow::Result<UserSettings>
    }, SaveChannel {
        token: Uuid, channel: Uuid, value: NotificationSettings
    }, ChannelSaved {
        token: Uuid, channel: Uuid, value: NotificationSettings, result: anyhow::Result<()>
    },
}
#[derive(Default)]
pub(super) struct Account {
    settings:Option<settings::View>,
    close_after_save:bool,
    parked_profile:Option<ProfileView>,profile: Option<ProfileView>, preferences: Option<PreferencesView>, channel: Option<(Uuid, gtk::Window, gtk::Box, gtk::Label)>, jobs: Vec<tokio::task::JoinHandle<()>>, mutation: Option<Uuid>, settings_pending: bool, channel_pending: bool, pub config: UserConfig, css: Option<gtk::CssProvider>,
}
struct ProfileView {
    baseline:Vec<String>,reconcile:Option<Uuid>,identity:Option<(adw::Avatar,gtk::Label)>,banner:Option<gtk::Picture>,
    embedded:bool,hero:Option<profile::Hero>,
    text:Vec<ProfileText>,
    loading:bool,refresh_again:bool,refresh_scheduled:bool,
    token: Uuid, id: Uuid, window: gtk::Window, body: gtk::Box, error: gtk::Label, save:Option<gtk::Button>, fields: Option<ProfileFields>, pending: bool, image_write: bool
}
struct ProfileText {container:gtk::Box,value:String,preview:bool}
struct ProfileFields {
    nickname: adw::EntryRow, status: adw::EntryRow, description: gtk::TextBuffer, typing: adw::EntryRow, manual: gtk::DropDown
}
struct PreferencesView {
    baseline:Vec<String>,save:gtk::Button,
    embedded:bool,
    token: Uuid, window: gtk::Window, body: gtk::Box, error: gtk::Label, motion:adw::ComboRow, theme: adw::ComboRow, font: adw::ComboRow, density: adw::ComboRow, toggles: Vec<adw::SwitchRow>, pending: bool
}
impl Drop for Account {
    fn drop(&mut self) {
        for j in self.jobs.drain(..) {
            j.abort();
        }
        if let Some(v)=self.settings.take(){v.force_close();}
        if let Some(v)=self.parked_profile.take(){v.window.close();}
        if let Some(v) = self.profile.take() {
            v.window.close();
        }
        if let Some(v) = self.preferences.take() {
            v.window.close();
        }
        if let Some((_, w, _, _)) = self.channel.take() {
            w.close();
        }
        if let (Some(display), Some(css)) = (gtk::gdk::Display::default(), self.css.take()) {
            gtk::style_context_remove_provider_for_display(&display, &css);
        }
    }
}
fn dropdown(body: &gtk::Box, label: &str, values: &[&str], selected: u32)->gtk::DropDown {
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    body.append(&l);
    let e = gtk::DropDown::from_strings(values);
    e.set_selected(selected);
    body.append(&e);
    e
}
impl MainWindowModel {
    fn refresh_profile_text(&mut self,sender:&ComponentSender<Self>){
        let Some(profile)=self.account.profile.as_ref().filter(|v|v.window.is_visible())else{return;};
        let chat=self.chat.model();
        for text in &profile.text {
            while let Some(child)=text.container.first_child(){text.container.remove(&child);}
            if text.preview {
                let visible=chat.has_profile_emoji(&text.value);
                if let Some(parent)=text.container.parent(){parent.set_visible(visible);}
                if !visible{continue;}
            }
            let view=chat.profile_text(&text.value);view.add_css_class("papo-profile-text");
            if !text.preview{
                view.set_justification(gtk::Justification::Center);
                let fitted=std::rc::Rc::new(std::cell::Cell::new(false));let input=sender.input_sender().clone();let token=profile.token;
                view.connect_notify_local(Some("height-request"),move |view,_|{if view.is_mapped()&&!fitted.replace(true){let _=input.send(MainWindowMsg::Account(AccountMsg::FitProfile(token)));}});
            }
            text.container.append(&view);
        }
        if profile.fields.is_none(){fit_profile(profile);}
    }
    pub(super) fn profile_action_status(&self,text:&str){if let Some(v)=&self.account.profile{if v.window.is_visible(){v.error.set_text(text);}}}
    pub(super) fn close_profile_for_direct(&mut self,peer:Uuid){if self.account.profile.as_ref().is_some_and(|v|v.id==peer){if let Some(v)=self.account.profile.take(){v.window.close();}self.account.profile=self.account.parked_profile.take();}}

    pub(super) fn apply_config(&mut self, root: &gtk::Box) {
        use adw::prelude::*;
        self.account.config = self.account.config.complete();
        let config = &self.account.config;
        adw::StyleManager::default().set_color_scheme(match config.theme {
            Some(Theme::Dark) => adw::ColorScheme::ForceDark, Some(Theme::Light) => adw::ColorScheme::ForceLight, _ => adw::ColorScheme::Default
        });
        self.chat.emit(ChatMsg::SetConfig(config.clone()));
        self.sidebar.emit(SidebarMsg::SetConfig(config.clone()));
        self.user_list.emit(UserListMsg::SetConfig(config.clone()));
        let font = match config.display.as_ref().and_then(|d|d.font_size.as_ref()) {
            Some(FontSize::Small) => "0.9em", Some(FontSize::Huge) => "1.3em", _ => "1em"
        };
        root.add_css_class("papo-app");
        let css = self.account.css.get_or_insert_with(gtk::CssProvider::new);
        css.load_from_string(&format!(".papo-message-text {{font-size: {font};}}"));
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }
    pub(super) fn restore_profile_editor(&mut self){
        if let Some(editor)=self.account.parked_profile.take(){if let Some(preview)=self.account.profile.take(){preview.window.close();}self.account.profile=Some(editor);}
    }
    pub(super) fn account_event(&mut self,msg:AccountMsg,sender:&ComponentSender<Self>,root:&gtk::Box){
        // Profile previews and the mounted editor have independent lifetimes.
        // Route in-flight form results by token while a preview is on top.
        let token=match &msg{
            AccountMsg::ProfileLoaded{token,..}|AccountMsg::ProfilePrepared{token,..}|AccountMsg::ProfileAccepted{token,..}|AccountMsg::ProfileSaved{token,..}|AccountMsg::ProfileRefreshReady{token,..}|AccountMsg::Image{token,..}|AccountMsg::ImageSelected{token,..}|AccountMsg::OwnUpdated{token,..}=>Some(*token),
            AccountMsg::SaveProfile(token)|AccountMsg::DescriptionChanged(token)|AccountMsg::FitProfile(token)=>Some(*token),_=>None};
        let parked=token.is_some_and(|token|self.account.parked_profile.as_ref().is_some_and(|v|v.token==token));
        if parked{std::mem::swap(&mut self.account.profile,&mut self.account.parked_profile);}
        self.account_event_for_profile(msg,sender,root);
        if parked{std::mem::swap(&mut self.account.profile,&mut self.account.parked_profile);}
    }
    fn account_event_for_profile(&mut self, msg: AccountMsg, sender: &ComponentSender<Self>, root: &gtk::Box) {
        self.account.jobs.retain(|j|!j.is_finished());
        match msg {
            AccountMsg::Dirty=>self.settings_dirty(),
            AccountMsg::CloseSettings=>self.request_settings_close(sender),
            AccountMsg::CloseChoice(choice)=>self.settings_close_choice(&choice,sender),
            AccountMsg::ProfileAccepted{token,generation,draft,result}=>{
                let Some(v)=self.account.profile.as_mut().filter(|v|v.token==token&&v.reconcile==Some(generation))else{return;};
                v.reconcile=None;
                match result{
                    Ok((p,banner,image))=>{
                        let image=image.map(crate::media::PreparedImage::texture);
                        if let Some(hero)=&v.hero{
                            hero.name.set_text(p.display_name());hero.avatar.set_text(Some(p.display_name()));
                            if v.image_write{hero.avatar.set_custom_image(image.as_ref());let texture=banner.as_deref().and_then(crate::media::bounded_texture);hero.set_banner(texture.as_ref());}
                        }
                        if let Some((avatar,name))=&v.identity{name.set_text(p.display_name());if v.image_write{avatar.set_custom_image(image.as_ref());}}
                        if v.image_write&&v.hero.is_none(){if let Some(old)=v.banner.take(){v.body.remove(&old);}if let Some(texture)=banner.as_deref().and_then(crate::media::bounded_texture){let picture=gtk::Picture::for_paintable(&texture);picture.set_height_request(140);picture.set_can_shrink(true);v.body.prepend(&picture);v.banner=Some(picture);}}
                        if !v.image_write{if let Some(f)=&v.fields{
                            let current=profile_values(f);let manual=match p.status{Some(UserStatus::Away)=>1,Some(UserStatus::Busy)=>2,None=>0};
                            let accepted=vec![p.nickname.unwrap_or_default(),p.status_message.unwrap_or_default(),p.description.unwrap_or_default(),p.typing.unwrap_or_default(),manual.to_string()];
                            // Reconcile each unchanged field independently: editing a
                            // description must not hide an accepted nickname normalization.
                            if current.first()==draft.first(){f.nickname.set_text(&accepted[0]);}
                            if current.get(1)==draft.get(1){f.status.set_text(&accepted[1]);}
                            if current.get(2)==draft.get(2){f.description.set_text(&accepted[2]);}
                            if current.get(3)==draft.get(3){f.typing.set_text(&accepted[3]);}
                            if current.get(4)==draft.get(4){f.manual.set_selected(manual);}
                            v.baseline=accepted;
                        }}
                        v.error.set_text("Salvo.");
                    },Err(e)=>{v.error.set_text(&format!("Salvo; não foi possível atualizar o perfil: {e}"));self.account.close_after_save=false;}
                }
                self.settings_dirty();self.finish_settings_close();
            },
            AccountMsg::Preferences=>{self.open_settings_at(settings::Tab::Preferences,root,sender);},
            AccountMsg::Settings=>{self.restore_profile_editor();self.open_settings(root,sender);},
            AccountMsg::PreviewClosed(token)=>{if self.account.profile.as_ref().is_some_and(|v|!v.embedded&&v.token==token){self.account.profile.take();self.account.profile=self.account.parked_profile.take();}},
            AccountMsg::SettingsTab(tab)=>self.select_settings(tab,sender),
            AccountMsg::ChangeStatus(status)=>{
                if self.account.mutation.is_some()||status==self.current_user.status {
                    self.user_list.emit(UserListMsg::StatusPending(self.account.mutation.is_some()));
                    return;
                }
                let token=Uuid::new_v4();self.account.mutation=Some(token);
                self.user_list.emit(UserListMsg::StatusPending(true));
                if let Some(v)=self.account.profile.as_ref().filter(|v|v.fields.is_some()){
                    v.body.set_sensitive(false);if let Some(save)=&v.save{save.set_sensitive(false);}
                }
                if let Some(v)=self.account.profile.as_mut(){v.reconcile=None;}
                let api=self.api_client.clone();let id=self.current_user.id;let s=sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result=api.update_status(id,status.clone()).await;
                    s.input(MainWindowMsg::Account(AccountMsg::StatusSaved{token,status,result}));
                }));
            }
            AccountMsg::StatusSaved{token,status,result}=>{
                if self.account.mutation!=Some(token){return;}
                self.account.mutation=None;
                self.user_list.emit(UserListMsg::StatusPending(false));
                if let Some(v)=self.account.profile.as_ref().filter(|v|v.fields.is_some()){
                    v.body.set_sensitive(true);if let Some(save)=&v.save{save.set_sensitive(true);}
                }
                match result {
                    Ok(())=>{
                        // A successful status-only write needs no profile or channel reload.
                        self.current_user.status=status.clone();
                        self.members.invalidate(self.current_user.id);
                        if let Some(user)=self.users.iter_mut().find(|u|u.id==self.current_user.id){user.status=status.clone();}
                        if let Some(v)=self.account.profile.as_mut(){if let Some(fields)=&v.fields{
                            fields.manual.set_selected(match status{Some(UserStatus::Away)=>1,Some(UserStatus::Busy)=>2,None=>0});
                            if v.baseline.len()==5{v.baseline[4]=fields.manual.selected().to_string();}
                        }}
                        self.publish_users();
                        self.sidebar.emit(SidebarMsg::SetCurrentUser(self.current_user.clone()));
                    }
                    Err(e)=>{
                        self.user_list.emit(UserListMsg::StatusError(format!("Não foi possível alterar o status: {e}")));
                        sender.input(MainWindowMsg::OperationFailed(e));
                    }
                }
            }
            AccountMsg::EmojisChanged=>{self.refresh_profile_text(sender);self.render_search(sender);},
            AccountMsg::DescriptionChanged(token)=>{
                let Some(v)=self.account.profile.as_mut().filter(|v|v.token==token&&v.window.is_visible())else{return;};
                let Some(fields)=&v.fields else{return;};let buffer=&fields.description;let value=buffer.text(&buffer.start_iter(),&buffer.end_iter(),false).to_string();for text in &mut v.text{if text.preview{text.value=value.clone();}}
                self.refresh_profile_text(sender);
            },
            AccountMsg::FitProfile(token)=>{
                if let Some(v)=self.account.profile.as_ref().filter(|v|v.token==token&&v.fields.is_none()&&!v.loading&&v.window.is_visible()){
                    fit_profile(v);
                }
            },
            AccountMsg::RefreshProfile(id) => {
                if id!=self.current_user.id{
                    if let Some(v)=self.account.profile.as_mut().filter(|v|v.id==id&&v.fields.is_none()&&v.window.is_visible()){
                        if v.loading{v.refresh_again=true;}
                        else if !v.refresh_scheduled{
                            v.refresh_scheduled=true;let token=v.token;let s=sender.clone();
                            self.account.jobs.push(tokio::spawn(async move{tokio::time::sleep(Duration::from_millis(500)).await;s.input(MainWindowMsg::Account(AccountMsg::ProfileRefreshReady{token,id}));}));
                        }
                    }
                }
            }
            AccountMsg::ProfileRefreshReady{token,id}=>{
                if let Some(v)=self.account.profile.as_mut().filter(|v|v.token==token&&v.id==id&&v.window.is_visible()){
                    v.refresh_scheduled=false;
                    if v.loading{v.refresh_again=true;}else if v.fields.is_none(){sender.input(MainWindowMsg::Account(AccountMsg::Profile(id)));}
                }
            }
            AccountMsg::Profile(_) | AccountMsg::SettingsProfile => {
                let embedded=matches!(msg,AccountMsg::SettingsProfile);
                let id=if let AccountMsg::Profile(id)=msg{id}else{self.current_user.id};
                if embedded&&self.settings_mount(settings::Tab::Profile).is_none(){return;}
                if let Some(v) = self.account.profile.take() {
                    if v.embedded&&!embedded{self.account.parked_profile=Some(v);}else if v.embedded{self.clear_settings_tab(settings::Tab::Profile,&v.window);}else{v.window.close();}
                }
                let token = Uuid::new_v4();
                let own=embedded;
                let mount=embedded.then(||self.settings_mount(settings::Tab::Profile).unwrap());
                let w:gtk::Window=mount.as_ref().map(|(w,_)|w.clone()).unwrap_or_else(||adw::Window::builder().title(if own{"Editar perfil"}else{"Perfil"}).default_width(if own{620}else{460}).default_height(if own{780}else{-1}).modal(true).build().upcast());
                if !embedded{if let Some(parent)=root.root().and_downcast::<gtk::Window>(){w.set_transient_for(Some(&parent));}}
                let toolbar=adw::ToolbarView::new();let header=adw::HeaderBar::new();let title=adw::WindowTitle::new(if own{"Editar perfil"}else{"Perfil"},"");header.set_title_widget(Some(&title));
                let actions=gtk::Box::new(gtk::Orientation::Horizontal,12);
                if embedded{actions.add_css_class("papo-profile-actions");toolbar.add_bottom_bar(&actions);}else{toolbar.add_top_bar(&header);}
                let save=own.then(||{let save=gtk::Button::with_label("Salvar perfil");save.add_css_class("suggested-action");save.set_valign(gtk::Align::Center);save.set_sensitive(false);let output=sender.input_sender().clone();save.connect_clicked(move |_|{let _=output.send(MainWindowMsg::Account(AccountMsg::SaveProfile(token)));});if embedded{actions.append(&save);}else{header.pack_end(&save);}save});
                let body=gtk::Box::new(gtk::Orientation::Vertical,0);toolbar.set_content(Some(&body));if let Some((_,page))=mount{page.append(&toolbar);}else{w.downcast_ref::<adw::Window>().unwrap().set_content(Some(&toolbar));}
                let scroll = gtk::ScrolledWindow::new();
                scroll.set_vexpand(own);scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
                if !own {
                    // Read-only profiles fit their content instead of reserving editor space.
                    scroll.set_propagate_natural_height(true);
                    scroll.set_min_content_height(140);
                    scroll.set_max_content_height(560);
                }
                let content = gtk::Box::new(gtk::Orientation::Vertical, if own{20}else{16});content.set_valign(gtk::Align::Start);
                content.set_margin_top(if embedded{18}else{24});content.set_margin_bottom(if own{24}else{16});content.set_margin_start(if embedded{32}else{18});content.set_margin_end(if embedded{32}else{18});
                let clamp=adw::Clamp::new();clamp.set_maximum_size(if embedded{768}else{560});clamp.set_tightening_threshold(if embedded{640}else{440});clamp.set_child(Some(&content));scroll.set_child(Some(&clamp));
                body.append(&scroll);
                let error = gtk::Label::new(Some("Carregando perfil…"));
                error.set_wrap(true);
                error.connect_notify_local(Some("label"),|label,_|label.set_visible(!label.text().is_empty()));
                if embedded{error.set_xalign(0.0);let status=gtk::Box::new(gtk::Orientation::Vertical,0);status.set_hexpand(true);status.set_valign(gtk::Align::Center);status.append(&error);actions.prepend(&status);}else{body.append(&error);}
                if !embedded{let input=sender.input_sender().clone();w.connect_close_request(move |_|{let _=input.send(MainWindowMsg::Account(AccountMsg::PreviewClosed(token)));gtk::glib::Propagation::Proceed});}
                w.present();
                self.account.profile = Some(ProfileView {
                    baseline:vec![],reconcile:None,identity:None,banner:None,embedded,hero:None,
                    text:vec![],
                    token, id, window: w, body: content, error, save, fields: None, pending: false, image_write: false,loading:true,refresh_again:false,refresh_scheduled:false
                });
                let api = self.api_client.clone();
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = async {
                        let p = api.get_user_profile(id).await?;
                        anyhow::ensure!(p.id==id, "Perfil retornado para outro usuário");
                        let banner = if let Some(sha) = &p.banner_media {
                            api.media_bytes(&format!("/media/{sha}"), 2<<20).await.ok()
                        }
                        else {
                            None
                        };
                        Ok((p, banner))
                    }.await;
                    s.input(MainWindowMsg::Account(AccountMsg::ProfileLoaded {
                        token, result
                    }));
                }));
            }
            AccountMsg::ProfileLoaded{token,result}=>{
                if !self.account.profile.as_ref().is_some_and(|v|v.token==token&&v.loading&&v.window.is_visible()){return;}
                let s=sender.clone();self.account.jobs.push(tokio::spawn(async move{
                    let result=match result{Ok((profile,banner))=>{let image=crate::media::avatars::prepare_blob(profile.avatar_blob.clone(),192).await;Ok((profile,banner,image))},Err(e)=>Err(e)};
                    s.input(MainWindowMsg::Account(AccountMsg::ProfilePrepared{token,result}));
                }));
            }
            AccountMsg::ProfilePrepared{token,result} => {
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token&&v.loading&&v.window.is_visible())else {
                    return;
                };
                v.loading=false;if std::mem::take(&mut v.refresh_again){sender.input(MainWindowMsg::Account(AccountMsg::RefreshProfile(v.id)));}
                match result {
                    Err(e) => {
                        v.error.set_text(&format!("Perfil indisponível: {e}"));
                        sender.input(MainWindowMsg::ActionError(e));
                    }, Ok((p, banner, image)) => {
                        v.error.set_text("");
                        if let Some(save)=&v.save{save.set_sensitive(self.account.mutation.is_none());v.body.set_sensitive(self.account.mutation.is_none());}
                        let avatar=image.map(crate::media::PreparedImage::texture);
                        if v.embedded{
                            let banner=banner.as_deref().and_then(crate::media::bounded_texture);
                            let hero=profile::Hero::new(&p,avatar.as_ref(),banner.as_ref());
                            v.body.append(&hero.widget);v.hero=Some(hero);
                        }else{
                            if let Some(t) = banner.as_deref().and_then(crate::media::bounded_texture) {
                                let picture = gtk::Picture::for_paintable(&t);
                                picture.set_height_request(140);
                                picture.set_can_shrink(true);
                                v.body.append(&picture);v.banner=Some(picture);
                            }
                            let identity=gtk::Box::new(gtk::Orientation::Horizontal,16);let picture=crate::media::avatar_image(avatar.as_ref(),80);picture.set_text(Some(p.display_name()));identity.append(&picture);
                            let names=gtk::Box::new(gtk::Orientation::Vertical,4);names.set_valign(gtk::Align::Center);let name=gtk::Label::new(Some(p.display_name()));name.add_css_class("title-2");name.set_xalign(0.0);name.set_ellipsize(gtk::pango::EllipsizeMode::End);if let Some(roles)=p.roles.as_deref(){crate::ui::style::role_color(&name,roles);}names.append(&name);let username=gtk::Label::new(Some(&format!("@{}",p.username)));username.add_css_class("dim-label");username.set_xalign(0.0);names.append(&username);identity.append(&names);v.body.append(&identity);v.identity=Some((picture,name));
                            let badges=gtk::FlowBox::new();badges.add_css_class("papo-profile-roles");badges.set_selection_mode(gtk::SelectionMode::None);badges.set_min_children_per_line(1);badges.set_max_children_per_line(8);badges.set_column_spacing(4);badges.set_row_spacing(4);badges.set_halign(gtk::Align::Start);
                            for role in p.roles.iter().flatten() {
                                let badge = gtk::Label::new(Some(&role.name));
                                badge.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                crate::ui::style::role_color(&badge,std::slice::from_ref(role));badges.insert(&badge,-1);
                            }
                            if badges.first_child().is_some(){v.body.append(&badges);}
                        }
                        if v.embedded {
                            let appearance=adw::PreferencesGroup::new();appearance.set_title("Imagem do perfil");v.body.append(&appearance);
                            for (title,banner) in [("Avatar",false),("Banner",true)]{
                                let row=adw::ActionRow::builder().title(title).subtitle(if banner{"Imagem no topo do seu perfil"}else{"Sua imagem nas mensagens e na lista de membros"}).build();
                                let buttons=gtk::Box::new(gtk::Orientation::Horizontal,0);buttons.add_css_class("linked");buttons.set_valign(gtk::Align::Center);
                                for (label,remove) in [(if banner{"Alterar banner"}else{"Alterar avatar"},false),(if banner{"Remover banner"}else{"Remover avatar"},true)]{
                                    let b=gtk::Button::with_label(if remove{"Remover"}else{"Alterar…"});b.set_tooltip_text(Some(label));b.update_property(&[gtk::accessible::Property::Label(label)]);let output=sender.input_sender().clone();b.connect_clicked(move |_|{let _=output.send(MainWindowMsg::Account(AccountMsg::Image{token,banner,remove}));});buttons.append(&b);
                                }row.add_suffix(&buttons);appearance.add(&row);
                            }
                            let identity=adw::PreferencesGroup::new();identity.set_title("Sobre você");v.body.append(&identity);
                            let nickname=profile_entry(&identity,"Apelido",p.nickname.as_deref().unwrap_or(""),"Até 32 caracteres");
                            let status=profile_entry(&identity,"Mensagem de status",p.status_message.as_deref().unwrap_or(""),"Até 64 caracteres");
                            let about=adw::PreferencesGroup::new();about.set_title("Descrição");about.set_description(Some("Conte um pouco sobre você · até 512 caracteres"));v.body.append(&about);
                            let text=gtk::TextView::new();text.set_wrap_mode(gtk::WrapMode::WordChar);text.set_top_margin(12);text.set_bottom_margin(12);text.set_left_margin(12);text.set_right_margin(12);
                            let description=text.buffer();description.set_text(p.description.as_deref().unwrap_or(""));
                            let editor=gtk::ScrolledWindow::builder().min_content_height(110).hscrollbar_policy(gtk::PolicyType::Never).child(&text).build();let frame=gtk::Frame::new(None);frame.add_css_class("papo-profile-description");frame.set_overflow(gtk::Overflow::Hidden);frame.set_child(Some(&editor));about.add(&frame);
                            let preview=gtk::Box::new(gtk::Orientation::Vertical,6);preview.set_margin_top(8);preview.set_margin_bottom(8);preview.set_margin_start(12);preview.set_margin_end(12);
                            let caption=gtk::Label::new(Some("Prévia"));caption.add_css_class("caption");caption.add_css_class("dim-label");caption.set_xalign(0.0);preview.append(&caption);
                            let content=gtk::Box::new(gtk::Orientation::Vertical,0);preview.append(&content);about.add(&preview);
                            v.text.push(ProfileText{container:content,value:p.description.as_deref().unwrap_or("").into(),preview:true});
                            let input=sender.input_sender().clone();description.connect_changed(move |_|{let _=input.send(MainWindowMsg::Account(AccountMsg::DescriptionChanged(token)));});
                            let presence=adw::PreferencesGroup::new();presence.set_title("Presença");v.body.append(&presence);
                            let typing=profile_entry(&presence,"Texto ao digitar",p.typing.as_deref().unwrap_or(""),"Até 64 caracteres");
                            // A status-only write can finish while this profile read is in flight.
                            let manual=gtk::DropDown::from_strings(&["Automática","Ausente","Ocupado"]);manual.set_selected(match self.current_user.status{Some(UserStatus::Away)=>1,Some(UserStatus::Busy)=>2,_=>0});manual.set_valign(gtk::Align::Center);
                            let row=adw::ActionRow::builder().title("Disponibilidade").subtitle("A presença automática acompanha sua atividade").build();row.add_suffix(&manual);row.set_activatable_widget(Some(&manual));presence.add(&row);
                            v.fields = Some(ProfileFields {
                                nickname, status, description, typing, manual
                            });
                            v.baseline=profile_values(v.fields.as_ref().unwrap());
                            watch_profile(v.fields.as_ref().unwrap(),sender);
                            let buffer=&v.fields.as_ref().unwrap().description;v.text[0].value=buffer.text(&buffer.start_iter(),&buffer.end_iter(),false).into();

                        }
                        else {
                            for (label,action) in [("Mensagem direta",0),("Bloquear usuário",1)].into_iter().filter(|_|p.id!=self.current_user.id) {
                                let b=gtk::Button::with_label(label);let s=sender.clone();let id=p.id;
                                b.connect_clicked(move |_|s.input(MainWindowMsg::Direct(if action==0{super::direct::DirectMsg::Open(id)}else{super::direct::DirectMsg::Block(id,true)})));v.body.append(&b);
                            }
                            for text in [p.description, p.status_message].into_iter().flatten().filter(|text|!text.trim().is_empty()) {
                                let container=gtk::Box::new(gtk::Orientation::Vertical,0);v.body.append(&container);
                                v.text.push(ProfileText{container,value:text,preview:false});
                            }
                            // GTK retains the loading window's allocation unless a new size is requested.
                            fit_profile(v);
                        }
                    }
                }
                self.refresh_profile_text(sender);self.publish_avatars();self.settings_dirty();
            }
            AccountMsg::SaveProfile(token) => {
                if self.account.mutation.is_some() {
                    return;
                }
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                let Some(f) = &v.fields else {
                    return;
                };
                let request = UpdateUserRequest {
                    nickname: f.nickname.text().into(), status: f.status.text().into(), description: f.description.text(&f.description.start_iter(), &f.description.end_iter(), false).into(), typing: Some(f.typing.text().into())
                };
                let status = match f.manual.selected() {
                    1 => Some(UserStatus::Away), 2 => Some(UserStatus::Busy), _ => None
                };
                v.reconcile=None;v.pending = true;
                v.image_write = false;
                self.account.mutation = Some(token);
                self.user_list.emit(UserListMsg::StatusPending(true));
                v.body.set_sensitive(false);if let Some(save)=&v.save{save.set_sensitive(false);}
                v.error.set_text("Salvando…");
                let id = v.id;
                let api = self.api_client.clone();
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = async {
                        api.update_profile(id, &request).await?;
                        api.update_status(id, status).await
                    }.await;
                    s.input(MainWindowMsg::Account(AccountMsg::OwnUpdated {
                        token, result: api.whoami().await
                    }));
                    s.input(MainWindowMsg::Account(AccountMsg::ProfileSaved {
                        token, result
                    }));
                }));
            }
            AccountMsg::Image {
                token, banner, remove
            } => {
                let Some(v) = self.account.profile.as_ref().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                if remove {
                    sender.input(MainWindowMsg::Account(AccountMsg::ImageSelected {
                        token, banner, result: Ok(Vec::new())
                    }));
                }
                else {
                    let parent = v.window.clone();
                    let s = sender.clone();
                    gtk::glib::spawn_future_local(async move {
                        if let Ok(file) = gtk::FileDialog::builder().title("Escolher imagem (até 2 MiB)").build().open_future(Some(&parent)).await {
                            if let Some(path) = file.path() {
                                tokio::spawn(async move {
                                    let result = async {
                                        anyhow::ensure!(tokio::fs::metadata(&path).await?.len()<=2<<20, "Use uma imagem de até 2 MiB");
                                        let bytes = tokio::fs::read(path).await?;
                                        crate::api::features::validate_profile_image(&bytes, banner)?;
                                        Ok(bytes)
                                    }.await;
                                    s.input(MainWindowMsg::Account(AccountMsg::ImageSelected {
                                        token, banner, result
                                    }));
                                });
                            }
                        }
                    });
                }
            }
            AccountMsg::ImageSelected {
                token, banner, result
            } => {
                if self.account.mutation.is_some() {
                    return;
                }
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                match result {
                    Err(e) => v.error.set_text(&e.to_string()), Ok(bytes) => {
                        v.reconcile=None;v.pending = true;
                        v.image_write = true;
                        self.account.mutation = Some(token);
                        self.user_list.emit(UserListMsg::StatusPending(true));
                        v.body.set_sensitive(false);if let Some(save)=&v.save{save.set_sensitive(false);}
                        v.error.set_text("Salvando imagem…");
                        let id = v.id;
                        let api = self.api_client.clone();
                        let s = sender.clone();
                        self.account.jobs.push(tokio::spawn(async move {
                            let result = api.update_image(id, banner, &bytes).await;
                            s.input(MainWindowMsg::Account(AccountMsg::OwnUpdated {
                                token, result: api.whoami().await
                            }));
                            s.input(MainWindowMsg::Account(AccountMsg::ProfileSaved {
                                token, result
                            }));
                        }));
                    }
                }
            }
            AccountMsg::ProfileSaved {
                token, result
            } => {
                if self.account.mutation==Some(token) {
                    self.account.mutation = None;
                    self.user_list.emit(UserListMsg::StatusPending(false));
                }
                let Some(v) = self.account.profile.as_mut().filter(|v|v.token==token)else {
                    return;
                };
                v.pending = false;
                v.body.set_sensitive(true);if let Some(save)=&v.save{save.set_sensitive(true);}
                match result {
                    Err(e) => {
                        v.error.set_text(&format!("Falha ao salvar: {e}"));self.account.close_after_save=false;
                        sender.input(MainWindowMsg::ActionError(e));
                    }, Ok(()) => {
                        v.error.set_text("Salvo.");
                        let id=v.id;let generation=Uuid::new_v4();v.reconcile=Some(generation);
                        let draft=v.fields.as_ref().map(profile_values).unwrap_or_default();
                        if !v.image_write{v.baseline=draft.clone();}
                        let images=v.image_write;let api=self.api_client.clone();let output=sender.clone();
                        self.account.jobs.push(tokio::spawn(async move{
                            let result=async{let p=api.get_user_profile(id).await?;anyhow::ensure!(p.id==id,"Perfil retornado para outro usuário");
                                let banner=if images{if let Some(sha)=&p.banner_media{Some(api.media_bytes(&format!("/media/{sha}"),2<<20).await?)}else{None}}else{None};
                                let image=if images{crate::media::avatars::prepare_blob(p.avatar_blob.clone(),192).await}else{None};Ok((p,banner,image))}.await;
                            output.input(MainWindowMsg::Account(AccountMsg::ProfileAccepted{token,generation,draft,result}));
                        }));
                        self.settings_dirty();
                    }
                }
            }
            AccountMsg::OwnUpdated {
                token, result
            } => {
                if self.account.mutation!=Some(token) {
                    return;
                }
                match result {
                    Ok(mut user) => {
                        // Settings writes have their own accepted snapshot; an overlapping profile read cannot undo them.
                        user.settings = Some(WhoamiSettings {
                            version: 1, config: self.account.config.clone()
                        });
                        if self.current_user.avatar_blob!=user.avatar_blob{self.load_avatar_blob(user.id,user.avatar_blob.clone(),sender.clone());}
                        self.members.invalidate(user.id);
                        if let Some(summary)=self.users.iter_mut().find(|u|u.id==user.id){summary.nickname=user.nickname.clone();summary.status=user.status.clone();summary.status_message=user.status_message.clone();summary.typing=user.typing.clone();summary.status_updated_at=user.status_updated_at;}
                        self.current_user = user;
                        self.publish_users();
                        self.publish_avatars();
                        self.sidebar.emit(SidebarMsg::SetCurrentUser(self.current_user.clone()));
                        self.user_list.emit(UserListMsg::SetStatus(self.current_user.status.clone()));
                    }, Err(e) => sender.input(MainWindowMsg::OperationFailed(e))
                }
            }, AccountMsg::SettingsPreferences => {
                let embedded=matches!(msg,AccountMsg::SettingsPreferences);
                if embedded&&self.settings_mount(settings::Tab::Preferences).is_none(){return;}
                if let Some(v) = self.account.preferences.take() {
                    if v.embedded{self.clear_settings_tab(settings::Tab::Preferences,&v.window);}else{v.window.close();}
                }
                let token = Uuid::new_v4();
                let (w, body) = if embedded{self.settings_form(settings::Tab::Preferences).unwrap()}else{window(root, "Preferências")};
                let c = self.account.config.complete();
                let d = c.display.as_ref().unwrap();
                let n = c.notifications.as_ref().unwrap();
                body.add_css_class("papo-preferences");body.set_spacing(16);body.set_margin_top(20);body.set_margin_bottom(20);
                let appearance=preferences::group(&body,"Aparência");
                let theme = preferences::choice(&appearance, "Tema", &["Sistema", "Claro", "Escuro"], match c.theme {
                    Some(Theme::Light) => 1, Some(Theme::Dark) => 2, _ => 0
                });
                let font = preferences::choice(&appearance, "Tamanho do texto", &["Pequeno", "Médio", "Grande"], match d.font_size {
                    Some(FontSize::Small) => 0, Some(FontSize::Huge) => 2, _ => 1
                });
                let density = preferences::choice(&appearance, "Espaçamento", &["Compacto", "Normal", "Confortável"], match d.message_density {
                    Some(MessageDensity::Compact) => 0, Some(MessageDensity::Comfortable) => 2, _ => 1
                });
                let motion=preferences::choice(&appearance,"Animações de GIFs e emojis (neste dispositivo)",&["Seguir o sistema","Sempre animar","Reduzir movimento"],crate::media::animation::motion().index());
                let display=preferences::group(&body,"Exibição");
                let notifications=preferences::group(&body,"Notificações");
                let mut toggles = Vec::new();
                for (index,(label, value)) in [("Mostrar horários", d.show_timestamps), ("Mostrar avatares", d.show_avatars), ("Ativar notificações", n.enabled), ("Prévia nas notificações", n.message_preview), ("Som das notificações", n.sound), ("Notificar menções", n.mentions)].into_iter().enumerate() {
                    let row=adw::SwitchRow::new();row.set_title(label);row.set_title_lines(0);row.set_active(value.unwrap_or(true));
                    if index<2{display.add(&row);}else{notifications.add(&row);}
                    toggles.push(row);
                }
                let save = gtk::Button::with_label("Salvar preferências");
                save.add_css_class("suggested-action");save.set_halign(gtk::Align::End);save.set_margin_top(4);
                let s = sender.clone();
                save.connect_clicked(move |_|s.input(MainWindowMsg::Account(AccountMsg::SavePreferences(token))));
                body.append(&save);
                let error = gtk::Label::new(None);
                error.set_wrap(true);error.set_xalign(0.0);error.set_visible(false);error.connect_label_notify(|label|label.set_visible(!label.text().is_empty()));
                body.append(&error);
                w.present();
                self.account.preferences = Some(PreferencesView {
                    baseline:vec![],save:save.clone(),embedded,
                    token, window: w, body, error, motion, theme, font, density, toggles, pending: false
                });
                let v=self.account.preferences.as_mut().unwrap();v.baseline=preferences_values(v);watch_preferences(v,sender);self.settings_dirty();
            }
            AccountMsg::SavePreferences(token) => {
                if self.account.settings_pending {
                    return;
                }
                let Some(v) = self.account.preferences.as_mut().filter(|v|v.token==token&&!v.pending&&v.window.is_visible())else {
                    return;
                };
                let mut config = self.account.config.complete();
                config.theme = Some(match v.theme.selected() {
                    1 => Theme::Light, 2 => Theme::Dark, _ => Theme::System
                });
                let d = config.display.as_mut().unwrap();
                d.font_size = Some(match v.font.selected() {
                    0 => FontSize::Small, 2 => FontSize::Huge, _ => FontSize::Medium
                });
                d.message_density = Some(match v.density.selected() {
                    0 => MessageDensity::Compact, 2 => MessageDensity::Comfortable, _ => MessageDensity::Normal
                });
                d.show_timestamps = Some(v.toggles[0].is_active());
                d.show_avatars = Some(v.toggles[1].is_active());
                let n = config.notifications.as_mut().unwrap();
                n.enabled = Some(v.toggles[2].is_active());
                n.message_preview = Some(v.toggles[3].is_active());
                n.sound = Some(v.toggles[4].is_active());
                n.mentions = Some(v.toggles[5].is_active());
                v.pending = true;
                self.account.settings_pending = true;
                v.body.set_sensitive(false);
                v.error.set_text("Salvando…");
                let motion=crate::config::Motion::from_index(v.motion.selected());
                let api = self.api_client.clone();
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result=async{
                        tokio::task::spawn_blocking(move ||{let mut local=crate::config::AppConfig::load();local.motion=motion;local.save()}).await??;
                        api.save_settings(&config).await
                    }.await;
                    s.input(MainWindowMsg::Account(AccountMsg::PreferencesSaved {
                        token, motion, result
                    }));
                }));
            }
            AccountMsg::PreferencesSaved {
                token, motion, result
            } => {
                self.account.settings_pending = false;
                // Apply a committed write even when its dialog has been replaced or closed.
                match result {
                    Ok(settings) => {
                        if settings.user_id!=self.current_user.id {
                            return;
                        }
                        crate::media::animation::set_motion(motion);
                        self.account.config = settings.config.complete();
                        self.current_user.settings = Some(WhoamiSettings {
                            version: settings.version, config: self.account.config.clone()
                        });
                        self.apply_config(root);
                        if let Some(v) = self.account.preferences.as_mut().filter(|v|v.token==token) {
                            v.pending = false;
                            v.body.set_sensitive(true);
                            v.error.set_text("Preferências salvas.");v.baseline=preferences_values(v);
                        }
                    }, Err(e) => {
                        if let Some(v) = self.account.preferences.as_mut().filter(|v|v.token==token) {
                            v.pending = false;
                            v.body.set_sensitive(true);
                            v.error.set_text(&format!("Falha ao salvar: {e}"));self.account.close_after_save=false;
                        }
                        sender.input(MainWindowMsg::ActionError(e));
                    }
                }
                self.settings_dirty();self.finish_settings_close();
            }
            AccountMsg::ChannelPreferences => {
                let Some(channel) = self.channels.iter().find(|c|Some(c.id)==self.active_channel_id)else {
                    return;
                };
                if let Some((_, w, _, _)) = self.account.channel.take() {
                    w.close();
                }
                let token = Uuid::new_v4();
                let id = channel.id;
                let (w, body) = window(root, &format!("Notificações em #{}", channel.name));
                let mode = dropdown(&body, "Notificar", &["Nunca", "Somente menções", "Todas as mensagens"], match channel.notification_settings {
                    Some(NotificationSettings::Off) => 0, Some(NotificationSettings::All) => 2, _ => 1
                });
                let save = gtk::Button::with_label("Salvar notificações do canal");
                let s = sender.clone();
                save.connect_clicked(move |_|s.input(MainWindowMsg::Account(AccountMsg::SaveChannel {
                    token, channel: id, value: match mode.selected() {
                        0 => NotificationSettings::Off, 1 => NotificationSettings::OnlyMentions, _ => NotificationSettings::All
                    }
                })));
                body.append(&save);
                let error = gtk::Label::new(None);
                error.set_wrap(true);
                body.append(&error);
                w.present();
                self.account.channel = Some((token, w, body, error));
            }
            AccountMsg::SaveChannel {
                token, channel, value
            } => {
                if self.account.channel_pending {
                    return;
                }
                let Some((_, w, body, error)) = self.account.channel.as_ref().filter(|(t, _, _, _)|*t==token)else {
                    return;
                };
                if !w.is_visible()||!body.is_sensitive() {
                    return;
                }
                self.account.channel_pending = true;
                body.set_sensitive(false);
                error.set_text("Salvando…");
                let api = self.api_client.clone();
                let user = self.current_user.id;
                let s = sender.clone();
                self.account.jobs.push(tokio::spawn(async move {
                    let result = api.channel_notifications(channel, user, value.clone()).await;
                    s.input(MainWindowMsg::Account(AccountMsg::ChannelSaved {
                        token, channel, value, result
                    }));
                }));
            }
            AccountMsg::ChannelSaved {
                token, channel, value, result
            } => {
                self.account.channel_pending = false;
                match result {
                    Ok(()) => {
                        if let Some(c) = self.channels.iter_mut().find(|c|c.id==channel) {
                            self.notifications.journal.preference(channel,value.clone());
                            c.notification_settings = Some(value);
                            if self.active_channel_id==Some(channel) {
                                self.chat.emit(ChatMsg::UpdateChannel(c.clone()));
                            }
                        }
                        self.publish_voice_channels();
                        if let Some((_, _, body, error)) = self.account.channel.as_ref().filter(|(t, _, _, _)|*t==token) {
                            body.set_sensitive(true);
                            error.set_text("Salvo.");
                        }
                    }, Err(e) => {
                        if let Some((_, _, body, error)) = self.account.channel.as_ref().filter(|(t, _, _, _)|*t==token) {
                            body.set_sensitive(true);
                            error.set_text(&e.to_string());
                        }
                        sender.input(MainWindowMsg::ActionError(e));
                    }
                }
            }
        }
        self.settings_dirty();
    }
}
fn fit_profile(view:&ProfileView){
    if let Some(content)=view.window.downcast_ref::<adw::Window>().and_then(|w|w.content()){
        view.window.set_default_height(content.measure(gtk::Orientation::Vertical,view.window.width()).1);
    }
}
fn profile_entry(group:&adw::PreferencesGroup,title:&str,value:&str,help:&str)->adw::EntryRow{
    let row=adw::EntryRow::new();row.set_title(title);row.set_text(value);row.set_tooltip_text(Some(help));group.add(&row);row
}

#[cfg(test)]
pub(crate) fn exercise_presence_selector(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,descendants,find_button,pump,until};
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    backend.lock().unwrap().profile["status"]=serde_json::json!("busy");
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1280).default_height(740).content(main.widget()).build();host.present();
    until(context,||main.model().background_started&&main.model().access_request.is_none());
    let selector=descendants(main.model().user_list.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::MenuButton>().ok()).unwrap();
    let caption=||descendants(&selector.child().unwrap()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok()).unwrap().text().to_string();
    until(context,||selector.is_mapped()&&caption()=="Ocupado");
    let popover=selector.popover().unwrap();let choices=popover.child().unwrap();
    for label in ["Online","Ocupado","Ausente"] {
        let option=find_button(&choices,label);
        let icon=descendants(option.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Image>().ok()).unwrap();
        assert!(icon.paintable().is_some(),"{label} must have a colored resource icon");
    }
    let heading=descendants(main.model().user_list.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|l|l.text().starts_with("Membros ·"))).unwrap();
    assert!(selector.compute_bounds(&host).unwrap().y()<heading.compute_bounds(&host).unwrap().y(),"selector belongs above the member list");
    selector.popup();pump(context);crate::ui::main_window::layout::tests::preview(&host,"presence-selector",context);
    let count=||backend.lock().unwrap().requests.iter().filter(|(m,p,_)|m=="PUT"&&p.ends_with("/status")).count();
    find_button(&choices,"Ocupado").emit_clicked();pump(context);assert_eq!(count(),0,"selecting the saved status is a no-op");
    let id=main.model().current_user.id;
    main.emit(MainWindowMsg::Account(AccountMsg::Profile(id)));
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.fields.is_some()));
    main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.set_text("Unsaved draft");
    backend.lock().unwrap().fail_status=true;
    let away=find_button(&choices,"Ausente");away.emit_clicked();away.emit_clicked();
    until(context,||count()==1&&main.model().account.mutation.is_none()&&selector.is_sensitive());
    assert_eq!(caption(),"Ocupado","failed writes retain the accepted status");
    assert_eq!(main.model().current_user.status,Some(UserStatus::Busy));
    assert!(descendants(main.model().user_list.widget().upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text().contains("Status failed"))));
    for (label,status,value,expected) in [("Ausente",Some(UserStatus::Away),serde_json::json!("away"),2),("Online",None,serde_json::Value::Null,3)] {
        find_button(&choices,label).emit_clicked();
        until(context,||count()==expected&&main.model().account.mutation.is_none()&&caption()==label&&selector.is_sensitive());
        assert_eq!(main.model().current_user.status,status);
        assert_eq!(backend.lock().unwrap().profile["status"],value);
        let m=main.model();let fields=m.account.profile.as_ref().unwrap().fields.as_ref().unwrap();
        assert_eq!(fields.nickname.text(),"Unsaved draft","status-only writes retain profile drafts");
        assert_eq!(fields.manual.selected(),if status.is_some(){1}else{0});
    }
    {
        let b=backend.lock().unwrap();let writes:Vec<_>=b.requests.iter().filter(|(m,_,_)|m=="PUT").collect();
        assert_eq!(writes.len(),3,"changing presence must only write the status endpoint");
        assert!(writes.iter().all(|(_,p,_)|p==&format!("/users/{id}/status")));
    }
    // Saving presence through the existing editor must update the quick selector too.
    backend.lock().unwrap().fail_profile=false;
    main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().manual.set_selected(2);
    let token=main.model().account.profile.as_ref().unwrap().token;
    main.emit(MainWindowMsg::Account(AccountMsg::SaveProfile(token)));
    until(context,||caption()=="Ocupado"&&selector.is_sensitive()&&main.model().account.profile.as_ref().is_some_and(|v|v.token==token&&v.fields.is_some()&&!v.pending&&v.reconcile.is_none()&&v.error.text()=="Salvo."));
    assert_eq!(count(),4);
    host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}

#[cfg(test)]
pub(crate) fn exercise_profile_emojis(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::{ActionMsg,tests::{mock,pump,until,find_button}};
    use base64::Engine;
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let peer:Uuid="12345678-1234-4234-8234-123456789ac0".parse().unwrap();
    let description="Olá 😄 :ricardao: :unknown:";
    backend.lock().unwrap().peer_profile=Some(serde_json::json!({"id":peer,"username":"bob","description":description,"status_message":"","typing":"","created_at":"2026-10-03T00:00:00Z"}));
    let current_user=serde_json::from_value(backend.lock().unwrap().profile.clone()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    until(context,||!main.model().channels.is_empty()&&main.model().access_request.is_none());
    main.emit(MainWindowMsg::Account(AccountMsg::Profile(peer)));until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.loading&&!v.text.is_empty()));
    let token=main.model().account.profile.as_ref().unwrap().token;
    let view=||main.model().account.profile.as_ref().unwrap().text[0].container.first_child().and_downcast::<gtk::TextView>();
    let images=|view:&gtk::TextView|{let b=view.buffer();(0..b.char_count()).filter(|i|b.iter_at_offset(*i).paintable().is_some()).count()};
    assert_eq!(images(&view().unwrap()),0);let buffer=view().unwrap().buffer();assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false),description);
    let profile_requests=||backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&format!("/users/{peer}/profile")).count();let requests=profile_requests();
    let mut png=std::io::Cursor::new(Vec::new());let bitmap=image::ImageBuffer::from_fn(32,32,|x,y|image::Rgba(if (x==9||x==22)&&(10..14).contains(&y){[20,20,20,255]}else{[255,200,30,255]}));image::DynamicImage::ImageRgba8(bitmap).write_to(&mut png,image::ImageFormat::Png).unwrap();
    backend.lock().unwrap().emojis=vec![serde_json::json!({"id":peer,"name":"ricardao","format":"PNG","image_blob":base64::engine::general_purpose::STANDARD.encode(png.into_inner()),"created_at":"2026-10-03T00:00:00Z"})];
    main.model().chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));until(context,||view().is_some_and(|v|images(&v)==1));
    assert_eq!(profile_requests(),requests,"emoji availability must not refetch the profile");assert_eq!(main.model().account.profile.as_ref().unwrap().token,token);
    let buffer=view().unwrap().buffer();let plain=buffer.text(&buffer.start_iter(),&buffer.end_iter(),false);assert!(plain.contains("😄")&&plain.contains(":unknown:")&&!plain.contains(":ricardao:"));
    let w=main.model().account.profile.as_ref().unwrap().window.clone();crate::ui::main_window::layout::tests::preview(&w,"profile-emojis",context);
    main.emit(MainWindowMsg::Account(AccountMsg::Settings));until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.fields.is_some()));
    let raw="Editar :ricardao: 😄";let buffer=main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().description.clone();buffer.set_text(raw);
    until(context,||view().is_some_and(|v|images(&v)==1));assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false),raw);
    let w=main.model().account.profile.as_ref().unwrap().window.clone();crate::ui::main_window::layout::tests::preview(&w,"profile-emoji-editor",context);
    find_button(w.upcast_ref(),"Salvar perfil").emit_clicked();until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Profile failed")));
    let saved_token=main.model().account.profile.as_ref().unwrap().token;
    find_button(w.upcast_ref(),"Salvar perfil").emit_clicked();until(context,||main.model().account.profile.as_ref().is_some_and(|v|v.token==saved_token&&!v.loading&&v.fields.is_some()&&!v.pending&&v.reconcile.is_none()&&v.error.text()=="Salvo."));
    assert_eq!(backend.lock().unwrap().profile["description"],raw);
    backend.lock().unwrap().emojis.clear();main.model().chat.emit(ChatMsg::Action(ActionMsg::ReloadEmojis));
    until(context,||main.model().account.profile.as_ref().unwrap().text[0].container.parent().is_some_and(|p|!p.is_visible()));
    let buffer=main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().description.clone();
    assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false),raw);
    drop(main);server.abort();pump(context);
}

#[cfg(test)]
pub(crate) fn exercise_preferences_and_profiles(main: &Controller<MainWindowModel>, context: &gtk::glib::MainContext) {
    use crate::ui::chat::actions::tests:: {
        find_button, pump, until, descendants
    };
    find_button(main.widget().upcast_ref(), "Configurações da conta").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|v.fields.is_some()));
    let profile=main.model().account.profile.as_ref().unwrap().window.clone();assert!(profile.default_height()>=720);crate::ui::main_window::layout::tests::preview(&profile,"profile",context);
    let image_button=find_button(profile.upcast_ref(),"Alterar avatar");until(context,||image_button.height()>0);
    let bounds=image_button.compute_bounds(&profile).unwrap();assert!(bounds.y()+bounds.height()<profile.height() as f32,"profile image controls should be visible when the window opens: button={bounds:?}, window={}x{}",profile.width(),profile.height());
    assert!(descendants(profile.upcast_ref()).iter().any(|w|w.is::<adw::HeaderBar>()));assert!(descendants(profile.upcast_ref()).iter().any(|w|w.is::<adw::EntryRow>()));
    let save=find_button(profile.upcast_ref(),"Salvar perfil");let save_bounds=save.compute_bounds(&profile).unwrap();assert!(save_bounds.y()>profile.height() as f32-90.0&&save_bounds.y()+save_bounds.height()<=profile.height() as f32,"save stays visible in the fixed bottom bar");
    profile.set_default_size(390,650);until(context,||profile.width()<=420);crate::ui::main_window::layout::tests::preview(&profile,"profile-narrow",context);
    assert!(profile.width()<=420,"the profile editor must fit a narrow window");let bounds=save.compute_bounds(&profile).unwrap();assert!(bounds.y()>profile.height() as f32-90.0&&bounds.y()+bounds.height()<=profile.height() as f32);
    let old = main.model().account.profile.as_ref().unwrap().token;
    {
        let m = main.model();
        let f = m.account.profile.as_ref().unwrap().fields.as_ref().unwrap();
        f.nickname.set_text("Updated name");
        f.status.set_text("Custom message");
        f.description.set_text("Updated description");
        f.typing.set_text("Custom typing");
        f.manual.set_selected(2);
    }
    let w = main.model().account.profile.as_ref().unwrap().window.clone();
    find_button(w.upcast_ref(), "Salvar perfil").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Profile failed")));
    assert_eq!(main.model().account.profile.as_ref().unwrap().fields.as_ref().unwrap().nickname.text(), "Updated name");
    find_button(w.upcast_ref(), "Salvar perfil").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|v.token==old&&v.fields.is_some()&&!v.pending&&v.reconcile.is_none()&&v.error.text()=="Salvo."));
    assert_eq!(main.model().current_user.nickname.as_deref(), Some("Updated name"));
    assert_eq!(main.model().current_user.status, Some(UserStatus::Busy));
    let id = main.model().current_user.id;
    main.emit(MainWindowMsg::Account(AccountMsg::ProfileLoaded {
        token: old, result: Err(anyhow::anyhow!("Stale profile"))
    }));
    pump(context);
    assert!(!main.model().account.profile.as_ref().unwrap().error.text().contains("Stale"));
    let token = main.model().account.profile.as_ref().unwrap().token;
    let window = main.model().account.profile.as_ref().unwrap().window.clone();
    find_button(window.upcast_ref(), "Remover avatar").emit_clicked();
    until(context, ||main.model().account.profile.as_ref().is_some_and(|v|v.token==token&&v.fields.is_some()&&!v.pending&&v.reconcile.is_none()&&v.error.text()=="Salvo."));
    main.model().account.profile.as_ref().unwrap().window.close();
    main.emit(MainWindowMsg::Account(AccountMsg::Profile("12345678-1234-4234-8234-123456789ac0".parse().unwrap())));
    until(context,||main.model().account.profile.as_ref().is_some_and(|v|!v.loading));
    let other=main.model().account.profile.as_ref().unwrap().window.clone();
    crate::ui::main_window::layout::tests::preview(&other,"other-profile",context);
    other.close();
    main.emit(MainWindowMsg::Account(AccountMsg::Preferences));
    pump(context);
    {
        let m = main.model();
        let v = m.account.preferences.as_ref().unwrap();
        v.theme.set_selected(2);
        v.font.set_selected(2);
        v.density.set_selected(0);
        for c in &v.toggles {
            c.set_active(false);
        }
    }
    let w = main.model().account.preferences.as_ref().unwrap().window.clone();
    find_button(w.upcast_ref(), "Salvar preferências").emit_clicked();
    until(context, ||main.model().account.preferences.as_ref().is_some_and(|v|!v.pending&&v.error.text().contains("Settings failed")));
    assert_eq!(main.model().account.config.theme, Some(Theme::System));
    assert_eq!(main.model().account.preferences.as_ref().unwrap().theme.selected(), 2);
    find_button(w.upcast_ref(), "Salvar preferências").emit_clicked();
    until(context, ||main.model().account.config.theme==Some(Theme::Dark));
    assert!(!main.model().account.config.display.as_ref().unwrap().show_avatars.unwrap());
    assert!(!main.model().account.config.notifications.as_ref().unwrap().sound.unwrap());
    pump(context);
    assert!(descendants(main.widget().upcast_ref()).iter().filter_map(|w|w.downcast_ref::<adw::Avatar>()).all(|a|!a.is_visible()));
    w.close();
    find_button(main.widget().upcast_ref(), "Preferências do canal").emit_clicked();
    pump(context);
    let w = main.model().account.channel.as_ref().unwrap().1.clone();
    let mode = descendants(w.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::DropDown>().ok()).unwrap();
    mode.set_selected(0);
    let read = main.model().channels[0].last_read_message;
    find_button(w.upcast_ref(), "Salvar notificações do canal").emit_clicked();
    until(context, ||main.model().channels[0].notification_settings==Some(NotificationSettings::Off));
    assert_eq!(main.model().channels[0].last_read_message, read);
    w.close();
    // Preferences returned by whoami survive a fresh application component.
    let api = main.model().api_client.clone();
    let current_user = tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    assert_eq!(current_user.id, id);
    let restored = MainWindowModel::builder().launch(MainWindowInit {
        current_user, api_client: api
    }).detach();
    pump(context);
    assert_eq!(restored.model().account.config.theme, Some(Theme::Dark));
    assert_eq!(restored.model().account.config.display.as_ref().unwrap().show_avatars, Some(false));
}
