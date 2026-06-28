//! Integration test: get-token through a stubbed broker returns the access token.
//!
//! The stub is a plain TCP listener that serves a single HTTP/1.1 response with
//! a fixed JSON token body. `HostState::get_token` (the broker-v1 Host impl)
//! must pass the permission gate and forward the request to the stub, then
//! deserialize and return the access token.
use greentic_ext_runtime::host_bindings::greentic::oauth_broker::broker_v1::Host as OAuthHost;
use greentic_ext_runtime::oauth::OAuthBrokerConfig;
use greentic_ext_runtime::{HostState, reqwest};
use greentic_extension_sdk_contract::describe::Permissions;

#[test]
fn get_token_returns_access_token_via_stub_broker() {
    // Bind to an ephemeral port so the test never collides with another service.
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    // Serve exactly one request in a background thread, then exit.
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = server.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let body = r#"{"access_token":"AT-123","expires_at":999}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let mut permissions = Permissions::default();
    permissions.oauth_providers.push("hubspot".into());

    let broker_config = OAuthBrokerConfig {
        // `request_resource_token_blocking` normally requires https; the
        // localhost allowance added in oauth.rs lets us use a plain http://
        // stub server in tests while keeping production https-only.
        http_base_url: format!("http://{addr}/"),
        env: "dev".into(),
        tenant: "acme".into(),
        team: None,
        shared_secret: None,
    };

    let mut host = HostState::builder("ext".into(), permissions)
        .http_client(Some(reqwest::blocking::Client::new()))
        .oauth_config(Some(broker_config))
        .build();

    let token_json = host.get_token(
        "hubspot".into(),
        String::new(),
        vec!["crm.objects.contacts.read".into()],
    );

    handle.join().ok();

    assert!(
        token_json.contains("AT-123"),
        "expected token JSON to contain 'AT-123', got: {token_json}"
    );
}

/// The shared secret must travel only in the `Authorization: Bearer` header —
/// never in the POST body — and the successful response must contain the token.
#[test]
fn bearer_secret_is_sent_in_header_not_body() {
    use std::sync::{Arc, Mutex};

    // Capture the raw HTTP request bytes so we can inspect both headers and body.
    let captured_request = Arc::new(Mutex::new(Vec::<u8>::new()));
    let captured_for_thread = Arc::clone(&captured_request);

    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = server.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            *captured_for_thread.lock().unwrap() = buf[..n].to_vec();
            let body = r#"{"access_token":"AT-xyz","expires_at":999}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body,
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let mut permissions = Permissions::default();
    permissions.oauth_providers.push("hubspot".into());

    let broker_config = OAuthBrokerConfig {
        http_base_url: format!("http://{addr}/"),
        env: "dev".into(),
        tenant: "acme".into(),
        team: None,
        shared_secret: Some("s3cr3t-shared".into()),
    };

    let mut host = HostState::builder("ext".into(), permissions)
        .http_client(Some(reqwest::blocking::Client::new()))
        .oauth_config(Some(broker_config))
        .build();

    let token_json = host.get_token(
        "hubspot".into(),
        String::new(),
        vec!["crm.objects.contacts.read".into()],
    );

    handle.join().ok();

    assert!(
        token_json.contains("AT-xyz"),
        "expected token JSON to contain 'AT-xyz', got: {token_json}"
    );

    let raw_bytes = captured_request.lock().unwrap().clone();
    let raw_request = String::from_utf8_lossy(&raw_bytes);

    // HTTP/1.1 header names are case-insensitive; reqwest normalises them to
    // lowercase on the wire.  Check the lowercased form of both the header name
    // and the scheme keyword while preserving the secret value exactly.
    assert!(
        raw_request.contains("authorization: Bearer s3cr3t-shared"),
        "expected 'authorization: Bearer s3cr3t-shared' header in captured request, got:\n{raw_request}"
    );

    // Everything after the blank line that separates HTTP headers from the body.
    let request_body = raw_request
        .split_once("\r\n\r\n")
        .map_or("", |(_, body)| body);

    assert!(
        !request_body.contains("s3cr3t-shared"),
        "shared secret must not appear in request body, body:\n{request_body}"
    );
}

/// A non-2xx broker response must produce the normalized `broker_request_failed`
/// error string without leaking the shared secret or the internal broker URL.
#[test]
fn broker_non_2xx_returns_normalized_error_without_leaking_secret() {
    let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = server.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = r#"{"error":"boom"}"#;
            let resp = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body,
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });

    let mut permissions = Permissions::default();
    permissions.oauth_providers.push("hubspot".into());

    let broker_config = OAuthBrokerConfig {
        http_base_url: format!("http://{addr}/"),
        env: "dev".into(),
        tenant: "acme".into(),
        team: None,
        shared_secret: Some("s3cr3t-shared".into()),
    };

    let mut host = HostState::builder("ext".into(), permissions)
        .http_client(Some(reqwest::blocking::Client::new()))
        .oauth_config(Some(broker_config))
        .build();

    let error_json = host.get_token(
        "hubspot".into(),
        String::new(),
        vec!["crm.objects.contacts.read".into()],
    );

    handle.join().ok();

    assert!(
        error_json.contains("broker_request_failed"),
        "expected normalized error 'broker_request_failed', got: {error_json}"
    );

    assert!(
        !error_json.contains("s3cr3t-shared"),
        "shared secret must not appear in error response, got: {error_json}"
    );

    let addr_str = addr.to_string();
    assert!(
        !error_json.contains(&addr_str),
        "broker URL must not appear in error response, got: {error_json}"
    );
}
