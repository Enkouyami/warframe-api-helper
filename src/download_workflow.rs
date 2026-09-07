//! Pick best match, download inventory, and save JSON / `lastData.dat`.

use anyhow::{Context, Result};
use crate::pattern::PatternMatcher;
use crate::output::OutputFormatter;
use crate::api::InventoryDownloader;
use crate::storage::save_inventory_data;
use crate::config::Config;
use serde_json::Value;

pub struct DownloadWorkflow;

impl DownloadWorkflow {
    /// Download using the best complete auth match, then write outputs.
    pub fn execute(
        formatter: &OutputFormatter,
        matches: &[crate::types::Match],
        config: &Config,
        output_file: Option<String>,
    ) -> Result<()> {
        formatter.println_colored(
            crate::output::Colors::YELLOW,
            "=== Downloading Inventory ==="
        );
        
        let (best_account_id, best_nonce) = PatternMatcher::find_best_match(matches);
        
        let account_id = best_account_id.ok_or_else(|| {
            anyhow::anyhow!("Could not extract accountId from matches. Make sure Warframe is running and you are logged in.")
        })?;
        
        let nonce = best_nonce.ok_or_else(|| {
            anyhow::anyhow!("Could not extract nonce from matches. Make sure Warframe is running and you are logged in.")
        })?;
        
        formatter.println_colored(
            crate::output::Colors::YELLOW,
            &format!("Using accountId: {}", account_id)
        );
        formatter.println_colored(
            crate::output::Colors::YELLOW,
            &format!("Using nonce: {}", nonce)
        );
        
        // Download inventory
        let inventory_json = InventoryDownloader::download(
            &account_id,
            &nonce,
            output_file.clone(),
            formatter.verbose,
        )
        .context("Failed to download inventory")?;
        
        // Parse JSON to extract account ID and name
        let inventory: Value = serde_json::from_str(&inventory_json)
            .context("Received invalid JSON response")?;
        
        let account_name = Self::extract_account_name(&inventory);
        
        save_inventory_data(
            &inventory,
            &inventory_json,
            Some(&account_id),
            &account_name,
            config,
            output_file,
        )?;
        
        formatter.println_colored(
            crate::output::Colors::GREEN,
            "Inventory download successful!"
        );
        
        Ok(())
    }
    
    fn extract_account_name(inventory: &Value) -> String {
        let name_fields = ["playerName", "PlayerName", "alias", "Alias", "name", "Name", "username", "Username"];
        
        for field in &name_fields {
            if let Some(name) = inventory.get(field).and_then(|v| v.as_str()) {
                return name.to_string();
            }
        }
        
        "unknown".to_string()
    }
}
