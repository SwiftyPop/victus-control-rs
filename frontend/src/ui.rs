use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, DropDown, HeaderBar, Label, Orientation,
    Scale, StringList, Switch,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use zbus::Connection;

use crate::dbus_client::VictusControlProxy;

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(380)
        .default_height(370)
        .build();

    // Compact HeaderBar
    let header_bar = HeaderBar::new();
    let title_label = Label::builder()
        .label("<span weight='bold'>HP Victus Control</span>")
        .use_markup(true)
        .build();
    header_bar.set_title_widget(Some(&title_label));
    window.set_titlebar(Some(&header_bar));

    let main_box = GtkBox::new(Orientation::Vertical, 10);
    main_box.set_margin_top(12);
    main_box.set_margin_bottom(12);
    main_box.set_margin_start(12);
    main_box.set_margin_end(12);

    // Card 1: Thermal Monitoring Bar (CPU & GPU Badges)
    let temp_card = GtkBox::new(Orientation::Horizontal, 12);
    temp_card.add_css_class("compact-card");
    temp_card.set_halign(Align::Fill);

    let temp_title = Label::builder()
        .label("Thermals:")
        .halign(Align::Start)
        .build();
    temp_title.add_css_class("section-title");

    let cpu_temp_label = Label::new(Some("CPU: -- °C"));
    cpu_temp_label.add_css_class("temp-badge-cpu");

    let gpu_temp_label = Label::new(Some("GPU: -- °C"));
    gpu_temp_label.add_css_class("temp-badge-gpu");

    temp_card.append(&temp_title);
    temp_card.append(&cpu_temp_label);
    temp_card.append(&gpu_temp_label);
    main_box.append(&temp_card);

    // Card 2: Fan Control Mode & Overheat Protection (Inline Compact Row)
    let control_card = GtkBox::new(Orientation::Horizontal, 12);
    control_card.add_css_class("compact-card");

    // Left Column: Fan Mode Dropdown
    let mode_box = GtkBox::new(Orientation::Vertical, 4);
    mode_box.set_hexpand(true);
    let mode_title = Label::builder()
        .label("Fan Mode")
        .halign(Align::Start)
        .build();
    mode_title.add_css_class("section-title");

    let modes = StringList::new(&["AUTO", "BETTER_AUTO", "MANUAL", "MAX"]);
    let mode_dropdown = DropDown::new(Some(modes), None::<gtk4::Expression>);
    mode_dropdown.set_selected(1); // Default to BETTER_AUTO

    mode_box.append(&mode_title);
    mode_box.append(&mode_dropdown);

    // Right Column: Overheat Warnings Switch
    let notify_box = GtkBox::new(Orientation::Vertical, 4);
    notify_box.set_halign(Align::End);
    let notify_title = Label::builder()
        .label("Warnings")
        .halign(Align::Start)
        .build();
    notify_title.add_css_class("section-title");

    let notify_switch = Switch::new();
    notify_switch.set_valign(Align::Center);

    // Check initial service state asynchronously using GLib SubprocessLauncher (no Tokio runtime required)
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
    if let Ok(proc) = launcher.spawn(&[
        std::ffi::OsStr::new("systemctl"),
        std::ffi::OsStr::new("--user"),
        std::ffi::OsStr::new("is-active"),
        std::ffi::OsStr::new("--quiet"),
        std::ffi::OsStr::new("victus-monitor.service"),
    ]) {
        let sw_clone = notify_switch.clone();
        glib::spawn_future_local(async move {
            let _ = proc.wait_future().await;
            sw_clone.set_active(proc.is_successful());
        });
    }

    // Async toggle for systemd monitor service using GLib SubprocessLauncher
    notify_switch.connect_active_notify(move |sw| {
        let active = sw.is_active();
        let cmd_arg = if active { "enable" } else { "disable" };
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        let _ = launcher.spawn(&[
            std::ffi::OsStr::new("systemctl"),
            std::ffi::OsStr::new("--user"),
            std::ffi::OsStr::new(cmd_arg),
            std::ffi::OsStr::new("--now"),
            std::ffi::OsStr::new("victus-monitor.service"),
        ]);
    });

    notify_box.append(&notify_title);
    notify_box.append(&notify_switch);

    control_card.append(&mode_box);
    control_card.append(&notify_box);
    main_box.append(&control_card);

    // Card 3: Live Fan Speeds (Compact Sliders)
    let fans_card = GtkBox::new(Orientation::Vertical, 8);
    fans_card.add_css_class("compact-card");

    let fans_title = Label::builder()
        .label("Fan Speeds")
        .halign(Align::Start)
        .build();
    fans_title.add_css_class("section-title");
    fans_card.append(&fans_title);

    // Fan 1 Speed Slider & Label
    let fan1_section = GtkBox::new(Orientation::Vertical, 2);
    let fan1_label = Label::builder()
        .label("Fan 1: -- RPM")
        .halign(Align::Start)
        .build();
    fan1_label.add_css_class("section-subtitle");
    let fan1_scale = Scale::with_range(Orientation::Horizontal, 2000.0, 6000.0, 100.0);
    fan1_scale.set_draw_value(false);
    fan1_scale.set_hexpand(true);
    fan1_scale.set_sensitive(false); // Disabled until MANUAL mode is selected
    fan1_section.append(&fan1_label);
    fan1_section.append(&fan1_scale);
    fans_card.append(&fan1_section);

    // Fan 2 Speed Slider & Label
    let fan2_section = GtkBox::new(Orientation::Vertical, 2);
    let fan2_label = Label::builder()
        .label("Fan 2: -- RPM")
        .halign(Align::Start)
        .build();
    fan2_label.add_css_class("section-subtitle");
    let fan2_scale = Scale::with_range(Orientation::Horizontal, 2000.0, 6100.0, 100.0);
    fan2_scale.set_draw_value(false);
    fan2_scale.set_hexpand(true);
    fan2_scale.set_sensitive(false); // Disabled until MANUAL mode is selected
    fan2_section.append(&fan2_label);
    fan2_section.append(&fan2_scale);
    fans_card.append(&fan2_section);

    main_box.append(&fans_card);

    // Error status notification label
    let error_label = Label::builder()
        .label("")
        .halign(Align::Center)
        .visible(false)
        .build();
    error_label.add_css_class("error-message");
    main_box.append(&error_label);

    window.set_child(Some(&main_box));
    window.present();

    // Guard flag to suppress initial dropdown notification callback
    let is_initializing = Rc::new(Cell::new(true));

    // Setup async D-Bus connection
    let proxy_cell: Rc<RefCell<Option<VictusControlProxy<'static>>>> = Rc::new(RefCell::new(None));
    let proxy_cell_clone = proxy_cell.clone();
    let mode_dropdown_init = mode_dropdown.clone();
    let f1_scale_init = fan1_scale.clone();
    let f2_scale_init = fan2_scale.clone();
    let is_init_clone = is_initializing.clone();
    let err_lbl_init = error_label.clone();

    glib::spawn_future_local(async move {
        match Connection::system().await {
            Ok(connection) => match VictusControlProxy::new(&connection).await {
                Ok(proxy) => {
                    // Dynamically set fan slider max speeds from daemon
                    if let Ok(max1) = proxy.get_fan_max_speed(1).await {
                        f1_scale_init.set_range(2000.0, max1 as f64);
                    }
                    if let Ok(max2) = proxy.get_fan_max_speed(2).await {
                        f2_scale_init.set_range(2000.0, max2 as f64);
                    }

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

                        if is_manual {
                            if let Ok(rpm1) = proxy.get_fan_speed(1).await {
                                if rpm1 >= 2000 {
                                    f1_scale_init.set_value(rpm1 as f64);
                                }
                            }
                            if let Ok(rpm2) = proxy.get_fan_speed(2).await {
                                if rpm2 >= 2000 {
                                    f2_scale_init.set_value(rpm2 as f64);
                                }
                            }
                        }
                    }
                    *proxy_cell_clone.borrow_mut() = Some(proxy);
                }
                Err(e) => {
                    err_lbl_init.set_text(&format!("D-Bus proxy error: {}", e));
                    err_lbl_init.set_visible(true);
                }
            },
            Err(e) => {
                err_lbl_init.set_text(&format!("D-Bus connection error: {}", e));
                err_lbl_init.set_visible(true);
            }
        }
        is_init_clone.set(false);
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
                    if cpu > 0.0 {
                        cpu_lbl.set_text(&format!("CPU: {:.1} °C", cpu));
                    } else {
                        cpu_lbl.set_text("CPU: -- °C");
                    }
                }
                if let Ok(gpu) = p_gpu.get_gpu_temp().await {
                    if gpu > 0.0 {
                        gpu_lbl.set_text(&format!("GPU: {:.1} °C", gpu));
                    } else {
                        gpu_lbl.set_text("GPU: -- °C");
                    }
                }
                if let Ok(rpm1) = p_f1.get_fan_speed(1).await {
                    if rpm1 > 0 {
                        f1_lbl.set_text(&format!("Fan 1: {} RPM", rpm1));
                    } else {
                        f1_lbl.set_text("Fan 1: -- RPM");
                    }
                }
                if let Ok(rpm2) = p_f2.get_fan_speed(2).await {
                    if rpm2 > 0 {
                        f2_lbl.set_text(&format!("Fan 2: {} RPM", rpm2));
                    } else {
                        f2_lbl.set_text("Fan 2: -- RPM");
                    }
                }
            });
        }
        glib::ControlFlow::Continue
    });

    // Fan mode change callback
    let proxy_mode = proxy_cell.clone();
    let f1_scale_mode = fan1_scale.clone();
    let f2_scale_mode = fan2_scale.clone();
    let err_lbl_mode = error_label.clone();
    let is_init_mode = is_initializing.clone();

    mode_dropdown.connect_selected_notify(move |dropdown| {
        if is_init_mode.get() {
            return;
        }

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
            let f1_s = f1_scale_mode.clone();
            let f2_s = f2_scale_mode.clone();
            let err_lbl = err_lbl_mode.clone();

            glib::spawn_future_local(async move {
                err_lbl.set_visible(false);

                if is_manual {
                    if let Ok(rpm1) = p.get_fan_speed(1).await {
                        if rpm1 >= 2000 {
                            f1_s.set_value(rpm1 as f64);
                        }
                    }
                    if let Ok(rpm2) = p.get_fan_speed(2).await {
                        if rpm2 >= 2000 {
                            f2_s.set_value(rpm2 as f64);
                        }
                    }
                }

                match p.set_fan_mode(m).await {
                    Ok(resp) => {
                        if resp.starts_with("ERROR:") {
                            err_lbl.set_text(&resp);
                            err_lbl.set_visible(true);
                        }
                    }
                    Err(e) => {
                        err_lbl.set_text(&format!("Failed to set mode: {}", e));
                        err_lbl.set_visible(true);
                    }
                }
            });
        }
    });

    // Fan 1 slider callback
    let proxy_f1 = proxy_cell.clone();
    let err_lbl_f1 = error_label.clone();
    fan1_scale.connect_value_changed(move |scale| {
        let val = scale.value() as u32;
        if let Some(ref proxy) = *proxy_f1.borrow() {
            let p = proxy.clone();
            let err_lbl = err_lbl_f1.clone();
            glib::spawn_future_local(async move {
                match p.set_fan_speed(1, val).await {
                    Ok(resp) if resp.starts_with("ERROR:") => {
                        err_lbl.set_text(&resp);
                        err_lbl.set_visible(true);
                    }
                    Err(e) => {
                        err_lbl.set_text(&format!("Fan 1 speed update error: {}", e));
                        err_lbl.set_visible(true);
                    }
                    _ => {}
                }
            });
        }
    });

    // Fan 2 slider callback
    let proxy_f2 = proxy_cell.clone();
    let err_lbl_f2 = error_label;
    fan2_scale.connect_value_changed(move |scale| {
        let val = scale.value() as u32;
        if let Some(ref proxy) = *proxy_f2.borrow() {
            let p = proxy.clone();
            let err_lbl = err_lbl_f2.clone();
            glib::spawn_future_local(async move {
                match p.set_fan_speed(2, val).await {
                    Ok(resp) if resp.starts_with("ERROR:") => {
                        err_lbl.set_text(&resp);
                        err_lbl.set_visible(true);
                    }
                    Err(e) => {
                        err_lbl.set_text(&format!("Fan 2 speed update error: {}", e));
                        err_lbl.set_visible(true);
                    }
                    _ => {}
                }
            });
        }
    });
}
