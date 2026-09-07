//! Warframe API Helper — scan the running game for `accountId`/`nonce`, download
//! inventory, and write `inventory*.json` plus encrypted `lastData*.dat`.
//!
//! Default AES key/IV match Sainan/warframe-api-helper (and AlecaFrame / browse.wf).

mod config;
mod crypto;
mod memory;
mod api;
mod ee_log;
mod storage;
mod types;
mod pattern;
mod output;
mod process;
mod search_workflow;
mod download_workflow;

use anyhow::{Context, Result};
use clap::Parser;
use std::env;

use config::{Config, EncryptionKey};
use output::OutputFormatter;
use search_workflow::SearchWorkflow;
use download_workflow::DownloadWorkflow;
use pattern::PatternMatcher;
use ee_log::read_account_id_from_ee_log;
use storage::load_account_id_from_dat_files;

#[derive(Parser, Debug)]
#[command(name = "warframe-api-helper")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Search for ?accountId= and &nonce= strings in the running Warframe process, then download inventory.")]
struct Args {
    /// Skip downloading inventory data from API after finding accountId (default: download is enabled)
    #[arg(long)]
    no_download: bool,

    /// Continue searching after finding first match with accountId and nonce (default: stop at first match)
    #[arg(long)]
    all_matches: bool,

    /// Verbose output: print raw memory matches (address/region/string/context) and the
    /// extracted "?accountId=...&nonce=..." string
    #[arg(short, long)]
    verbose: bool,

    /// Process ID (auto-detected if not specified)
    #[arg(long)]
    pid: Option<u32>,

    /// Output file for downloaded inventory data (default: inventory.json)
    #[arg(long)]
    output: Option<String>,

    /// Include accountId in JSON output
    #[arg(short = 'o', long)]
    original_format: bool,

    /// Password for Argon2 key derivation
    #[arg(long, value_name = "PASSWORD")]
    password: Option<String>,

    /// Short form for password
    #[arg(short = 'p', value_name = "PASSWORD")]
    password_short: Option<String>,

    /// Account ID (24 hex characters)
    #[arg(long = "account-id", value_name = "ACCOUNT_ID")]
    account_id: Option<String>,

    /// Short form for account ID
    #[arg(short = 'a', value_name = "ACCOUNT_ID")]
    account_id_short: Option<String>,

    /// Nonce (numeric string)
    #[arg(long, value_name = "NONCE")]
    nonce: Option<String>,

    /// Short form for nonce
    #[arg(short = 'n', value_name = "NONCE")]
    nonce_short: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let formatter = OutputFormatter::with_verbose(args.verbose);
    // std::env::consts::OS is lowercase ("windows"/"linux"/"macos"), but the rest of
    // the code compares against platform.system()-style names ("Windows"/"Linux"/"Darwin").
    let platform_name = match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "Darwin",
        "linux" => "Linux",
        other => other,
    };

    formatter.print_platform_info(platform_name);

    // --nonce / -n or NONCE skips process scanning and uses that nonce.
    if let Some(provided_nonce) = args
        .nonce
        .as_ref()
        .or(args.nonce_short.as_ref())
        .cloned()
        .or_else(|| env::var("NONCE").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return handle_nonce_path(args, formatter, provided_nonce);
    }

    let matches = SearchWorkflow::execute(
        &formatter,
        platform_name,
        args.pid,
        args.all_matches,
    )?;

    if matches.is_empty() {
        formatter.println_colored(
            crate::output::Colors::YELLOW,
            "Process scanning found no matches. Trying EE.log as fallback..."
        );

        if let Some(account_id) = read_account_id_from_ee_log()? {
            formatter.println_colored(
                crate::output::Colors::GREEN,
                &format!("Found accountId in EE.log: {}", account_id)
            );
            formatter.println_colored(
                crate::output::Colors::YELLOW,
                "Note: EE.log only provides accountId. To download inventory, you need to provide --nonce flag."
            );
            println!("accountId: {}", account_id);
            return Ok(());
        } else {
            anyhow::bail!("No matches found in process memory or EE.log.");
        }
    }

    let (best_account_id, best_nonce) = PatternMatcher::find_best_match(&matches);

    if let (Some(account_id), Some(nonce)) = (best_account_id, best_nonce) {
        // Print auth string only in verbose mode.
        if formatter.verbose {
            let auth = format!("?accountId={}&nonce={}", account_id, nonce);
            println!("{}", auth);
        }

        if !args.no_download {
            let config = get_config_for_download(&args)?;
            let output_clone = args.output.clone();
            DownloadWorkflow::execute(&formatter, &matches, &config, output_clone)?;
        }
    } else {
        anyhow::bail!("Could not extract accountId and nonce from matches.");
    }

    Ok(())
}

/// Nonce path: build auth from provided nonce (no process scan).
fn handle_nonce_path(
    args: Args,
    formatter: OutputFormatter,
    provided_nonce: String,
) -> Result<()> {
    use storage::save_inventory_data;
    use api::InventoryDownloader;

    let config = Config {
        encryption_key: get_encryption_key(&args)?,
        original_format: args.original_format,
    };

    let account_id = get_account_id_from_sources(&config, &args)?;
    if account_id.len() != 24 {
        anyhow::bail!(
            "Could not find account ID. Please:\n  - Provide it via --account-id flag or ACCOUNT_ID environment variable\n  - Or run the tool once without --nonce to create a lastData.dat file\n  - Or ensure EE.log is accessible"
        );
    }

    let auth = format!("?accountId={}&nonce={}", account_id, provided_nonce);
    formatter.println_colored(
        output::Colors::YELLOW,
        "Using provided nonce (--nonce or NONCE environment variable)."
    );
    if args.verbose {
        println!("{}", auth);
    } else {
        formatter.println_colored(
            output::Colors::YELLOW,
            &format!("Using accountId: {} (nonce redacted; pass -v to echo)", account_id),
        );
    }

    if !args.no_download {
        let (account_id_val, nonce_val) = PatternMatcher::extract_account_id_and_nonce(&auth);
        let account_id_val = account_id_val.ok_or_else(|| anyhow::anyhow!("No accountId in authz string"))?;
        let nonce_val = nonce_val.ok_or_else(|| anyhow::anyhow!("No nonce in authz string"))?;

        let inventory_json = InventoryDownloader::download(
            &account_id_val,
            &nonce_val,
            args.output.clone(),
            args.verbose,
        )
            .context("Request failed")?;

        let inventory: serde_json::Value = serde_json::from_str(&inventory_json)
            .context("Received an invalid response")?;

        let account_name = extract_account_name(&inventory);
        save_inventory_data(
            &inventory,
            &inventory_json,
            Some(&account_id_val),
            &account_name,
            &config,
            args.output,
        )?;

        formatter.println_colored(
            output::Colors::GREEN,
            "Inventory download successful!"
        );
    }

    Ok(())
}

fn get_config_for_download(args: &Args) -> Result<Config> {
    Ok(Config {
        encryption_key: get_encryption_key(args)?,
        original_format: args.original_format,
    })
}

fn get_encryption_key(args: &Args) -> Result<EncryptionKey> {
    if let Some(password) = args.password.as_ref().or(args.password_short.as_ref()) {
        return crypto::derive_key_from_password(password)
            .map(EncryptionKey::from_bytes)
            .context("Failed to derive key from password");
    }

    if let Ok(password) = env::var("ENCRYPTION_PASSWORD") {
        return crypto::derive_key_from_password(&password)
            .map(EncryptionKey::from_bytes)
            .context("Failed to derive key from ENCRYPTION_PASSWORD");
    }

    // Same default key as the C++ helper when no --password is set.
    Ok(EncryptionKey::from_bytes(crypto::DEFAULT_KEY))
}

fn get_account_id_from_sources(config: &Config, args: &Args) -> Result<String> {
    let account_id_from_args = args.account_id.as_ref()
        .or(args.account_id_short.as_ref());
    let account_id_from_env = env::var("ACCOUNT_ID").ok();

    if let Some(account_id) = account_id_from_args.or(account_id_from_env.as_ref()) {
        let account_id = account_id.trim().to_string();
        if account_id.len() == 24 {
            return Ok(account_id);
        }
    }

    if let Some(account_id) = load_account_id_from_dat_files(&config.encryption_key)? {
        if account_id.len() == 24 {
            return Ok(account_id);
        }
    }

    if let Some(account_id) = read_account_id_from_ee_log()? {
        if account_id.len() == 24 {
            return Ok(account_id);
        }
    }

    Ok(String::new())
}

fn extract_account_name(inventory: &serde_json::Value) -> String {
    let name_fields = ["playerName", "PlayerName", "alias", "Alias", "name", "Name", "username", "Username"];

    for field in &name_fields {
        if let Some(name) = inventory.get(field).and_then(|v| v.as_str()) {
            return name.to_string();
        }
    }

    "unknown".to_string()
}
