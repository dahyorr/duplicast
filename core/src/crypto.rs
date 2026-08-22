//! Encrypts relay stream keys at rest using a key derived from this machine's
//! ID, so `duplicast_relays.json` doesn't contain plaintext stream keys. This
//! protects against casual exposure (accidental commit, backup, disk browsing)
//! on a single-user machine - it is not a defense against an attacker who
//! already has full access to that same machine (which could also just read
//! the running process's memory).

// aes-gcm 0.10 pins an older generic-array whose `from_slice` is soft-deprecated
// pending an ecosystem-wide move to generic-array 1.x - not something this crate
// can fix on its own without bumping aes-gcm to a version with a churnier API.
#![allow(deprecated)]

use aes_gcm::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

const APP_SALT: &[u8] = b"duplicast-relay-key-v1";
const NONCE_LEN: usize = 12;

fn derive_key() -> [u8; 32] {
    let machine_id = machine_uid::get().unwrap_or_else(|_| "duplicast-fallback-machine-id".to_string());
    let mut hasher = Sha256::new();
    hasher.update(machine_id.as_bytes());
    hasher.update(APP_SALT);
    hasher.finalize().into()
}

/// Encrypts a plaintext stream key for storage on disk. Empty strings are left
/// as empty strings (no point encrypting "no key").
pub fn encrypt(plaintext: &str) -> String {
    if plaintext.is_empty() {
        return String::new();
    }

    let key = derive_key();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("AES-GCM encryption cannot fail for valid inputs");

    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    BASE64.encode(combined)
}

/// Decrypts a stream key previously encrypted with `encrypt`. Returns an empty
/// string for empty input, and an error if the ciphertext is malformed or the
/// key doesn't match (e.g. the file was copied from a different machine).
pub fn decrypt(encoded: &str) -> anyhow::Result<String> {
    if encoded.is_empty() {
        return Ok(String::new());
    }

    let combined = BASE64.decode(encoded)?;
    if combined.len() < NONCE_LEN {
        anyhow::bail!("encrypted stream key is too short to contain a nonce");
    }
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_LEN);

    let key = derive_key();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("failed to decrypt stream key (wrong machine, or corrupted file?)"))?;

    Ok(String::from_utf8(plaintext)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_stream_key() {
        let original = "my-secret-stream-key-123";
        let encrypted = encrypt(original);
        assert_ne!(encrypted, original);
        assert_eq!(decrypt(&encrypted).unwrap(), original);
    }

    #[test]
    fn empty_key_stays_empty() {
        assert_eq!(encrypt(""), "");
        assert_eq!(decrypt("").unwrap(), "");
    }

    #[test]
    fn two_encryptions_of_the_same_key_differ() {
        // Different random nonce each time, even for identical plaintext.
        let a = encrypt("same-key");
        let b = encrypt("same-key");
        assert_ne!(a, b);
        assert_eq!(decrypt(&a).unwrap(), "same-key");
        assert_eq!(decrypt(&b).unwrap(), "same-key");
    }

    #[test]
    fn rejects_corrupted_ciphertext() {
        let mut encrypted = encrypt("my-secret-stream-key-123").into_bytes();
        // Flip a byte in the middle of the base64 payload.
        let mid = encrypted.len() / 2;
        encrypted[mid] = if encrypted[mid] == b'A' { b'B' } else { b'A' };
        let corrupted = String::from_utf8(encrypted).unwrap();
        assert!(decrypt(&corrupted).is_err());
    }
}
