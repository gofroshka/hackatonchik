//! AES-256-GCM encryption/decryption for the acoustic link.
//!
//! Two API levels:
//!
//! **High-level** (`encrypt` / `decrypt`): nonce is automatically generated and
//! prepended to the ciphertext.  Simple round-trip:
//!
//! ```
//! let key = sonic_share_core::crypto::generate_key();
//! let ciphertext = sonic_share_core::crypto::encrypt(b"hello", &key).unwrap();
//! let plaintext  = sonic_share_core::crypto::decrypt(&ciphertext, &key).unwrap();
//! assert_eq!(&plaintext, b"hello");
//! ```
//!
//! **Low-level** (`encrypt_with_nonce` / `decrypt_with_nonce`): caller manages
//! the nonce (e.g. as a counter from the handshake).  Nonce is NOT embedded.
//!
//! ```
//! let key    = sonic_share_core::crypto::generate_key();
//! let nonce  = sonic_share_core::crypto::generate_nonce();
//! let ct     = sonic_share_core::crypto::encrypt_with_nonce(b"data", &key, &nonce).unwrap();
//! let pt     = sonic_share_core::crypto::decrypt_with_nonce(&ct, &key, &nonce).unwrap();
//! assert_eq!(&pt, b"data");
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};
use x25519_dalek::{EphemeralSecret, PublicKey};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum CryptoError {
    EncryptionFailed,
    DecryptionFailed,
    InvalidKeyLength,
    InvalidNonceLength,
    TruncatedCiphertext,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EncryptionFailed => write!(f, "AES-256-GCM encryption failed"),
            Self::DecryptionFailed => write!(f, "AES-256-GCM decryption failed"),
            Self::InvalidKeyLength => write!(f, "key must be exactly 32 bytes"),
            Self::InvalidNonceLength => write!(f, "nonce must be exactly 12 bytes"),
            Self::TruncatedCiphertext => write!(f, "ciphertext is too short (missing nonce or tag)"),
        }
    }
}

impl std::error::Error for CryptoError {}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// AES-256 key length in bytes.
pub const KEY_LEN: usize = 32;

/// AES-256-GCM nonce length in bytes.
pub const NONCE_LEN: usize = 12;

/// AES-256-GCM authentication tag length in bytes.
pub const TAG_LEN: usize = 16;

/// Overhead of the high-level format: nonce + tag.
pub const OVERHEAD: usize = NONCE_LEN + TAG_LEN;

// ---------------------------------------------------------------------------
// Key and nonce generation
// ---------------------------------------------------------------------------

/// Generate a random 256-bit key suitable for AES-256-GCM.
pub fn generate_key() -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut key);
    key
}

/// Generate a random 12-byte nonce for AES-256-GCM.
pub fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    nonce
}

// ---------------------------------------------------------------------------
// High-level API (nonce embedded in ciphertext)
// ---------------------------------------------------------------------------

/// Encrypt `plaintext` with `key`.
///
/// Returns `[nonce (12 bytes)] [ciphertext] [tag (16 bytes)]`.
///
/// Use [`decrypt`] to recover the plaintext.
pub fn encrypt(plaintext: &[u8], key: &[u8; KEY_LEN]) -> Result<Vec<u8>, CryptoError> {
    let nonce = generate_nonce();
    let mut output = encrypt_with_nonce(plaintext, key, &nonce)?;
    let mut result = Vec::with_capacity(NONCE_LEN + output.len());
    result.extend_from_slice(&nonce);
    result.append(&mut output);
    Ok(result)
}

/// Decrypt a ciphertext previously produced by [`encrypt`].
///
/// Expects `[nonce (12)] [ciphertext + tag]`.
pub fn decrypt(data: &[u8], key: &[u8; KEY_LEN]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < OVERHEAD {
        return Err(CryptoError::TruncatedCiphertext);
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce_bytes
        .try_into()
        .map_err(|_| CryptoError::InvalidNonceLength)?;
    decrypt_with_nonce(ciphertext, key, &nonce)
}

// ---------------------------------------------------------------------------
// Low-level API (caller manages nonce)
// ---------------------------------------------------------------------------

/// Encrypt `plaintext` with `key` and `nonce`.
///
/// Returns `[ciphertext] [tag (16 bytes)]` — the nonce is **not** embedded.
pub fn encrypt_with_nonce(
    plaintext: &[u8],
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?;
    let nonce = Nonce::from_slice(nonce);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)
}

/// Decrypt a ciphertext previously produced by [`encrypt_with_nonce`].
///
/// Expects `[ciphertext + tag]` without embedded nonce.
pub fn decrypt_with_nonce(
    data: &[u8],
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::InvalidKeyLength)?;
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, data)
        .map_err(|_| CryptoError::DecryptionFailed)
}

// ---------------------------------------------------------------------------
// X25519 key agreement
// ---------------------------------------------------------------------------

/// An X25519 ephemeral keypair for Diffie-Hellman key exchange.
pub struct Keypair {
    /// 32-byte X25519 public key.
    pub public: [u8; 32],
    secret: EphemeralSecret,
}

/// Generate a new ephemeral X25519 keypair.
///
/// The secret is consumed by [`derive_session_key`] and cannot be reused.
pub fn generate_keypair() -> Keypair {
    let secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
    let public = PublicKey::from(&secret);
    Keypair {
        public: public.to_bytes(),
        secret,
    }
}

/// Perform X25519 Diffie-Hellman and derive a 32-byte AES-256-GCM session key.
///
/// Takes ownership of `keypair` (the secret is consumed by the DH operation).
pub fn derive_session_key(
    keypair: Keypair,
    peer_public: &[u8; KEY_LEN],
) -> [u8; KEY_LEN] {
    let peer_pk = PublicKey::from(*peer_public);
    let shared = keypair.secret.diffie_hellman(&peer_pk);
    let shared_bytes = shared.to_bytes();
    let mut hasher = Sha256::new();
    hasher.update(shared_bytes);
    hasher.update(b"sonic-share-session-key-v1");
    let result: [u8; KEY_LEN] = hasher.finalize().into();
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_level_roundtrip() {
        let key = generate_key();
        let plaintext = b"Hello, Sonic Share!";
        let ciphertext = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn high_level_overhead_is_correct() {
        let key = generate_key();
        let plaintext = b"abcdefghij";
        let ciphertext = encrypt(plaintext, &key).unwrap();
        assert_eq!(ciphertext.len(), plaintext.len() + OVERHEAD);
    }

    #[test]
    fn high_level_empty_input() {
        let key = generate_key();
        let ciphertext = encrypt(b"", &key).unwrap();
        assert_eq!(ciphertext.len(), OVERHEAD);
        let decrypted = decrypt(&ciphertext, &key).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn low_level_roundtrip() {
        let key = generate_key();
        let nonce = generate_nonce();
        let plaintext = b"low-level API";
        let ct = encrypt_with_nonce(plaintext, &key, &nonce).unwrap();
        assert_eq!(ct.len(), plaintext.len() + TAG_LEN);
        let pt = decrypt_with_nonce(&ct, &key, &nonce).unwrap();
        assert_eq!(&pt, plaintext);
    }

    #[test]
    fn decryption_fails_with_wrong_key() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = b"secret data";
        let ciphertext = encrypt(plaintext, &key1).unwrap();
        assert!(decrypt(&ciphertext, &key2).is_err());
    }

    #[test]
    fn decryption_fails_with_wrong_nonce() {
        let key = generate_key();
        let nonce1 = generate_nonce();
        let nonce2 = generate_nonce();
        let plaintext = b"secret data";
        let ct = encrypt_with_nonce(plaintext, &key, &nonce1).unwrap();
        assert!(decrypt_with_nonce(&ct, &key, &nonce2).is_err());
    }

    #[test]
    fn decryption_fails_on_tampered_ciphertext() {
        let key = generate_key();
        let mut ciphertext = encrypt(b"tamper me", &key).unwrap();
        ciphertext[NONCE_LEN + 1] ^= 0xFF; // flip a bit in the ciphertext
        assert!(decrypt(&ciphertext, &key).is_err());
    }

    #[test]
    fn decryption_fails_on_truncated_input() {
        let key = generate_key();
        assert!(decrypt(b"", &key).is_err());
        assert!(decrypt(&[0u8; OVERHEAD - 1], &key).is_err());
    }

    #[test]
    fn key_generation_produces_32_bytes() {
        let key = generate_key();
        assert_eq!(key.len(), KEY_LEN);
    }

    #[test]
    fn nonce_generation_produces_12_bytes() {
        let nonce = generate_nonce();
        assert_eq!(nonce.len(), NONCE_LEN);
    }

    #[test]
    fn different_keys_are_different() {
        let key1 = generate_key();
        let key2 = generate_key();
        assert_ne!(key1, key2);
    }

    #[test]
    fn encrypt_produces_different_ciphertexts_each_time() {
        let key = generate_key();
        let plaintext = b"same data";
        let c1 = encrypt(plaintext, &key).unwrap();
        let c2 = encrypt(plaintext, &key).unwrap();
        // Different nonces -> different outputs
        assert_ne!(c1, c2);
        // Both should decrypt correctly
        assert_eq!(decrypt(&c1, &key).unwrap(), plaintext);
        assert_eq!(decrypt(&c2, &key).unwrap(), plaintext);
    }

    #[test]
    fn large_data_roundtrip() {
        let key = generate_key();
        let plaintext = vec![0xABu8; 100_000];
        let ciphertext = encrypt(&plaintext, &key).unwrap();
        assert_eq!(
            ciphertext.len(),
            plaintext.len() + OVERHEAD
        );
        let decrypted = decrypt(&ciphertext, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn x25519_keypair_public_is_32_bytes() {
        let kp = generate_keypair();
        assert_eq!(kp.public.len(), KEY_LEN);
    }

    #[test]
    fn x25519_shared_secret_matches_on_both_sides() {
        let alice = generate_keypair();
        let bob = generate_keypair();
        let alice_pk = alice.public;
        let bob_pk = bob.public;
        let alice_key = derive_session_key(alice, &bob_pk);
        let bob_key = derive_session_key(bob, &alice_pk);
        assert_eq!(alice_key, bob_key);
    }

    #[test]
    fn x25519_different_keys_produce_different_secrets() {
        let alice = generate_keypair();
        let bob = generate_keypair();
        let eve = generate_keypair();
        let _alice_pk = alice.public;
        let bob_pk = bob.public;
        let eve_pk = eve.public;
        let alice_bob = derive_session_key(alice, &bob_pk);
        let alice_eve = derive_session_key(bob, &eve_pk);
        assert_ne!(alice_bob, alice_eve);
    }

    #[test]
    fn x25519_derived_key_works_with_aes_gcm() {
        let alice = generate_keypair();
        let bob = generate_keypair();
        let alice_pk = alice.public;
        let bob_pk = bob.public;
        let alice_key = derive_session_key(alice, &bob_pk);
        let bob_key = derive_session_key(bob, &alice_pk);
        assert_eq!(alice_key, bob_key);
        let plaintext = b"encrypted after DH key exchange!";
        let ct = encrypt(plaintext, &alice_key).unwrap();
        let pt = decrypt(&ct, &bob_key).unwrap();
        assert_eq!(&pt, plaintext);
    }
}
