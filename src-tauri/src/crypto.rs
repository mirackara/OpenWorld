use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::hkdf;
use ring::rand::{SecureRandom, SystemRandom};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

const HKDF_INFO: &[u8] = b"openworld-encryption-key";

pub struct CryptoEngine {
    key: LessSafeKey,
    rng: SystemRandom,
}

impl CryptoEngine {
    pub fn new(master_secret: &[u8]) -> Result<Self, String> {
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"openworld-salt");
        let prk = salt.extract(master_secret);
        let okm = prk
            .expand(&[HKDF_INFO], &aead::AES_256_GCM)
            .map_err(|e| format!("HKDF expand failed: {}", e))?;

        let mut key_bytes = [0u8; 32];
        okm.fill(&mut key_bytes)
            .map_err(|e| format!("HKDF fill failed: {}", e))?;

        let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
            .map_err(|e| format!("Key creation failed: {}", e))?;
        let key = LessSafeKey::new(unbound_key);

        Ok(Self {
            key,
            rng: SystemRandom::new(),
        })
    }

    /// Encrypt plaintext, returns base64-encoded "nonce:ciphertext"
    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        let mut nonce_bytes = [0u8; 12];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|e| format!("RNG failed: {}", e))?;

        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut in_out = plaintext.as_bytes().to_vec();

        self.key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        let nonce_b64 = BASE64.encode(nonce_bytes);
        let ciphertext_b64 = BASE64.encode(&in_out);

        Ok(format!("{}:{}", nonce_b64, ciphertext_b64))
    }

    /// Decrypt base64-encoded "nonce:ciphertext" back to plaintext
    pub fn decrypt(&self, encrypted: &str) -> Result<String, String> {
        let parts: Vec<&str> = encrypted.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err("Invalid encrypted format".to_string());
        }

        let nonce_bytes = BASE64
            .decode(parts[0])
            .map_err(|e| format!("Nonce decode failed: {}", e))?;
        let mut ciphertext = BASE64
            .decode(parts[1])
            .map_err(|e| format!("Ciphertext decode failed: {}", e))?;

        if nonce_bytes.len() != 12 {
            return Err("Invalid nonce length".to_string());
        }

        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(&nonce_bytes);
        let nonce = Nonce::assume_unique_for_key(nonce_arr);

        let plaintext = self
            .key
            .open_in_place(nonce, Aad::empty(), &mut ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext.to_vec())
            .map_err(|e| format!("UTF-8 decode failed: {}", e))
    }
}

/// Get or create a machine-unique secret for key derivation.
/// In production this would use the OS keychain; for MVP we use a file-based approach.
pub fn get_or_create_master_secret(data_dir: &std::path::Path) -> Result<Vec<u8>, String> {
    let key_file = data_dir.join(".keyfile");

    if key_file.exists() {
        std::fs::read(&key_file).map_err(|e| format!("Failed to read keyfile: {}", e))
    } else {
        let rng = SystemRandom::new();
        let mut secret = vec![0u8; 32];
        rng.fill(&mut secret)
            .map_err(|e| format!("RNG failed: {}", e))?;
        std::fs::write(&key_file, &secret)
            .map_err(|e| format!("Failed to write keyfile: {}", e))?;
        Ok(secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let engine = CryptoEngine::new(b"test-secret-key-material").unwrap();
        let plaintext = "Hello, OpenWorld! 🌍";
        let encrypted = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_nonces() {
        let engine = CryptoEngine::new(b"test-secret-key-material").unwrap();
        let e1 = engine.encrypt("same text").unwrap();
        let e2 = engine.encrypt("same text").unwrap();
        assert_ne!(e1, e2); // Different nonces = different ciphertexts
    }
}
