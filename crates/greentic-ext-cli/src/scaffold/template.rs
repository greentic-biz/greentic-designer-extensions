//! Template rendering and file writing.

use std::{collections::HashMap, fs, path::Path};

// used by scaffold::commands::new::run in Task 16
#[allow(dead_code)]
pub struct Context {
    values: HashMap<&'static str, String>,
}

// used by scaffold::commands::new::run in Task 16
#[allow(dead_code)]
impl Context {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &'static str, value: impl Into<String>) -> &mut Self {
        self.values.insert(key, value.into());
        self
    }

    pub fn render(&self, template: &str) -> anyhow::Result<String> {
        let mut out = template.to_string();
        let mut remaining_passes = 4;
        while remaining_passes > 0 {
            let before = out.clone();
            for (key, value) in &self.values {
                let token = format!("{{{{{key}}}}}");
                out = out.replace(&token, value);
            }
            if out == before {
                break;
            }
            remaining_passes -= 1;
        }
        if let Some(pos) = out.find("{{") {
            let end = out[pos..].find("}}").map_or(out.len(), |e| pos + e + 2);
            anyhow::bail!("unsubstituted placeholder: {}", &out[pos..end]);
        }
        Ok(out)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

// used by scaffold::commands::new::run in Task 16
#[allow(dead_code)]
pub fn write_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

// used by scaffold::commands::new::run in Task 16
#[allow(dead_code)]
pub fn render_and_write(ctx: &Context, template: &str, path: &Path) -> anyhow::Result<()> {
    let rendered = ctx.render(template)?;
    write_file(path, rendered.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_substitutes_placeholder() {
        let mut ctx = Context::new();
        ctx.set("name", "demo");
        let out = ctx.render("hello {{name}}!").unwrap();
        assert_eq!(out, "hello demo!");
    }

    #[test]
    fn render_multiple_placeholders() {
        let mut ctx = Context::new();
        ctx.set("name", "demo").set("version", "0.1.0");
        let out = ctx.render("{{name}}@{{version}}").unwrap();
        assert_eq!(out, "demo@0.1.0");
    }

    #[test]
    fn render_unsubstituted_placeholder_errors() {
        let ctx = Context::new();
        let err = ctx.render("hello {{missing}}").unwrap_err();
        assert!(err.to_string().contains("{{missing}}"));
    }

    #[test]
    fn render_literal_text_passthrough() {
        let ctx = Context::new();
        let out = ctx.render("plain text no braces").unwrap();
        assert_eq!(out, "plain text no braces");
    }

    #[test]
    fn write_file_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let dst = tmp.path().join("a/b/c/file.txt");
        write_file(&dst, b"hello").unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), b"hello");
    }

    #[test]
    fn render_and_write_substitutes_before_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let dst = tmp.path().join("out.txt");
        let mut ctx = Context::new();
        ctx.set("who", "world");
        render_and_write(&ctx, "hello {{who}}", &dst).unwrap();
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "hello world");
    }
}
