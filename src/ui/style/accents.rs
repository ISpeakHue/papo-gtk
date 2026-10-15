//! One CSS provider per display, shared by the colors of live embed widgets.
use std::{cell::RefCell, collections::BTreeMap, rc::{Rc, Weak}};
use gtk::prelude::*;

const MAX_COLORS: usize = 1024;
const PREFIX: &str = "papo-embed-accent-";
thread_local! {
    static PALETTES: RefCell<Vec<Weak<RefCell<Palette>>>> = const { RefCell::new(Vec::new()) };
}

struct Palette {
    display: gtk::glib::WeakRef<gtk::gdk::Display>,
    provider: gtk::CssProvider,
    colors: BTreeMap<u32, usize>,
    rendered: Vec<u32>,
    pending: Option<gtk::glib::SourceId>,
    installed: bool,
    #[cfg(test)] loads: usize,
}
impl Drop for Palette {
    fn drop(&mut self) {
        if self.installed {
            if let Some(display) = self.display.upgrade() {
                gtk::style_context_remove_provider_for_display(&display, &self.provider);
            }
        }
    }
}
fn color(value: &str) -> Option<u32> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) { return None; }
    u32::from_str_radix(hex, 16).ok()
}
fn palette(display: &gtk::gdk::Display) -> Rc<RefCell<Palette>> {
    PALETTES.with(|palettes| {
        let mut palettes = palettes.borrow_mut();
        palettes.retain(|p| p.strong_count() != 0);
        if let Some(palette) = palettes.iter().filter_map(Weak::upgrade)
            .find(|p| p.borrow().display.upgrade().as_ref() == Some(display)) {
            return palette;
        }
        let palette = Rc::new(RefCell::new(Palette {
            display: display.downgrade(), provider: gtk::CssProvider::new(),
            colors: BTreeMap::new(), rendered: Vec::new(), pending: None, installed: false,
            #[cfg(test)] loads: 0,
        }));
        palettes.push(Rc::downgrade(&palette));
        palette
    })
}
fn schedule(palette: &Rc<RefCell<Palette>>) {
    if palette.borrow().pending.is_some() { return; }
    let current = palette.clone();
    // Run before GTK's resize/redraw idles: newly mounted cards get their
    // color on their first frame, while all changes in this batch share a load.
    let source = gtk::glib::idle_add_local_full(gtk::glib::Priority::HIGH_IDLE, move || {
        let mut palette = current.borrow_mut();
        palette.pending = None;
        let colors: Vec<_> = palette.colors.keys().copied().collect();
        if colors == palette.rendered { return gtk::glib::ControlFlow::Break; }
        let Some(display) = palette.display.upgrade() else { return gtk::glib::ControlFlow::Break; };
        if colors.is_empty() {
            if palette.installed {
                gtk::style_context_remove_provider_for_display(&display, &palette.provider);
                palette.installed = false;
            }
        } else {
            let css: String = colors.iter().map(|color| format!(
                ".papo-link-preview.{PREFIX}{color:06x} {{ border-left-color: #{color:06x}; }}\n"
            )).collect();
            palette.provider.load_from_string(&css);
            #[cfg(test)] { palette.loads += 1; }
            if !palette.installed {
                gtk::style_context_add_provider_for_display(&display, &palette.provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
                palette.installed = true;
            }
        }
        palette.rendered = colors;
        gtk::glib::ControlFlow::Break
    });
    palette.borrow_mut().pending = Some(source);
}

/// Apply once to a new card. Invalid colors, or an exhausted palette, keep the
/// theme's accent. The cap bounds stylesheet size even for adversarial history.
pub(in crate::ui) fn apply(card: &gtk::Box, value: Option<&str>) {
    let Some(color) = value.and_then(color) else { return; };
    if card.css_classes().iter().any(|c| c.starts_with(PREFIX)) { return; }
    let palette = palette(&card.display());
    {
        let mut state = palette.borrow_mut();
        if !state.colors.contains_key(&color) && state.colors.len() >= MAX_COLORS { return; }
        *state.colors.entry(color).or_default() += 1;
    }
    card.add_css_class(&format!("{PREFIX}{color:06x}"));
    schedule(&palette);
    // No widget capture: the callback owns its palette lease, never the card.
    // GLib invokes it on final disposal, including when cached rows are evicted.
    card.add_weak_ref_notify_local(move || {
        {
            let mut state = palette.borrow_mut();
            if let Some(uses) = state.colors.get_mut(&color) {
                *uses -= 1;
                if *uses == 0 { state.colors.remove(&color); }
            }
        }
        schedule(&palette);
    });
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn colors_are_canonical_and_never_interpreted_as_css() {
        assert_eq!(color("#12aBcF"), Some(0x12abcf));
        for value in ["", "#abc", "123456", "#1234567", "#é1234", "#123456; color:red", "red", "#gggggg"] {
            assert_eq!(color(value), None, "{value}");
        }
    }
}

#[cfg(test)]
pub(crate) fn sample(card: &gtk::Widget, window: &gtk::Window) -> [u8; 4] {
    let paintable = gtk::WidgetPaintable::new(Some(card));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(card.width()), f64::from(card.height()));
    let node = snapshot.to_node().expect("a mapped card must render");
    let viewport = gtk::graphene::Rect::new(0.0, 0.0, card.width() as f32, card.height() as f32);
    let texture = window.renderer().unwrap().render_texture(&node, Some(&viewport));
    let bytes = texture.save_to_png_bytes();
    let image = image::load_from_memory(bytes.as_ref()).unwrap().to_rgba8();
    // Custom cards use a 2px border; its inner pixel is antialiased by GTK.
    // Sample the fully covered outer pixel at mid-height, away from corners.
    image.get_pixel(0, image.height() / 2).0
}

#[cfg(test)]
pub(crate) fn exercise(context: &gtk::glib::MainContext) {
    use crate::ui::chat::{actions::tests::{pump, until}, performance::settle};
    use adw::prelude::*;
    let display = gtk::gdk::Display::default().unwrap();
    let palette = palette(&display);
    assert!(palette.borrow().colors.is_empty());
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let card = || {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        card.add_css_class("card"); card.add_css_class("papo-link-preview");
        card.set_size_request(100, 56); body.append(&card); card
    };
    let red = card(); let blue = card(); let plain = card();
    let window = gtk::Window::builder().title("Embed accent palette").default_width(200).child(&body).build();
    window.present(); settle(context);
    let original = sample(plain.upcast_ref(), &window);
    apply(&red, Some("#D32F2F")); apply(&blue, Some("#123aBc"));
    apply(&plain, Some("#123456; color: red"));
    // Both assignments share one high-priority idle and one CSS parse.
    assert_eq!(palette.borrow().loads, 0);
    settle(context);
    assert_eq!(palette.borrow().loads, 1);
    assert_eq!(sample(red.upcast_ref(), &window), [211,47,47,255]);
    assert_eq!(sample(blue.upcast_ref(), &window), [18,58,188,255]);
    assert_eq!(sample(plain.upcast_ref(), &window), original, "unrelated cards keep the theme's accent");
    let manager = adw::StyleManager::default(); let previous = manager.color_scheme();
    for scheme in [adw::ColorScheme::ForceDark, adw::ColorScheme::ForceLight] {
        manager.set_color_scheme(scheme); settle(context);
        assert_eq!(sample(red.upcast_ref(), &window), [211,47,47,255]);
        assert_eq!(sample(blue.upcast_ref(), &window), [18,58,188,255]);
    }
    manager.set_color_scheme(previous);
    // A newly introduced color must be installed before its first snapshot.
    let first = gtk::Box::new(gtk::Orientation::Vertical, 0);
    first.add_css_class("papo-link-preview"); apply(&first, Some("#006400"));
    let frame = crate::media::frame::frame(&first, 100, 56);
    let painted = Rc::new(std::cell::Cell::new(false));let observed = painted.clone();let styles = palette.clone();
    frame.on_first_snapshot(move || {
        assert!(styles.borrow().installed && styles.borrow().rendered.contains(&0x006400), "the first frame must already have its color");
        observed.set(true);
    });
    body.append(&frame);settle(context);assert!(painted.get());
    body.remove(&frame);drop((frame, first));settle(context);
    let loads = palette.borrow().loads;
    // Mount/drop churn with the same active palette must not restyle the display.
    for _ in 0..5000 {
        let temporary = gtk::Box::new(gtk::Orientation::Vertical, 0);
        apply(&temporary, Some("#123ABC"));
        apply(&temporary, Some("#123abc")); // idempotent on the same widget
    }
    settle(context); assert_eq!(palette.borrow().loads, loads);
    assert_eq!(palette.borrow().colors.len(), 2);
    assert!(palette.borrow().colors.values().all(|uses| *uses == 1));
    let mut many = Vec::new();
    for value in 0..MAX_COLORS + 32 {
        let temporary = gtk::Box::new(gtk::Orientation::Vertical, 0);
        apply(&temporary, Some(&format!("#{:06x}", 0x10000 + value)));
        many.push(temporary);
    }
    assert_eq!(palette.borrow().colors.len(), MAX_COLORS);
    assert!(!many.last().unwrap().css_classes().iter().any(|c| c.starts_with(PREFIX)), "overflow keeps the theme's fallback");
    settle(context); assert_eq!(palette.borrow().loads, loads + 1);
    drop(many); settle(context);
    assert_eq!(palette.borrow().colors.len(), 2);
    window.set_child(None::<&gtk::Widget>); window.close();
    drop((body, red, blue, plain));
    until(context, || palette.borrow().colors.is_empty() && !palette.borrow().installed);
    let weak = Rc::downgrade(&palette); let provider = palette.borrow().provider.downgrade();
    drop(palette); pump(context);
    assert!(weak.upgrade().is_none() && provider.upgrade().is_none(), "disposed cards must release their provider");
}
