//! Inventory download (`mobile.warframe.com`, then `api.warframe.com`).

use anyhow::{Context, Result};
use crate::output::{Colors, OutputFormatter};
use crate::storage::resolve_data_path;

/// Prefix for HTTP 409 / `Log-in expired` failures.
pub const LOGIN_EXPIRED_PREFIX: &str = "LOGIN_EXPIRED";

pub struct InventoryDownloader;

impl InventoryDownloader {
    const ENDPOINTS: &'static [&'static str] = &[
        "https://mobile.warframe.com/api/inventory.php",
        "https://api.warframe.com/api/inventory.php",
    ];

    /// HTTP 409 or body `Log-in expired` means the nonce is no longer valid.
    fn is_login_expired(status: reqwest::StatusCode, body: &str) -> bool {
        status.as_u16() == 409 || body.trim().eq_ignore_ascii_case("log-in expired")
    }

    /// GET inventory with `?accountId=` / `&nonce=`; tries each endpoint in order.
    ///
    /// Non-verbose mode prints the endpoint only (not the full URL with nonce).
    pub fn download(
        account_id: &str,
        nonce: &str,
        output_file: Option<String>,
        verbose: bool,
    ) -> Result<String> {
        if account_id.is_empty() || nonce.is_empty() {
            anyhow::bail!("Both accountId and nonce are required to download inventory");
        }

        let formatter = OutputFormatter::with_verbose(verbose);
        let authz = format!("?accountId={}&nonce={}", account_id, nonce);

        // Try mobile.warframe.com first, fallback to api.warframe.com
        for (idx, endpoint) in Self::ENDPOINTS.iter().enumerate() {
            let url = format!("{}{}", endpoint, authz);

            formatter.println_colored(
                Colors::YELLOW,
                &format!(
                    "Trying endpoint {} ({}/{})...",
                    endpoint,
                    idx + 1,
                    Self::ENDPOINTS.len()
                ),
            );
            if verbose {
                formatter.println_colored(
                    Colors::YELLOW,
                    &format!("Downloading inventory from {}...", url),
                );
            } else {
                formatter.println_colored(
                    Colors::YELLOW,
                    &format!("Downloading inventory from {}...", endpoint),
                );
            }

            match reqwest::blocking::Client::new()
                .get(&url)
                .header(
                    "User-Agent",
                    format!(
                        "Obsidian-Jackal/warframe-api-helper/{}",
                        env!("CARGO_PKG_VERSION")
                    ),
                )
                .timeout(std::time::Duration::from_secs(30))
                .send()
            {
                Ok(response) => {
                    let status = response.status();
                    let body = response
                        .text()
                        .context("Failed to read response body")?;

                    if Self::is_login_expired(status, &body) {
                        anyhow::bail!(
                            "{}: Warframe log-in expired. The session nonce is no longer \
                             valid. Re-scan while Warframe is running (or pass a fresh \
                             --nonce) and try again.",
                            LOGIN_EXPIRED_PREFIX
                        );
                    }

                    if status.is_success() {
                        // Parse JSON to validate
                        serde_json::from_str::<serde_json::Value>(&body)
                            .context("Received invalid JSON response")?;

                        // Save to file (default to inventory.json if not specified)
                        let output_basename =
                            output_file.unwrap_or_else(|| "inventory.json".to_string());
                        let output_path = resolve_data_path(&output_basename)?;

                        std::fs::write(&output_path, &body).context(format!(
                            "Failed to write {}",
                            output_path.display()
                        ))?;

                        formatter.println_colored(
                            Colors::GREEN,
                            &format!("Successfully downloaded from {}", endpoint),
                        );
                        formatter.println_colored(
                            Colors::GREEN,
                            &format!("Inventory data saved to: {}", output_path.display()),
                        );

                        return Ok(body);
                    } else if idx < Self::ENDPOINTS.len() - 1 {
                        formatter.println_colored(
                            Colors::YELLOW,
                            &format!(
                                "Endpoint {} returned status {}, trying next endpoint ({})...",
                                endpoint,
                                status,
                                Self::ENDPOINTS[idx + 1]
                            ),
                        );
                        continue;
                    } else {
                        let detail = body.trim();
                        if detail.is_empty() {
                            anyhow::bail!("HTTP request failed with status: {}", status);
                        }
                        anyhow::bail!(
                            "HTTP request failed with status {}: {}",
                            status,
                            detail
                        );
                    }
                }
                Err(error) => {
                    if idx < Self::ENDPOINTS.len() - 1 {
                        formatter.println_colored(
                            Colors::YELLOW,
                            &format!(
                                "Request to {} failed: {}. Trying next endpoint ({})...",
                                endpoint,
                                error,
                                Self::ENDPOINTS[idx + 1]
                            ),
                        );
                        continue;
                    } else {
                        return Err(error).context("HTTP request failed");
                    }
                }
            }
        }

        anyhow::bail!("Failed to download inventory from all endpoints");
    }
}
