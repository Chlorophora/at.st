use crate::errors::ServiceError;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use once_cell::sync::Lazy;

// 警告: このキーは絶対に秘密にし、32バイト（256ビット）である必要があります。
// 環境変数やシークレットマネージャーなど、安全な場所から読み込むようにしてください。
static ENCRYPTION_KEY: Lazy<[u8; 32]> = Lazy::new(|| {
    let key_hex = std::env::var("ENCRYPTION_KEY")
        .expect("ENCRYPTION_KEY must be set in environment variables");
    let mut key = [0u8; 32];
    hex::decode_to_slice(&key_hex, &mut key)
        .expect("ENCRYPTION_KEY must be a 64-character hex string");
    key
});

pub fn encrypt(plaintext: &str) -> Result<Vec<u8>, ServiceError> {
    let cipher = Aes256Gcm::new_from_slice(&*ENCRYPTION_KEY).unwrap();
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bit nonce
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| ServiceError::InternalServerError("Encryption failed".to_string()))?;

    // Nonceを暗号文の先頭に結合して保存します。復号時に必要になります。
    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt(encrypted_data: &[u8]) -> Result<String, ServiceError> {
    if encrypted_data.len() < 12 {
        return Err(ServiceError::InternalServerError(
            "Invalid encrypted data format".to_string(),
        ));
    }
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(&*ENCRYPTION_KEY).unwrap();

    let decrypted_bytes = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        ServiceError::InternalServerError(
            "Decryption failed. Data may be corrupt or key is incorrect.".to_string(),
        )
    })?;

    String::from_utf8(decrypted_bytes).map_err(|_| {
        ServiceError::InternalServerError("Failed to convert decrypted data to string".to_string())
    })
}
