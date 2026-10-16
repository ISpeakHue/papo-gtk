//! Opt-in aggregate diagnostics. Labels are fixed in source, never user data.
use std::{collections::BTreeMap,sync::{LazyLock,Mutex},time::Instant};
static ENABLED:LazyLock<bool>=LazyLock::new(||std::env::var("PAPO_PERFORMANCE").as_deref()==Ok("1"));
#[derive(Default)]struct Sample{count:u64,total:u64,max:u64}
static SAMPLES:LazyLock<Mutex<BTreeMap<&'static str,Sample>>>=LazyLock::new(Mutex::default);
pub fn enabled()->bool{*ENABLED}
pub fn record(label:&'static str,value:u64){if !enabled(){return;}if let Ok(mut samples)=SAMPLES.lock(){let sample=samples.entry(label).or_default();sample.count+=1;sample.total=sample.total.saturating_add(value);sample.max=sample.max.max(value);}}
pub struct Timer{label:&'static str,start:Option<Instant>}
pub fn timer(label:&'static str)->Timer{Timer{label,start:enabled().then(Instant::now)}}
impl Drop for Timer{fn drop(&mut self){if let Some(start)=self.start{record(self.label,start.elapsed().as_micros().min(u64::MAX as u128) as u64);}}}
pub fn report(){if !enabled(){return;}if let Ok(samples)=SAMPLES.lock(){let rows:BTreeMap<_,_>=samples.iter().map(|(key,s)|(key,serde_json::json!({"samples":s.count,"total":s.total,"max":s.max}))).collect();eprintln!("Papo aggregate performance: {}",serde_json::to_string(&rows).unwrap_or_default());}}

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
