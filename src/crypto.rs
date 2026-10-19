use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use rand::RngCore;

/// Encrypts an API key using AES-256-GCM.
/// Generates a random 12-byte nonce and returns `(ciphertext, nonce)`.
#[allow(deprecated)]
pub fn encrypt_api_key(plaintext: &str, key: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new(key.into());

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("encryption failed: {e}"))?;

    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Decrypts an API key using AES-256-GCM with the provided ciphertext, 12-byte nonce, and 32-byte key.
#[allow(deprecated)]
pub fn decrypt_api_key(ciphertext: &[u8], nonce: &[u8], key: &[u8; 32]) -> Result<String, String> {
    if nonce.len() != 12 {
        return Err("invalid nonce length: expected 12 bytes".to_string());
    }

    let nonce = Nonce::from_slice(nonce);
    let cipher = Aes256Gcm::new(key.into());

    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("decryption failed: {e}"))?;

    String::from_utf8(plaintext_bytes).map_err(|e| format!("invalid UTF-8 string: {e}"))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0x42u8; 32];
        let original = "AIzaSyFakeApiKey1234567890";

        let (ciphertext, nonce) = encrypt_api_key(original, &key).expect("encryption should succeed");
        assert_ne!(ciphertext, original.as_bytes());

        let decrypted = decrypt_api_key(&ciphertext, &nonce, &key).expect("decryption should succeed");
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key1 = [0x42u8; 32];
        let key2 = [0x43u8; 32];
        let original = "AIzaSyFakeApiKey1234567890";

        let (ciphertext, nonce) = encrypt_api_key(original, &key1).expect("encryption should succeed");
        let result = decrypt_api_key(&ciphertext, &nonce, &key2);

        assert!(result.is_err(), "decryption with wrong key must fail");
    }

    #[test]
    fn test_unique_nonces() {
        let key = [0x42u8; 32];
        let original = "AIzaSyFakeApiKey1234567890";

        let (ciphertext1, nonce1) = encrypt_api_key(original, &key).expect("encryption 1 should succeed");
        let (ciphertext2, nonce2) = encrypt_api_key(original, &key).expect("encryption 2 should succeed");

        assert_ne!(nonce1, nonce2, "nonces must be unique per encryption");
        assert_ne!(ciphertext1, ciphertext2, "ciphertexts must differ due to unique nonces");
    }
}
