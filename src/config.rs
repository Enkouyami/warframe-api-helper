//! Runtime options for download/save (`--password`, `-o`).

pub use crate::crypto::EncryptionKey;

/// Options shared by download and `lastData` / JSON writers.
#[derive(Debug, Clone)]
pub struct Config {
    /// Key for encrypting/decrypting `lastData*.dat`.
    pub encryption_key: EncryptionKey,
    /// Keep `accountId` in the written `inventory*.json` (default: strip it).
    pub original_format: bool,
}
