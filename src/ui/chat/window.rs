//! Bound GTK row ownership while keeping a scrollable, variable-height history.
use super::*;
pub(super) const ROW_LIMIT:usize=200;
const ESTIMATE:f64=64.0;
#[derive(Default)]
pub(super) struct RenderWindow{
    pub range:std::ops::Range<usize>,
    heights:HashMap<Uuid,f64>,
}
impl RenderWindow{
    pub fn measure(&mut self,list:&gtk::ListBox){
        let mut row=list.first_child();while let Some(w)=row{row=w.next_sibling();if let Some(id)=w.widget_name().strip_prefix("message-").and_then(|s|Uuid::parse_str(s).ok()){
            if w.height()>0{self.heights.insert(id,f64::from(w.height()));}
        }}
    }
    pub fn select(&mut self,messages:&[Message],position:&Position,value:f64,page:f64)->bool{
        let count=messages.len();
        let center=match position{
            Position::Bottom=>count.saturating_sub(1),
            Position::Message(id)|Position::Unread(id)|Position::Anchor{id:Some(id),..}=>messages.iter().position(|m|m.id==*id).unwrap_or(count.saturating_sub(1)),
            _=>{let mut y=0.0;messages.iter().position(|m|{y+=self.height(m);y>=value+page/2.0}).unwrap_or(count.saturating_sub(1))}
        };
        // Small reading movements and unrelated live updates keep the existing
        // materialized rows. Recenter only when nearing the window's edge;
        // explicit navigation and following the newest messages still recenter.
        let keep=matches!(position,Position::Anchor{..})&&self.range.end<=count&&self.range.len()==ROW_LIMIT
            &&center>=self.range.start+ROW_LIMIT/4&&center<self.range.end-ROW_LIMIT/4;
        let start=if keep{self.range.start}else{center.saturating_sub(ROW_LIMIT/2).min(count.saturating_sub(ROW_LIMIT))};
        let range=start..(start+ROW_LIMIT).min(count);let changed=self.range!=range;self.range=range;
        let ids:std::collections::HashSet<_>=messages.iter().map(|m|m.id).collect();self.heights.retain(|id,_|ids.contains(id));changed
    }
    fn height(&self,m:&Message)->f64{self.heights.get(&m.id).copied().unwrap_or(ESTIMATE)}
    pub fn spacer(&self,messages:&[Message],before:bool)->i32{
        let slice=if before{&messages[..self.range.start.min(messages.len())]}else{&messages[self.range.end.min(messages.len())..]};
        slice.iter().map(|m|self.height(m)).sum::<f64>().min(f64::from(i32::MAX/2)) as i32
    }
    pub fn needs_shift(&self,list:&gtk::ListBox,scroll:&gtk::ScrolledWindow,messages:&[Message])->bool{
        if messages.len()<=ROW_LIMIT{return false;}
        let adj=scroll.vadjustment();let top=adj.value();let bottom=top+adj.page_size();
        let mut first=None;let mut last=None;let mut row=list.first_child();
        while let Some(w)=row{row=w.next_sibling();if w.widget_name().starts_with("message-"){if let Some(b)=w.compute_bounds(list){first.get_or_insert(f64::from(b.y()));last=Some(f64::from(b.y()+b.height()));}}}
        (self.range.start>0&&first.is_some_and(|y|top<y+adj.page_size()))||(self.range.end<messages.len()&&last.is_some_and(|y|bottom>y-adj.page_size()))
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn large_history_keeps_bounded_windows_and_can_navigate_both_ends(){
        let channel=Uuid::new_v4();let messages:Vec<Message>=(0..5000).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":chrono::DateTime::from_timestamp(i,0),"content":"hello"})).unwrap()).collect();
        let mut window=RenderWindow::default();window.select(&messages,&Position::Bottom,0.0,600.0);assert_eq!(window.range,4800..5000);assert_eq!(window.spacer(&messages,false),0);
        window.select(&messages,&Position::Message(messages[10].id),0.0,600.0);assert_eq!(window.range,0..200);assert!(window.spacer(&messages,false)>0);
    }
    #[test]fn reading_updates_keep_the_window_until_its_edge(){
        let channel=Uuid::new_v4();let messages:Vec<Message>=(0..1000).map(|i|serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"channel_id":channel,"created_at":chrono::DateTime::from_timestamp(i,0)})).unwrap()).collect();
        let mut window=RenderWindow::default();window.select(&messages,&Position::Message(messages[500].id),0.0,600.0);let original=window.range.clone();
        assert!(!window.select(&messages,&Position::Anchor{id:Some(messages[510].id),offset:0.0,value:0.0},0.0,600.0));assert_eq!(window.range,original);
        assert!(window.select(&messages,&Position::Anchor{id:Some(messages[570].id),offset:0.0,value:0.0},0.0,600.0));assert!(window.range.contains(&570));assert_eq!(window.range.len(),ROW_LIMIT);
        window.select(&messages,&Position::Bottom,0.0,600.0);assert_eq!(window.range.end,messages.len());
    }
}
