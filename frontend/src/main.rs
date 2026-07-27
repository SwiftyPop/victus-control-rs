mod dbus_client;
mod ui;

use gtk4::prelude::*;
use gtk4::Application;

fn main() {
    let app = Application::builder()
        .application_id("org.hp.VictusControl")
        .build();

    app.connect_activate(ui::build_ui);
    app.run();
}
