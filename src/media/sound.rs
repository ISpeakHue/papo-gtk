//! Room-entry cue isolated from GTK's native player lifecycle.
use std::sync::{Arc,Mutex,atomic::{AtomicU32,Ordering}};
use tokio::{io::AsyncWriteExt,process::Command,task::JoinHandle};

#[derive(Default)]
pub struct CallCue {
    job:Option<JoinHandle<()>>,
    outcome:Arc<Mutex<Option<Result<(),String>>>>,
    pid:Arc<AtomicU32>,
    pub played:u64,
    #[cfg(test)]test_sink:bool,
}
impl CallCue {
    pub fn play(&mut self,enabled:bool){
        self.play_kind(enabled,false);
    }
    pub fn play_leave(&mut self,enabled:bool){self.play_kind(enabled,true);}
    fn play_kind(&mut self,enabled:bool,leave:bool){
        if !enabled{return;}
        self.stop();self.outcome=Default::default();self.pid=Default::default();
        let outcome=self.outcome.clone();let pid=self.pid.clone();
        #[cfg(test)]let test_sink=self.test_sink;
        #[cfg(not(test))]let test_sink=false;
        self.job=Some(tokio::spawn(async move{
            let result=play_cue(test_sink,pid,leave).await;
            if let Err(error)=&result{tracing::warn!("Call cue failed: {error}");}
            *outcome.lock().unwrap()=Some(result);
        }));self.played+=1;
    }
    pub fn stop(&mut self){if let Some(job)=self.job.take(){job.abort();}}
    #[cfg(test)]pub fn use_test_sink(&mut self){self.test_sink=true;}
}
impl Drop for CallCue{fn drop(&mut self){self.stop();}}

async fn play_cue(test_sink:bool,pid:Arc<AtomicU32>,leave:bool)->Result<(),String>{
    let mut command=Command::new("/usr/bin/python3");
    command.args(["-u","-c",include_str!("sound.py")]);if test_sink{command.arg("--test-sink");}
    command.kill_on_drop(true).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child=command.spawn().map_err(|e|e.to_string())?;pid.store(child.id().unwrap_or(0),Ordering::SeqCst);
    // Dropping a cancelled task kills and reaps the helper; GTK never waits on
    // a GStreamer pipeline being torn down while it is still preparing.
    let mut stdin=child.stdin.take().unwrap();stdin.write_all(if leave{include_bytes!("voice_leave.wav")}else{include_bytes!("voice_join.wav")}).await.map_err(|e|e.to_string())?;drop(stdin);
    let output=tokio::time::timeout(std::time::Duration::from_secs(8),child.wait_with_output()).await.map_err(|_|"Call cue process timed out".to_string())?.map_err(|e|e.to_string())?;
    if !output.status.success(){return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());}
    let report:serde_json::Value=serde_json::from_slice(&output.stdout).map_err(|e|e.to_string())?;
    if report["decoded_bytes"].as_u64().unwrap_or(0)==0{return Err("Call cue produced no audio".into());}Ok(())
}
#[cfg(test)]mod tests{
    #[test]fn departure_has_its_own_short_wave(){let leave=include_bytes!("voice_leave.wav");assert_eq!(&leave[..4],b"RIFF");assert_eq!(&leave[8..12],b"WAVE");assert_ne!(leave.as_slice(),include_bytes!("voice_join.wav").as_slice());assert!(leave.len()<14000);}

    #[test]fn call_chime_is_short_bounded_pcm_wave(){let wav=include_bytes!("voice_join.wav");assert_eq!(&wav[..4],b"RIFF");assert_eq!(&wav[8..12],b"WAVE");assert_eq!(u16::from_le_bytes([wav[22],wav[23]]),1);let rate=u32::from_le_bytes(wav[24..28].try_into().unwrap());let len=u32::from_le_bytes(wav[40..44].try_into().unwrap());assert_eq!(len as usize+44,wav.len());assert!(f64::from(len)/f64::from(rate*2)<0.3);}
}
#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::until;
    let mut cue=CallCue::default();cue.use_test_sink();cue.play(false);assert_eq!(cue.played,0);assert!(cue.job.is_none());
    cue.play(true);until(context,||cue.outcome.lock().unwrap().is_some());assert_eq!(cue.outcome.lock().unwrap().as_ref().unwrap(),&Ok(()));assert_eq!(cue.played,1);
    let pid=cue.pid.load(Ordering::SeqCst);assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists(),"completed cue must reap its process");
    cue.play_leave(true);until(context,||cue.outcome.lock().unwrap().is_some());assert_eq!(cue.outcome.lock().unwrap().as_ref().unwrap(),&Ok(()));
    cue.play(true);until(context,||cue.pid.load(Ordering::SeqCst)!=0);let pid=cue.pid.load(Ordering::SeqCst);cue.stop();until(context,||!std::path::Path::new(&format!("/proc/{pid}")).exists());
    for _ in 0..10{cue.play(true);cue.stop();}assert!(cue.job.is_none(),"rapid join/leave never blocks GTK");
}
