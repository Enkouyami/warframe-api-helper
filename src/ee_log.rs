//! Read `accountId` from Warframe `EE.log` (Windows / Steam Proton / Wine paths).

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use regex::Regex;

/// Return the first 24-hex `accountId` found in a known `EE.log` location.
pub fn read_account_id_from_ee_log() -> Result<Option<String>> {
    let ee_log_paths = get_ee_log_paths();

    for path in ee_log_paths {
        if !path.exists() {
            continue;
        }

        let content = fs::read_to_string(&path)
            .context(format!("Failed to read EE.log: {:?}", path))?;

        if content.is_empty() {
            continue;
        }

        // Try Format 1: "Logged in Username (0123456789abcdef01234567)"
        if let Some(account_id) = extract_from_logged_in(&content) {
            return Ok(Some(account_id));
        }

        // Try Format 2: "AccountId: 0123456789abcdef01234567"
        if let Some(account_id) = extract_from_account_id_line(&content) {
            return Ok(Some(account_id));
        }

        // Fallback: Search for any 24-character hex string
        if let Some(account_id) = extract_hex_string(&content) {
            return Ok(Some(account_id));
        }
    }

    Ok(None)
}

fn get_ee_log_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Windows locations
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        paths.push(PathBuf::from(local_app_data).join("Warframe").join("EE.log"));
    }

    // Linux/Wine locations
    if let Ok(home) = std::env::var("HOME") {
        // Linux (Steam)
        paths.push(PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("Steam")
            .join("steamapps")
            .join("compatdata")
            .join("230410")
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser")
            .join("AppData")
            .join("Local")
            .join("Warframe")
            .join("EE.log"));

        // Steam Flatpak
        paths.push(PathBuf::from(&home)
            .join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join(".local")
            .join("share")
            .join("Steam")
            .join("steamapps")
            .join("compatdata")
            .join("230410")
            .join("pfx")
            .join("drive_c")
            .join("users")
            .join("steamuser")
            .join("AppData")
            .join("Local")
            .join("Warframe")
            .join("EE.log"));

        // Generic Wine prefix
        let user = std::env::var("USER").unwrap_or_else(|_| "steamuser".to_string());
        paths.push(PathBuf::from(&home)
            .join(".wine")
            .join("drive_c")
            .join("users")
            .join(&user)
            .join("AppData")
            .join("Local")
            .join("Warframe")
            .join("EE.log"));
    }

    // Current directory
    paths.push(PathBuf::from("EE.log"));

    paths
}

fn extract_from_logged_in(content: &str) -> Option<String> {
    let re = Regex::new(r"Logged in[^(]*\(([0-9a-fA-F]{24})\)").ok()?;
    if let Some(captures) = re.captures(content) {
        if let Some(account_id) = captures.get(1) {
            return Some(account_id.as_str().to_string());
        }
    }
    None
}

fn extract_from_account_id_line(content: &str) -> Option<String> {
    let re = Regex::new(r"AccountId:\s*([0-9a-fA-F]{24})").ok()?;
    if let Some(captures) = re.captures(content) {
        if let Some(account_id) = captures.get(1) {
            return Some(account_id.as_str().to_string());
        }
    }
    None
}

fn extract_hex_string(content: &str) -> Option<String> {
    let re = Regex::new(r"([0-9a-fA-F]{24})").ok()?;
    for captures in re.captures_iter(content) {
        if let Some(matched) = captures.get(1) {
            let account_id = matched.as_str();
            // Verify it's not part of a longer hex string
            let start = matched.start();
            let end = matched.end();
            if (start == 0 || !content.chars().nth(start - 1).unwrap_or(' ').is_ascii_hexdigit())
                && (end >= content.len() || !content.chars().nth(end).unwrap_or(' ').is_ascii_hexdigit())
            {
                return Some(account_id.to_string());
            }
        }
    }
    None
}
