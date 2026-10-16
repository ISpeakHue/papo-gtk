//! Native call preferences; changes keep their existing immediate behavior.
use super::*;

pub(super) struct Controls {
    pub window: gtk::Window,
    pub body: gtk::Box,
    pub devices: adw::ComboRow,
    pub gain: gtk::Scale,
}

pub(super) fn device_row(initial: &str) -> adw::ComboRow {
    let row = adw::ComboRow::builder().title("Dispositivo").model(&gtk::StringList::new(&[initial])).build();
    row.set_title_lines(0);
    // Keep the native popup while allowing longer device names in the row.
    row.set_list_factory(row.factory().as_ref());
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let label = gtk::Label::new(None);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_max_width_chars(40);
        label.set_xalign(1.0);
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let value = item.item().and_downcast::<gtk::StringObject>().unwrap().string();
        let label = item.child().and_downcast::<gtk::Label>().unwrap();
        label.set_text(&value);
        label.set_tooltip_text(Some(&value));
    });
    row.set_factory(Some(&factory));
    row
}

pub(super) fn group(body: &gtk::Box, title: &str, refresh_label: &str, sender: &ComponentSender<MainWindowModel>, action: impl Fn() -> VoiceMsg + 'static) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(title);
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.add_css_class("flat");
    refresh.set_tooltip_text(Some(refresh_label));
    refresh.update_property(&[gtk::accessible::Property::Label(refresh_label)]);
    let input = sender.input_sender().clone();
    refresh.connect_clicked(move |_| { let _ = input.send(MainWindowMsg::Voice(action())); });
    group.set_header_suffix(Some(&refresh));
    body.append(&group);
    group
}

pub(super) fn new(sender: &ComponentSender<MainWindowModel>) -> Controls {
    let native = adw::Window::builder().title("Configurar chamada").default_width(760).default_height(560).hide_on_close(true).build();
    native.set_size_request(320,360);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("Configurar chamada", "")));
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 24);
    body.set_valign(gtk::Align::Start);
    body.set_margin_top(12);
    body.set_margin_bottom(24);
    body.set_margin_start(32);
    body.set_margin_end(32);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(840);
    clamp.set_tightening_threshold(640);
    clamp.set_child(Some(&body));
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
    toolbar.set_content(Some(&scroll));
    let info = gtk::Label::new(Some("O microfone começa silenciado. Trocar o dispositivo encerra a chamada para liberar o microfone anterior."));
    info.set_wrap(true);
    info.set_xalign(0.0);
    info.add_css_class("dim-label");
    body.append(&info);

    let microphone = group(&body, "Microfone", "Atualizar microfones", sender, || VoiceMsg::ReloadDevices);
    let devices = device_row("Microfone padrão do sistema");
    let input = sender.input_sender().clone();
    devices.connect_selected_notify(move |_| { let _ = input.send(MainWindowMsg::Voice(VoiceMsg::Device)); });
    microphone.add(&devices);
    let row = adw::ActionRow::builder().title("Volume do microfone").title_lines(0).build();
    let volume = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    volume.set_hexpand(true);
    volume.set_valign(gtk::Align::Center);
    let icon = gtk::Image::from_icon_name("microphone-sensitivity-high-symbolic");
    icon.add_css_class("dim-label");
    volume.append(&icon);
    let gain = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 300.0, 5.0);
    gain.set_widget_name("microphone-gain");
    gain.set_hexpand(true);
    gain.set_width_request(96);
    gain.set_value(150.0);
    gain.set_draw_value(true);
    gain.set_value_pos(gtk::PositionType::Right);
    gain.set_format_value_func(|_, value| format!("{value:.0}%"));
    gain.set_tooltip_text(Some("Ajuste a amplificação do microfone. 100% mantém o nível original."));
    gain.update_property(&[gtk::accessible::Property::Label("Volume do microfone")]);
    let input = sender.input_sender().clone();
    gain.connect_value_changed(move |_| { let _ = input.send(MainWindowMsg::Voice(VoiceMsg::Gain)); });
    volume.append(&gain);
    row.add_suffix(&volume);
    microphone.add(&row);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    actions.add_css_class("papo-call-actions");
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    actions.append(&spacer);
    let done = gtk::Button::with_label("Concluir");
    done.add_css_class("suggested-action");
    let window = native.downgrade();
    done.connect_clicked(move |_| { if let Some(window) = window.upgrade() { window.close(); } });
    actions.append(&done);
    toolbar.add_bottom_bar(&actions);
    native.set_content(Some(&toolbar));
    let compact = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 520sp").unwrap());
    compact.add_setter(&body, "margin-start", Some(&16i32.to_value()));
    compact.add_setter(&body, "margin-end", Some(&16i32.to_value()));
    native.add_breakpoint(compact);
    Controls { window: native.upcast(), body, devices, gain }
}
