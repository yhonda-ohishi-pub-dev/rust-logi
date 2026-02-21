use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};

/// Encrypt a secret using AES-256-GCM with the given key
pub fn encrypt_secret(plaintext: &str, key_material: &str) -> Result<String, String> {
    let key_bytes = derive_key(key_material);
    let unbound_key =
        UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|e| format!("Key error: {}", e))?;
    let key = LessSafeKey::new(unbound_key);

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| format!("RNG error: {}", e))?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|e| format!("Encryption error: {}", e))?;

    // Format: base64(nonce + ciphertext + tag)
    let mut result = Vec::with_capacity(12 + in_out.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&in_out);
    Ok(BASE64.encode(&result))
}

/// Decrypt a secret using AES-256-GCM with the given key
pub fn decrypt_secret(ciphertext_b64: &str, key_material: &str) -> Result<String, String> {
    let key_bytes = derive_key(key_material);
    let unbound_key =
        UnboundKey::new(&AES_256_GCM, &key_bytes).map_err(|e| format!("Key error: {}", e))?;
    let key = LessSafeKey::new(unbound_key);

    let data = BASE64
        .decode(ciphertext_b64)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    if data.len() < 12 + aead::AES_256_GCM.tag_len() {
        return Err("Ciphertext too short".to_string());
    }

    let (nonce_bytes, ciphertext_and_tag) = data.split_at(12);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes.try_into().unwrap());

    let mut in_out = ciphertext_and_tag.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::empty(), &mut in_out)
        .map_err(|e| format!("Decryption error: {}", e))?;

    String::from_utf8(plaintext.to_vec()).map_err(|e| format!("UTF-8 error: {}", e))
}

/// Derive a 256-bit key from arbitrary key material using SHA-256
fn derive_key(key_material: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key_material.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "my-client-secret-123";
        let key = "jwt-secret-key";

        let encrypted = encrypt_secret(secret, key).unwrap();
        let decrypted = decrypt_secret(&encrypted, key).unwrap();

        assert_eq!(secret, decrypted);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let secret = "my-client-secret-123";
        let encrypted = encrypt_secret(secret, "correct-key").unwrap();
        let result = decrypt_secret(&encrypted, "wrong-key");
        assert!(result.is_err());
    }
}
