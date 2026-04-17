use ed25519_dalek::SigningKey;
use greentic_ext_contract::{artifact_sha256, sign_ed25519, verify_ed25519};
use rand::rngs::OsRng;

#[test]
fn sha256_is_deterministic() {
    assert_eq!(artifact_sha256(b"hello"), artifact_sha256(b"hello"));
    assert_ne!(artifact_sha256(b"hello"), artifact_sha256(b"world"));
}

#[test]
fn round_trip_sign_verify() {
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    let pk_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        pk.to_bytes(),
    );
    let payload = b"arbitrary payload";
    let sig = sign_ed25519(&sk, payload);
    verify_ed25519(&pk_b64, &sig, payload).expect("signature must verify");
}

#[test]
fn tampered_payload_fails_verification() {
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    let pk_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        pk.to_bytes(),
    );
    let sig = sign_ed25519(&sk, b"original");
    let err = verify_ed25519(&pk_b64, &sig, b"tampered").unwrap_err();
    assert!(format!("{err}").contains("verify"));
}
