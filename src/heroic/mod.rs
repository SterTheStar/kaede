use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

const KAEDE_MARKER_KEY: &str = "KAEDE_GPU_MANAGED";

pub fn apply_heroic_launch_env(platform: &str, app_name: &str, env_vars: &[String]) -> Result<()> {
    let files = find_heroic_game_config_candidates(app_name);
    if files.is_empty() {
        anyhow::bail!("Heroic config not found for app {}", app_name);
    }

    let mut matched = false;
    let mut validated = false;

    for path in files {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read Heroic config {}", path.display()))?;
        let mut json: Value = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse Heroic config {}", path.display()))?;

        if !heroic_config_matches_game(&json, &raw, &path, app_name) {
            debug!(path = %path.display(), app = app_name, "Heroic config does not match game");
            continue;
        }

        matched = true;
        let changed = apply_env_to_heroic_json(&mut json, app_name, env_vars)?;
        if changed {
            write_backup_if_missing(&path, &raw)?;
            let body = serde_json::to_string_pretty(&json)
                .with_context(|| format!("failed to serialize Heroic config {}", path.display()))?;
            fs::write(&path, body)
                .with_context(|| format!("failed to write Heroic config {}", path.display()))?;
            info!(path = %path.display(), platform = platform, app = app_name, "Heroic env updated");
        } else {
            info!(path = %path.display(), platform = platform, app = app_name, "Heroic env already in desired state");
        }

        let verify_raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to re-read Heroic config {}", path.display()))?;
        let verify_json: Value = serde_json::from_str(&verify_raw).with_context(|| {
            format!(
                "failed to parse Heroic config after write {}",
                path.display()
            )
        })?;
        if validate_env_in_heroic_json(&verify_json, app_name, env_vars) {
            validated = true;
        }
    }

    if !matched {
        warn!(
            platform = platform,
            app = app_name,
            "Heroic config candidates found but no matching game key"
        );
        anyhow::bail!("Heroic game {} not matched in configs", app_name);
    }
    if !validated {
        anyhow::bail!("Heroic game {} found but env validation failed", app_name);
    }

    Ok(())
}

fn find_heroic_game_config_candidates(app_name: &str) -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let bases = [
        PathBuf::from(&home).join(".config/heroic/GamesConfig"),
        PathBuf::from(&home).join(".var/app/com.heroicgameslauncher.hgl/config/heroic/GamesConfig"),
    ];

    let mut out = Vec::new();
    for base in bases {
        let Ok(entries) = fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            // prioritize exact filename matches first
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if stem.eq_ignore_ascii_case(app_name) {
                out.push(path.clone());
            } else {
                out.push(path);
            }
        }
    }

    out.sort_by_key(|p| {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        if stem.eq_ignore_ascii_case(app_name) {
            0
        } else {
            1
        }
    });
    out.dedup();
    out
}

fn heroic_config_matches_game(json: &Value, raw: &str, path: &Path, app_name: &str) -> bool {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .trim();
    if stem.eq_ignore_ascii_case(app_name) {
        return true;
    }

    if let Some(v) = json.get("appName").and_then(|v| v.as_str()) {
        if v.eq_ignore_ascii_case(app_name) {
            return true;
        }
    }
    if let Some(v) = json.get("gameId").and_then(|v| v.as_str()) {
        if v.eq_ignore_ascii_case(app_name) {
            return true;
        }
    }
    if let Some(v) = json.get("title").and_then(|v| v.as_str()) {
        if v.eq_ignore_ascii_case(app_name) {
            return true;
        }
    }

    // Fallback for schema variations: look for the app id token in raw JSON body.
    raw.contains(app_name)
}

fn apply_env_to_heroic_json(json: &mut Value, app_name: &str, env_vars: &[String]) -> Result<bool> {
    let obj = json
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Heroic game config root is not a JSON object"))?;

    let mut desired = env_pairs_to_map(env_vars);
    if !env_vars.is_empty() {
        desired.insert(KAEDE_MARKER_KEY.to_string(), Value::String("1".to_string()));
    }

    let mut changed = false;

    // Keep top-level envVariables in sync for compatibility.
    match obj.get_mut("envVariables") {
        Some(Value::Object(env_obj)) => {
            changed |= upsert_env_object(env_obj, &desired);
            changed |= remove_absent_managed(env_obj, &desired);
        }
        Some(Value::Array(arr)) => {
            changed |= upsert_env_array(arr, &desired);
            changed |= remove_absent_managed_array(arr, &desired);
        }
        Some(_) => {
            let mut env_obj = Map::new();
            for (k, v) in &desired {
                env_obj.insert(k.clone(), v.clone());
            }
            obj.insert("envVariables".to_string(), Value::Object(env_obj));
            changed = true;
        }
        None => {
            let mut env_obj = Map::new();
            for (k, v) in &desired {
                env_obj.insert(k.clone(), v.clone());
            }
            obj.insert("envVariables".to_string(), Value::Object(env_obj));
            changed = true;
        }
    }

    // Heroic UI reads this field for per-game environment variables.
    changed |= upsert_game_enviroment_options(obj, app_name, &desired);

    Ok(changed)
}

fn validate_env_in_heroic_json(json: &Value, app_name: &str, env_vars: &[String]) -> bool {
    let mut desired = env_pairs_to_map(env_vars);
    if !env_vars.is_empty() {
        desired.insert(KAEDE_MARKER_KEY.to_string(), Value::String("1".to_string()));
    }

    let top_ok = match json.get("envVariables") {
        Some(env) => match env {
            Value::Object(map) => desired
                .iter()
                .all(|(k, v)| map.get(k).and_then(|x| x.as_str()) == v.as_str()),
            Value::Array(arr) => desired.iter().all(|(k, v)| {
                let val = v.as_str().unwrap_or_default();
                arr.iter().any(|item| {
                    item.get("name").and_then(|n| n.as_str()) == Some(k.as_str())
                        && item.get("value").and_then(|vv| vv.as_str()) == Some(val)
                })
            }),
            _ => false,
        },
        None => desired.is_empty(),
    };
    let game_ok = validate_game_enviroment_options(json, app_name, &desired);

    top_ok && game_ok
}

fn env_pairs_to_map(env_vars: &[String]) -> Map<String, Value> {
    let mut out = Map::new();
    for pair in env_vars {
        if let Some((k, v)) = pair.split_once('=') {
            out.insert(k.to_string(), Value::String(v.to_string()));
        }
    }
    out
}

fn upsert_env_object(env_obj: &mut Map<String, Value>, desired: &Map<String, Value>) -> bool {
    let mut changed = false;
    for (k, v) in desired {
        if env_obj.get(k) != Some(v) {
            env_obj.insert(k.clone(), v.clone());
            changed = true;
        }
    }
    changed
}

fn remove_absent_managed(env_obj: &mut Map<String, Value>, desired: &Map<String, Value>) -> bool {
    let managed_keys = [
        "DRI_PRIME",
        "PRESSURE_VESSEL_IMPORT_VARS",
        "__NV_PRIME_RENDER_OFFLOAD",
        "__GLX_VENDOR_LIBRARY_NAME",
        "__VK_LAYER_NV_optimus",
        "MESA_VK_DEVICE_SELECT",
        "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE",
        "DXVK_FILTER_DEVICE_NAME",
        KAEDE_MARKER_KEY,
    ];

    let mut changed = false;
    for key in managed_keys {
        if !desired.contains_key(key) && env_obj.remove(key).is_some() {
            changed = true;
        }
    }
    changed
}

fn upsert_env_array(arr: &mut Vec<Value>, desired: &Map<String, Value>) -> bool {
    let mut changed = false;
    for (k, v) in desired {
        let val = v.as_str().unwrap_or_default();
        let mut found = false;
        for item in arr.iter_mut() {
            if item.get("name").and_then(|n| n.as_str()) == Some(k.as_str()) {
                found = true;
                if item.get("value").and_then(|vv| vv.as_str()) != Some(val) {
                    item["value"] = Value::String(val.to_string());
                    changed = true;
                }
            }
        }
        if !found {
            arr.push(serde_json::json!({"name": k, "value": val}));
            changed = true;
        }
    }
    changed
}

fn remove_absent_managed_array(arr: &mut Vec<Value>, desired: &Map<String, Value>) -> bool {
    let managed_keys = [
        "DRI_PRIME",
        "PRESSURE_VESSEL_IMPORT_VARS",
        "__NV_PRIME_RENDER_OFFLOAD",
        "__GLX_VENDOR_LIBRARY_NAME",
        "__VK_LAYER_NV_optimus",
        "MESA_VK_DEVICE_SELECT",
        "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE",
        "DXVK_FILTER_DEVICE_NAME",
        KAEDE_MARKER_KEY,
    ];

    let before = arr.len();
    arr.retain(|item| {
        let Some(name) = item.get("name").and_then(|n| n.as_str()) else {
            return true;
        };
        if managed_keys.contains(&name) {
            return desired.contains_key(name);
        }
        true
    });
    arr.len() != before
}

fn upsert_game_enviroment_options(
    root: &mut Map<String, Value>,
    app_name: &str,
    desired: &Map<String, Value>,
) -> bool {
    let Some(game_obj) = root.get_mut(app_name).and_then(|v| v.as_object_mut()) else {
        return false;
    };

    let mut existing_pairs = Vec::<(String, String)>::new();
    if let Some(Value::Array(arr)) = game_obj.get("enviromentOptions") {
        for item in arr {
            if let Some((k, v)) = parse_env_option_entry(item) {
                existing_pairs.push((k, v));
            }
        }
    }

    let managed_keys = [
        "DRI_PRIME",
        "PRESSURE_VESSEL_IMPORT_VARS",
        "__NV_PRIME_RENDER_OFFLOAD",
        "__GLX_VENDOR_LIBRARY_NAME",
        "__VK_LAYER_NV_optimus",
        "MESA_VK_DEVICE_SELECT",
        "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE",
        "DXVK_FILTER_DEVICE_NAME",
        KAEDE_MARKER_KEY,
    ];

    existing_pairs.retain(|(k, _)| !managed_keys.contains(&k.as_str()));

    let mut new_pairs = existing_pairs;
    for (k, v) in desired {
        if let Some(vs) = v.as_str() {
            new_pairs.push((k.clone(), vs.to_string()));
        }
    }

    let new_array = new_pairs
        .iter()
        .map(|(k, v)| serde_json::json!({ "key": k, "value": v }))
        .collect::<Vec<_>>();

    let changed = game_obj.get("enviromentOptions") != Some(&Value::Array(new_array.clone()));
    if changed {
        game_obj.insert("enviromentOptions".to_string(), Value::Array(new_array));
    }
    changed
}

fn validate_game_enviroment_options(
    json: &Value,
    app_name: &str,
    desired: &Map<String, Value>,
) -> bool {
    let Some(game_obj) = json.get(app_name).and_then(|v| v.as_object()) else {
        return false;
    };
    let Some(Value::Array(arr)) = game_obj.get("enviromentOptions") else {
        return desired.is_empty();
    };

    desired.iter().all(|(k, v)| {
        let val = v.as_str().unwrap_or_default();
        arr.iter().any(|item| {
            parse_env_option_entry(item)
                .map(|(ek, ev)| ek == *k && ev == val)
                .unwrap_or(false)
        })
    })
}

fn parse_env_option_entry(item: &Value) -> Option<(String, String)> {
    if let Some(s) = item.as_str() {
        let (k, v) = s.split_once('=')?;
        return Some((k.to_string(), v.to_string()));
    }
    let k = item
        .get("key")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("name").and_then(|v| v.as_str()))?;
    let v = item.get("value").and_then(|v| v.as_str())?;
    Some((k.to_string(), v.to_string()))
}

fn write_backup_if_missing(path: &Path, content: &str) -> Result<()> {
    let backup = path.with_extension("json.kaede.bak");
    if !backup.exists() {
        fs::write(&backup, content)
            .with_context(|| format!("failed to write Heroic backup {}", backup.display()))?;
        info!(backup = %backup.display(), "Heroic config backup created");
    }
    Ok(())
}
