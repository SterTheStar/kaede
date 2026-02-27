use crate::models::GpuChoice;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};

const KAEDE_STEAM_START: &str = "KAEDE_GPU_MANAGED=1";
const KAEDE_STEAM_END: &str = "KAEDE_GPU_MANAGED_END=1";

pub fn apply_steam_launch_options(
    app_id: &str,
    choice: &GpuChoice,
    managed_env: &[String],
    use_env_wrapper: bool,
) -> Result<()> {
    if is_steam_running() {
        warn!("Steam appears to be running; it may overwrite localconfig.vdf changes on exit");
    }

    let files = find_localconfig_files();
    debug!(count = files.len(), "found Steam localconfig candidates");
    if files.is_empty() {
        warn!("no Steam localconfig.vdf files found");
        anyhow::bail!("no Steam localconfig.vdf files found");
    }

    let mut matched_any = false;
    let mut changed_any = false;
    let mut validated_any = false;

    for path in files {
        debug!(path = %path.display(), app_id = app_id, "processing Steam config");
        let original = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let before = app_state_in_localconfig(&original, app_id);
        if before.app_found {
            matched_any = true;
        }

        let (updated, changed) = update_localconfig_content(&original, app_id, choice, managed_env, use_env_wrapper);
        let current_content = if changed {
            write_backup_if_missing(&path, &original)?;
            fs::write(&path, &updated)
                .with_context(|| format!("failed to write {}", path.display()))?;
            info!(path = %path.display(), app_id = app_id, "Steam LaunchOptions updated");
            changed_any = true;
            fs::read_to_string(&path)
                .with_context(|| format!("failed to read back {}", path.display()))?
        } else {
            debug!(
                path = %path.display(),
                app_id = app_id,
                "no Steam content changes needed for this config"
            );
            original.clone()
        };

        let after = app_state_in_localconfig(&current_content, app_id);
        if after.app_found {
            matched_any = true;
            if validate_expected_state(after.launch_options.as_deref(), choice) {
                validated_any = true;
                if changed {
                    info!(
                        path = %path.display(),
                        app_id = app_id,
                        "Steam LaunchOptions validation succeeded after write"
                    );
                } else {
                    info!(
                        path = %path.display(),
                        app_id = app_id,
                        "Steam LaunchOptions already in desired state"
                    );
                }
            } else {
                warn!(
                    path = %path.display(),
                    app_id = app_id,
                    launch_options = ?after.launch_options,
                    "Steam LaunchOptions present but validation failed"
                );
            }
        }
    }

    if !matched_any {
        warn!(app_id = app_id, "Steam App ID not found in localconfig.vdf");
        anyhow::bail!("Steam App ID {} not found in localconfig.vdf", app_id);
    }
    if !validated_any {
        anyhow::bail!(
            "Steam App ID {} was found but LaunchOptions validation failed",
            app_id
        );
    }
    if !changed_any {
        debug!(
            app_id = app_id,
            "Steam LaunchOptions required no file modifications"
        );
    }

    Ok(())
}

fn find_localconfig_files() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let bases = [
        PathBuf::from(&home).join(".steam/steam"),
        PathBuf::from(&home).join(".local/share/Steam"),
        PathBuf::from(&home).join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];

    let mut out = Vec::new();
    for base in bases {
        let userdata = base.join("userdata");
        let Ok(entries) = fs::read_dir(userdata) else {
            continue;
        };

        for entry in entries.flatten() {
            let userdir = entry.path();
            if !userdir.is_dir() {
                continue;
            }
            let cfg = userdir.join("config/localconfig.vdf");
            if cfg.exists() {
                out.push(cfg);
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

fn write_backup_if_missing(path: &Path, content: &str) -> Result<()> {
    let backup = path.with_file_name("localconfig.vdf.kaede.bak");
    if !backup.exists() {
        fs::write(&backup, content)
            .with_context(|| format!("failed to write backup {}", backup.display()))?;
        info!(backup = %backup.display(), "Steam config backup created");
    }
    Ok(())
}

fn update_localconfig_content(
    content: &str,
    app_id: &str,
    choice: &GpuChoice,
    managed_env: &[String],
    use_env_wrapper: bool,
) -> (String, bool) {
    let apps_block = find_steam_apps_block(content).or_else(|| {
        warn!("Steam apps block not found at canonical path; trying fallback global apps search");
        find_block_by_key_in_range_ci(content, "apps", 0, content.len())
    });

    let Some((apps_key, apps_open, apps_close)) = apps_block else {
        warn!("Steam localconfig missing apps block");
        return (content.to_string(), false);
    };

    let desired_prefix = build_managed_prefix(choice, managed_env, use_env_wrapper);
    let (mut out, changed) = upsert_app_launch_options(
        content,
        apps_key,
        apps_open,
        apps_close,
        app_id,
        desired_prefix.as_deref(),
    );

    if !changed {
        return (content.to_string(), false);
    }

    if content.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }

    (out, true)
}

fn find_steam_apps_block(content: &str) -> Option<(usize, usize, usize)> {
    let (_, ulcs_open, ulcs_close) =
        find_block_by_key_in_range_ci(content, "UserLocalConfigStore", 0, content.len())?;
    let (_, software_open, software_close) =
        find_block_by_key_in_range_ci(content, "Software", ulcs_open + 1, ulcs_close)?;
    let (_, valve_open, valve_close) =
        find_block_by_key_in_range_ci(content, "Valve", software_open + 1, software_close)?;
    let (_, steam_open, steam_close) =
        find_block_by_key_in_range_ci(content, "Steam", valve_open + 1, valve_close)?;
    find_block_by_key_in_range_ci(content, "apps", steam_open + 1, steam_close)
}

pub fn is_steam_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "steam"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn upsert_app_launch_options(
    content: &str,
    apps_key: usize,
    apps_open: usize,
    apps_close: usize,
    app_id: &str,
    desired_prefix: Option<&str>,
) -> (String, bool) {
    let Some((app_key, app_open, app_close)) =
        find_block_by_key_in_range(content, app_id, apps_open + 1, apps_close)
    else {
        if desired_prefix.is_none() {
            return (content.to_string(), false);
        }

        let apps_indent = indentation_at(content, apps_key);
        let app_indent = format!("{}\t", apps_indent);
        let launch_indent = format!("{}\t", app_indent);
        let launch = format!(
            "{}\"LaunchOptions\"\t\t\"{}\"",
            launch_indent,
            apply_prefix_to_existing(None, desired_prefix)
        );

        let block = format!(
            "\n{}\"{}\"\n{}{{\n{}\n{}}}",
            app_indent, app_id, app_indent, launch, app_indent
        );

        let mut out = content.to_string();
        out.insert_str(apps_close, &block);
        return (out, true);
    };

    let app_indent = indentation_at(content, app_key);
    let launch_indent_default = format!("{}\t", app_indent);

    let (line_start, line_end, existing_value, line_indent) = find_launch_options_line(
        content,
        app_open + 1,
        app_close,
    )
    .unwrap_or((app_close, app_close, None, launch_indent_default.clone()));

    let updated_value = apply_prefix_to_existing(existing_value.as_deref(), desired_prefix);

    if line_start < line_end {
        if updated_value.is_empty() {
            let mut out = content.to_string();
            out.replace_range(line_start..line_end, "");
            return (out, true);
        }

        if existing_value.as_deref() == Some(updated_value.as_str()) {
            return (content.to_string(), false);
        }

        let new_line = format!(
            "{}\"LaunchOptions\"\t\t\"{}\"\n",
            line_indent, updated_value
        );
        let mut out = content.to_string();
        out.replace_range(line_start..line_end, &new_line);
        return (out, true);
    }

    if updated_value.is_empty() {
        return (content.to_string(), false);
    }

    let insertion = format!(
        "\n{}\"LaunchOptions\"\t\t\"{}\"",
        launch_indent_default, updated_value
    );
    let mut out = content.to_string();
    out.insert_str(app_close, &insertion);
    (out, true)
}

fn apply_prefix_to_existing(existing: Option<&str>, desired_prefix: Option<&str>) -> String {
    let existing = existing.unwrap_or_default().trim();
    let tail = strip_managed_prefix(existing).trim().to_string();

    match desired_prefix {
        Some(prefix) => {
            let tail = if tail.is_empty() {
                "%command%".to_string()
            } else {
                tail
            };
            if tail != "%command%" {
                debug!(tail = %tail, "preserving existing custom Steam LaunchOptions tail");
            }
            format!("{} {}", prefix.trim(), tail).trim().to_string()
        }
        None => tail,
    }
}

fn build_managed_prefix(choice: &GpuChoice, managed_env: &[String], use_env_wrapper: bool) -> Option<String> {
    let GpuChoice::Gpu(idx) = choice else {
        return None;
    };
    let vars = if managed_env.is_empty() {
        vec![format!("DRI_PRIME={idx}")]
    } else {
        managed_env.to_vec()
    };

    let prefix = if use_env_wrapper { "env " } else { "" };
    Some(format!(
        "{}{} {} {}",
        prefix,
        KAEDE_STEAM_START,
        vars.join(" "),
        KAEDE_STEAM_END
    ))
}

fn strip_managed_prefix(value: &str) -> String {
    if let Some(start) = value.find(KAEDE_STEAM_START) {
        if let Some(end_rel) = value[start..].find(KAEDE_STEAM_END) {
            let end = start + end_rel + KAEDE_STEAM_END.len();
            let mut out = String::new();
            out.push_str(value[..start].trim_end());
            if !out.is_empty() && !value[end..].trim().is_empty() {
                out.push(' ');
            }
            out.push_str(value[end..].trim_start());
            return out.trim().to_string();
        } else {
            // Fallback: if start marker exists but end marker is missing,
            // we at least try to remove the start marker literal and hope for the best.
            // This handles leftovers from potentially interrupted writes or old versions.
            let mut out = value.to_string();
            out.replace_range(start..start + KAEDE_STEAM_START.len(), "");
            return out.trim().to_string();
        }
    }

    value.to_string()
}

fn find_launch_options_line(
    content: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize, Option<String>, String)> {
    let key = "\"LaunchOptions\"";
    let rel = content[start..end].find(key)?;
    let key_pos = start + rel;
    let line_start = content[..key_pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = content[key_pos..]
        .find('\n')
        .map(|i| key_pos + i + 1)
        .unwrap_or(content.len());
    let line = &content[line_start..line_end];
    let value = parse_launch_options_value(line);
    let indent = line
        .chars()
        .take_while(|c| *c == '\t' || *c == ' ')
        .collect::<String>();
    Some((line_start, line_end, value, indent))
}

#[derive(Debug, Clone)]
struct AppState {
    app_found: bool,
    launch_options: Option<String>,
}

fn app_state_in_localconfig(content: &str, app_id: &str) -> AppState {
    let Some((_, apps_open, apps_close)) =
        find_block_by_key_in_range_ci(content, "apps", 0, content.len())
    else {
        return AppState {
            app_found: false,
            launch_options: None,
        };
    };

    let Some((_, app_open, app_close)) =
        find_block_by_key_in_range(content, app_id, apps_open + 1, apps_close)
    else {
        return AppState {
            app_found: false,
            launch_options: None,
        };
    };

    let launch_options = find_launch_options_line(content, app_open + 1, app_close)
        .and_then(|(_, _, value, _)| value);

    AppState {
        app_found: true,
        launch_options,
    }
}

fn validate_expected_state(launch_options: Option<&str>, choice: &GpuChoice) -> bool {
    match choice {
        GpuChoice::Default => launch_options
            .map(|v| !v.contains(KAEDE_STEAM_START) && !v.contains(KAEDE_STEAM_END))
            .unwrap_or(true),
        GpuChoice::Gpu(idx) => launch_options
            .map(|v| {
                v.contains(KAEDE_STEAM_START)
                    && v.contains(KAEDE_STEAM_END)
                    && v.contains(&format!("DRI_PRIME={idx}"))
            })
            .unwrap_or(false),
    }
}

fn parse_launch_options_value(line: &str) -> Option<String> {
    let quote_positions = line.match_indices('"').map(|(i, _)| i).collect::<Vec<_>>();
    if quote_positions.len() < 4 {
        return None;
    }

    let start = quote_positions[2] + 1;
    let end = quote_positions[3];
    if end < start || end > line.len() {
        return None;
    }

    Some(line[start..end].trim().to_string())
}

fn indentation_at(content: &str, idx: usize) -> String {
    let start = content[..idx].rfind('\n').map(|v| v + 1).unwrap_or(0);
    content[start..idx]
        .chars()
        .take_while(|c| *c == '\t' || *c == ' ')
        .collect::<String>()
}

fn find_block_by_key_in_range(
    content: &str,
    key: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize, usize)> {
    let needle = format!("\"{key}\"");
    let mut search = start;

    while search < end {
        let rel = content[search..end].find(&needle)?;
        let key_pos = search + rel;
        let mut i = key_pos + needle.len();
        let bytes = content.as_bytes();

        while i < end && (bytes[i] as char).is_whitespace() {
            i += 1;
        }

        if i >= end || bytes[i] != b'{' {
            search = key_pos + needle.len();
            continue;
        }

        let close = match_matching_brace(content, i, end)?;
        return Some((key_pos, i, close));
    }

    None
}

fn find_block_by_key_in_range_ci(
    content: &str,
    key: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize, usize)> {
    let key_lower = key.to_ascii_lowercase();
    let mut search = start;

    while search < end {
        let rel = content[search..end].find('\"')?;
        let q1 = search + rel;
        let q2_rel = content[q1 + 1..end].find('\"')?;
        let q2 = q1 + 1 + q2_rel;
        let token = &content[q1 + 1..q2];

        if token.eq_ignore_ascii_case(&key_lower) {
            let mut i = q2 + 1;
            let bytes = content.as_bytes();
            while i < end && (bytes[i] as char).is_whitespace() {
                i += 1;
            }
            if i < end && bytes[i] == b'{' {
                let close = match_matching_brace(content, i, end)?;
                return Some((q1, i, close));
            }
        }

        search = q2 + 1;
    }

    None
}

fn match_matching_brace(content: &str, open: usize, end_limit: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    if open >= end_limit || bytes[open] != b'{' {
        return None;
    }

    let mut depth = 0isize;
    let mut in_string = false;
    let mut escaped = false;

    for (i, b) in bytes.iter().enumerate().take(end_limit).skip(open) {
        let ch = *b as char;

        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            continue;
        }

        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }

    None
}
