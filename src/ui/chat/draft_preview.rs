//! Small local thumbnails for staged attachments; no upload or live player.
use super::*;
use crate::api::features::UploadFile;
use std::{path::{Path,PathBuf},sync::Arc};

#[derive(Clone,Debug,PartialEq,Eq,Hash)]
struct Key {path:PathBuf,size:u64}
impl From<&UploadFile> for Key {fn from(file:&UploadFile)->Self{Self{path:file.path.clone(),size:file.size}}}
#[derive(Clone,Copy,PartialEq,Eq)]
enum Kind {Image,Video,File}
impl Kind {
    fn of(file:&UploadFile)->Self {
        // Recognize supported image extensions without reading the file on GTK.
        // GIO can classify PNG as an empty file when given an empty sniff buffer.
        if image::ImageFormat::from_path(&file.path).is_ok(){return Self::Image;}
        let (kind,_)=gtk::gio::content_type_guess(Some(file.path.as_path()),&[]);
        let mime=gtk::gio::content_type_get_mime_type(&kind);
        match mime.as_deref(){Some(v) if v.starts_with("image/")=>Self::Image,Some(v) if v.starts_with("video/")=>Self::Video,_=>Self::File}
    }
    fn icon(self)->&'static str{match self{Self::Image=>"image-x-generic-symbolic",Self::Video=>"video-x-generic-symbolic",Self::File=>"text-x-generic-symbolic"}}
}
struct Entry {
    token:Uuid,texture:Option<gtk::gdk::Texture>,job:Option<tokio::task::JoinHandle<()>>,
    views:Vec<(gtk::glib::WeakRef<gtk::Picture>,gtk::glib::WeakRef<gtk::Image>)>,
}
impl Drop for Entry {fn drop(&mut self){if let Some(job)=self.job.take(){job.abort();}}}
pub(super) struct Previews {
    context:Option<(Uuid,Option<Uuid>)>,entries:HashMap<Key,Entry>,limiter:Arc<tokio::sync::Semaphore>,
}
impl Default for Previews {fn default()->Self{Self{context:None,entries:Default::default(),limiter:Arc::new(tokio::sync::Semaphore::new(1))}}}
impl Previews {
    pub fn finish(&mut self,token:Uuid,image:Option<crate::media::PreparedImage>){
        let Some(entry)=self.entries.values_mut().find(|entry|entry.token==token)else{return;};
        entry.job.take();entry.texture=image.map(crate::media::PreparedImage::texture);
        entry.views.retain(|(picture,fallback)|{
            let (Some(picture),Some(fallback))=(picture.upgrade(),fallback.upgrade())else{return false;};
            picture.set_paintable(entry.texture.as_ref());fallback.set_visible(entry.texture.is_none());true
        });
    }
    pub fn render(&mut self,root:&gtk::Box,files:&[UploadFile],context:Option<(Uuid,Option<Uuid>)>,enabled:bool,sending:bool,sender:&ComponentSender<ChatModel>){
        if self.context!=context||!enabled{self.entries.clear();self.context=context;}
        let keep:std::collections::HashSet<_>=files.iter().map(Key::from).collect();self.entries.retain(|key,_|keep.contains(key));
        for entry in self.entries.values_mut(){entry.views.clear();}
        while let Some(child)=root.first_child(){root.remove(&child);}
        for (index,file) in files.iter().enumerate(){
            let kind=Kind::of(file);let key=Key::from(file);
            if enabled&&kind!=Kind::File&&!self.entries.contains_key(&key){
                let token=Uuid::new_v4();let path=file.path.clone();let limiter=self.limiter.clone();let input=sender.input_sender().clone();
                let job=tokio::spawn(async move{
                    let Ok(permit)=limiter.acquire_owned().await else{return;};
                    let image=if kind==Kind::Video{
                        let result=crate::media::poster::extract(&path).await.ok();
                        result.and_then(|bytes|crate::media::PreparedImage::decode_thumbnail(&bytes,128,80))
                    }else{
                        // Keep the permit inside the blocking worker: aborting its
                        // waiter cannot start unbounded decoders on channel switches.
                        tokio::task::spawn_blocking(move ||{let _permit=permit;decode_image(&path)}).await.ok().flatten()
                    };
                    let _=input.send(ChatMsg::DraftPreviewReady{token,image});
                });
                self.entries.insert(key.clone(),Entry{token,texture:None,job:Some(job),views:Vec::new()});
            }
            let card=gtk::Box::new(gtk::Orientation::Vertical,4);card.set_widget_name(&format!("draft-attachment-{index}"));card.add_css_class("papo-draft-attachment");card.set_size_request(128,-1);card.set_valign(gtk::Align::Start);
            card.set_tooltip_text(Some(&format!("{} · {}",file.name,format_file_size(file.size as i64))));
            let preview=gtk::Overlay::new();preview.add_css_class("papo-draft-thumbnail");
            let picture=gtk::Picture::new();picture.set_size_request(128,80);picture.set_content_fit(gtk::ContentFit::Contain);picture.set_can_shrink(true);
            let fallback=gtk::Image::from_icon_name(kind.icon());fallback.set_pixel_size(32);fallback.set_halign(gtk::Align::Center);fallback.set_valign(gtk::Align::Center);
            if let Some(entry)=self.entries.get_mut(&key){picture.set_paintable(entry.texture.as_ref());fallback.set_visible(entry.texture.is_none());entry.views.push((picture.downgrade(),fallback.downgrade()));}
            preview.set_child(Some(&picture));preview.add_overlay(&fallback);
            if kind==Kind::Video{let play=gtk::Image::from_icon_name("media-playback-start-symbolic");play.set_halign(gtk::Align::Start);play.set_valign(gtk::Align::End);play.set_margin_start(6);play.set_margin_bottom(4);play.add_css_class("osd");preview.add_overlay(&play);}
            let remove=gtk::Button::from_icon_name("window-close-symbolic");remove.set_widget_name(&format!("remove-draft-attachment-{index}"));remove.add_css_class("circular");remove.add_css_class("osd");remove.set_halign(gtk::Align::End);remove.set_valign(gtk::Align::Start);remove.set_margin_end(4);remove.set_margin_top(4);remove.set_sensitive(!sending);
            remove.set_tooltip_text(Some(&format!("Remover {}",file.name)));let input=sender.input_sender().clone();remove.connect_clicked(move |_|{let _=input.send(ChatMsg::Transfer(TransferMsg::Remove(index)));});preview.add_overlay(&remove);card.append(&preview);
            let name=gtk::Label::new(Some(&file.name));name.set_max_width_chars(16);name.set_ellipsize(pango::EllipsizeMode::Middle);name.add_css_class("caption");name.set_margin_bottom(4);card.append(&name);root.append(&card);
        }
    }
}
fn decode_image(path:&Path)->Option<crate::media::PreparedImage>{
    // Read from disk without retaining a full encoded copy or a full-size texture.
    let reader=image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let (w,h)=reader.into_dimensions().ok()?;
    if w==0||h==0||w>16384||h>16384||u64::from(w)*u64::from(h)>32_000_000{return None;}
    let mut reader=image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut limits=image::Limits::default();limits.max_alloc=Some(128<<20);reader.limits(limits);
    let pixels=reader.decode().ok()?.thumbnail(128.min(w),80.min(h)).into_rgba8();
    Some(crate::media::PreparedImage{width:pixels.width(),height:pixels.height(),pixels:pixels.into_raw()})
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn local_image_preview_bounds_dimensions_and_rejects_invalid_files(){
        let file=crate::api::features::TemporaryFile(std::env::temp_dir().join(format!("papo-preview-{}.png",Uuid::new_v4())));
        for (width,height,expected) in [(640,320,(128,64)),(60,120,(40,80)),(4,2,(4,2))] {
            image::DynamicImage::new_rgba8(width,height).save(&file.0).unwrap();
            let image=decode_image(&file.0).unwrap();assert_eq!((image.width,image.height),expected);assert!(image.pixels.len()<=128*80*4);
        }
        std::fs::write(&file.0,b"not an image").unwrap();assert!(decode_image(&file.0).is_none());
        std::fs::remove_file(&file.0).unwrap();assert!(decode_image(&file.0).is_none());
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{mock,descendants,find_button,pump,until};use adw::prelude::*;
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let user="12345678-1234-4234-8234-123456789abc".parse().unwrap();let channel="12345678-1234-4234-8234-123456789abd".parse().unwrap();
    let target=|id|serde_json::from_value::<Channel>(serde_json::json!({"id":id,"name":"Attachment preview","created_at":chrono::Utc::now()})).unwrap();
    let mut sources=Vec::new();
    let mut source=|extension:&str|{let file=crate::api::features::TemporaryFile(std::env::temp_dir().join(format!("papo-preview-{}.{extension}",Uuid::new_v4())));let path=file.0.clone();sources.push(file);path};
    let png=source("png");image::DynamicImage::new_rgb8(640,320).save(&png).unwrap();
    let gif=source("gif");std::fs::write(&gif,crate::media::animation::fixture()).unwrap();
    let video=source("webm");std::fs::copy("tests/fixtures/chat-video.webm",&video).unwrap();
    let file=source("pdf");std::fs::write(&file,b"%PDF-1.4 fixture").unwrap();
    let broken=source("jpg");std::fs::write(&broken,b"not an image").unwrap();
    let files:Vec<_>=[&png,&gif,&video,&file,&broken,&png].into_iter().map(|path|tokio::runtime::Handle::current().block_on(UploadFile::inspect(path.clone())).unwrap()).collect();
    let original=std::fs::read(&png).unwrap();
    let chat=ChatModel::builder().launch(ChatInit{api:Some(api),active_channel:Some(target(channel)),user_id:Some(user),..Default::default()}).detach();
    chat.emit(ChatMsg::SetAccess{user_id:user,access:crate::models::Access::direct()});
    let window=adw::Window::builder().default_width(850).default_height(650).content(chat.widget()).build();window.present();performance::settle(context);
    let epoch=chat.model().transfers.epoch;
    chat.emit(ChatMsg::Transfer(TransferMsg::Selected{epoch,channel,result:Ok(files.clone())}));
    until(context,||chat.model().draft_previews.entries.len()==4&&chat.model().draft_previews.entries.values().all(|e|e.job.is_none()));performance::settle(context);
    let cached=chat.model().draft_previews.entries[&Key::from(&files[0])].texture.clone().unwrap();
    assert_eq!((cached.width(),cached.height()),(128,64));
    assert!(chat.model().draft_previews.entries[&Key::from(&files[2])].texture.is_some(),"video produces a real poster");
    assert!(chat.model().draft_previews.entries[&Key::from(&files[4])].texture.is_none(),"broken media keeps a removable fallback");
    let widgets=descendants(chat.widget().upcast_ref());
    let strip=widgets.iter().find(|w|w.widget_name()=="draft-attachments").unwrap();
    let pictures:Vec<_>=descendants(strip).into_iter().filter_map(|w|w.downcast::<gtk::Picture>().ok()).collect();
    assert_eq!(pictures.len(),6);assert_eq!(pictures.iter().filter(|p|p.paintable().is_some()).count(),4);
    assert!(!descendants(strip).iter().any(|w|w.is::<gtk::Video>()),"video previews never start playback");
    assert!(!backend.lock().unwrap().requests.iter().any(|(method,_,_)|method=="POST"),"thumbnails must not upload anything");
    for widget in descendants(strip).iter().filter(|w|w.has_css_class("papo-draft-attachment")){assert!(widget.height()<=140&&widget.width()<=144);}
    let card=descendants(strip).into_iter().find(|w|w.widget_name()=="draft-attachment-0").unwrap();
    composer::field(chat.widget().upcast_ref()).set_text("A caption");pump(context);assert!(descendants(strip).contains(&card),"typing does not rebuild the attachment strip");
    chat.emit(ChatMsg::SendClicked);until(context,||!chat.model().draft.is_sending()&&chat.model().draft.error.as_ref().is_some_and(|e|e.contains("Upload failed")));
    assert_eq!(chat.model().draft.files.len(),6);assert_eq!(chat.model().draft_previews.entries[&Key::from(&files[0])].texture.as_ref(),Some(&cached));
    find_button(chat.widget().upcast_ref(),&format!("Remover {}",files[3].name)).emit_clicked();until(context,||chat.model().draft.files.len()==5);
    assert_eq!(chat.model().draft.files[3].path,broken,"removal targets the selected attachment");
    crate::ui::main_window::layout::tests::preview(&window,"draft-attachment-previews",context);
    chat.emit(ChatMsg::SendClicked);until(context,||chat.model().draft.files.is_empty()&&!chat.model().draft.is_sending());
    assert!(chat.model().draft_previews.entries.is_empty());assert!(!strip.is_visible());assert_eq!(std::fs::read(&png).unwrap(),original,"previews never rewrite the original upload");
    // A queued local decode cannot populate a different conversation or a removed card.
    let permit=tokio::runtime::Handle::current().block_on(chat.model().draft_previews.limiter.clone().acquire_owned()).unwrap();
    chat.emit(ChatMsg::Transfer(TransferMsg::Selected{epoch,channel,result:Ok(vec![files[0].clone()])}));
    until(context,||chat.model().draft_previews.entries.len()==1);let token=chat.model().draft_previews.entries.values().next().unwrap().token;
    chat.emit(ChatMsg::SetChannel(target(Uuid::new_v4())));pump(context);assert!(chat.model().draft_previews.entries.is_empty());
    chat.emit(ChatMsg::DraftPreviewReady{token,image:Some(crate::media::PreparedImage{width:1,height:1,pixels:vec![255;4]})});pump(context);assert!(chat.model().draft.files.is_empty());
    drop(permit);chat.emit(ChatMsg::SetChannel(target(channel)));until(context,||chat.model().draft_previews.entries.values().any(|e|e.texture.is_some()));
    let token=chat.model().draft_previews.entries.values().next().unwrap().token;
    chat.emit(ChatMsg::Transfer(TransferMsg::Remove(0)));pump(context);
    chat.emit(ChatMsg::DraftPreviewReady{token,image:Some(crate::media::PreparedImage{width:1,height:1,pixels:vec![255;4]})});pump(context);
    assert!(chat.model().draft_previews.entries.is_empty());assert!(chat.model().draft.files.is_empty());
    window.set_content(None::<&gtk::Widget>);window.close();drop(chat);server.abort();drop(sources);pump(context);
}
