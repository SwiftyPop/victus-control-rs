use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, DropDown, HeaderBar, Label, Orientation,
    Scale, StringList, Switch,
};
use std::cell::RefCell;
use std::rc::Rc;
use zbus::Connection;

use crate::dbus_client::VictusControlProxy;

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(460)
        .default_height(540)
        .build();

    // GTK4 HeaderBar
    let header_bar = HeaderBar::new();
    let title_label = Label::builder()
        .label("<span weight='bold'>HP Victus Fan Control</span>")
        .use_markup(true)
        .build();
    header_bar.set_title_widget(Some(&title_label));
    window.set_titlebar(Some(&header_bar));

    let main_box = GtkBox::new(Orientation::Vertical, 16);
    main_box.set_margin_top(16);
    main_box.set_margin_bottom(16);
    main_box.set_margin_start(16);
    main_box.set_margin_end(16);

    // Card 1: Thermal Status
    let temp_card = GtkBox::new(Orientation::Vertical, 8);
    temp_card.add_css_class("card-box");

    let temp_title = Label::builder()
        .label("Thermal Status")
        .halign(Align::Start)
        .build();
    temp_title.add_css_class("section-title");
    temp_card.append(&temp_title);

    let temp_badges_box = GtkBox::new(Orientation::Horizontal, 16);
    temp_badges_box.set_halign(Align::Center);

    let cpu_temp_label = Label::new(Some("CPU: -- °C"));
    cpu_temp_label.add_css_class("temp-badge");

    let gpu_temp_label = Label::new(Some("GPU: -- °C"));
    gpu_temp_label.add_css_class("temp-badge-gpu");

    temp_badges_box.append(&cpu_temp_label);
    temp_badges_box.append(&gpu_temp_label);
    temp_card.append(&temp_badges_box);
    main_box.append(&temp_card);

    // Card 2: Fan Control Mode
    let mode_card = GtkBox::new(Orientation::Vertical, 8);
    mode_card.add_css_class("card-box");

    let mode_title = Label::builder()
        .label("Fan Control Mode")
        .halign(Align::Start)
        .build();
    mode_title.add_css_class("section-title");
    mode_card.append(&mode_title);

    let mode_subtitle = Label::builder()
        .label("Select thermal profile or manual speed override")
        .halign(Align::Start)
        .build();
    mode_subtitle.add_css_class("section-subtitle");
    mode_card.append(&mode_subtitle);

    let modes = StringList::new(&["AUTO", "BETTER_AUTO", "MANUAL", "MAX"]);
    let mode_dropdown = DropDown::new(Some(modes), None::<gtk4::Expression>);
    mode_dropdown.set_selected(1); // Default to BETTER_AUTO
    mode_card.append(&mode_dropdown);
    main_box.append(&mode_card);

    // Card 3: Overheat Protection Switch
    let notify_card = GtkBox::new(Orientation::Horizontal, 12);
    notify_card.add_css_class("card-box");

    let notify_text_box = GtkBox::new(Orientation::Vertical, 2);
    notify_text_box.set_hexpand(true);

    let notify_title = Label::builder()
        .label("Overheat Warnings")
        .halign(Align::Start)
        .build();
    notify_title.add_css_class("section-title");

    let notify_subtitle = Label::builder()
        .label("Desktop notifications for high temperatures")
        .halign(Align::Start)
        .build();
    notify_subtitle.add_css_class("section-subtitle");

    notify_text_box.append(&notify_title);
    notify_text_box.append(&notify_subtitle);

    let notify_switch = Switch::new();
    notify_switch.set_valign(Align::Center);

    // Check initial service state
    let is_monitor_active = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "victus-monitor.service"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    notify_switch.set_active(is_monitor_active);

    notify_switch.connect_active_notify(move |sw| {
        let active = sw.is_active();
        let cmd_arg = if active { "enable" } else { "disable" };
        let _ = std::process::Command::new("systemctl")
            .args(["--user", cmd_arg, "--now", "victus-monitor.service"])
            .status();
    });

    notify_card.append(&notify_text_box);
    notify_card.append(&notify_switch);
    main_box.append(&notify_card);

    // Card 4: Fan Speeds & Sliders
    let fans_card = GtkBox::new(Orientation::Vertical, 12);
    fans_card.add_css_class("card-box");

    let fans_title = Label::builder()
        .label("Live Fan Speeds")
        .halign(Align::Start)
        .build();
    fans_title.add_css_class("section-title");
    fans_card.append(&fans_title);

    // Fan 1 Speed Slider & Label
    let fan1_section = GtkBox::new(Orientation::Vertical, 4);
    let fan1_label = Label::builder()
        .label("Fan 1 Speed: -- RPM")
        .halign(Align::Start)
        .build();
    let fan1_scale = Scale::with_range(Orientation::Horizontal, 2000.0, 6000.0, 100.0);
    fan1_scale.set_draw_value(true);
    fan1_scale.set_hexpand(true);
    fan1_scale.set_width_request(240);
    fan1_scale.set_sensitive(false); // Disabled until MANUAL mode is selected
    fan1_section.append(&fan1_label);
    fan1_section.append(&fan1_scale);
    fans_card.append(&fan1_section);

    // Fan 2 Speed Slider & Label
    let fan2_section = GtkBox::new(Orientation::Vertical, 4);
    let fan2_label = Label::builder()
        .label("Fan 2 Speed: -- RPM")
        .halign(Align::Start)
        .build();
    let fan2_scale = Scale::with_range(Orientation::Horizontal, 2000.0, 6000.0, 100.0);
    fan2_scale.set_draw_value(true);
    fan2_scale.set_hexpand(true);
    fan2_scale.set_width_request(240);
    fan2_scale.set_sensitive(false); // Disabled until MANUAL mode is selected
    fan2_section.append(&fan2_label);
    fan2_section.append(&fan2_scale);
    fans_card.append(&fan2_section);

    main_box.append(&fans_card);

    window.set_child(Some(&main_box));
    window.present();

    // Setup async D-Bus connection
    let proxy_cell: Rc<RefCell<Option<VictusControlProxy<'static>>>> = Rc::new(RefCell::new(None));
    let proxy_cell_clone = proxy_cell.clone();
    let mode_dropdown_init = mode_dropdown.clone();
    let f1_scale_init = fan1_scale.clone();
    let f2_scale_init = fan2_scale.clone();

    glib::spawn_future_local(async move {
        if let Ok(connection) = Connection::system().await {
            if let Ok(proxy) = VictusControlProxy::new(&connection).await {
                if let Ok(current_mode) = proxy.get_fan_mode().await {
                    let idx = match current_mode.as_str() {
                        "AUTO" => 0,
                        "BETTER_AUTO" => 1,
                        "MANUAL" => 2,
                        "MAX" => 3,
                        _ => 1,
                    };
                    mode_dropdown_init.set_selected(idx);
                    let is_manual = idx == 2;
                    f1_scale_init.set_sensitive(is_manual);
                    f2_scale_init.set_sensitive(is_manual);
                }
                *proxy_cell_clone.borrow_mut() = Some(proxy);
            }
        }
    });

    // Thermal & Fan Speed polling ticker loop
    let cpu_lbl_clone = cpu_temp_label.clone();
    let gpu_lbl_clone = gpu_temp_label.clone();
    let fan1_lbl_clone = fan1_label.clone();
    let fan2_lbl_clone = fan2_label.clone();
    let proxy_poll = proxy_cell.clone();

    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        if let Some(ref proxy) = *proxy_poll.borrow() {
            let p_cpu = proxy.clone();
            let p_gpu = proxy.clone();
            let p_f1 = proxy.clone();
            let p_f2 = proxy.clone();

            let cpu_lbl = cpu_lbl_clone.clone();
            let gpu_lbl = gpu_lbl_clone.clone();
            let f1_lbl = fan1_lbl_clone.clone();
            let f2_lbl = fan2_lbl_clone.clone();

            glib::spawn_future_local(async move {
                if let Ok(cpu) = p_cpu.get_cpu_temp().await {
                    cpu_lbl.set_text(&format!("CPU: {:.1} °C", cpu));
                }
                if let Ok(gpu) = p_gpu.get_gpu_temp().await {
                    gpu_lbl.set_text(&format!("GPU: {:.1} °C", gpu));
                }
                if let Ok(rpm1) = p_f1.get_fan_speed(1).await {
                    f1_lbl.set_text(&format!("Fan 1 Speed: {} RPM", rpm1));
                }
                if let Ok(rpm2) = p_f2.get_fan_speed(2).await {
                    f2_lbl.set_text(&format!("Fan 2 Speed: {} RPM", rpm2));
                }
            });
        }
        glib::ControlFlow::Continue
    });

    // Fan mode change callback
    let proxy_mode = proxy_cell.clone();
    let f1_scale_mode = fan1_scale.clone();
    let f2_scale_mode = fan2_scale.clone();
    mode_dropdown.connect_selected_notify(move |dropdown| {
        let idx = dropdown.selected();
        let is_manual = idx == 2;
        f1_scale_mode.set_sensitive(is_manual);
        f2_scale_mode.set_sensitive(is_manual);

        let mode_str = match idx {
            0 => "AUTO",
            1 => "BETTER_AUTO",
            2 => "MANUAL",
            3 => "MAX",
            _ => "AUTO",
        };
        if let Some(ref proxy) = *proxy_mode.borrow() {
            let p = proxy.clone();
            let m = mode_str.to_string();
            glib::spawn_future_local(async move {
                let _ = p.set_fan_mode(m).await;
            });
        }
    });

    // Fan 1 slider callback
    let proxy_f1 = proxy_cell.clone();
    fan1_scale.connect_value_changed(move |scale| {
        let val = scale.value() as u32;
        if let Some(ref proxy) = *proxy_f1.borrow() {
            let p = proxy.clone();
            glib::spawn_future_local(async move {
                let _ = p.set_fan_speed(1, val).await;
            });
        }
    });

    // Fan 2 slider callback
    let proxy_f2 = proxy_cell.clone();
    fan2_scale.connect_value_changed(move |scale| {
        let val = scale.value() as u32;
        if let Some(ref proxy) = *proxy_f2.borrow() {
            let p = proxy.clone();
            glib::spawn_future_local(async move {
                let _ = p.set_fan_speed(2, val).await;
            });
        }
    });
}
