//! EN-US: Reuse recent text rows, without retaining players, GIF timers or large media textures.
//! PT-BR: Reutiliza linhas recentes de texto sem reter reprodutores, temporizadores de GIF ou grandes texturas.
use super::*;
use std::collections::VecDeque;
type Rows=HashMap<String,(String,gtk::ListBoxRow)>;
#[derive(Default)]
pub(super) struct Surfaces{entries:VecDeque<(Uuid,Rows,window::RenderWindow,usize)>}
impl Surfaces{
    pub fn store(&mut self,id:Uuid,mut rows:Rows,mut window:window::RenderWindow,list:&gtk::ListBox,recent:&std::collections::HashSet<Uuid>){
        self.invalidate(id);
        // Players have been shut down by Transfers before any widget unmaps.
        // Cached rows keep only text and controls. Media is mounted from the
        // existing bounded media cache when the channel is selected again.
        rows.retain(|key,_|key.strip_prefix("message-").and_then(|id|Uuid::parse_str(id).ok()).is_some_and(|id|recent.contains(&id))||key.starts_with("date-"));
        window.retain_ids(recent);
        let bytes=rows.iter().map(|(key,(sig,_))|key.capacity()+sig.capacity()).sum::<usize>();
        if rows.len()>210||bytes>2<<20{while let Some(child)=list.first_child(){list.remove(&child);}return;}
        for (_,row) in rows.values(){
            if let Some(media)=find_media_box(row.upcast_ref()){while let Some(child)=media.first_child(){media.remove(&child);}}
            release_images(row.upcast_ref());
        }
        // Invalidate media render nodes while rows are still mapped, before
        // unparenting them; this also releases cached snapshot texture refs.
        while let Some(child)=list.first_child(){list.remove(&child);}
        self.entries.push_back((id,rows,window,bytes));
        while self.entries.len()>2||self.entries.iter().map(|e|e.3).sum::<usize>()>4<<20{self.entries.pop_front();}
    }
    pub fn take(&mut self,id:Uuid)->Option<(Rows,window::RenderWindow)>{let index=self.entries.iter().position(|e|e.0==id)?;let (_,rows,window,_)=self.entries.remove(index)?;Some((rows,window))}
    pub fn invalidate(&mut self,id:Uuid){self.entries.retain(|e|e.0!=id);}
    pub fn clear(&mut self){self.entries.clear();}
    pub fn retain(&mut self,ids:&[Uuid]){self.entries.retain(|e|ids.contains(&e.0));}
}
fn release_images(widget:&gtk::Widget){
    if let Some(avatar)=widget.downcast_ref::<adw::Avatar>(){avatar.set_custom_image(None::<&gtk::gdk::Paintable>);}
    if let Some(popover)=widget.downcast_ref::<gtk::Popover>(){popover.popdown();}
    let mut child=widget.first_child();while let Some(w)=child{child=w.next_sibling();release_images(&w);}
}
