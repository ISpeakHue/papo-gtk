//! EN-US: Search owns each query generation; closing or replacing it aborts the old request.
//! PT-BR: Cada busca tem sua própria geração; fechar ou substituir a consulta cancela a requisição anterior.
use super::*;
use adw::prelude::*;
use crate::models::{SearchRequest, SearchResponse, SearchResult};
mod rows;

#[derive(Debug)]
pub enum SearchMsg { ExternalVisible{view:Uuid,channel:Uuid,author:crate::models::ExternalAuthor}, Open, MemberVisible{view:Uuid,id:Uuid}, AvatarBatch{view:Uuid,batch:Uuid}, Changed, Debounced(Uuid), Submit, More, Cancel(Uuid), Loaded { token: Uuid, result: anyhow::Result<SearchResponse> }, Navigate(Uuid, Uuid) }
mod choice;
use choice::Choice;
#[derive(Default)]
pub(super) struct Search {
    pub view: Option<SearchView>, pub request: Option<SearchRequest>, pub token: Option<Uuid>,
    pub cursor: Option<MessageCursor>, pub rows: Vec<SearchResult>, pub more: bool,
    pub job: Option<tokio::task::JoinHandle<()>>, debounce:Option<(Uuid,gtk::glib::SourceId)>,
    ids:std::collections::HashSet<Uuid>,avatar_pending:std::collections::HashSet<Uuid>,avatar_batch:Option<Uuid>,
}
pub(super) struct SearchView {
    token:Uuid, window: gtk::Window, fields: Vec<gtk::Entry>, author: Choice, channel: Choice,
    mention: Choice, links: gtk::CheckButton, files: Choice, order: Choice,
    results: gtk::gio::ListStore, status: gtk::Label, more: gtk::Button,
}
impl Drop for Search { fn drop(&mut self) { if let Some((_,timer))=self.debounce.take(){if let Some(source)=gtk::glib::MainContext::default().find_source_by_id(&timer){source.destroy();}} if let Some(job)=self.job.take(){job.abort();} if let Some(v)=self.view.take(){v.window.close();} } }
impl MainWindowModel {
    pub(super) fn navigation_action(&self,channel:Uuid,id:Uuid)->crate::ui::chat::actions::ActionMsg{
        use crate::ui::chat::actions::ActionMsg;
        let date=self.search.rows.iter().find(|r|r.id==id&&r.channel_id==channel).map(|r|r.created_at)
            .or_else(||self.notifications.inbox.messages.get(&id).filter(|m|m.channel_id==channel).map(|m|m.created_at));
        date.map_or(ActionMsg::Navigate(id),|created_at|ActionMsg::NavigateAt(MessageCursor{created_at,id}))
    }

    pub(super) fn search_event(&mut self, msg: SearchMsg, root: &gtk::Box, sender: ComponentSender<Self>) {
        match msg {
            SearchMsg::ExternalVisible{view,channel,author}=>{if self.can_read_target(channel)&&self.search.view.as_ref().is_some_and(|v|v.token==view&&v.window.is_visible()){self.chat.emit(ChatMsg::ExternalAvatars(vec![author]));}},
            SearchMsg::Open => {
                if let Some(v)=self.search.view.as_ref().filter(|v|v.window.is_visible()){v.window.present();return;}
                if let Some((_,timer))=self.search.debounce.take(){if let Some(source)=gtk::glib::MainContext::default().find_source_by_id(&timer){source.destroy();}}
                if let Some(job)=self.search.job.take(){job.abort();} self.search.token=None;
                let view_token=Uuid::new_v4();
                let native=adw::Window::builder().title("Pesquisar mensagens").default_width(760).default_height(720).build();
                let window:gtk::Window=native.clone().upcast();
                if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
                let toolbar=adw::ToolbarView::new();let header=adw::HeaderBar::new();header.set_title_widget(Some(&adw::WindowTitle::new("Pesquisar mensagens","")));toolbar.add_top_bar(&header);native.set_content(Some(&toolbar));
                let body=gtk::Box::new(gtk::Orientation::Vertical,12);body.set_margin_top(16);body.set_margin_bottom(16);body.set_margin_start(16);body.set_margin_end(16);toolbar.set_content(Some(&body));
                let fields:Vec<_>=["Pesquisar nas mensagens", "De (AAAA-MM-DD)", "Até (AAAA-MM-DD)"].iter().map(|label|{let e=gtk::Entry::new();e.set_placeholder_text(Some(label));e.set_tooltip_text(Some(label));e}).collect();
                fields[0].set_primary_icon_name(Some("papo-edit-find-symbolic"));fields[0].set_hexpand(true);let input=sender.input_sender().clone();fields[0].connect_activate(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Submit));});body.append(&fields[0]);
                let members=|label:&str|Choice::members(label,&self.users,&self.avatars.textures,sender.input_sender().clone(),view_token);
                let author=members("Qualquer autor");let mention=members("Qualquer menção");let mut channel=Choice::new();channel.append(Some(""),"Todos os canais permitidos");for c in &self.channels{channel.append(Some(&c.id.to_string()),&c.name);}for d in &self.direct.items{channel.append(Some(&d.id.to_string()),&format!("DM · {}",d.user.display_name()));}channel.set_active(Some(0));
                let links=gtk::CheckButton::with_label("Contém link");
                let mut files=Choice::new();for (id,label) in [("any","Com ou sem anexos"),("yes","Com anexos"),("no","Sem anexos")]{files.append(Some(id),label);}files.set_active(Some(0));
                let mut order=Choice::new();order.append(Some("desc"),"Mais recentes primeiro");order.append(Some("asc"),"Mais antigas primeiro");order.set_active(Some(0));
                let filter=gtk::FlowBox::new();filter.set_selection_mode(gtk::SelectionMode::None);filter.set_min_children_per_line(1);filter.set_max_children_per_line(2);filter.set_column_spacing(12);filter.set_row_spacing(10);filter.set_margin_top(12);
                for (label,widget) in [("Autor",author.widget.upcast_ref::<gtk::Widget>()),("Canal",channel.widget.upcast_ref()),("Menção",mention.widget.upcast_ref()),("Anexos",files.widget.upcast_ref()),("Data inicial",fields[1].upcast_ref()),("Data final",fields[2].upcast_ref()),("Ordenação",order.widget.upcast_ref()),("Conteúdo",links.upcast_ref())]{let field=gtk::Box::new(gtk::Orientation::Vertical,4);let title=gtk::Label::new(Some(label));title.set_xalign(0.0);title.add_css_class("caption");field.append(&title);field.append(widget);filter.insert(&field,-1);}
                let filters=gtk::Expander::new(Some("Filtros avançados"));filters.set_child(Some(&filter));body.append(&filters);
                let actions=gtk::Box::new(gtk::Orientation::Horizontal,8);
                for (label,submit) in [("Pesquisar",true),("Cancelar pesquisa",false)]{let b=gtk::Button::with_label(label);if submit{b.add_css_class("suggested-action");}let input=sender.input_sender().clone();b.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Search(if submit{SearchMsg::Submit}else{SearchMsg::Cancel(view_token)}));});actions.append(&b);}body.append(&actions);
                let status=gtk::Label::new(Some("Digite um termo ou escolha filtros para pesquisar."));status.set_wrap(true);status.set_xalign(0.0);status.add_css_class("dim-label");body.append(&status);
                let(results,result_view)=rows::view(&sender,view_token);let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&result_view).build();scroll.set_widget_name("message-search-results");body.append(&scroll);
                let more=gtk::Button::with_label("Mais resultados");more.set_visible(false);let input=sender.input_sender().clone();more.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::More));});body.append(&more);
                let keys=gtk::EventControllerKey::new();keys.set_propagation_phase(gtk::PropagationPhase::Capture);let weak=window.downgrade();
                keys.connect_key_pressed(move |_,key,_,_|{if key==gtk::gdk::Key::Escape{if let Some(window)=weak.upgrade(){window.close();}gtk::glib::Propagation::Stop}else{gtk::glib::Propagation::Proceed}});window.add_controller(keys);
                let input=sender.input_sender().clone();window.connect_close_request(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Cancel(view_token)));gtk::glib::Propagation::Proceed});window.present();fields[0].grab_focus();
                for field in &fields{let input=sender.input_sender().clone();field.connect_changed(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Changed));});}
                for choice in [&author,&mention,&channel,&files,&order]{let input=sender.input_sender().clone();choice.selection.connect_selected_notify(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Changed));});}
                let input=sender.input_sender().clone();links.connect_toggled(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Changed));});
                self.search.view=Some(SearchView{token:view_token,window,fields,author,channel,mention,links,files,order,results,status,more});self.search.rows.clear();self.search.ids.clear();self.search.cursor=None;self.search.more=false;self.search.avatar_pending.clear();self.search.avatar_batch=None;
            }
            SearchMsg::Cancel(view_token) => {if !self.search.view.as_ref().is_some_and(|v|v.token==view_token){return;}if let Some((_,timer))=self.search.debounce.take(){if let Some(source)=gtk::glib::MainContext::default().find_source_by_id(&timer){source.destroy();}}if let Some(job)=self.search.job.take(){job.abort();}self.search.token=None;if let Some(v)=&self.search.view{v.status.set_text("Pesquisa cancelada.");v.more.set_sensitive(self.search.more);}}
            SearchMsg::MemberVisible{view,id}=>{
                if !self.search.view.as_ref().is_some_and(|v|v.token==view&&v.window.is_visible())||self.avatars.textures.contains_key(&id){return;}
                self.search.avatar_pending.insert(id);
                if self.search.avatar_batch.is_none(){let batch=Uuid::new_v4();self.search.avatar_batch=Some(batch);let input=sender.input_sender().clone();
                    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(40),move ||{let _=input.send(MainWindowMsg::Search(SearchMsg::AvatarBatch{view,batch}));});}
            }
            SearchMsg::AvatarBatch{view,batch}=>{
                if self.search.avatar_batch!=Some(batch)||!self.search.view.as_ref().is_some_and(|v|v.token==view&&v.window.is_visible()){return;}
                self.search.avatar_batch=None;let ids=std::mem::take(&mut self.search.avatar_pending).into_iter().collect();self.load_avatars(ids,false,sender);
            }
            SearchMsg::Changed=>{
                if !self.search.view.as_ref().is_some_and(|v|v.window.is_visible()){return;}
                if let Some((_,timer))=self.search.debounce.take(){if let Some(source)=gtk::glib::MainContext::default().find_source_by_id(&timer){source.destroy();}}
                if let Some(job)=self.search.job.take(){job.abort();}self.search.token=None;self.search.more=false;if let Some(v)=&self.search.view{v.more.set_visible(false);}
                let token=Uuid::new_v4();let input=sender.input_sender().clone();
                let timer=gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(350),move ||{let _=input.send(MainWindowMsg::Search(SearchMsg::Debounced(token)));});self.search.debounce=Some((token,timer));
            }
            SearchMsg::Debounced(token)=>{if self.search.debounce.as_ref().is_some_and(|(id,_)|*id==token){self.search.debounce.take();sender.input(MainWindowMsg::Search(SearchMsg::Submit));}}
            SearchMsg::Submit => {
                if let Some((_,timer))=self.search.debounce.take(){if let Some(source)=gtk::glib::MainContext::default().find_source_by_id(&timer){source.destroy();}}
                if let Some(job)=self.search.job.take(){job.abort();}self.search.token=None;
                let Some(v)=self.search.view.as_ref().filter(|v|v.window.is_visible()) else{return;};
                let id=|c:&Choice|c.id().to_owned();
                let request=SearchRequest{text:v.fields[0].text().trim().to_owned(),date_start:v.fields[1].text().trim().to_owned(),date_end:v.fields[2].text().trim().to_owned(),author:id(&v.author),channel_id:id(&v.channel),mention:id(&v.mention),has:if v.links.is_active(){"link".into()}else{String::new()},order:id(&v.order),contains_attachment:match id(&v.files).as_str(){"yes"=>Some(true),"no"=>Some(false),_=>None}};
                if let Err(e)=request.validate(){v.status.set_text(&e.to_string());return;}
                self.search.request=Some(request);self.search.rows.clear();self.search.ids.clear();self.search.cursor=None;self.search.more=false;v.results.remove_all();self.start_search(sender);
            }
            SearchMsg::More => {if self.search.debounce.is_none()&&self.search.token.is_none()&&self.search.more{self.start_search(sender);}}
            SearchMsg::Loaded{token,result} => {
                if self.search.token!=Some(token){return;}self.search.token=None;self.search.job=None;
                let Some(v)=&self.search.view else{return;};
                match result {
                    Ok(page)=>{
                        let next=page.results.last().map(|r|MessageCursor{created_at:r.created_at,id:r.id});
                        self.search.more=page.has_more&&next.is_some()&&next!=self.search.cursor;
                        self.search.cursor=next.or(self.search.cursor);
                        for r in page.results {if self.search.ids.insert(r.id){self.search.rows.push(r);}}
                        v.status.set_text(if self.search.rows.is_empty(){"Nenhuma mensagem encontrada."}else{"Selecione uma mensagem para abrir no chat."});
                    }
                    Err(e)=>{v.status.set_text(&e.to_string());sender.input(MainWindowMsg::ActionError(e));}
                }v.more.set_visible(self.search.more);v.more.set_sensitive(self.search.more);self.render_search(&sender);
            }
            SearchMsg::Navigate(channel_id,message_id)=>{if let Some(v)=&self.search.view{v.window.close();}sender.input(MainWindowMsg::Navigate{channel_id,message_id});}
        }
    }
    pub(super) fn render_search(&self,_sender:&ComponentSender<Self>){
        let Some(v)=self.search.view.as_ref().filter(|v|v.window.is_visible())else{return;};
        let _timing=crate::performance::timer("search_update_us");
        for choice in [&v.author,&v.mention]{if let Some(avatars)=&choice.avatars{crate::media::update_member_avatars(choice.widget.upcast_ref(),&avatars.borrow(),&self.avatars.textures);*avatars.borrow_mut()=self.avatars.textures.clone();}}
        let users:HashMap<_,_>=self.users.iter().map(|u|(u.id,u.clone())).collect();
        let (revision,renderer)=self.chat.model().text_renderer(users.clone());
        let desired=self.search.rows.iter().filter(|r|self.can_read_target(r.channel_id)).map(|r|rows::Row{
            external:r.external().cloned(),avatar:r.external().and_then(|a|self.chat.model().external_avatar(a)),id:r.id,channel:r.channel_id,revision,renderer:renderer.clone(),display:crate::ui::chat::mentions::render(r.content.as_deref().unwrap_or("[Anexo]"),&users),
            heading:format!("#{} · {} · {}",r.channel_name,r.external().map(|a|a.name.as_str()).or(r.author_username.as_deref()).unwrap_or(if r.webhook_id.is_some(){"Webhook"}else{"Usuário removido"}),r.created_at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M")),
            text:r.content.as_deref().unwrap_or("[Anexo]").to_owned(),
        }).collect();rows::update(&v.results,desired);
    }
    fn start_search(&mut self,sender:ComponentSender<Self>){
        let Some(request)=self.search.request.clone() else{return;};if let Some(job)=self.search.job.take(){job.abort();}
        let token=Uuid::new_v4();self.search.token=Some(token);let cursor=self.search.cursor;let api=self.api_client.clone();
        if let Some(v)=&self.search.view{v.status.set_text("Pesquisando…");v.more.set_sensitive(false);}
        self.search.job=Some(tokio::spawn(async move{let result=api.search(&request,cursor).await;sender.input(MainWindowMsg::Search(SearchMsg::Loaded{token,result}));}));
    }
}
#[cfg(test)]
pub(crate) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{find_button,pump,until};
    find_button(main.widget().upcast_ref(),"Buscar mensagens (Ctrl+K)").emit_clicked();pump(context);let w=main.model().search.view.as_ref().unwrap().window.clone();
    find_button(w.upcast_ref(),"Pesquisar").emit_clicked();pump(context);assert!(main.model().search.view.as_ref().unwrap().status.text().contains("filtro"));
    main.model().search.view.as_ref().unwrap().fields[0].set_text("hello");find_button(w.upcast_ref(),"Pesquisar").emit_clicked();until(context,||main.model().search.token.is_none()&&main.model().search.view.as_ref().unwrap().status.text().contains("Search failed"));assert_eq!(main.model().search.view.as_ref().unwrap().fields[0].text(),"hello");
    find_button(w.upcast_ref(),"Pesquisar").emit_clicked();until(context,||main.model().search.token.is_none()&&main.model().search.rows.len()==1);
    let before=main.model().search.rows[0].id;
    main.emit(MainWindowMsg::Search(SearchMsg::Loaded{token:Uuid::new_v4(),result:Ok(SearchResponse{results:vec![],has_more:false})}));pump(context);assert_eq!(main.model().search.rows[0].id,before);
    find_button(w.upcast_ref(),"Mais resultados").emit_clicked();until(context,||main.model().search.token.is_none()&&main.model().search.rows.len()==2);assert!(!main.model().search.more);
    assert!(!crate::ui::chat::actions::tests::descendants(w.upcast_ref()).iter().find_map(|w|w.downcast_ref::<gtk::Expander>().cloned()).unwrap().is_expanded());crate::ui::main_window::layout::tests::preview(&w,"search",context);
    let target=main.model().search.rows[1].clone();main.emit(MainWindowMsg::Search(SearchMsg::Navigate(target.channel_id,target.id)));until(context,||crate::ui::chat::actions::tests::descendants(main.model().chat.widget().upcast_ref()).iter().any(|w|w.widget_name()==format!("message-{}",target.id)&&w.has_css_class("papo-message-highlight")));
    main.emit(MainWindowMsg::Search(SearchMsg::Open));pump(context);
    let view_token=main.model().search.view.as_ref().unwrap().token;
    main.model().search.view.as_ref().unwrap().fields[0].set_text("cancel me");
    main.emit(MainWindowMsg::Search(SearchMsg::Submit));main.emit(MainWindowMsg::Search(SearchMsg::Cancel(view_token)));pump(context);
    assert!(main.model().search.token.is_none());assert!(main.model().search.rows.is_empty());
    main.emit(MainWindowMsg::Search(SearchMsg::Loaded{token:Uuid::new_v4(),result:Ok(SearchResponse{results:vec![target.clone()],has_more:false})}));pump(context);assert!(main.model().search.rows.is_empty());
    main.model().search.view.as_ref().unwrap().window.close();pump(context);
    // Mention insertion uses the real composer and permission gate.
    let id=main.model().current_user.id;main.model().chat.emit(ChatMsg::InputChanged("Olá @al".into()));pump(context);
    // Set editable text, since this synthetic input is also used for state testing.
    let composer=crate::ui::chat::composer::field(main.model().chat.widget().upcast_ref());composer.set_text("Olá @al");composer.set_position(-1);pump(context);main.model().chat.emit(ChatMsg::MentionSelected(Some(id)));pump(context);assert_eq!(composer.text(),format!("Olá @{} ",main.model().current_user.display_name()));
    main.model().chat.emit(ChatMsg::MentionSelected(None));pump(context);assert!(composer.text().contains("@everyone"));
}

#[cfg(test)]pub(crate) fn exercise_recycling(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let emoji=Uuid::new_v4();backend.lock().unwrap().emojis=vec![serde_json::json!({"id":emoji,"name":"searchdance","format":"PNG","image_blob":crate::ui::chat::actions::tests::tiny_png_base64(),"created_at":"2026-10-03T00:00:00Z"})];
    let current_user=api.clone();let user=tokio::runtime::Handle::current().block_on(current_user.whoami()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:user,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1000).default_height(720).content(main.widget()).build();host.present();until(context,||main.model().background_started&&main.model().access_request.is_none());
    choice::exercise_scroll_reset(context,main.sender().clone());
    exercise(&main,context);
    main.emit(MainWindowMsg::Search(SearchMsg::Open));pump(context);
    let baseline=backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path.starts_with("/search")).count();
    {let model=main.model();let view=model.search.view.as_ref().unwrap();view.fields[0].set_text("r");view.fields[0].set_text("ru");view.fields[0].set_text("rust");
        let filter=gtk::StringFilter::new(view.author.filter.expression().as_ref());filter.set_match_mode(gtk::StringFilterMatchMode::Substring);filter.set_ignore_case(true);filter.set_search(Some("LIC"));
        let filtered=gtk::FilterListModel::new(Some(view.author.labels.clone()),Some(filter));assert_eq!(filtered.n_items(),1,"typing a substring must filter members by nickname/username");
        assert!(view.author.list.factory().is_some()&&view.mention.list.factory().is_some());
    }
    let (button,list,labels,selection)={let model=main.model();let view=model.search.view.as_ref().unwrap();(view.author.widget.clone(),view.author.list.clone(),view.author.labels.clone(),view.author.selection.clone())};
    {let window=main.model().search.view.as_ref().unwrap().window.clone();for widget in descendants(window.upcast_ref()){if let Some(expander)=widget.downcast_ref::<gtk::Expander>(){expander.set_expanded(true);}}}
    crate::ui::chat::performance::settle(context);button.popup();crate::ui::chat::performance::settle(context);
    let popup=button.popover().unwrap();let query=descendants(popup.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::SearchEntry>().ok()).unwrap();
    let popup_model=list.model().unwrap();let selected=selection.selected();
    query.set_text("LIC");pump(context);assert_eq!(popup_model.n_items(),1);
    query.set_text("");pump(context);assert_eq!(popup_model.n_items(),labels.n_items());
    assert_eq!(popup_model.item(0).and_downcast::<gtk::StringObject>().unwrap().string(),"Qualquer autor");
    assert_eq!(selection.selected(),selected,"typing and clearing never commits a filter selection");button.popdown();
    until(context,||main.model().search.request.as_ref().is_some_and(|r|r.text=="rust")&&main.model().search.token.is_none()&&main.model().search.debounce.is_none());
    assert_eq!(backend.lock().unwrap().requests.iter().filter(|(_,path,_)|path.starts_with("/search")).count(),baseline+1,"typing a burst issues one debounced search");
    let channel=main.model().channels[0].id;let own=main.model().current_user.id;
    until(context,||main.model().chat.model().has_profile_emoji(":searchdance:"));
    {let mut state=main.state().get_mut();let search=&mut state.model.search;search.rows.clear();search.ids.clear();search.view.as_ref().unwrap().results.remove_all();}
    let results:Vec<SearchResult>=(0..1000).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"channel_name":"test","type":"message","author_id":own,"author_username":"alice","content":format!("Result {i} 😄 :searchdance: @mention(<@{own}>)"),"created_at":"2026-10-15T00:00:00Z"})).unwrap()).collect();
    let token=Uuid::new_v4();main.state().get_mut().model.search.token=Some(token);
    main.emit(MainWindowMsg::Search(SearchMsg::Loaded{token,result:Ok(SearchResponse{results:results[..500].to_vec(),has_more:true})}));until(context,||main.model().search.view.as_ref().unwrap().results.n_items()==500);
    let store=main.model().search.view.as_ref().unwrap().results.clone();let first=store.item(0).unwrap();let w=main.model().search.view.as_ref().unwrap().window.clone();
    let scroll=descendants(w.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::ScrolledWindow>().ok().filter(|s|s.widget_name()=="message-search-results")).unwrap();
    let position=scroll.vadjustment().value();
    let token=Uuid::new_v4();main.state().get_mut().model.search.token=Some(token);
    main.emit(MainWindowMsg::Search(SearchMsg::Loaded{token,result:Ok(SearchResponse{results:results[490..].to_vec(),has_more:false})}));until(context,||store.n_items()==1000);
    assert_eq!(store.item(0).unwrap(),first,"pagination retains existing model objects");assert_eq!(scroll.vadjustment().value(),position);
    assert_eq!(main.model().search.rows.len(),1000,"overlapping pages are deduplicated");
    let widgets=descendants(scroll.upcast_ref()).len();assert!(widgets<1500,"search must recycle widgets: {widgets}");
    let rich=descendants(scroll.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::TextView>().ok()).find(|v|{let b=v.buffer();b.text(&b.start_iter(),&b.end_iter(),false).contains("Result 0")}).unwrap();
    let buffer=rich.buffer();assert!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),false).contains("😄"));let mut iter=buffer.start_iter();let mut painted=false;loop{painted|=iter.paintable().is_some();if !iter.forward_char(){break;}}assert!(painted,"search results render custom emoji rather than colon codes");
    let object=store.item(0).and_downcast::<gtk::glib::BoxedAnyObject>().unwrap();assert!(!object.borrow::<rows::Row>().display.contains("@mention("));
    w.close();host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}

#[cfg(test)]pub(crate) fn exercise_backend_identity(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until,descendants};
    let(api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let hook=Uuid::new_v4();let message=Uuid::new_v4();let notice=Uuid::new_v4();let channel:Uuid=backend.lock().unwrap().message["channel_id"].as_str().unwrap().parse().unwrap();
    let url=format!("/webhooks/{hook}/avatar");let name="Bridge sender";
    let external=serde_json::json!({"source":"webhook","id":hook.to_string(),"name":name,"avatar_url":url});
    let author:crate::models::ExternalAuthor=serde_json::from_value(external.clone()).unwrap();
    let mut png=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(32,32).write_to(&mut png,image::ImageFormat::Png).unwrap();
    {let mut b=backend.lock().unwrap();b.attachment_thumbnails.insert(url.clone(),png.into_inner());
        b.search_results=Some(vec![serde_json::json!({"type":"message","id":message,"channel_id":channel,"channel_name":"general","author_id":null,"webhook_id":hook,"external_author":external,"author_username":"fallback","content":"Metadata result","created_at":"2026-10-19T12:00:00Z"})]);
        b.discovery_notifications=Some(vec![serde_json::json!({"id":notice,"message_id":message,"channel_id":channel,"author_id":null,"webhook_id":hook,"external_author":external,"message_content":"Metadata notice","read":false,"created_at":"2026-10-19T12:00:00Z"})]);
    }
    let own=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user:own,api_client:api}).detach();
    let host=adw::Window::builder().default_width(1000).default_height(720).content(main.widget()).build();host.present();until(context,||main.model().background_started&&main.model().access_request.is_none());
    main.emit(MainWindowMsg::Search(SearchMsg::Open));pump(context);let window=main.model().search.view.as_ref().unwrap().window.clone();
    main.model().search.view.as_ref().unwrap().fields[0].set_text("Metadata");main.emit(MainWindowMsg::Search(SearchMsg::Submit));until(context,||main.model().search.rows.len()==1);
    until(context,||main.model().chat.model().external_avatar(&author).is_some());crate::ui::chat::performance::settle(context);
    let avatars=|root:&gtk::Widget|descendants(root).into_iter().filter_map(|w|w.downcast::<adw::Avatar>().ok()).filter(|a|a.text().as_deref()==Some(name)).collect::<Vec<_>>();
    assert!(avatars(window.upcast_ref()).iter().any(|a|a.custom_image().is_some()),"search binds the REST identity and avatar without opening its channel");
    assert!(descendants(window.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text().contains(name))));
    let count=||backend.lock().unwrap().requests.iter().filter(|(_,p,_)|p==&url).count();assert_eq!(count(),1);
    main.emit(MainWindowMsg::Notification(super::notifications::NoticeMsg::Open));
    until(context,||main.model().notifications.inbox.rows.contains_key(&notice));
    assert!(!main.model().notifications.inbox.messages.contains_key(&message),"this identity must come from the REST notice, not observed chat history");
    let inbox=||gtk::Window::list_toplevels().into_iter().filter_map(|w|w.downcast::<gtk::Window>().ok()).find(|w|w.is_visible()&&w.title().as_deref()==Some("Notificações"));
    until(context,||inbox().is_some());let inbox=inbox().unwrap();
    let group=||descendants(inbox.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Expander>().ok().filter(|w|w.widget_name()==format!("notification-channel-{channel}")));
    until(context,||group().is_some());group().unwrap().set_expanded(true);crate::ui::chat::performance::settle(context);
    assert!(avatars(inbox.upcast_ref()).iter().any(|a|a.custom_image().is_some()),"notices reuse the already decoded search avatar");assert_eq!(count(),1);
    assert!(descendants(inbox.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()==name)));
    window.close();main.emit(MainWindowMsg::Search(SearchMsg::Open));pump(context);main.model().search.view.as_ref().unwrap().fields[0].set_text("Metadata");main.emit(MainWindowMsg::Search(SearchMsg::Submit));until(context,||main.model().search.rows.len()==1);crate::ui::chat::performance::settle(context);assert_eq!(count(),1,"reopening discovery views cannot refetch the same picture");
    {main.state().get_mut().model.access.insert(channel,crate::models::Access::default());}main.emit(MainWindowMsg::ExternalAvatarsChanged);pump(context);
    assert_eq!(main.model().search.view.as_ref().unwrap().results.n_items(),0);assert!(!descendants(inbox.upcast_ref()).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.text()==name)),"revoked channels cannot expose cached external identities");
    inbox.close();main.model().search.view.as_ref().unwrap().window.close();host.set_content(None::<&gtk::Widget>);host.close();drop(main);server.abort();pump(context);
}
