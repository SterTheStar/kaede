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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub assignments: BTreeMap<String, GpuChoice>,
}
