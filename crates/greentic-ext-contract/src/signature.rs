use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signature, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::error::ContractError;

/// Compute SHA256 of artifact bytes as hex string.
#[must_use]
pub fn artifact_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn sign_ed25519(key: &SigningKey, payload: &[u8]) -> String {
    use ed25519_dalek::Signer;
    let sig: Signature = key.sign(payload);
    B64.encode(sig.to_bytes())
}

pub fn verify_ed25519(
    public_key_b64: &str,
    signature_b64: &str,
    payload: &[u8],
) -> Result<(), ContractError> {
    let public_key_bytes = B64
        .decode(strip_prefix(public_key_b64))
        .map_err(|e| ContractError::SignatureInvalid(format!("pubkey b64: {e}")))?;
    let sig_bytes = B64
        .decode(signature_b64)
        .map_err(|e| ContractError::SignatureInvalid(format!("sig b64: {e}")))?;
    let public_key_array: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::SignatureInvalid("pubkey length != 32".into()))?;
    let sig_array: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::SignatureInvalid("sig length != 64".into()))?;
    let key = VerifyingKey::from_bytes(&public_key_array)
        .map_err(|e| ContractError::SignatureInvalid(format!("pubkey parse: {e}")))?;
    let signature = Signature::from_bytes(&sig_array);
    key.verify(payload, &signature)
        .map_err(|e| ContractError::SignatureInvalid(format!("verify: {e}")))
}

fn strip_prefix(s: &str) -> &str {
    s.strip_prefix("ed25519:").unwrap_or(s)
}
