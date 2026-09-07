//! Regex helpers for `accountId` / `nonce` extraction from match strings.

use regex::Regex;
use crate::types::Match;

pub struct PatternMatcher;

impl PatternMatcher {
    /// Pattern to search for: "?accountId="
    pub const PATTERN: &'static [u8] = b"?accountId=";

    /// Regex pattern for accountId (24 hex characters)
    pub fn account_id_pattern() -> Regex {
        Regex::new(r"\?accountId=([0-9a-fA-F]{24})").unwrap()
    }

    /// Regex pattern for nonce (numeric string)
    pub fn nonce_pattern() -> Regex {
        Regex::new(r"&nonce=([0-9]+)").unwrap()
    }

    /// Extract accountId and nonce from a match string
    pub fn extract_account_id_and_nonce(match_string: &str) -> (Option<String>, Option<String>) {
        let account_id_re = Self::account_id_pattern();
        let nonce_re = Self::nonce_pattern();

        let account_id = account_id_re
            .captures(match_string)
            .and_then(|caps| caps.get(1))
            .map(|matched| matched.as_str().to_string());

        let nonce = nonce_re
            .captures(match_string)
            .and_then(|caps| caps.get(1))
            .map(|matched| matched.as_str().to_string());

        (account_id, nonce)
    }

    /// Check if match string contains both ?accountId= and &nonce=
    pub fn has_both_patterns(match_string: &str) -> bool {
        match_string.contains("?accountId=") && match_string.contains("&nonce=")
    }

    /// Find the best match from a list of matches, preferring 24-character accountId
    pub fn find_best_match(matches: &[Match]) -> (Option<String>, Option<String>) {
        const ACCOUNT_ID_LENGTH: usize = 24;

        let mut best_account_id = None;
        let mut best_nonce = None;

        for match_item in matches {
            let (account_id, nonce) = Self::extract_account_id_and_nonce(&match_item.string);

            if let (Some(acc_id), Some(_nonce_value)) = (&account_id, &nonce) {
                if acc_id.len() == ACCOUNT_ID_LENGTH {
                    return (account_id, nonce);
                } else if best_account_id.is_none() {
                    best_account_id = account_id;
                    best_nonce = nonce;
                }
            }
        }

        (best_account_id, best_nonce)
    }

    /// Extract a complete string from data starting at position
    pub fn extract_string_from_data(data: &[u8], pattern_bytes: &[u8], pos: usize) -> String {
        let string_start = pos;
        let mut string_end = pos + pattern_bytes.len();

        while string_end < data.len()
            && data[string_end] != 0
            && (32 <= data[string_end] && data[string_end] <= 126
                || data[string_end] == 9
                || data[string_end] == 10
                || data[string_end] == 13)
        {
            string_end += 1;
        }

        String::from_utf8_lossy(&data[string_start..string_end]).to_string()
    }
}
