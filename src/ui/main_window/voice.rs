//! Voice controls share the chat WebSocket but own their media lifetime.
use super::*;
use crate::models::{VoiceEvent,VoiceMember,IceConfig,ChannelType};
use crate::voice::{Engine,EngineEvent,Device};
use std::collections::{HashMap,HashSet};
pub(super) mod video;
use video::{VideoControls,VideoMsg};
#[derive(Debug)]
pub enum VoiceMsg {#[cfg(test)]TestEngine,Video(VideoMsg),Select(Uuid),Join,Leave,Close,Settings,Mute,Device,ReloadDevices,
    Devices{request:Uuid,result:anyhow::Result<Vec<Device>>},Ice{call:Uuid,result:anyhow::Result<IceConfig>},Engine{call:Uuid,event:EngineEvent}}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum Phase {Ice,Joining,Negotiating,Connected}
struct Call {id:Uuid,channel:Uuid,connection:Uuid,phase:Phase,started:Instant,ice:Option<IceConfig>,engine:Option<Engine>,muted:bool}
pub(super) struct Voice {
    pub panel:gtk::Box,status:gtk::Label,title:gtk::Label,join:gtk::Button,leave:gtk::Button,mute:gtk::ToggleButton,settings:gtk::Window,settings_button:gtk::Button,
    devices:gtk::DropDown,device_list:Vec<Device>,device:Option<String>,device_request:Option<Uuid>,
    pub(super) connection:Option<Uuid>,selected:Option<Uuid>,call:Option<Call>,pending_leave:Option<(Uuid,Uuid,Instant)>,
    members:HashMap<Uuid,HashMap<Uuid,VoiceMember>>,speakers:HashMap<Uuid,HashSet<Uuid>>,pub(super) presence:HashMap<Uuid,Vec<Uuid>>,routes:HashMap<String,Uuid>,
    jobs:Vec<tokio::task::JoinHandle<()>>,
    cue:crate::media::sound::CallCue,video:VideoControls,local_speaking:bool,
    #[cfg(test)]test_engine:bool,
}
impl Drop for Voice{fn drop(&mut self){self.settings.close();self.video.clear();self.video.viewer_window.close();self.call=None;for j in self.jobs.drain(..){j.abort();}}}
fn wrapping_controls()->gtk::FlowBox{
    let controls=gtk::FlowBox::new();controls.set_selection_mode(gtk::SelectionMode::None);controls.set_min_children_per_line(1);controls.set_max_children_per_line(4);controls.set_column_spacing(6);controls.set_row_spacing(6);controls.set_homogeneous(false);controls
}
impl Voice {
    pub fn new(s:&ComponentSender<MainWindowModel>)->Self{
        let panel=gtk::Box::new(gtk::Orientation::Vertical,4);panel.add_css_class("papo-voice-dock");panel.set_visible(false);
        let heading=gtk::Box::new(gtk::Orientation::Horizontal,4);let title=gtk::Label::new(None);title.set_xalign(0.0);title.set_hexpand(true);title.set_ellipsize(gtk::pango::EllipsizeMode::End);title.add_css_class("caption-heading");heading.append(&title);panel.append(&heading);
        fn button(icon:&str,label:&str,s:&ComponentSender<MainWindowModel>,f:impl Fn()->VoiceMsg+'static)->gtk::Button{let w=if icon.is_empty(){gtk::Button::with_label(label)}else{gtk::Button::from_icon_name(icon)};w.set_tooltip_text(Some(label));let s=s.clone();w.connect_clicked(move |_|s.input(MainWindowMsg::Voice(f())));w}
        let close=button("window-close-symbolic","Fechar voz",s,||VoiceMsg::Close);close.add_css_class("flat");heading.append(&close);
        let controls=gtk::Box::new(gtk::Orientation::Horizontal,4);panel.append(&controls);
        let join=button("","Entrar na voz",s,||VoiceMsg::Join);join.add_css_class("suggested-action");join.set_hexpand(true);controls.append(&join);
        let mute=gtk::ToggleButton::new();mute.set_icon_name("microphone-disabled-symbolic");mute.set_tooltip_text(Some("Microfone silenciado"));mute.set_active(true);let s2=s.clone();mute.connect_toggled(move |_|s2.input(MainWindowMsg::Voice(VoiceMsg::Mute)));controls.append(&mute);
        let settings_button=button("emblem-system-symbolic","Configurar chamada",s,||VoiceMsg::Settings);settings_button.add_css_class("flat");controls.append(&settings_button);
        let leave=button("call-stop-symbolic","Sair da voz",s,||VoiceMsg::Leave);leave.add_css_class("flat");controls.append(&leave);
        let status=gtk::Label::new(Some("Microfone inicialmente silenciado"));status.set_xalign(0.0);status.set_ellipsize(gtk::pango::EllipsizeMode::End);status.add_css_class("caption");status.add_css_class("dim-label");panel.append(&status);
        let settings=gtk::Window::builder().title("Configurar chamada").default_width(480).default_height(480).hide_on_close(true).build();let body=gtk::Box::new(gtk::Orientation::Vertical,12);body.set_margin_top(16);body.set_margin_bottom(16);body.set_margin_start(16);body.set_margin_end(16);settings.set_child(Some(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&body).build()));
        let info=gtk::Label::new(Some("O microfone começa silenciado. Trocar o dispositivo encerra a chamada para liberar o microfone anterior."));info.set_wrap(true);info.set_xalign(0.0);body.append(&info);let label=gtk::Label::new(Some("Microfone"));label.set_xalign(0.0);body.append(&label);
        let devices=gtk::DropDown::from_strings(&["Microfone padrão do sistema"]);let s2=s.clone();devices.connect_selected_notify(move |_|s2.input(MainWindowMsg::Voice(VoiceMsg::Device)));body.append(&devices);body.append(&button("","Atualizar microfones",s,||VoiceMsg::ReloadDevices));
        let video=VideoControls::new(&body,&controls,s);
        Self{local_speaking:false,cue:Default::default(),video,panel,title,status,join,leave,mute,settings,settings_button,devices,device_list:vec![],device:None,device_request:None,connection:None,selected:None,call:None,pending_leave:None,members:Default::default(),speakers:Default::default(),presence:Default::default(),routes:Default::default(),jobs:vec![],#[cfg(test)]test_engine:false}

    }
}
impl MainWindowModel {
    fn voice_frame(&self,connection:Uuid,value:serde_json::Value)->bool{
        self.voice.connection==Some(connection)&&self.ws_tx.try_send(ws::WsCommand::Voice{connection,text:value.to_string()}).is_ok()
    }
    pub(super) fn stop_voice(&mut self,leave:bool,message:&str){
        let had_call=self.voice.call.is_some();let mut joined=false;
        if let Some(call)=self.voice.call.take(){
            joined=matches!(call.phase,Phase::Negotiating|Phase::Connected);
            // Drop capture and transports before any further UI or network work.
            let channel=call.channel;let connection=call.connection;drop(call);
            self.voice.routes.clear();
            if leave&&self.voice_frame(connection,serde_json::json!({"type":"voice_leave","channel_id":channel})){self.voice.pending_leave=Some((channel,connection,Instant::now()));}
        }
        self.voice.local_speaking=false;
        if joined{self.voice.cue.play_leave(self.account.config.notifications.as_ref().and_then(|n|n.sound).unwrap_or(true));}else if had_call{self.voice.cue.stop();}self.voice.video.clear();self.voice.status.set_text(message);self.render_voice();
    }
    pub(super) fn voice_disconnected(&mut self){self.voice.connection=None;self.voice.pending_leave=None;self.voice.members.clear();self.voice.speakers.clear();self.voice.presence.clear();self.stop_voice(false,"Conexão encerrada. Entre na voz novamente após reconectar.");}
    pub(super) fn publish_voice_channels(&self){
        let mut channels=self.channels.clone();channels.retain(|c|c.channel_type!=Some(ChannelType::Voice)||self.voice_allowed(c.id));channels.extend(self.managed_channels.iter().filter(|c|self.voice_allowed(c.id)&&!channels.iter().any(|item|item.id==c.id)).cloned().collect::<Vec<_>>());channels.sort_by_key(|c|c.position.unwrap_or(0));self.sidebar.emit(SidebarMsg::SetChannels(channels));
    }
    pub(super) fn sync_voice_access(&mut self){
        if self.voice.call.as_ref().is_some_and(|c|!self.voice_allowed(c.channel)){self.stop_voice(true,"Acesso à voz revogado ou canal excluído.");}
        let allowed:HashSet<_>=self.managed_channels.iter().filter(|c|self.voice_allowed(c.id)).map(|c|c.id).collect();
        self.voice.members.retain(|id,_|allowed.contains(id));self.voice.speakers.retain(|id,_|allowed.contains(id));
        if self.voice.selected.is_some_and(|id|!allowed.contains(&id)){self.voice.selected=None;self.voice.settings.set_visible(false);}
        self.render_voice();
    }
    pub(super) fn voice_allowed(&self,id:Uuid)->bool{self.access.get(&id).is_some_and(|a|a.voice)&&self.managed_channels.iter().any(|c|c.id==id&&c.channel_type==Some(ChannelType::Voice))}
    pub(super) fn render_voice(&self){
        let rooms=self.managed_channels.iter().filter(|c|self.voice_allowed(c.id)).map(|channel|{
            let mut ids:HashSet<_>=self.voice.members.get(&channel.id).into_iter().flat_map(|m|m.keys().copied()).collect();ids.extend(self.voice.presence.iter().filter(|(_,channels)|channels.contains(&channel.id)).map(|(id,_)|*id));if self.voice.call.as_ref().is_some_and(|c|c.channel==channel.id){ids.insert(self.current_user.id);}let mut ids:Vec<_>=ids.into_iter().collect();ids.sort();
            crate::ui::sidebar::VoiceRoom{id:channel.id,members:ids.into_iter().map(|id|{let state=self.voice.members.get(&channel.id).and_then(|m|m.get(&id));crate::ui::sidebar::VoiceParticipant{id,name:self.users.iter().find(|u|u.id==id).map(|u|u.display_name().to_owned()).unwrap_or_else(||id.to_string()),muted:if id==self.current_user.id{self.voice.call.as_ref().filter(|c|c.channel==channel.id).map_or(state.is_some_and(|m|m.muted),|c|c.muted)}else{state.is_some_and(|m|m.muted)},speaking:if id==self.current_user.id&&self.voice.call.as_ref().is_some_and(|c|c.channel==channel.id){self.voice.local_speaking&&self.voice.call.as_ref().is_some_and(|c|!c.muted)}else{self.voice.speakers.get(&channel.id).is_some_and(|s|s.contains(&id))},camera:id!=self.current_user.id&&state.is_some_and(|m|m.camera_on)&&self.voice.call.as_ref().is_some_and(|c|c.channel==channel.id&&c.phase==Phase::Connected),screen:id!=self.current_user.id&&state.is_some_and(|m|m.screen_sharing)&&self.voice.call.as_ref().is_some_and(|c|c.channel==channel.id&&c.phase==Phase::Connected)}}).collect()}
        }).collect();self.sidebar.emit(SidebarMsg::VoiceRooms(rooms));
        let id=self.voice.call.as_ref().map(|c|c.channel).or(self.voice.selected);
        let Some(id)=id else{self.voice.panel.set_visible(false);return;};
        self.render_video();
        self.voice.title.set_text(&format!("Voz · {}",self.managed_channels.iter().find(|c|c.id==id).map(|c|c.name.as_str()).unwrap_or("Canal indisponível")));
        self.voice.join.set_sensitive(self.voice_allowed(id)&&self.voice.connection.is_some()&&self.voice.call.is_none()&&self.voice.pending_leave.is_none());
        self.voice.panel.set_visible(true);self.voice.join.set_visible(self.voice.call.is_none());self.voice.leave.set_visible(self.voice.call.is_some());self.voice.mute.set_visible(self.voice.call.is_some());self.voice.video.screen.set_visible(self.voice.call.is_some());self.voice.settings_button.set_sensitive(self.voice_allowed(id));
        self.voice.leave.set_sensitive(self.voice.call.is_some());self.voice.mute.set_sensitive(self.voice.call.as_ref().is_some_and(|c|matches!(c.phase,Phase::Negotiating|Phase::Connected)));
        if let Some(call)=&self.voice.call{if self.voice.mute.is_active()!=call.muted{self.voice.mute.set_active(call.muted);}}
        self.voice.mute.set_icon_name(if self.voice.mute.is_active(){"microphone-disabled-symbolic"}else{"microphone-sensitivity-high-symbolic"});
        self.voice.mute.set_tooltip_text(Some(if self.voice.mute.is_active(){"Ativar microfone"}else{"Silenciar microfone"}));
        self.voice.status.set_tooltip_text(Some(&self.voice.status.text()));
        let mut ids:HashSet<_>=self.voice.members.get(&id).into_iter().flat_map(|m|m.keys().copied()).collect();
        ids.extend(self.voice.presence.iter().filter(|(_,channels)|channels.contains(&id)).map(|(user,_)|*user));
        let mut ids:Vec<_>=ids.into_iter().collect();ids.sort();
        for user in ids{let state=self.voice.members.get(&id).and_then(|m|m.get(&user));
            self.video_member_buttons(user,state);}
    }
    pub(super) fn voice_presence(&mut self,p:&ws::PresenceEntry){if p.online(){self.voice.presence.insert(p.user_id,p.user_voice.clone());}else{self.voice.presence.remove(&p.user_id);for members in self.voice.members.values_mut(){members.remove(&p.user_id);}}self.sync_video_watch();self.render_voice();}
    pub(super) fn voice_tick(&mut self){
        if self.voice.call.as_ref().is_some_and(|c|c.phase!=Phase::Connected&&c.started.elapsed()>Duration::from_secs(45)){self.stop_voice(true,"A conexão de voz demorou demais. Tente novamente.");}
        self.video_tick();
        if self.voice.pending_leave.is_some_and(|(_,_,t)|t.elapsed()>Duration::from_secs(3)){self.voice.pending_leave=None;self.render_voice();}
    }
    pub(super) fn voice_error(&mut self,message:&str,code:Option<&str>)->bool{
        let recognized=code.is_some_and(|c|c.starts_with("voice-"))||self.voice.call.as_ref().is_some_and(|c|c.phase==Phase::Joining)&&matches!(message,"voz indisponível"|"canal não encontrado"|"evento inválido");
        if recognized && self.voice.call.as_ref().is_some_and(|c|c.phase==Phase::Connected) && !self.voice.video.busy && matches!(code,Some("voice-not-found"|"voice-room-full"|"voice-rate-limited")) {
            self.stop_video_watch();self.voice.status.set_text("Vídeo indisponível. Escolha uma transmissão novamente.");return true;
        }
        if recognized&&self.voice.call.is_some(){let status=match code{Some("voice-room-full")=>"A sala de voz está cheia.",Some("voice-forbidden")=>"Sem permissão para entrar na voz.",Some("voice-already-in-room")=>"Sua conta já participa desta sala em outra conexão.",_=>"Não foi possível continuar a chamada de voz."};let leave=!matches!(code,Some("voice-already-in-room"));self.stop_voice(leave,status);return true;}recognized
    }
    pub(super) fn voice_event(&mut self,msg:VoiceMsg,s:ComponentSender<Self>){
        self.voice.jobs.retain(|j|!j.is_finished());
        match msg {
            VoiceMsg::Video(msg)=>self.video_event(msg,s),
            #[cfg(test)]VoiceMsg::TestEngine=>{self.voice.test_engine=true;self.voice.cue.use_test_sink();},
            VoiceMsg::Select(id)=>{if !self.voice_allowed(id){return;}if self.voice.call.as_ref().is_some_and(|c|c.channel!=id){self.voice.status.set_text("Saia da chamada atual antes de escolher outra sala.");return;}self.voice.selected=Some(id);self.voice.panel.set_visible(true);self.render_voice();s.input(MainWindowMsg::Voice(VoiceMsg::ReloadDevices));s.input(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::Reload)));}
            VoiceMsg::Settings=>{if let Some(parent)=self.sidebar.widget().root().and_downcast::<gtk::Window>(){self.voice.settings.set_transient_for(Some(&parent));}self.voice.settings.present();s.input(MainWindowMsg::Voice(VoiceMsg::ReloadDevices));s.input(MainWindowMsg::Voice(VoiceMsg::Video(VideoMsg::Reload)));},
            VoiceMsg::ReloadDevices=>{let request=Uuid::new_v4();self.voice.device_request=Some(request);self.voice.jobs.push(tokio::spawn(async move{let result=crate::voice::devices().await;let _=s.input_sender().send(MainWindowMsg::Voice(VoiceMsg::Devices{request,result}));}));}
            VoiceMsg::Devices{request,result}=>{if self.voice.device_request!=Some(request){return;}self.voice.device_request=None;match result{Ok(devices)=>{let previous=self.voice.device.clone();let mut labels=vec!["Microfone padrão do sistema".to_owned()];labels.extend(devices.iter().map(|d|d.name.clone()));self.voice.device_list=devices;self.voice.devices.set_model(Some(&gtk::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>())));let selected=previous.and_then(|id|self.voice.device_list.iter().position(|d|d.id==id)).map(|i|i as u32+1).unwrap_or(0);self.voice.devices.set_selected(selected);},Err(e)=>self.voice.status.set_text(&e.to_string())}}
            VoiceMsg::Device=>{let device=self.voice.devices.selected().checked_sub(1).and_then(|i|self.voice.device_list.get(i as usize)).map(|d|d.id.clone());if device!=self.voice.device{self.voice.device=device;if self.voice.call.is_some(){self.stop_voice(true,"Microfone alterado. Entre novamente para usá-lo.");}}}
            VoiceMsg::Join=>{
                if self.voice.call.is_some()||self.voice.pending_leave.is_some(){return;}let(Some(channel),Some(connection))=(self.voice.selected,self.voice.connection)else{return;};if !self.voice_allowed(channel){return;}
                let id=Uuid::new_v4();self.voice.routes.clear();self.voice.call=Some(Call{id,channel,connection,phase:Phase::Ice,started:Instant::now(),ice:None,engine:None,muted:true});self.voice.status.set_text("Consultando servidores ICE…");self.render_voice();let api=self.api_client.clone();
                self.voice.jobs.push(tokio::spawn(async move{let result=api.voice_ice_servers().await;s.input(MainWindowMsg::Voice(VoiceMsg::Ice{call:id,result}));}));
            }
            VoiceMsg::Ice{call,result}=>{
                if !self.voice.call.as_ref().is_some_and(|c|c.id==call&&c.phase==Phase::Ice){return;}
                match result{Ok(ice)=>{let c=self.voice.call.as_mut().unwrap();c.ice=Some(ice);c.phase=Phase::Joining;let connection=c.connection;let channel=c.channel;if !self.voice_frame(connection,serde_json::json!({"type":"voice_join","channel_id":channel})){self.stop_voice(false,"Sem conexão para entrar na voz.");}else{self.voice.status.set_text("Entrando na sala…");}},Err(e)=>{self.stop_voice(false,&e.to_string());s.input(MainWindowMsg::ActionError(e));}}
            }
            VoiceMsg::Leave=>self.stop_voice(true,"Você saiu da voz."),
            VoiceMsg::Close=>{self.stop_voice(true,"Você saiu da voz.");self.voice.selected=None;self.voice.panel.set_visible(false);},
            VoiceMsg::Mute=>{
                let Some(c)=self.voice.call.as_ref().filter(|c|matches!(c.phase,Phase::Negotiating|Phase::Connected))else{return;};let muted=self.voice.mute.is_active();if muted==c.muted{return;}
                if !self.voice_frame(c.connection,serde_json::json!({"type":"voice_mute","channel_id":c.channel,"muted":muted})){self.stop_voice(true,"Sem conexão para alterar o microfone.");return;}
                if muted{self.voice.local_speaking=false;}let c=self.voice.call.as_mut().unwrap();c.muted=muted;if let Some(engine)=&c.engine{if !engine.send(serde_json::json!({"type":"mute","muted":muted})){self.stop_voice(true,"O processo de áudio não responde.");}}self.render_voice();
            }
            VoiceMsg::Engine{call,event}=>{
                let Some(c)=self.voice.call.as_ref().filter(|c|c.id==call)else{return;};let channel=c.channel;let connection=c.connection;
                let frame=match event{EngineEvent::Speaking{active}=>{self.voice.local_speaking=active&&self.voice.call.as_ref().is_some_and(|c|!c.muted);self.render_voice();None},EngineEvent::Offer{sdp}=>Some(serde_json::json!({"type":"voice_offer","channel_id":channel,"sdp":sdp})),EngineEvent::Answer{sdp}=>Some(serde_json::json!({"type":"voice_answer","channel_id":channel,"sdp":sdp})),EngineEvent::Candidate{candidate,sdp_mid,sdp_mline_index}=>Some(serde_json::json!({"type":"voice_ice_candidate","channel_id":channel,"candidate":candidate,"sdp_mid":sdp_mid,"sdp_mline_index":sdp_mline_index})),EngineEvent::Connection{state}=>{if state=="connected"{self.voice.call.as_mut().unwrap().phase=Phase::Connected;self.voice.status.set_text("Voz conectada.");self.render_voice();}None},EngineEvent::Track{..}|EngineEvent::Started{..}=>None,event @ (EngineEvent::Negotiated|EngineEvent::MediaIntent{..}|EngineEvent::MediaState{..}|EngineEvent::MediaError{..}|EngineEvent::VideoFrame{..}|EngineEvent::VideoReset{..})=>{self.video_engine(event);None},EngineEvent::Error{message}=>{self.stop_voice(true,&message);None}};
                if let Some(frame)=frame{if !self.voice_frame(connection,frame){self.stop_voice(true,"Não foi possível enviar a sinalização de voz.");}}
            }
        }
    }
    pub(super) fn voice_received(&mut self,event:VoiceEvent,s:ComponentSender<Self>){
        let channel=event.channel();if !self.voice_allowed(channel){return;}
        let ours=self.voice.call.as_ref().is_some_and(|c|c.channel==channel);
        match event {
            VoiceEvent::Joined{members,active_speakers,..}=>{
                if !self.voice.call.as_ref().is_some_and(|c|c.channel==channel&&c.phase==Phase::Joining){return;}
                self.voice.members.insert(channel,members.into_iter().map(|m|(m.user_id,m)).collect());self.voice.speakers.insert(channel,active_speakers.into_iter().collect());
                let c=self.voice.call.as_mut().unwrap();c.phase=Phase::Negotiating;let call=c.id;let ice=c.ice.take().unwrap();let config=serde_json::json!({"type":"start","ice_servers":ice.ice_servers,"device":self.voice.device,"muted":true});
                #[cfg(test)]let start=!self.voice.test_engine;#[cfg(not(test))]let start=true;
                if start{c.engine=Some(Engine::start(config,move|event|s.input(MainWindowMsg::Voice(VoiceMsg::Engine{call,event}))));}
                let enabled=self.account.config.notifications.as_ref().and_then(|n|n.sound).unwrap_or(true);
                self.voice.cue.play(enabled);
                self.voice.status.set_text("Conectando áudio · microfone silenciado…");
            }
            VoiceEvent::Answer{..}|VoiceEvent::Offer{..}|VoiceEvent::Candidate{..}=>{if ours{if let Some(engine)=self.voice.call.as_ref().and_then(|c|c.engine.as_ref()){let value=serde_json::to_value(event).unwrap();if !engine.send(value){self.stop_voice(true,"O processo de áudio não responde.");}}}}
            VoiceEvent::State{member,..}=>{if member.user_id==self.current_user.id&&ours{if member.muted{self.voice.local_speaking=false;}let c=self.voice.call.as_mut().unwrap();c.muted=member.muted;if let Some(engine)=&c.engine{if !engine.send(serde_json::json!({"type":"mute","muted":member.muted})){self.stop_voice(true,"O processo de áudio não responde.");}}}self.voice.members.entry(channel).or_default().insert(member.user_id,member);}
            VoiceEvent::Leave{user_id,..}=>{
                if let Some(m)=self.voice.members.get_mut(&channel){m.remove(&user_id);}if let Some(s)=self.voice.speakers.get_mut(&channel){s.remove(&user_id);}if let Some(channels)=self.voice.presence.get_mut(&user_id){channels.retain(|id|*id!=channel);}
                if user_id==self.current_user.id{if self.voice.pending_leave.is_some_and(|(id,_,_)|id==channel){self.voice.pending_leave=None;}else if ours{self.stop_voice(false,"A participação na voz foi encerrada.");}}
            }
            VoiceEvent::Speakers{user_ids,..}=>{self.voice.speakers.insert(channel,user_ids.into_iter().collect());}
            VoiceEvent::Routes{routes,..}=>{if ours{self.voice.routes=routes.iter().map(|r|(r.track_id.clone(),r.user_id)).collect();if let Some(engine)=self.voice.call.as_ref().and_then(|c|c.engine.as_ref()){if !engine.send(serde_json::json!({"type":"voice_audio_routes","routes":routes})){self.stop_voice(true,"O processo de áudio não responde.");}}}}
        }
        self.sync_video_watch();self.render_voice();
    }
}

#[cfg(test)]
pub(crate) mod tests;
