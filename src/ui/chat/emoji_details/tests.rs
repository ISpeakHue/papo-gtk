use super::*;
use actions::tests::{descendants,pump,until};
use adw::prelude::*;

fn choices()->Vec<emoji::Choice>{
    [("thumbsup","👍"),("woman","👩"),("family","👩‍👩‍👧‍👧"),("heart","❤️"),("flag_br","🇧🇷"),("smile","😄")].into_iter().map(|(name,text)|emoji::Choice{name:name.into(),text:text.into(),id:None}).collect()
}
#[test]fn unicode_sequences_have_whole_glyph_targets_and_character_offsets(){
    let index=UnicodeIndex::new(choices());
    let spans=index.spans("é 👍🏽 ❤️ 👩‍👩‍👧‍👧 🇧🇷 😄");
    assert_eq!(spans.iter().map(|(a,b,d)|(*a,*b,d.name.as_str())).collect::<Vec<_>>(),[(2,4,"thumbsup"),(5,7,"heart"),(8,15,"family"),(16,18,"flag_br"),(19,20,"smile")]);
    let Preview::Unicode(glyph)=&spans[0].2.preview else{panic!("Unicode glyph expected")};assert_eq!(glyph,"👍🏽");
    assert!(index.spans("hello :unknown:").is_empty());
    // Unknown joined sequences must not identify their first component as the whole emoji.
    assert_eq!(index.spans("👩🏽‍🦰 x 😄").iter().map(|(a,b,_)|(*a,*b)).collect::<Vec<_>>(),[(7,8)]);
    let spans=index.spans("👩‍🦰 😄");assert_eq!(spans.iter().map(|(a,b,d)|(*a,*b,d.name.as_str())).collect::<Vec<_>>(),[(4,5,"smile")]);
}

fn popup(view:&gtk::TextView)->Option<gtk::Popover>{
    // A text view's own popovers are parented directly to the view.
    let mut child=view.first_child();while let Some(widget)=child{child=widget.next_sibling();if let Ok(pop)=widget.downcast::<gtk::Popover>(){if pop.widget_name()=="emoji-details"&&pop.is_visible(){return Some(pop);}}}None
}
fn click(view:&gtk::TextView,offset:i32,drag:bool){
    let buffer=view.buffer();let rect=view.iter_location(&buffer.iter_at_offset(offset));let(x,y)=view.buffer_to_window_coords(gtk::TextWindowType::Widget,rect.x()+1,rect.y()+1);let(x,y)=(f64::from(x),f64::from(y));
    let controllers=view.observe_controllers();let click=(0..controllers.n_items()).find_map(|i|controllers.item(i).and_downcast::<gtk::GestureClick>().filter(|g|g.propagation_phase()==gtk::PropagationPhase::Capture)).unwrap();
    click.emit_by_name::<()>("pressed",&[&1i32,&x,&y]);click.emit_by_name::<()>("released",&[&1i32,&(x+if drag{30.0}else{0.0}),&y]);
}
fn label(popover:&gtk::Popover,name:&str)->String{
    descendants(popover.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Label>().ok().filter(|l|l.widget_name()==name)).unwrap().text().into()
}
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let custom=Uuid::new_v4();let animated=Uuid::new_v4();let mut actions=Actions::default();*actions.emoji_source.borrow_mut()="Papo <Test>".into();
    actions.emoji_names.insert("ameliawideson".into(),custom);actions.emoji_names.insert("dance".into(),animated);
    actions.textures.insert(custom,crate::media::PreparedImage{width:32,height:16,pixels:vec![180;32*16*4]}.texture());
    let animation=crate::media::animation::PreparedAnimation::decode(&crate::media::animation::fixture()).unwrap().paintable();actions.textures.insert(animated,animation.first());actions.animations.insert(animated,animation.clone());
    let value="é :ameliawideson: :dance: 😄 ||:dance:|| `:dance: 😄`";
    let view=text::widget(value,&HashMap::new(),&actions);let window=adw::Window::builder().title("Papo emoji details test").default_width(650).default_height(280).content(&view).build();window.present();until(context,||view.is_mapped());performance::settle(context);
    click(&view,2,false);until(context,||popup(&view).is_some_and(|p|p.is_mapped()&&p.width()>0));let pop=popup(&view).unwrap();
    assert_eq!(label(&pop,"emoji-details-name"),":ameliawideson:");assert_eq!(label(&pop,"emoji-details-source"),"Este emoji é do servidor Papo <Test>");
    assert!(pop.is_autohide());assert_eq!(pop.pointing_to().1.x(),view.iter_location(&view.buffer().iter_at_offset(2)).x());assert!(pop.width()<430&&pop.height()<150,"emoji info should be compact: {}×{}",pop.width(),pop.height());
    if std::env::var_os("PAPO_DESIGN_PREVIEW").is_some(){
        performance::settle(context);let paintable=gtk::WidgetPaintable::new(Some(&pop));let snapshot=gtk::Snapshot::new();paintable.snapshot(&snapshot,pop.width() as f64,pop.height() as f64);
        let node=snapshot.to_node().unwrap();pop.renderer().unwrap().render_texture(&node,Some(&gtk::graphene::Rect::new(0.0,0.0,pop.width() as f32,pop.height() as f32))).save_to_png("/tmp/papo-emoji-details-popover.png").unwrap();
    }
    let preview=descendants(pop.upcast_ref()).into_iter().find(|w|w.widget_name()=="emoji-details-preview").unwrap();let weak_preview=preview.downgrade();drop(preview);
    pop.popdown();until(context,||popup(&view).is_none()&&pop.parent().is_none());drop(pop);pump(context);assert!(weak_preview.upgrade().is_none(),"closed popovers must release their preview widgets");
    click(&view,2,true);pump(context);assert!(popup(&view).is_none(),"drag selection must not open emoji details");
    view.buffer().select_range(&view.buffer().start_iter(),&view.buffer().end_iter());click(&view,2,false);pump(context);assert!(popup(&view).is_none());view.buffer().place_cursor(&view.buffer().end_iter());
    // Source changes are read when opening, without rebuilding existing text.
    *actions.emoji_source.borrow_mut()="Renamed server".into();click(&view,4,false);until(context,||popup(&view).is_some());let pop=popup(&view).unwrap();assert_eq!(label(&pop,"emoji-details-name"),":dance:");assert!(label(&pop,"emoji-details-source").ends_with("Renamed server"));
    let image=descendants(pop.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Image>().ok()).unwrap();assert_eq!(image.paintable().unwrap(),animation.clone().upcast::<gtk::gdk::Paintable>());let first=animation.current_image();until(context,||animation.current_image()!=first);
    crate::ui::main_window::layout::tests::preview(&window,"emoji-details",context);
    if std::env::var_os("PAPO_NATIVE_INPUT_TEST").is_some(){multiline::keys_for("Papo emoji details test",&["Escape"]);}else{pop.popdown();}until(context,||popup(&view).is_none());drop(pop);drop(image);
    click(&view,6,false);until(context,||popup(&view).is_some());let pop=popup(&view).unwrap();assert_eq!(label(&pop,"emoji-details-source"),"Emoji Unicode");assert!(label(&pop,"emoji-details-name").starts_with(':'));pop.popdown();until(context,||popup(&view).is_none());
    // A hidden spoiler wins over its emoji target; a second click inspects the revealed emoji.
    click(&view,8,false);pump(context);assert!(popup(&view).is_none());click(&view,8,false);until(context,||popup(&view).is_some());let pop=popup(&view).unwrap();assert_eq!(label(&pop,"emoji-details-name"),":dance:");pop.popdown();until(context,||popup(&view).is_none());
    click(&view,12,false);pump(context);assert!(popup(&view).is_none(),"inline code must stay literal");
    click(&view,2,false);until(context,||popup(&view).is_some());let pop=popup(&view).unwrap();view.set_visible(false);until(context,||!pop.is_visible()&&pop.parent().is_none());
    window.set_content(None::<&gtk::Widget>);window.close();drop(pop);drop(view);drop(actions);drop(animation);pump(context);
}
