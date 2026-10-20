//! EN-US: Clipboard images enter the same bounded, permission-checked attachment flow as files.
//! PT-BR: Imagens da área de transferência usam o mesmo fluxo de anexos, com limites e verificação de permissões.
use super::*;
use crate::api::features::{UploadFile, MAX_FILE};

fn has_image(clipboard: &gtk::gdk::Clipboard) -> bool {
    let formats=clipboard.formats();
    formats.contains_type(gtk::gdk::Texture::static_type())
        || formats.mime_types().iter().any(|mime|mime.starts_with("image/"))
}

pub(super) fn intercept(entry: &gtk::TextView, input: &relm4::Sender<ChatMsg>, epoch: u64) -> bool {
    if !entry.is_sensitive() || !entry.is_editable() || !has_image(&entry.clipboard()) {return false;}
    let _=input.send(ChatMsg::Transfer(TransferMsg::PasteImage(epoch)));
    true
}

pub(super) fn connect(entry: &gtk::TextView, sender: &ComponentSender<ChatModel>, epoch: std::rc::Rc<std::cell::Cell<u64>>) {
    let input=sender.input_sender().clone();entry.connect_paste_clipboard(move |view|{
        if intercept(view,&input,epoch.get()){view.stop_signal_emission_by_name("paste-clipboard");}
    });
}

impl ChatModel {
    pub(super) fn paste_image(&mut self, epoch: u64, sender: &ComponentSender<Self>, root: &gtk::Box) {
        if epoch!=self.viewport.input_epoch() || self.draft.is_sending() || !self.access.send || !self.access.attachments || self.transfers.clipboard_request.is_some() {return;}
        let Some(channel)=self.active_channel.as_ref().map(|c|c.id()) else{return;};
        if self.draft.files.len()>=10 {self.draft.error=Some("Escolha até 10 arquivos.".into());return;}
        let request=Uuid::new_v4();self.transfers.clipboard_request=Some(request);
        let epoch=self.transfers.epoch;let clipboard=root.clipboard();let input=sender.input_sender().clone();
        gtk::glib::spawn_future_local(async move {
            let result=async {
                let texture=tokio::time::timeout(std::time::Duration::from_secs(10),clipboard.read_texture_future()).await??;
                texture.ok_or_else(||anyhow::anyhow!("Nenhuma imagem disponível na área de transferência."))
            }.await;
            let _=input.send(ChatMsg::Transfer(TransferMsg::ClipboardTexture{epoch,channel,request,result}));
        });
    }

    pub(super) fn prepare_clipboard_image(&mut self, epoch: Uuid, channel: Uuid, request: Uuid, result: anyhow::Result<gtk::gdk::Texture>, sender: &ComponentSender<Self>) {
        if epoch!=self.transfers.epoch || self.transfers.clipboard_request!=Some(request) {return;}
        let input=sender.input_sender().clone();
        self.transfers.jobs.push(tokio::spawn(async move {
            let result=async {
                let texture=result?;
                // Bound decoded pixels before encoding, and keep PNG compression off the UI thread.
                anyhow::ensure!((texture.width() as u64)*(texture.height() as u64)*4<=MAX_FILE,"Imagem muito grande (máximo 100 MiB).");
                let bytes=tokio::task::spawn_blocking(move ||texture.save_to_png_bytes()).await?;
                Ok(vec![UploadFile::clipboard_png(bytes.as_ref()).await?])
            }.await;
            let _=input.send(ChatMsg::Transfer(TransferMsg::PasteReady{epoch,channel,request,result}));
        }));
    }
}

#[cfg(test)]
pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    use actions::tests::{mock,pump,until};
    use adw::prelude::*;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user="12345678-1234-4234-8234-123456789abc".parse().unwrap();
    let a="12345678-1234-4234-8234-123456789abd".parse().unwrap();let b=Uuid::new_v4();
    let channel=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"clipboard","created_at":chrono::Utc::now()})).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{active_channel:Some(channel(a)),user_id:Some(user),api:Some(api),..Default::default()}).detach();
    let access=crate::models::Access::direct();chat.emit(ChatMsg::SetAccess{user_id:user,access:access.clone()});
    let window=adw::Window::builder().default_width(800).default_height(640).content(chat.widget()).build();window.present();performance::settle(context);
    let entry=composer::field(chat.widget().upcast_ref());
    let text=entry.clone();
    let clipboard=entry.clipboard();entry.grab_focus();
    let controllers=entry.observe_controllers();
    let key=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::EventControllerKey>().filter(|c|c.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    let press=|keyval,mods|key.emit_by_name::<bool>("key-pressed",&[&keyval,&0u32,&mods]);
    clipboard.set_text("ordinary pasted text");
    assert!(!press(gtk::gdk::Key::v,gtk::gdk::ModifierType::CONTROL_MASK));
    text.emit_by_name::<()>("paste-clipboard",&[]);
    until(context,||entry.text()=="ordinary pasted text");assert!(chat.model().draft.files.is_empty());
    entry.set_text("caption retained");pump(context);
    let texture=crate::media::PreparedImage{width:2,height:2,pixels:vec![255;16]}.texture();
    clipboard.set_texture(&texture);
    text.emit_by_name::<()>("paste-clipboard",&[]);
    until(context,||chat.model().draft.files.len()==1&&chat.model().transfers.clipboard_request.is_none());
    let path=chat.model().draft.files[0].path.clone();
    let image=image::load_from_memory(&std::fs::read(&path).unwrap()).unwrap();assert_eq!((image.width(),image.height()),(2,2));
    assert_eq!(entry.text(),"caption retained");assert_eq!(chat.model().draft.files[0].name,"imagem-colada.png");
    assert!(backend.lock().unwrap().requests.iter().all(|(method,_,_)|method!="POST"),"paste must only stage an attachment");
    chat.emit(ChatMsg::SendClicked);until(context,||!chat.model().draft.is_sending()&&chat.model().draft.error.as_ref().is_some_and(|e|e.contains("Upload failed")));
    assert!(path.exists());assert_eq!(chat.model().draft.files.len(),1);assert_eq!(entry.text(),"caption retained");
    chat.emit(ChatMsg::SendClicked);until(context,||chat.model().draft.files.is_empty()&&!path.exists());
    let uploads:Vec<_>=backend.lock().unwrap().requests.iter().filter(|(method,path,_)|method=="POST"&&path=="/messages").cloned().collect();
    assert_eq!(uploads.len(),2);assert!(uploads.iter().all(|(_,_,body)|body.as_str().unwrap().contains("filename=\"imagem-colada.png\"")));

    // Keyboard paste, channel drafts and removal share the same staging lifetime.
    let png=texture.save_to_png_bytes();
    clipboard.set_content(Some(&gtk::gdk::ContentProvider::for_bytes("image/png",&png))).unwrap();
    assert!(press(gtk::gdk::Key::v,gtk::gdk::ModifierType::CONTROL_MASK));
    until(context,||chat.model().draft.files.len()==1);
    let path=chat.model().draft.files[0].path.clone();
    let old_input=chat.model().viewport.input_epoch();let old_epoch=chat.model().transfers.epoch;
    chat.emit(ChatMsg::SetChannel(channel(b)));pump(context);assert!(chat.model().draft.files.is_empty()&&path.exists());
    let stale=tokio::runtime::Handle::current().block_on(UploadFile::clipboard_png(b"late clipboard data")).unwrap();let stale_path=stale.path.clone();
    chat.emit(ChatMsg::Transfer(TransferMsg::PasteReady{epoch:old_epoch,channel:a,request:Uuid::new_v4(),result:Ok(vec![stale])}));
    chat.emit(ChatMsg::Transfer(TransferMsg::PasteImage(old_input)));pump(context);
    assert!(chat.model().draft.files.is_empty()&&chat.model().transfers.clipboard_request.is_none()&&!stale_path.exists());
    chat.emit(ChatMsg::SetChannel(channel(a)));pump(context);assert_eq!(chat.model().draft.files.len(),1);
    chat.emit(ChatMsg::Transfer(TransferMsg::Remove(0)));pump(context);assert!(!path.exists());
    assert!(press(gtk::gdk::Key::Insert,gtk::gdk::ModifierType::SHIFT_MASK));
    until(context,||chat.model().draft.files.len()==1);let path=chat.model().draft.files[0].path.clone();
    chat.emit(ChatMsg::Transfer(TransferMsg::Remove(0)));pump(context);assert!(!path.exists());

    // Pending reads cannot send a caption early, and a revoked permission rejects completion.
    entry.set_text("waiting for image");pump(context);
    let request=Uuid::new_v4();chat.state().get_mut().model.transfers.clipboard_request=Some(request);
    let before=backend.lock().unwrap().requests.len();chat.emit(ChatMsg::SendClicked);pump(context);
    assert!(!chat.model().draft.is_sending());assert_eq!(backend.lock().unwrap().requests.len(),before);
    let mut denied=access.clone();denied.attachments=false;chat.emit(ChatMsg::SetAccess{user_id:user,access:denied});pump(context);
    let late=tokio::runtime::Handle::current().block_on(UploadFile::clipboard_png(b"revoked clipboard data")).unwrap();let path=late.path.clone();
    let epoch=chat.model().transfers.epoch;
    chat.emit(ChatMsg::Transfer(TransferMsg::PasteReady{epoch,channel:a,request,result:Ok(vec![late])}));pump(context);
    assert!(chat.model().draft.files.is_empty()&&!path.exists());
    chat.emit(ChatMsg::Transfer(TransferMsg::PasteImage(chat.model().viewport.input_epoch())));pump(context);assert!(chat.model().transfers.clipboard_request.is_none());
    chat.emit(ChatMsg::SetAccess{user_id:user,access});pump(context);
    let staged=tokio::runtime::Handle::current().block_on(UploadFile::clipboard_png(b"limit fixture")).unwrap();let path=staged.path.clone();
    chat.state().get_mut().model.draft.files=vec![staged;10];
    chat.emit(ChatMsg::Transfer(TransferMsg::PasteImage(chat.model().viewport.input_epoch())));pump(context);
    assert_eq!(chat.model().draft.files.len(),10);assert!(chat.model().draft.error.as_ref().unwrap().contains("10"));assert!(chat.model().transfers.clipboard_request.is_none());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();pump(context);assert!(!path.exists());
}
