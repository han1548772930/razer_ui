//! Offline half of the current host's `electron/lib/getAppFeature.js`.
//! Read the installed fallback JWS and verify the original Ed25519 key set.
//! This module never fetches flags or treats an unsigned JSON file as evidence.
use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde_json::Value;
use std::path::{Path, PathBuf};

const KEYS: &[(&str, &str)] = &[
    (
        "fflags-Nov2025-key-1",
        "8BWvEkt9VkGv8PKbPTgIpf_Fx3TkmvvcuFCnrWahlZ0",
    ),
    (
        "fflags-Nov2025-key-2",
        "VYXxBFt-mLqFZTZTWAItz_EsKHSKrLR8wwhFeQdjveA",
    ),
    (
        "fflags-Nov2025-key-3",
        "y--rzoeRMhu1Bp5VYChyUsTeXmIKmg2XwI-C3unGiFY",
    ),
];

fn verify_with_keys(compact: &str, keys: &[(&str, &str)]) -> anyhow::Result<Value> {
    let compact = compact.trim();
    ensure!(
        compact.len() <= 1024 * 1024,
        "Local feature settings are too large"
    );
    let parts: Vec<_> = compact.split('.').collect();
    ensure!(
        parts.len() == 3 && parts.iter().all(|part| !part.is_empty()),
        "Invalid local feature JWS"
    );
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0])?)?;
    ensure!(
        header["alg"] == "EdDSA" && header.get("crit").is_none() && header.get("b64").is_none(),
        "Unsupported local feature JWS header"
    );
    let kid = header["kid"]
        .as_str()
        .context("Local feature JWS has no key ID")?;
    let (_, encoded_key) = keys
        .iter()
        .find(|(name, _)| *name == kid)
        .context("Local feature JWS key ID is unknown")?;
    let key = URL_SAFE_NO_PAD.decode(encoded_key)?;
    let signature = URL_SAFE_NO_PAD.decode(parts[2])?;
    let signed = &compact[..parts[0].len() + 1 + parts[1].len()];
    UnparsedPublicKey::new(&ED25519, key)
        .verify(signed.as_bytes(), &signature)
        .map_err(|_| anyhow::anyhow!("Local feature JWS signature does not match"))?;
    let payload: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1])?)?;
    ensure!(
        payload.is_object(),
        "Local feature JWS payload is not an object"
    );
    Ok(payload)
}

pub fn verify(compact: &str) -> anyhow::Result<Value> {
    verify_with_keys(compact, KEYS)
}

/// Absence is distinct from an invalid or unreadable installed fallback file.
pub fn read(path: &Path) -> anyhow::Result<Option<Value>> {
    super::read_document(path, verify)
}

pub fn installed_path() -> anyhow::Result<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .context("Local application data directory is unavailable")?;
    Ok(PathBuf::from(root).join("Razer/RazerAppEngine/User Data/Apps/Common/featureSettings.jws"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn signed(payload: &str) -> (String, String) {
        let pair = Ed25519KeyPair::from_seed_unchecked(&[42; 32]).unwrap();
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","kid":"fixture"}"#);
        let message = format!("{header}.{}", URL_SAFE_NO_PAD.encode(payload));
        let signature = URL_SAFE_NO_PAD.encode(pair.sign(message.as_bytes()).as_ref());
        (
            format!("{message}.{signature}"),
            URL_SAFE_NO_PAD.encode(pair.public_key().as_ref()),
        )
    }

    #[test]
    fn verifies_signed_local_payload_and_rejects_modified_flags() {
        let (compact, key) = signed(r#"{"global.multipairing":{"provisionFlags":1}}"#);
        let keys = [("fixture", key.as_str())];
        assert_eq!(
            verify_with_keys(&compact, &keys).unwrap()["global.multipairing"]["provisionFlags"],
            1
        );
        let mut parts: Vec<_> = compact.split('.').map(str::to_owned).collect();
        parts[1] = URL_SAFE_NO_PAD.encode(br#"{"global.multipairing":{"provisionFlags":0}}"#);
        assert!(verify_with_keys(&parts.join("."), &keys).is_err());
        assert!(verify(&compact).is_err());
        assert!(verify(r#"{"global.multipairing":{"provisionFlags":1}}"#).is_err());
    }
}
