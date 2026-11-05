//! EN-US: Keyboard help and explicit public update checks; no background downloads or installation.
//! PT-BR: Ajuda de teclado e consulta explícita de versões; sem downloads ou instalação automática.
use super::*;
use adw::prelude::*;
use crate::releases::{self,Channel,Installation,Release};
#[derive(Debug)]pub enum UtilityMsg {Shortcuts,Updates,Check,Loaded{token:Uuid,result:anyhow::Result<Option<Release>>}}
#[derive(Default)]pub(super) struct Utilities {help:Option<gtk::Window>,updates:Option<Updates>,job:Option<tokio::task::JoinHandle<()>>}
struct Updates{token:Uuid,window:gtk::Window,channel:gtk::DropDown,check:gtk::Button,status:gtk::Label,notes:gtk::Label,link:gtk::LinkButton,download:gtk::LinkButton}
impl Drop for Utilities{fn drop(&mut self){if let Some(job)=self.job.take(){job.abort();}if let Some(w)=self.help.take(){w.close();}if let Some(v)=self.updates.take(){v.window.close();}}}
pub(super) fn shortcuts()->Vec<(&'static str,String)>{let primary=crate::platform::shortcuts::accelerator;vec![
    (crate::i18n::tr("Alternar conversa"),primary("k")),(crate::i18n::tr("Pesquisar mensagens"),primary("f")),
    (crate::i18n::tr("Preferências"),primary("comma")),(crate::i18n::tr("Notificações"),primary("<Shift>n")),
    (crate::i18n::tr("Atalhos de teclado"),primary("F1")),(crate::i18n::tr("Mostrar canais"),"<Alt>1".into()),(crate::i18n::tr("Mostrar mensagens diretas"),"<Alt>2".into()),
    (crate::i18n::tr("Mostrar navegação"),"F9".into()),(crate::i18n::tr("Lista de Membros"),primary("<Shift>m")),
    (crate::i18n::tr("Focar mensagem mais recente"),primary("<Shift>h")),(crate::i18n::tr("Focar caixa de mensagem"),primary("<Shift>l")),
    (crate::i18n::tr("Ações da mensagem em foco"),"<Shift>F10".into()),(crate::i18n::tr("Enviar mensagem"),"Return".into()),(crate::i18n::tr("Quebra de linha"),"<Shift>Return".into()),
    (crate::i18n::tr("Fechar janela ou painel"),"Escape".into()),(crate::i18n::tr("Página anterior do chat em foco"),"Page_Up".into()),(crate::i18n::tr("Página seguinte do chat em foco"),"Page_Down".into()),(crate::i18n::tr("Mensagens mais recentes do chat em foco"),"End".into()),
]}
impl MainWindowModel{pub(super) fn utility_event(&mut self,msg:UtilityMsg,root:&gtk::Box,sender:ComponentSender<Self>){match msg{
    UtilityMsg::Shortcuts=>{
        if let Some(w)=self.utilities.help.as_ref().filter(|w|w.is_visible()){w.present();return;}
        let (w,body)=crate::ui::chat::actions::window(root,crate::i18n::tr("Atalhos de teclado"));w.set_default_size(560,600);
        let rows=gtk::ListBox::new();rows.set_selection_mode(gtk::SelectionMode::None);rows.add_css_class("boxed-list");for(label,accelerator)in shortcuts(){let row=adw::ActionRow::new();row.set_title(label);row.set_title_lines(0);row.add_suffix(&gtk::ShortcutLabel::new(&accelerator));rows.append(&row);}
        body.append(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&rows).build());w.present();self.utilities.help=Some(w);
    },
    UtilityMsg::Updates=>{
        if let Some(v)=self.utilities.updates.as_ref().filter(|v|v.window.is_visible()){v.window.present();return;}
        if let Some(job)=self.utilities.job.take(){job.abort();}
        let (window,body)=crate::ui::chat::actions::window(root,crate::i18n::tr("Sobre e atualizações"));window.set_default_size(600,620);
        let installed=gtk::Label::new(Some(&crate::i18n::fmt("Papo · versão instalada {version}",&[("version",env!("CARGO_PKG_VERSION").into())])));installed.add_css_class("title-2");body.append(&installed);
        let guidance=gtk::Label::new(Some(Installation::current().guidance()));guidance.set_wrap(true);guidance.set_xalign(0.0);guidance.set_selectable(true);body.append(&guidance);
        let channel=gtk::DropDown::from_strings(&[crate::i18n::tr("Estável"),crate::i18n::tr("Experimental (inclui pré-lançamentos)")]);channel.update_property(&[gtk::accessible::Property::Label(crate::i18n::tr("Canal de atualizações"))]);channel.set_selected(if crate::config::AppConfig::load().release_channel==Channel::Experimental{1}else{0});body.append(&channel);
        let info=gtk::Label::new(Some(crate::i18n::tr("A consulta envia somente uma requisição pública ao GitHub. Nenhuma atualização é instalada automaticamente.")));info.set_wrap(true);info.set_xalign(0.0);body.append(&info);
        let check=gtk::Button::with_label(crate::i18n::tr("Verificar atualizações"));let s=sender.clone();check.connect_clicked(move |_|s.input(MainWindowMsg::Utility(UtilityMsg::Check)));body.append(&check);
        let status=gtk::Label::new(None);status.set_wrap(true);body.append(&status);
        let notes=gtk::Label::new(None);notes.set_selectable(true);notes.set_xalign(0.0);notes.set_yalign(0.0);notes.set_wrap(true);notes.update_property(&[gtk::accessible::Property::Label(crate::i18n::tr("Notas da versão"))]);body.append(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&notes).build());
        let link=gtk::LinkButton::with_label(releases::PAGE,crate::i18n::tr("Releases no GitHub"));body.append(&link);
        let download=gtk::LinkButton::with_label(releases::PAGE,crate::i18n::tr("Baixar atualização"));download.set_visible(false);body.append(&download);
        // A changed channel invalidates old notes/downloads before another explicit check.
        let clean=status.clone();let text=notes.clone();let asset=download.clone();let page=link.clone();channel.connect_selected_notify(move |_|{clean.set_text("");text.set_text("");asset.set_visible(false);page.set_uri(releases::PAGE);});
        let weak=window.downgrade();let button=check.downgrade();window.connect_hide(move |_|{if let (Some(w),Some(b))=(weak.upgrade(),button.upgrade()){if !w.is_visible(){b.set_sensitive(true);}}});
        window.set_child(None::<&gtk::Widget>);let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&body).build();scroll.set_widget_name("release-discovery-scroll");window.set_child(Some(&scroll));
        window.present();self.utilities.updates=Some(Updates{token:Uuid::new_v4(),window,channel,check,status,notes,link,download});
    },
    UtilityMsg::Check=>{
        let Some(v)=self.utilities.updates.as_mut().filter(|v|v.window.is_visible()&&v.check.is_sensitive())else{return;};
        if let Some(job)=self.utilities.job.take(){job.abort();}
        let channel=if v.channel.selected()==1{Channel::Experimental}else{Channel::Stable};let token=Uuid::new_v4();v.token=token;v.check.set_sensitive(false);v.channel.set_sensitive(false);v.download.set_visible(false);v.notes.set_text("");v.link.set_uri(releases::PAGE);v.status.set_text(crate::i18n::tr("Consultando GitHub…"));
        self.utilities.job=Some(tokio::spawn(async move{let result=async{tokio::task::spawn_blocking(move||crate::config::AppConfig::update(|c|c.release_channel=channel)).await??;releases::check(channel).await}.await;sender.input(MainWindowMsg::Utility(UtilityMsg::Loaded{token,result}));}));
    },
    UtilityMsg::Loaded{token,result}=>{
        let Some(v)=self.utilities.updates.as_mut().filter(|v|v.token==token&&v.window.is_visible())else{return;};self.utilities.job=None;v.check.set_sensitive(true);v.channel.set_sensitive(true);
        match result{
            Ok(Some(release))=>{
                let newer=release.version().is_some_and(|r|r.cmp_precedence(&semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()).is_gt());
                v.status.set_text(&crate::i18n::fmt(if newer{"Nova versão disponível: {version}"}else{"Sua versão está atualizada. Versão publicada: {version}"},&[("version",release.tag_name.clone())]));
                v.notes.set_text(&release.body.as_deref().unwrap_or(crate::i18n::tr("Sem notas de versão.")).chars().take(32_768).collect::<String>());v.link.set_uri(&release.html_url);
                if newer{if let Some(asset)=releases::download(&release,Installation::current(),std::env::consts::ARCH){v.download.set_uri(&asset.browser_download_url);v.download.set_label(&crate::i18n::fmt("Baixar {file}",&[("file",asset.name)]));v.download.set_visible(true);}else{v.status.set_text(&crate::i18n::fmt("Nova versão {version}. Consulte as notas: nenhum pacote compatível foi encontrado para esta instalação.",&[("version",release.tag_name)]));}}
            },Ok(None)=>v.status.set_text(crate::i18n::tr("Nenhuma versão encontrada neste canal.")),
            Err(_)=>v.status.set_text(crate::i18n::tr("Não foi possível consultar as versões. Verifique a conexão ou o limite do GitHub e tente novamente. As versões também estão disponíveis no link abaixo.")),
        }
    }
}}}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::{mock,pump,until};
    let(api,_backend,server)=tokio::runtime::Handle::current().block_on(mock());let own=tokio::runtime::Handle::current().block_on(api.whoami()).unwrap();let main=MainWindowModel::builder().launch(MainWindowInit{current_user:own,api_client:api}).detach();let window=adw::Window::builder().default_width(1000).default_height(720).content(main.widget()).build();window.present();
    main.widget().activate_action("papo.updates",None).unwrap();until(context,||main.model().utilities.updates.is_some());let token=main.model().utilities.updates.as_ref().unwrap().token;
    let release:Release=serde_json::from_value(serde_json::json!({"tag_name":"v99.0.0","html_url":format!("{}/tag/v99.0.0",releases::PAGE),"body":"Synthetic release notes <b>stay plain text</b>"})).unwrap();
    main.emit(MainWindowMsg::Utility(UtilityMsg::Loaded{token:Uuid::new_v4(),result:Ok(Some(release.clone()))}));pump(context);assert!(main.model().utilities.updates.as_ref().unwrap().notes.text().is_empty());
    main.emit(MainWindowMsg::Utility(UtilityMsg::Loaded{token,result:Ok(Some(release))}));until(context,||main.model().utilities.updates.as_ref().unwrap().notes.text().starts_with("Synthetic"));let channel={let model=main.model();let view=model.utilities.updates.as_ref().unwrap();assert!(!view.notes.uses_markup());assert_eq!(view.link.uri(),format!("{}/tag/v99.0.0",releases::PAGE));assert!(!view.download.is_visible(),"local source builds must not get a guessed package");view.channel.clone()};
    channel.set_selected(1);pump(context);assert!(main.model().utilities.updates.as_ref().unwrap().notes.text().is_empty());assert_eq!(main.model().utilities.updates.as_ref().unwrap().link.uri(),releases::PAGE);
    main.emit(MainWindowMsg::Utility(UtilityMsg::Loaded{token,result:Err(anyhow::anyhow!("synthetic offline"))}));until(context,||!main.model().utilities.updates.as_ref().unwrap().status.text().is_empty());assert!(!main.model().utilities.updates.as_ref().unwrap().download.is_visible());assert!(main.model().utilities.updates.as_ref().unwrap().check.is_sensitive());
    main.model().utilities.updates.as_ref().unwrap().window.close();main.emit(MainWindowMsg::Utility(UtilityMsg::Loaded{token,result:Ok(None)}));pump(context);
    window.set_content(None::<&gtk::Widget>);window.close();drop(main);server.abort();pump(context);
}
