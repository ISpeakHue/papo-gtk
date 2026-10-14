//! CommonMark/GFM events become native text runs, never HTML or remote widgets.
use pulldown_cmark::{Event,Options,Parser,Tag,TagEnd};

#[derive(Clone,Debug,Default,PartialEq,Eq,Hash)]
pub(super) struct Style {
    pub bold:bool,pub italic:bool,pub underline:bool,pub strike:bool,pub code:bool,
    pub heading:u8,pub quote:usize,pub table:bool,pub spoiler:bool,pub spoiler_group:usize,pub link:Option<String>,
}
#[derive(Clone,Debug,PartialEq,Eq)]pub(super) struct Run {pub text:String,pub style:Style}
#[derive(Default)]struct Document {runs:Vec<Run>}
impl Document {
    fn push(&mut self,text:&str,style:&Style){
        if text.is_empty(){return;}
        if let Some(last)=self.runs.last_mut().filter(|r|r.style==*style){last.text.push_str(text);}
        else{self.runs.push(Run{text:text.into(),style:style.clone()});}
    }
    fn newline(&mut self,count:usize,style:&Style){
        if self.runs.is_empty(){return;}
        let present=self.runs.iter().rev().flat_map(|r|r.text.chars().rev()).take_while(|c|*c=='\n').count();
        for _ in present..count{self.push("\n",style);}
    }
}
pub(super) fn parse(source:&str)->Vec<Run>{
    let options=Options::ENABLE_TABLES|Options::ENABLE_TASKLISTS|Options::ENABLE_STRIKETHROUGH|Options::ENABLE_FOOTNOTES|Options::ENABLE_GFM;
    let mut doc=Document::default();let mut style=Style::default();let mut stack=Vec::new();let mut lists=Vec::<Option<u64>>::new();
    for (event,range) in Parser::new_ext(source,options).into_offset_iter(){match event{
        Event::Start(tag)=>{
            stack.push(style.clone());
            match tag{
                Tag::Heading{level,..}=>{doc.newline(2,&style);style.heading=level as u8;style.bold=true;},
                Tag::BlockQuote(_)=>{doc.newline(1,&style);style.quote+=1;},
                Tag::CodeBlock(_)=>{doc.newline(1,&style);style.code=true;},
                Tag::Strong=>{if source[range].starts_with("__"){style.underline=true;}else{style.bold=true;}},
                Tag::Emphasis=>style.italic=true,Tag::Strikethrough=>style.strike=true,
                Tag::Link{dest_url,..}=>{style.link=super::rich::safe_url(&dest_url).map(|u|u.to_string());},
                Tag::Image{..}=>{}, // Alt text only: backend embeds own media fetching.
                Tag::List(start)=>{doc.newline(1,&style);lists.push(start);},
                Tag::Item=>{
                    doc.newline(1,&style);doc.push(&"  ".repeat(lists.len().saturating_sub(1).min(16)),&style);
                    let marker=match lists.last_mut(){Some(Some(number))=>{let text=format!("{number}. ");*number=number.saturating_add(1);text},_=>"• ".into()};doc.push(&marker,&style);
                },
                Tag::Table(_)=>{doc.newline(1,&style);style.table=true;},
                Tag::TableHead=>{style.bold=true;doc.push("│ ",&style);},Tag::TableRow=>doc.push("│ ",&style),
                Tag::FootnoteDefinition(name)=>{doc.newline(1,&style);doc.push(&format!("[{name}] "),&style);},
                _=>{},
            }
        },
        Event::End(tag)=>{
            match tag{
                TagEnd::Paragraph|TagEnd::Heading(_)=>doc.newline(if lists.is_empty(){2}else{1},&style),
                TagEnd::Item|TagEnd::CodeBlock|TagEnd::BlockQuote(_)|TagEnd::FootnoteDefinition|TagEnd::TableHead|TagEnd::TableRow=>doc.newline(1,&style),
                TagEnd::List(_)=>{lists.pop();doc.newline(1,&style);},
                TagEnd::TableCell=>doc.push(" │ ",&style),TagEnd::Table=>doc.newline(1,&style),_=>{},
            }
            style=stack.pop().unwrap_or_default();
        },
        Event::Text(text)|Event::Html(text)|Event::InlineHtml(text)=>doc.push(&text,&style),
        Event::Code(text)=>{let mut code=style.clone();code.code=true;code.link=None;doc.push(&text,&code);},
        Event::SoftBreak|Event::HardBreak=>doc.push("\n",&style),
        Event::Rule=>{doc.newline(1,&style);doc.push("────────────",&style);doc.newline(1,&style);},
        Event::TaskListMarker(done)=>doc.push(if done{"☑ "}else{"☐ "},&style),
        Event::FootnoteReference(name)=>doc.push(&format!("[{name}]"),&style),
        Event::InlineMath(text)|Event::DisplayMath(text)=>doc.push(&text,&style),
    }}
    while let Some(last)=doc.runs.last_mut(){last.text.truncate(last.text.trim_end_matches('\n').len());if !last.text.is_empty(){break;}doc.runs.pop();}
    // Discord-style spoilers are deliberately kept out of code runs.
    let mut remaining=doc.runs.iter().filter(|r|!r.style.code).map(|r|r.text.matches("||").count()).sum::<usize>();
    let mut output=Document::default();let mut hidden=false;let mut group=0;
    for run in doc.runs{
        if run.style.code{let mut style=run.style;style.spoiler=hidden;style.spoiler_group=if hidden{group}else{0};output.push(&run.text,&style);continue;}
        let mut start=0;
        for (at,_) in run.text.match_indices("||"){
            let mut style=run.style.clone();style.spoiler=hidden;style.spoiler_group=if hidden{group}else{0};output.push(&run.text[start..at],&style);
            if hidden||remaining>=2{hidden=!hidden;if hidden{group+=1;}}else{output.push("||",&style);}remaining=remaining.saturating_sub(1);start=at+2;
        }
        let mut style=run.style;style.spoiler=hidden;style.spoiler_group=if hidden{group}else{0};output.push(&run.text[start..],&style);
    }
    output.runs
}
pub(super) fn plain(source:&str)->String{parse(source).into_iter().map(|r|r.text).collect()}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn nested_unicode_formatting_and_discord_extensions_keep_exact_text(){
        let runs=parse("# Título\n\n**forte _é_** __sublinhado__ ~~riscado~~ ||oculto||");let text=runs.iter().map(|r|r.text.as_str()).collect::<String>();
        assert_eq!(text,"Título\n\nforte é sublinhado riscado oculto");assert_eq!(runs[0].style.heading,1);
        assert!(runs.iter().any(|r|r.text.contains('é')&&r.style.bold&&r.style.italic));assert!(runs.iter().any(|r|r.text.contains("sublinhado")&&r.style.underline));
        assert!(runs.iter().any(|r|r.text.contains("riscado")&&r.style.strike));assert!(runs.iter().any(|r|r.text=="oculto"&&r.style.spoiler));
    }
    #[test]fn lists_quotes_tables_and_footnotes_preserve_content(){
        let text=plain("> Quote\n\n3. Three\n4. Four\n\n- [x] Done\n- [ ] Todo\n\n| A | B |\n|---|---|\n| á | é |\n\nFoot[^1]\n\n[^1]: Note");
        for expected in ["Quote","3. Three","4. Four","☑ Done","☐ Todo","│ A │ B │","│ á │ é │","Foot[1]","[1] Note"]{assert!(text.contains(expected),"{expected}: {text}");}
        assert!(parse("> Quote").iter().any(|r|r.style.quote>0));
    }
    #[test]fn code_html_images_and_unsafe_links_never_become_active_content(){
        let source="`**literal** :dance: @someone`\n\n```rust\n||literal|| <script> :dance:\n```\n\n[unsafe](javascript:alert(1)) ![alt](https://example.test/image.png) <b>literal</b>";
        let runs=parse(source);assert!(runs.iter().any(|r|r.style.code&&r.text.contains("**literal** :dance: @someone")));
        assert!(runs.iter().any(|r|r.style.code&&r.text.contains("||literal|| <script> :dance:")));assert!(runs.iter().all(|r|r.style.link.is_none()));
        let text=plain(source);assert!(text.contains("unsafe alt <b>literal</b>"));assert!(!text.contains("image.png"));
        assert_eq!(plain("\\*literal\\* &amp; incomplete **"),"*literal* & incomplete **");
    }
    #[test]fn wire_mentions_survive_parser_event_boundaries(){let token=format!("@mention(<@{}>)",uuid::Uuid::new_v4());assert_eq!(plain(&format!("**{token}**")),token);}
}
