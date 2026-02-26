use crate::models::DesktopApp;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn scan_desktop_entries() -> Vec<DesktopApp> {
    let mut map: BTreeMap<String, DesktopApp> = BTreeMap::new();

    for dir in application_dirs() {
        if !dir.exists() {
            continue;
        }

        if let Ok(read_dir) = fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }

                if let Some(app) = parse_desktop_file(&path) {
                    // Later directories override earlier ones (user local last).
                    map.insert(app.desktop_id.clone(), app);
                }
            }
        }
    }

    let mut apps: Vec<DesktopApp> = map.into_values().collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from(home.clone()).join(".local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from(home).join(".local/share/flatpak/exports/share/applications"),
    ]
}

fn parse_desktop_file(path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut name: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut no_display = false;
    let mut hidden = false;
    let mut typ = String::new();
    let mut flatpak_app_id: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().to_string();

        match key {
            "Name" => name = Some(value),
            "Icon" => icon = Some(value),
            "Exec" => exec = Some(strip_desktop_exec_placeholders(&value)),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "Type" => typ = value,
            "X-Flatpak" => flatpak_app_id = Some(value),
            _ => {}
        }
    }

    if no_display || hidden || typ != "Application" {
        return None;
    }

    let desktop_id = path.file_name()?.to_string_lossy().to_string();
    let id_from_filename = desktop_id.strip_suffix(".desktop").map(|s| s.to_string());
    let id_from_exec = flatpak_app_id_from_exec(exec.as_deref().unwrap_or_default());
    let flatpak_id = flatpak_app_id.or(id_from_exec).or(id_from_filename);
    let is_flatpak = is_flatpak_entry(path, exec.as_deref().unwrap_or_default());
    let steam_app_id = steam_app_id_from_exec(exec.as_deref().unwrap_or_default());
    let (heroic_platform, heroic_app_name) =
        heroic_game_from_exec(exec.as_deref().unwrap_or_default()).unwrap_or_else(|| (None, None));
    let is_heroic_game = heroic_platform.is_some() && heroic_app_name.is_some();

    Some(DesktopApp {
        desktop_id,
        path: path.to_path_buf(),
        name: name.unwrap_or_else(|| "Unnamed Application".to_string()),
        icon,
        exec: exec.unwrap_or_default(),
        is_steam_game: steam_app_id.is_some(),
        steam_app_id,
        is_heroic_game,
        heroic_platform,
        heroic_app_name,
        is_flatpak,
        flatpak_app_id: if is_flatpak { flatpak_id } else { None },
    })
}

fn strip_desktop_exec_placeholders(exec: &str) -> String {
    ["%f", "%F", "%u", "%U", "%i", "%c", "%k"]
        .iter()
        .fold(exec.to_string(), |acc, token| acc.replace(token, ""))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_flatpak_entry(path: &Path, exec: &str) -> bool {
    let path_str = path.to_string_lossy();
    path_str.contains("/flatpak/exports/share/applications")
        || exec.contains("flatpak run")
        || exec.contains("/flatpak")
}

fn flatpak_app_id_from_exec(exec: &str) -> Option<String> {
    if !exec.contains("flatpak") || !exec.contains("run") {
        return None;
    }

    let parts = exec.split_whitespace().collect::<Vec<_>>();
    let run_pos = parts.iter().position(|p| *p == "run")?;

    for token in parts.iter().skip(run_pos + 1) {
        if token.starts_with('-') {
            continue;
        }
        if looks_like_flatpak_app_id(token) {
            return Some((*token).to_string());
        }
    }

    None
}

fn looks_like_flatpak_app_id(value: &str) -> bool {
    value.contains('.')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

fn steam_app_id_from_exec(exec: &str) -> Option<String> {
    if let Some(idx) = exec.find("steam://rungameid/") {
        let tail = &exec[idx + "steam://rungameid/".len()..];
        let id = tail
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        if !id.is_empty() {
            return Some(id);
        }
    }

    let parts = exec.split_whitespace().collect::<Vec<_>>();
    if let Some(i) = parts.iter().position(|p| *p == "-applaunch") {
        let id = parts.get(i + 1).copied().unwrap_or_default();
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
            return Some(id.to_string());
        }
    }

    None
}

fn heroic_game_from_exec(exec: &str) -> Option<(Option<String>, Option<String>)> {
    let marker = "heroic://launch/";
    if let Some(idx) = exec.find(marker) {
        let tail = &exec[idx + marker.len()..];

        let mut parts = tail.split(['/', '?', ' ', '"']);
        let platform = parts.next()?.trim();
        let app = parts.next()?.trim();
        if !platform.is_empty() && !app.is_empty() {
            return Some((Some(platform.to_string()), Some(app.to_string())));
        }
    }

    // Heroic also uses query format:
    // heroic://launch?appName=<id>&runner=<platform>
    let query_marker = "heroic://launch?";
    let idx = exec.find(query_marker)?;
    let query = &exec[idx + query_marker.len()..];

    let mut app_name: Option<String> = None;
    let mut runner: Option<String> = None;

    for pair in query.split('&') {
        let pair = pair
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches('"');
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        match k {
            "appName" if !v.is_empty() => app_name = Some(v.to_string()),
            "runner" if !v.is_empty() => runner = Some(v.to_string()),
            _ => {}
        }
    }

    let app_name = app_name?;
    Some((runner, Some(app_name)))
}
