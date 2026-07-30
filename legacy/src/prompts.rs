use std::fs;
use std::path::PathBuf;

pub const COMPACT: &str = "COMPACT.md";
pub const SUMMARY: &str = "SUMMARY.md";
pub const SYSTEM: &str = "SYSTEM.md";

const COMPACT_DEFAULT: &str = include_str!("prompts/COMPACT.md");
const SUMMARY_DEFAULT: &str = include_str!("prompts/SUMMARY.md");
const SYSTEM_DEFAULT: &str = include_str!("prompts/SYSTEM.md");
fn prompts_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BIMO_PROMPTS_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join("bimo").join("prompts");
        if p.is_dir() {
            return Some(p);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join(".agents").join("prompts");
        if p.is_dir() {
            return Some(p);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let p = home.join(".agents").join("prompts");
        if p.is_dir() {
            return Some(p);
        }
    }

    let cwd_prompts = PathBuf::from("prompts");
    if cwd_prompts.is_dir() {
        return Some(cwd_prompts);
    }

    let manifest_prompts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompts");
    if manifest_prompts.is_dir() {
        return Some(manifest_prompts);
    }

    None
}

pub fn load(name: &str) -> String {
    if let Some(dir) = prompts_dir() {
        let path = dir.join(name);
        if let Ok(content) = fs::read_to_string(&path) {
            tracing::debug!(path = %path.display(), "loaded prompt from disk");
            return content;
        }
        tracing::warn!(path = %path.display(), "failed to read prompt file, using default");
    } else {
        tracing::debug!("no prompts directory found, using compiled-in defaults");
    }

    if name == SYSTEM {
        if let Ok(cwd) = std::env::current_dir() {
            for ancestor in cwd.ancestors() {
                let path = ancestor.join(".agents").join("SYSTEM.md");
                if let Ok(content) = fs::read_to_string(&path) {
                    tracing::debug!(path = %path.display(), "loaded system prompt from .agents/SYSTEM.md");
                    return content;
                }
            }
        }

        if let Some(home) = dirs::home_dir() {
            let path = home.join(".agents").join("SYSTEM.md");
            if let Ok(content) = fs::read_to_string(&path) {
                tracing::debug!(path = %path.display(), "loaded system prompt from ~/.agents/SYSTEM.md");
                return content;
            }
        }
    }

    match name {
        COMPACT => COMPACT_DEFAULT.to_string(),
        SUMMARY => SUMMARY_DEFAULT.to_string(),
        SYSTEM => SYSTEM_DEFAULT.to_string(),
        _ => String::new(),
    }
}

pub fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    out
}
