//! AES-128-CBC for `lastData.dat` (default key or `--password`).

use anyhow::{Context, Result};
use aes::Aes128;
use cbc::{Decryptor, Encryptor};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use generic_array::GenericArray;

/// AES-CBC IV from Sainan/warframe-api-helper `main.cpp`.
const IV: [u8; 16] = [49, 50, 70, 71, 66, 51, 54, 45, 76, 69, 51, 45, 113, 61, 57, 0];

/// Default AES key from Sainan/warframe-api-helper `main.cpp`
/// (`LEO-ALEC\tEO-ALEC`). Same bytes in calamity-inc/browse.wf `inventory.php`
/// for AlecaFrame `lastData.dat`.
pub const DEFAULT_KEY: [u8; 16] = [
    76, 69, 79, 45, 65, 76, 69, 67, 9, 69, 79, 45, 65, 76, 69, 67,
];

/// 16-byte AES key used for `lastData.dat`.
#[derive(Debug, Clone)]
pub struct EncryptionKey([u8; 16]);

impl EncryptionKey {
    pub fn from_bytes(key: [u8; 16]) -> Self {
        Self(key)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Derive a 16-byte key from a password (Argon2id, fixed salt `WarframeAPIHelper`).
pub fn derive_key_from_password(password: &str) -> Result<[u8; 16]> {
    use argon2::{Argon2, Algorithm, Version, Params};

    // Argon2id: 64 MiB, 3 iterations, parallelism 4; fixed salt for stable keys.
    let params = Params::new(65536, 3, 4, None)
        .map_err(|error| anyhow::anyhow!("Failed to create Argon2 parameters: {:?}", error))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let salt_str = "WarframeAPIHelper";
    let mut salt = [0u8; 16];
    let salt_len = salt_str.len().min(16);
    salt[..salt_len].copy_from_slice(&salt_str.as_bytes()[..salt_len]);

    let mut output = [0u8; 16];
    argon2.hash_password_into(password.as_bytes(), &salt, &mut output)
        .map_err(|error| anyhow::anyhow!("Failed to hash password with Argon2: {:?}", error))?;

    Ok(output)
}

/// Encrypt UTF-8 plaintext with AES-128-CBC + PKCS7 (same layout as the C++ helper).
pub fn encrypt_data(data: &str, key: &[u8; 16]) -> Result<Vec<u8>> {
    let data_bytes = data.as_bytes();
    let data_len = data_bytes.len();

    let key_array = GenericArray::from_slice(key);
    let iv_array = GenericArray::from_slice(&IV);
    let encryptor = Encryptor::<Aes128>::new(key_array, iv_array);

    let block_size = 16;
    let padded_size = ((data_len + block_size - 1) / block_size) * block_size;
    let padded_size = if data_len % block_size == 0 {
        data_len + block_size
    } else {
        padded_size
    };
    let mut buffer = vec![0u8; padded_size];
    buffer[..data_len].copy_from_slice(data_bytes);

    let encrypted = encryptor.encrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(
        &mut buffer,
        data_len
    ).map_err(|error| anyhow::anyhow!("Encryption failed: {:?}", error))?;

    Ok(encrypted.to_vec())
}

/// Decrypt AES-128-CBC + PKCS7 ciphertext to UTF-8.
pub fn decrypt_data(data: &[u8], key: &[u8; 16]) -> Result<String> {
    let key_array = GenericArray::from_slice(key);
    let iv_array = GenericArray::from_slice(&IV);
    let decryptor = Decryptor::<Aes128>::new(key_array, iv_array);

    let mut data_mut = data.to_vec();
    let decrypted = decryptor.decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut data_mut)
        .map_err(|error| anyhow::anyhow!("Decryption failed: {:?}", error))?;

    String::from_utf8(decrypted.to_vec())
        .context("Decrypted data is not valid UTF-8")
}
