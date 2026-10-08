//! Search owns each query generation; closing or replacing it aborts the old request.
use super::*;
use adw::prelude::*;
use crate::models::{SearchRequest, SearchResponse, SearchResult};

#[derive(Debug)]
pub enum SearchMsg { Open, Submit, More, Cancel(Uuid), Loaded { token: Uuid, result: anyhow::Result<SearchResponse> }, Navigate(Uuid, Uuid) }
struct Choice {widget:gtk::DropDown,ids:Vec<String>,labels:gtk::StringList}
impl Choice {
    fn new()->Self{let labels=gtk::StringList::new(&[]);let widget=gtk::DropDown::new(Some(labels.clone()),None::<gtk::Expression>);widget.set_enable_search(true);Self{widget,ids:Vec::new(),labels}}
    fn append(&mut self,id:Option<&str>,label:&str){self.ids.push(id.unwrap_or("").to_owned());self.labels.append(label);}
    fn set_active(&self,index:Option<u32>){self.widget.set_selected(index.unwrap_or(gtk::INVALID_LIST_POSITION));}
    fn id(&self)->&str{self.ids.get(self.widget.selected() as usize).map(String::as_str).unwrap_or("")}
}
#[derive(Default)]
pub(super) struct Search {
    pub view: Option<SearchView>, pub request: Option<SearchRequest>, pub token: Option<Uuid>,
    pub cursor: Option<MessageCursor>, pub rows: Vec<SearchResult>, pub more: bool,
    pub job: Option<tokio::task::JoinHandle<()>>,
}
pub(super) struct SearchView {
    token:Uuid, window: gtk::Window, fields: Vec<gtk::Entry>, author: Choice, channel: Choice,
    mention: Choice, links: gtk::CheckButton, files: Choice, order: Choice,
    results: gtk::Box, status: gtk::Label, more: gtk::Button,
}
impl Drop for Search { fn drop(&mut self) { if let Some(job)=self.job.take(){job.abort();} if let Some(v)=self.view.take(){v.window.close();} } }
impl MainWindowModel {
    pub(super) fn search_event(&mut self, msg: SearchMsg, root: &gtk::Box, sender: ComponentSender<Self>) {
        match msg {
            SearchMsg::Open => {
                if let Some(v)=self.search.view.as_ref().filter(|v|v.window.is_visible()){v.window.present();return;}
                if let Some(job)=self.search.job.take(){job.abort();} self.search.token=None;
                let view_token=Uuid::new_v4();
                let native=adw::Window::builder().title("Pesquisar mensagens").default_width(540).default_height(520).build();
                let window:gtk::Window=native.clone().upcast();
                if let Some(parent)=root.root().and_downcast::<gtk::Window>(){window.set_transient_for(Some(&parent));}
                let toolbar=adw::ToolbarView::new();let header=adw::HeaderBar::new();header.set_title_widget(Some(&adw::WindowTitle::new("Pesquisar mensagens","")));toolbar.add_top_bar(&header);native.set_content(Some(&toolbar));
                let body=gtk::Box::new(gtk::Orientation::Vertical,12);body.set_margin_top(16);body.set_margin_bottom(16);body.set_margin_start(16);body.set_margin_end(16);toolbar.set_content(Some(&body));
                let fields:Vec<_>=["Pesquisar nas mensagens", "De (AAAA-MM-DD)", "Até (AAAA-MM-DD)"].iter().map(|label|{let e=gtk::Entry::new();e.set_placeholder_text(Some(label));e.set_tooltip_text(Some(label));e}).collect();
                fields[0].set_primary_icon_name(Some("edit-find-symbolic"));fields[0].set_hexpand(true);let input=sender.input_sender().clone();fields[0].connect_activate(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Submit));});body.append(&fields[0]);
                let members=|label:&str|{let mut c=Choice::new();c.append(Some(""),label);for u in &self.users{c.append(Some(&u.id.to_string()),u.display_name());}c.set_active(Some(0));c};
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
                let results=gtk::Box::new(gtk::Orientation::Vertical,8);let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&results).build();body.append(&scroll);
                let more=gtk::Button::with_label("Mais resultados");more.set_visible(false);let input=sender.input_sender().clone();more.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::More));});body.append(&more);
                let input=sender.input_sender().clone();window.connect_close_request(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Cancel(view_token)));gtk::glib::Propagation::Proceed});window.present();fields[0].grab_focus();
                self.search.view=Some(SearchView{token:view_token,window,fields,author,channel,mention,links,files,order,results,status,more});self.search.rows.clear();self.search.cursor=None;self.search.more=false;
            }
            SearchMsg::Cancel(view_token) => {if !self.search.view.as_ref().is_some_and(|v|v.token==view_token){return;}if let Some(job)=self.search.job.take(){job.abort();}self.search.token=None;if let Some(v)=&self.search.view{v.status.set_text("Pesquisa cancelada.");v.more.set_sensitive(self.search.more);}}
            SearchMsg::Submit => {
                if let Some(job)=self.search.job.take(){job.abort();}self.search.token=None;
                let Some(v)=&self.search.view else{return;};
                let id=|c:&Choice|c.id().to_owned();
                let request=SearchRequest{text:v.fields[0].text().trim().to_owned(),date_start:v.fields[1].text().trim().to_owned(),date_end:v.fields[2].text().trim().to_owned(),author:id(&v.author),channel_id:id(&v.channel),mention:id(&v.mention),has:if v.links.is_active(){"link".into()}else{String::new()},order:id(&v.order),contains_attachment:match id(&v.files).as_str(){"yes"=>Some(true),"no"=>Some(false),_=>None}};
                if let Err(e)=request.validate(){v.status.set_text(&e.to_string());return;}
                self.search.request=Some(request);self.search.rows.clear();self.search.cursor=None;self.search.more=false;while let Some(c)=v.results.first_child(){v.results.remove(&c);}self.start_search(sender);
            }
            SearchMsg::More => {if self.search.token.is_none()&&self.search.more{self.start_search(sender);}}
            SearchMsg::Loaded{token,result} => {
                if self.search.token!=Some(token){return;}self.search.token=None;self.search.job=None;
                let Some(v)=&self.search.view else{return;};
                match result {
                    Ok(page)=>{
                        let next=page.results.last().map(|r|MessageCursor{created_at:r.created_at,id:r.id});
                        self.search.more=page.has_more&&next.is_some()&&next!=self.search.cursor;
                        self.search.cursor=next.or(self.search.cursor);
                        for r in page.results {if !self.search.rows.iter().any(|old|old.id==r.id){self.search.rows.push(r);}}
                        v.status.set_text(if self.search.rows.is_empty(){"Nenhuma mensagem encontrada."}else{"Selecione uma mensagem para abrir no chat."});
                    }
                    Err(e)=>{v.status.set_text(&e.to_string());sender.input(MainWindowMsg::ActionError(e));}
                }v.more.set_visible(self.search.more);v.more.set_sensitive(self.search.more);self.render_search(&sender);
            }
            SearchMsg::Navigate(channel_id,message_id)=>{if let Some(v)=&self.search.view{v.window.set_visible(false);}sender.input(MainWindowMsg::Navigate{channel_id,message_id});}
        }
    }
    pub(super) fn render_search(&self,sender:&ComponentSender<Self>){
        let Some(v)=&self.search.view else{return;};
                        while let Some(c)=v.results.first_child(){v.results.remove(&c);}
                        for r in &self.search.rows {
                            if !self.can_read_target(r.channel_id){continue;}
                            let b=gtk::Button::new();b.add_css_class("flat");b.add_css_class("card");let content=gtk::Box::new(gtk::Orientation::Vertical,6);let heading=gtk::Label::new(Some(&format!("#{} · {} · {}",r.channel_name,r.author_username.as_deref().unwrap_or("Usuário removido"),r.created_at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M"))));heading.set_xalign(0.0);heading.set_ellipsize(gtk::pango::EllipsizeMode::End);heading.add_css_class("caption");heading.add_css_class("dim-label");content.append(&heading);
                            let text=crate::ui::chat::mentions::render(r.content.as_deref().unwrap_or("[Anexo]"),&self.users.iter().map(|u|(u.id,u.clone())).collect());let l=gtk::Label::new(Some(&text));l.set_wrap(true);l.set_wrap_mode(gtk::pango::WrapMode::WordChar);l.set_xalign(0.0);content.append(&l);b.set_child(Some(&content));let input=sender.input_sender().clone();let(ch,id)=(r.channel_id,r.id);b.connect_clicked(move |_|{let _=input.send(MainWindowMsg::Search(SearchMsg::Navigate(ch,id)));});v.results.append(&b);
                        }
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
    find_button(main.widget().upcast_ref(),"Pesquisar").emit_clicked();pump(context);let w=main.model().search.view.as_ref().unwrap().window.clone();
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
    let composer=crate::ui::chat::actions::tests::descendants(main.model().chat.widget().upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Entry>().ok()).unwrap();composer.set_text("Olá @al");composer.set_position(-1);pump(context);main.model().chat.emit(ChatMsg::MentionSelected(Some(id)));pump(context);assert_eq!(composer.text(),format!("Olá @mention(<@{id}>) "));
    main.model().chat.emit(ChatMsg::MentionSelected(None));pump(context);assert!(composer.text().contains("@everyone"));
}
