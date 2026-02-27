use crate::models::{AppConfig, GpuChoice};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
    data: AppConfig,
}

impl ConfigStore {
    pub fn load() -> Self {
        let path = config_path();
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| toml::from_str::<AppConfig>(&raw).ok())
            .unwrap_or_default();

        Self { path, data }
    }

    pub fn get_choice(&self, desktop_id: &str) -> GpuChoice {
        self.data
            .assignments
            .get(desktop_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_choice(&mut self, desktop_id: &str, choice: GpuChoice) {
        self.data.assignments.insert(desktop_id.to_string(), choice);
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory at {}", parent.display())
            })?;
        }

        let body = toml::to_string_pretty(&self.data).context("failed to serialize config")?;
        fs::write(&self.path, body)
            .with_context(|| format!("failed to write config at {}", self.path.display()))?;
        Ok(())
    }

    pub fn show_steam_apps(&self) -> bool {
        self.data.show_steam_apps
    }

    pub fn set_show_steam_apps(&mut self, value: bool) {
        self.data.show_steam_apps = value;
    }

    pub fn show_heroic_apps(&self) -> bool {
        self.data.show_heroic_apps
    }

    pub fn set_show_heroic_apps(&mut self, value: bool) {
        self.data.show_heroic_apps = value;
    }

    pub fn show_flatpak_apps(&self) -> bool {
        self.data.show_flatpak_apps
    }

    pub fn set_show_flatpak_apps(&mut self, value: bool) {
        self.data.show_flatpak_apps = value;
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()))
                .join(".config")
        });

    base.join("kaede").join("config.toml")
}
