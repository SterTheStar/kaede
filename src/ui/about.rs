use adw::prelude::*;

use super::{APP_AUTHOR, APP_DESCRIPTION, APP_GITHUB, APP_LICENSE, APP_NAME};

pub(crate) fn show_about_dialog(window: &adw::ApplicationWindow, update_dot: Option<gtk::Widget>) {
    let dialog = gtk::Dialog::builder()
        .transient_for(window)
        .modal(true)
        .title("About Kaede")
        .default_width(520)
        .default_height(360)
        .build();

    let content = dialog.content_area();
    content.set_spacing(0);

    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 12);
    wrapper.set_margin_top(18);
    wrapper.set_margin_bottom(18);
    wrapper.set_margin_start(18);
    wrapper.set_margin_end(18);

    let hero = gtk::Box::new(gtk::Orientation::Vertical, 6);
    hero.set_halign(gtk::Align::Center);
    let icon = gtk::Image::new();
    super::details::apply_icon_to_image(&icon, Some(super::APP_ICON_PATH), 96);
    icon.set_halign(gtk::Align::Center);
    let name = gtk::Label::new(Some(APP_NAME));
    name.set_xalign(0.5);
    name.add_css_class("title-2");
    let version = gtk::Label::new(Some(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
    version.set_xalign(0.5);
    version.add_css_class("dim-label");
    hero.append(&icon);
    hero.append(&name);
    hero.append(&version);
    wrapper.append(&hero);

    let description = gtk::Label::new(Some(APP_DESCRIPTION));
    description.set_wrap(true);
    description.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    description.set_xalign(0.5);
    description.set_justify(gtk::Justification::Center);
    description.add_css_class("caption");
    wrapper.append(&description);

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);

    let author_row = adw::ActionRow::builder()
        .title("Author")
        .subtitle(APP_AUTHOR)
        .build();
    list.append(&author_row);

    let license_row = adw::ActionRow::builder()
        .title("License")
        .subtitle(APP_LICENSE)
        .build();
    list.append(&license_row);

    let github_row = adw::ActionRow::builder()
        .title("Project")
        .subtitle("Source code and issue tracker")
        .build();
    let github = gtk::LinkButton::new(APP_GITHUB);
    github.set_label("GitHub Repository");
    github.add_css_class("flat");
    github_row.add_suffix(&github);
    github_row.set_activatable_widget(Some(&github));
    list.append(&github_row);

    wrapper.append(&list);

    // Update check in background using a standard channel and glib idle loop
    let (tx, rx) = std::sync::mpsc::channel::<crate::updates::UpdateResult>();
    std::thread::spawn(move || {
        if let Ok(res) = crate::updates::check_for_updates() {
            let _ = tx.send(res);
        }
    });

    let version_label = version.clone();
    glib::idle_add_local(move || {
        if let Ok(res) = rx.try_recv() {
            use crate::updates::UpdateResult::*;
            match res {
                NewRelease(latest) => {
                    if let Some(ref dot) = update_dot {
                        dot.set_visible(true);
                    }
                    version_label.set_markup(&format!(
                        "Version {} <span color='#2ec27e' weight='bold'>(New version: {})</span>",
                        env!("CARGO_PKG_VERSION"),
                        latest
                    ));
                    
                    // Make the version label clickable to download
                    let click = gtk::GestureClick::new();
                    click.connect_released(|_, _, _, _| {
                        let _ = gio::AppInfo::launch_default_for_uri("https://github.com/SterTheStar/kaede/releases", None::<&gio::AppLaunchContext>);
                    });
                    version_label.add_controller(click);
                    version_label.set_cursor_from_name(Some("pointer"));
                }
                Beta => {
                    version_label.set_markup(&format!(
                        "Version {} <span color='#3584e4' weight='bold'>(Development)</span>",
                        env!("CARGO_PKG_VERSION")
                    ));
                }
                UpToDate => {
                    version_label.set_markup(&format!(
                        "Version {} <span color='#818181'>(Latest)</span>",
                        env!("CARGO_PKG_VERSION")
                    ));
                }
            }
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });

    content.append(&wrapper);
    dialog.present();
}

