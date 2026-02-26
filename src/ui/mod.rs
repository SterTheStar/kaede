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
    assignment: gtk::Label,
    source: gtk::Label,
    desktop_id: gtk::Label,
    path: gtk::Label,
    exec: gtk::Label,
    raw_buffer: gtk::TextBuffer,
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

    let details_outer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    details_outer.set_margin_top(12);
    details_outer.set_margin_bottom(12);
    details_outer.set_margin_start(12);
    details_outer.set_margin_end(12);
    details_outer.set_size_request(380, -1);
    details_outer.set_vexpand(true);

    let details_title = gtk::Label::new(Some("Application Details"));
    details_title.add_css_class("title-4");
    details_title.set_xalign(0.0);
    details_outer.append(&details_title);

    let summary_frame = gtk::Frame::new(None);
    summary_frame.add_css_class("card");
    let summary_card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    summary_card.set_margin_top(12);
    summary_card.set_margin_bottom(12);
    summary_card.set_margin_start(12);
    summary_card.set_margin_end(12);

    let details_top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let details_icon = gtk::Image::builder()
        .icon_name("application-x-executable")
        .pixel_size(48)
        .build();
    details_icon.set_halign(gtk::Align::Start);
    details_icon.set_valign(gtk::Align::Start);
    let details_name = gtk::Label::new(Some("Select an application"));
    details_name.set_xalign(0.0);
    details_name.add_css_class("heading");
    details_name.set_wrap(true);
    details_name.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    details_name.set_hexpand(true);
    details_top.append(&details_icon);
    details_top.append(&details_name);
    summary_card.append(&details_top);

    let details_assignment = gtk::Label::new(Some("Current GPU: Default GPU"));
    details_assignment.set_xalign(0.0);
    details_assignment.add_css_class("caption");
    details_assignment.set_wrap(true);
    details_assignment.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    summary_card.append(&details_assignment);

    let details_source = gtk::Label::new(Some("Source: Native desktop entry"));
    details_source.set_xalign(0.0);
    details_source.add_css_class("caption");
    details_source.set_wrap(true);
    details_source.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    summary_card.append(&details_source);

    let details_id = gtk::Label::new(Some("Desktop ID: -"));
    details_id.set_xalign(0.0);
    details_id.add_css_class("caption");
    details_id.set_wrap(true);
    details_id.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    summary_card.append(&details_id);

    let details_path = gtk::Label::new(Some("Path: -"));
    details_path.set_xalign(0.0);
    details_path.add_css_class("caption");
    details_path.set_wrap(true);
    details_path.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    details_path.set_selectable(true);
    summary_card.append(&details_path);

    let details_exec = gtk::Label::new(Some("Exec: -"));
    details_exec.set_xalign(0.0);
    details_exec.add_css_class("caption");
    details_exec.set_wrap(true);
    details_exec.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    details_exec.set_selectable(true);
    summary_card.append(&details_exec);
    summary_frame.set_child(Some(&summary_card));
    details_outer.append(&summary_frame);

    let raw_buffer = gtk::TextBuffer::new(None);
    raw_buffer.set_text("Select an application to inspect its desktop entry.");
    let raw_title = gtk::Label::new(Some(".desktop Content"));
    raw_title.set_xalign(0.0);
    raw_title.add_css_class("caption");
    raw_title.set_hexpand(true);
    let raw_copy_btn = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy desktop content")
        .build();
    raw_copy_btn.add_css_class("flat");
    let raw_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    raw_header.append(&raw_title);
    raw_header.append(&raw_copy_btn);
    let raw_view = gtk::TextView::builder()
        .buffer(&raw_buffer)
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .cursor_visible(false)
        .build();
    let raw_scrolled = gtk::ScrolledWindow::builder()
        .child(&raw_view)
        .min_content_height(220)
        .vexpand(true)
        .build();
    raw_scrolled.set_hexpand(true);
    raw_scrolled.set_vexpand(true);

    let raw_frame = gtk::Frame::new(None);
    raw_frame.add_css_class("card");
    raw_frame.set_vexpand(true);
    let raw_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    raw_box.set_margin_top(12);
    raw_box.set_margin_bottom(12);
    raw_box.set_margin_start(12);
    raw_box.set_margin_end(12);
    raw_box.set_vexpand(true);
    raw_box.append(&raw_header);
    raw_box.append(&raw_scrolled);
    raw_frame.set_child(Some(&raw_box));
    details_outer.append(&raw_frame);

    let details_revealer = gtk::Revealer::builder()
        .reveal_child(false)
        .transition_type(gtk::RevealerTransitionType::SlideLeft)
        .build();
    details_revealer.set_child(Some(&details_outer));
    content.set_end_child(None::<&gtk::Widget>);
    content.set_resize_end_child(false);
    content.set_shrink_end_child(true);

    let details_widgets = AppDetailsWidgets {
        icon: details_icon,
        name: details_name,
        assignment: details_assignment,
        source: details_source,
        desktop_id: details_id,
        path: details_path,
        exec: details_exec,
        raw_buffer,
    };

    {
        let raw_buffer = details_widgets.raw_buffer.clone();
        raw_copy_btn.connect_clicked(move |_| {
            let (start, end) = raw_buffer.bounds();
            let text = raw_buffer.text(&start, &end, false).to_string();
            if let Some(display) = gtk::gdk::Display::default() {
                display.clipboard().set_text(&text);
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
        .assignment
        .set_text(&format!("Current GPU: {}", gpu_choice_label(gpus, choice)));
    if app.is_steam_game {
        let app_id = app.steam_app_id.as_deref().unwrap_or("unknown");
        details
            .source
            .set_text(&format!("Source: Steam game ({app_id})"));
    } else if app.is_heroic_game {
        let platform = app.heroic_platform.as_deref().unwrap_or("unknown");
        let app_name = app.heroic_app_name.as_deref().unwrap_or("unknown");
        details
            .source
            .set_text(&format!("Source: Heroic {platform} ({app_name})"));
    } else if app.is_flatpak {
        let app_id = app.flatpak_app_id.as_deref().unwrap_or("unknown");
        details
            .source
            .set_text(&format!("Source: Flatpak ({app_id})"));
    } else {
        details.source.set_text("Source: Native desktop entry");
    }
    details
        .desktop_id
        .set_text(&format!("Desktop ID: {}", app.desktop_id));
    details
        .path
        .set_text(&format!("Path: {}", override_path.to_string_lossy()));
    details.exec.set_text(&format!("Exec: {}", app.exec));

    let raw = std::fs::read_to_string(&override_path)
        .unwrap_or_else(|_| "Failed to read desktop file content.".to_string());
    details.raw_buffer.set_text(&raw);
}

fn set_app_details_empty(details: &AppDetailsWidgets, gpus: &[GpuInfo]) {
    details.icon.set_icon_name(Some("application-x-executable"));
    details.name.set_text("Select an application");
    details
        .assignment
        .set_text(&format!("Current GPU: {}", gpu_choice_label(gpus, &GpuChoice::Default)));
    details.source.set_text("Source: Native desktop entry");
    details.desktop_id.set_text("Desktop ID: -");
    details.path.set_text("Path: -");
    details.exec.set_text("Exec: -");
    details
        .raw_buffer
        .set_text("Select an application to inspect its desktop entry.");
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
    content: &gtk::Paned,
    details_revealer: &gtk::Revealer,
    apps_scrolled: &gtk::ScrolledWindow,
    visible: bool,
) {
    if visible {
        apps_scrolled.set_margin_end(10);
        if content.end_child().is_none() {
            content.set_end_child(Some(details_revealer));
        }
        details_revealer.set_reveal_child(true);
    } else {
        apps_scrolled.set_margin_end(0);
        details_revealer.set_reveal_child(false);
        content.set_end_child(None::<&gtk::Widget>);
    }
}
