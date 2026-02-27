mod config;
mod desktop;
mod gpu;
mod heroic;
mod launcher;
mod logger;
mod models;
mod nvidia;
mod steam;
mod ui;
mod updates;

use adw::prelude::*;

fn main() {
    logger::init();
    let app = adw::Application::builder()
        .application_id("com.kaede.gpu-manager")
        .build();

    app.connect_activate(|app| {
        ui::build_ui(app);
    });

    app.run();
}
