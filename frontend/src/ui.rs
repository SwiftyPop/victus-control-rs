use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, DropDown, Label, Orientation, Scale,
    StringList,
};
use std::cell::RefCell;
use std::rc::Rc;
use zbus::Connection;

use crate::dbus_client::VictusControlProxy;

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("HP Victus Fan Control")
        .default_width(440)
        .default_height(420)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 16);
    main_box.set_margin_top(24);
    main_box.set_margin_bottom(24);
    main_box.set_margin_start(24);
    main_box.set_margin_end(24);

    // Title / Header
    let header_label = Label::builder()
        .label("<span size='x-large' weight='bold'>HP Victus Fan Control</span>")
        .use_markup(true)
        .halign(Align::Center)
        .build();
    main_box.append(&header_label);

    // Thermal Status Group
    let temp_box = GtkBox::new(Orientation::Horizontal, 24);
    temp_box.set_halign(Align::Center);

    let cpu_temp_label = Label::new(Some("CPU: -- °C"));
    let gpu_temp_label = Label::new(Some("GPU: -- °C"));
    temp_box.append(&cpu_temp_label);
    temp_box.append(&gpu_temp_label);
    main_box.append(&temp_box);

    // Fan Mode DropDown
    let mode_section = GtkBox::new(Orientation::Vertical, 8);
    let mode_label = Label::builder()
        .label("<span weight='bold'>Fan Control Mode</span>")
        .use_markup(true)
        .halign(Align::Start)
        .build();
    mode_section.append(&mode_label);

    let modes = StringList::new(&["AUTO", "BETTER_AUTO", "MANUAL", "MAX"]);
    let mode_dropdown = DropDown::new(Some(modes), None::<gtk4::Expression>);
    mode_dropdown.set_selected(1); // Default to BETTER_AUTO
    mode_section.append(&mode_dropdown);
    main_box.append(&mode_section);

    // Fan 1 Speed Slider & Label
    let fan1_section = GtkBox::new(Orientation::Vertical, 6);
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
    main_box.append(&fan1_section);

    // Fan 2 Speed Slider & Label
    let fan2_section = GtkBox::new(Orientation::Vertical, 6);
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
    main_box.append(&fan2_section);

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
