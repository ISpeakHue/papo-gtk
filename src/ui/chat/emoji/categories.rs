//! EN-US: Unicode categories come from GTK's bundled CLDR catalog, including aliases.
//! PT-BR: Categorias Unicode obtidas do catálogo CLDR do GTK, incluindo nomes alternativos.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui::chat) enum Category {
    Smileys, People, Nature, Food, Travel, Activities, Objects, Symbols, Flags,
}
impl Category {
    const ALL: [Self;9] = [Self::Smileys,Self::People,Self::Nature,Self::Food,Self::Travel,Self::Activities,Self::Objects,Self::Symbols,Self::Flags];
    pub(super) fn from_gtk(group:u32)->Self {
        match group {
            0=>Self::Smileys,1|2=>Self::People,3=>Self::Nature,4=>Self::Food,
            5=>Self::Travel,6=>Self::Activities,7=>Self::Objects,9=>Self::Flags,_=>Self::Symbols,
        }
    }
    pub(super) fn alias(name:&str)->Self {
        match name {
            "+1"|"-1"|"thumbsup"|"thumbsdown"|"wave"|"eyes"|"ok_hand"|"clap"|"pray"|"muscle"=>Self::People,
            "fire"|"rocket"=>Self::Travel,"tada"=>Self::Activities,
            "warning"|"white_check_mark"|"x"=>Self::Symbols,_=>Self::Smileys,
        }
    }
    pub(super) fn title(self)->&'static str {
        match self {
            Self::Smileys=>"Carinhas e emoções",Self::People=>"Pessoas e corpo",Self::Nature=>"Animais e natureza",
            Self::Food=>"Comidas e bebidas",Self::Travel=>"Viagens e lugares",Self::Activities=>"Atividades",
            Self::Objects=>"Objetos",Self::Symbols=>"Símbolos",Self::Flags=>"Bandeiras",
        }
    }
    fn icon(self)->&'static str {
        match self {Self::Smileys=>"😀",Self::People=>"👋",Self::Nature=>"🐱",Self::Food=>"🍔",Self::Travel=>"🚗",Self::Activities=>"⚽",Self::Objects=>"💡",Self::Symbols=>"💯",Self::Flags=>"🏳️"}
    }
    fn name(self)->&'static str {
        match self {Self::Smileys=>"smileys",Self::People=>"people",Self::Nature=>"nature",Self::Food=>"food",Self::Travel=>"travel",Self::Activities=>"activities",Self::Objects=>"objects",Self::Symbols=>"symbols",Self::Flags=>"flags"}
    }
}
pub(super) fn navigation()->(gtk::ScrolledWindow,Vec<(Option<Category>,gtk::ToggleButton)>) {
    let bar=gtk::Box::new(gtk::Orientation::Horizontal,0);bar.set_widget_name("emoji-categories");
    let all=gtk::ToggleButton::new();all.set_icon_name("view-grid-symbolic");all.set_active(true);
    let mut buttons=vec![(None,all.clone())];
    for category in Category::ALL {
        let button=gtk::ToggleButton::with_label(category.icon());button.set_group(Some(&all));
        buttons.push((Some(category),button));
    }
    for (category,button) in &buttons {
        let title=category.map_or("Todos os emojis",Category::title);
        button.set_tooltip_text(Some(title));button.update_property(&[gtk::accessible::Property::Label(title)]);
        button.set_widget_name(&format!("emoji-category-{}",category.map_or("all",Category::name)));
        button.add_css_class("flat");button.add_css_class("papo-emoji-category");bar.append(button);
    }
    let scroll=gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Automatic).vscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(48).propagate_natural_height(true).child(&bar).build();
    (scroll,buttons)
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn gtk_groups_include_components_and_keep_unicode_aliases_consistent(){
        assert_eq!(Category::from_gtk(2),Category::People);
        assert_eq!(Category::from_gtk(8),Category::Symbols);
        assert_eq!(Category::from_gtk(9),Category::Flags);
        let catalog=builtin();
        for (name,category) in [("smile",Category::Smileys),("thumbsup",Category::People),("tada",Category::Activities),("fire",Category::Travel),("rocket",Category::Travel),("warning",Category::Symbols)] {
            assert_eq!(catalog.iter().find(|c|c.name==name).unwrap().category,Some(category));
        }
        for choice in &catalog {assert!(choice.category.is_some());assert!(catalog.iter().filter(|c|c.text==choice.text).all(|c|c.category==choice.category),"aliases must stay in the same category");}
    }
}

#[cfg(test)]pub(crate) fn exercise(context:&gtk::glib::MainContext){
    use actions::tests::{descendants,pump,until};use adw::prelude::*;
    let root=gtk::Box::new(gtk::Orientation::Vertical,8);
    for set in [gtk::Widget::set_margin_top,gtk::Widget::set_margin_bottom,gtk::Widget::set_margin_start,gtk::Widget::set_margin_end]{set(root.upcast_ref(),16);}
    let catalog=builtin();assert!(catalog.len()>100);
    for category in Category::ALL {assert!(catalog.iter().any(|c|c.category==Some(category)),"missing {category:?}");}
    let chosen=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));let selected=chosen.clone();
    let picker=Picker::new(&root,&catalog,move |choice|selected.borrow_mut().push(choice));
    let custom:crate::models::Emoji=serde_json::from_value(serde_json::json!({"id":Uuid::new_v4(),"name":"server_test","format":"PNG","created_at":"2026-10-16T12:00:00Z"})).unwrap();
    picker.update("Test server",&[custom.clone()],&HashMap::new(),&HashMap::new());
    let window=adw::Window::builder().default_width(400).default_height(500).content(&root).build();window.present();performance::settle(context);
    let widgets=descendants(root.upcast_ref());
    let tabs=widgets.iter().find_map(|w|w.downcast_ref::<gtk::Stack>()).unwrap();
    let unicode=tabs.visible_child().unwrap();
    let unicode_widgets=descendants(&unicode);
    let grid=unicode_widgets.iter().find_map(|w|w.downcast_ref::<gtk::FlowBox>()).unwrap();
    let search=widgets.iter().find_map(|w|w.downcast_ref::<gtk::SearchEntry>()).unwrap();
    let title=unicode_widgets.iter().find_map(|w|w.downcast_ref::<gtk::Label>().filter(|l|l.widget_name()=="emoji-category-title")).unwrap();
    let button=|name:&str|unicode_widgets.iter().find_map(|w|w.downcast_ref::<gtk::ToggleButton>().filter(|b|b.widget_name()==format!("emoji-category-{name}"))).unwrap();
    let displayed=||descendants(grid.upcast_ref()).into_iter().filter_map(|w|w.downcast::<gtk::Button>().ok().and_then(|b|b.tooltip_text().map(|s|s.trim_matches(':').to_string()))).collect::<Vec<_>>();
    for category in Category::ALL {
        button(category.name()).set_active(true);performance::settle(context);
        assert_eq!(title.text(),category.title());
        let names=displayed();assert!(!names.is_empty());
        assert!(names.iter().all(|name|catalog.iter().any(|c|c.name==*name&&c.category==Some(category))),"category must show only its own emojis");
    }
    button("nature").set_active(true);performance::settle(context);
    let first=grid.first_child().unwrap();
    picker.update("Renamed server",&[custom],&HashMap::new(),&HashMap::new());pump(context);
    assert!(button("nature").is_active());assert_eq!(grid.first_child().unwrap(),first,"API updates preserve category and cells");
    search.set_text("rocket");until(context,||title.text()=="Resultados da busca");
    assert!(displayed().iter().any(|name|name=="rocket"),"search spans categories");
    let rocket=descendants(grid.upcast_ref()).into_iter().find_map(|w|w.downcast::<gtk::Button>().ok().filter(|b|b.tooltip_text().as_deref()==Some(":rocket:"))).unwrap();
    rocket.emit_clicked();assert_eq!(chosen.borrow().last().unwrap().text,"🚀");
    button("food").set_active(true);performance::settle(context);assert!(search.text().is_empty());assert_eq!(title.text(),Category::Food.title());
    search.set_text("no-such-emoji-293848");until(context,||grid.first_child().is_none());
    assert!(descendants(&unicode).iter().any(|w|w.downcast_ref::<gtk::Label>().is_some_and(|l|l.is_visible()&&l.text()=="Nenhum emoji encontrado")));
    search.set_text("");until(context,||grid.first_child().is_some());
    tabs.set_visible_child_name("server");performance::settle(context);
    assert!(!button("food").is_mapped());assert!(descendants(&tabs.visible_child().unwrap()).iter().any(|w|w.downcast_ref::<gtk::Button>().is_some_and(|b|b.tooltip_text().as_deref()==Some(":server_test:"))));
    tabs.set_visible_child_name("unicode");performance::settle(context);assert!(button("food").is_active());
    window.set_default_size(320,500);performance::settle(context);
    let scroll=grid.ancestor(gtk::ScrolledWindow::static_type()).and_downcast::<gtk::ScrolledWindow>().unwrap();
    assert!(scroll.hadjustment().upper()<=scroll.hadjustment().page_size()+1.0);
    crate::ui::main_window::layout::tests::preview(&window,"emoji-categories",context);
    window.set_content(None::<&gtk::Widget>);window.close();drop(picker);pump(context);
}
