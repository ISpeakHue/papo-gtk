//! EN-US: Native media process with bounded IPC and deterministic capture cleanup.
//! PT-BR: Processo nativo de mídia com comunicação limitada e liberação determinística dos recursos de captura.
use serde::Deserialize;
use serde_json::Value;
use tokio::{io::{AsyncBufReadExt,AsyncWriteExt,BufReader},sync::mpsc,process::Command};
use std::{process::Stdio,time::Duration};
use crate::platform::capture::WORKER;
#[derive(Debug,Clone,Deserialize)]
pub struct Device {pub id:String,pub name:String}
#[derive(Clone,Deserialize)]
#[serde(tag="type")]
pub enum EngineEvent {
    #[serde(rename="microphone_gain")]MicrophoneGain{
        #[cfg_attr(not(test), expect(dead_code, reason = "Retained voice worker protocol metadata; decoding still validates its required type."))]
        gain:f64},
    #[serde(rename="speaking")]Speaking{active:bool},
    #[serde(rename="started")]Started{
        #[cfg_attr(not(test), expect(dead_code, reason = "Retained voice worker protocol metadata; decoding still validates its required type."))]
        pid:u32},
    #[serde(rename="voice_offer")]Offer{sdp:String},
    #[serde(rename="voice_answer")]Answer{sdp:String},
    #[serde(rename="voice_ice_candidate")]Candidate{candidate:String,sdp_mid:Option<String>,sdp_mline_index:u32},
    #[serde(rename="connection")]Connection{state:String},
    #[serde(rename="track")]Track{
        #[expect(dead_code, reason = "Retained voice worker protocol metadata; decoding still validates its required type.")]
        track_id:String},
    #[serde(rename="error")]Error{message:String},
    #[serde(rename="negotiated")]Negotiated,
    #[serde(rename="media_intent")]MediaIntent{kind:String,on:bool},
    #[serde(rename="media_state")]MediaState{kind:String,on:bool},
    #[serde(rename="media_error")]MediaError{kind:String,message:String},
    #[serde(rename="video_frame")]VideoFrame{track_id:String,epoch:u64,jpeg:String},
    #[serde(rename="video_reset")]VideoReset{epoch:u64},
}
impl std::fmt::Debug for EngineEvent{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("EngineEvent { media details: [redacted] }")}}
// EN-US: Send embedded source over the private pipe; Windows command lines have a 32K limit.
// PT-BR: Envia o código embutido pelo pipe privado; linhas de comando Windows têm limite de 32K.
const BOOTSTRAP:&str="import sys;exec(compile(sys.stdin.buffer.read(int(sys.stdin.buffer.readline())), '<papo-call-worker>', 'exec'))";
async fn source(input:&mut tokio::process::ChildStdin)->std::io::Result<()>{
    input.write_all(format!("{}\n",WORKER.len()).as_bytes()).await?;input.write_all(WORKER.as_bytes()).await
}
fn command()->Command{let mut c=crate::media::runtime::python();c.args(["-u","-c",BOOTSTRAP]).env("GST_DEBUG","0").stderr(Stdio::null()).kill_on_drop(true);c}
pub async fn devices()->anyhow::Result<Vec<Device>>{list_devices("--devices").await}
pub async fn cameras()->anyhow::Result<Vec<Device>>{list_devices("--cameras").await}
async fn list_devices(flag:&str)->anyhow::Result<Vec<Device>>{
    let output=tokio::time::timeout(Duration::from_secs(8),async {
        let mut child=command().arg(flag).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
        let mut input=child.stdin.take().unwrap();source(&mut input).await?;drop(input);
        child.wait_with_output().await
    }).await.map_err(|_|anyhow::anyhow!("A lista de dispositivos demorou demais."))?.map_err(|_|anyhow::anyhow!("Instale Python GI e os plugins GStreamer de voz."))?;
    #[derive(Deserialize)]struct List {devices:Vec<Device>}
    let list:List=serde_json::from_slice(&output.stdout).map_err(|_|anyhow::anyhow!("Não foi possível listar dispositivos. Verifique Python GI e GStreamer."))?;
    Ok(list.devices)
}
pub struct Engine {commands:mpsc::Sender<Value>,task:tokio::task::JoinHandle<()>}
impl Drop for Engine {fn drop(&mut self){self.task.abort();}}
impl Engine {
    pub fn send(&self,command:Value)->bool{self.commands.try_send(command).is_ok()}
    pub fn start(config:Value,report:impl Fn(EngineEvent)+Send+'static)->Self{
        let(commands,mut rx)=mpsc::channel::<Value>(64);
        let task=tokio::spawn(async move{
            let mut child=match command().stdin(Stdio::piped()).stdout(Stdio::piped()).spawn(){Ok(c)=>c,Err(_)=>{report(EngineEvent::Error{message:"Instale Python GI e os plugins GStreamer de voz.".into()});return;}};
            let mut input=child.stdin.take().unwrap();let mut output=BufReader::new(child.stdout.take().unwrap()).lines();
            async fn write(input:&mut tokio::process::ChildStdin,value:&Value)->std::io::Result<()>{let mut data=serde_json::to_vec(value)?;data.push(b'\n');input.write_all(&data).await}
            if source(&mut input).await.is_err()||write(&mut input,&config).await.is_err(){return;}
            drop(config);
            loop{tokio::select!{
                line=output.next_line()=>match line{Ok(Some(line)) if line.len()<=128*1024=>match serde_json::from_str::<EngineEvent>(&line){Ok(event)=>report(event),Err(_)=>{report(EngineEvent::Error{message:"Resposta inválida do processo de áudio.".into()});break;}},_=>{report(EngineEvent::Error{message:"O processo de áudio foi encerrado.".into()});break;}},
                command=rx.recv()=>match command{Some(c)=>if write(&mut input,&c).await.is_err(){report(EngineEvent::Error{message:"O processo de áudio não responde.".into()});break;},None=>break},
            }}
            let _=child.kill().await;let _=child.wait().await;
        });Self{commands,task}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore="requires native GStreamer/libnice and local ICE sockets; uses synthetic audio"]
    async fn native_voice_worker_drop_releases_process() {
        let (tx,mut rx)=mpsc::unbounded_channel();
        let engine=Engine::start(serde_json::json!({"type":"start","test_source":true,"muted":true,"microphone_gain":2.0}),move|e|{let _=tx.send(e);});
        let pid=tokio::time::timeout(Duration::from_secs(10),async {loop{match rx.recv().await.unwrap(){EngineEvent::Started{pid}=>break pid,EngineEvent::Error{message}=>panic!("{message}"),_=>{}}}}).await.unwrap();
        assert_ne!(pid,0,"worker reports a native process identifier");
        let gain=tokio::time::timeout(Duration::from_secs(5),async{loop{match rx.recv().await.unwrap(){EngineEvent::MicrophoneGain{gain}=>break gain,EngineEvent::Error{message}=>panic!("{message}"),_=>{}}}}).await.unwrap();assert_eq!(gain,2.0,"startup honors the configured capture gain");
        for (requested,expected) in [(-1.0,0.0),(1.5,1.5),(99.0,3.0)]{
            assert!(engine.send(serde_json::json!({"type":"microphone_gain","gain":requested})));
            let gain=tokio::time::timeout(Duration::from_secs(5),async{loop{match rx.recv().await.unwrap(){EngineEvent::MicrophoneGain{gain}=>break gain,EngineEvent::Error{message}=>panic!("{message}"),_=>{}}}}).await.unwrap();assert_eq!(gain,expected,"live gain is bounded without replacing or unmuting capture");
        }
        #[cfg(target_os="linux")]assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
        // EN-US: Retain the Windows process handle before dropping the worker to avoid PID reuse races.
        // PT-BR: Retém o handle do processo Windows antes de encerrar o auxiliar para evitar reutilização do PID.
        #[cfg(windows)]let process=unsafe{windows_sys::Win32::System::Threading::OpenProcess(0x00100000,0,pid)};
        #[cfg(windows)]assert!(!process.is_null());
        drop(engine);
        #[cfg(windows)]{
            let process=process as usize;
            tokio::task::spawn_blocking(move ||unsafe{
                use windows_sys::Win32::{Foundation::{CloseHandle,WAIT_OBJECT_0},System::Threading::WaitForSingleObject};
                let result=WaitForSingleObject(process as _,5000);CloseHandle(process as _);
                assert_eq!(result,WAIT_OBJECT_0,"dropping native voice must release its Windows capture process");
            }).await.unwrap();
        }
        #[cfg(target_os="linux")]tokio::time::timeout(Duration::from_secs(5),async{while std::path::Path::new(&format!("/proc/{pid}")).exists(){tokio::time::sleep(Duration::from_millis(20)).await;}}).await.expect("dropping native voice must release and reap the capture process");
    }
}
