use greentic_pack_registry::{PackRef, PackRegistryClient, StoreServerClient};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn pack_ref_parse_happy_path() {
    let r = PackRef::parse("greentic.dentist-template@1.2.0").unwrap();
    assert_eq!(r.name, "greentic.dentist-template");
    assert_eq!(r.version, "1.2.0");
}

#[test]
fn pack_ref_parse_missing_version_errors() {
    assert!(PackRef::parse("greentic.dentist-template").is_err());
}

#[test]
fn pack_ref_parse_empty_segment_errors() {
    assert!(PackRef::parse("@1.2.0").is_err());
    assert!(PackRef::parse("greentic.foo@").is_err());
}

#[tokio::test]
async fn fetch_artifact_returns_bytes_on_200() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v1/packs/greentic.dentist-template/1.2.0/artifact",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"PK\x03\x04zip-bytes"))
        .mount(&server)
        .await;

    let client = StoreServerClient::new(server.uri());
    let pack_ref = PackRef::parse("greentic.dentist-template@1.2.0").unwrap();
    let bytes = client.fetch_artifact(&pack_ref).await.unwrap();
    assert!(bytes.starts_with(b"PK"));
}

#[tokio::test]
async fn fetch_artifact_returns_error_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = StoreServerClient::new(server.uri());
    let pack_ref = PackRef::parse("greentic.missing@0.1.0").unwrap();
    assert!(client.fetch_artifact(&pack_ref).await.is_err());
}

#[tokio::test]
async fn fetch_metadata_returns_parsed_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/packs/greentic.foo/0.1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "manifest": { "format_version": "1.0" },
            "artifactSha256": "abc123",
            "publishedAt": "2024-01-01T00:00:00Z",
            "yanked": false
        })))
        .mount(&server)
        .await;

    let client = StoreServerClient::new(server.uri());
    let pack_ref = PackRef::parse("greentic.foo@0.1.0").unwrap();
    let metadata = client.fetch_metadata(&pack_ref).await.unwrap();
    assert_eq!(metadata.artifact_sha256, "abc123");
    assert!(!metadata.yanked);
}
