//! A single explicit subscription avoids guessing publisher mappings for the
//! backend's reusable video slots (the current protocol has no video-route ack).
use super::*;
use base64::Engine as _;
use serde_json::json;

#[derive(Debug)]
pub enum VideoMsg {
    Camera, Screen, Reload, Cameras{request:Uuid,result:anyhow::Result<Vec<Device>>},
    Watch{user:Uuid,kind:&'static str}, StopWatching,
}
pub(super) struct VideoControls {
    camera:gtk::Button, pub(super) screen:gtk::Button, devices:gtk::DropDown,
    list:Vec<Device>, request:Option<Uuid>, picture:gtk::Picture,
    pub(super) viewer_window:gtk::Window,viewer:gtk::Box, label:gtk::Label, choices:gtk::Box,
    sender:ComponentSender<MainWindowModel>,
    pub busy:bool, negotiated:bool, camera_on:bool, screen_on:bool,
    pub watching:Option<(Uuid,&'static str)>, epoch:u64, started:Instant,
    watch_started:Instant, last_frame:Option<Instant>, pending_subscribe:bool, problem:bool,
}
impl VideoControls {
    pub fn new(panel:&gtk::Box,dock:&gtk::Box,s:&ComponentSender<MainWindowModel>)->Self{
        let controls=wrapping_controls();
        let camera=gtk::Button::with_label("Ligar câmera");let screen=gtk::Button::from_icon_name("video-display-symbolic");screen.set_tooltip_text(Some("Compartilhar tela"));
        for (button,is_camera) in [(&camera,true),(&screen,false)]{let s=s.clone();button.connect_clicked(move |_|s.input(MainWindowMsg::Voice(VoiceMsg::Video(if is_camera{VideoMsg::Camera}else{VideoMsg::Screen}))));if is_camera{controls.insert(button,-1);}else{dock.append(button);}}
        panel.append(&controls);
        let devices=gtk::DropDown::from_strings(&["Nenhuma câmera disponível"]);panel.append(&devices);
        let refresh=gtk::Button::with_label("Atualizar câmeras");let s2=s.clone();refresh.connect_clicked(move |_|s2.input(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::Reload))));panel.append(&refresh);
        let viewer_window=gtk::Window::builder().title("Transmissões da chamada").default_width(800).default_height(560).hide_on_close(true).build();let streams=gtk::Box::new(gtk::Orientation::Vertical,8);streams.set_margin_top(12);streams.set_margin_bottom(12);streams.set_margin_start(12);streams.set_margin_end(12);viewer_window.set_child(Some(&streams));let s2=s.clone();viewer_window.connect_close_request(move |_|{let _=s2.input_sender().send(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::StopWatching)));gtk::glib::Propagation::Proceed});
        let choices=gtk::Box::new(gtk::Orientation::Vertical,3);let subscriptions=gtk::ScrolledWindow::builder().max_content_height(100).propagate_natural_height(true).child(&choices).build();streams.append(&subscriptions);
        let viewer=gtk::Box::new(gtk::Orientation::Vertical,4);viewer.set_visible(false);
        let label=gtk::Label::new(None);viewer.append(&label);
        let picture=gtk::Picture::new();picture.set_can_shrink(true);picture.set_vexpand(true);picture.set_hexpand(true);picture.set_size_request(240,135);viewer.set_vexpand(true);picture.set_content_fit(gtk::ContentFit::Contain);viewer.append(&picture);
        let stop=gtk::Button::with_label("Parar de assistir");let s2=s.clone();stop.connect_clicked(move |_|s2.input(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::StopWatching))));viewer.append(&stop);streams.append(&viewer);
        Self{camera,screen,devices,viewer_window,list:vec![],request:None,picture,viewer,label,choices,sender:s.clone(),busy:false,negotiated:false,camera_on:false,screen_on:false,watching:None,epoch:0,started:Instant::now(),watch_started:Instant::now(),last_frame:None,pending_subscribe:false,problem:false}
    }
    pub fn clear(&mut self){self.problem=false;self.busy=false;self.negotiated=false;self.camera_on=false;self.screen_on=false;self.watching=None;self.pending_subscribe=false;self.epoch+=1;self.last_frame=None;self.picture.set_paintable(None::<&gtk::gdk::Texture>);self.viewer.set_visible(false);self.viewer_window.set_visible(false);while let Some(child)=self.choices.first_child(){self.choices.remove(&child);}}
}
impl MainWindowModel {
    fn video_command(&mut self,value:serde_json::Value)->bool{
        let Some(c)=self.voice.call.as_ref() else{return false;};
        #[cfg(test)]if self.voice.test_engine{return true;}
        if c.engine.as_ref().is_some_and(|e|e.send(value)){true}else{self.stop_voice(true,"O processo de mídia não responde.");false}
    }
    pub(super) fn render_video(&self){
        let v=&self.voice.video;let connected=self.voice.call.as_ref().is_some_and(|c|c.phase==Phase::Connected);
        let ready=connected&&v.negotiated&&!v.busy;
        v.camera.set_sensitive(ready&&(v.camera_on||!v.list.is_empty()));v.screen.set_sensitive(ready);
        v.devices.set_sensitive(!v.camera_on&&!v.busy);
        v.camera.set_label(if v.camera_on{"Desligar câmera"}else{"Ligar câmera"});v.screen.set_tooltip_text(Some(if v.screen_on{"Parar compartilhamento"}else{"Compartilhar tela"}));
        let mut keep=HashSet::new();if let Some(call)=self.voice.call.as_ref().filter(|c|c.phase==Phase::Connected){for member in self.voice.members.get(&call.channel).into_iter().flat_map(|members|members.values()){if member.user_id==self.current_user.id{continue;}for (active,kind) in [(member.camera_on,"video"),(member.screen_sharing,"screen")]{if active{keep.insert(format!("watch-{}-{kind}",member.user_id));}}}}
        let mut child=v.choices.first_child();while let Some(w)=child{child=w.next_sibling();if !keep.contains(w.widget_name().as_str()){v.choices.remove(&w);}}
    }
    pub(super) fn video_member_buttons(&self,user:Uuid,state:Option<&VoiceMember>){
        if user==self.current_user.id||!self.voice.call.as_ref().is_some_and(|c|c.phase==Phase::Connected){return;}
        let Some(member)=state else{return;};
        for (active,kind,text) in [(member.camera_on,"video","Ver câmera"),(member.screen_sharing,"screen","Ver tela")]{
            if !active{continue;}
            let name=self.users.iter().find(|u|u.id==user).map(|u|u.display_name().to_owned()).unwrap_or_else(||user.to_string());
            let key=format!("watch-{user}-{kind}");let label=format!("{text} · {name}");let mut child=self.voice.video.choices.first_child();let mut existing=None;while let Some(w)=child{child=w.next_sibling();if w.widget_name()==key{existing=w.downcast::<gtk::Button>().ok();break;}}
            if let Some(button)=existing{button.set_label(&label);continue;}
            let button=gtk::Button::with_label(&label);button.set_widget_name(&key);let s=self.voice.video.sender.clone();button.connect_clicked(move |_|s.input(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::Watch{user,kind}))));self.voice.video.choices.append(&button);
        }
    }
    pub(super) fn video_event(&mut self,msg:VideoMsg,s:ComponentSender<Self>){
        match msg{
            VideoMsg::Reload=>{let request=Uuid::new_v4();self.voice.video.request=Some(request);self.voice.jobs.push(tokio::spawn(async move{let result=crate::voice::cameras().await;let _=s.input_sender().send(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::Cameras{request,result})));}));}
            VideoMsg::Cameras{request,result}=>{if self.voice.video.request!=Some(request){return;}self.voice.video.request=None;match result{Ok(list)=>{let v=&mut self.voice.video;let old=v.list.get(v.devices.selected() as usize).map(|d|d.id.clone());let mut labels:Vec<_>=list.iter().map(|d|d.name.as_str()).collect();if labels.is_empty(){labels.push("Nenhuma câmera disponível");}v.devices.set_model(Some(&gtk::StringList::new(&labels)));v.devices.set_selected(old.and_then(|id|list.iter().position(|d|d.id==id)).unwrap_or(0) as u32);v.list=list;},Err(e)=>self.voice.status.set_text(&e.to_string())}}
            VideoMsg::Camera|VideoMsg::Screen=>{
                let v=&self.voice.video;if v.busy||!v.negotiated||!self.voice.call.as_ref().is_some_and(|c|c.phase==Phase::Connected){return;}
                let camera=matches!(msg,VideoMsg::Camera);let on=if camera{!v.camera_on}else{!v.screen_on};let kind=if camera{"video"}else{"screen"};let device=v.list.get(v.devices.selected() as usize).map(|d|d.id.clone());
                if camera&&on&&device.is_none(){self.voice.status.set_text("Escolha uma câmera disponível.");return;}
                self.voice.video.problem=false;self.voice.video.busy=true;self.voice.video.started=Instant::now();
                self.voice.status.set_text(if !camera&&on{"Escolha a janela ou tela no seletor do desktop…"}else{"Atualizando vídeo…"});
                self.video_command(json!({"type":"media","kind":kind,"on":on,"device":device}));
            }
            VideoMsg::Watch{user,kind}=>{
                let Some(c)=self.voice.call.as_ref().filter(|c|c.phase==Phase::Connected) else{return;};
                if user==self.current_user.id||!self.voice.members.get(&c.channel).and_then(|m|m.get(&user)).is_some_and(|m|if kind=="video"{m.camera_on}else{m.screen_sharing}){return;}
                self.stop_video_watch();if self.voice.call.is_none(){return;}
                let v=&mut self.voice.video;v.epoch+=1;v.watching=Some((user,kind));v.pending_subscribe=true;v.watch_started=Instant::now();v.last_frame=None;
                let name=self.users.iter().find(|u|u.id==user).map(|u|u.display_name().to_owned()).unwrap_or_else(||user.to_string());v.label.set_text(&format!("{} · {name} · aguardando vídeo…",if kind=="video"{"Câmera"}else{"Tela"}));v.viewer.set_visible(true);if let Some(parent)=self.sidebar.widget().root().and_downcast::<gtk::Window>(){v.viewer_window.set_transient_for(Some(&parent));}v.viewer_window.present();let epoch=v.epoch;
                // Send subscribe only after the worker resets its decoder.
                self.video_command(json!({"type":"video_watch","enabled":true,"epoch":epoch}));
            }
            VideoMsg::StopWatching=>self.stop_video_watch(),
        }
        self.render_voice();
    }
    pub(super) fn stop_video_watch(&mut self){
        let v=&mut self.voice.video;let old=v.watching.take();v.epoch+=1;v.pending_subscribe=false;v.last_frame=None;v.picture.set_paintable(None::<&gtk::gdk::Texture>);v.viewer.set_visible(false);v.viewer_window.set_visible(false);let epoch=v.epoch;
        if let Some(c)=&self.voice.call{if let Some((user,kind))=old{if !self.voice_frame(c.connection,json!({"type":"track_unsubscribe","channel_id":c.channel,"publisher_id":user,"kind":kind})){self.stop_voice(true,"Não foi possível encerrar a assinatura de vídeo.");return;}}self.video_command(json!({"type":"video_watch","enabled":false,"epoch":epoch}));}
    }
    pub(super) fn sync_video_watch(&mut self){
        if let Some((user,kind))=self.voice.video.watching{let active=self.voice.call.as_ref().and_then(|c|self.voice.members.get(&c.channel)).and_then(|m|m.get(&user)).is_some_and(|m|if kind=="video"{m.camera_on}else{m.screen_sharing});if !active{self.stop_video_watch();}}
    }
    pub(super) fn video_tick(&mut self){
        if self.voice.video.busy&&self.voice.video.started.elapsed()>Duration::from_secs(135){self.stop_voice(true,"A negociação de vídeo demorou demais. Entre novamente.");return;}
        if self.voice.video.watching.is_some(){
            if self.voice.video.last_frame.is_some_and(|t|t.elapsed()>Duration::from_secs(3)){self.voice.video.picture.set_paintable(None::<&gtk::gdk::Texture>);}
            if self.voice.video.last_frame.is_none()&&self.voice.video.watch_started.elapsed()>Duration::from_secs(15){self.stop_video_watch();self.voice.status.set_text("A transmissão não enviou vídeo. Tente selecioná-la novamente.");}
        }
    }
    pub(super) fn video_engine(&mut self,event:EngineEvent){
        match event{
            EngineEvent::Negotiated=>{self.voice.video.negotiated=true;self.voice.video.busy=false;self.render_voice();}
            EngineEvent::MediaIntent{kind,on}=>{let Some(c)=&self.voice.call else{return;};let frame=if kind=="video"{json!({"type":"voice_camera","channel_id":c.channel,"on":on})}else{json!({"type":if on{"screen_share_start"}else{"screen_share_stop"},"channel_id":c.channel})};if !self.voice_frame(c.connection,frame){self.stop_voice(true,"Não foi possível atualizar o vídeo.");}}
            EngineEvent::MediaState{kind,on}=>{if kind=="video"{self.voice.video.camera_on=on;}else{self.voice.video.screen_on=on;}if !self.voice.video.problem{self.voice.status.set_text(if on{"Vídeo iniciado."}else{"Captura encerrada."});}self.render_voice();}
            EngineEvent::MediaError{kind,message}=>{self.voice.video.problem=true;if kind=="video"{self.voice.video.camera_on=false;}else{self.voice.video.screen_on=false;}self.voice.status.set_text(&message);self.render_voice();}
            EngineEvent::VideoReset{epoch}=>{
                if epoch==self.voice.video.epoch&&self.voice.video.pending_subscribe{self.voice.video.pending_subscribe=false;if let (Some(c),Some((user,kind)))=(&self.voice.call,self.voice.video.watching){if !self.voice_frame(c.connection,json!({"type":"track_subscribe","channel_id":c.channel,"publisher_id":user,"kind":kind})){self.stop_voice(true,"Não foi possível solicitar o vídeo.");}}}
            }
            EngineEvent::VideoFrame{track_id,epoch,jpeg}=>{
                // One outstanding frame per worker; acknowledge even stale frames.
                if epoch==self.voice.video.epoch&&self.voice.video.watching.is_some()&&track_id=="papo-video-0"&&jpeg.len()<=120000{
                    if let Ok(bytes)=base64::engine::general_purpose::STANDARD.decode(jpeg){if let Ok(texture)=gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_owned(bytes)){self.voice.video.picture.set_paintable(Some(&texture));self.voice.video.last_frame=Some(Instant::now());let label=self.voice.video.label.text();self.voice.video.label.set_text(label.trim_end_matches(" · aguardando vídeo…"));}}
                }
                self.video_command(json!({"type":"frame_ack"}));
            }
            _=>{}
        }
    }
}

#[cfg(test)]
pub(super) fn exercise(main:&Controller<MainWindowModel>,context:&gtk::glib::MainContext,call:Uuid,peer:Uuid){
    use crate::ui::chat::actions::tests::{pump,find_button};
    let engine=|event|{main.emit(MainWindowMsg::Voice(VoiceMsg::Engine{call,event}));pump(context);};
    let room=main.model().voice.call.as_ref().unwrap().channel;
    engine(EngineEvent::Negotiated);
    assert!(main.model().voice.video.screen.is_sensitive());
    main.model().voice.video.screen.emit_clicked();pump(context);
    assert!(main.model().voice.video.busy);
    assert!(!main.model().voice.video.screen.is_sensitive());
    engine(EngineEvent::MediaIntent{kind:"screen".into(),on:true});
    engine(EngineEvent::MediaState{kind:"screen".into(),on:true});
    engine(EngineEvent::Negotiated);
    assert_eq!(main.model().voice.video.screen.tooltip_text().unwrap(),"Parar compartilhamento");
    main.model().voice.video.screen.emit_clicked();pump(context);
    engine(EngineEvent::MediaState{kind:"screen".into(),on:false});engine(EngineEvent::Negotiated);
    // Cancellation is recoverable and keeps the call/microphone alive.
    main.model().voice.video.screen.emit_clicked();pump(context);
    engine(EngineEvent::MediaError{kind:"screen".into(),message:"Cancelado".into()});
    engine(EngineEvent::MediaState{kind:"screen".into(),on:false});engine(EngineEvent::Negotiated);
    assert!(main.model().voice.call.is_some());assert!(main.model().voice.video.screen.is_sensitive());
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::State{channel_id:room,member:VoiceMember{user_id:peer,muted:false,camera_on:true,screen_sharing:true}})));pump(context);
    find_button(main.model().sidebar.widget().upcast_ref(),"Ver câmera").emit_clicked();pump(context);assert!(main.model().voice.video.viewer_window.is_visible());
    let epoch=main.model().voice.video.epoch;engine(EngineEvent::VideoReset{epoch});
    let mut jpeg=vec![];image::codecs::jpeg::JpegEncoder::new(&mut jpeg).encode(&[255,0,0],1,1,image::ExtendedColorType::Rgb8).unwrap();let jpeg=base64::engine::general_purpose::STANDARD.encode(jpeg);
    engine(EngineEvent::VideoFrame{track_id:"papo-video-0".into(),epoch,jpeg:jpeg.clone()});assert!(main.model().voice.video.picture.paintable().is_some());
    find_button(main.model().voice.video.choices.upcast_ref(),"Ver tela · Bob").emit_clicked();pump(context);
    assert!(main.model().voice.video.picture.paintable().is_none());
    engine(EngineEvent::VideoFrame{track_id:"papo-video-0".into(),epoch,jpeg:jpeg.clone()});assert!(main.model().voice.video.picture.paintable().is_none(),"frames queued before switching must be discarded");
    let epoch=main.model().voice.video.epoch;engine(EngineEvent::VideoReset{epoch});engine(EngineEvent::VideoFrame{track_id:"papo-video-0".into(),epoch,jpeg});assert!(main.model().voice.video.picture.paintable().is_some());
    main.emit(MainWindowMsg::WsReceived(WsEvent::Voice(VoiceEvent::Leave{channel_id:room,user_id:peer})));pump(context);
    assert!(main.model().voice.video.watching.is_none());assert!(main.model().voice.video.picture.paintable().is_none());assert!(!main.model().voice.video.viewer_window.is_visible());
    assert!(main.model().voice.call.is_some());
    main.emit(MainWindowMsg::WsReceived(WsEvent::Error{message:"Publisher disappeared".into(),code:Some("voice-not-found".into())}));pump(context);
    assert!(main.model().voice.call.is_some(),"a late subscription error after publisher removal must preserve audio");
}
