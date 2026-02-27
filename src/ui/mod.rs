use crate::config::ConfigStore;
use crate::desktop::scan_desktop_entries;
use crate::gpu::detect_gpus;
use crate::models::{DesktopApp, GpuInfo};
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::{info, warn};

const APP_NAME: &str = "Kaede";
const APP_DESCRIPTION: &str =
    "Select and manage GPU assignments for apps, games, and launchers on Linux.";
const APP_AUTHOR: &str = "Esther";
const APP_GITHUB: &str = "https://github.com/esther/KaedeGPU";
const APP_LICENSE: &str = "GNU GPL-3.0";
// Use the installed themed icon name so it works from the packaged build.
const APP_ICON_PATH: &str = "com.kaede.gpu-manager";

mod about;
mod app_list;
mod details;
mod settings;
mod util;

use self::about::show_about_dialog;
use self::app_list::rebuild_app_list;
use self::details::{set_app_details, set_app_details_empty, AppDetailsWidgets};
use self::settings::build_settings_widget;
use self::util::{set_details_panel_visible, widget_is_descendant_of};

#[derive(Clone)]
struct UiState {
    gpus: Vec<GpuInfo>,
    apps: Vec<DesktopApp>,
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
    // (steam, heroic, flatpak, native) — session-level filter, independent of settings
    let ui_filter: Rc<RefCell<(bool, bool, bool, bool)>> = Rc::new(RefCell::new((true, true, true, true)));
    let filter_suspended: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));

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

    let back_btn = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Back")
        .build();
    back_btn.set_visible(false);
    header.pack_start(&back_btn);

    let refresh_btn = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh GPU and app scan")
        .build();
    let settings_btn = gtk::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("NVIDIA graphics mode")
        .build();
    let about_btn = gtk::Button::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text("About Kaede")
        .build();
    header.pack_end(&settings_btn);
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

    let search_slot = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    search_slot.append(&search_btn);
    search_slot.append(&search_overlay);

    // Filter popover content
    let filter_popover_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    filter_popover_box.set_margin_top(8);
    filter_popover_box.set_margin_bottom(8);
    filter_popover_box.set_margin_start(12);
    filter_popover_box.set_margin_end(12);

    let steam_check = gtk::CheckButton::with_label("Steam games");
    steam_check.set_active(true);
    let heroic_check = gtk::CheckButton::with_label("Heroic games");
    heroic_check.set_active(true);
    let flatpak_check = gtk::CheckButton::with_label("Flatpak apps");
    flatpak_check.set_active(true);
    let native_check = gtk::CheckButton::with_label("Native / Desktop");
    native_check.set_active(true);

    let filter_sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    filter_sep.set_margin_top(6);
    filter_sep.set_margin_bottom(2);

    let clear_filters_btn = gtk::Button::with_label("Clear filters");
    clear_filters_btn.add_css_class("flat");
    clear_filters_btn.set_halign(gtk::Align::Center);

    filter_popover_box.append(&steam_check);
    filter_popover_box.append(&heroic_check);
    filter_popover_box.append(&flatpak_check);
    filter_popover_box.append(&native_check);
    filter_popover_box.append(&filter_sep);
    filter_popover_box.append(&clear_filters_btn);

    let filter_popover = gtk::Popover::new();
    filter_popover.set_child(Some(&filter_popover_box));

    let filter_menu_btn = gtk::MenuButton::new();
    filter_menu_btn.set_icon_name("view-list-symbolic");
    filter_menu_btn.set_tooltip_text(Some("Filter apps by source"));
    filter_menu_btn.set_popover(Some(&filter_popover));
    filter_menu_btn.set_valign(gtk::Align::Center);

    search_slot.append(&filter_menu_btn);
    header.pack_start(&search_slot);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.append(&header);

    let content = gtk::Paned::new(gtk::Orientation::Horizontal);
    content.set_margin_top(0);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    // Content stack: switches between apps view and settings view
    let content_stack = gtk::Stack::new();
    content_stack.set_vexpand(true);
    content_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    content_stack.set_transition_duration(220);
    content_stack.add_named(&content, Some("apps"));

    let settings_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    settings_slot.set_vexpand(true);
    content_stack.add_named(&settings_slot, Some("settings"));

    root.append(&content_stack);

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
    desktop_header.set_margin_bottom(8);
    desktop_header.set_margin_start(12);
    desktop_header.set_margin_end(8);

    let desktop_title = gtk::Label::new(Some(".desktop file"));
    desktop_title.set_xalign(0.0);
    desktop_title.add_css_class("heading");
    desktop_title.set_hexpand(true);

    let desktop_open_button = gtk::Button::with_label("Open in editor");
    desktop_open_button.add_css_class("flat");
    desktop_open_button.set_valign(gtk::Align::Center);

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
        .visible(false)
        .transition_type(gtk::RevealerTransitionType::SlideLeft)
        .build();
    details_revealer.connect_child_revealed_notify(|revealer| {
        if !revealer.reveals_child() && !revealer.is_child_revealed() {
            revealer.set_visible(false);
        }
    });
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
            *ui_filter.borrow(),
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
        let ui_filter = ui_filter.clone();
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
                *ui_filter.borrow(),
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
        let search_btn_widget = search_btn.clone().upcast::<gtk::Widget>();
        let refresh_btn_widget = refresh_btn.clone().upcast::<gtk::Widget>();
        let settings_btn_widget = settings_btn.clone().upcast::<gtk::Widget>();
        let about_btn_widget = about_btn.clone().upcast::<gtk::Widget>();
        let filter_menu_btn_widget = filter_menu_btn.clone().upcast::<gtk::Widget>();
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
        click.connect_released(move |gesture, _n_press, x, y| {
            let Some(widget) = gesture.widget() else {
                return;
            };
            let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) else {
                return;
            };

            let in_header = widget_is_descendant_of(&picked, &header_widget);
            let in_search_btn = widget_is_descendant_of(&picked, &search_btn_widget);
            let in_search_entry = widget_is_descendant_of(&picked, &search_widget);
            let in_refresh = widget_is_descendant_of(&picked, &refresh_btn_widget);
            let in_settings = widget_is_descendant_of(&picked, &settings_btn_widget);
            let in_about = widget_is_descendant_of(&picked, &about_btn_widget);
            let in_filter = widget_is_descendant_of(&picked, &filter_menu_btn_widget);

            let is_action_widget =
                in_search_btn || in_search_entry || in_refresh || in_settings || in_about || in_filter;

            if search_overlay.is_visible() && !in_search_entry {
                if search.text().is_empty() {
                    search_overlay.set_visible(false);
                    search_btn.set_visible(true);
                } else if !is_action_widget {
                    gtk::prelude::GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
                }
            }

            if is_action_widget {
                return;
            }

            let in_apps = widget_is_descendant_of(&picked, &apps_box_widget);
            let in_details = widget_is_descendant_of(&picked, &details_revealer_widget);
            if in_header || (!in_apps && !in_details) {
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

    // Filter CheckButton handlers (with suspend flag to avoid redundant rebuilds on Clear)
    macro_rules! on_check_filter {
        ($check:expr, $field:tt) => {{
            let apps_box = apps_box.clone();
            let window = window.clone();
            let state = state.clone();
            let config = config.clone();
            let visible_apps = visible_apps.clone();
            let search = search.clone();
            let details_widgets = details_widgets.clone();
            let selected_app_id = selected_app_id.clone();
            let ui_filter = ui_filter.clone();
            let filter_suspended = filter_suspended.clone();
            $check.connect_toggled(move |check| {
                if filter_suspended.get() { return; }
                ui_filter.borrow_mut().$field = check.is_active();
                let text = search.text().to_string();
                let data = state.borrow();
                rebuild_app_list(
                    &apps_box, &window, &data.apps, &data.gpus,
                    &config, &visible_apps, &text, &details_widgets, &selected_app_id,
                    *ui_filter.borrow(),
                );
            });
        }};
    }
    on_check_filter!(steam_check, 0);
    on_check_filter!(heroic_check, 1);
    on_check_filter!(flatpak_check, 2);
    on_check_filter!(native_check, 3);

    // Clear filters: reset all checks + rebuild once
    {
        let apps_box = apps_box.clone();
        let window = window.clone();
        let state = state.clone();
        let config = config.clone();
        let visible_apps = visible_apps.clone();
        let search = search.clone();
        let details_widgets = details_widgets.clone();
        let selected_app_id = selected_app_id.clone();
        let ui_filter = ui_filter.clone();
        let filter_suspended = filter_suspended.clone();
        let filter_popover = filter_popover.clone();
        clear_filters_btn.connect_clicked(move |_| {
            filter_suspended.set(true);
            steam_check.set_active(true);
            heroic_check.set_active(true);
            flatpak_check.set_active(true);
            native_check.set_active(true);
            *ui_filter.borrow_mut() = (true, true, true, true);
            filter_suspended.set(false);
            filter_popover.popdown();
            let text = search.text().to_string();
            let data = state.borrow();
            rebuild_app_list(
                &apps_box, &window, &data.apps, &data.gpus,
                &config, &visible_apps, &text, &details_widgets, &selected_app_id,
                *ui_filter.borrow(),
            );
        });
    }

    {
        let window = window.clone();
        let state = state.clone();
        let config = config.clone();
        let content_stack = content_stack.clone();
        let settings_slot = settings_slot.clone();
        let back_btn = back_btn.clone();
        let settings_btn_ref = settings_btn.clone();
        let refresh_btn_ref = refresh_btn.clone();
        let about_btn_ref = about_btn.clone();
        let search_slot = search_slot.clone();
        let title = title.clone();
        let header = header.clone();
        settings_btn.connect_clicked(move |_| {
            while let Some(child) = settings_slot.first_child() {
                settings_slot.remove(&child);
            }
            let gpus = state.borrow().gpus.clone();
            let (widget, switcher) = build_settings_widget(&window, &gpus, &config);
            settings_slot.append(&widget);
            header.set_title_widget(Some(&switcher));
            content_stack.set_visible_child_name("settings");
            back_btn.set_visible(true);
            settings_btn_ref.set_visible(false);
            refresh_btn_ref.set_visible(false);
            about_btn_ref.set_visible(false);
            search_slot.set_visible(false);
            title.set_title("Settings");
        });
    }

    {
        let content_stack = content_stack.clone();
        let back_btn_ref = back_btn.clone();
        let settings_btn = settings_btn.clone();
        let refresh_btn = refresh_btn.clone();
        let about_btn = about_btn.clone();
        let search_slot = search_slot.clone();
        let title = title.clone();
        let header = header.clone();
        back_btn.connect_clicked(move |_| {
            content_stack.set_visible_child_name("apps");
            header.set_title_widget(Some(&title));
            back_btn_ref.set_visible(false);
            settings_btn.set_visible(true);
            refresh_btn.set_visible(true);
            about_btn.set_visible(true);
            search_slot.set_visible(true);
            title.set_title("Kaede");
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
        let ui_filter = ui_filter.clone();

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
                *ui_filter.borrow(),
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
