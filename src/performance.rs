//! EN-US: Opt-in aggregate diagnostics. Labels are fixed in source, never user data.
//! PT-BR: Diagnósticos agregados opcionais; os rótulos são fixos no código e não contêm dados do usuário.
use std::{collections::BTreeMap,sync::{LazyLock,Mutex},time::Instant};
static ENABLED:LazyLock<bool>=LazyLock::new(||std::env::var("PAPO_PERFORMANCE").as_deref()==Ok("1"));
#[derive(Default,Clone,serde::Serialize)]pub(crate) struct Sample{pub count:u64,pub total:u64,pub max:u64}
static SAMPLES:LazyLock<Mutex<BTreeMap<&'static str,Sample>>>=LazyLock::new(Mutex::default);
pub fn enabled()->bool{*ENABLED}
pub fn record(label:&'static str,value:u64){if !enabled(){return;}if let Ok(mut samples)=SAMPLES.lock(){let sample=samples.entry(label).or_default();sample.count+=1;sample.total=sample.total.saturating_add(value);sample.max=sample.max.max(value);}}
pub struct Timer{label:&'static str,start:Option<Instant>}
pub fn timer(label:&'static str)->Timer{Timer{label,start:enabled().then(Instant::now)}}
impl Drop for Timer{fn drop(&mut self){if let Some(start)=self.start{record(self.label,start.elapsed().as_micros().min(u64::MAX as u128) as u64);}}}
pub fn report(){if !enabled(){return;}if let Some(bytes)=resident_bytes(){record("process_resident_bytes",bytes);}if let Ok(samples)=SAMPLES.lock(){let rows:BTreeMap<_,_>=samples.iter().map(|(key,s)|(key,serde_json::json!({"samples":s.count,"total":s.total,"max":s.max}))).collect();eprintln!("Papo aggregate performance: {}",serde_json::to_string(&rows).unwrap_or_default());}}

pub struct Report;
impl Drop for Report{fn drop(&mut self){report();}}

/// Measures GTK CPU paint work against the display's reported refresh period.
/// This does not claim to count compositor/GPU presentation misses.
pub fn watch(widget:&impl gtk::prelude::IsA<gtk::Widget>){
    use gtk::prelude::*;
    if !enabled(){return;}
    type Hooks=Option<(gtk::gdk::FrameClock,gtk::glib::SignalHandlerId,gtk::glib::SignalHandlerId)>;
    let hooks=std::rc::Rc::new(std::cell::RefCell::new(None::<(gtk::gdk::FrameClock,gtk::glib::SignalHandlerId,gtk::glib::SignalHandlerId)>));let mapped=hooks.clone();
    widget.as_ref().connect_map(move |w|{if mapped.borrow().is_some(){return;}let Some(clock)=w.frame_clock()else{return;};let started=std::rc::Rc::new(std::cell::Cell::new(None::<Instant>));let begin=started.clone();
        let before=clock.connect_before_paint(move |_|begin.set(Some(Instant::now())));
        let after=clock.connect_after_paint(move |clock|{if let Some(start)=started.take(){let elapsed=start.elapsed().as_micros().min(u64::MAX as u128) as u64;record("frame_paint_us",elapsed);let(interval,_)=clock.refresh_info(clock.frame_time());if interval>0{record("frame_budget_us",interval as u64);if elapsed>interval as u64{record("frame_paint_over_budget",1);}}}});
        *mapped.borrow_mut()=Some((clock,before,after));
    });
    let hooks:std::rc::Rc<std::cell::RefCell<Hooks>>=hooks;
    widget.as_ref().connect_unmap(move |_|{if let Some((clock,before,after))=hooks.borrow_mut().take(){clock.disconnect(before);clock.disconnect(after);}});
}

// EN-US: Current resident memory is distinct from allocator/cache accounting and GPU memory.
// PT-BR: A memória residente atual difere da contabilidade de caches/alocador e da memória da GPU.
pub fn resident_bytes()->Option<u64>{
    #[cfg(target_os="linux")]{
        let status=std::fs::read_to_string("/proc/self/status").ok()?;
        status.lines().find_map(|line|line.strip_prefix("VmRSS:").and_then(|s|s.split_whitespace().next())?.parse::<u64>().ok()?.checked_mul(1024))
    }
    #[cfg(windows)]{
        use windows_sys::Win32::System::{ProcessStatus::{GetProcessMemoryInfo,PROCESS_MEMORY_COUNTERS},Threading::GetCurrentProcess};
        let mut counters:PROCESS_MEMORY_COUNTERS=unsafe{std::mem::zeroed()};
        let size=std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;counters.cb=size;
        // EN-US: Read only the current process; no elevated rights or other processes are inspected.
        // PT-BR: Lê apenas o processo atual, sem privilégios elevados nem inspeção de outros processos.
        (unsafe{GetProcessMemoryInfo(GetCurrentProcess(),&mut counters,size)}!=0).then_some(counters.WorkingSetSize as u64)
    }
    #[cfg(not(any(target_os="linux",windows)))]{None}
}
#[cfg(test)]pub(crate) fn snapshot()->BTreeMap<&'static str,Sample>{SAMPLES.lock().unwrap().clone()}
