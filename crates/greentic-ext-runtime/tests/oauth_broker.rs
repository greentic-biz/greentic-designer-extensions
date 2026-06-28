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
