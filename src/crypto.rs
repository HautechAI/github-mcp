use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use std::sync::OnceLock;

pub trait SealedBoxEncryptor: Send + Sync {
    fn encrypt(&self, plaintext: &str, b64_public_key: &str) -> Result<String, String>;
}

struct SodiumEncryptor;

impl SealedBoxEncryptor for SodiumEncryptor {
    fn encrypt(&self, plaintext: &str, b64_public_key: &str) -> Result<String, String> {
        // Decode base64 public key (32 bytes)
        let key_bytes = B64
            .decode(b64_public_key.as_bytes())
            .map_err(|e| format!("invalid base64 key: {}", e))?;
        if key_bytes.len() != 32 {
            return Err("invalid public key length".into());
        }
        // sodiumoxide init is idempotent
        let _ = sodiumoxide::init();
        let pk = sodiumoxide::crypto::box_::PublicKey::from_slice(&key_bytes)
            .ok_or_else(|| "invalid public key".to_string())?;
        let cipher = sodiumoxide::crypto::sealedbox::seal(plaintext.as_bytes(), &pk);
        Ok(B64.encode(cipher))
    }
}

static ENCRYPTOR: OnceLock<Box<dyn SealedBoxEncryptor>> = OnceLock::new();

fn get_encryptor() -> &'static dyn SealedBoxEncryptor {
    ENCRYPTOR
        .get_or_init(|| Box::new(SodiumEncryptor) as Box<dyn SealedBoxEncryptor>)
        .as_ref()
}

pub fn encrypt_sealed_box(plaintext: &str, b64_public_key: &str) -> Result<String, String> {
    get_encryptor().encrypt(plaintext, b64_public_key)
}

#[cfg(test)]
pub fn set_encryptor_for_test(enc: Box<dyn SealedBoxEncryptor>) {
    let _ = ENCRYPTOR.set(enc);
}
