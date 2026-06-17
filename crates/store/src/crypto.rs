use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;

const ENC_PREFIX: &str = "$enc$";
const NONCE_LEN: usize = 12;

/// AES-256-GCM cipher used to encrypt `body` and `previous_body` columns at rest.
///
/// Encrypted values are stored as `$enc$<base64(nonce_12bytes || ciphertext)>`.
/// Plaintext values pass through unchanged, so existing databases work without a key.
pub struct Cipher {
    inner: Aes256Gcm,
}

impl Cipher {
    /// Construct from a 64-character lowercase hex string (32 bytes).
    pub fn from_hex(hex: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            hex.len() == 64,
            "SDM_ENCRYPTION_KEY must be exactly 64 hex characters (32 bytes)"
        );
        let mut bytes = [0u8; 32];
        for (i, pair) in hex.as_bytes().chunks(2).enumerate() {
            let s = std::str::from_utf8(pair)
                .map_err(|_| anyhow::anyhow!("non-UTF8 in SDM_ENCRYPTION_KEY"))?;
            bytes[i] = u8::from_str_radix(s, 16)
                .map_err(|_| anyhow::anyhow!("invalid hex char in SDM_ENCRYPTION_KEY: {s}"))?;
        }
        Ok(Self {
            inner: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&bytes)),
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> anyhow::Result<String> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .inner
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encryption error: {e}"))?;
        let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&ciphertext);
        Ok(format!("{}{}", ENC_PREFIX, B64.encode(&payload)))
    }

    /// Decrypt a value that may or may not be encrypted.
    /// Returns the original string unchanged if it lacks the `$enc$` prefix
    /// (backwards-compatible with plaintext data stored before a key was set).
    pub fn decrypt(&self, text: &str) -> anyhow::Result<String> {
        if !text.starts_with(ENC_PREFIX) {
            return Ok(text.to_string());
        }
        let b64_part = &text[ENC_PREFIX.len()..];
        let payload = B64
            .decode(b64_part)
            .map_err(|e| anyhow::anyhow!("base64 decode error: {e}"))?;
        anyhow::ensure!(payload.len() > NONCE_LEN, "encrypted payload too short");
        let nonce = Nonce::from_slice(&payload[..NONCE_LEN]);
        let plaintext = self
            .inner
            .decrypt(nonce, &payload[NONCE_LEN..])
            .map_err(|e| anyhow::anyhow!("decryption error: {e}"))?;
        String::from_utf8(plaintext).map_err(Into::into)
    }

    pub fn maybe_encrypt(cipher: Option<&Self>, text: &str) -> anyhow::Result<String> {
        match cipher {
            Some(c) => c.encrypt(text),
            None => Ok(text.to_string()),
        }
    }

    pub fn maybe_decrypt(cipher: Option<&Self>, text: &str) -> anyhow::Result<String> {
        match cipher {
            Some(c) => c.decrypt(text),
            None => Ok(text.to_string()),
        }
    }
}
