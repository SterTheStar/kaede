use crate::heroic::apply_heroic_launch_env;
use crate::models::{DesktopApp, GpuChoice, GpuInfo};
use crate::steam::apply_steam_launch_options;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

const KAEDE_MARKER: &str = "X-Kaede-Managed=true";

pub fn apply_launcher_override(
    app: &DesktopApp,
    choice: &GpuChoice,
    selected_gpu: Option<&GpuInfo>,
) -> Result<()> {
    if app.is_steam_game {
        if let Some(app_id) = app.steam_app_id.as_deref() {
            // Steam games should be configured through Steam LaunchOptions.
            let _ = remove_kaede_override_if_present(&user_launcher_path(&app.desktop_id));
            let steam_env = steam_env_vars(choice, selected_gpu);
            info!(
                app_id = app_id,
                gpu_choice = %choice.label(),
                env = ?steam_env,
                "applying Steam LaunchOptions override"
            );
            return apply_steam_launch_options(app_id, choice, &steam_env);
        }
        warn!(
            desktop_id = %app.desktop_id,
            "steam game detected without steam app id, falling back to desktop override"
        );
    }

    if app.is_heroic_game {
        if let (Some(platform), Some(app_name)) = (
            app.heroic_platform.as_deref(),
            app.heroic_app_name.as_deref(),
        ) {
            let heroic_env = match choice {
                GpuChoice::Default => Vec::new(),
                GpuChoice::Gpu(index) => build_env_pairs(*index, false, selected_gpu),
            };
            info!(
                platform = platform,
                app_name = app_name,
                gpu_choice = %choice.label(),
                env = ?heroic_env,
                "applying Heroic game env override"
            );
            return apply_heroic_launch_env(platform, app_name, &heroic_env);
        }
        warn!(
            desktop_id = %app.desktop_id,
            "heroic game detected without platform/app key, falling back to desktop override"
        );
    }

    if app.is_flatpak {
        if let Some(app_id) = app.flatpak_app_id.as_deref() {
            let profile = gpu_profile(selected_gpu);
            info!(
                app_id = app_id,
                gpu_choice = %choice.label(),
                nvidia = profile.is_nvidia,
                mesa = profile.is_mesa,
                "applying Flatpak override"
            );
            return apply_flatpak_override(app_id, choice, selected_gpu);
        }
        warn!(
            desktop_id = %app.desktop_id,
            "flatpak app detected without app id, falling back to desktop override"
        );
    }

    let target = user_launcher_path(&app.desktop_id);

    match choice {
        GpuChoice::Default => remove_kaede_override_if_present(&target),
        GpuChoice::Gpu(index) => write_override(app, *index, selected_gpu, &target),
    }
}

fn apply_flatpak_override(
    app_id: &str,
    choice: &GpuChoice,
    selected_gpu: Option<&GpuInfo>,
) -> Result<()> {
    let mut cmd = Command::new("flatpak");
    cmd.args(["override", "--user"]);

    match choice {
        GpuChoice::Default => {
            cmd.args([
                "--unset-env=DRI_PRIME",
                "--unset-env=PRESSURE_VESSEL_IMPORT_VARS",
                "--unset-env=__NV_PRIME_RENDER_OFFLOAD",
                "--unset-env=__GLX_VENDOR_LIBRARY_NAME",
                "--unset-env=__VK_LAYER_NV_optimus",
                "--unset-env=MESA_VK_DEVICE_SELECT",
                "--unset-env=MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE",
                app_id,
            ]);
        }
        GpuChoice::Gpu(index) => {
            for env in build_env_pairs(*index, false, selected_gpu) {
                cmd.arg(format!("--env={env}"));
            }
            cmd.arg(app_id);
        }
    }

    let status = cmd
        .status()
        .with_context(|| format!("failed to execute flatpak override for {app_id}"))?;

    if !status.success() {
        anyhow::bail!("flatpak override command failed for {app_id}");
    }
    debug!(app_id = app_id, "flatpak override command succeeded");

    Ok(())
}

fn write_override(
    app: &DesktopApp,
    index: usize,
    selected_gpu: Option<&GpuInfo>,
    target: &Path,
) -> Result<()> {
    if app.path == target && !file_contains_marker(target) {
        anyhow::bail!(
            "refusing to overwrite unmanaged local desktop file: {}",
            target.display()
        );
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let source_content = fs::read_to_string(&app.path).unwrap_or_default();
    let original_exec = desktop_exec_value(&source_content)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| app.exec.clone());
    let wrapped_exec = wrap_exec_for_gpu(&original_exec, index, selected_gpu);
    let content = rewrite_desktop_override_content(&source_content, &wrapped_exec, app);

    fs::write(target, content)
        .with_context(|| format!("failed to write launcher {}", target.display()))?;
    debug!(target = %target.display(), "desktop override launcher written");
    Ok(())
}

fn wrap_exec_for_gpu(exec: &str, index: usize, selected_gpu: Option<&GpuInfo>) -> String {
    let is_steam = is_steam_exec(exec);
    let env_pairs = build_env_pairs(index, is_steam, selected_gpu);

    if looks_like_flatpak_run(exec) {
        return wrap_flatpak_run_with_env(exec, &env_pairs);
    }

    format!("env {} {}", env_pairs.join(" "), exec)
}

fn build_env_pairs(index: usize, is_steam: bool, selected_gpu: Option<&GpuInfo>) -> Vec<String> {
    let profile = gpu_profile(selected_gpu);
    let mut env_pairs = vec![format!("DRI_PRIME={index}")];

    if profile.is_nvidia {
        env_pairs.push("__NV_PRIME_RENDER_OFFLOAD=1".to_string());
        env_pairs.push("__GLX_VENDOR_LIBRARY_NAME=nvidia".to_string());
        env_pairs.push("__VK_LAYER_NV_optimus=NVIDIA_only".to_string());
    }

    if profile.is_mesa {
        if let Some(sel) = profile.mesa_vk_device_select {
            env_pairs.push(format!("MESA_VK_DEVICE_SELECT={sel}"));
        }
        if index == 0 {
            env_pairs.push("MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1".to_string());
        }
    }

    if is_steam {
        let mut imported = vec!["DRI_PRIME".to_string()];
        if profile.is_nvidia {
            imported.push("__NV_PRIME_RENDER_OFFLOAD".to_string());
            imported.push("__GLX_VENDOR_LIBRARY_NAME".to_string());
            imported.push("__VK_LAYER_NV_optimus".to_string());
        }
        if profile.is_mesa {
            imported.push("MESA_VK_DEVICE_SELECT".to_string());
            if index == 0 {
                imported.push("MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE".to_string());
            }
        }
        env_pairs.push(format!(
            "PRESSURE_VESSEL_IMPORT_VARS={}",
            imported.join(",")
        ));
    }

    env_pairs
}

fn steam_env_vars(choice: &GpuChoice, selected_gpu: Option<&GpuInfo>) -> Vec<String> {
    match choice {
        GpuChoice::Default => Vec::new(),
        GpuChoice::Gpu(index) => build_env_pairs(*index, true, selected_gpu),
    }
}

#[derive(Debug, Clone)]
struct GpuProfile {
    is_nvidia: bool,
    is_mesa: bool,
    mesa_vk_device_select: Option<String>,
}

fn gpu_profile(selected_gpu: Option<&GpuInfo>) -> GpuProfile {
    let Some(gpu) = selected_gpu else {
        return GpuProfile {
            is_nvidia: false,
            is_mesa: false,
            mesa_vk_device_select: None,
        };
    };

    let mut hay = gpu.name.to_ascii_lowercase();
    if let Some(driver) = &gpu.driver {
        hay.push(' ');
        hay.push_str(&driver.to_ascii_lowercase());
    }
    if let Some(renderer) = &gpu.renderer {
        hay.push(' ');
        hay.push_str(&renderer.to_ascii_lowercase());
    }

    let driver = gpu
        .driver
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let is_nvidia = driver == "nvidia" || hay.contains("nvidia");
    let is_mesa = !is_nvidia
        && (hay.contains("mesa")
            || driver.contains("amdgpu")
            || driver.contains("radeon")
            || driver.contains("i915")
            || driver.contains("iris")
            || driver.contains("nouveau"));

    GpuProfile {
        is_nvidia,
        is_mesa,
        mesa_vk_device_select: mesa_vk_device_select_from_pci(gpu.pci_slot.as_deref()),
    }
}

fn mesa_vk_device_select_from_pci(pci: Option<&str>) -> Option<String> {
    let slot = pci?.trim();
    if slot.is_empty() {
        return None;
    }

    let mut normalized = slot.to_string();
    if normalized.matches(':').count() == 1 {
        normalized = format!("0000:{normalized}");
    }

    let normalized = normalized.replace(':', "_").replace('.', "_");
    Some(format!("pci-{normalized}"))
}

fn looks_like_flatpak_run(exec: &str) -> bool {
    let parts = exec.split_whitespace().collect::<Vec<_>>();
    parts.windows(2).any(|w| {
        let a = w[0];
        let b = w[1];
        (a == "flatpak" || a.ends_with("/flatpak")) && b == "run"
    })
}

fn wrap_flatpak_run_with_env(exec: &str, env_pairs: &[String]) -> String {
    let mut parts = exec
        .split_whitespace()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();

    if let Some(i) = parts.windows(2).position(|w| {
        let a = w[0].as_str();
        let b = w[1].as_str();
        (a == "flatpak" || a.ends_with("/flatpak")) && b == "run"
    }) {
        let insert_at = i + 2;
        let mut env_args = env_pairs
            .iter()
            .map(|kv| format!("--env={kv}"))
            .collect::<Vec<_>>();
        for (offset, arg) in env_args.drain(..).enumerate() {
            parts.insert(insert_at + offset, arg);
        }
        return parts.join(" ");
    }

    format!("env {} {}", env_pairs.join(" "), exec)
}

fn is_steam_exec(exec: &str) -> bool {
    let lower = exec.to_ascii_lowercase();
    (lower.contains("steam") && lower.contains("rungameid"))
        || (lower.contains("steam") && lower.contains("-applaunch"))
        || (lower.contains("steam") && lower.contains("steam://run"))
}

fn desktop_exec_value(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        line.strip_prefix("Exec=")
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(std::string::ToString::to_string)
    })
}

fn rewrite_desktop_override_content(source: &str, wrapped_exec: &str, app: &DesktopApp) -> String {
    if source.trim().is_empty() {
        let icon = app.icon.as_deref().unwrap_or("application-x-executable");
        return format!(
            "[Desktop Entry]\nType=Application\nName={}\nIcon={}\nExec={}\nTerminal=false\n{}\n",
            app.name, icon, wrapped_exec, KAEDE_MARKER
        );
    }

    let mut lines = Vec::new();
    let mut replaced_exec = false;
    let mut has_marker = false;
    let mut in_desktop_entry = false;
    let mut inserted_exec_in_section = false;

    for line in source.lines() {
        if line.trim_start().starts_with('[') {
            if in_desktop_entry && !replaced_exec && !inserted_exec_in_section {
                lines.push(format!("Exec={wrapped_exec}"));
                replaced_exec = true;
                inserted_exec_in_section = true;
            }
            in_desktop_entry = line.trim() == "[Desktop Entry]";
            lines.push(line.to_string());
            continue;
        }

        if line.starts_with("X-Kaede-Managed=") {
            has_marker = true;
            lines.push(KAEDE_MARKER.to_string());
            continue;
        }

        if in_desktop_entry && line.starts_with("Exec=") && !replaced_exec {
            lines.push(format!("Exec={wrapped_exec}"));
            replaced_exec = true;
            inserted_exec_in_section = true;
            continue;
        }

        lines.push(line.to_string());
    }

    if !replaced_exec {
        let mut insert_at = 0usize;
        for (idx, line) in lines.iter().enumerate() {
            if line.trim() == "[Desktop Entry]" {
                insert_at = idx + 1;
                break;
            }
        }
        lines.insert(insert_at, format!("Exec={wrapped_exec}"));
    }

    if !has_marker {
        lines.push(KAEDE_MARKER.to_string());
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn remove_kaede_override_if_present(path: &Path) -> Result<()> {
    if path.exists() && file_contains_marker(path) {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove launcher {}", path.display()))?;
    }
    Ok(())
}

fn file_contains_marker(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|body| body.contains(KAEDE_MARKER))
        .unwrap_or(false)
}

fn user_launcher_path(desktop_id: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".local/share/applications")
        .join(desktop_id)
}
