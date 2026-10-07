use serde::{Deserialize,Serialize};
use uuid::Uuid;
#[derive(Clone,Deserialize,Serialize)]
pub struct IceServer {pub urls:Vec<String>,pub username:Option<String>,pub credential:Option<String>}
impl std::fmt::Debug for IceServer{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("IceServer { credentials: [redacted] }")}}
#[derive(Debug,Clone,Deserialize)]
pub struct IceConfig {pub ice_servers:Vec<IceServer>}
#[derive(Debug,Clone,Deserialize,Serialize)]
pub struct VoiceMember {pub user_id:Uuid,pub muted:bool,pub camera_on:bool,pub screen_sharing:bool}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq,Eq)]
pub struct AudioRoute {pub track_id:String,pub user_id:Uuid}
#[derive(Clone,Deserialize,Serialize)]
#[serde(tag="type")]
pub enum VoiceEvent {
    #[serde(rename="voice_joined")]
    Joined {channel_id:Uuid,members:Vec<VoiceMember>,active_speakers:Vec<Uuid>},
    #[serde(rename="voice_answer")]
    Answer {channel_id:Uuid,sdp:String},
    #[serde(rename="voice_offer")]
    Offer {channel_id:Uuid,sdp:String},
    #[serde(rename="voice_ice_candidate")]
    Candidate {channel_id:Uuid,candidate:String,sdp_mid:Option<String>,sdp_mline_index:Option<u32>},
    #[serde(rename="voice_state_update")]
    State {channel_id:Uuid,#[serde(flatten)]member:VoiceMember},
    #[serde(rename="voice_leave")]
    Leave {channel_id:Uuid,user_id:Uuid},
    #[serde(rename="active_speaker_update")]
    Speakers {channel_id:Uuid,user_ids:Vec<Uuid>},
    #[serde(rename="voice_audio_routes")]
    Routes {channel_id:Uuid,routes:Vec<AudioRoute>},
}
impl VoiceEvent {pub fn channel(&self)->Uuid{match self{Self::Joined{channel_id,..}|Self::Answer{channel_id,..}|Self::Offer{channel_id,..}|Self::Candidate{channel_id,..}|Self::State{channel_id,..}|Self::Leave{channel_id,..}|Self::Speakers{channel_id,..}|Self::Routes{channel_id,..}=>*channel_id}}}
impl std::fmt::Debug for VoiceEvent{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("VoiceEvent").field("channel_id",&self.channel()).finish_non_exhaustive()}}
