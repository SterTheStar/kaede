use adw::prelude::*;

use super::{APP_AUTHOR, APP_DESCRIPTION, APP_GITHUB, APP_LICENSE, APP_NAME};

pub(crate) fn show_about_dialog(window: &adw::ApplicationWindow) {
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
    let github = gtk::LinkButton::with_label(APP_GITHUB, "GitHub Repository");
    github.add_css_class("flat");
    github_row.add_suffix(&github);
    github_row.set_activatable_widget(Some(&github));
    list.append(&github_row);

    wrapper.append(&list);
    content.append(&wrapper);
    dialog.present();
}

