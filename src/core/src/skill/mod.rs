use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A loaded skill with metadata and instruction content.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Directory name (e.g. `"my-skill"`).
    pub id: String,
    /// Human-readable name from frontmatter (falls back to `id`).
    pub name: String,
    /// One-line description from frontmatter.
    pub description: String,
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
                continue;
            };
            let meta: SkillMeta = match serde_yaml::from_str(&frontmatter) {
                Ok(m) => m,
                Err(_) => continue,
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
                content: body,
                path,
            });
        }
    }

    skills
}

/// Parses YAML frontmatter (delimited by `---`) and the body from markdown.
///
/// Returns `(frontmatter_yaml, body)` or `None` when no frontmatter is present.
fn parse_frontmatter(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end = after_first.find("\n---")?;
    let frontmatter = after_first[..end].trim().to_string();
    let body = after_first[end + 4..].trim().to_string();
    Some((frontmatter, body))
}

/// Formats a list of skills as compact bullet entries for prompt injection.
///
/// Returns an empty string when `skills` is empty, so the `{{SKILLS}}`
/// placeholder renders as nothing when no skills are configured.
pub fn render_skills(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for skill in skills {
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
