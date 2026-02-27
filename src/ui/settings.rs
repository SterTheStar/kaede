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

pub(crate) fn build_settings_widget(
    window: &adw::ApplicationWindow,
    gpus: &[GpuInfo],
    config: &Rc<RefCell<ConfigStore>>,
) -> (gtk::Box, adw::ViewSwitcher) {
    let has_nvidia = has_nvidia_gpu(gpus);
    let current_mode = get_current_mode();
    let skip_warning = config.borrow().skip_nvidia_warning();

    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);

    // Application-wide settings (General tab)
    let general_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build();

    let general_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    general_page.set_margin_top(12);
    general_page.set_margin_bottom(12);
    general_page.set_margin_start(18);
    general_page.set_margin_end(18);
    general_scrolled.set_child(Some(&general_page));

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

    let use_env_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    use_env_switch.set_active(config.borrow().use_env_wrapper());
    let use_env_row = adw::ActionRow::builder()
        .title("Use 'env' command wrapper")
        .subtitle("Prepends 'env' before GPU vars in the launch command. Does not apply to Heroic games.")
        .build();
    use_env_row.add_suffix(&use_env_switch);
    use_env_row.set_activatable_widget(Some(&use_env_switch));
    app_list.append(&use_env_row);

    let check_updates_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
    check_updates_switch.set_active(config.borrow().check_updates_at_startup());
    let check_updates_row = adw::ActionRow::builder()
        .title("Check for updates on startup")
        .subtitle("Automatically check for new versions on GitHub when opening Kaede")
        .build();
    check_updates_row.add_suffix(&check_updates_switch);
    check_updates_row.set_activatable_widget(Some(&check_updates_switch));
    app_list.append(&check_updates_row);

    let reset_cfg_btn = gtk::Button::builder()
        .label("Clear all app data")
        .valign(gtk::Align::Center)
        .build();
    reset_cfg_btn.add_css_class("destructive-action");
    let reset_cfg_row = adw::ActionRow::builder()
        .title("Reset configuration")
        .subtitle("Permanently delete all assignments and settings")
        .build();
    reset_cfg_row.add_suffix(&reset_cfg_btn);

    general_page.append(&app_list);

    let reset_cfg_list = gtk::ListBox::new();
    reset_cfg_list.add_css_class("boxed-list");
    reset_cfg_list.set_selection_mode(gtk::SelectionMode::None);
    reset_cfg_list.append(&reset_cfg_row);

    let reset_desc = gtk::Label::new(Some("Maintenance and recovery options. Use with caution."));
    reset_desc.add_css_class("dim-label");
    reset_desc.set_xalign(0.0);
    reset_desc.set_margin_top(12);

    general_page.append(&reset_desc);
    general_page.append(&reset_cfg_list);

    {
        let config = config.clone();
        let window = window.clone();
        reset_cfg_btn.connect_clicked(move |_| {
            let dlg = gtk::MessageDialog::builder()
                .transient_for(&window)
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .text("Clear all app data?")
                .secondary_text("This will reset all your GPU assignments and settings to defaults. The application will close to apply changes.")
                .build();
            dlg.add_button("Cancel", gtk::ResponseType::Cancel);
            dlg.add_button("Reset", gtk::ResponseType::Accept);
            dlg.set_default_response(gtk::ResponseType::Accept);
            
            let config = config.clone();
            dlg.connect_response(move |d, res| {
                if res == gtk::ResponseType::Accept {
                    if let Err(e) = config.borrow_mut().reset() {
                        error!("failed to reset config: {}", e);
                    }
                    std::process::exit(0);
                }
                d.close();
            });
            dlg.present();
        });
    }

    // NVIDIA-specific settings (Advanced tab)
    let nvidia_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build();

    let nvidia_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    nvidia_page.set_margin_top(12);
    nvidia_page.set_margin_bottom(12);
    nvidia_page.set_margin_start(18);
    nvidia_page.set_margin_end(18);
    nvidia_scrolled.set_child(Some(&nvidia_page));

    let nvidia_main_stack = gtk::Stack::new();
    nvidia_main_stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    let warning_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
    warning_box.set_valign(gtk::Align::Center);
    warning_box.set_halign(gtk::Align::Center);
    warning_box.set_margin_start(40);
    warning_box.set_margin_end(40);

    let warning_icon = gtk::Image::builder()
        .icon_name("dialog-warning-symbolic")
        .pixel_size(64)
        .build();
    warning_icon.add_css_class("warning");
    
    let warning_title = gtk::Label::new(None);
    warning_title.set_markup("<span size='large' weight='bold'>Advanced NVIDIA Settings</span>");
    
    let optimus_explanation = gtk::Label::new(Some(
        "NVIDIA Optimus is a hybrid computing technology designed to save power and maximize performance on laptops. It combines an integrated GPU (low power) with a dedicated NVIDIA GPU (high performance).\n\nThe system automatically switches between them: the integrated GPU handles simple tasks, while the NVIDIA GPU is activated for games or heavy applications. This ensures longer battery life without sacrificing power."
    ));
    optimus_explanation.set_wrap(true);
    optimus_explanation.set_max_width_chars(60);
    optimus_explanation.set_justify(gtk::Justification::Center);
    optimus_explanation.add_css_class("dim-label");

    let agree_btn = gtk::Button::with_label("I understand the risks and wish to proceed");
    agree_btn.add_css_class("suggested-action");
    agree_btn.set_halign(gtk::Align::Center);

    let skip_warning_check = gtk::CheckButton::with_label("Don't show this again");
    skip_warning_check.set_halign(gtk::Align::Center);

    warning_box.append(&warning_icon);
    warning_box.append(&warning_title);
    warning_box.append(&optimus_explanation);
    warning_box.append(&agree_btn);
    warning_box.append(&skip_warning_check);

    nvidia_main_stack.add_named(&warning_box, Some("warning"));
    nvidia_main_stack.add_named(&nvidia_scrolled, Some("settings"));

    if skip_warning {
        nvidia_main_stack.set_visible_child_name("settings");
    }
    
    {
        let stack = nvidia_main_stack.clone();
        let config = config.clone();
        let skip_warning_check = skip_warning_check.clone();
        agree_btn.connect_clicked(move |_| {
            if skip_warning_check.is_active() {
                let mut cfg = config.borrow_mut();
                cfg.set_skip_nvidia_warning(true);
                let _ = cfg.save();
            }
            stack.set_visible_child_name("settings");
        });
    }

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

    let reset_btn = gtk::Button::with_label("Full reset");
    reset_btn.add_css_class("destructive-action");
    reset_btn.set_visible(false);
    let reset_sddm_btn = gtk::Button::with_label("Reset SDDM Xsetup");
    reset_sddm_btn.set_visible(false);

    if !has_nvidia {
        reset_btn.set_sensitive(false);
        reset_sddm_btn.set_sensitive(false);
    }

    let nvidia_help = gtk::Label::new(Some(
        "NVIDIA settings apply system-wide and require a reboot; use only if you understand how hybrid graphics work on your system.",
    ));
    nvidia_help.set_wrap(true);
    nvidia_help.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    nvidia_help.add_css_class("dim-label");
    nvidia_help.set_xalign(0.0);

    nvidia_page.append(&list);
    nvidia_page.append(&nvidia_help);

    let gen_page = stack.add_titled(&general_scrolled, Some("general"), "General");
    gen_page.set_icon_name(Some("view-list-symbolic"));
    let nvid_page = stack.add_titled(&nvidia_main_stack, Some("nvidia"), "NVIDIA & power");
    nvid_page.set_icon_name(Some("emblem-system-symbolic"));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_vexpand(true);

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();

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

    let apply_btn = gtk::Button::with_label("Applied");
    apply_btn.add_css_class("suggested-action");
    apply_btn.set_sensitive(false);
    apply_btn.set_size_request(90, 34);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    button_box.append(&reset_btn);
    button_box.append(&reset_sddm_btn);
    button_box.append(&spacer);
    button_box.append(&apply_btn);
    root.append(&button_box);

    // Dynamic visibility for NVIDIA reset buttons in the footer
    {
        let stack = stack.clone();
        let n_stack = nvidia_main_stack.clone();
        let r_btn = reset_btn.clone();
        let rs_btn = reset_sddm_btn.clone();
        let a_btn = apply_btn.clone();
        
        let update_visibility = move |st: &adw::ViewStack, nst: &gtk::Stack, b1: &gtk::Button, b2: &gtk::Button, b3: &gtk::Button| {
            let is_nvidia_tab = st.visible_child_name().as_deref() == Some("nvidia");
            let is_agreed = nst.visible_child_name().as_deref() == Some("settings");
            
            b1.set_visible(is_nvidia_tab && is_agreed);
            b2.set_visible(is_nvidia_tab && is_agreed);
            b3.set_visible(!is_nvidia_tab || is_agreed);
        };

        // Initial check
        update_visibility(&stack, &nvidia_main_stack, &reset_btn, &reset_sddm_btn, &apply_btn);

        // On tab change
        let n_stack_c = n_stack.clone();
        let r_btn_c = r_btn.clone();
        let rs_btn_c = rs_btn.clone();
        let a_btn_c = a_btn.clone();
        stack.connect_visible_child_notify(move |st| {
            update_visibility(st, &n_stack_c, &r_btn_c, &rs_btn_c, &a_btn_c);
        });

        // On agreement (warning dismissed)
        let stack_c = stack.clone();
        let r_btn_c2 = r_btn.clone();
        let rs_btn_c2 = rs_btn.clone();
        let a_btn_c2 = a_btn.clone();
        n_stack.connect_visible_child_name_notify(move |nst| {
            update_visibility(&stack_c, nst, &r_btn_c2, &rs_btn_c2, &a_btn_c2);
        });
    }

    // Reativa o Apply sempre que qualquer configuração for alterada
    macro_rules! on_change {
        ($widget:expr, $method:ident) => {{
            let btn = apply_btn.clone();
            $widget.$method(move |_| {
                btn.set_label("Apply");
                btn.set_sensitive(true);
            });
        }};
    }
    on_change!(show_steam_switch, connect_active_notify);
    on_change!(show_heroic_switch, connect_active_notify);
    on_change!(show_flatpak_switch, connect_active_notify);
    on_change!(use_env_switch, connect_active_notify);
    on_change!(check_updates_switch, connect_active_notify);
    on_change!(mode_dropdown, connect_selected_notify);
    on_change!(force_switch, connect_active_notify);
    on_change!(coolbits_switch, connect_active_notify);
    on_change!(rtd3_dropdown, connect_selected_notify);
    on_change!(nvidia_current_switch, connect_active_notify);
    on_change!(dm_dropdown, connect_selected_notify);
    {
        let btn = apply_btn.clone();
        coolbits_entry.connect_value_changed(move |_| {
            btn.set_label("Apply");
            btn.set_sensitive(true);
        });
    }

    {
        let window = window.clone();
        reset_btn.connect_clicked(move |_| {
            match reset_all() {
                Ok(()) => {
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&window)
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
                        .transient_for(&window)
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
        let window = window.clone();
        reset_sddm_btn.connect_clicked(move |_| {
            match reset_sddm() {
                Ok(()) => {
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&window)
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
                        .transient_for(&window)
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
        let window = window.clone();
        let config = config.clone();
        apply_btn.connect_clicked(move |btn| {
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

            {
                let mut cfg = config.borrow_mut();
                cfg.set_show_steam_apps(show_steam_switch.is_active());
                cfg.set_show_heroic_apps(show_heroic_switch.is_active());
                cfg.set_show_flatpak_apps(show_flatpak_switch.is_active());
                cfg.set_use_env_wrapper(use_env_switch.is_active());
                cfg.set_check_updates_at_startup(check_updates_switch.is_active());
                if let Err(err) = cfg.save() {
                    error!(%err, "failed to save app settings");
                }
            }

            btn.set_label("Applied");
            btn.set_sensitive(false);

            if !has_nvidia {
                return;
            }

            let nvidia_config = NvidiaSwitchConfig {
                mode,
                display_manager,
                enable_force_comp,
                coolbits_value,
                rtd3_value,
                use_nvidia_current,
            };

            if let Err(err) = switch_graphics_mode(&nvidia_config) {
                error!(%err, "failed to switch NVIDIA graphics mode");
                let dlg = gtk::MessageDialog::builder()
                    .transient_for(&window)
                    .modal(true)
                    .message_type(gtk::MessageType::Error)
                    .text("Failed to switch NVIDIA graphics mode")
                    .secondary_text(&err)
                    .build();
                dlg.add_button("Close", gtk::ResponseType::Close);
                dlg.connect_response(|d, _| d.close());
                dlg.present();
            }
        });
    }

    (root, switcher)
}

