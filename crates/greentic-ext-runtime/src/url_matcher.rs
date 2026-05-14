//! Strict URL allow-list matcher.
//!
//! The matcher parses both the pattern and the request URL with
//! `url::Url`, then compares three orthogonal facets:
//!
//! 1. **Scheme** — exact match; we additionally reject non-`https://`
//!    by default to defend against scheme-downgrade attacks. (Operators
//!    can opt into `http` by calling `with_allow_http(true)`.)
//! 2. **Host** — either exact match or `*.<suffix>` wildcard. The
//!    wildcard requires at least one label (so `*.example.com` does
//!    NOT match `example.com`). Substring matches are explicitly
//!    rejected — the test suite pins both the "subdomain confusion"
//!    (`evil.com.allowed.com`) and the "open-redirect via query
//!    string" cases.
//! 3. **Path** — prefix match against `pattern.path` with the trailing
//!    `*` stripped. `/v1/*` matches `/v1/chat/completions`. A bare
//!    pattern path of `/` matches any path.

use url::Url;

#[derive(Debug, Clone, Default)]
pub struct UrlMatcher {
    patterns: Vec<ParsedPattern>,
    raw: Vec<String>,
    allow_http: bool,
}

#[derive(Debug, Clone)]
struct ParsedPattern {
    scheme: String,
    host_rule: HostRule,
    path_prefix: String,
}

#[derive(Debug, Clone)]
enum HostRule {
    Exact(String),
    /// `*.example.com` — matches any host that ends with `.example.com`.
    WildcardSuffix(String),
}

impl UrlMatcher {
    #[must_use]
    pub fn from_patterns(patterns: Vec<String>) -> Self {
        let mut parsed = Vec::with_capacity(patterns.len());
        for p in &patterns {
            if let Some(pp) = Self::parse_pattern(p) {
                parsed.push(pp);
            } else {
                tracing::warn!(pattern = %p, "unparseable url pattern; ignoring");
            }
        }
        Self {
            patterns: parsed,
            raw: patterns,
            allow_http: false,
        }
    }

    #[must_use]
    pub fn with_allow_http(mut self, allow: bool) -> Self {
        self.allow_http = allow;
        self
    }

    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.raw
    }

    fn parse_pattern(pattern: &str) -> Option<ParsedPattern> {
        let stripped = pattern.trim_end_matches("/*");
        let (placeholder_used, normalized) = if let Some(rest) = stripped.strip_prefix("https://*.")
        {
            (true, format!("https://__wildcard__.{rest}"))
        } else if let Some(rest) = stripped.strip_prefix("http://*.") {
            (true, format!("http://__wildcard__.{rest}"))
        } else {
            (false, stripped.to_string())
        };
        let parsed = Url::parse(&normalized).ok()?;
        let host_str = parsed.host_str()?.to_string();
        let host_rule = if placeholder_used {
            let suffix = host_str.strip_prefix("__wildcard__.")?.to_string();
            HostRule::WildcardSuffix(suffix)
        } else {
            HostRule::Exact(host_str)
        };
        let path_prefix = if parsed.path().is_empty() || parsed.path() == "/" {
            "/".to_string()
        } else {
            parsed.path().to_string()
        };
        Some(ParsedPattern {
            scheme: parsed.scheme().to_string(),
            host_rule,
            path_prefix,
        })
    }

    #[must_use]
    pub fn is_allowed(&self, url: &str) -> bool {
        let Ok(parsed) = Url::parse(url) else {
            return false;
        };
        if parsed.scheme() != "https" && !self.allow_http {
            return false;
        }
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let path = parsed.path();
        self.patterns.iter().any(|pat| {
            if pat.scheme != parsed.scheme() {
                return false;
            }
            let host_ok = match &pat.host_rule {
                HostRule::Exact(h) => h.eq_ignore_ascii_case(host),
                HostRule::WildcardSuffix(suffix) => {
                    let lower_host = host.to_ascii_lowercase();
                    let lower_suffix = suffix.to_ascii_lowercase();
                    if !lower_host.ends_with(&lower_suffix) {
                        return false;
                    }
                    let prefix_len = lower_host.len().saturating_sub(lower_suffix.len());
                    if prefix_len == 0 {
                        return false;
                    }
                    let prefix = &lower_host[..prefix_len];
                    prefix.ends_with('.')
                }
            };
            if !host_ok {
                return false;
            }
            if pat.path_prefix == "/" {
                return true;
            }
            path.starts_with(&pat.path_prefix)
        })
    }
}
