use super::StoreError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
pub(super) struct Envelope {
    v: u32,
    alg: String,
    /// A digest, never the plaintext context. Distinguishes a misplaced file
    /// from authentication failure; AES-GCM still authenticates the full AAD.
    ctx: String,
    nonce: String,
    ct: String,
}

impl Envelope {
    pub fn new(aad: &str, nonce: &[u8], ct: &[u8]) -> Self {
        Self {
            v: 1,
            alg: "A256GCM".into(),
            ctx: context_digest(aad),
            nonce: BASE64.encode(nonce),
            ct: BASE64.encode(ct),
        }
    }

    pub fn parse(bytes: &[u8], aad: &str) -> Result<Self, StoreError> {
        let value: Self = serde_json::from_slice(bytes).map_err(|_| StoreError::Corrupted)?;
        if value.v != 1 || value.alg != "A256GCM" {
            return Err(StoreError::UnsupportedVersion);
        }
        if value.ctx != context_digest(aad) {
            return Err(StoreError::WrongContext);
        }
        Ok(value)
    }

    pub fn decode(&self) -> Result<([u8; 12], Vec<u8>), StoreError> {
        let nonce: [u8; 12] = BASE64
            .decode(&self.nonce)
            .map_err(|_| StoreError::Corrupted)?
            .try_into()
            .map_err(|_| StoreError::Corrupted)?;
        let ct = BASE64.decode(&self.ct).map_err(|_| StoreError::Corrupted)?;
        if ct.len() < 16 {
            return Err(StoreError::Corrupted);
        }
        Ok((nonce, ct))
    }
}

fn context_digest(aad: &str) -> String {
    BASE64.encode(Sha256::digest(aad.as_bytes()))
}

pub(crate) fn is_sealed(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes).is_ok_and(|value| {
        value.get("alg").is_some() || value.get("ct").is_some() || value.get("nonce").is_some()
    })
}
