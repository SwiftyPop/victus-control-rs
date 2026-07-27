mod dbus_client;
mod ui;

use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{Application, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};

fn main() {
    let app = Application::builder()
        .application_id("org.hp.VictusControl")
        .build();

    app.connect_activate(|app| {
        // Load CSS styling
        let provider = CssProvider::new();
        let css_bytes = include_bytes!("style.css");
        provider.load_from_string(std::str::from_utf8(css_bytes).unwrap_or(""));

        if let Some(display) = Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        ui::build_ui(app);
    });

    app.run();
}
