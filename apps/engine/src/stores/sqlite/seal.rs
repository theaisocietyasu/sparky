//! XChaCha20-Poly1305 sealing of sensitive memory text.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

/// Length of an XChaCha20 nonce in bytes.
const NONCE_BYTES: usize = 24;

/// Seals and opens memory text with one key.
#[derive(Clone)]
pub struct Seal {
    cipher: XChaCha20Poly1305,
}

impl Seal {
    /// A seal over a 32-byte key.
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(&Key::from(*key)),
        }
    }

    /// The nonce and the sealed bytes of text, or None if the cipher refused.
    pub fn seal(&self, text: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let sealed = self.cipher.encrypt(&nonce, text.as_bytes()).ok()?;
        Some((nonce.to_vec(), sealed))
    }

    /// The text sealed under nonce, or None if the nonce, the key or the bytes do not match.
    pub fn open(&self, nonce: &[u8], sealed: &[u8]) -> Option<String> {
        let nonce: [u8; NONCE_BYTES] = nonce.try_into().ok()?;
        let plain = self.cipher.decrypt(&XNonce::from(nonce), sealed).ok()?;
        String::from_utf8(plain).ok()
    }
}
