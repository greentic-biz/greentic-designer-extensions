//! Per-extension network permission resolution.
//!
//! Split out of [`crate::loaded`]: deciding which URLs an extension may reach
//! is its own concern with its own adversarial edge cases (loopback spellings,
//! bracketed IPv6, plain-http downgrade), and it earns the room to state them.

/// Select the URL matcher for a single extension instantiation.
///
/// **Replace semantics:** when the extension's `describe.json` declares one
/// or more patterns under `runtime.permissions.network`, those patterns are
/// the authoritative allow-list for that extension and a fresh
/// [`UrlMatcher`] is built from them (with the loopback-http rule applied —
/// see below). The host-level `override_matcher` is **ignored** in this
/// path — it is the host-wide default that applies only to extensions that
/// make no network declaration.
///
/// When the declaration is empty the host-level override is returned
/// unchanged, which is the deny-all default in most deployments. This
/// preserves existing behavior for extensions that do not need outbound HTTP.
///
/// # Loopback-http rule
///
/// [`UrlMatcher`] rejects non-`https` URLs by default (scheme-downgrade
/// defence) and only honours plain `http` when `with_allow_http(true)` is
/// set. That toggle is **matcher-wide** — it cannot be scoped to a single
/// pattern. To let an extension talk to a local dev service over
/// `http://127.0.0.1` / `http://localhost` WITHOUT also opening plain http
/// to public hosts, we:
///
/// 1. drop any declared `http://` pattern whose host is NOT loopback (it
///    could never be safely honoured — a public-host plain-http downgrade
///    is exactly the attack the matcher defends against), and
/// 2. enable `with_allow_http(true)` only when at least one *loopback*
///    `http://` pattern survives.
///
/// Because the matcher matches scheme exactly per declared pattern, a
/// co-declared `https://host/*` pattern still requires `https` even when
/// the toggle is on — the toggle only decides whether `http` patterns are
/// consulted at all, and after step 1 the only surviving `http` patterns
/// are loopback.
///
/// # Arguments
///
/// * `declared_patterns` — the `runtime.permissions.network` slice from
///   the extension's parsed `describe.json`.
/// * `override_matcher` — the host-level matcher supplied via
///   [`HostOverrides`]. Used only when `declared_patterns` is empty.
///
/// # Returns
///
/// A [`UrlMatcher`] that enforces the correct allow-list for this extension.
pub(crate) fn effective_url_matcher(
    declared_patterns: &[String],
    override_matcher: crate::url_matcher::UrlMatcher,
) -> crate::url_matcher::UrlMatcher {
    if declared_patterns.is_empty() {
        return override_matcher;
    }

    // Replace path: build the effective matcher exclusively from the
    // extension's declared patterns (the host override does NOT apply).
    let mut patterns: Vec<String> = declared_patterns.to_vec();

    // Loopback-http handling: keep loopback http patterns, drop public-host
    // http patterns (they can never be honoured safely), and record whether
    // any loopback http pattern remains so we can flip the matcher-wide
    // allow_http toggle.
    let mut allow_loopback_http = false;
    patterns.retain(|p| {
        if let Some(host) = http_pattern_host(p) {
            if is_loopback_host(host) {
                allow_loopback_http = true;
                true
            } else {
                tracing::warn!(
                    pattern = %p,
                    "dropping non-loopback http url pattern; plain http is only honoured for loopback hosts"
                );
                false
            }
        } else {
            // https (or any non-http) pattern — kept verbatim; UrlMatcher
            // validates it on construction.
            true
        }
    });

    crate::url_matcher::UrlMatcher::from_patterns(patterns).with_allow_http(allow_loopback_http)
}

/// Return the host portion of a `http://` pattern, or `None` when the
/// pattern is not plain http. The leading `*.` wildcard label (e.g.
/// `http://*.example.com/*`) is stripped so the remaining host can be
/// classified; a bare wildcard host is treated as non-loopback.
///
/// Bracketed IPv6 literals (e.g. `[::1]` in `http://[::1]:8787/*`) are
/// returned with their brackets intact so that `is_loopback_host` can strip
/// them: splitting on the first `:` would otherwise yield the bare `"["`
/// opener and misclassify `[::1]` as non-loopback.
fn http_pattern_host(pattern: &str) -> Option<&str> {
    let rest = pattern.strip_prefix("http://")?;
    let host_and_port = rest.split('/').next().unwrap_or(rest);
    // Strip the userinfo (`user@host`) if present.
    let host_and_port = host_and_port.rsplit('@').next().unwrap_or(host_and_port);
    // Bracketed IPv6 literal: `[::1]` or `[::1]:8787`.
    // Return the bracketed token (including the `]`) so is_loopback_host can
    // strip the brackets and compare against `::1`.
    let host = if let Some(bracket_end) = host_and_port.find(']') {
        &host_and_port[..=bracket_end]
    } else {
        // Plain hostname or IPv4: split on first `:` to drop optional port.
        host_and_port.split(':').next().unwrap_or(host_and_port)
    };
    Some(host.trim_start_matches("*."))
}

/// Loopback hosts for which plain http is acceptable: `localhost`,
/// `127.0.0.1` (any IPv4 loopback in `127.0.0.0/8` would also qualify, but
/// the only spellings extensions declare in practice are these two and
/// `[::1]`), and the IPv6 loopback.
fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::url_matcher::UrlMatcher;

    fn empty_override() -> UrlMatcher {
        UrlMatcher::default()
    }

    fn override_with_pattern(pattern: &str) -> UrlMatcher {
        UrlMatcher::from_patterns(vec![pattern.to_string()])
    }

    /// Extensions that declare network patterns must have exactly those
    /// patterns enforced — the host-level override must NOT apply.
    #[test]
    fn declared_patterns_allow_declared_host_and_deny_undeclared() {
        let declared = vec!["https://api.github.com/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("https://api.github.com/repos/org/repo"),
            "declared host must be allowed"
        );
        assert!(
            !matcher.is_allowed("https://evil.com/"),
            "undeclared host must be denied even though host override is empty"
        );
    }

    /// When no network patterns are declared the host-level override is
    /// returned verbatim — behavior is unchanged for legacy extensions.
    #[test]
    fn empty_declaration_falls_back_to_host_override() {
        let override_matcher = override_with_pattern("https://allowed.com/*");
        let matcher = effective_url_matcher(&[], override_matcher);

        assert!(
            matcher.is_allowed("https://allowed.com/path"),
            "host-override host must be reachable when declare is empty"
        );
        assert!(
            !matcher.is_allowed("https://other.com/path"),
            "host-override deny must still apply"
        );
    }

    /// Non-empty declaration REPLACES (not unions) the host-level
    /// override. A broader operator override must not bleed through to
    /// an extension that declared its own narrower allow-list.
    #[test]
    fn declared_patterns_replace_not_union_host_override() {
        let declared = vec!["https://api.github.com/*".to_string()];
        let override_matcher = override_with_pattern("https://operator-allowed.com/*");
        let matcher = effective_url_matcher(&declared, override_matcher);

        assert!(
            matcher.is_allowed("https://api.github.com/repos/org/repo"),
            "declared host must be allowed"
        );
        assert!(
            !matcher.is_allowed("https://operator-allowed.com/anything"),
            "operator override must NOT bleed through when declaration is non-empty"
        );
    }

    /// Empty declaration + empty host override must deny every URL —
    /// this is the default deny-all posture for extensions that never
    /// call the network.
    #[test]
    fn empty_declaration_and_empty_override_denies_everything() {
        let matcher = effective_url_matcher(&[], empty_override());

        assert!(
            !matcher.is_allowed("https://api.github.com/anything"),
            "empty declaration + empty override must produce deny-all matcher"
        );
    }

    /// A declared loopback `http://127.0.0.1` pattern must be reachable
    /// over plain http. The matcher rejects non-https by default, so the
    /// effective matcher has to opt http in — but ONLY because the
    /// declared pattern is loopback.
    #[test]
    fn declared_http_loopback_127_allows_plain_http() {
        let declared = vec!["http://127.0.0.1:8787/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://127.0.0.1:8787/execute"),
            "declared http loopback pattern must permit plain http to that loopback"
        );
    }

    /// `http://localhost` is the other loopback spelling and must behave
    /// the same as `127.0.0.1`.
    #[test]
    fn declared_http_loopback_localhost_allows_plain_http() {
        let declared = vec!["http://localhost:8787/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://localhost:8787/execute"),
            "declared http localhost pattern must permit plain http to localhost"
        );
    }

    /// The loopback-http opt-in must NOT leak to non-loopback http: a
    /// declared `http://evil.com` pattern must stay denied (no plain-http
    /// downgrade for a public host) even though the pattern technically
    /// targets http.
    #[test]
    fn declared_http_non_loopback_stays_denied() {
        let declared = vec!["http://evil.com/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            !matcher.is_allowed("http://evil.com/anything"),
            "plain http must stay denied for a non-loopback declared host"
        );
    }

    /// A mixed declaration (loopback http + a normal https host) must keep
    /// https reachable AND the loopback http reachable, while still
    /// refusing plain http to the https host (the global `allow_http` toggle
    /// must not downgrade the https-only host because no http pattern for
    /// it exists, and `is_allowed` matches scheme exactly per pattern).
    #[test]
    fn mixed_loopback_http_and_https_host() {
        let declared = vec![
            "http://127.0.0.1:8787/*".to_string(),
            "https://api.example.com/*".to_string(),
        ];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://127.0.0.1:8787/execute"),
            "loopback http must be allowed in a mixed declaration"
        );
        assert!(
            matcher.is_allowed("https://api.example.com/v1/foo"),
            "declared https host must stay reachable"
        );
        assert!(
            !matcher.is_allowed("http://api.example.com/v1/foo"),
            "plain http to the https-only host must stay denied even with loopback http enabled"
        );
    }

    /// A bracketed IPv6 loopback `http://[::1]:8787/*` must survive the
    /// loopback filter and allow plain http to `http://[::1]:8787/x`.
    ///
    /// The url crate's `host_str()` returns the bracketed form `"[::1]"` for
    /// both the pattern and the request URL, so the Exact host rule matches.
    /// The bug this test guards against: `http_pattern_host` previously split
    /// on the first `:`, yielding `"["` as the host, which was classified as
    /// non-loopback and dropped.
    #[test]
    fn declared_http_ipv6_loopback_allows_plain_http() {
        let declared = vec!["http://[::1]:8787/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://[::1]:8787/x"),
            "declared http IPv6 loopback pattern must permit plain http to [::1]"
        );
        // Must not bleed to arbitrary non-loopback hosts.
        assert!(
            !matcher.is_allowed("http://evil.com/x"),
            "IPv6 loopback opt-in must not permit plain http to non-loopback hosts"
        );
    }

    /// An adversarial pattern `http://[::1].evil.com/*` that tries to smuggle
    /// a non-loopback host inside brackets must be rejected. The url crate
    /// refuses to parse this (it is not a valid bracketed IPv6 literal), so
    /// the pattern is either unparseable (dropped by `UrlMatcher`) or the
    /// resulting host does not match `[::1]` in `is_loopback_host`.
    ///
    /// Either way the request to `http://[::1].evil.com/x` must be denied.
    #[test]
    fn adversarial_fake_ipv6_bracket_host_is_denied() {
        let declared = vec!["http://[::1].evil.com/*".to_string()];
        let matcher = effective_url_matcher(&declared, empty_override());

        // The pattern is malformed: url::Url::parse rejects `[::1].evil.com`
        // as a host, so the pattern is silently dropped and the matcher
        // remains deny-all for this declaration.
        assert!(
            !matcher.is_allowed("http://[::1].evil.com/x"),
            "malformed bracketed host must not be allowed"
        );
        // Real IPv6 loopback must also NOT be granted by a bad pattern.
        assert!(
            !matcher.is_allowed("http://[::1]/x"),
            "bad pattern must not accidentally allow real IPv6 loopback"
        );
    }
}
