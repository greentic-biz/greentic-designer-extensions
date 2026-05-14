//! Strict URL allow-list matcher. Real implementation lands in Task B.4.

/// Strict URL allow-list matcher. Filled in by Task B.4.
#[derive(Debug, Default, Clone)]
pub struct UrlMatcher {
    patterns: Vec<String>,
}

impl UrlMatcher {
    #[must_use]
    pub fn from_patterns(patterns: Vec<String>) -> Self {
        Self { patterns }
    }

    /// Always returns `false` in this stub. Real logic in Task B.4.
    #[must_use]
    pub fn is_allowed(&self, _url: &str) -> bool {
        false
    }

    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}
