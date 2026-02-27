use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub card: String,
    pub name: String,
    pub driver: Option<String>,
    pub pci_slot: Option<String>,
    pub render_node: Option<String>,
    pub dri_prime_index: Option<usize>,
    pub renderer: Option<String>,
}

impl GpuInfo {
    pub fn name_for_filter(&self) -> String {
        let source = self
            .renderer
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(&self.name);

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

        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub desktop_id: String,
    pub path: PathBuf,
    pub name: String,
    pub icon: Option<String>,
    pub exec: String,
    pub is_steam_game: bool,
    pub steam_app_id: Option<String>,
    pub is_heroic_game: bool,
    pub heroic_platform: Option<String>,
    pub heroic_app_name: Option<String>,
    pub is_flatpak: bool,
    pub flatpak_app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum GpuChoice {
    Default,
    Gpu(usize),
}

impl Default for GpuChoice {
    fn default() -> Self {
        Self::Default
    }
}

impl GpuChoice {
    pub fn label(&self) -> String {
        match self {
            GpuChoice::Default => "Default GPU".to_string(),
            GpuChoice::Gpu(idx) => format!("GPU {}", idx),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub assignments: BTreeMap<String, GpuChoice>,
    #[serde(default = "default_true")]
    pub show_steam_apps: bool,
    #[serde(default = "default_true")]
    pub show_heroic_apps: bool,
    #[serde(default = "default_true")]
    pub show_flatpak_apps: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            assignments: BTreeMap::new(),
            show_steam_apps: true,
            show_heroic_apps: true,
            show_flatpak_apps: true,
        }
    }
}
