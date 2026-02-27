use adw::prelude::*;
use std::path::Path;

use crate::models::{DesktopApp, GpuChoice, GpuInfo};

#[derive(Clone)]
pub(crate) struct AppDetailsWidgets {
    pub(crate) icon: gtk::Image,
    pub(crate) name: gtk::Label,
    pub(crate) assignment_row: adw::ActionRow,
    pub(crate) source_row: adw::ActionRow,
    pub(crate) desktop_id_row: adw::ActionRow,
    pub(crate) path_row: adw::ActionRow,
    pub(crate) exec_row: adw::ActionRow,
    pub(crate) desktop_path_label: gtk::Label,
    pub(crate) desktop_open_button: gtk::Button,
    pub(crate) desktop_preview: gtk::TextView,
}

fn user_override_path(desktop_id: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join(".local/share/applications")
            .join(desktop_id),
    )
}

pub(crate) fn set_app_details(
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

pub(crate) fn set_app_details_empty(details: &AppDetailsWidgets, gpus: &[GpuInfo]) {
    details.icon.set_icon_name(Some("application-x-executable"));
    details.name.set_text("Select an application");
    details
        .assignment_row
        .set_subtitle(&gpu_choice_label(gpus, &GpuChoice::Default));
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

pub(crate) fn build_app_icon(icon: Option<&str>, pixel_size: i32) -> gtk::Image {
    let image = gtk::Image::new();
    apply_icon_to_image(&image, icon, pixel_size);
    image
}

pub(crate) fn apply_icon_to_image(image: &gtk::Image, icon: Option<&str>, pixel_size: i32) {
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

pub(crate) fn build_gpu_choices(gpus: &[GpuInfo]) -> Vec<(String, GpuChoice)> {
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

pub(crate) fn gpu_choice_label(gpus: &[GpuInfo], choice: &GpuChoice) -> String {
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

pub(crate) fn selected_gpu_for_choice(gpus: &[GpuInfo], choice: &GpuChoice) -> Option<GpuInfo> {
    let GpuChoice::Gpu(idx) = choice else {
        return None;
    };

    gpus.iter()
        .find(|g| g.dri_prime_index == Some(*idx))
        .cloned()
}

