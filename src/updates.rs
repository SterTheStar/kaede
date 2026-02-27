use serde::Deserialize;
use tracing::{info, error};

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

pub enum UpdateResult {
    NewRelease(String),
    UpToDate,
    Beta,
}

pub fn check_for_updates() -> anyhow::Result<UpdateResult> {
    info!("Checking for updates on GitHub...");
    let url = "https://api.github.com/repos/SterTheStar/kaede/releases/latest";
    
    let agent = ureq::Agent::new();
    let resp = match agent.get(url)
        .set("User-Agent", "kaede-update-checker")
        .call() {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to connect to GitHub API: {}", e);
                return Err(e.into());
            }
        };

    if resp.status() == 200 {
        let release: GithubRelease = match resp.into_json() {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to parse GitHub release JSON: {}", e);
                return Err(e.into());
            }
        };
        
        let latest_version = release.tag_name.trim_start_matches('v').to_string();
        let current_version = env!("CARGO_PKG_VERSION");

        match compare_versions(&latest_version, current_version) {
            std::cmp::Ordering::Greater => {
                info!("Update found: {} (currently running {})", latest_version, current_version);
                Ok(UpdateResult::NewRelease(latest_version))
            }
            std::cmp::Ordering::Less => {
                info!("Running a pre-release/beta version: {} (latest stable: {})", current_version, latest_version);
                Ok(UpdateResult::Beta)
            }
            std::cmp::Ordering::Equal => {
                info!("No newer updates found. Running version: {}", current_version);
                Ok(UpdateResult::UpToDate)
            }
        }
    } else {
        error!("Unexpected response from GitHub API: status {}", resp.status());
        Ok(UpdateResult::UpToDate)
    }
}

fn compare_versions(latest: &str, current: &str) -> std::cmp::Ordering {
    let latest_parts: Vec<u32> = latest.split('.').filter_map(|s| s.parse().ok()).collect();
    let current_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
        if l > c { return std::cmp::Ordering::Greater; }
        if l < c { return std::cmp::Ordering::Less; }
    }

    latest_parts.len().cmp(&current_parts.len())
}
