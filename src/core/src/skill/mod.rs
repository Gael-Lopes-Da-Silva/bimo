//! Skill loading — scans project and user directories for `SKILL.md` files,
//! parses their frontmatter, and renders them for prompt injection.

use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

/// A loaded skill with metadata and instruction content.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Directory name (e.g. `"my-skill"`).
    pub id: String,
    /// Human-readable name from frontmatter (falls back to `id`).
    pub name: String,
    /// One-line description from frontmatter.
    pub description: String,
    /// Whether the skill is active.  Set via frontmatter `enabled:` or
    /// disabled per session.
    pub enabled: bool,
    /// Instruction body — everything after the frontmatter block in `SKILL.md`.
    pub content: String,
    /// Absolute path to the skill directory (for resolving relative references).
    pub path: PathBuf,
}

/// Parsed YAML frontmatter from a `SKILL.md` file.
#[derive(Debug, Deserialize)]
struct SkillMeta {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Parses YAML frontmatter (delimited by `---`) and the body from markdown.
///
/// Returns `(frontmatter_yaml, body)` or `None` when no frontmatter is present.
fn parse_frontmatter(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = trimmed.strip_prefix("---")?;
    let rest = after_first.trim_start();
    // Find the closing delimiter on its own line
    let mut search_start = 0;
    let mut end_pos = None;
    while let Some(pos) = rest[search_start..].find("\n---") {
        let absolute_pos = search_start + pos;
        // Check that the closing delimiter is followed by newline or end of string
        let after_delim = absolute_pos + 4; // skip \n---
        if after_delim == rest.len() || rest[after_delim..].starts_with('\n') {
            end_pos = Some(absolute_pos);
            break;
        }
        search_start = absolute_pos + 1;
    }
    let end = end_pos?;
    let frontmatter = rest[..end].trim().to_string();
    let body_start = end + 4; // skip \n---
    let body = if body_start < rest.len() {
        rest[body_start..].trim_start().to_string()
    } else {
        String::new()
    };
    Some((frontmatter, body))
}

/// Scans the given base directories for skill directories containing `SKILL.md`.
///
/// Later directories take precedence (earlier skills with the same `id` are
/// skipped), so project-local skills override user-global ones.
pub fn load_skills(base_dirs: &[PathBuf]) -> Vec<Skill> {
    let mut skills = Vec::new();
    let mut seen = HashSet::new();

    for base in base_dirs {
        let skills_dir = base.join("skills");
        if !skills_dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&skills_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let id = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if id.starts_with('.') || !seen.insert(id.clone()) {
                continue;
            }
            let skill_file = path.join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&skill_file) else {
                continue;
            };
            let Some((frontmatter, body)) = parse_frontmatter(&content) else {
                warn!(
                    "Skill file {} has no frontmatter, skipping",
                    skill_file.display()
                );
                continue;
            };
            let meta: SkillMeta = match serde_yaml::from_str(&frontmatter) {
                Ok(m) => m,
                Err(e) => {
                    warn!(
                        "Failed to parse frontmatter in {}: {}",
                        skill_file.display(),
                        e
                    );
                    continue;
                }
            };
            let name = if meta.name.is_empty() {
                id.clone()
            } else {
                meta.name
            };
            skills.push(Skill {
                id,
                name,
                description: meta.description,
                enabled: meta.enabled,
                content: body,
                path,
            });
        }
    }

    skills
}

/// Formats enabled skills as compact bullet entries for prompt injection.
///
/// Skills with `enabled: false` are silently skipped.  Returns an empty
/// string when no skills are active, so the `{{SKILLS}}` placeholder
/// renders as nothing when no skills are loaded or all are disabled.
pub fn render_skills(skills: &[Skill]) -> String {
    let mut out = String::new();
    for skill in skills {
        if !skill.enabled {
            continue;
        }
        out.push_str(&format!("- **{}**: {}\n", skill.name, skill.description));
    }
    out
}

/// Returns the default search directories: project-local then user-global.
///
/// Project-local: `<project_dir>/.agents/skills`
/// User-global:   `~/.agents/skills`
pub fn default_skill_dirs(project_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(project) = project_dir {
        dirs.push(project.join(".agents").join("skills"));
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".agents").join("skills"));
    }
    dirs
}
