//! Short local call cue, played once after a successful connection.
use gtk::prelude::*;
#[derive(Default)]
pub struct CallCue { stream:Option<gtk::MediaFile>,pub played:u64 }
impl CallCue {
    pub fn play(&mut self,enabled:bool){
        if !enabled{return;}
        self.stop();let bytes=gtk::glib::Bytes::from_static(include_bytes!("voice_join.wav"));
        let input=gtk::gio::MemoryInputStream::from_bytes(&bytes);
        let stream=gtk::MediaFile::for_input_stream(&input);stream.set_volume(0.45);stream.play();
        self.stream=Some(stream);self.played+=1;
    }
    pub fn stop(&mut self){if let Some(stream)=self.stream.take(){stream.pause();stream.clear();}}
}
impl Drop for CallCue{fn drop(&mut self){self.stop();}}
#[cfg(test)]mod tests{
    #[test]fn call_chime_is_short_bounded_pcm_wave(){let wav=include_bytes!("voice_join.wav");assert_eq!(&wav[..4],b"RIFF");assert_eq!(&wav[8..12],b"WAVE");assert_eq!(u16::from_le_bytes([wav[22],wav[23]]),1);let rate=u32::from_le_bytes(wav[24..28].try_into().unwrap());let len=u32::from_le_bytes(wav[40..44].try_into().unwrap());assert_eq!(len as usize+44,wav.len());assert!(f64::from(len)/f64::from(rate*2)<0.3);}
}
#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::until;
    let mut cue=CallCue::default();cue.play(false);assert_eq!(cue.played,0);assert!(cue.stream.is_none());cue.play(true);let media=cue.stream.as_ref().unwrap().clone();media.set_muted(true);
    until(context,||media.is_prepared()||media.error().is_some());assert!(media.error().is_none(),"call cue must decode: {:?}",media.error());assert_eq!(cue.played,1);cue.stop();assert!(!media.is_playing()&&cue.stream.is_none());
}
