use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::error;

use crate::config::ConfigStore;
use crate::models::GpuInfo;
use crate::nvidia::{get_current_mode, switch_graphics_mode, DisplayManager, GraphicsMode, NvidiaSwitchConfig, reset_all, reset_sddm};

fn has_nvidia_gpu(gpus: &[GpuInfo]) -> bool {
    gpus.iter().any(|g| {
        let name = g
            .renderer
            .as_deref()
            .unwrap_or(&g.name)
            .to_lowercase();
        name.contains("nvidia") || name.contains("geforce")
    })
}

pub(crate) fn show_settings_dialog(
    parent: &adw::ApplicationWindow,
    gpus: &[GpuInfo],
    config: &Rc<RefCell<ConfigStore>>,
) {
    let has_nvidia = has_nvidia_gpu(gpus);
    let current_mode = get_current_mode();

    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Kaede settings")
        .default_width(960)
        .default_height(540)
        .build();

    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    let switcher = adw::ViewSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_hexpand(true);

    // Application-wide settings (General tab)
    let general_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    general_page.set_margin_top(12);
    general_page.set_margin_bottom(12);
    general_page.set_margin_start(18);
    general_page.set_margin_end(18);

    let general_desc = gtk::Label::new(Some(
        "Configure how Kaede discovers and displays applications.",
    ));
    general_desc.set_wrap(true);
    general_desc.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    general_desc.add_css_class("dim-label");
    general_desc.set_xalign(0.0);
    general_page.append(&general_desc);

    let app_list = gtk::ListBox::new();
    app_list.add_css_class("boxed-list");
    app_list.set_selection_mode(gtk::SelectionMode::None);

    let show_steam_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    show_steam_switch.set_active(config.borrow().show_steam_apps());
    let show_steam_row = adw::ActionRow::builder()
        .title("Show Steam apps")
        .subtitle("Include Steam games and entries in the list")
        .build();
    show_steam_row.add_suffix(&show_steam_switch);
    show_steam_row.set_activatable_widget(Some(&show_steam_switch));
    app_list.append(&show_steam_row);

    let show_heroic_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    show_heroic_switch.set_active(config.borrow().show_heroic_apps());
    let show_heroic_row = adw::ActionRow::builder()
        .title("Show Heroic games")
        .subtitle("Include entries imported from Heroic")
        .build();
    show_heroic_row.add_suffix(&show_heroic_switch);
    show_heroic_row.set_activatable_widget(Some(&show_heroic_switch));
    app_list.append(&show_heroic_row);

    let show_flatpak_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    show_flatpak_switch.set_active(config.borrow().show_flatpak_apps());
    let show_flatpak_row = adw::ActionRow::builder()
        .title("Show Flatpak apps")
        .subtitle("Include applications detected from Flatpak")
        .build();
    show_flatpak_row.add_suffix(&show_flatpak_switch);
    show_flatpak_row.set_activatable_widget(Some(&show_flatpak_switch));
    app_list.append(&show_flatpak_row);

    general_page.append(&app_list);

    // NVIDIA-specific settings (may be disabled when no NVIDIA GPU is present) - Advanced tab
    let nvidia_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    nvidia_page.set_margin_top(12);
    nvidia_page.set_margin_bottom(12);
    nvidia_page.set_margin_start(18);
    nvidia_page.set_margin_end(18);

    let nvidia_desc = gtk::Label::new(Some(
        "Advanced options for NVIDIA systems. These settings may require root privileges and a reboot.",
    ));
    nvidia_desc.set_wrap(true);
    nvidia_desc.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    nvidia_desc.add_css_class("dim-label");
    nvidia_desc.set_xalign(0.0);
    nvidia_page.append(&nvidia_desc);

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);

    let mode_dropdown =
        gtk::DropDown::from_strings(&["Integrated (iGPU only)", "Hybrid", "NVIDIA only"]);
    mode_dropdown.set_valign(gtk::Align::Center);
    mode_dropdown.set_vexpand(false);

    let active_index = match current_mode {
        GraphicsMode::Integrated => 0,
        GraphicsMode::Hybrid => 1,
        GraphicsMode::Nvidia => 2,
    };
    mode_dropdown.set_selected(active_index);

    let mode_row = adw::ActionRow::builder()
        .title("Preferred mode")
        .subtitle("Applies on next reboot")
        .build();
    mode_row.add_suffix(&mode_dropdown);
    mode_row.set_activatable_widget(Some(&mode_dropdown));
    list.append(&mode_row);

    // ForceCompositionPipeline toggle
    let force_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    let force_row = adw::ActionRow::builder()
        .title("ForceCompositionPipeline")
        .subtitle("NVIDIA mode: enable tear-free composition pipeline")
        .build();
    force_row.add_suffix(&force_switch);
    force_row.set_activatable_widget(Some(&force_switch));
    list.append(&force_row);

    // Coolbits toggle + value
    let coolbits_entry = gtk::SpinButton::with_range(0.0, 255.0, 1.0);
    coolbits_entry.set_value(28.0);
    coolbits_entry.set_valign(gtk::Align::Center);
    coolbits_entry.set_vexpand(false);
    let coolbits_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    let coolbits_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    coolbits_box.set_valign(gtk::Align::Center);
    coolbits_box.set_vexpand(false);
    coolbits_box.append(&coolbits_switch);
    coolbits_box.append(&coolbits_entry);
    let coolbits_row = adw::ActionRow::builder()
        .title("Coolbits")
        .subtitle("NVIDIA mode: enable overclocking / fan control (advanced)")
        .build();
    coolbits_row.add_suffix(&coolbits_box);
    coolbits_row.set_activatable_widget(Some(&coolbits_switch));
    list.append(&coolbits_row);

    // RTD3 Power Management
    let rtd3_dropdown = gtk::DropDown::from_strings(&["Disabled", "0", "1", "2", "3"]);
    rtd3_dropdown.set_valign(gtk::Align::Center);
    rtd3_dropdown.set_vexpand(false);
    rtd3_dropdown.set_selected(0);
    let rtd3_row = adw::ActionRow::builder()
        .title("RTD3 power management")
        .subtitle("Hybrid mode: PCIe Runtime D3 (0–3)")
        .build();
    rtd3_row.add_suffix(&rtd3_dropdown);
    rtd3_row.set_activatable_widget(Some(&rtd3_dropdown));
    list.append(&rtd3_row);

    // Kernel modules flavor
    let nvidia_current_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    let nvidia_current_row = adw::ActionRow::builder()
        .title("Use nvidia-current modules")
        .subtitle("Use nvidia-current* module names instead of nvidia*")
        .build();
    nvidia_current_row.add_suffix(&nvidia_current_switch);
    nvidia_current_row.set_activatable_widget(Some(&nvidia_current_switch));
    list.append(&nvidia_current_row);

    // Display Manager selection
    let dm_dropdown =
        gtk::DropDown::from_strings(&["Auto-detect", "GDM", "GDM3", "SDDM", "LightDM"]);
    dm_dropdown.set_valign(gtk::Align::Center);
    dm_dropdown.set_vexpand(false);
    dm_dropdown.set_selected(0);
    let dm_row = adw::ActionRow::builder()
        .title("Display Manager")
        .subtitle("NVIDIA mode: used for login screen configuration")
        .build();
    dm_row.add_suffix(&dm_dropdown);
    dm_row.set_activatable_widget(Some(&dm_dropdown));
    list.append(&dm_row);

    let nvidia_help = gtk::Label::new(Some(
        "NVIDIA settings apply system-wide and require a reboot; use only if you understand how hybrid graphics work on your system.",
    ));
    nvidia_help.set_wrap(true);
    nvidia_help.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    nvidia_help.add_css_class("dim-label");
    nvidia_help.set_xalign(0.0);

    nvidia_page.append(&list);
    nvidia_page.append(&nvidia_help);

    let gen_page = stack.add_titled(&general_page, Some("general"), "General");
    gen_page.set_icon_name(Some("view-list-symbolic"));
    let nvid_page = stack.add_titled(&nvidia_page, Some("nvidia"), "NVIDIA & power");
    nvid_page.set_icon_name(Some("emblem-system-symbolic"));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let header_row = gtk::CenterBox::new();
    header_row.set_margin_bottom(8);

    let title_label = gtk::Label::new(None);
    title_label.set_markup("<b>Settings</b>");
    title_label.set_xalign(0.0);
    title_label.set_margin_start(6);
    title_label.set_margin_top(2);
    header_row.set_start_widget(Some(&title_label));

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    header_row.set_center_widget(Some(&switcher));

    let close_img = gtk::Image::from_icon_name("window-close-symbolic");
    close_img.set_pixel_size(18);

    let close_btn = gtk::Button::builder()
        .child(&close_img)
        .build();
    close_btn.add_css_class("circular");
    close_btn.set_size_request(26, 26);
    close_btn.set_valign(gtk::Align::Start);
    close_btn.set_halign(gtk::Align::End);
    {
        let window = window.clone();
        close_btn.connect_clicked(move |_| {
            window.close();
        });
    }
    header_row.set_end_widget(Some(&close_btn));

    root.append(&header_row);
    root.append(&stack);

    if !has_nvidia {
        mode_dropdown.set_sensitive(false);
        force_switch.set_sensitive(false);
        coolbits_switch.set_sensitive(false);
        coolbits_entry.set_sensitive(false);
        rtd3_dropdown.set_sensitive(false);
        nvidia_current_switch.set_sensitive(false);
        dm_dropdown.set_sensitive(false);
    }

    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    button_box.set_halign(gtk::Align::Fill);
    button_box.set_margin_top(6);
    button_box.set_margin_bottom(12);
    button_box.set_margin_start(18);
    button_box.set_margin_end(18);

    let reset_btn = gtk::Button::with_label("Full reset");
    reset_btn.add_css_class("destructive-action");
    let reset_sddm_btn = gtk::Button::with_label("Reset SDDM Xsetup");
    let cancel_btn = gtk::Button::with_label("Cancel");
    let apply_btn = gtk::Button::with_label("Apply");
    apply_btn.add_css_class("suggested-action");

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    button_box.append(&reset_btn);
    button_box.append(&reset_sddm_btn);
    button_box.append(&spacer);
    button_box.append(&cancel_btn);
    button_box.append(&apply_btn);
    root.append(&button_box);

    window.set_content(Some(&root));

    if !has_nvidia {
        reset_btn.set_sensitive(false);
        reset_sddm_btn.set_sensitive(false);
    }

    {
        let window = window.clone();
        cancel_btn.connect_clicked(move |_| {
            window.close();
        });
    }

    {
        let parent = parent.clone();
        let window = window.clone();
        reset_btn.connect_clicked(move |_| {
            window.close();
            match reset_all() {
                Ok(()) => {
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .message_type(gtk::MessageType::Info)
                        .text("NVIDIA configuration reset")
                        .secondary_text("All NVIDIA graphics configuration files managed by Kaede were removed.\nReboot the system to fully apply changes.")
                        .build();
                    dlg.add_button("OK", gtk::ResponseType::Ok);
                    dlg.connect_response(|d, _| d.close());
                    dlg.present();
                }
                Err(err) => {
                    error!(%err, "failed to reset NVIDIA configuration");
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .message_type(gtk::MessageType::Error)
                        .text("Failed to reset NVIDIA configuration")
                        .secondary_text(&err)
                        .build();
                    dlg.add_button("Close", gtk::ResponseType::Close);
                    dlg.connect_response(|d, _| d.close());
                    dlg.present();
                }
            }
        });
    }

    {
        let parent = parent.clone();
        let window = window.clone();
        reset_sddm_btn.connect_clicked(move |_| {
            window.close();
            match reset_sddm() {
                Ok(()) => {
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .message_type(gtk::MessageType::Info)
                        .text("SDDM Xsetup reset")
                        .secondary_text("The SDDM Xsetup script was restored to a minimal default.\nReboot the system to apply changes.")
                        .build();
                    dlg.add_button("OK", gtk::ResponseType::Ok);
                    dlg.connect_response(|d, _| d.close());
                    dlg.present();
                }
                Err(err) => {
                    error!(%err, "failed to reset SDDM Xsetup");
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .message_type(gtk::MessageType::Error)
                        .text("Failed to reset SDDM Xsetup")
                        .secondary_text(&err)
                        .build();
                    dlg.add_button("Close", gtk::ResponseType::Close);
                    dlg.connect_response(|d, _| d.close());
                    dlg.present();
                }
            }
        });
    }

    {
        let parent = parent.clone();
        let window = window.clone();
        let config = config.clone();
        apply_btn.connect_clicked(move |_| {
            let selected = mode_dropdown.selected();
            let mode = match selected {
                0 => GraphicsMode::Integrated,
                1 => GraphicsMode::Hybrid,
                2 => GraphicsMode::Nvidia,
                _ => get_current_mode(),
            };

            let enable_force_comp = force_switch.is_active();
            let coolbits_value = if coolbits_switch.is_active() {
                Some(coolbits_entry.value() as i32)
            } else {
                None
            };
            let rtd3_value = match rtd3_dropdown.selected() {
                0 => None,
                1 => Some(0),
                2 => Some(1),
                3 => Some(2),
                4 => Some(3),
                _ => None,
            };
            let use_nvidia_current = nvidia_current_switch.is_active();

            let display_manager = match dm_dropdown.selected() {
                0 => None,
                1 => Some(DisplayManager::Gdm),
                2 => Some(DisplayManager::Gdm3),
                3 => Some(DisplayManager::Sddm),
                4 => Some(DisplayManager::Lightdm),
                _ => None,
            };

            window.close();

            {
                let mut cfg = config.borrow_mut();
                cfg.set_show_steam_apps(show_steam_switch.is_active());
                cfg.set_show_heroic_apps(show_heroic_switch.is_active());
                cfg.set_show_flatpak_apps(show_flatpak_switch.is_active());
                if let Err(err) = cfg.save() {
                    error!(%err, "failed to save app settings");
                }
            }

            if !has_nvidia {
                let dlg = gtk::MessageDialog::builder()
                    .transient_for(&parent)
                    .modal(true)
                    .message_type(gtk::MessageType::Info)
                    .text("Settings updated")
                    .secondary_text("Application settings were updated successfully.")
                    .build();
                dlg.add_button("OK", gtk::ResponseType::Ok);
                dlg.connect_response(|d, _| d.close());
                dlg.present();
                return;
            }

            let config = NvidiaSwitchConfig {
                mode,
                display_manager,
                enable_force_comp,
                coolbits_value,
                rtd3_value,
                use_nvidia_current,
            };

            match switch_graphics_mode(&config) {
                Ok(()) => {
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .message_type(gtk::MessageType::Info)
                        .text("GPU mode switch requested")
                        .secondary_text("Configuration files were updated successfully.\nYou should reboot the system for changes to take effect.")
                        .build();
                    dlg.add_button("OK", gtk::ResponseType::Ok);
                    dlg.connect_response(|d, _| d.close());
                    dlg.present();
                }
                Err(err) => {
                    error!(%err, "failed to switch NVIDIA graphics mode");
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .message_type(gtk::MessageType::Error)
                        .text("Failed to switch NVIDIA graphics mode")
                        .secondary_text(&err)
                        .build();
                    dlg.add_button("Close", gtk::ResponseType::Close);
                    dlg.connect_response(|d, _| d.close());
                    dlg.present();
                }
            }
        });
    }

    window.present();
}

