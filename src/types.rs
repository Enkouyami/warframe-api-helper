//! Match found in process memory (`string`, address, region, context bytes).

/// One `?accountId=…&nonce=…` hit from a memory region.
#[derive(Debug, Clone)]
pub struct Match {
    /// The full matched string (e.g., "?accountId=...&nonce=...")
    pub string: String,
    /// Memory address
    pub address: Option<usize>,
    /// Memory region info
    pub region: Option<String>,
    /// Context bytes around the match
    pub context: Vec<u8>,
}

impl Match {
    /// Create a match from process memory
    pub fn from_process_memory(string: String, address: usize, region: String, context: Vec<u8>) -> Self {
        Self {
            string,
            address: Some(address),
            region: Some(region),
            context,
        }
    }
}
