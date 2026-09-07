//! Inventory JSON and encrypted `lastData.dat` I/O under [`data_directory`].

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::crypto::{decrypt_data, encrypt_data, EncryptionKey};

/// Directory where `inventory.json`, `lastData.dat`, and related files are stored.
///
/// Normally this is the executable's directory. When the binary lives under
/// `target/release` or `target/debug`, files go to the crate root (the parent
/// of `target`) so they are not buried in build output.
pub fn data_directory() -> Result<PathBuf> {
    let exe_path = std::env::current_exe().context("Failed to resolve executable path")?;
    let exe_dir = exe_path
        .parent()
        .context("Executable has no parent directory")?
        .to_path_buf();

    let profile_name = exe_dir.file_name().and_then(|name| name.to_str());
    let parent_name = exe_dir
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str());

    if parent_name == Some("target")
        && matches!(profile_name, Some("release") | Some("debug"))
    {
        if let Some(crate_root) = exe_dir.parent().and_then(|target_dir| target_dir.parent()) {
            return Ok(crate_root.to_path_buf());
        }
    }

    Ok(exe_dir)
}

/// Resolve a user-provided or default filename against [`data_directory`].
pub fn resolve_data_path(filename: impl AsRef<Path>) -> Result<PathBuf> {
    let path = filename.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(data_directory()?.join(path))
    }
}

/// Read `accountId` from `lastData.dat` / `lastData_*.dat` using `key`.
pub fn load_account_id_from_dat_files(key: &EncryptionKey) -> Result<Option<String>> {
    let data_dir = data_directory()?;
    let mut dat_files = Vec::new();

    let default_dat = data_dir.join("lastData.dat");
    if default_dat.exists() {
        dat_files.push(default_dat);
    }

    // Search for lastData_*.dat files in the data directory
    for entry in WalkDir::new(&data_dir).max_depth(1).into_iter() {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                let filename = entry.file_name().to_string_lossy();
                if filename.starts_with("lastData_") && filename.ends_with(".dat") {
                    dat_files.push(entry.path().to_path_buf());
                }
            }
        }
    }

    for dat_file in dat_files {
        let encrypted = fs::read(&dat_file)
            .context(format!("Failed to read {:?}", dat_file))?;

        if encrypted.is_empty() {
            continue;
        }

        // Decrypt
        let decrypted = decrypt_data(&encrypted, key.as_bytes())
            .context(format!("Failed to decrypt {:?}", dat_file))?;

        // Parse JSON to extract accountId
        let json: Value = serde_json::from_str(&decrypted)
            .context(format!("Failed to parse JSON from {:?}", dat_file))?;

        if let Some(account_id) = json.get("accountId").and_then(|v| v.as_str()) {
            if account_id.len() == 24 {
                return Ok(Some(account_id.to_string()));
            }
        }
    }

    Ok(None)
}

/// Write `inventory*.json` and encrypted `lastData*.dat` (inventory + `accountId`).
pub fn save_inventory_data(
    inventory: &Value,
    inventory_json: &str,
    account_id: Option<&str>,
    account_name: &str,
    config: &crate::config::Config,
    output_file: Option<String>,
) -> Result<()> {
    // Create filename with account name suffix (unless original format or output file specified)
    let json_basename = if let Some(output) = output_file {
        output
    } else if !config.original_format && account_name != "unknown" && !account_name.is_empty() {
        let sanitized_name = sanitize_filename(account_name);
        format!("inventory_{}.json", sanitized_name)
    } else {
        "inventory.json".to_string()
    };
    let json_path = resolve_data_path(&json_basename)?;

    // Remove accountId from JSON before saving (unless original format)
    let mut json_to_save = inventory.clone();
    if !config.original_format {
        if let Some(obj) = json_to_save.as_object_mut() {
            obj.remove("accountId");
        }
    }

    // Save JSON
    let json_pretty = serde_json::to_string_pretty(&json_to_save)
        .context("Failed to serialize JSON")?;
    fs::write(&json_path, json_pretty)
        .context(format!("Failed to write {}", json_path.display()))?;
    println!("Saved to {}", json_path.display());

    // Ensure accountId is present in the lastData.dat payload (C++ helper).
    let data_to_encrypt = if let Some(aid) = account_id {
        let mut data_with_account = inventory.clone();
        if let Some(obj) = data_with_account.as_object_mut() {
            obj.insert("accountId".to_string(), Value::String(aid.to_string()));
            obj.remove("nonce");
        }
        serde_json::to_string(&data_with_account).context("Failed to serialize data for encryption")?
    } else {
        let mut data_for_encrypt = inventory.clone();
        if let Some(obj) = data_for_encrypt.as_object_mut() {
            obj.remove("nonce");
        }
        serde_json::to_string(&data_for_encrypt)
            .unwrap_or_else(|_| inventory_json.to_string())
    };

    let encrypted = encrypt_data(&data_to_encrypt, config.encryption_key.as_bytes())
        .context("Failed to encrypt data")?;

    let dat_basename = if !config.original_format && account_name != "unknown" && !account_name.is_empty() {
        let sanitized_name = sanitize_filename(account_name);
        format!("lastData_{}.dat", sanitized_name)
    } else {
        "lastData.dat".to_string()
    };
    let dat_path = resolve_data_path(&dat_basename)?;

    fs::write(&dat_path, encrypted)
        .context(format!("Failed to write {}", dat_path.display()))?;
    println!("Saved to {} (encrypted)", dat_path.display());

    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let mut sanitized = name.to_string();
    // Replace invalid filename characters
    for character in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        sanitized = sanitized.replace(character, "_");
    }
    // Limit length
    if sanitized.len() > 50 {
        sanitized.truncate(50);
    }
    sanitized
}
