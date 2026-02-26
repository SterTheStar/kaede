use crate::config::ConfigStore;
use crate::desktop::scan_desktop_entries;
use crate::gpu::detect_gpus;
use crate::launcher::apply_launcher_override;
use crate::models::{DesktopApp, GpuChoice, GpuInfo};
use crate::steam::is_steam_running;
use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use tracing::{error, info, warn};

const APP_NAME: &str = "Kaede";
const APP_DESCRIPTION: &str =
    "Select and manage GPU assignments for apps, games, and launchers on Linux.";
const APP_AUTHOR: &str = "Esther";
const APP_GITHUB: &str = "https://github.com/esther/KaedeGPU";
const APP_LICENSE: &str = "GNU GPL-3.0";
// Use the installed themed icon name so it works from the packaged build.
const APP_ICON_PATH: &str = "com.kaede.gpu-manager";

#[derive(Clone)]
struct UiState {
    gpus: Vec<GpuInfo>,
    apps: Vec<DesktopApp>,
}

#[derive(Clone)]
struct AppDetailsWidgets {
    icon: gtk::Image,
    name: gtk::Label,
    assignment_row: adw::ActionRow,
    source_row: adw::ActionRow,
    desktop_id_row: adw::ActionRow,
    path_row: adw::ActionRow,
    exec_row: adw::ActionRow,
    desktop_path_label: gtk::Label,
    desktop_open_button: gtk::Button,
    desktop_preview: gtk::TextView,
}

fn user_override_path(desktop_id: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join(".local/share/applications")
            .join(desktop_id),
    )
}

pub fn build_ui(app: &adw::Application) {
    let _ = adw::init();

    let state = Rc::new(RefCell::new(UiState {
        gpus: detect_gpus(),
        apps: scan_desktop_entries(),
    }));
    let config = Rc::new(RefCell::new(ConfigStore::load()));
    let visible_apps: Rc<RefCell<Vec<DesktopApp>>> = Rc::new(RefCell::new(Vec::new()));
    let selected_app_id: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Kaede")
        .default_width(1100)
        .default_height(760)
        .build();
    window.set_icon_name(Some(APP_ICON_PATH));

    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::builder().title("Kaede").build();
    header.set_title_widget(Some(&title));

    let refresh_btn = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh GPU and app scan")
        .build();
    let about_btn = gtk::Button::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text("About Kaede")
        .build();
    header.pack_end(&about_btn);
    header.pack_end(&refresh_btn);

    let search_btn = gtk::Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Search applications")
        .build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search applications")
        .build();
    search.set_width_chars(24);
    let search_overlay = gtk::Overlay::new();
    search_overlay.set_child(Some(&search));
    search_overlay.set_visible(false);

    let search_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    search_slot.append(&search_btn);
    search_slot.append(&search_overlay);
    header.pack_start(&search_slot);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.append(&header);

    let content = gtk::Paned::new(gtk::Orientation::Horizontal);
    content.set_margin_top(0);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    root.append(&content);

    let apps_box = gtk::ListBox::new();
    apps_box.add_css_class("boxed-list");
    apps_box.set_vexpand(true);
    apps_box.set_selection_mode(gtk::SelectionMode::Single);

    let apps_scrolled = gtk::ScrolledWindow::builder()
        .child(&apps_box)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    apps_scrolled.set_margin_end(0);
    content.set_start_child(Some(&apps_scrolled));

    let details_outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    details_outer.set_margin_top(0);
    details_outer.set_margin_bottom(12);
    details_outer.set_margin_start(12);
    details_outer.set_margin_end(12);
    details_outer.set_size_request(380, -1);
    details_outer.set_vexpand(true);

    // Summary card: modern card-style container with tight spacing.
    let summary_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    summary_card.add_css_class("card");
    summary_card.set_margin_top(0);
    summary_card.set_margin_bottom(12);
    summary_card.set_margin_start(0);
    summary_card.set_margin_end(0);

    let details_top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    details_top.set_margin_start(12);
    details_top.set_margin_end(12);
    details_top.set_margin_top(12);
    details_top.set_margin_bottom(0);
    let details_icon = gtk::Image::builder()
        .icon_name("application-x-executable")
        .pixel_size(48)
        .build();
    details_icon.set_halign(gtk::Align::Start);
    details_icon.set_valign(gtk::Align::Start);
    details_icon.set_margin_start(4);
    let details_name = gtk::Label::new(Some("Select an application"));
    details_name.set_xalign(0.0);
    details_name.add_css_class("heading");
    details_name.set_wrap(true);
    details_name.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    details_name.set_hexpand(true);
    details_top.append(&details_icon);
    details_top.append(&details_name);
    summary_card.append(&details_top);

    // Boxed list of rows for individual fields.
    let details_list = gtk::ListBox::new();
    details_list.add_css_class("boxed-list");
    details_list.set_selection_mode(gtk::SelectionMode::None);

    let details_assignment = adw::ActionRow::builder()
        .title("Current GPU")
        .subtitle("Default GPU")
        .build();
    details_list.append(&details_assignment);

    let details_source = adw::ActionRow::builder()
        .title("Source")
        .subtitle("Native desktop entry")
        .build();
    details_list.append(&details_source);

    let details_id = adw::ActionRow::builder()
        .title("Desktop ID")
        .subtitle("-")
        .build();
    details_list.append(&details_id);

    let details_path = adw::ActionRow::builder()
        .title("Path")
        .subtitle("-")
        .build();
    details_list.append(&details_path);

    let details_exec = adw::ActionRow::builder()
        .title("Exec")
        .subtitle("-")
        .build();
    details_list.append(&details_exec);

    summary_card.append(&details_list);
    details_outer.append(&summary_card);

    // Separate card for the .desktop file preview that takes the remaining height.
    let desktop_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    desktop_card.add_css_class("card");
    // Ensure children (header + preview) are clipped to the card's rounded corners.
    desktop_card.set_overflow(gtk::Overflow::Hidden);
    desktop_card.set_margin_top(0);
    desktop_card.set_margin_bottom(-12);
    desktop_card.set_margin_start(0);
    desktop_card.set_margin_end(0);
    desktop_card.set_vexpand(true);

    let desktop_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    desktop_header.set_margin_top(8);
    desktop_header.set_margin_bottom(4);
    desktop_header.set_margin_start(12);
    desktop_header.set_margin_end(12);

    let desktop_title = gtk::Label::new(Some(".desktop file"));
    desktop_title.set_xalign(0.0);
    desktop_title.add_css_class("heading");
    desktop_title.set_hexpand(true);

    let desktop_open_button = gtk::Button::with_label("Open in editor");
    desktop_open_button.add_css_class("flat");

    desktop_header.append(&desktop_title);
    desktop_header.append(&desktop_open_button);
    desktop_card.append(&desktop_header);

    let desktop_path_label = gtk::Label::new(Some("Open in external editor"));
    desktop_path_label.set_xalign(0.0);
    desktop_path_label.add_css_class("dim-label");
    desktop_path_label.set_margin_start(12);
    desktop_path_label.set_margin_end(12);
    desktop_path_label.set_margin_bottom(4);
    desktop_path_label.set_visible(false);
    desktop_card.append(&desktop_path_label);

    let desktop_preview = gtk::TextView::new();
    desktop_preview.set_editable(false);
    desktop_preview.set_cursor_visible(false);
    desktop_preview.set_monospace(true);
    desktop_preview.set_wrap_mode(gtk::WrapMode::None);

    let desktop_scrolled = gtk::ScrolledWindow::builder()
        .child(&desktop_preview)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();

    desktop_card.append(&desktop_scrolled);
    details_outer.append(&desktop_card);

    // Make the details panel scrollable so it doesn't force the window to grow.
    let details_scrolled = gtk::ScrolledWindow::builder()
        .child(&details_outer)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let details_revealer = gtk::Revealer::builder()
        .reveal_child(false)
        .transition_type(gtk::RevealerTransitionType::SlideLeft)
        .build();
    details_revealer.set_child(Some(&details_scrolled));
    content.set_end_child(Some(&details_revealer));
    content.set_resize_end_child(false);
    content.set_shrink_end_child(true);

    let details_widgets = AppDetailsWidgets {
        icon: details_icon,
        name: details_name,
        assignment_row: details_assignment,
        source_row: details_source,
        desktop_id_row: details_id,
        path_row: details_path,
        exec_row: details_exec,
        desktop_path_label: desktop_path_label.clone(),
        desktop_open_button: desktop_open_button.clone(),
        desktop_preview: desktop_preview.clone(),
    };

    {
        desktop_open_button.connect_clicked(move |btn| {
            let path_str = btn
                .tooltip_text()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if path_str.is_empty() {
                return;
            }

            let uri = format!("file://{}", path_str);
            if let Err(err) =
                gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
            {
                warn!(
                    error = %err,
                    "failed to open desktop file in external editor"
                );
            }
        });
    }

    {
        let data = state.borrow();
        rebuild_app_list(
            &apps_box,
            &window,
            &data.apps,
            &data.gpus,
            &config,
            &visible_apps,
            "",
            &details_widgets,
            &selected_app_id,
        );
    }

    {
        let search = search.clone();
        let search_btn = search_btn.clone();
        let search_btn_for_cb = search_btn.clone();
        let search_overlay = search_overlay.clone();
        search_btn.connect_clicked(move |_| {
            search_btn_for_cb.set_visible(false);
            search_overlay.set_visible(true);
            search.grab_focus();
        });
    }

    {
        let search = search.clone();
        let search_btn = search_btn.clone();
        let search_overlay = search_overlay.clone();
        let search_for_cb = search.clone();
        search.clone().connect_stop_search(move |_| {
            if search_for_cb.text().is_empty() {
                search_overlay.set_visible(false);
                search_btn.set_visible(true);
            } else {
                search_for_cb.set_text("");
            }
        });
    }

    {
        let search_btn = search_btn.clone();
        let search_overlay = search_overlay.clone();
        search.connect_has_focus_notify(move |entry| {
            if !entry.has_focus() && search_overlay.is_visible() && entry.text().is_empty() {
                search_overlay.set_visible(false);
                search_btn.set_visible(true);
            }
        });
    }

    {
        let apps_box = apps_box.clone();
        let window = window.clone();
        let state = state.clone();
        let config = config.clone();
        let visible_apps = visible_apps.clone();
        let details_widgets = details_widgets.clone();
        let selected_app_id = selected_app_id.clone();
        search.connect_search_changed(move |entry| {
            let text = entry.text().to_string();
            let data = state.borrow();
            rebuild_app_list(
                &apps_box,
                &window,
                &data.apps,
                &data.gpus,
                &config,
                &visible_apps,
                &text,
                &details_widgets,
                &selected_app_id,
            );
        });
    }

    {
        let state = state.clone();
        let config = config.clone();
        let details_widgets = details_widgets.clone();
        let details_revealer = details_revealer.clone();
        let content = content.clone();
        let apps_scrolled = apps_scrolled.clone();
        let visible_apps = visible_apps.clone();
        let selected_app_id = selected_app_id.clone();
        apps_box.connect_row_selected(move |_, row| {
            let Some(row) = row else {
                *selected_app_id.borrow_mut() = None;
                let gpus = state.borrow().gpus.clone();
                set_app_details_empty(&details_widgets, &gpus);
                set_details_panel_visible(&content, &details_revealer, &apps_scrolled, false);
                return;
            };

            let idx = row.index();
            if idx < 0 {
                *selected_app_id.borrow_mut() = None;
                let gpus = state.borrow().gpus.clone();
                set_app_details_empty(&details_widgets, &gpus);
                set_details_panel_visible(&content, &details_revealer, &apps_scrolled, false);
                return;
            }

            let app = visible_apps.borrow().get(idx as usize).cloned();
            if let Some(app) = app {
                let choice = config.borrow().get_choice(&app.desktop_id);
                *selected_app_id.borrow_mut() = Some(app.desktop_id.clone());
                let gpus = state.borrow().gpus.clone();
                set_app_details(&details_widgets, &app, &choice, &gpus);
                set_details_panel_visible(&content, &details_revealer, &apps_scrolled, true);
            } else {
                *selected_app_id.borrow_mut() = None;
                let gpus = state.borrow().gpus.clone();
                set_app_details_empty(&details_widgets, &gpus);
                set_details_panel_visible(&content, &details_revealer, &apps_scrolled, false);
            }
        });
    }

    {
        let window = window.clone();
        let apps_box = apps_box.clone();
        let header_widget = header.clone().upcast::<gtk::Widget>();
        let search = search.clone();
        let search_btn = search_btn.clone();
        let search_overlay = search_overlay.clone();
        let search_widget = search.clone().upcast::<gtk::Widget>();
        let details_widgets = details_widgets.clone();
        let details_revealer = details_revealer.clone();
        let details_revealer_widget = details_revealer.clone().upcast::<gtk::Widget>();
        let apps_box_widget = apps_box.clone().upcast::<gtk::Widget>();
        let content = content.clone();
        let apps_scrolled = apps_scrolled.clone();
        let selected_app_id = selected_app_id.clone();
        let state = state.clone();
        let click = gtk::GestureClick::new();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(widget) = gesture.widget() else {
                return;
            };
            let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) else {
                return;
            };

            if widget_is_descendant_of(&picked, &header_widget) {
                return;
            }

            let in_search = widget_is_descendant_of(&picked, &search_widget);
            if search_overlay.is_visible() && !in_search {
                if search.text().is_empty() {
                    search_overlay.set_visible(false);
                    search_btn.set_visible(true);
                } else {
                    gtk::prelude::GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
                }
            }

            let in_apps = widget_is_descendant_of(&picked, &apps_box_widget);
            let in_details = widget_is_descendant_of(&picked, &details_revealer_widget);
            if !in_apps && !in_details {
                apps_box.unselect_all();
                *selected_app_id.borrow_mut() = None;
                let gpus = state.borrow().gpus.clone();
                set_app_details_empty(&details_widgets, &gpus);
                set_details_panel_visible(&content, &details_revealer, &apps_scrolled, false);
            }
        });
        root.add_controller(click);
    }

    {
        let window = window.clone();
        about_btn.connect_clicked(move |_| {
            show_about_dialog(&window);
        });
    }

    {
        let window = window.clone();
        let state = state.clone();
        let apps_box = apps_box.clone();
        let visible_apps = visible_apps.clone();
        let search = search.clone();
        let config = config.clone();
        let details_widgets = details_widgets.clone();
        let details_revealer = details_revealer.clone();
        let content = content.clone();
        let apps_scrolled = apps_scrolled.clone();
        let selected_app_id = selected_app_id.clone();

        refresh_btn.connect_clicked(move |_| {
            info!("refresh requested: rescanning GPUs and applications");
            {
                let mut s = state.borrow_mut();
                s.gpus = detect_gpus();
                s.apps = scan_desktop_entries();
            }

            let current_filter = search.text().to_string();
            let data = state.borrow();
            rebuild_app_list(
                &apps_box,
                &window,
                &data.apps,
                &data.gpus,
                &config,
                &visible_apps,
                &current_filter,
                &details_widgets,
                &selected_app_id,
            );

            if let Some(selected) = selected_app_id.borrow().clone() {
                if let Some(app) = data.apps.iter().find(|a| a.desktop_id == selected).cloned() {
                    let choice = config.borrow().get_choice(&app.desktop_id);
                    set_app_details(&details_widgets, &app, &choice, &data.gpus);
                    set_details_panel_visible(&content, &details_revealer, &apps_scrolled, true);
                } else {
                    set_app_details_empty(&details_widgets, &data.gpus);
                    set_details_panel_visible(&content, &details_revealer, &apps_scrolled, false);
                }
            } else {
                set_app_details_empty(&details_widgets, &data.gpus);
                set_details_panel_visible(&content, &details_revealer, &apps_scrolled, false);
            }
        });
    }

    window.set_content(Some(&root));
    window.present();
}

fn show_about_dialog(window: &adw::ApplicationWindow) {
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
    apply_icon_to_image(&icon, Some(APP_ICON_PATH), 96);
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

fn rebuild_app_list(
    list: &gtk::ListBox,
    window: &adw::ApplicationWindow,
    apps: &[DesktopApp],
    gpus: &[GpuInfo],
    config: &Rc<RefCell<ConfigStore>>,
    visible_apps: &Rc<RefCell<Vec<DesktopApp>>>,
    filter: &str,
    details_widgets: &AppDetailsWidgets,
    selected_app_id: &Rc<RefCell<Option<String>>>,
) {
    clear_listbox(list);
    visible_apps.borrow_mut().clear();
    let gpus_shared = Rc::new(gpus.to_vec());
    let normalized = filter.to_lowercase();

    for app in apps {
        if !normalized.is_empty() && !app.name.to_lowercase().contains(&normalized) {
            continue;
        }
        visible_apps.borrow_mut().push(app.clone());

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(8);
        row.set_margin_bottom(8);
        row.set_margin_start(8);
        row.set_margin_end(14);

        let icon = build_app_icon(app.icon.as_deref(), 32);
        row.append(&icon);

        let center = gtk::Box::new(gtk::Orientation::Vertical, 2);
        center.set_hexpand(true);

        let name = gtk::Label::new(Some(&app.name));
        name.set_xalign(0.0);
        name.add_css_class("title-5");
        center.append(&name);

        let current_choice = config.borrow().get_choice(&app.desktop_id);
        let current = gtk::Label::new(Some(&format!(
            "Current: {}",
            gpu_choice_label(gpus, &current_choice)
        )));
        current.set_xalign(0.0);
        current.add_css_class("caption");
        center.append(&current);

        row.append(&center);

        let choices = build_gpu_choices(gpus);
        let combo = gtk::ComboBoxText::new();
        // Prevent accidental GPU changes when scrolling over the combo.
        let scroll_block =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll_block.connect_scroll(|_, _, _| glib::Propagation::Stop);
        combo.add_controller(scroll_block);
        for (label, _) in &choices {
            combo.append_text(label);
        }

        let selected_index = choices
            .iter()
            .position(|(_, choice)| *choice == current_choice)
            .unwrap_or(0);
        combo.set_active(Some(selected_index as u32));

        {
            let app = app.clone();
            let current = current.clone();
            let config = config.clone();
            let gpus_shared = gpus_shared.clone();
            let window = window.clone();
            let last_choice = Rc::new(RefCell::new(current_choice.clone()));
            let suppress_change = Rc::new(Cell::new(false));
            let details_widgets = details_widgets.clone();
            let selected_app_id = selected_app_id.clone();
            combo.connect_changed(move |c| {
                if suppress_change.get() {
                    suppress_change.set(false);
                    return;
                }

                let Some(idx) = c.active() else {
                    return;
                };

                let choice = choices
                    .get(idx as usize)
                    .map(|(_, choice)| choice.clone())
                    .unwrap_or(GpuChoice::Default);
                let selected_gpu = selected_gpu_for_choice(&gpus_shared, &choice);
                info!(
                    app_name = %app.name,
                    desktop_id = %app.desktop_id,
                    steam_app_id = ?app.steam_app_id,
                    flatpak_app_id = ?app.flatpak_app_id,
                    gpu_choice = %gpu_choice_label(gpus_shared.as_ref(), &choice),
                    selected_gpu = ?selected_gpu.as_ref().map(|g| g.name.clone()),
                    "changing GPU assignment"
                );
                if app.is_steam_game && is_steam_running() {
                    warn!(
                        app_name = %app.name,
                        steam_app_id = ?app.steam_app_id,
                        "Steam is running while changing a Steam game; blocking change"
                    );
                    show_steam_running_dialog(&window);
                    let previous = last_choice.borrow().clone();
                    let previous_idx = choices
                        .iter()
                        .position(|(_, ch)| *ch == previous)
                        .unwrap_or(0);
                    suppress_change.set(true);
                    c.set_active(Some(previous_idx as u32));
                    return;
                }

                config
                    .borrow_mut()
                    .set_choice(&app.desktop_id, choice.clone());
                if let Err(err) = config.borrow().save() {
                    error!(
                        desktop_id = %app.desktop_id,
                        error = %err,
                        "failed to save assignment config"
                    );
                }

                match apply_launcher_override(&app, &choice, selected_gpu.as_ref()) {
                    Ok(()) => info!(
                        app_name = %app.name,
                        desktop_id = %app.desktop_id,
                        "GPU assignment applied successfully"
                    ),
                    Err(err) => warn!(
                        app_name = %app.name,
                        desktop_id = %app.desktop_id,
                        error = %err,
                        "failed to apply GPU assignment override"
                    ),
                }

                last_choice.replace(choice.clone());
                current.set_text(&format!(
                    "Current: {}",
                    gpu_choice_label(gpus_shared.as_ref(), &choice)
                ));
                let selected = selected_app_id.borrow().clone();
                if selected.as_deref() == Some(app.desktop_id.as_str()) {
                    set_app_details(&details_widgets, &app, &choice, &gpus_shared);
                }
            });
        }

        row.append(&combo);
        list.append(&row);
    }
}

fn show_steam_running_dialog(window: &adw::ApplicationWindow) {
    let dialog = gtk::MessageDialog::builder()
        .transient_for(window)
        .modal(true)
        .message_type(gtk::MessageType::Warning)
        .text("Steam is running")
        .secondary_text("Close Steam completely before changing GPU assignment for Steam games.")
        .build();
    dialog.add_button("OK", gtk::ResponseType::Ok);
    dialog.connect_response(|d, _| d.close());
    dialog.present();
}

fn build_gpu_choices(gpus: &[GpuInfo]) -> Vec<(String, GpuChoice)> {
    let mut choices = vec![(
        format!("Default GPU ({})", default_gpu_hint(gpus)),
        GpuChoice::Default,
    )];

    for gpu in gpus {
        if let Some(idx) = gpu.dri_prime_index {
            let pretty = pretty_gpu_name(gpu);
            choices.push((format!("{pretty} (#{idx})"), GpuChoice::Gpu(idx)));
        }
    }

    choices
}

fn gpu_choice_label(gpus: &[GpuInfo], choice: &GpuChoice) -> String {
    match choice {
        GpuChoice::Default => format!("Default GPU ({})", default_gpu_hint(gpus)),
        GpuChoice::Gpu(idx) => gpus
            .iter()
            .find(|g| g.dri_prime_index == Some(*idx))
            .map(|gpu| format!("{} (#{idx})", pretty_gpu_name(gpu)))
            .unwrap_or_else(|| format!("GPU {idx}")),
    }
}

fn default_gpu_hint(gpus: &[GpuInfo]) -> String {
    let name = gpus
        .iter()
        .find(|g| g.dri_prime_index == Some(0))
        .or_else(|| gpus.first())
        .map(pretty_gpu_name)
        .unwrap_or_else(|| "System".to_string());

    truncate_with_dots(&compact_default_name(&name), 18)
}

fn truncate_with_dots(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    let prefix = text.chars().take(keep).collect::<String>();
    format!("{prefix}...")
}

fn compact_default_name(name: &str) -> String {
    let mut out = name.to_string();
    for term in [
        "Radeon",
        "GeForce",
        "Graphics",
        "Series",
        "Integrated",
        "Discrete",
        "AMD",
        "NVIDIA",
        "Intel",
    ] {
        out = out.replace(term, "");
    }
    let compact = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        name.to_string()
    } else {
        compact
    }
}

fn pretty_gpu_name(gpu: &GpuInfo) -> String {
    let source = gpu
        .renderer
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(&gpu.name);

    let mut cleaned = source.trim().to_string();

    if let Some((_, rhs)) = cleaned.split_once(':') {
        cleaned = rhs.trim().to_string();
    }

    for suffix in ["(TM)", "(tm)", "(R)", "(r)", "Corporation", "Inc."] {
        cleaned = cleaned.replace(suffix, "");
    }

    for splitter in [" (", ", ", " [", " / "] {
        if let Some((left, _)) = cleaned.split_once(splitter) {
            cleaned = left.trim().to_string();
        }
    }

    if let Some(pos) = cleaned.find("Series") {
        let keep = &cleaned[..pos + "Series".len()];
        cleaned = keep.trim().to_string();
    }

    let compact = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        format!("GPU {}", gpu.dri_prime_index.unwrap_or(0))
    } else {
        compact
    }
}

fn selected_gpu_for_choice(gpus: &[GpuInfo], choice: &GpuChoice) -> Option<GpuInfo> {
    let GpuChoice::Gpu(idx) = choice else {
        return None;
    };

    gpus.iter()
        .find(|g| g.dri_prime_index == Some(*idx))
        .cloned()
}

fn clear_listbox(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn set_app_details(
    details: &AppDetailsWidgets,
    app: &DesktopApp,
    choice: &GpuChoice,
    gpus: &[GpuInfo],
) {
    let override_path = user_override_path(&app.desktop_id)
        .filter(|path| path.exists())
        .unwrap_or_else(|| app.path.clone());
    apply_icon_to_image(&details.icon, app.icon.as_deref(), 48);
    details.name.set_text(&app.name);
    details
        .assignment_row
        .set_subtitle(&gpu_choice_label(gpus, choice));
    if app.is_steam_game {
        let app_id = app.steam_app_id.as_deref().unwrap_or("unknown");
        details
            .source_row
            .set_subtitle(&format!("Steam game ({app_id})"));
    } else if app.is_heroic_game {
        let platform = app.heroic_platform.as_deref().unwrap_or("unknown");
        let app_name = app.heroic_app_name.as_deref().unwrap_or("unknown");
        details
            .source_row
            .set_subtitle(&format!("Heroic {platform} ({app_name})"));
    } else if app.is_flatpak {
        let app_id = app.flatpak_app_id.as_deref().unwrap_or("unknown");
        details
            .source_row
            .set_subtitle(&format!("Flatpak ({app_id})"));
    } else {
        details
            .source_row
            .set_subtitle("Native desktop entry");
    }
    details
        .desktop_id_row
        .set_subtitle(&app.desktop_id);
    let override_path_str = override_path.to_string_lossy().to_string();
    details.path_row.set_subtitle(&override_path_str);
    details.exec_row.set_subtitle(&app.exec);
    // Do not show the file name in the row; only use tooltip on the button.
    details.desktop_path_label.set_visible(false);
    details.desktop_path_label.set_text("");
    details
        .desktop_open_button
        .set_tooltip_text(Some(&override_path_str));

    // Load the .desktop file contents into the preview.
    let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    match std::fs::read_to_string(&override_path) {
        Ok(contents) => buffer.set_text(&contents),
        Err(err) => buffer.set_text(&format!("Failed to read desktop file:\n{err}")),
    }
    details.desktop_preview.set_buffer(Some(&buffer));
}

fn set_app_details_empty(details: &AppDetailsWidgets, gpus: &[GpuInfo]) {
    details.icon.set_icon_name(Some("application-x-executable"));
    details.name.set_text("Select an application");
    details.assignment_row.set_subtitle(&gpu_choice_label(gpus, &GpuChoice::Default));
    details
        .source_row
        .set_subtitle("Native desktop entry");
    details.desktop_id_row.set_subtitle("-");
    details.path_row.set_subtitle("-");
    details.exec_row.set_subtitle("-");
    details.desktop_path_label.set_visible(false);
    details.desktop_path_label.set_text("Open in external editor");
    details.desktop_open_button.set_tooltip_text(None);
    details.desktop_path_label.set_tooltip_text(None);

    let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    buffer.set_text("Select an application to preview its .desktop file.");
    details.desktop_preview.set_buffer(Some(&buffer));
}

fn build_app_icon(icon: Option<&str>, pixel_size: i32) -> gtk::Image {
    let image = gtk::Image::new();
    apply_icon_to_image(&image, icon, pixel_size);
    image
}

fn apply_icon_to_image(image: &gtk::Image, icon: Option<&str>, pixel_size: i32) {
    image.set_pixel_size(pixel_size);

    if let Some(icon_value) = icon {
        if let Some(path) = icon_file_path(icon_value) {
            let file = gio::File::for_path(path);
            match gtk::gdk::Texture::from_file(&file) {
                Ok(texture) => {
                    image.set_paintable(Some(&texture));
                    return;
                }
                Err(_) => {
                    // Fallback to icon name below.
                }
            }
        }

        image.set_icon_name(Some(icon_value));
        return;
    }

    image.set_icon_name(Some("application-x-executable"));
}

fn icon_file_path(icon: &str) -> Option<&Path> {
    let path = Path::new(icon);
    if path.is_absolute() && path.exists() {
        Some(path)
    } else {
        None
    }
}

fn widget_is_descendant_of(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(node) = current {
        if node == *ancestor {
            return true;
        }
        current = node.parent();
    }
    false
}

fn set_details_panel_visible(
    _content: &gtk::Paned,
    details_revealer: &gtk::Revealer,
    apps_scrolled: &gtk::ScrolledWindow,
    visible: bool,
) {
    if visible {
        apps_scrolled.set_margin_end(10);
    } else {
        apps_scrolled.set_margin_end(0);
    }
    details_revealer.set_reveal_child(visible);
}
