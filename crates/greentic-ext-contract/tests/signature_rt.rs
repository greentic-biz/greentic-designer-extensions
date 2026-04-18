use ed25519_dalek::SigningKey;
use greentic_ext_contract::{artifact_sha256, canonical_signing_payload, sign_ed25519, verify_ed25519, DescribeJson};
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
    let pk_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, pk.to_bytes());
    let payload = b"arbitrary payload";
    let sig = sign_ed25519(&sk, payload);
    verify_ed25519(&pk_b64, &sig, payload).expect("signature must verify");
}

#[test]
fn tampered_payload_fails_verification() {
    let sk = SigningKey::generate(&mut OsRng);
    let pk = sk.verifying_key();
    let pk_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, pk.to_bytes());
    let sig = sign_ed25519(&sk, b"original");
    let err = verify_ed25519(&pk_b64, &sig, b"tampered").unwrap_err();
    assert!(format!("{err}").contains("verify"));
}

fn sample_describe_with_sig(sig_value: Option<&str>) -> DescribeJson {
    let json = serde_json::json!({
        "apiVersion": "greentic.ai/v1",
        "kind": "DesignExtension",
        "metadata": {
            "id": "greentic.canonicalize-test",
            "name": "Canonicalize Test",
            "version": "0.1.0",
            "summary": "test fixture",
            "author": { "name": "test" },
            "license": "MIT"
        },
        "engine": { "greenticDesigner": "*", "extRuntime": "*" },
        "capabilities": { "offered": [], "required": [] },
        "runtime": { "component": "x.wasm", "memoryLimitMB": 64, "permissions": {} },
        "contributions": {},
        "signature": sig_value.map(|v| serde_json::json!({
            "algorithm": "ed25519",
            "publicKey": "AAAA",
            "value": v
        }))
    });
    serde_json::from_value(json).expect("sample describe parses")
}

#[test]
fn canonical_payload_omits_signature_field() {
    let with_sig = sample_describe_with_sig(Some("SIG_A"));
    let bytes_with = canonical_signing_payload(&with_sig).expect("canonicalize with sig");
    let without_sig = sample_describe_with_sig(None);
    let bytes_without = canonical_signing_payload(&without_sig).expect("canonicalize without sig");
    assert_eq!(bytes_with, bytes_without, "canonical bytes must ignore .signature");
}

#[test]
fn canonical_payload_is_deterministic_across_serde_round_trip() {
    let d1 = sample_describe_with_sig(None);
    let json = serde_json::to_string(&d1).unwrap();
    let d2: DescribeJson = serde_json::from_str(&json).unwrap();
    let b1 = canonical_signing_payload(&d1).unwrap();
    let b2 = canonical_signing_payload(&d2).unwrap();
    assert_eq!(b1, b2, "canonical form must survive serde round trip");
}
