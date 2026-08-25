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
    patterns.retain(|p| match classify_http_pattern(p) {
        HttpClass::Loopback => {
            allow_loopback_http = true;
            true
        }
        HttpClass::Public => {
            tracing::warn!(
                pattern = %p,
                "dropping non-loopback http url pattern; plain http is only honoured for loopback hosts"
            );
            false
        }
        // https (or any non-http) pattern — kept verbatim; UrlMatcher
        // validates it on construction.
        HttpClass::Other => true,
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
/// How a declared pattern is classified for the plain-http rule.
#[derive(Debug, PartialEq, Eq)]
enum HttpClass {
    /// Plain http to a loopback address — keep it, and switch the toggle on.
    Loopback,
    /// Plain http to anything else — drop it; it could never be honoured safely.
    Public,
    /// Not plain http (https, or a scheme the matcher will reject on its own).
    Other,
}

/// Classify a declared pattern with the *same parser the matcher uses*.
///
/// This used to be hand-rolled string surgery — `strip_prefix("http://")`, then
/// split on `/`, then `rsplit('@')` — and the two disagreed about where the
/// authority ends. `Url` terminates it at `/`, `?`, `#`, or `\`, and normalizes
/// the scheme; the string version stopped only at `/` and matched the scheme
/// literally. Every gap between them was a plain-http downgrade to a public
/// host: `http://evil.com?@localhost/*` classified as loopback (last `@` yields
/// `localhost`) and so both survived the drop *and* switched the toggle on,
/// while the matcher resolved the host to `evil.com` with a path prefix of `/`.
/// `HTTP://evil.com/*` and a leading space did the same by failing the literal
/// prefix test and being waved through as "not http".
///
/// One parser decides both, so there is no gap left to disagree in.
fn classify_http_pattern(pattern: &str) -> HttpClass {
    // The matcher normalizes `*.` into a placeholder label before parsing;
    // do the same here so a wildcard pattern reaches `Url` in the same shape.
    let normalized = pattern
        .trim_end_matches("/*")
        .replace("://*.", "://__wildcard__.");
    let Ok(url) = url::Url::parse(&normalized) else {
        // Unparseable patterns are dropped by `UrlMatcher::from_patterns`
        // anyway; classifying as NotHttp keeps them out of the toggle.
        return HttpClass::Other;
    };
    if url.scheme() != "http" {
        return HttpClass::Other;
    }
    match url.host() {
        Some(url::Host::Ipv4(ip)) if ip.is_loopback() => HttpClass::Loopback,
        Some(url::Host::Ipv6(ip)) if ip.is_loopback() => HttpClass::Loopback,
        // `localhost` only, and only as the whole host. A wildcard pattern
        // arrives here as `__wildcard__.localhost`, which is not loopback —
        // `*.localhost` resolves through the system resolver like any other
        // name, so treating it as loopback would hand plain http to whatever a
        // search-domain quirk points `evil.localhost` at.
        Some(url::Host::Domain(d)) if d.eq_ignore_ascii_case("localhost") => HttpClass::Loopback,
        _ => HttpClass::Public,
    }
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

    /// A declaration that mixes loopback http with a *public* http host must
    /// keep only the loopback one.
    ///
    /// This is the case that isolates the drop step. Every other http test
    /// declares a public host alone, where the denial comes from
    /// `allow_loopback_http` never being switched on — so the `retain` that
    /// drops public-host http patterns could be deleted outright and the suite
    /// would stay green, while an extension declaring both got cleartext http
    /// to the public host.
    #[test]
    fn a_loopback_declaration_does_not_carry_a_public_http_host_with_it() {
        let declared = vec![
            "http://127.0.0.1:8787/*".to_string(),
            "http://evil.com/*".to_string(),
        ];
        let matcher = effective_url_matcher(&declared, empty_override());

        assert!(
            matcher.is_allowed("http://127.0.0.1:8787/execute"),
            "the loopback pattern must survive"
        );
        assert!(
            !matcher.is_allowed("http://evil.com/anything"),
            "the loopback opt-in must not carry a public http host through with it"
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

    /// Authority-delimiter smuggling. `Url` ends the authority at `?`, `#` or
    /// `\` as well as `/`; the old hand-rolled parser stopped only at `/` and
    /// then took the text after the last `@`, so each of these read as
    /// `localhost` — surviving the drop AND switching the matcher-wide http
    /// toggle on — while the matcher resolved the host to `evil.com`.
    #[test]
    fn a_pattern_that_smuggles_loopback_past_the_authority_is_not_loopback() {
        for pattern in [
            "http://evil.com?@localhost/*",
            "http://evil.com#@localhost/*",
            "http://evil.com\\@localhost/*",
        ] {
            let matcher = effective_url_matcher(&[pattern.to_string()], empty_override());
            assert!(
                !matcher.is_allowed("http://evil.com/anything"),
                "{pattern} must not grant plain http to evil.com"
            );
            assert!(
                !matcher.is_allowed("http://evil.com/@localhost/pwn"),
                "{pattern} must not grant plain http to evil.com"
            );
        }
    }

    /// Scheme spelling and leading whitespace. `strip_prefix("http://")` is a
    /// literal, non-trimming match, so both of these were misclassified as
    /// "not http" and kept verbatim — and once any loopback pattern turned the
    /// toggle on, the matcher honoured them.
    #[test]
    fn an_oddly_spelled_http_pattern_is_still_classified_as_http() {
        for odd in ["HTTP://evil.com/*", " http://evil.com/*"] {
            let declared = vec!["http://127.0.0.1:1/*".to_string(), odd.to_string()];
            let matcher = effective_url_matcher(&declared, empty_override());
            assert!(
                !matcher.is_allowed("http://evil.com/anything"),
                "{odd} must not survive the public-http drop"
            );
        }
    }

    /// `*.localhost` is not loopback: it resolves through the system resolver
    /// like any other name, so a search-domain quirk could point
    /// `evil.localhost` anywhere.
    #[test]
    fn a_wildcard_localhost_pattern_is_not_treated_as_loopback() {
        let matcher =
            effective_url_matcher(&["http://*.localhost/*".to_string()], empty_override());
        assert!(
            !matcher.is_allowed("http://evil.localhost/x"),
            "a wildcard under .localhost must not get the loopback exemption"
        );
    }

    /// Any address in `127.0.0.0/8` is loopback, not just the canonical
    /// spelling — and `Url` normalizes `127.1` and `0x7f000001` into it.
    #[test]
    fn other_ipv4_loopback_spellings_are_loopback() {
        for pattern in ["http://127.0.0.2:8787/*", "http://127.1:8787/*"] {
            let matcher = effective_url_matcher(&[pattern.to_string()], empty_override());
            assert!(
                matcher.patterns().iter().any(|p| p == pattern),
                "{pattern} should have been kept as loopback"
            );
        }
    }
}
