//! Strict URL allow-list matcher tests. The cases below pin the behaviour
//! that defends against the three classic attack vectors (open redirect,
//! subdomain confusion, scheme downgrade) plus a happy path.

use greentic_ext_runtime::UrlMatcher;

fn matcher(patterns: &[&str]) -> UrlMatcher {
    UrlMatcher::from_patterns(patterns.iter().map(|s| (*s).to_string()).collect())
}

#[test]
fn allows_exact_host_and_path_prefix_match() {
    let m = matcher(&["https://api.openai.com/v1/*"]);
    assert!(m.is_allowed("https://api.openai.com/v1/chat/completions"));
    assert!(m.is_allowed("https://api.openai.com/v1/embeddings"));
}

#[test]
fn rejects_open_redirect_via_query_string() {
    let m = matcher(&["https://evil.com/*"]);
    assert!(
        !m.is_allowed("https://allowed.com/redirect?to=https://evil.com/"),
        "matcher must not be fooled by a substring of evil.com in the query"
    );
}

#[test]
fn rejects_subdomain_confusion() {
    let m = matcher(&["https://allowed.com/*"]);
    assert!(
        !m.is_allowed("https://evil.com.allowed.com/"),
        "matcher must require a host boundary, not a substring"
    );
}

#[test]
fn rejects_scheme_downgrade() {
    let m = matcher(&["https://allowed.com/*"]);
    assert!(
        !m.is_allowed("http://allowed.com/"),
        "matcher must require exact scheme — no http when https expected"
    );
}

#[test]
fn allows_wildcard_subdomain_when_pattern_uses_star_dot() {
    let m = matcher(&["https://*.example.com/*"]);
    assert!(m.is_allowed("https://api.example.com/v1/foo"));
    assert!(m.is_allowed("https://cdn.example.com/assets/logo.png"));
    assert!(
        !m.is_allowed("https://example.com/x"),
        "bare host must not match *.example.com — wildcard needs at least one label"
    );
}

#[test]
fn rejects_non_https_by_default() {
    let m = matcher(&["http://allowed.com/*"]);
    assert!(
        !m.is_allowed("http://allowed.com/"),
        "http rejected by default even when pattern uses http://"
    );
}

#[test]
fn opt_in_allow_http_lets_http_through() {
    let m = UrlMatcher::from_patterns(vec!["http://allowed.com/*".into()]).with_allow_http(true);
    assert!(m.is_allowed("http://allowed.com/anything"));
}

#[test]
fn rejects_url_that_fails_to_parse() {
    let m = matcher(&["https://allowed.com/*"]);
    assert!(!m.is_allowed("not a url at all"));
}
