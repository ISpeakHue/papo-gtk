//! EN-US: Room-entry cue isolated from GTK's native player lifecycle.
//! PT-BR: Som de entrada na chamada isolado do ciclo de vida do reprodutor nativo do GTK.
use std::sync::{Arc,Mutex,atomic::{AtomicU32,Ordering}};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},sync::oneshot,task::JoinHandle};

#[derive(Default)]
pub struct CallCue {
    job:Option<JoinHandle<()>>,
    cancel:Option<oneshot::Sender<()>>,
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
        let (cancel,cancelled)=oneshot::channel();self.cancel=Some(cancel);
        self.job=Some(tokio::spawn(async move{
            let result=play_cue(test_sink,pid,leave,cancelled).await;
            if let Err(error)=&result{tracing::warn!("Call cue failed: {error}");}
            *outcome.lock().unwrap()=Some(result);
        }));self.played+=1;
    }
    pub fn stop(&mut self){if let Some(cancel)=self.cancel.take(){let _=cancel.send(());}self.job.take();}
    #[cfg(test)]pub fn use_test_sink(&mut self){self.test_sink=true;}
}
impl Drop for CallCue{fn drop(&mut self){self.stop();}}

async fn play_cue(test_sink:bool,pid:Arc<AtomicU32>,leave:bool,mut cancelled:oneshot::Receiver<()>)->Result<(),String>{
    if cancelled.try_recv()!=Err(oneshot::error::TryRecvError::Empty){return Ok(());}
    let mut command=crate::media::runtime::python();
    command.args(["-u","-c",crate::platform::worker::BOOTSTRAP]);if test_sink{command.arg("--test-sink");}
    command.kill_on_drop(true).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child=crate::platform::worker::Worker::spawn(&mut command).map_err(|e|e.to_string())?;pid.store(child.id().unwrap_or(0),Ordering::SeqCst);
    // Keep ownership while cancellation is handled: kill_on_drop alone only
    // queues best-effort orphan reaping. Explicit wait also handles a helper
    // cancelled immediately after spawn, without making GTK wait for it.
    let mut stdin=child.stdin.take().unwrap();let stdout=child.stdout.take().unwrap();let stderr=child.stderr.take().unwrap();
    let exchange=async{
        crate::platform::worker::source(&mut stdin,include_str!("sound.py")).await?;
        stdin.write_all(if leave{include_bytes!("voice_leave.wav")}else{include_bytes!("voice_join.wav")}).await?;drop(stdin);
        let mut out=Vec::new();let mut err=Vec::new();
        let mut stdout=stdout.take(65537);let mut stderr=stderr.take(65537);
        let(status,_,_)=tokio::try_join!(child.wait(),stdout.read_to_end(&mut out),stderr.read_to_end(&mut err))?;
        Ok::<_,std::io::Error>((status,out,err))
    };
    let result=tokio::select!{_=&mut cancelled=>None,result=tokio::time::timeout(std::time::Duration::from_secs(8),exchange)=>Some(result)};
    let(status,out,err)=match result{
        Some(Ok(Ok(output)))=>output,
        result=>{let _=child.kill().await;let _=child.wait().await;return match result{None=>Ok(()),Some(Err(_))=>Err("Call cue process timed out".into()),Some(Ok(Err(e)))=>Err(e.to_string()),_=>unreachable!()};}
    };
    if !status.success(){return Err(String::from_utf8_lossy(&err).trim().to_owned());}
    let report:serde_json::Value=serde_json::from_slice(&out).map_err(|e|e.to_string())?;
    if report["decoded_bytes"].as_u64().unwrap_or(0)==0{return Err("Call cue produced no audio".into());}Ok(())
}
#[cfg(test)]mod tests{
    #[tokio::test]
    #[ignore="requires native GStreamer; decodes synthetic cues to a discard sink"]
    async fn native_call_cues_complete_and_cancel_without_devices(){
        use super::*;
        for leave in [false,true]{
            let (_cancel,cancelled)=oneshot::channel();
            play_cue(true,Default::default(),leave,cancelled).await.unwrap();
        }
        for _ in 0..3{
            let(cancel,cancelled)=oneshot::channel();let pid=Arc::new(AtomicU32::new(0));let started=pid.clone();
            let task=tokio::spawn(async move{play_cue(true,started,false,cancelled).await});
            tokio::time::timeout(std::time::Duration::from_secs(5),async{while pid.load(Ordering::SeqCst)==0{tokio::task::yield_now().await;}}).await.unwrap();
            cancel.send(()).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(5),task).await.unwrap().unwrap().unwrap();
        }
    }
    #[test]fn departure_has_its_own_short_wave(){let leave=include_bytes!("voice_leave.wav");assert_eq!(&leave[..4],b"RIFF");assert_eq!(&leave[8..12],b"WAVE");assert_ne!(leave.as_slice(),include_bytes!("voice_join.wav").as_slice());assert!(leave.len()<14000);}

    #[test]fn call_chime_is_short_bounded_pcm_wave(){let wav=include_bytes!("voice_join.wav");assert_eq!(&wav[..4],b"RIFF");assert_eq!(&wav[8..12],b"WAVE");assert_eq!(u16::from_le_bytes([wav[22],wav[23]]),1);let rate=u32::from_le_bytes(wav[24..28].try_into().unwrap());let len=u32::from_le_bytes(wav[40..44].try_into().unwrap());assert_eq!(len as usize+44,wav.len());assert!(f64::from(len)/f64::from(rate*2)<0.3);}
}
#[cfg(test)]
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use crate::ui::chat::actions::tests::until;
    let mut cue=CallCue::default();cue.use_test_sink();cue.play(false);assert_eq!(cue.played,0);assert!(cue.job.is_none());
    cue.play(true);until(context,||cue.outcome.lock().unwrap().is_some());assert_eq!(cue.outcome.lock().unwrap().as_ref().unwrap(),&Ok(()));assert_eq!(cue.played,1);
    until(context,||cue.job.as_ref().unwrap().is_finished());
    cue.play_leave(true);until(context,||cue.outcome.lock().unwrap().is_some());assert_eq!(cue.outcome.lock().unwrap().as_ref().unwrap(),&Ok(()));
    cue.play(true);until(context,||cue.pid.load(Ordering::SeqCst)!=0);let outcome=cue.outcome.clone();cue.stop();until(context,||outcome.lock().unwrap().is_some());
    for _ in 0..10{cue.play(true);cue.stop();}assert!(cue.job.is_none(),"rapid join/leave never blocks GTK");
}
