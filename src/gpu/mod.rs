use crate::models::GpuInfo;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn detect_gpus() -> Vec<GpuInfo> {
    let lspci_map = read_lspci_gpu_names();
    let render_map = read_render_nodes_from_sysfs();

    let mut cards = Vec::new();
    let drm_dir = Path::new("/sys/class/drm");

    if let Ok(read_dir) = fs::read_dir(drm_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("card") || file_name.contains('-') {
                continue;
            }

            let device_path = path.join("device");
            let driver = read_driver_name(&device_path);
            let pci_slot = read_file_trimmed(device_path.join("uevent")).and_then(|u| {
                u.lines().find_map(|line| {
                    line.strip_prefix("PCI_SLOT_NAME=")
                        .map(std::string::ToString::to_string)
                })
            });

            let render_node = render_map.get(&file_name).cloned();
            let card_name = pci_slot
                .as_ref()
                .and_then(|slot| lspci_map.get(slot).cloned())
                .unwrap_or_else(|| file_name.clone());

            cards.push(GpuInfo {
                card: file_name,
                name: card_name,
                driver,
                pci_slot,
                render_node,
                dri_prime_index: None,
                renderer: None,
            });
        }
    }

    cards.sort_by_key(|g| card_number(&g.card));

    let fallback_nodes = read_render_nodes_from_dev();
    for (idx, gpu) in cards.iter_mut().enumerate() {
        if gpu.render_node.is_none() {
            gpu.render_node = fallback_nodes.get(idx).cloned();
        }
    }

    for (idx, gpu) in cards.iter_mut().enumerate() {
        gpu.dri_prime_index = Some(idx);
        gpu.renderer = detect_renderer(gpu.dri_prime_index);
    }

    cards
}

fn read_lspci_gpu_names() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(output) = Command::new("lspci").arg("-nn").output() else {
        return map;
    };

    if !output.status.success() {
        return map;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let is_gpu_line = line.contains("VGA compatible controller")
            || line.contains("3D controller")
            || line.contains("Display controller");

        if !is_gpu_line {
            continue;
        }

        let mut parts = line.splitn(2, ' ');
        let slot = parts.next().unwrap_or_default().to_string();
        let name = parts
            .next()
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| line.to_string());

        if !slot.is_empty() {
            map.insert(slot, name);
        }
    }

    map
}

fn read_render_nodes_from_sysfs() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();

    let Ok(read_dir) = fs::read_dir("/sys/class/drm") else {
        return map;
    };

    for entry in read_dir.flatten() {
        let card = entry.file_name().to_string_lossy().to_string();
        if !card.starts_with("card") || card.contains('-') {
            continue;
        }

        let render_dir = entry.path().join("device/drm");
        let Ok(render_entries) = fs::read_dir(render_dir) else {
            continue;
        };

        for render in render_entries.flatten() {
            let node = render.file_name().to_string_lossy().to_string();
            if node.starts_with("renderD") {
                map.insert(card.clone(), format!("/dev/dri/{node}"));
                break;
            }
        }
    }

    map
}

fn read_render_nodes_from_dev() -> Vec<String> {
    let mut nodes = Vec::new();
    let Ok(entries) = fs::read_dir("/dev/dri") else {
        return nodes;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("renderD") {
            nodes.push(format!("/dev/dri/{name}"));
        }
    }

    nodes.sort();
    nodes
}

fn read_driver_name(device_path: &Path) -> Option<String> {
    let link = fs::read_link(device_path.join("driver")).ok()?;
    link.file_name().map(|v| v.to_string_lossy().to_string())
}

fn read_file_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn card_number(card: &str) -> usize {
    card.trim_start_matches("card")
        .parse::<usize>()
        .unwrap_or(usize::MAX)
}

fn detect_renderer(dri_prime: Option<usize>) -> Option<String> {
    let mut cmd = Command::new("glxinfo");
    cmd.arg("-B");
    if let Some(idx) = dri_prime {
        cmd.env("DRI_PRIME", idx.to_string());
    }

    let Ok(output) = cmd.output() else {
        return detect_renderer_vulkan(dri_prime);
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(renderer) = line.strip_prefix("OpenGL renderer string:") {
                return Some(renderer.trim().to_string());
            }
        }
    }

    detect_renderer_vulkan(dri_prime)
}

fn detect_renderer_vulkan(dri_prime: Option<usize>) -> Option<String> {
    let mut cmd = Command::new("vulkaninfo");
    cmd.arg("--summary");
    if let Some(idx) = dri_prime {
        cmd.env("DRI_PRIME", idx.to_string());
    }

    let Ok(output) = cmd.output() else {
        return None;
    };

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("GPU") && trimmed.contains(':') {
            return Some(trimmed.to_string());
        }
    }

    None
}
