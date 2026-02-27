use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use tracing::{error, info, warn};

use crate::config::ConfigStore;
use crate::launcher::apply_launcher_override;
use crate::models::{DesktopApp, GpuChoice, GpuInfo};
use crate::steam::is_steam_running;

use super::details::{
    build_app_icon, build_gpu_choices, gpu_choice_label, selected_gpu_for_choice, AppDetailsWidgets,
};
use super::util::clear_listbox;

pub(crate) fn rebuild_app_list(
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
                    super::details::set_app_details(&details_widgets, &app, &choice, &gpus_shared);
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

