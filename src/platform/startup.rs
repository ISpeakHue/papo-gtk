//! EN-US: Per-user/session Windows instance ownership and bounded, local-only activation IPC.
//! PT-BR: Instância Windows por usuário/sessão e IPC de ativação local com limites explícitos.
use super::activation::{Activation,parse_uri};
use anyhow::{Context,Result};
use serde::{Deserialize,Serialize};
use std::{collections::VecDeque,ffi::c_void,os::windows::{ffi::OsStrExt,io::AsRawHandle},sync::Mutex,time::Duration};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::windows::named_pipe::{NamedPipeServer,NamedPipeClient,ClientOptions,ServerOptions}};
use windows_sys::Win32::{Foundation::{CloseHandle,LocalFree},Security::{GetTokenInformation,TokenUser,TOKEN_QUERY,TOKEN_USER,SECURITY_ATTRIBUTES},Security::Authorization::{ConvertSidToStringSidW,ConvertStringSecurityDescriptorToSecurityDescriptorW},System::Threading::{GetCurrentProcess,OpenProcessToken,OpenProcess,PROCESS_QUERY_LIMITED_INFORMATION},System::Pipes::GetNamedPipeServerProcessId,System::RemoteDesktop::ProcessIdToSessionId,UI::{Shell::SetCurrentProcessExplicitAppUserModelID,WindowsAndMessaging::AllowSetForegroundWindow}};

#[derive(Serialize,Deserialize)]
#[serde(tag="type",deny_unknown_fields)]
enum Request {Present,Ticket{ticket:uuid::Uuid},Status}
#[derive(Serialize,Deserialize)]
struct Reply {ok:bool,pid:u32,accepted:u64,delivered:u64,queued:usize,console:bool}
struct State {queue:VecDeque<Activation>,accepted:u64,delivered:u64}
static STATE:Mutex<State>=Mutex::new(State{queue:VecDeque::new(),accepted:0,delivered:0});

pub fn drain()->Vec<Activation>{
    let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
    let result:Vec<_>=state.queue.drain(..).collect();state.delivered+=result.len() as u64;result
}
fn reply(request:Request)->Reply{
    let mut state=STATE.lock().unwrap_or_else(|e|e.into_inner());
    let activation=match request{Request::Present=>Some(Activation::Present),Request::Ticket{ticket}=>Some(Activation::Ticket(ticket)),Request::Status=>None};
    let ok=activation.is_none()||state.queue.len()<32;
    if ok{if let Some(activation)=activation{state.queue.push_back(activation);state.accepted+=1;}}
    Reply{ok,pid:std::process::id(),accepted:state.accepted,delivered:state.delivered,queued:state.queue.len(),console:!unsafe{windows_sys::Win32::System::Console::GetConsoleWindow()}.is_null()}
}
fn wide(value:&str)->Vec<u16>{std::ffi::OsStr::new(value).encode_wide().chain(Some(0)).collect()}
fn process_sid(process:windows_sys::Win32::Foundation::HANDLE)->Result<String>{unsafe{
    let mut token=std::ptr::null_mut();anyhow::ensure!(OpenProcessToken(process,TOKEN_QUERY,&mut token)!=0,"Cannot read process identity");
    let result=(||{let mut length=0;GetTokenInformation(token,TokenUser,std::ptr::null_mut(),0,&mut length);
        anyhow::ensure!(length>0&&length<65536,"Invalid token size");
        let mut buffer=vec![0usize;(length as usize).div_ceil(std::mem::size_of::<usize>())];
        anyhow::ensure!(GetTokenInformation(token,TokenUser,buffer.as_mut_ptr().cast(),length,&mut length)!=0,"Cannot read user SID");
        let user=&*buffer.as_ptr().cast::<TOKEN_USER>();let mut text=std::ptr::null_mut();
        anyhow::ensure!(ConvertSidToStringSidW(user.User.Sid,&mut text)!=0,"Cannot encode user SID");
        let mut size=0;while *text.add(size)!=0{size+=1;}
        let sid=String::from_utf16(std::slice::from_raw_parts(text,size));LocalFree(text.cast());Ok(sid?)})();
    CloseHandle(token);result
}}
fn user_sid()->Result<String>{process_sid(unsafe{GetCurrentProcess()})}
fn verify_peer(client:&NamedPipeClient)->Result<u32>{unsafe{
    // EN-US: Validate the kernel-reported peer before sending, including a pre-created/squatted pipe.
    // PT-BR: Valida o peer informado pelo kernel antes de enviar, inclusive se o pipe foi reservado por outro processo.
    let mut pid=0;anyhow::ensure!(GetNamedPipeServerProcessId(client.as_raw_handle().cast(),&mut pid)!=0,"Cannot identify activation peer");
    let process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid);anyhow::ensure!(!process.is_null(),"Cannot inspect activation peer");
    let sid=process_sid(process);CloseHandle(process);anyhow::ensure!(sid?==user_sid()?,"Activation peer belongs to another user");
    let mut peer_session=0;let mut session=0;
    anyhow::ensure!(ProcessIdToSessionId(pid,&mut peer_session)!=0&&ProcessIdToSessionId(std::process::id(),&mut session)!=0&&peer_session==session,"Activation peer belongs to another session");
    Ok(pid)
}}
fn pipe_name(sid:&str)->Result<String>{let mut session=0;anyhow::ensure!(unsafe{ProcessIdToSessionId(std::process::id(),&mut session)}!=0,"Cannot identify desktop session");Ok(format!(r"\\.\pipe\br.com.papo.gtk.{sid}.{session}"))}
fn server(name:&str,sid:&str,first:bool)->std::io::Result<NamedPipeServer>{unsafe{
    // EN-US: A name containing a SID is insufficient; the DACL grants access only to that user.
    // PT-BR: Um nome com SID não basta; a DACL permite acesso somente a esse usuário.
    let mut descriptor=std::ptr::null_mut();let sddl=wide(&format!("D:P(A;;GA;;;{sid})"));
    if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(),1,&mut descriptor,std::ptr::null_mut())==0{return Err(std::io::Error::last_os_error());}
    let mut attributes=SECURITY_ATTRIBUTES{nLength:std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,lpSecurityDescriptor:descriptor,bInheritHandle:0};
    let result=ServerOptions::new().first_pipe_instance(first).reject_remote_clients(true).in_buffer_size(1024).out_buffer_size(1024)
        .create_with_security_attributes_raw(name,(&mut attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>());
    LocalFree(descriptor);result
}}
async fn serve(mut pipe:NamedPipeServer,name:String,sid:String){
    loop{
        if pipe.connect().await.is_err(){break;}
        // EN-US: Reserve the next listener before releasing the previous instance's name.
        // PT-BR: Reserva o próximo listener antes de liberar o nome da instância anterior.
        let Ok(next)=server(&name,&sid,false)else{break;};
        let _=tokio::time::timeout(Duration::from_secs(2),async{
            let size=pipe.read_u32_le().await? as usize;
            if size>512{return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,"Oversized activation"));}
            let mut bytes=vec![0;size];pipe.read_exact(&mut bytes).await?;
            let request:Request=serde_json::from_slice(&bytes)?;
            let bytes=serde_json::to_vec(&reply(request))?;pipe.write_u32_le(bytes.len() as u32).await?;pipe.write_all(&bytes).await?;pipe.flush().await
        }).await;
        pipe=next;
    }
}
async fn exchange(name:&str,request:&Request)->Result<Reply>{
    tokio::time::timeout(Duration::from_secs(3),async{
        let mut client=loop{match ClientOptions::new().open(name){Ok(client)=>break client,Err(e)if e.raw_os_error()==Some(231)=>tokio::time::sleep(Duration::from_millis(25)).await,Err(e)=>return Err(e.into())}};
        let peer=verify_peer(&client)?;
        let bytes=serde_json::to_vec(request)?;client.write_u32_le(bytes.len() as u32).await?;client.write_all(&bytes).await?;
        let size=client.read_u32_le().await?;anyhow::ensure!(size<=512,"Invalid activation acknowledgement");let mut bytes=vec![0;size as usize];client.read_exact(&mut bytes).await?;
        let response=serde_json::from_slice::<Reply>(&bytes)?;anyhow::ensure!(response.pid==peer,"Activation reply has a forged process identity");Ok(response)
    }).await.context("Activation timed out")?
}
pub struct Guard(tokio::task::JoinHandle<()>);
impl Drop for Guard{fn drop(&mut self){self.0.abort();}}
pub async fn start()->Result<Option<Guard>>{
    let id=wide(crate::app::APP_ID);anyhow::ensure!(unsafe{SetCurrentProcessExplicitAppUserModelID(id.as_ptr())}>=0,"Cannot set Windows application identity");
    let args:Vec<_>=std::env::args().skip(1).collect();let sid=user_sid()?;let name=pipe_name(&sid)?;
    if args==["--startup-info"]{println!("{}",serde_json::to_string(&exchange(&name,&Request::Status).await?)?);return Ok(None);}
    let request=if args.is_empty(){Request::Present}else{
        anyhow::ensure!(args.len()==2&&args[0]=="--activate","Unknown Windows startup arguments");
        Request::Ticket{ticket:parse_uri(&args[1]).context("Invalid activation URI")?}
    };
    if let Ok(status)=exchange(&name,&Request::Status).await{
        unsafe{AllowSetForegroundWindow(status.pid);}
        anyhow::ensure!(exchange(&name,&request).await?.ok,"Activation queue full");return Ok(None);
    }
    let pipe=match server(&name,&sid,true){Ok(pipe)=>pipe,Err(error)=>{
        for _ in 0..40{tokio::time::sleep(Duration::from_millis(25)).await;if let Ok(status)=exchange(&name,&Request::Status).await{unsafe{AllowSetForegroundWindow(status.pid);}anyhow::ensure!(exchange(&name,&request).await?.ok,"Activation queue full");return Ok(None);}}
        return Err(error).context("Cannot own Windows application instance");
    }};
    reply(request);Ok(Some(Guard(tokio::spawn(serve(pipe,name,sid)))))
}

#[cfg(test)]mod tests{
    use super::*;
    #[tokio::test]async fn native_activation_pipe_is_bounded_and_exclusive(){
        let sid=user_sid().unwrap();let name=format!(r"\\.\pipe\Papo-test-{}",uuid::Uuid::new_v4());
        let pipe=server(&name,&sid,true).unwrap();assert!(server(&name,&sid,true).is_err());let job=tokio::spawn(serve(pipe,name.clone(),sid));
        let before=exchange(&name,&Request::Status).await.unwrap();let ticket=uuid::Uuid::new_v4();assert!(exchange(&name,&Request::Ticket{ticket}).await.unwrap().ok);
        assert_eq!(exchange(&name,&Request::Status).await.unwrap().accepted,before.accepted+1);
        assert!(drain().contains(&Activation::Ticket(ticket)));
        let mut client=ClientOptions::new().open(&name).unwrap();client.write_u32_le(1024*1024).await.unwrap();drop(client);
        tokio::time::sleep(Duration::from_millis(20)).await;assert!(exchange(&name,&Request::Status).await.is_ok());job.abort();
    }
    #[tokio::test]async fn native_activation_rejects_forged_reply_identity(){
        let sid=user_sid().unwrap();let name=format!(r"\\.\pipe\Papo-peer-test-{}",uuid::Uuid::new_v4());let mut pipe=server(&name,&sid,true).unwrap();
        let job=tokio::spawn(async move{pipe.connect().await.unwrap();let size=pipe.read_u32_le().await.unwrap();let mut bytes=vec![0;size as usize];pipe.read_exact(&mut bytes).await.unwrap();let mut response=reply(Request::Status);response.pid=response.pid.wrapping_add(1);let bytes=serde_json::to_vec(&response).unwrap();pipe.write_u32_le(bytes.len() as u32).await.unwrap();pipe.write_all(&bytes).await.unwrap();tokio::time::sleep(Duration::from_millis(100)).await;});
        assert!(exchange(&name,&Request::Status).await.is_err());job.await.unwrap();
    }

}
