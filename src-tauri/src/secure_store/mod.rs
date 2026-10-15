//! Authenticated encryption for AI artifacts. No plaintext fallback.

mod envelope;
mod io;
mod key;

#[cfg(test)]
pub(crate) mod testing;

pub(crate) use key::{exclusive_in, with_store_in};

pub fn seal(plaintext: &[u8], aad: &str) -> Result<Vec<u8>, StoreError> {
    with_store_in(&crate::paths::get_app_config_dir(), true, |store| {
        store.seal(plaintext, aad)
    })
}

pub fn open(sealed: &[u8], aad: &str) -> Result<Vec<u8>, StoreError> {
    with_store_in(&crate::paths::get_app_config_dir(), false, |store| {
        store.open(sealed, aad)
    })
}

pub fn write_sealed(path: &Path, plaintext: &[u8], aad: &str) -> Result<(), StoreError> {
    with_store_in(&crate::paths::get_app_config_dir(), true, |store| {
        store.write_sealed(path, plaintext, aad)
    })
}

pub fn read_sealed(path: &Path, aad: &str) -> Result<Vec<u8>, StoreError> {
    with_store_in(&crate::paths::get_app_config_dir(), false, |store| {
        store.read_sealed(path, aad)
    })
}

pub fn seal_line(plaintext: &[u8], aad: &str) -> Result<String, StoreError> {
    with_store_in(&crate::paths::get_app_config_dir(), true, |store| {
        store.seal_line(plaintext, aad)
    })
}

pub fn open_line(line: &str, aad: &str) -> Result<Vec<u8>, StoreError> {
    with_store_in(&crate::paths::get_app_config_dir(), false, |store| {
        store.open_line(line, aad)
    })
}

use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use zeroize::Zeroizing;

use envelope::Envelope;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoreError {
    KeyUnavailable,
    Corrupted,
    WrongContext,
    Io,
    InvalidInput,
    TooLarge,
    UnsupportedVersion,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::KeyUnavailable => "The AI encryption key is unavailable. Unlock the system keychain or reset AI storage in Settings.",
            Self::Corrupted => "The AI data cannot be decrypted. The file or encryption key may have changed.",
            Self::WrongContext => "This encrypted AI file belongs to a different context.",
            Self::Io => "Could not read or write AI storage.",
            Self::InvalidInput => "The AI storage request contains invalid data.",
            Self::TooLarge => "The AI storage item exceeds the size limit.",
            Self::UnsupportedVersion => "This AI data was created by an unsupported version of Tabularis.",
        })
    }
}

impl std::error::Error for StoreError {}
impl From<std::io::Error> for StoreError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

/// Key material has no Debug/Serialize implementation and is zeroized on drop.
#[derive(Clone)]
pub(crate) struct CipherStore {
    key: Arc<Zeroizing<[u8; 32]>>,
}

impl CipherStore {
    pub(crate) fn from_key(key: Zeroizing<[u8; 32]>) -> Self {
        Self { key: Arc::new(key) }
    }

    pub(crate) fn seal(&self, plaintext: &[u8], aad: &str) -> Result<Vec<u8>, StoreError> {
        let cipher = Aes256Gcm::new_from_slice(self.key.as_ref().as_ref())
            .map_err(|_| StoreError::KeyUnavailable)?;
        let mut nonce = [0; 12];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| StoreError::KeyUnavailable)?;
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StoreError::Corrupted)?;
        serde_json::to_vec(&Envelope::new(aad, &nonce, &ct)).map_err(|_| StoreError::Corrupted)
    }

    pub(crate) fn open(&self, sealed: &[u8], aad: &str) -> Result<Vec<u8>, StoreError> {
        let envelope = Envelope::parse(sealed, aad)?;
        let (nonce, ct) = envelope.decode()?;
        let cipher = Aes256Gcm::new_from_slice(self.key.as_ref().as_ref())
            .map_err(|_| StoreError::KeyUnavailable)?;
        cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ct,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| StoreError::Corrupted)
    }

    pub(crate) fn write_sealed(
        &self,
        path: &Path,
        plaintext: &[u8],
        aad: &str,
    ) -> Result<(), StoreError> {
        io::write_private_atomic(path, &self.seal(plaintext, aad)?)
    }

    pub(crate) fn read_sealed(&self, path: &Path, aad: &str) -> Result<Vec<u8>, StoreError> {
        self.open(&std::fs::read(path)?, aad)
    }

    pub(crate) fn seal_line(&self, plaintext: &[u8], aad: &str) -> Result<String, StoreError> {
        Ok(BASE64.encode(self.seal(plaintext, aad)?))
    }

    pub(crate) fn open_line(&self, line: &str, aad: &str) -> Result<Vec<u8>, StoreError> {
        self.open(
            &BASE64.decode(line).map_err(|_| StoreError::Corrupted)?,
            aad,
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests/artifacts.rs"]
mod artifact_tests;
