//! EN-US: Async ownership around the narrow native AppContainer/Job bridge.
//! PT-BR: Controle assíncrono da ponte nativa restrita AppContainer/Job.
use std::{ffi::{c_void,OsStr},io,os::windows::{ffi::OsStrExt,process::ExitStatusExt},path::{Path,PathBuf},process::{Output,ExitStatus},sync::{Arc,Mutex},time::Duration};
use windows_sys::Win32::{Foundation::{CloseHandle,HANDLE},System::Threading::{CreateEventW,SetEvent}};

unsafe extern "C" {
    fn papo_media_run(runtime:*const u16,source:*const u16,name:*const u16,script:*const u16,cancel:HANDLE,timeout:u32,
        sink:extern "C" fn(*mut c_void,i32,*const u8,usize),context:*mut c_void,exit_code:*mut u32)->u32;
    fn papo_media_profile_exists(name:*const u16)->u32;
    fn papo_media_profile_remove(name:*const u16)->u32;
}
fn wide(value:impl AsRef<OsStr>)->Vec<u16>{value.as_ref().encode_wide().chain(Some(0)).collect()}
struct Event(usize);
impl Event{fn new()->io::Result<Self>{let handle=unsafe{CreateEventW(std::ptr::null(),1,0,std::ptr::null())};if handle.is_null(){Err(io::Error::last_os_error())}else{Ok(Self(handle as usize))}}fn handle(&self)->HANDLE{self.0 as HANDLE}}
impl Drop for Event{fn drop(&mut self){unsafe{CloseHandle(self.handle());}}}
struct Cancel(Arc<Event>);
impl Drop for Cancel{fn drop(&mut self){unsafe{SetEvent(self.0.handle());}}}
#[derive(Default)]struct Buffers{stdout:Vec<u8>,stderr:Vec<u8>,done:bool,error:u32}
extern "C" fn collect(context:*mut c_void,channel:i32,bytes:*const u8,size:usize){
    // EN-US: The bridge calls synchronously on this blocking worker, after checking each channel's cap.
    // PT-BR: A ponte chama de modo síncrono neste worker bloqueante após verificar o limite do canal.
    let buffers=unsafe{&*context.cast::<Mutex<Buffers>>()};let mut buffers=buffers.lock().unwrap_or_else(|e|e.into_inner());
    let target=if channel==1{&mut buffers.stdout}else{&mut buffers.stderr};target.extend_from_slice(unsafe{std::slice::from_raw_parts(bytes,size)});
}
pub(crate) struct Command{runtime:PathBuf,source:PathBuf,script:String,name:String,timeout:u32,buffers:Arc<Mutex<Buffers>>}
impl Command{
    pub async fn output(&mut self)->io::Result<Output>{
        let event=Arc::new(Event::new()?);let _cancel=Cancel(event.clone());
        let runtime=wide(&self.runtime);let source=wide(&self.source);let name=wide(&self.name);let script=wide(&self.script);
        let timeout=self.timeout;let buffers=self.buffers.clone();
        tokio::task::spawn_blocking(move||{
            let mut exit_code=1;let error=unsafe{papo_media_run(runtime.as_ptr(),source.as_ptr(),name.as_ptr(),script.as_ptr(),event.handle(),timeout,
                collect,Arc::as_ptr(&buffers).cast_mut().cast(),&mut exit_code)};
            let mut buffers=buffers.lock().unwrap_or_else(|e|e.into_inner());buffers.done=true;buffers.error=error;
            if error!=0{return Err(io::Error::from_raw_os_error(error as i32));}
            Ok(Output{status:ExitStatus::from_raw(exit_code),stdout:std::mem::take(&mut buffers.stdout),stderr:std::mem::take(&mut buffers.stderr)})
        }).await.map_err(io::Error::other)?
    }
}
pub(crate) fn command(path:&Path,script:&str)->anyhow::Result<Command>{
    anyhow::ensure!(!script.contains('\0')&&script.encode_utf16().count()<12000,"Invalid media helper script");
    let runtime=crate::platform::runtime::directory().ok_or_else(||anyhow::anyhow!("Media runtime unavailable"))?.canonicalize()?;
    anyhow::ensure!(runtime.join("bin/python.exe").is_file(),"Packaged media interpreter missing");
    Ok(Command{runtime,source:path.canonicalize()?,script:script.to_owned(),name:format!("Papo.Media.{}.{}",std::process::id(),uuid::Uuid::new_v4()),timeout:25000,buffers:Arc::default()})
}

/// EN-US: Explicit disposable acceptance; no user settings, credentials, or media are inspected.
/// PT-BR: Aceitação explícita descartável; não inspeciona configurações, credenciais ou mídias do usuário.
pub(crate) async fn check()->anyhow::Result<serde_json::Value>{
    use anyhow::ensure;
    access_check().await?;
    struct Scratch(PathBuf);impl Drop for Scratch{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let scratch=Scratch(std::env::temp_dir().join(format!("papo-isolation-{}",uuid::Uuid::new_v4())));std::fs::create_dir(&scratch.0)?;
    let input=scratch.0.join("input.mp4");std::fs::write(&input,include_bytes!("../../../tests/fixtures/chat-h264.mp4"))?;
    // EN-US: Test bounded channels, a real job memory quota, and the internal wall-clock deadline.
    // PT-BR: Testa canais limitados, cota real de memória do job e prazo interno de execução.
    for script in ["import sys; sys.stdout.buffer.write(b'x'*(49<<20))","import sys; sys.stderr.buffer.write(b'x'*70000)"]{
        let mut cmd=command(&input,script)?;let name=cmd.name.clone();let error=cmd.output().await.expect_err("oversized helper output must fail");
        ensure!(error.raw_os_error()==Some(111),"Unexpected output-bound error: {error}");ensure!(!profile_exists(&name),"Output-limit profile leaked");
    }
    let output=command(&input,"try:\n    data=bytearray(600<<20)\nexcept MemoryError:\n    print('memory-denied')\nelse:\n    raise AssertionError('job memory limit missing')")?.output().await?;
    ensure!(output.status.success()&&String::from_utf8_lossy(&output.stdout).trim()=="memory-denied","Memory quota not enforced");
    let mut cmd=command(&input,"import time; time.sleep(60)")?;cmd.timeout=2000;let name=cmd.name.clone();let before=std::time::Instant::now();
    ensure!(cmd.output().await.is_err()&&before.elapsed()<Duration::from_secs(6),"Internal deadline not bounded");ensure!(!profile_exists(&name),"Timeout profile leaked");
    cancellation(&input).await?;
    parent_exit(&scratch.0,&input).await?;
    println!("Windows AppContainer limits, cancellation, descendant and parent-exit checks passed");
    Ok(serde_json::json!({"backend":"appcontainer_job","private_files_denied":true,"input_runtime_writes_denied":true,"credential_read_denied":true,"handle_inheritance_denied":true,"tcp4_tcp6_udp_requests":0,"stdout_stderr_limits":true,"committed_memory_limit":true,"internal_deadline":true,"cancellation_tree_cleanup":true,"parent_exit_tree_cleanup":true,"orphan_reaping":true}))
}
fn profile_exists(name:&str)->bool{unsafe{papo_media_profile_exists(wide(name).as_ptr())!=0}}
fn alive(pid:u32)->bool{
    use windows_sys::Win32::System::Threading::{OpenProcess,WaitForSingleObject,PROCESS_SYNCHRONIZE};
    let handle=unsafe{OpenProcess(PROCESS_SYNCHRONIZE,0,pid)};if handle.is_null(){return unsafe{windows_sys::Win32::Foundation::GetLastError()}!=87;}
    let running=unsafe{WaitForSingleObject(handle,0)}==258;unsafe{CloseHandle(handle);}running
}
const TREE:&str="import os, subprocess, sys, time\nchild=subprocess.Popen([sys.executable,'-s','-P','-B','-u','-c',\"import time; print('ready', flush=True); time.sleep(60)\"], close_fds=True, stdout=subprocess.PIPE, creationflags=0x08000000)\nassert child.stdout.readline().strip()==b'ready' and child.poll() is None\nprint(str(os.getpid())+','+str(child.pid), flush=True)\ntime.sleep(60)";
async fn pids(buffers:&Mutex<Buffers>)->anyhow::Result<Vec<u32>>{
    tokio::time::timeout(Duration::from_secs(12),async{loop{
        let (output,done,error,stderr)={let buffers=buffers.lock().unwrap_or_else(|e|e.into_inner());(buffers.stdout.clone(),buffers.done,buffers.error,buffers.stderr.clone())};
        if output.contains(&b'\n'){return String::from_utf8(output)?.trim().split(',').map(|s|s.parse::<u32>().map_err(Into::into)).collect::<anyhow::Result<Vec<_>>>();}
        anyhow::ensure!(!done,"Tree helper exited before readiness (native code {error}): {}",String::from_utf8_lossy(&stderr));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }}).await?
}
async fn gone(pids:&[u32],name:&str)->anyhow::Result<()>{
    tokio::time::timeout(Duration::from_secs(4),async{loop{
        if pids.iter().all(|pid|!alive(*pid))&&!profile_exists(name){return;}
        tokio::time::sleep(Duration::from_millis(20)).await;
    }}).await.map_err(|_|anyhow::anyhow!("Sandbox tree/profile cleanup timed out"))
}
async fn cancellation(input:&Path)->anyhow::Result<()>{
    let mut cmd=command(input,TREE)?;let buffers=cmd.buffers.clone();let name=cmd.name.clone();
    let task=tokio::spawn(async move{cmd.output().await});let result=pids(&buffers).await;
    task.abort();let _=task.await;let pids=result?;anyhow::ensure!(pids.len()==2,"Expected root and descendant");gone(&pids,&name).await
}
#[derive(serde::Serialize,serde::Deserialize)]struct Tree{pids:Vec<u32>,profile:String}
// EN-US: Private test child writes only synthetic process IDs after its sandbox descendant is alive.
// PT-BR: O filho privado de teste grava apenas IDs sintéticos quando o descendente isolado está ativo.
pub(crate) async fn parent_probe(input:&Path,report:&Path)->anyhow::Result<()>{
    let mut cmd=command(input,TREE)?;let buffers=cmd.buffers.clone();let name=cmd.name.clone();
    let task=tokio::spawn(async move{cmd.output().await});let tree=Tree{pids:pids(&buffers).await?,profile:name};
    std::fs::write(report,serde_json::to_vec(&tree)?)?;task.await??;Ok(())
}
async fn parent_exit(directory:&Path,input:&Path)->anyhow::Result<()>{
    let report=directory.join("tree.json");let mut cmd=tokio::process::Command::new(std::env::current_exe()?);
    cmd.args(["--windows-media-parent-probe"]).arg(input).arg(&report).creation_flags(0x08000000).kill_on_drop(true)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let mut child=cmd.spawn()?;
    let tree=tokio::time::timeout(Duration::from_secs(15),async{loop{
        if let Ok(bytes)=std::fs::read(&report){if let Ok(tree)=serde_json::from_slice::<Tree>(&bytes){break tree;}}
        tokio::time::sleep(Duration::from_millis(20)).await;
    }}).await?;
    child.kill().await?;child.wait().await?;
    tokio::time::timeout(Duration::from_secs(3),async{while tree.pids.iter().any(|pid|alive(*pid)){tokio::time::sleep(Duration::from_millis(20)).await;}}).await?;
    // EN-US: Abrupt host exit closes the kernel job. Remove only this known disposable orphan profile.
    // PT-BR: A saída abrupta fecha o job no kernel. Remove somente o perfil órfão descartável conhecido.
    let reaped=command(input,"print('reaped')")?.output().await?;
    let removed=!profile_exists(&tree.profile);
    if !removed{unsafe{papo_media_profile_remove(wide(&tree.profile).as_ptr());}}
    anyhow::ensure!(reaped.status.success()&&removed,"Orphan profile reaper failed");
    anyhow::ensure!(tree.pids.len()==2&&!profile_exists(&tree.profile),"Parent exit left a process/profile");Ok(())
}

async fn access_check()->anyhow::Result<()>{
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{SetHandleInformation,HANDLE_FLAG_INHERIT};
    struct Scratch(PathBuf);impl Drop for Scratch{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let scratch=Scratch(std::env::temp_dir().join(format!("papo-access-{}",uuid::Uuid::new_v4())));std::fs::create_dir(&scratch.0)?;
    let source=scratch.0.join("source.mp4");let private=scratch.0.join("private.txt");
    std::fs::write(&source,include_bytes!("../../../tests/fixtures/chat-h264.mp4"))?;std::fs::write(&private,b"synthetic private marker")?;
    let handle=std::fs::File::open(&private)?;
    anyhow::ensure!(unsafe{SetHandleInformation(handle.as_raw_handle().cast(),HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT)}!=0,"Cannot create inheritable fixture handle");
    let tcp=std::net::TcpListener::bind("127.0.0.1:0")?;tcp.set_nonblocking(true)?;
    let tcp6=std::net::TcpListener::bind("[::1]:0")?;tcp6.set_nonblocking(true)?;
    let udp=std::net::UdpSocket::bind("127.0.0.1:0")?;udp.set_nonblocking(true)?;
    // EN-US: This explicit CLI creates/deletes only its random synthetic native entry, never enumerates credentials.
    // PT-BR: Esta CLI explícita cria/remove só a entrada nativa sintética aleatória, sem enumerar credenciais.
    use windows_sys::Win32::Storage::FileSystem::{GetFileInformationByHandle,BY_HANDLE_FILE_INFORMATION};
    let mut information:BY_HANDLE_FILE_INFORMATION=unsafe{std::mem::zeroed()};
    anyhow::ensure!(unsafe{GetFileInformationByHandle(handle.as_raw_handle().cast(),&mut information)}!=0,"Cannot inspect synthetic fixture identity");
    let script=format!(r#"import ctypes, sys
from ctypes import wintypes as w
class Info(ctypes.Structure):
    _fields_=[('attributes',w.DWORD),('created',w.FILETIME),('accessed',w.FILETIME),('written',w.FILETIME),('volume',w.DWORD),('size_high',w.DWORD),('size_low',w.DWORD),('links',w.DWORD),('index_high',w.DWORD),('index_low',w.DWORD)]
kernel=ctypes.WinDLL('kernel32', use_last_error=True)
kernel.GetFileInformationByHandle.argtypes=[w.HANDLE,ctypes.POINTER(Info)]
info=Info()
print('handle-ready', flush=True)
valid=kernel.GetFileInformationByHandle(w.HANDLE({handle}),ctypes.byref(info))
assert not valid or (info.volume,info.index_high,info.index_low)!=({volume},{high},{low}), 'private handle inherited'
print('handle-denied')
"#,handle=handle.as_raw_handle() as usize,volume=information.dwVolumeSerialNumber,high=information.nFileIndexHigh,low=information.nFileIndexLow);
    let mut cmd=command(&source,&script)?;let name=cmd.name.clone();let output=cmd.output().await?;
    // EN-US: Strict handle faults prove this value is not the inherited file; unrelated crashes still fail.
    // PT-BR: Falhas estritas de handle provam que não é o arquivo herdado; outros crashes continuam falhando.
    let native_invalid=matches!(output.status.code().map(|code|code as u32),Some(0xc0000008|0xc0000024));
    let text=String::from_utf8_lossy(&output.stdout);
    anyhow::ensure!((output.status.success()&&text.lines().last().is_some_and(|s|s.trim()=="handle-denied"))||(native_invalid&&text.trim()=="handle-ready"),"Private handle inheritance probe failed: {:?}",output.status.code());
    anyhow::ensure!(!profile_exists(&name),"Handle probe cleanup failed");
    let target=format!("Papo.Media.Probe.{}",uuid::Uuid::new_v4());
    struct Credential(keyring::Entry);impl Drop for Credential{fn drop(&mut self){let _=self.0.delete_credential();}}
    let credential=Credential(keyring::Entry::new_with_target(&target,"Papo media acceptance","Papo")?);credential.0.set_password("synthetic fixture only")?;
    let runtime=command(&source,"")?.runtime;
    let marker=runtime.join(format!("papo-write-probe-{}",uuid::Uuid::new_v4()));
    struct RuntimeMarker(PathBuf);impl Drop for RuntimeMarker{fn drop(&mut self){let _=std::fs::remove_file(&self.0);}}
    let _marker_guard=RuntimeMarker(marker.clone());
    let script=format!(r#"import ctypes, os, socket, sys, tempfile
from ctypes import wintypes as w
assert not os.environ.get('PAPO_RUNTIME_DIR')
assert not os.environ.get('HTTP_PROXY') and not os.environ.get('DBUS_SESSION_BUS_ADDRESS')
print('access: files', file=sys.stderr, flush=True)
for path, mode in [({private:?}, 'rb'), ({source:?}, 'rb'), (sys.argv[1], 'ab'), ({marker:?}, 'xb')]:
    try:
        f=open(path, mode)
    except OSError:
        pass
    else:
        f.close()
        raise AssertionError('private file/input/runtime boundary failed')
print('access: attributes', file=sys.stderr, flush=True)
try:
    os.chmod(sys.argv[1], 0o777)
except OSError:
    pass
else:
    raise AssertionError('input attributes writable')
print('access: replace', file=sys.stderr, flush=True)
try:
    os.rename(os.path.dirname(sys.argv[1]), os.path.dirname(sys.argv[1])+'-replaced')
except OSError:
    pass
else:
    raise AssertionError('input directory replaceable')
print('access: scratch', file=sys.stderr, flush=True)
with tempfile.TemporaryFile() as f:
    f.write(b'private writable scratch')
print('access: credentials', file=sys.stderr, flush=True)
cred=ctypes.WinDLL('advapi32', use_last_error=True)
cred.CredReadW.argtypes=[w.LPCWSTR,w.DWORD,w.DWORD,ctypes.POINTER(ctypes.c_void_p)]
result=ctypes.c_void_p()
assert not cred.CredReadW({target:?},1,0,ctypes.byref(result)), 'credential read escaped container'
print('access: tcp', file=sys.stderr, flush=True)
for family,address in [(socket.AF_INET,('127.0.0.1',{tcp})), (socket.AF_INET6,('::1',{tcp6}))]:
    s=socket.socket(family,socket.SOCK_STREAM); s.settimeout(0.3)
    try:
        s.connect(address)
    except OSError:
        pass
    else:
        raise AssertionError('TCP escaped container')
    finally:
        s.close()
print('access: udp', file=sys.stderr, flush=True)
s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM)
try:
    s.sendto(b'synthetic',('127.0.0.1',{udp}))
except OSError:
    pass
finally:
    s.close()
print('access-denied')
"#,private=private.to_string_lossy(),source=source.to_string_lossy(),marker=marker.to_string_lossy(),tcp=tcp.local_addr()?.port(),tcp6=tcp6.local_addr()?.port(),udp=udp.local_addr()?.port());
    let mut cmd=command(&source,&script)?;let name=cmd.name.clone();let output=cmd.output().await?;
    anyhow::ensure!(output.status.success()&&String::from_utf8_lossy(&output.stdout).trim()=="access-denied","Native access probe failed (exit {:?}, stdout {} bytes): {}",output.status.code(),output.stdout.len(),String::from_utf8_lossy(&output.stderr));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut bytes=[0;64];anyhow::ensure!(tcp.accept().is_err()&&tcp6.accept().is_err()&&udp.recv_from(&mut bytes).is_err(),"Sandbox made a network request");
    anyhow::ensure!(!profile_exists(&name)&&!marker.exists(),"Access probe cleanup failed");
    anyhow::ensure!(std::fs::read(&source)?==include_bytes!("../../../tests/fixtures/chat-h264.mp4"),"Original source changed");
    drop(credential);
    anyhow::ensure!(matches!(keyring::Entry::new_with_target(&target,"Papo media acceptance","Papo")?.get_password(),Err(keyring::Error::NoEntry)),"Fixture credential leaked");
    println!("Windows AppContainer private files, input/runtime writes, credentials, handles and TCP4/TCP6/UDP denied");Ok(())
}
