//! EN-US: Own the whole call/cue process tree, independently of decoder sandboxing.
//! PT-BR: Mantém toda a árvore de processos de chamadas/sons, separada do isolamento de decodificadores.
use std::{io, ops::{Deref, DerefMut}};
use tokio::process::{Child, Command};

pub const BOOTSTRAP: &str = "import sys;exec(compile(sys.stdin.buffer.read(int(sys.stdin.buffer.readline())), '<papo-worker>', 'exec'))";

pub async fn source(input: &mut tokio::process::ChildStdin, source: &str) -> io::Result<()> {
    use tokio::io::AsyncWriteExt;
    input.write_all(format!("{}\n", source.len()).as_bytes()).await?;
    input.write_all(source.as_bytes()).await
}

pub struct Worker {
    child: Child,
    _tree: Tree,
}
impl Deref for Worker {type Target = Child; fn deref(&self) -> &Child {&self.child}}
impl DerefMut for Worker {fn deref_mut(&mut self) -> &mut Child {&mut self.child}}
impl Worker {
    // EN-US: The child waits for source on stdin until tree ownership is established.
    // PT-BR: O filho aguarda código em stdin até que a árvore esteja sob controle.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        command.kill_on_drop(true);
        #[cfg(unix)]command.process_group(0);
        let mut child = command.spawn()?;
        match Tree::attach(&child) {
            Ok(tree) => Ok(Self {child, _tree: tree}),
            Err(error) => {let _ = child.start_kill(); Err(error)}
        }
    }
}

#[cfg(unix)]struct Tree(libc::pid_t);
#[cfg(unix)]impl Tree {
    fn attach(child: &Child) -> io::Result<Self> {
        child.id().map(|id| Self(id as libc::pid_t)).ok_or_else(|| io::Error::other("worker already exited"))
    }
}
#[cfg(unix)]impl Drop for Tree {
    fn drop(&mut self) {
        // EN-US: Descendants inherit this dedicated group; never signal the application's group.
        // PT-BR: Descendentes herdam este grupo exclusivo; nunca sinaliza o grupo da aplicação.
        unsafe {libc::kill(-self.0, libc::SIGKILL);}
    }
}

#[cfg(windows)]struct Tree(windows_sys::Win32::Foundation::HANDLE);
// EN-US: The exclusively owned Job handle can move with its Tokio task.
// PT-BR: O handle exclusivo do Job pode acompanhar sua tarefa Tokio entre threads.
#[cfg(windows)]unsafe impl Send for Tree {}
#[cfg(windows)]impl Tree {
    fn attach(child: &Child) -> io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {return Err(io::Error::last_os_error());}
            let tree = Self(job);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let process = child.raw_handle().ok_or_else(|| io::Error::other("worker already exited"))?;
            if SetInformationJobObject(job, JobObjectExtendedLimitInformation, &limits as *const _ as _, std::mem::size_of_val(&limits) as u32) == 0
                || AssignProcessToJobObject(job, process as _) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(tree)
        }
    }
}
#[cfg(windows)]impl Drop for Tree {fn drop(&mut self) {unsafe {windows_sys::Win32::Foundation::CloseHandle(self.0);}}}

// EN-US: Keep partial lines outside the future so select cancellation cannot discard bytes.
// PT-BR: Mantém linhas parciais fora do future para que cancelamentos do select não descartem bytes.
pub async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R, pending: &mut Vec<u8>, limit: usize) -> io::Result<Option<Vec<u8>>> {
    use tokio::io::AsyncBufReadExt;
    loop {
        let data = reader.fill_buf().await?;
        if data.is_empty() {
            return if pending.is_empty() {Ok(None)} else {Err(io::Error::other("incomplete worker response"))};
        }
        let newline = data.iter().position(|b| *b == b'\n');
        let count = newline.map_or(data.len(), |i| i + 1);
        if pending.len() + count > limit {return Err(io::Error::other("worker response exceeds limit"));}
        pending.extend_from_slice(&data[..count]);
        reader.consume(count);
        if newline.is_some() {return Ok(Some(std::mem::take(pending)));}
    }
}

#[cfg(test)]mod tests {
    use super::*;
    #[tokio::test]async fn bounded_lines_preserve_cancelled_partial_input() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(64);
        let mut reader = tokio::io::BufReader::new(reader); let mut pending = Vec::new();
        writer.write_all(b"part").await.unwrap();
        assert!(tokio::time::timeout(std::time::Duration::from_millis(10), read_line(&mut reader, &mut pending, 16)).await.is_err());
        writer.write_all(b"ial\nnext\n").await.unwrap();
        assert_eq!(read_line(&mut reader, &mut pending, 16).await.unwrap().unwrap(), b"partial\n");
        assert_eq!(read_line(&mut reader, &mut pending, 16).await.unwrap().unwrap(), b"next\n");
        writer.write_all(b"0123456789abcdefg").await.unwrap();
        assert!(read_line(&mut reader, &mut pending, 16).await.is_err());
        assert!(pending.len() <= 16);
    }
    #[tokio::test]
    #[ignore="spawns native Python and a synthetic descendant; no capture devices"]
    async fn native_call_worker_drop_closes_descendant_pipes() {
        use tokio::io::AsyncBufReadExt;
        for _ in 0..3 {
            let mut command = crate::media::runtime::python();
            command.args(["-u", "-c", BOOTSTRAP]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
            let mut worker = Worker::spawn(&mut command).unwrap();
            let mut input = worker.stdin.take().unwrap();
            source(&mut input, "import subprocess,sys,time\np=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)'])\nprint('ready',flush=True)\ntime.sleep(60)").await.unwrap();
            drop(input);
            let mut output = tokio::io::BufReader::new(worker.stdout.take().unwrap());
            let mut line = String::new();
            tokio::time::timeout(std::time::Duration::from_secs(10), output.read_line(&mut line)).await.unwrap().unwrap();
            assert_eq!(line.trim(), "ready");
            drop(worker);
            line.clear();
            let count = tokio::time::timeout(std::time::Duration::from_secs(5), output.read_line(&mut line)).await.expect("descendant must not retain the pipe after worker teardown").unwrap();
            assert_eq!(count, 0);
        }
    }
}
