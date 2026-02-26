mod config;
mod desktop;
mod gpu;
mod heroic;
mod launcher;
mod logger;
mod models;
mod steam;
mod ui;

use adw::prelude::*;

fn main() {
    logger::init();
    let app = adw::Application::builder()
        .application_id("com.kaede.Kaede")
        .build();

    app.connect_activate(|app| {
        ui::build_ui(app);
    });

    app.run();
}
