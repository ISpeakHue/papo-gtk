//! Local syntax grammars run on bounded workers; GTK only installs color tags.
use std::{cell::RefCell,collections::HashMap,rc::Rc,sync::{Arc,LazyLock,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use gtk::prelude::*;
use syntect::{highlighting::{HighlightIterator,HighlightState,Highlighter,ThemeSet},parsing::{ParseState,ScopeStack,SyntaxSet},util::LinesWithEndings};

static SYNTAXES:LazyLock<SyntaxSet>=LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEMES:LazyLock<ThemeSet>=LazyLock::new(ThemeSet::load_defaults);
static WORKERS:LazyLock<Arc<tokio::sync::Semaphore>>=LazyLock::new(||Arc::new(tokio::sync::Semaphore::new(2)));
const MAX_BYTES:usize=32<<10;
const MAX_SPANS:usize=4096;

pub(super) struct Block{pub language:String,pub text:String,pub start:i32}
#[derive(Debug,PartialEq,Eq)]struct Span{start:i32,end:i32,color:[u8;3]}
#[derive(Default)]struct Prepared{dark:Vec<Span>,light:Vec<Span>}

fn language(token:&str)->String{
    let token=token.split(',').next().unwrap_or("").to_ascii_lowercase();
    match token.as_str(){"rust"=>"rs","python"|"python3"=>"py","javascript"|"node"|"nodejs"=>"js","shell"|"bash"|"zsh"=>"sh","c++"=>"cpp","csharp"|"c#"=>"cs","yml"=>"yaml","plaintext"|"text"|"txt"=>"",_=>&token}.to_owned()
}
fn background(dark:bool)->[u8;3]{if dark{[0x29,0x29,0x2d]}else{[0xdd,0xdd,0xe1]}}
fn luminance(color:[u8;3])->f64{
    let linear=|v:u8|{let v=f64::from(v)/255.0;if v<=0.04045{v/12.92}else{((v+0.055)/1.055).powf(2.4)}};
    0.2126*linear(color[0])+0.7152*linear(color[1])+0.0722*linear(color[2])
}
fn contrast(a:[u8;3],b:[u8;3])->f64{let(a,b)=(luminance(a),luminance(b));(a.max(b)+0.05)/(a.min(b)+0.05)}
fn readable(mut color:[u8;3],dark:bool)->[u8;3]{
    let bg=background(dark);
    for _ in 0..32{if contrast(color,bg)>=4.5{break;}for channel in &mut color{*channel=if dark{channel.saturating_add(8)}else{channel.saturating_sub(8)};}}
    color
}
fn append(spans:&mut Vec<Span>,start:i32,end:i32,color:[u8;3]){
    if start==end{return;}
    if let Some(last)=spans.last_mut().filter(|s|s.end==start&&s.color==color){last.end=end;}else{spans.push(Span{start,end,color});}
}
fn prepare(blocks:&[Block],cancelled:&AtomicBool)->Option<Prepared>{
    if blocks.len()>32||blocks.iter().map(|b|b.text.len()).sum::<usize>()>MAX_BYTES{return None;}
    // Reject pathological long lines before loading grammars or invoking regexes.
    if blocks.iter().any(|b|b.text.lines().count()>512||b.text.lines().any(|line|line.len()>2048)){return None;}
    let syntaxes=&*SYNTAXES;let themes=&*THEMES;
    let dark=Highlighter::new(&themes.themes["base16-ocean.dark"]);
    let light=Highlighter::new(&themes.themes["InspiredGitHub"]);
    let deadline=Instant::now()+Duration::from_millis(500);let mut result=Prepared::default();
    for block in blocks{
        let token=language(&block.language);if token.is_empty(){continue;}
        let Some(syntax)=syntaxes.find_syntax_by_token(&token)else{continue;};
        let mut parser=ParseState::new(syntax);
        let mut dark_state=HighlightState::new(&dark,ScopeStack::new());let mut light_state=HighlightState::new(&light,ScopeStack::new());
        let mut offset=block.start;
        for line in LinesWithEndings::from(&block.text){
            if cancelled.load(Ordering::Relaxed)||Instant::now()>deadline{return None;}
            let ops=parser.parse_line(line,syntaxes).ok()?;
            // Parse once, derive both palettes so theme changes do not re-run grammars.
            for (is_dark,highlighter,state,spans) in [(true,&dark,&mut dark_state,&mut result.dark),(false,&light,&mut light_state,&mut result.light)]{
                let mut at=offset;
                for (style,part) in HighlightIterator::new(state,&ops,line,highlighter){
                    let end=at+part.chars().count() as i32;
                    append(spans,at,end,readable([style.foreground.r,style.foreground.g,style.foreground.b],is_dark));at=end;
                    if spans.len()>MAX_SPANS{return None;}
                }
            }
            offset+=line.chars().count() as i32;
        }
    }Some(result)
}
struct Render{
    buffer:gtk::TextBuffer,code_tags:Vec<gtk::TextTag>,color_tags:Vec<gtk::TextTag>,prepared:Prepared,
    cancelled:Arc<AtomicBool>,
}
impl Render{
    fn apply(&mut self){
        let manager=adw::StyleManager::default();let dark=manager.is_dark();let high_contrast=manager.is_high_contrast();
        let [r,g,b]=background(dark);let shade=format!("#{r:02x}{g:02x}{b:02x}");
        for tag in &self.code_tags{tag.set_background(if high_contrast{None}else{Some(&shade)});}
        let table=self.buffer.tag_table();for tag in self.color_tags.drain(..){table.remove(&tag);}
        if high_contrast{return;}
        let mut colors=HashMap::new();
        for span in if dark{&self.prepared.dark}else{&self.prepared.light}{
            if span.start<0||span.end>self.buffer.char_count(){continue;}
            let tag=colors.entry(span.color).or_insert_with(||{
                let [r,g,b]=span.color;let tag=gtk::TextTag::builder().name(format!("papo-syntax-{r:02x}{g:02x}{b:02x}")).foreground(format!("#{r:02x}{g:02x}{b:02x}")).build();table.add(&tag);
                // Spoilers/mentions keep priority over asynchronous syntax colors.
                tag.set_priority(0);tag
            });
            self.buffer.apply_tag(tag,&self.buffer.iter_at_offset(span.start),&self.buffer.iter_at_offset(span.end));
        }
        self.color_tags=colors.into_values().collect();
    }
}
pub(super) fn attach(view:&gtk::TextView,blocks:Vec<Block>,code_tags:Vec<gtk::TextTag>){
    let state=Rc::new(RefCell::new(Render{buffer:view.buffer(),code_tags,color_tags:vec![],prepared:Prepared::default(),cancelled:Arc::new(AtomicBool::new(false))}));
    state.borrow_mut().apply();let manager=adw::StyleManager::default();
    let weak=Rc::downgrade(&state);let dark=manager.connect_dark_notify(move |_|{if let Some(state)=weak.upgrade(){state.borrow_mut().apply();}});
    let weak=Rc::downgrade(&state);let contrast=manager.connect_high_contrast_notify(move |_|{if let Some(state)=weak.upgrade(){state.borrow_mut().apply();}});
    let weak=Rc::downgrade(&state);let cancelled=state.borrow().cancelled.clone();
    gtk::glib::spawn_future_local(async move{
        let Ok(permit)=WORKERS.clone().acquire_owned().await else{return;};if cancelled.load(Ordering::Relaxed){return;}
        let prepared=tokio::task::spawn_blocking(move||{let _permit=permit;prepare(&blocks,&cancelled)}).await;
        if let (Ok(Some(prepared)),Some(state))=(prepared,weak.upgrade()){let mut state=state.borrow_mut();state.prepared=prepared;state.apply();}
    });
    view.add_weak_ref_notify_local(move||{state.borrow().cancelled.store(true,Ordering::Relaxed);manager.disconnect(dark);manager.disconnect(contrast);});
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn languages_unicode_and_multiline_scopes_are_preserved(){
        for (token,code) in [("rust","fn main() {\n    let café = \"Olá 🦀\"; // comentário\n}\n"),("python","def hello():\n    return 'Olá'\n"),("js","const café = 42; // comentário\n"),("json","{\"café\": true}"),("sh","echo \"Olá\"\n")]{
            let start=7;let spans=prepare(&[Block{language:token.into(),text:code.into(),start}],&AtomicBool::new(false)).unwrap();
            for spans in [spans.dark,spans.light]{assert_eq!(spans.first().unwrap().start,start);assert_eq!(spans.last().unwrap().end,start+code.chars().count() as i32);assert!(spans.windows(2).all(|s|s[0].end==s[1].start));assert!(spans.iter().map(|s|s.color).collect::<std::collections::HashSet<_>>().len()>1,"{token}");}
        }
        let spans=prepare(&[Block{language:"rust".into(),text:"/* multi\nline */\nfn main() {}".into(),start:0}],&AtomicBool::new(false)).unwrap();assert_eq!(spans.dark[0].start,0);assert!(spans.dark[0].end>=16);
    }
    #[test]fn unknown_plain_oversized_and_cancelled_code_falls_back_safely(){
        for token in ["unknown-language","text","plaintext",""]{let result=prepare(&[Block{language:token.into(),text:"fn main() {}".into(),start:0}],&AtomicBool::new(false)).unwrap();assert!(result.dark.is_empty()&&result.light.is_empty());}
        for text in ["x".repeat(MAX_BYTES+1),"x".repeat(2049),"x\n".repeat(513)]{assert!(prepare(&[Block{language:"rs".into(),text,start:0}],&AtomicBool::new(false)).is_none());}
        assert!(prepare(&[Block{language:"rust".into(),text:"fn main() {}".into(),start:0}],&AtomicBool::new(true)).is_none());
        assert_eq!(language("Python3"),"py");assert_eq!(language("RUST,ignore"),"rs");
    }
    #[test]fn palette_colors_remain_readable_on_code_backgrounds(){
        for dark in [true,false]{for color in [[0,0,0],[255,255,255],[128,128,128],[255,0,0],[0,255,0],[0,0,255]]{assert!(contrast(readable(color,dark),background(dark))>=4.5);}}
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use super::{actions::{Actions,tests::{pump,until}},text};use adw::prelude::*;
    let manager=adw::StyleManager::default();let original=manager.color_scheme();manager.set_color_scheme(adw::ColorScheme::ForceDark);
    let source="Olá 🦀 **Markdown**\n\n```rust\nfn main() {\n    let café = \"Olá 🦀\"; // comentário\n    println!(\"{café}\");\n}\n```\n\n```python\ndef hello():\n    return 'Olá'\n```\n\n```not-a-language\nliteral <script> :dance: @mention(<@id>)\n```\n\n`inline code`\n\n||\n\n```rs\nlet secret = 42;\n```\n\n||";
    // Holding the slots proves GTK can build/display code without waiting on grammars.
    let held=WORKERS.clone().try_acquire_many_owned(2).unwrap();
    let view=text::widget(source,&HashMap::new(),&Actions::default());let buffer=view.buffer();
    let window=adw::Window::builder().title("Syntax highlighting").default_width(720).default_height(700).content(&view).build();window.present();pump(context);
    let rendered=buffer.text(&buffer.start_iter(),&buffer.end_iter(),true).to_string();
    let offset=|needle:&str|rendered[..rendered.find(needle).unwrap()].chars().count() as i32;
    let syntax_at=|at:i32|buffer.iter_at_offset(at).tags().into_iter().find(|tag|tag.name().is_some_and(|n|n.starts_with("papo-syntax-")));
    assert!(view.is_mapped());assert!(syntax_at(offset("fn main")).is_none());
    assert!(rendered.contains("literal <script> :dance: @mention(<@id>)"));
    drop(held);until(context,||syntax_at(offset("fn main")).is_some());
    let keyword=syntax_at(offset("fn main")).unwrap();let string=syntax_at(offset("Olá 🦀\";")).unwrap();assert_ne!(keyword.foreground_rgba(),string.foreground_rgba());
    assert!(syntax_at(offset("def hello")).is_some());assert!(syntax_at(offset("literal <script>")).is_none());assert!(syntax_at(offset("inline code")).is_none());assert!(syntax_at(offset("Markdown")).is_none());
    let foreground=||buffer.iter_at_offset(offset("secret")).tags().into_iter().filter(|t|t.is_foreground_set()).max_by_key(|t|t.priority()).unwrap();
    assert!(foreground().is_background_set(),"syntax must not reveal a code spoiler");
    let dark_color=keyword.foreground_rgba();manager.set_color_scheme(adw::ColorScheme::ForceLight);pump(context);let light_color=syntax_at(offset("fn main")).unwrap().foreground_rgba();assert_ne!(dark_color,light_color);
    assert_eq!(buffer.text(&buffer.start_iter(),&buffer.end_iter(),true),rendered,"color updates must preserve exact selectable text");assert!(foreground().is_background_set());
    let controllers=view.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>()).unwrap();
    super::performance::settle(context);let rect=view.iter_location(&buffer.iter_at_offset(offset("secret")));let(x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x()+1,rect.y()+1);let(x,y)=(f64::from(x),f64::from(y));
    click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&x,&y]);pump(context);assert!(!foreground().is_background_set(),"revealing the spoiler restores syntax colors");
    // Readable colors survive changes without altering the clipboard payload.
    buffer.select_range(&buffer.start_iter(),&buffer.end_iter());buffer.copy_clipboard(&view.clipboard());assert_eq!(context.block_on(view.clipboard().read_text_future()).unwrap().unwrap().as_str(),rendered);
    buffer.place_cursor(&buffer.start_iter());
    super::super::main_window::layout::tests::preview(&window,"syntax-highlighting",context);
    let weak=view.downgrade();drop(click);drop(controllers);window.set_content(None::<&gtk::Widget>);window.close();drop(window);drop(view);until(context,||weak.upgrade().is_none());
    // A row discarded while queued must not receive a late result or retain its view.
    let held=WORKERS.clone().try_acquire_many_owned(2).unwrap();let temporary=text::widget("```rust\nfn gone() {}\n```",&HashMap::new(),&Actions::default());let weak=temporary.downgrade();drop(temporary);pump(context);assert!(weak.upgrade().is_none());drop(held);
    manager.set_color_scheme(original);pump(context);
}
