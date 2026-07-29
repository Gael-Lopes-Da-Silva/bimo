use std::fs;
use std::path::PathBuf;

/// Prompt file names.
pub const COMPACT: &str = "COMPACT.md";
pub const COMPACT_PREFIX: &str = "COMPACT_PREFIX.md";
pub const SYSTEM: &str = "SYSTEM.md";

/// Compile-time fallback content embedded from the prompts/ directory.
const COMPACT_DEFAULT: &str = include_str!("../prompts/COMPACT.md");
const COMPACT_PREFIX_DEFAULT: &str = include_str!("../prompts/COMPACT_PREFIX.md");
const SYSTEM_DEFAULT: &str = include_str!("../prompts/SYSTEM.md");

/// Resolve the prompts directory. Checks, in order:
/// 1. `BIMO_PROMPTS_DIR` environment variable
/// 2. `prompts/` relative to the current working directory
/// 3. `prompts/` relative to `CARGO_MANIFEST_DIR` (compile-time dev path)
fn prompts_dir() -> Option<PathBuf> {
    // 1. Explicit env var
    if let Ok(dir) = std::env::var("BIMO_PROMPTS_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }

    // 2. Relative to CWD
    let cwd_prompts = PathBuf::from("prompts");
    if cwd_prompts.is_dir() {
        return Some(cwd_prompts);
    }

    // 3. Relative to manifest dir (compile-time)
    let manifest_prompts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompts");
    if manifest_prompts.is_dir() {
        return Some(manifest_prompts);
    }

    None
}

/// Load a prompt file by name, returning its content as a `String`.
/// Falls back to the compiled-in default if the file cannot be found or read.
///
/// For `SYSTEM.md`, also checks (in order):
/// 1. `$BIMO_PROMPTS_DIR/SYSTEM.md` (via [`prompts_dir`])
/// 2. `.agents/SYSTEM.md` – nearest ancestor from CWD
/// 3. `~/.agents/SYSTEM.md`
/// 4. Compiled-in default
pub fn load(name: &str) -> String {
    // 1. Prompts directory (BIMO_PROMPTS_DIR, ./prompts/, CARGO_MANIFEST_DIR/prompts/)
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

    // 2. Project-level .agents/SYSTEM.md (walk up from CWD)
    if name == SYSTEM
        && let Ok(cwd) = std::env::current_dir()
    {
        for ancestor in cwd.ancestors() {
            let path = ancestor.join(".agents").join("SYSTEM.md");
            if let Ok(content) = fs::read_to_string(&path) {
                tracing::debug!(path = %path.display(), "loaded system prompt from .agents/SYSTEM.md");
                return content;
            }
        }
    }

    // 3. User-level ~/.agents/SYSTEM.md
    if name == SYSTEM
        && let Some(home) = dirs::home_dir()
    {
        let path = home.join(".agents").join("SYSTEM.md");
        if let Ok(content) = fs::read_to_string(&path) {
            tracing::debug!(path = %path.display(), "loaded system prompt from ~/.agents/SYSTEM.md");
            return content;
        }
    }

    match name {
        COMPACT => COMPACT_DEFAULT.to_string(),
        COMPACT_PREFIX => COMPACT_PREFIX_DEFAULT.to_string(),
        SYSTEM => SYSTEM_DEFAULT.to_string(),
        _ => String::new(),
    }
}

/// Render a prompt by replacing `{{KEY}}` placeholders with values.
pub fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_nonempty_for_all_prompts() {
        let compact = load(COMPACT);
        assert!(!compact.is_empty());
        assert!(compact.contains("Summarize"));

        let prefix = load(COMPACT_PREFIX);
        assert!(!prefix.is_empty());
        assert!(prefix.contains("Previous conversation summary"));

        let system = load(SYSTEM);
        assert!(!system.is_empty());
        assert!(system.contains("Bimo"));
    }

    #[test]
    fn render_replaces_placeholders() {
        let template = "Hello {{name}}, you are {{age}} years old.";
        let result = render(template, &[("name", "Alice"), ("age", "30")]);
        assert_eq!(result, "Hello Alice, you are 30 years old.");
    }

    #[test]
    fn render_no_placeholders() {
        let template = "No placeholders here.";
        let result = render(template, &[("x", "y")]);
        assert_eq!(result, "No placeholders here.");
    }

    #[test]
    fn render_multiple_same_placeholder() {
        let template = "{{x}} and {{x}}";
        let result = render(template, &[("x", "foo")]);
        assert_eq!(result, "foo and foo");
    }

    #[test]
    fn compact_prompt_has_conversation_placeholder() {
        let template = load(COMPACT);
        assert!(template.contains("{{CONVERSATION}}"));
    }

    #[test]
    fn compact_prefix_has_summary_placeholder() {
        let template = load(COMPACT_PREFIX);
        assert!(template.contains("{{SUMMARY}}"));
    }

    #[test]
    fn system_prompt_has_tools_placeholder() {
        let template = load(SYSTEM);
        assert!(template.contains("{{TOOLS}}"));
    }

    #[test]
    fn system_prompt_has_date_placeholder() {
        let template = load(SYSTEM);
        assert!(template.contains("{{DATE}}"));
    }

    #[test]
    fn system_prompt_has_cwd_placeholder() {
        let template = load(SYSTEM);
        assert!(template.contains("{{CWD}}"));
    }

    #[test]
    fn system_prompt_has_project_context_placeholder() {
        let template = load(SYSTEM);
        assert!(template.contains("{{PROJECT_CONTEXT}}"));
    }
}
